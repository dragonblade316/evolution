use common::{MotorCMD, MotorData};
use cu29::prelude::*;
use masterpiece::{
    MasterpieceSimStep, MasterpieceRuntime, bridges, evo_visualizer,
};
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


fn main() {
    let logger_path = "logs/cu-test.copper";
    let rec = Rc::new(rerun::RecordingStream::disabled());
    // let rec = Rc::new(
    //     rerun::RecordingStreamBuilder::new("Uzi sim")
    //         .spawn()
    //         .expect("failed to spawn rerun"),
    // );

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
    let turret_angle = Rc::new(RefCell::new(Motor::new()));
    let shooter = Rc::new(RefCell::new(Motor::new()));

    let (robot_clock, robot_clock_mock) = RobotClock::mock();

    let sim_diff_drive = diff_drive.clone();
    let sim_indexer = indexer.clone();
    let sim_turret_angle = turret_angle.clone();
    let sim_shooter = shooter.clone();
    let sim_rec = rec.clone();
    let mut visualizer = evo_visualizer().expect("failed to spawn Copper visualizer");
    let mut sim_callback = move |step: MasterpieceSimStep<'_>| -> SimOverride {
        // println!("callback called");
        match step {
            MasterpieceSimStep::CopperListCompleted(copperlist) => {
                visualizer.copperlist_completed(copperlist)
            }
            MasterpieceSimStep::MoteusTxDriveLeft { channel, msg, output } => {
                let data = msg.payload().unwrap();
                match data {
                    MotorCMD::Velocity(v, t) => {
                        let vel = v.get::<radian_per_second>();
                        // debug!("vcmd left: {vel}");
                        let _ = sim_rec.log(
                            "motors/left/vcmd",
                            &Scalars::single(vel)
                        );
                        sim_diff_drive.borrow_mut().set_left_motor(v.clone())
                    }
                    MotorCMD::Stop => {}
                    _ => error!("Non velocity cmd not supported by sim. Data: {:?}", data)
                }
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::MoteusTxDriveRight { channel, msg, output } => {
                let data = msg.payload().unwrap();
                match data {
                    MotorCMD::Velocity(v, t) => {
                        let vel = v.get::<radian_per_second>();
                        // debug!("vcmd right: {vel}");
                        let _ = sim_rec.log(
                            "motors/right/vcmd",
                            &Scalars::single(vel)
                        );
                        sim_diff_drive.borrow_mut().set_right_motor(v.clone())
                    }
                    MotorCMD::Stop => {}
                    _ => error!("Non velocity cmd not supported by sim. Data: {:?}", data)
                }

                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::MoteusRxDriveLeftData { channel, msg } => {
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
            MasterpieceSimStep::MoteusRxDriveRightData { channel, msg } => {
                let diff_data= sim_diff_drive.borrow().noisy_state();
                let payload = MoteusData {
                    canid: 2,
                    data: MotorData { pos: diff_data.right_pos, vel: diff_data.right_speed, accel: None, torque: None },
                    temp: 32.2,
                    voltage: 22.2,
                    fault: 0,
                };
                msg.set_payload(payload);
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::MoteusTxIndexer { channel, msg, output } => {
                let data = msg.payload().unwrap();
                match data {
                    MotorCMD::Velocity(v, t) => {
                        let vel = v.get::<radian_per_second>();
                        // debug!("vcmd right: {vel}");
                        let _ = sim_rec.log(
                            "motors/indexer",
                            &Scalars::single(vel)
                        );
                        sim_indexer.borrow_mut().set_vel_target(v.clone(), AngularAcceleration::new::<radian_per_second_squared>(5.0))
                    }
                    MotorCMD::Stop => {}
                    _ => error!("Non velocity cmd not supported by sim. Data: {:?}", data)
                }
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::MoteusRxTurretData { channel, msg } => {
                let motor = sim_turret_angle.borrow();
                msg.set_payload(MoteusData {
                    canid: 4,
                    data: MotorData {
                        pos: motor.pos(),
                        vel: motor.speed(),
                        accel: None,
                        torque: None,
                    },
                    temp: 32.2,
                    voltage: 22.2,
                    fault: 0,
                });
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::MoteusTxTurret { channel, msg, output } => {
                let data = msg.payload().unwrap();
                match data {
                    MotorCMD::Position(p, v, t) => {
                        let pos = p.get::<radian>();
                        // debug!("vcmd right: {pos}");
                        let _ = sim_rec.log(
                            "motors/turret",
                            &Scalars::single(pos)
                        );
                        sim_turret_angle.borrow_mut().set_pos_target(p.clone(), AngularVelocity::new::<radian_per_second>(5.0))
                    }
                    MotorCMD::Stop => {}
                    _ => error!("Non velocity cmd not supported by sim. Data: {:?}", data)
                }
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::MoteusRxShooterData { channel, msg } => {
                let motor = sim_shooter.borrow();
                msg.set_payload(MoteusData {
                    canid: 5,
                    data: MotorData {
                        pos: motor.pos(),
                        vel: motor.speed(),
                        accel: None,
                        torque: None,
                    },
                    temp: 32.2,
                    voltage: 22.2,
                    fault: 0,
                });
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::MoteusTxShooter { channel, msg, output } => {
                let data = msg.payload().unwrap();
                match data {
                    MotorCMD::Velocity(v, t) => {
                        let vel = v.get::<radian_per_second>();
                        // debug!("vcmd right: {vel}");
                        let _ = sim_rec.log(
                            "motors/flywheel",
                            &Scalars::single(vel)
                        );
                        sim_shooter.borrow_mut().set_vel_target(v.clone(), AngularAcceleration::new::<radian_per_second_squared>(5.0))
                    }
                    MotorCMD::Stop => {}
                    _ => error!("Non velocity cmd not supported by sim. Data: {:?}", data)
                }
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::Camera(CuTaskCallbackState::Process(_, output)) => {
                output.clear_payload();
                SimOverride::ExecutedBySim
            }
            MasterpieceSimStep::BinaryServo(CuTaskCallbackState::Process(input, _output)) => {
                if let Some(deployed) = input.payload() {
                    let _ = sim_rec.log(
                        "actuators/binary_servo",
                        &rerun::AnyValues::default().with_component_from_data(
                            "deployed",
                            std::sync::Arc::new(
                                rerun::external::arrow::array::BooleanArray::from(vec![*deployed]),
                            ),
                        ),
                    );
                }
                SimOverride::ExecutedBySim
            }
            _ => SimOverride::ExecuteByRuntime,
        }
    };

    debug!("Logger created at {}.", &logger_path);
    debug!("Creating application... ");
    let mut application = MasterpieceRuntime::builder()
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
        turret_angle.borrow_mut().update(timestep.as_secs_f32());

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
            &rerun::Boxes3D::from_half_sizes([[0.15, 0.20, 0.05]]),
        )
        .ok();
        // +Y is forward in the robot frame (theta = 0 drives along world +Y).
        rec.log(
            "robot/front",
            &rerun::Arrows3D::from_vectors([[0.0, 0.30, 0.0]])
                .with_origins([[0.0, 0.0, 0.08]])
                .with_radii([0.02])
                .with_colors([rerun::Color::from_rgb(255, 80, 80)])
                .with_labels(["front"]),
        )
        .ok();
        {
            let angle = turret_angle.borrow().pos().get::<radian>();
            let speed = shooter.borrow().speed().get::<radian_per_second>();
            let scale = 0.01;
            let len = speed.abs() * scale + 0.35;
            // Turret zero is robot-forward (+Y); positive angles rotate CCW.
            let dx = -len * angle.sin();
            let dy = len * angle.cos();
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
            &Scalars::single(turret_angle.borrow().pos().get::<radian>()),
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
