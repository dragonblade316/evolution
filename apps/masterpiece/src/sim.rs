mod messages;
mod tasks;

use common::{MotorCMD, MotorData};
use cu29::prelude::*;
use cu29::units::si::angle::revolution;
use cu29::units::si::angular_velocity::radian_per_second;
use cu29::units::si::f32::{Angle, AngularVelocity, Torque};
use moteus_bridge::messages::{MoteusCMD, MoteusData};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

const PREALLOCATED_STORAGE_SIZE: Option<usize> = Some(1024 * 1024 * 100);

const TIME_STEP: f32 = 1.0 / 60.0;


// AI wrote the sim code bc I am lazy
// ---------------------------------------------------------------------------
// Motor — constant acceleration toward a target speed
// ---------------------------------------------------------------------------

pub struct Motor {
    current_speed: f32, // rad/s
    target_speed: f32,  // rad/s
    acceleration: f32,  // rad/s² (always positive, direction from sign of diff)
}

impl Motor {
    pub fn new(acceleration: f32) -> Self {
        Self {
            current_speed: 0.0,
            target_speed: 0.0,
            acceleration,
        }
    }

    pub fn set_target(&mut self, speed: f32) {
        self.target_speed = speed;
    }

    pub fn speed(&self) -> f32 {
        self.current_speed
    }

    /// Step the motor toward target_speed at constant acceleration.
    /// Clamps exactly to target if the step would overshoot.
    fn update(&mut self, dt: f32) {
        let diff = self.target_speed - self.current_speed;
        if diff == 0.0 {
            return;
        }
        let max_step = self.acceleration * dt;
        if diff.abs() <= max_step {
            self.current_speed = self.target_speed;
        } else {
            self.current_speed += max_step * diff.signum();
        }
    }
}

// ---------------------------------------------------------------------------
// DiffDrive — 2-wheel differential drive kinematics
// ---------------------------------------------------------------------------

/// The true (noiseless) kinematic state of the diff-drive system.
#[derive(Debug, Clone)]
pub struct DiffDriveState {
    pub x: f32,
    pub y: f32,
    pub theta: f32,       // heading, radians
    pub left_speed: f32,  // rad/s
    pub right_speed: f32, // rad/s
}

pub struct DiffDrive {
    left_motor: Motor,
    right_motor: Motor,
    wheel_radius: f32,
    track_width: f32, // centre-to-centre distance between wheels
    /// Distance from axle to robot center. Positive = center ahead of axle.
    axle_offset: f32,
    x: f32,
    y: f32,
    theta: f32,
}

impl DiffDrive {
    pub fn new(
        wheel_radius: f32,
        track_width: f32,
        motor_acceleration: f32,
        axle_offset: f32,
    ) -> Self {
        Self {
            left_motor: Motor::new(motor_acceleration),
            right_motor: Motor::new(motor_acceleration),
            wheel_radius,
            track_width,
            axle_offset,
            x: 0.0,
            y: 0.0,
            theta: 0.0,
        }
    }

    pub fn set_wheel_targets(&mut self, left: f32, right: f32) {
        self.left_motor.set_target(left);
        self.right_motor.set_target(right);
    }

    pub fn set_wheel_target(&mut self, can_id: u8, speed: f32) {
        match can_id {
            1 => self.left_motor.set_target(speed),
            2 => self.right_motor.set_target(speed),
            _ => {}
        }
    }

    /// The exact pose and wheel speeds — internal ground truth.
    pub fn true_state(&self) -> DiffDriveState {
        DiffDriveState {
            x: self.x,
            y: self.y,
            theta: self.theta,
            left_speed: self.left_motor.speed(),
            right_speed: self.right_motor.speed(),
        }
    }

    /// State with simulated sensor noise applied.
    /// Currently returns the true state verbatim — noise model TBD.
    pub fn noisy_state(&self) -> DiffDriveState {
        self.true_state()
    }

    fn moteus_data(&self, can_id: u8, speed: f32) -> MoteusData {
        MoteusData {
            canid: can_id,
            data: MotorData {
                pos: Angle::new::<revolution>(0.0),
                vel: AngularVelocity::new::<radian_per_second>(speed),
                accel: None,
                torque: Some(Torque::new::<cu29::units::si::torque::newton_meter>(0.0)),
            },
            temp: 0.0,
            voltage: 0.0,
            fault: 0,
        }
    }

    pub fn moteus_state(&self) -> HashMap<usize, MoteusData> {
        let state = self.noisy_state();
        HashMap::from([
            (1, self.moteus_data(1, state.left_speed)),
            (2, self.moteus_data(2, state.right_speed)),
        ])
    }

    fn update(&mut self, dt: f32) {
        // 1. Advance motors
        self.left_motor.update(dt);
        self.right_motor.update(dt);

        // 2. Diff-drive kinematics at the axle.
        let vl = self.left_motor.speed() * self.wheel_radius;
        let vr = self.right_motor.speed() * self.wheel_radius;

        let v_axle = (vl + vr) / 2.0;
        let omega = (vr - vl) / self.track_width;

        // 3. Translate to robot center.  When the axle is offset from the
        //    center, a turning robot experiences a sideways velocity at the
        //    center: v_y = ω · axle_offset (robot frame).
        let v_x = v_axle * self.theta.cos() - omega * self.axle_offset * self.theta.sin();
        let v_y = v_axle * self.theta.sin() + omega * self.axle_offset * self.theta.cos();

        self.theta += omega * dt;
        self.x += v_x * dt;
        self.y += v_y * dt;
    }
}

// ---------------------------------------------------------------------------
// Simulation — top-level container for all subsystems
// ---------------------------------------------------------------------------

pub struct Simulation {
    pub diff_drive: DiffDrive,
    // Future: pub turret: Turret,
    // Future: noise config, rng, etc.
}

impl Simulation {
    pub fn new() -> Self {
        Self {
            diff_drive: DiffDrive::new(
                0.05,   // wheel_radius (m)
                0.4,    // track_width (m)
                10.0,   // motor acceleration (rad/s²)
                0.0,    // axle_offset (m) — 0 = center on axle
            ),
        }
    }

    /// Step the entire simulation forward by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        self.diff_drive.update(dt);
        // Future: self.turret.update(dt);
    }
}

// ---------------------------------------------------------------------------

#[copper_runtime(config = "copperconfig.ron", sim_mode = true)]
struct MasterpieceApplication {}

fn main() {
    let logger_path = "logs/cu-test.copper";
    if let Some(parent) = Path::new(logger_path).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).expect("Failed to create logs directory");
        }
    }

    let mut sim = Simulation::new();

    let (robot_clock, robot_clock_mock) = RobotClock::mock();

    let mut sim_callback = move |step: default::SimStep<'_>| -> SimOverride {
        match step {
            // Copper does not construct the real MoteusBridge in simulation
            // because the bridge is marked `run_in_sim: false` in the config.
            default::SimStep::MoteusBridge(
                CuBridgeLifecycleState::New(_)
                | CuBridgeLifecycleState::Start
                | CuBridgeLifecycleState::Preprocess
                | CuBridgeLifecycleState::Postprocess
                | CuBridgeLifecycleState::Stop,
            ) => SimOverride::ExecutedBySim,
            default::SimStep::MoteusRxCommand { msg, .. } => {
                if let Some(command) = msg.payload() {
                    if let MotorCMD::Velocity(speed, _) = &command.cmd {
                        sim.diff_drive.set_wheel_target(
                            command.can_id,
                            speed.get::<radian_per_second>(),
                        );
                    }
                }
                SimOverride::ExecutedBySim
            }
            default::SimStep::MoteusTxStatus { output, .. } => {
                output.set_payload(sim.diff_drive.moteus_state());
                SimOverride::ExecutedBySim
            }
            _ => SimOverride::ExecuteByRuntime,
        }
    };

    debug!("Logger created at {}.", &logger_path);
    debug!("Creating application... ");
    let mut application = MasterpieceApplication::builder()
        .with_sim_callback(&mut sim_callback)
        .with_clock(robot_clock)
        .with_log_path(&logger_path, PREALLOCATED_STORAGE_SIZE)
        .expect("Failed to setup logger.")
        .build()
        .expect("Failed to create application.");

    let _ = application.start_all_tasks(&mut sim_callback);
    loop {
        sim.update(TIME_STEP);
        robot_clock_mock.increment(Duration::from_secs_f32(TIME_STEP).into());
        application.run_one_iteration(&mut sim_callback).unwrap();
    }
}
