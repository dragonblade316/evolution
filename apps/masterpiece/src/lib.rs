pub mod messages;
pub mod tasks;

#[cfg(feature = "sim-debug")]
use cu29::prelude::*;

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
pub struct MasterpieceVisualizer {
    rerun: rerun::RecordingStream,
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
        Ok(Self { rerun })
    }
}

#[cfg(feature = "sim-debug")]
impl cuviz::Visualizer<MasterpieceDataSet> for MasterpieceVisualizer {
    fn visualize_copperlist(
        &mut self,
        copperlist: &CopperList<MasterpieceDataSet>,
    ) -> CuResult<()> {
        self.rerun
            .set_time_sequence("copperlist", copperlist.id as i64);

        if let Some(pose) = copperlist
            .msgs
            .get_pose_estimator_output_0()
            .payload()
        {
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

            let intake_color = match copperlist
                .msgs
                .get_s_1_intake_state_machine_output_0()
                .payload()
            {
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

        if let Some(turret) = copperlist
            .msgs
            .get_moteus_turret_state_assembler_output()
            .payload()
        {
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
    cuviz::EvoVisualizer<MasterpieceDataSet, MasterpieceVisualizer>;

#[cfg(feature = "sim-debug")]
pub fn evo_visualizer() -> CuResult<MasterpieceEvoVisualizer> {
    Ok(cuviz::EvoVisualizer::new(MasterpieceVisualizer::spawn()?))
}
