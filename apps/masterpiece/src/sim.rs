mod messages;
mod tasks;

use common::{MotorCMD, MotorData};
use cu29::prelude::*;
use cu29::units::si::angular_velocity::radian_per_second;
use evosim::DiffDrive;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Simulated Moteus-style telemetry. Ancillary fields are stubbed to zero.
#[derive(Debug, Clone)]
struct MoteusData {
    canid: u8,
    data: MotorData,
    temp: f32,
    voltage: f32,
    fault: i8,
}

impl Default for MoteusData {
    fn default() -> Self {
        Self {
            canid: 0,
            data: MotorData::default(),
            temp: 0.0,
            voltage: 0.0,
            fault: 0,
        }
    }
}

const PREALLOCATED_STORAGE_SIZE: Option<usize> = Some(1024 * 1024 * 100);

const TIME_STEP: f32 = 1.0 / 60.0;


// Simulation types (Motor, DiffDriveState, DiffDrive)
// now live in the `evosim` crate under external/evosim.
//
// Task / sim boundary:
//   DiffDriveCmd  → MotorCMD (velocity per wheel)  → bridge → sim consumes
//   sim produces  → MotorData (telemetry per wheel) → bridge → MoteusDiff consumes

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

    let mut diff_drive = DiffDrive::new(
        0.05,   // wheel_radius (m)
        0.4,    // track_width (m)
        10.0,   // motor acceleration (rad/s²)
        0.0,    // axle_offset (m)
    );

    let (robot_clock, robot_clock_mock) = RobotClock::mock();

    let mut sim_callback = move |step: default::SimStep<'_>| -> SimOverride {
        match step {
                    // Intercept the velocity commands flowing from DiffDriveCmd through the bridge.
                    // The task outputs MotorCMD::Velocity per wheel; the bridge wraps it in
                    // MoteusCMD with a CAN ID.  We extract the raw speed and feed the sim.
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
                        diff_drive.set_wheel_target(
                            command.can_id,
                            speed.get::<radian_per_second>(),
                        );
                    }
                }
                SimOverride::ExecutedBySim
            }
            default::SimStep::MoteusTxStatus { output, .. } => {
                let motor_states = diff_drive.motor_states();
                let moteus_states: HashMap<usize, MoteusData> = motor_states
                    .into_iter()
                    .map(|(can_id, data)| {
                        (can_id, MoteusData {
                            canid: can_id as u8,
                            data,
                            temp: 0.0,
                            voltage: 0.0,
                            fault: 0,
                        })
                    })
                    .collect();
                output.set_payload(moteus_states);
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
        diff_drive.update(TIME_STEP);
        robot_clock_mock.increment(Duration::from_secs_f32(TIME_STEP).into());
        application.run_one_iteration(&mut sim_callback).unwrap();
    }
}
