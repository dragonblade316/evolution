mod messages;
mod tasks;

use common::{MotorCMD, MotorData};
use cu29::prelude::*;
use masterpiece::bridges;
use cu29::units::si::angle::radian;
use cu29::units::si::angular_acceleration::radian_per_second_squared;
use cu29::units::si::angular_velocity::radian_per_second;
use cu29::units::si::f32::{AngularAcceleration, AngularVelocity, Length};
use cu29::units::si::length::meter;
use evosim::{DiffDrive, Motor};
use moteus_bridge::messages::MoteusData;
use rerun::Scalars;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

const PREALLOCATED_STORAGE_SIZE: Option<usize> = Some(1024 * 1024 * 100);
const TIME_STEP: f32 = 1.0 / 200.0;


#[copper_runtime(config = "copperconfig.ron", sim_mode = true)]
struct MasterpieceApplication {}

fn main() {
    let logger_path = "logs/cu-test.copper";
    let rec = Rc::new(
        rerun::RecordingStreamBuilder::new("Uzi sim")
            .spawn()
            .expect("failed to spawn rerun"),
    );

    if let Some(parent) = Path::new(logger_path).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).expect("Failed to create logs directory");
        }
    }

    let diff_drive = Rc::new(RefCell::new(DiffDrive::new(
        Length::new::<meter>(0.1),  // wheel_radius (m)
        Length::new::<meter>(0.3),  // track_width (m)
        AngularAcceleration::new::<radian_per_second_squared>(10.0), // motor acceleration
        Length::new::<meter>(0.0),  // axle_offset (m)
    )));

    let indexer = Rc::new(RefCell::new(Motor::new()));
    let turret_angle = Rc::new(RefCell::new(0.0f32));
    let shooter = Rc::new(RefCell::new(Motor::new()));

    let (robot_clock, robot_clock_mock) = RobotClock::mock();

    let sim_diff_drive = diff_drive.clone();
    let _sim_indexer = indexer.clone();
    let _sim_turret_angle = turret_angle.clone();
    let _sim_shooter = shooter.clone();
    let sim_rec = rec.clone();
    let mut sim_callback = move |step: default::SimStep<'_>| -> SimOverride {
        // println!("callback called");
        match step {
            default::SimStep::MoteusTxDriveLeft { channel, msg, output } => {
                let data = msg.payload().unwrap();
                match data {
                    MotorCMD::Velocity(v, t) => {
                        let vel = v.get::<radian_per_second>();
                        debug!("vcmd left: {vel}");
                        let _ = sim_rec.log(
                            "motors/left/vcmd",
                            &Scalars::single(vel)
                        );
                        sim_diff_drive.borrow_mut().set_left_motor(v.clone())
                    }
                    _ => error!("Non velocity cmd not supported by sim")
                }
                SimOverride::ExecutedBySim
            }
            default::SimStep::MoteusTxDriveRight { channel, msg, output } => {
                let data = msg.payload().unwrap();
                match data {
                    MotorCMD::Velocity(v, t) => {
                        let vel = v.get::<radian_per_second>();
                        debug!("vcmd right: {vel}");
                        let _ = sim_rec.log(
                            "motors/right/vcmd",
                            &Scalars::single(vel)
                        );
                        sim_diff_drive.borrow_mut().set_right_motor(v.clone())
                    }
                    _ => error!("Non velocity cmd not supported by sim")
                }

                SimOverride::ExecutedBySim
            }
            default::SimStep::MoteusRxDriveLeftData { channel, msg } => {
                let diff_data= sim_diff_drive.borrow().noisy_state();
                let payload = MoteusData {
                    canid: 1,
                    data: MotorData { pos: diff_data.left_pos, vel: diff_data.left_speed, accel: None, torque: None },
                    temp: 32.2,
                    voltage: 22.2,
                    fault: 0,
                };
                msg.set_payload(payload);
                SimOverride::ExecutedBySim
            }
            default::SimStep::MoteusRxDriveRightData { channel, msg } => {
                let diff_data= sim_diff_drive.borrow().noisy_state();
                let payload = MoteusData {
                    canid: 1,
                    data: MotorData { pos: diff_data.right_pos, vel: diff_data.right_speed, accel: None, torque: None },
                    temp: 32.2,
                    voltage: 22.2,
                    fault: 0,
                };
                msg.set_payload(payload);
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
    let period = Duration::from_secs_f32(TIME_STEP);
    let mut timestamp = Instant::now();
    loop {
        let frame_start = Instant::now();
        let timestep = timestamp.elapsed();
        timestamp = Instant::now();

        diff_drive.borrow_mut().update(timestep.as_secs_f32());
        indexer.borrow_mut().update(timestep.as_secs_f32());
        shooter.borrow_mut().update(timestep.as_secs_f32());
        robot_clock_mock.increment(timestep.into());
        application.run_one_iteration(&mut sim_callback).unwrap();
        let state = diff_drive.borrow().true_state();
        rec.log(
            "robot",
            &rerun::Transform3D::from_translation_rotation(
                [state.x.get::<meter>(), state.y.get::<meter>(), 0.0],
                rerun::Rotation3D::AxisAngle(rerun::components::RotationAxisAngle::new(
                    [0.0, 0.0, 1.0],
                    rerun::Angle::from_radians(state.theta.get::<radian>()),
                )),
            ),
        )
        .ok();
        rec.log(
            "robot/body",
            &rerun::Boxes3D::from_half_sizes([[0.20, 0.15, 0.05]]),
        )
        .ok();
        // +X is forward in the robot frame (theta = 0 drives along world +X).
        rec.log(
            "robot/front",
            &rerun::Arrows3D::from_vectors([[0.30, 0.0, 0.0]])
                .with_origins([[0.0, 0.0, 0.08]])
                .with_radii([0.02])
                .with_colors([rerun::Color::from_rgb(255, 80, 80)])
                .with_labels(["front"]),
        )
        .ok();
        {
            let angle = *turret_angle.borrow();
            let speed = shooter.borrow().speed().get::<radian_per_second>();
            let scale = 0.01;
            let len = speed.abs() * scale + 0.08;
            let dx = len * angle.cos();
            let dy = len * angle.sin();
            rec.log(
                "robot/turret",
                &rerun::Arrows3D::from_vectors([[dx, dy, 0.0]])
                    .with_origins([[0.0, 0.0, 0.12]])
                    .with_radii([0.015])
                    .with_colors([rerun::Color::from_rgb(80, 80, 255)])
                    .with_labels(["turret"]),
            )
            .ok();
        }
        rec.log(
            "motors/left/vel",
            &Scalars::single(state.left_speed.get::<radian_per_second>()),
        )
        .ok();
        rec.log(
            "motors/right/vel",
            &Scalars::single(state.right_speed.get::<radian_per_second>()),
        )
        .ok();
        rec.log(
            "motors/indexer/vel",
            &Scalars::single(indexer.borrow().speed().get::<radian_per_second>()),
        )
        .ok();
        rec.log(
            "motors/turret/angle",
            &Scalars::single(*turret_angle.borrow()),
        )
        .ok();
        rec.log(
            "motors/shooter/vel",
            &Scalars::single(shooter.borrow().speed().get::<radian_per_second>()),
        )
        .ok();

        if let Some(remaining) = period.checked_sub(frame_start.elapsed()) {
            std::thread::sleep(remaining);
        }


    }
}
