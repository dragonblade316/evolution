use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use moteus::{
    BlockingController, Controller, Resolution,
    command::PositionCommand,
    diagnostic::DiagnosticStream,
    query::{QueryFormat, QueryResult},
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

const WRITE_INTERVAL: Duration = Duration::from_millis(50);
const POSITION_LIMIT_REV: f32 = 1000.0;
const VELOCITY_LIMIT_REV_S: f32 = 500.0;

#[derive(Clone)]
struct AppState {
    commands: mpsc::Sender<ClientMessage>,
    events: broadcast::Sender<ServerMessage>,
    client_connected: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ControlMode {
    Position,
    Velocity,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    AddMotor {
        can_id: u8,
    },
    RemoveMotor {
        motor_id: u64,
    },
    Reconnect {
        motor_id: u64,
    },
    ChangeCanId {
        motor_id: u64,
        can_id: u8,
    },
    SetTarget {
        motor_id: u64,
        mode: ControlMode,
        position: f32,
        velocity: f32,
        maximum_torque: Option<f32>,
    },
    ApplyPid {
        motor_id: u64,
        kp: f32,
        ki: f32,
        kd: f32,
    },
    ApplyMotorConfiguration {
        motor_id: u64,
        inverted: bool,
        gear_ratio: f32,
        position_min: Option<f32>,
        position_max: Option<f32>,
    },
    ApplySafetyConfiguration {
        motor_id: u64,
        max_current: f32,
        max_velocity: f32,
        default_velocity_limit: Option<f32>,
        default_accel_limit: Option<f32>,
        max_regen_power: Option<f32>,
    },
    StartAll,
    StopAll,
    ExportConfigs,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    Snapshot {
        stopped: bool,
        motors: Vec<MotorView>,
    },
    Export {
        files: Vec<ExportFile>,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
struct ExportFile {
    name: String,
    contents: String,
}

#[derive(Debug, Clone, Serialize)]
struct MotorView {
    id: u64,
    label: String,
    can_id: u8,
    connection: String,
    connection_error: Option<String>,
    control_mode: ControlMode,
    position: f32,
    velocity: f32,
    maximum_torque: Option<f32>,
    config: MotorConfiguration,
    config_revision: u64,
    telemetry: Option<Telemetry>,
    telemetry_age_ms: Option<u64>,
    last_written_position: Option<f32>,
    last_written_velocity: Option<f32>,
    last_written_kp: Option<f32>,
    last_written_ki: Option<f32>,
    last_written_kd: Option<f32>,
    pid_status: Option<String>,
    motor_config_status: Option<String>,
    safety_status: Option<String>,
    safety_configuration_loaded: bool,
    supports_velocity_limit: bool,
    supports_accel_limit: bool,
    supports_regen_limit: bool,
}

#[derive(Debug, Clone, Serialize)]
struct MotorConfiguration {
    kp: f32,
    ki: f32,
    kd: f32,
    inverted: bool,
    gear_ratio: f32,
    position_min: Option<f32>,
    position_max: Option<f32>,
    max_current: f32,
    max_velocity: f32,
    default_velocity_limit: Option<f32>,
    default_accel_limit: Option<f32>,
    max_regen_power: Option<f32>,
}

impl Default for MotorConfiguration {
    fn default() -> Self {
        Self {
            kp: 4.0,
            ki: 0.0,
            kd: 0.1,
            inverted: false,
            gear_ratio: 1.0,
            position_min: Some(-POSITION_LIMIT_REV),
            position_max: Some(POSITION_LIMIT_REV),
            max_current: 0.0,
            max_velocity: 0.0,
            default_velocity_limit: None,
            default_accel_limit: None,
            max_regen_power: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Telemetry {
    position: Option<f32>,
    velocity: Option<f32>,
    torque: Option<f32>,
    current: Option<f32>,
    voltage: Option<f32>,
    temperature: Option<f32>,
    mode: String,
    fault: i8,
}

impl From<QueryResult> for Telemetry {
    fn from(value: QueryResult) -> Self {
        Self {
            position: finite(value.position),
            velocity: finite(value.velocity),
            torque: finite(value.torque),
            current: finite(value.q_current),
            voltage: finite(value.voltage),
            temperature: finite(value.temperature),
            mode: format!("{:?}", value.mode),
            fault: value.fault,
        }
    }
}

struct Motor {
    id: u64,
    label: String,
    can_id: u8,
    connection: Connection,
    control_mode: ControlMode,
    position: f32,
    velocity: f32,
    maximum_torque: Option<f32>,
    config: MotorConfiguration,
    config_revision: u64,
    telemetry: Option<Telemetry>,
    telemetry_updated: Option<Instant>,
    last_write: Instant,
    last_written_position: Option<f32>,
    last_written_velocity: Option<f32>,
    last_written_kp: Option<f32>,
    last_written_ki: Option<f32>,
    last_written_kd: Option<f32>,
    pid_status: Option<String>,
    motor_config_status: Option<String>,
    safety_status: Option<String>,
    safety_configuration_loaded: bool,
    supports_velocity_limit: bool,
    supports_accel_limit: bool,
    supports_regen_limit: bool,
    connection_generation: u64,
}

enum Connection {
    Disconnected,
    Connecting,
    Connected(BlockingController),
    Error(String),
}

impl Motor {
    fn new(id: u64, label: String, can_id: u8) -> Self {
        Self {
            id,
            label,
            can_id,
            connection: Connection::Disconnected,
            control_mode: ControlMode::Position,
            position: 0.0,
            velocity: 0.0,
            maximum_torque: None,
            config: MotorConfiguration::default(),
            config_revision: 0,
            telemetry: None,
            telemetry_updated: None,
            last_write: Instant::now(),
            last_written_position: None,
            last_written_velocity: None,
            last_written_kp: None,
            last_written_ki: None,
            last_written_kd: None,
            pid_status: None,
            motor_config_status: None,
            safety_status: None,
            safety_configuration_loaded: false,
            supports_velocity_limit: false,
            supports_accel_limit: false,
            supports_regen_limit: false,
            connection_generation: 0,
        }
    }

    fn apply_loaded_configuration(&mut self, bytes: &[u8]) {
        let config = String::from_utf8_lossy(bytes);
        let output_sign = config_f32(&config, "motor_position.output.sign");
        let gear_ratio = config_f32(&config, "motor_position.rotor_to_output_ratio");
        let position_min = config_f32(&config, "servopos.position_min");
        let position_max = config_f32(&config, "servopos.position_max");

        if let Some(value) = output_sign {
            self.config.inverted = value < 0.0;
        }
        if let Some(value) = gear_ratio.filter(|value| value.is_finite() && *value > 0.0) {
            self.config.gear_ratio = value;
        }
        if let Some(value) = position_min {
            self.config.position_min = value.is_finite().then_some(value);
        }
        if let Some(value) = position_max {
            self.config.position_max = value.is_finite().then_some(value);
        }
        self.motor_config_status = Some(
            if output_sign.is_some()
                && gear_ratio.is_some()
                && position_min.is_some()
                && position_max.is_some()
            {
                "Motor configuration loaded from controller"
            } else {
                "Some motor configuration values were unavailable; defaults retained"
            }
            .into(),
        );

        let kp = config_f32(&config, "servo.pid_position.kp");
        let ki = config_f32(&config, "servo.pid_position.ki");
        let kd = config_f32(&config, "servo.pid_position.kd");
        if let Some(value) = kp.filter(|value| value.is_finite()) {
            self.config.kp = value;
        }
        if let Some(value) = ki.filter(|value| value.is_finite()) {
            self.config.ki = value;
        }
        if let Some(value) = kd.filter(|value| value.is_finite()) {
            self.config.kd = value;
        }
        self.pid_status = Some(
            if kp.is_some() && ki.is_some() && kd.is_some() {
                "PID settings loaded from controller"
            } else {
                "Some PID settings were unavailable; defaults retained"
            }
            .into(),
        );

        let max_current = config_f32(&config, "servo.max_current_A");
        let max_velocity = config_f32(&config, "servo.max_velocity");
        let velocity_limit = config_f32(&config, "servo.default_velocity_limit");
        let accel_limit = config_f32(&config, "servo.default_accel_limit");
        let max_regen_power = config_f32(&config, "servo.max_regen_power_W");

        if let Some(value) = max_current.filter(|value| value.is_finite() && *value > 0.0) {
            self.config.max_current = value;
        }
        if let Some(value) = max_velocity.filter(|value| value.is_finite() && *value > 0.0) {
            self.config.max_velocity = value;
        }
        self.config.default_velocity_limit = velocity_limit.filter(|value| value.is_finite());
        self.config.default_accel_limit = accel_limit.filter(|value| value.is_finite());
        self.config.max_regen_power = max_regen_power.filter(|value| value.is_finite());
        self.supports_velocity_limit = velocity_limit.is_some();
        self.supports_accel_limit = accel_limit.is_some();
        self.supports_regen_limit = max_regen_power.is_some();

        self.safety_configuration_loaded = max_current
            .is_some_and(|value| value.is_finite() && value > 0.0)
            && max_velocity.is_some_and(|value| value.is_finite() && value > 0.0);
        self.safety_status = Some(
            if self.safety_configuration_loaded
                && velocity_limit.is_some()
                && accel_limit.is_some()
                && max_regen_power.is_some()
            {
                "Safety settings loaded from controller"
            } else if self.safety_configuration_loaded {
                "Some safety settings were unavailable; defaults retained"
            } else {
                "Current or velocity limits could not be loaded; safety editing disabled"
            }
            .into(),
        );
        self.config_revision += 1;
    }

    fn stop(&mut self) {
        if let Connection::Connected(controller) = &mut self.connection {
            let _ = controller.set_stop_no_query();
        }
    }

    fn tick(&mut self, stopped: bool) {
        let controller = match &mut self.connection {
            Connection::Connected(controller) => controller,
            _ => return,
        };

        let result = if stopped {
            controller.query()
        } else if self.last_write.elapsed() >= WRITE_INTERVAL {
            let position = match self.control_mode {
                ControlMode::Position => {
                    self.position.clamp(-POSITION_LIMIT_REV, POSITION_LIMIT_REV)
                }
                ControlMode::Velocity => f32::NAN,
            };
            let velocity = self
                .velocity
                .clamp(-VELOCITY_LIMIT_REV_S, VELOCITY_LIMIT_REV_S);
            let mut command = PositionCommand::default()
                .position(position)
                .velocity(velocity)
                .kp_scale(1.0)
                .kd_scale(1.0);
            if let Some(maximum_torque) = self.maximum_torque {
                command = command.maximum_torque(maximum_torque.max(0.0));
            }
            let result = controller.set_position(command);
            if result.is_ok() {
                self.last_written_position =
                    (self.control_mode == ControlMode::Position).then_some(position);
                self.last_written_velocity = Some(velocity);
                self.last_write = Instant::now();
            }
            result
        } else {
            return;
        };

        match result {
            Ok(telemetry) => {
                self.telemetry = Some(telemetry.into());
                self.telemetry_updated = Some(Instant::now());
            }
            Err(error) => {
                self.connection = Connection::Error(format!(
                    "CAN ID {} communication failed: {error}",
                    self.can_id
                ));
            }
        }
    }

    fn view(&self) -> MotorView {
        let (connection, connection_error) = match &self.connection {
            Connection::Disconnected => ("Disconnected", None),
            Connection::Connecting => ("Connecting...", None),
            Connection::Connected(_) => ("Connected", None),
            Connection::Error(error) => ("Error", Some(error.clone())),
        };
        MotorView {
            id: self.id,
            label: self.label.clone(),
            can_id: self.can_id,
            connection: connection.into(),
            connection_error,
            control_mode: self.control_mode,
            position: self.position,
            velocity: self.velocity,
            maximum_torque: self.maximum_torque,
            config: self.config.clone(),
            config_revision: self.config_revision,
            telemetry: self.telemetry.clone(),
            telemetry_age_ms: self
                .telemetry_updated
                .map(|updated| updated.elapsed().as_millis() as u64),
            last_written_position: self.last_written_position,
            last_written_velocity: self.last_written_velocity,
            last_written_kp: self.last_written_kp,
            last_written_ki: self.last_written_ki,
            last_written_kd: self.last_written_kd,
            pid_status: self.pid_status.clone(),
            motor_config_status: self.motor_config_status.clone(),
            safety_status: self.safety_status.clone(),
            safety_configuration_loaded: self.safety_configuration_loaded,
            supports_velocity_limit: self.supports_velocity_limit,
            supports_accel_limit: self.supports_accel_limit,
            supports_regen_limit: self.supports_regen_limit,
        }
    }
}

struct ConnectionResult {
    motor_id: u64,
    generation: u64,
    result: Result<(BlockingController, Vec<u8>), String>,
}

struct MotorWorker {
    motors: Vec<Motor>,
    stopped: bool,
    next_motor_id: u64,
    motor_counter: u64,
    connection_sender: mpsc::Sender<ConnectionResult>,
    connection_receiver: mpsc::Receiver<ConnectionResult>,
    events: broadcast::Sender<ServerMessage>,
}

impl MotorWorker {
    fn run(commands: mpsc::Receiver<ClientMessage>, events: broadcast::Sender<ServerMessage>) {
        let (connection_sender, connection_receiver) = mpsc::channel();
        let mut worker = Self {
            motors: Vec::new(),
            stopped: true,
            next_motor_id: 1,
            motor_counter: 0,
            connection_sender,
            connection_receiver,
            events,
        };

        loop {
            while let Ok(result) = worker.connection_receiver.try_recv() {
                worker.finish_connection(result);
            }
            while let Ok(command) = commands.try_recv() {
                worker.handle(command);
            }
            for motor in &mut worker.motors {
                motor.tick(worker.stopped);
            }
            worker.publish_snapshot();
            thread::sleep(WRITE_INTERVAL);
        }
    }

    fn handle(&mut self, command: ClientMessage) {
        match command {
            ClientMessage::AddMotor { can_id } => self.add_motor(can_id),
            ClientMessage::RemoveMotor { motor_id } => {
                if let Some(index) = self.motors.iter().position(|motor| motor.id == motor_id) {
                    self.motors[index].stop();
                    self.motors.remove(index);
                }
            }
            ClientMessage::Reconnect { motor_id } => self.start_connection(motor_id),
            ClientMessage::ChangeCanId { motor_id, can_id } => {
                if !(1..=127).contains(&can_id)
                    || self
                        .motors
                        .iter()
                        .any(|motor| motor.id != motor_id && motor.can_id == can_id)
                {
                    return;
                }
                if let Some(motor) = self.motor_mut(motor_id) {
                    motor.stop();
                    motor.connection = Connection::Disconnected;
                    motor.can_id = can_id;
                    motor.label = format!("Motor {} (ID {})", motor.id, can_id);
                }
                self.start_connection(motor_id);
            }
            ClientMessage::SetTarget {
                motor_id,
                mode,
                position,
                velocity,
                maximum_torque,
            } => {
                if let Some(motor) = self.motor_mut(motor_id) {
                    if position.is_finite() && velocity.is_finite() {
                        motor.control_mode = mode;
                        motor.position = position;
                        motor.velocity = velocity;
                        motor.maximum_torque = maximum_torque.filter(|value| value.is_finite());
                    }
                }
            }
            ClientMessage::ApplyPid {
                motor_id,
                kp,
                ki,
                kd,
            } => self.apply_pid(motor_id, kp, ki, kd),
            ClientMessage::ApplyMotorConfiguration {
                motor_id,
                inverted,
                gear_ratio,
                position_min,
                position_max,
            } => self.apply_motor_configuration(
                motor_id,
                inverted,
                gear_ratio,
                position_min,
                position_max,
            ),
            ClientMessage::ApplySafetyConfiguration {
                motor_id,
                max_current,
                max_velocity,
                default_velocity_limit,
                default_accel_limit,
                max_regen_power,
            } => self.apply_safety_configuration(
                motor_id,
                max_current,
                max_velocity,
                default_velocity_limit,
                default_accel_limit,
                max_regen_power,
            ),
            ClientMessage::StartAll => {
                self.stopped = false;
                let last_write = Instant::now()
                    .checked_sub(WRITE_INTERVAL)
                    .unwrap_or_else(Instant::now);
                for motor in &mut self.motors {
                    motor.last_write = last_write;
                }
            }
            ClientMessage::StopAll => self.stop_all(),
            ClientMessage::ExportConfigs => self.export_configs(),
        }
        self.publish_snapshot();
    }

    fn add_motor(&mut self, can_id: u8) {
        if !(1..=127).contains(&can_id) || self.motors.iter().any(|motor| motor.can_id == can_id) {
            return;
        }
        self.motor_counter += 1;
        let id = self.next_motor_id;
        self.next_motor_id += 1;
        self.motors.push(Motor::new(
            id,
            format!("Motor {} (ID {})", self.motor_counter, can_id),
            can_id,
        ));
        self.start_connection(id);
    }

    fn start_connection(&mut self, motor_id: u64) {
        let Some(motor) = self.motor_mut(motor_id) else {
            return;
        };
        motor.connection = Connection::Connecting;
        motor.safety_configuration_loaded = false;
        motor.connection_generation += 1;
        let generation = motor.connection_generation;
        let can_id = motor.can_id;
        let sender = self.connection_sender.clone();
        thread::spawn(move || {
            let result = connect_controller(can_id);
            let _ = sender.send(ConnectionResult {
                motor_id,
                generation,
                result,
            });
        });
    }

    fn finish_connection(&mut self, result: ConnectionResult) {
        let Some(motor) = self.motor_mut(result.motor_id) else {
            return;
        };
        if motor.connection_generation != result.generation {
            return;
        }
        match result.result {
            Ok((controller, config)) => {
                motor.apply_loaded_configuration(&config);
                motor.connection = Connection::Connected(controller);
                motor.last_write = Instant::now()
                    .checked_sub(WRITE_INTERVAL)
                    .unwrap_or_else(Instant::now);
            }
            Err(error) => motor.connection = Connection::Error(error),
        }
    }

    fn motor_mut(&mut self, id: u64) -> Option<&mut Motor> {
        self.motors.iter_mut().find(|motor| motor.id == id)
    }

    fn stop_all(&mut self) {
        self.stopped = true;
        for motor in &mut self.motors {
            motor.stop();
        }
    }

    fn apply_pid(&mut self, id: u64, kp: f32, ki: f32, kd: f32) {
        let Some(motor) = self.motor_mut(id) else {
            return;
        };
        if [kp, ki, kd]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            motor.pid_status =
                Some("PID update failed: gains must be finite and non-negative".into());
            return;
        }
        let Connection::Connected(controller) = &mut motor.connection else {
            return;
        };
        motor.pid_status = Some("Saving PID configuration...".into());
        let result = (|| -> Result<String, moteus::Error> {
            let mut stream = DiagnosticStream::new(controller);
            stream.write_message(b"tel stop")?;
            stream.flush_read()?;
            stream.command(b"d stop")?;
            let old_kp = stream.command_oneline(b"conf get servo.pid_position.kp")?;
            let old_kp = String::from_utf8_lossy(&old_kp).trim().to_owned();
            stream.command(format!("conf set servo.pid_position.kp {kp}").as_bytes())?;
            stream.command(format!("conf set servo.pid_position.ki {ki}").as_bytes())?;
            stream.command(format!("conf set servo.pid_position.kd {kd}").as_bytes())?;
            stream.command(b"conf set servo.pid_position.ilimit 0.0")?;
            stream.command(b"conf write")?;
            Ok(old_kp)
        })();
        match result {
            Ok(old_kp) => {
                motor.config.kp = kp;
                motor.config.ki = ki;
                motor.config.kd = kd;
                motor.config_revision += 1;
                motor.last_written_kp = Some(kp);
                motor.last_written_ki = Some(ki);
                motor.last_written_kd = Some(kd);
                motor.pid_status = Some(format!("PID saved (old kp: {old_kp})"));
            }
            Err(error) => motor.pid_status = Some(format!("PID update failed: {error}")),
        }
    }

    fn apply_motor_configuration(
        &mut self,
        id: u64,
        inverted: bool,
        gear_ratio: f32,
        position_min: Option<f32>,
        position_max: Option<f32>,
    ) {
        let Some(motor) = self.motor_mut(id) else {
            return;
        };
        let invalid_limit = |value: Option<f32>| value.is_some_and(|value| !value.is_finite());
        if !gear_ratio.is_finite() || gear_ratio <= 0.0 {
            motor.motor_config_status =
                Some("Configuration update failed: gearing must be greater than zero".into());
            return;
        }
        if invalid_limit(position_min) || invalid_limit(position_max) {
            motor.motor_config_status =
                Some("Configuration update failed: position limits must be finite".into());
            return;
        }
        if position_min
            .zip(position_max)
            .is_some_and(|(min, max)| min >= max)
        {
            motor.motor_config_status = Some(
                "Configuration update failed: position minimum must be less than maximum".into(),
            );
            return;
        }
        let Connection::Connected(controller) = &mut motor.connection else {
            return;
        };
        motor.motor_config_status = Some("Saving motor configuration...".into());
        let output_sign = if inverted { -1 } else { 1 };
        let min = config_value(position_min);
        let max = config_value(position_max);
        let result = (|| -> Result<(), moteus::Error> {
            let mut stream = DiagnosticStream::new(controller);
            stream.write_message(b"tel stop")?;
            stream.flush_read()?;
            stream.command(b"d stop")?;
            stream
                .command(format!("conf set motor_position.output.sign {output_sign}").as_bytes())?;
            stream.command(
                format!("conf set motor_position.rotor_to_output_ratio {gear_ratio}").as_bytes(),
            )?;
            stream.command(format!("conf set servopos.position_min {min}").as_bytes())?;
            stream.command(format!("conf set servopos.position_max {max}").as_bytes())?;
            stream.command(b"conf write")?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                motor.config.inverted = inverted;
                motor.config.gear_ratio = gear_ratio;
                motor.config.position_min = position_min;
                motor.config.position_max = position_max;
                motor.config_revision += 1;
                motor.motor_config_status = Some("Motor configuration saved to flash".into());
            }
            Err(error) => {
                motor.motor_config_status = Some(format!("Configuration update failed: {error}"));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_safety_configuration(
        &mut self,
        id: u64,
        max_current: f32,
        max_velocity: f32,
        velocity_limit: Option<f32>,
        accel_limit: Option<f32>,
        regen_limit: Option<f32>,
    ) {
        let Some(motor) = self.motor_mut(id) else {
            return;
        };
        if !motor.safety_configuration_loaded {
            motor.safety_status =
                Some("Safety settings must be loaded before they can be applied".into());
            return;
        }
        if !max_current.is_finite() || max_current <= 0.0 {
            motor.safety_status =
                Some("Safety update failed: maximum current must be greater than zero".into());
            return;
        }
        if !max_velocity.is_finite() || max_velocity <= 0.0 {
            motor.safety_status =
                Some("Safety update failed: maximum velocity must be greater than zero".into());
            return;
        }
        if velocity_limit.is_some_and(|value| !value.is_finite() || value <= 0.0) {
            motor.safety_status = Some(
                "Safety update failed: default velocity limit must be greater than zero".into(),
            );
            return;
        }
        if accel_limit.is_some_and(|value| !value.is_finite() || value <= 0.0) {
            motor.safety_status = Some(
                "Safety update failed: default acceleration limit must be greater than zero".into(),
            );
            return;
        }
        if regen_limit.is_some_and(|value| !value.is_finite() || value < 0.0) {
            motor.safety_status =
                Some("Safety update failed: regenerative power limit cannot be negative".into());
            return;
        }
        let Connection::Connected(controller) = &mut motor.connection else {
            return;
        };
        motor.safety_status = Some("Saving safety configuration...".into());
        let supports_velocity_limit = motor.supports_velocity_limit;
        let supports_accel_limit = motor.supports_accel_limit;
        let supports_regen_limit = motor.supports_regen_limit;
        let result = (|| -> Result<(), String> {
            let mut stream = DiagnosticStream::new(controller);
            stream
                .write_message(b"tel stop")
                .map_err(|error| format!("tel stop: {error}"))?;
            stream
                .flush_read()
                .map_err(|error| format!("flush diagnostic stream: {error}"))?;
            stream
                .command(b"d stop")
                .map_err(|error| format!("d stop: {error}"))?;

            let mut set = |key: &str, value: String| -> Result<(), String> {
                stream
                    .command(format!("conf set {key} {value}").as_bytes())
                    .map_err(|error| format!("{key}={value}: {error}"))?;
                Ok(())
            };
            set("servo.max_current_A", max_current.to_string())?;
            set("servo.max_velocity", max_velocity.to_string())?;
            if supports_velocity_limit {
                set("servo.default_velocity_limit", config_value(velocity_limit))?;
            }
            if supports_accel_limit {
                set("servo.default_accel_limit", config_value(accel_limit))?;
            }
            if supports_regen_limit {
                set("servo.max_regen_power_W", config_value(regen_limit))?;
            }
            drop(set);
            stream
                .command(b"conf write")
                .map_err(|error| format!("conf write: {error}"))?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                motor.config.max_current = max_current;
                motor.config.max_velocity = max_velocity;
                if motor.supports_velocity_limit {
                    motor.config.default_velocity_limit = velocity_limit;
                }
                if motor.supports_accel_limit {
                    motor.config.default_accel_limit = accel_limit;
                }
                if motor.supports_regen_limit {
                    motor.config.max_regen_power = regen_limit;
                }
                motor.config_revision += 1;
                motor.safety_status = Some("Safety configuration saved to flash".into());
            }
            Err(error) => {
                motor.safety_status = Some(format!("Safety update failed: {error}"));
            }
        }
    }

    fn export_configs(&mut self) {
        let mut files = Vec::new();
        for motor in &mut self.motors {
            let Connection::Connected(controller) = &mut motor.connection else {
                continue;
            };
            match enumerate_config(controller) {
                Ok(config) => files.push(ExportFile {
                    name: format!("moteus-ID-{}.cfg", motor.can_id),
                    contents: String::from_utf8_lossy(&config).into_owned(),
                }),
                Err(error) => {
                    motor.motor_config_status =
                        Some(format!("Configuration export failed: {error}"));
                }
            }
        }
        if files.is_empty() {
            let _ = self.events.send(ServerMessage::Error {
                message: "No connected motors to export".into(),
            });
        } else {
            let _ = self.events.send(ServerMessage::Export { files });
        }
    }

    fn publish_snapshot(&self) {
        let _ = self.events.send(ServerMessage::Snapshot {
            stopped: self.stopped,
            motors: self.motors.iter().map(Motor::view).collect(),
        });
    }
}

fn finite(value: f32) -> Option<f32> {
    value.is_finite().then_some(value)
}

fn config_f32(config: &str, key: &str) -> Option<f32> {
    config.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if parts.next()? != key {
            return None;
        }
        let value = parts.next()?;
        if value.eq_ignore_ascii_case("nan") {
            Some(f32::NAN)
        } else {
            value.parse().ok()
        }
    })
}

fn config_value(value: Option<f32>) -> String {
    value.map_or_else(|| "nan".into(), |value| value.to_string())
}

fn connect_controller(can_id: u8) -> Result<(BlockingController, Vec<u8>), String> {
    let mut query_format = QueryFormat::default();
    query_format.q_current = Resolution::Float;
    let mut controller =
        BlockingController::with_controller(Controller::new(can_id).query_format(query_format))
            .map_err(|error| format!("CAN ID {can_id} connect failed: {error}"))?;
    let config = enumerate_config(&mut controller).map_err(|error| error.to_string())?;
    Ok((controller, config))
}

fn enumerate_config(controller: &mut BlockingController) -> Result<Vec<u8>, moteus::Error> {
    let mut stream = DiagnosticStream::new(controller);
    stream.write_message(b"tel stop")?;
    stream.flush_read()?;
    stream.command(b"conf enumerate")
}

async fn index() -> Response {
    asset(
        "text/html; charset=utf-8",
        include_str!("../web/index.html"),
    )
}

async fn javascript() -> Response {
    asset(
        "text/javascript; charset=utf-8",
        include_str!("../web/app.js"),
    )
}

async fn stylesheet() -> Response {
    asset("text/css; charset=utf-8", include_str!("../web/style.css"))
}

fn asset(content_type: &'static str, contents: &'static str) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        contents,
    )
        .into_response()
}

async fn websocket_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket(socket, state))
}

async fn websocket(socket: WebSocket, state: AppState) {
    if state
        .client_connected
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        let (mut sender, _) = socket.split();
        let message = serde_json::to_string(&ServerMessage::Error {
            message: "Another motctl browser is already connected".into(),
        })
        .unwrap();
        let _ = sender.send(Message::Text(message.into())).await;
        let _ = sender.send(Message::Close(None)).await;
        return;
    }

    let (mut sender, mut receiver) = socket.split();
    let mut events = state.events.subscribe();

    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(command) => {
                                if state.commands.send(command).is_err() {
                                    break;
                                }
                            }
                            Err(error) => {
                                let message = serde_json::to_string(&ServerMessage::Error {
                                    message: format!("Invalid command: {error}"),
                                }).unwrap();
                                if sender.send(Message::Text(message.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        let message = serde_json::to_string(&event).unwrap();
                        if sender.send(Message::Text(message.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    let _ = state.commands.send(ClientMessage::StopAll);
    state.client_connected.store(false, Ordering::Release);
}

#[tokio::main]
async fn main() {
    let (command_sender, command_receiver) = mpsc::channel();
    let (event_sender, _) = broadcast::channel(16);
    let worker_events = event_sender.clone();
    thread::spawn(move || MotorWorker::run(command_receiver, worker_events));

    let state = AppState {
        commands: command_sender,
        events: event_sender,
        client_connected: Arc::new(AtomicBool::new(false)),
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/app.js", get(javascript))
        .route("/style.css", get(stylesheet))
        .route("/ws", get(websocket_upgrade))
        .with_state(state);

    let address = std::env::var("MOTCTL_ADDRESS").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("failed to bind motctl to {address}: {error}"));
    println!("motctl listening on http://{address}");
    axum::serve(listener, app)
        .await
        .expect("motctl web server failed");
}
