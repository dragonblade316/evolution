pub mod messages;
pub mod tasks;

#[cfg(feature = "sim-debug")]
use cu29::prelude::*;
#[cfg(feature = "sim-debug")]
use std::time::{Duration, Instant};

//This file exists mostly to configure the moteus and zenoh bridges. The reason it is a seperate file is bc these defs will be needed for both the sim.rs and main.rs apps
pub mod bridges {
    use cu29::prelude::*;
    use cu_zenoh_bridge::ZenohBridge;
    use moteus_bridge::MoteusBridge;

    // drive_left => common::MotorCMD = "moteus/drive_left/cmd",

tx_channels! {
        pub struct MoteusTxChannels : MoteusTxId {
            drive_left => common::MotorCMD = "moteus/drive_left/cmd",
            drive_right => common::MotorCMD = "moteus/drive_right/cmd",
            indexer => common::MotorCMD = "moteus/indexer/cmd",
            turret => common::MotorCMD = "moteus/turret/cmd",
            shooter => common::MotorCMD = "moteus/shooter/cmd",
        }
    }

rx_channels! {
        pub struct MoteusRxChannels : MoteusRxId {
            drive_left_data => moteus_bridge::messages::MoteusData = "moteus/drive_left/data",
            drive_right_data => moteus_bridge::messages::MoteusData = "moteus/drive_right/data",
            indexer_data => moteus_bridge::messages::MoteusData = "moteus/indexer/data",
            turret_data => moteus_bridge::messages::MoteusData = "moteus/turret/data",
            shooter_data => moteus_bridge::messages::MoteusData = "moteus/shooter/data",
        }
    }

    pub type UziMoteusBridge = MoteusBridge<MoteusTxChannels, MoteusRxChannels>;

    tx_channels! {
        pub struct EvoDsTxChannels : EvoDsTxId {
            telemetry => cu_spatial_payloads::Pose<f64> = "evods/tel/pose",
        }
    }

    rx_channels! {
        pub struct EvoDsRxChannels : EvoDsRxId {
            command => evods::cu29::payloads::DsTxMsg = "evods/ds",
        }
    }

    pub type EvoDsBridge = ZenohBridge<EvoDsTxChannels, EvoDsRxChannels>;
}

#[cfg(test)]
mod tests {
    use cu29::prelude::TypePath;
    use cu29::units::si::f32::{Angle, Velocity};

    fn assert_type_path<T: TypePath>() {}

    #[test]
    fn copper_unit_payloads_have_type_paths() {
        assert_type_path::<Angle>();
        assert_type_path::<Velocity>();
    }
}

#[cfg(feature = "sim-debug")]
#[copper_runtime(config = "copperconfig.ron", sim_mode = true)]
struct MasterpieceSimulation {}

#[cfg(feature = "sim-debug")]
pub use default::{
    CuStampedDataSet as MasterpieceDataSet,
    MasterpieceSimulation as MasterpieceRuntime,
    SimStep as MasterpieceSimStep,
    recorded_replay_step as masterpiece_recorded_replay_step,
};

#[cfg(feature = "sim-debug")]
pub use pc::{
    CuStampedDataSet as MasterpiecePcDataSet,
    MasterpieceSimulation as MasterpiecePcRuntime,
};

#[cfg(feature = "sim-debug")]
pub struct MasterpieceVisualizer {
    rerun: rerun::RecordingStream,
    last_visualization: Option<Instant>,
    last_drive_faults: [Option<i8>; 2],
    last_drive_modes: [Option<i8>; 2],
}

#[cfg(feature = "sim-debug")]
impl MasterpieceVisualizer {
    fn spawn() -> CuResult<Self> {
        let rerun = rerun::RecordingStreamBuilder::new("masterpiece")
            .spawn()
            .map_err(|error| CuError::new_with_cause("failed to spawn Rerun visualizer", error))?;
        let field = rerun::Asset3D::from_file_path(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/s1/Season 1 Field V2.stl"
        ))
        .map_err(|error| CuError::from(format!("failed to load field STL: {error}")))?;
        rerun
            .log_static(
                "field",
                &rerun::Transform3D::from_scale([0.001, 0.001, 0.001]),
            )
            .map_err(|error| CuError::from(format!("failed to log field scale: {error}")))?;
        rerun
            .log_static("field/mesh", &field)
            .map_err(|error| CuError::from(format!("failed to log field STL: {error}")))?;
        Ok(Self {
            rerun,
            last_visualization: None,
            last_drive_faults: [None; 2],
            last_drive_modes: [None; 2],
        })
    }
}

#[cfg(feature = "sim-debug")]
trait MasterpieceVisualizationData: CopperListTuple + CuPayloadRawBytes {
    fn pose(&self) -> Option<&cu_spatial_payloads::Pose<f64>>;
    fn gamepad(&self) -> Option<&common::GamePadState>;
    fn intake_command(&self) -> Option<&common::MotorCMD>;
    fn turret_state(&self) -> Option<&common::TurretState>;
    fn left_drive_command(&self) -> Option<&common::MotorCMD>;
    fn right_drive_command(&self) -> Option<&common::MotorCMD>;
    fn drive_faults(&self) -> Option<&moteus_tasks::DriveMoteusFaults>;
    fn drive_modes(&self) -> Option<&moteus_tasks::DriveMoteusModes>;
}

#[cfg(feature = "sim-debug")]
impl MasterpieceVisualizationData for MasterpieceDataSet {
    fn pose(&self) -> Option<&cu_spatial_payloads::Pose<f64>> {
        self.get_pose_estimator_output_0().payload()
    }

    fn gamepad(&self) -> Option<&common::GamePadState> {
        self.get_dsrx_output_1().payload()
    }

    fn intake_command(&self) -> Option<&common::MotorCMD> {
        self.get_s_1_intake_state_machine_output_0().payload()
    }

    fn turret_state(&self) -> Option<&common::TurretState> {
        self.get_moteus_turret_state_assembler_output().payload()
    }

    fn left_drive_command(&self) -> Option<&common::MotorCMD> {
        self.get_diffdrivecmd_output_0().payload()
    }

    fn right_drive_command(&self) -> Option<&common::MotorCMD> {
        self.get_diffdrivecmd_output_1().payload()
    }

    fn drive_faults(&self) -> Option<&moteus_tasks::DriveMoteusFaults> {
        self.get_diffdrivereader_output_1().payload()
    }

    fn drive_modes(&self) -> Option<&moteus_tasks::DriveMoteusModes> {
        self.get_diffdrivereader_output_2().payload()
    }
}

#[cfg(feature = "sim-debug")]
impl MasterpieceVisualizationData for MasterpiecePcDataSet {
    fn pose(&self) -> Option<&cu_spatial_payloads::Pose<f64>> {
        self.get_pose_estimator_output_0().payload()
    }

    fn gamepad(&self) -> Option<&common::GamePadState> {
        self.get_dsrx_output_1().payload()
    }

    fn intake_command(&self) -> Option<&common::MotorCMD> {
        self.get_s_1_intake_state_machine_output_0().payload()
    }

    fn turret_state(&self) -> Option<&common::TurretState> {
        self.get_moteus_turret_state_assembler_output().payload()
    }

    fn left_drive_command(&self) -> Option<&common::MotorCMD> {
        self.get_diffdrivecmd_output_0().payload()
    }

    fn right_drive_command(&self) -> Option<&common::MotorCMD> {
        self.get_diffdrivecmd_output_1().payload()
    }

    fn drive_faults(&self) -> Option<&moteus_tasks::DriveMoteusFaults> {
        self.get_diffdrivereader_output_1().payload()
    }

    fn drive_modes(&self) -> Option<&moteus_tasks::DriveMoteusModes> {
        self.get_diffdrivereader_output_2().payload()
    }
}

#[cfg(feature = "sim-debug")]
fn moteus_mode_name(mode: i8) -> &'static str {
    match mode {
        0 => "stopped",
        1 => "fault",
        2 => "enabling",
        3 => "calibrating",
        4 => "calibration complete",
        5 => "pwm",
        6 => "voltage",
        7 => "voltage foc",
        8 => "voltage dq",
        9 => "current",
        10 => "position",
        11 => "timeout",
        12 => "zero velocity",
        13 => "stay within",
        14 => "measure inductance",
        15 => "brake",
        _ => "unknown",
    }
}

#[cfg(feature = "sim-debug")]
impl<P> evoviz::Visualizer<P> for MasterpieceVisualizer
where
    P: MasterpieceVisualizationData + 'static,
{
    fn visualize_copperlist(&mut self, copperlist: &CopperList<P>) -> CuResult<()> {
        let now = Instant::now();
        if self
            .last_visualization
            .is_some_and(|last| now.duration_since(last) < Duration::from_secs_f64(1.0 / 30.0))
        {
            return Ok(());
        }
        self.last_visualization = Some(now);

        self.rerun
            .set_time_sequence("copperlist", copperlist.id as i64);

        let messages = copperlist.msgs.cumsgs();
        let loop_start = messages.first().and_then(|message| {
            Option::<CuTime>::from(message.metadata().process_time().start)
        });
        let loop_end = messages.last().and_then(|message| {
            Option::<CuTime>::from(message.metadata().process_time().end)
        });
        if let (Some(start), Some(end)) = (loop_start, loop_end) {
            if end >= start {
                self.rerun
                    .log(
                        "copper/timing/end_to_end_ms",
                        &rerun::Scalars::single((end - start).as_nanos() as f64 / 1_000_000.0),
                    )
                    .map_err(|error| {
                        CuError::from(format!("failed to log Copper loop time: {error}"))
                    })?;
            }
        }

        let component_ids = P::get_all_task_ids();
        for (index, (component_id, message)) in
            component_ids.iter().zip(messages.iter()).enumerate()
        {
            if component_ids[..index].contains(component_id) {
                continue;
            }

            let process_time = message.metadata().process_time();
            let start = Option::<CuTime>::from(process_time.start);
            let end = Option::<CuTime>::from(process_time.end);
            if let (Some(start), Some(end)) = (start, end) {
                if end >= start {
                    self.rerun
                        .log(
                            format!("copper/timing/components/{component_id}_ms"),
                            &rerun::Scalars::single(
                                (end - start).as_nanos() as f64 / 1_000_000.0,
                            ),
                        )
                        .map_err(|error| {
                            CuError::from(format!(
                                "failed to log Copper component {component_id} time: {error}"
                            ))
                        })?;
                }
            }
        }

        if let Some(gamepad) = copperlist.msgs.gamepad() {
            for (axis, value) in [
                ("left_x", gamepad.left_x),
                ("left_y", gamepad.left_y),
                ("right_x", gamepad.right_x),
                ("right_y", gamepad.right_y),
            ] {
                self.rerun
                    .log(
                        format!("driver_station/joystick/{axis}"),
                        &rerun::Scalars::single(value),
                    )
                    .map_err(|error| {
                        CuError::from(format!("failed to log joystick {axis}: {error}"))
                    })?;
            }
        }

        if let Some(common::MotorCMD::Velocity(velocity, _)) =
            copperlist.msgs.left_drive_command()
        {
            self.rerun
                .log(
                    "diffdrive_cmd/left",
                    &rerun::Scalars::single(
                        velocity.get::<cu29::units::si::angular_velocity::radian_per_second>(),
                    ),
                )
                .map_err(|error| {
                    CuError::from(format!("failed to log left diff-drive command: {error}"))
                })?;
        }

        if let Some(common::MotorCMD::Velocity(velocity, _)) =
            copperlist.msgs.right_drive_command()
        {
            self.rerun
                .log(
                    "diffdrive_cmd/right",
                    &rerun::Scalars::single(
                        velocity.get::<cu29::units::si::angular_velocity::radian_per_second>(),
                    ),
                )
                .map_err(|error| {
                    CuError::from(format!("failed to log right diff-drive command: {error}"))
                })?;
        }

        if let Some(faults) = copperlist.msgs.drive_faults() {
            for (index, motor) in ["left", "right"].into_iter().enumerate() {
                let fault = faults[index];
                if self.last_drive_faults[index] == Some(fault) {
                    continue;
                }

                let level = if fault == 0 {
                    rerun::TextLogLevel::INFO
                } else {
                    rerun::TextLogLevel::ERROR
                };
                self.rerun
                    .log(
                        format!("faults/moteus/drive_{motor}"),
                        &rerun::TextLog::new(format!("fault code: {fault}"))
                            .with_level(level),
                    )
                    .map_err(|error| {
                        CuError::from(format!(
                            "failed to log drive {motor} Moteus fault: {error}"
                        ))
                    })?;
                self.last_drive_faults[index] = Some(fault);
            }
        }

        if let Some(modes) = copperlist.msgs.drive_modes() {
            for (index, motor) in ["left", "right"].into_iter().enumerate() {
                let mode = modes[index];
                if self.last_drive_modes[index] == Some(mode) {
                    continue;
                }

                let level = if matches!(mode, 1 | 11) {
                    rerun::TextLogLevel::ERROR
                } else {
                    rerun::TextLogLevel::INFO
                };
                self.rerun
                    .log(
                        format!("modes/moteus/drive_{motor}"),
                        &rerun::TextLog::new(format!(
                            "mode {mode}: {}",
                            moteus_mode_name(mode)
                        ))
                        .with_level(level),
                    )
                    .map_err(|error| {
                        CuError::from(format!(
                            "failed to log drive {motor} Moteus mode: {error}"
                        ))
                    })?;
                self.last_drive_modes[index] = Some(mode);
            }
        }

        if let Some(pose) = copperlist.msgs.pose() {
            let matrix = pose.to_matrix();
            let yaw = f32::atan2(matrix[1][0] as f32, matrix[0][0] as f32);
            self.rerun
                .log(
                    "robot",
                    &rerun::Transform3D::from_translation_rotation(
                        [
                            matrix[0][3] as f32,
                            matrix[1][3] as f32,
                            matrix[2][3] as f32,
                        ],
                        rerun::Rotation3D::AxisAngle(rerun::components::RotationAxisAngle::new(
                            [0.0, 0.0, 1.0],
                            rerun::Angle::from_radians(yaw),
                        )),
                    ),
                )
                .map_err(|error| CuError::from(format!("failed to log IEKF pose: {error}")))?;
            self.rerun
                .log(
                    "robot/body",
                    &rerun::Boxes3D::from_half_sizes([[0.1143, 0.1143, 0.05]]),
                )
                .map_err(|error| CuError::from(format!("failed to log robot body: {error}")))?;
            self.rerun
                .log(
                    "robot/forward",
                    &rerun::Arrows3D::from_vectors([[0.0, 0.3, 0.0]])
                        .with_origins([[0.0, 0.0, 0.08]])
                        .with_radii([0.02])
                        .with_colors([rerun::Color::from_rgb(255, 80, 80)]),
                )
                .map_err(|error| CuError::from(format!("failed to log robot heading: {error}")))?;

            let intake_color = match copperlist.msgs.intake_command() {
                Some(common::MotorCMD::Velocity(velocity, _)) if velocity.raw() > 0.0 => {
                    rerun::Color::from_rgb(0, 255, 0)
                }
                Some(common::MotorCMD::Velocity(velocity, _)) if velocity.raw() < 0.0 => {
                    rerun::Color::from_rgb(0, 80, 255)
                }
                _ => rerun::Color::from_rgb(255, 0, 0),
            };
            self.rerun
                .log(
                    "robot/intake_indicator",
                    &rerun::LineStrips3D::new([[[-0.1143, 0.15, 0.08], [0.1143, 0.15, 0.08]]])
                        .with_radii([0.01])
                        .with_colors([intake_color]),
                )
                .map_err(|error| {
                    CuError::from(format!("failed to log intake indicator: {error}"))
                })?;
        }

        if let Some(turret) = copperlist.msgs.turret_state() {
            let yaw = turret.position.get::<cu29::units::si::angle::radian>()
                + std::f32::consts::FRAC_PI_2;
            self.rerun
                .log(
                    "robot/turret",
                    &rerun::Transform3D::from_translation_rotation(
                        [0.0, 0.0, 0.1],
                        rerun::Rotation3D::AxisAngle(rerun::components::RotationAxisAngle::new(
                            [0.0, 0.0, 1.0],
                            rerun::Angle::from_radians(yaw),
                        )),
                    ),
                )
                .map_err(|error| CuError::from(format!("failed to log turret pose: {error}")))?;
            self.rerun
                .log(
                    "robot/turret/body",
                    &rerun::Boxes3D::from_half_sizes([[0.15, 0.075, 0.05]]),
                )
                .map_err(|error| CuError::from(format!("failed to log turret body: {error}")))?;
            self.rerun
                .log(
                    "robot/turret/forward",
                    &rerun::Arrows3D::from_vectors([[0.35, 0.0, 0.0]])
                        .with_origins([[0.0, 0.0, 0.06]])
                        .with_radii([0.015])
                        .with_colors([rerun::Color::from_rgb(80, 80, 255)]),
                )
                .map_err(|error| CuError::from(format!("failed to log turret heading: {error}")))?;
        }

        Ok(())
    }
}

#[cfg(feature = "sim-debug")]
pub type MasterpieceEvoVisualizer =
    evoviz::EvoVisualizer<MasterpieceDataSet, MasterpieceVisualizer>;
#[cfg(feature = "sim-debug")]
pub type MasterpiecePcEvoVisualizer =
    evoviz::EvoVisualizer<MasterpiecePcDataSet, MasterpieceVisualizer>;

#[cfg(feature = "sim-debug")]
pub fn evo_visualizer() -> CuResult<MasterpieceEvoVisualizer> {
    Ok(evoviz::EvoVisualizer::new(MasterpieceVisualizer::spawn()?))
}

#[cfg(feature = "sim-debug")]
pub fn evo_pc_visualizer() -> CuResult<MasterpiecePcEvoVisualizer> {
    Ok(evoviz::EvoVisualizer::new(MasterpieceVisualizer::spawn()?))
}
