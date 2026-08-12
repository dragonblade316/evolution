use cu29::cutask::{CuSinkTask, Freezable};
use cu29::prelude::*;
use cu29::units::si::length::meter;
use cu29::units::si::f32::{Angle, Length};
use cu29::units::si::angle::radian;
use cu_spatial_payloads::Pose;
use rerun::{RecordingStream, RecordingStreamBuilder};

const DEFAULT_LENGTH_M: f32 = 0.5;
const DEFAULT_WIDTH_M: f32 = 0.5;
const DEFAULT_HEIGHT_M: f32 = 0.2;

#[derive(Reflect)]
pub struct RobotVisualizer {
    rec: RecordingStream,
    length: Length,
    width: Length,
    height: Length,
}

impl Freezable for RobotVisualizer {}

impl CuSinkTask for RobotVisualizer {
    type Input<'m> = input_msg!(Pose<f64>);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let (length, width, height) = match config {
            Some(cfg) => (
                Length::new::<meter>(cfg.get::<f32>("length")?.unwrap_or(DEFAULT_LENGTH_M)),
                Length::new::<meter>(cfg.get::<f32>("width")?.unwrap_or(DEFAULT_WIDTH_M)),
                Length::new::<meter>(cfg.get::<f32>("height")?.unwrap_or(DEFAULT_HEIGHT_M)),
            ),
            None => (
                Length::new::<meter>(DEFAULT_LENGTH_M),
                Length::new::<meter>(DEFAULT_WIDTH_M),
                Length::new::<meter>(DEFAULT_HEIGHT_M),
            ),
        };

        let rec = RecordingStreamBuilder::new("evoviz")
            .connect_grpc()
            .map_err(|e| CuError::from(format!("RobotVisualizer: failed to connect rerun grpc: {e}")))?;

        Ok(Self { rec, length, width, height })
    }

    fn process(&mut self, _ctx: &CuContext, input: &Self::Input<'_>) -> CuResult<()> {
        let Some(pose) = input.payload() else {
            return Ok(());
        };

        let [tx, ty, tz] = pose.translation();
        let rot = pose.rotation();
        let yaw = f32::atan2(
            rot[1][0].get::<radian>() as f32,
            rot[0][0].get::<radian>() as f32,
        );

        self.rec
            .log(
                "robot",
                &rerun::Transform3D::from_translation_rotation(
                    [tx.get::<meter>() as f32, ty.get::<meter>() as f32, tz.get::<meter>() as f32],
                    rerun::Rotation3D::AxisAngle(rerun::components::RotationAxisAngle::new(
                        [0.0, 0.0, 1.0],
                        rerun::Angle::from_radians(yaw),
                    )),
                ),
            )
            .map_err(|e| CuError::from(format!("RobotVisualizer: failed to log transform: {e}")))?;

        self.rec
            .log(
                "robot/box",
                &rerun::Boxes3D::from_half_sizes([[
                    self.length.get::<meter>() / 2.0,
                    self.width.get::<meter>() / 2.0,
                    self.height.get::<meter>() / 2.0,
                ]]),
            )
            .map_err(|e| CuError::from(format!("RobotVisualizer: failed to log box: {e}")))?;

        Ok(())
    }
}

const DEFAULT_TURRET_LENGTH_M: f32 = 0.3;
const DEFAULT_TURRET_WIDTH_M: f32 = 0.15;
const DEFAULT_TURRET_HEIGHT_M: f32 = 0.1;
const DEFAULT_TURRET_X_M: f32 = 0.0;
const DEFAULT_TURRET_Y_M: f32 = 0.0;
const DEFAULT_TURRET_Z_M: f32 = 0.0;

#[derive(Reflect)]
pub struct TurretVisualizer {
    rec: RecordingStream,
    parent: Option<String>,
    x: Length,
    y: Length,
    z: Length,
    length: Length,
    width: Length,
    height: Length,
}

impl Freezable for TurretVisualizer {}

impl CuSinkTask for TurretVisualizer {
    type Input<'m> = input_msg!(Angle);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let (parent, x, y, z, length, width, height) = match config {
            Some(cfg) => (
                cfg.get::<String>("parent")?,
                Length::new::<meter>(cfg.get::<f32>("x")?.unwrap_or(DEFAULT_TURRET_X_M)),
                Length::new::<meter>(cfg.get::<f32>("y")?.unwrap_or(DEFAULT_TURRET_Y_M)),
                Length::new::<meter>(cfg.get::<f32>("z")?.unwrap_or(DEFAULT_TURRET_Z_M)),
                Length::new::<meter>(cfg.get::<f32>("length")?.unwrap_or(DEFAULT_TURRET_LENGTH_M)),
                Length::new::<meter>(cfg.get::<f32>("width")?.unwrap_or(DEFAULT_TURRET_WIDTH_M)),
                Length::new::<meter>(cfg.get::<f32>("height")?.unwrap_or(DEFAULT_TURRET_HEIGHT_M)),
            ),
            None => (
                None,
                Length::new::<meter>(DEFAULT_TURRET_X_M),
                Length::new::<meter>(DEFAULT_TURRET_Y_M),
                Length::new::<meter>(DEFAULT_TURRET_Z_M),
                Length::new::<meter>(DEFAULT_TURRET_LENGTH_M),
                Length::new::<meter>(DEFAULT_TURRET_WIDTH_M),
                Length::new::<meter>(DEFAULT_TURRET_HEIGHT_M),
            ),
        };

        let rec = RecordingStreamBuilder::new("evoviz")
            .connect_grpc()
            .map_err(|e| CuError::from(format!("TurretVisualizer: failed to connect rerun grpc: {e}")))?;

        Ok(Self { rec, parent, x, y, z, length, width, height })
    }

    fn process(&mut self, _ctx: &CuContext, input: &Self::Input<'_>) -> CuResult<()> {
        let Some(angle) = input.payload() else {
            return Ok(());
        };

        let yaw = angle.get::<radian>();

        let turret_path = match &self.parent {
            Some(p) => format!("{p}/turret"),
            None => "turret".to_string(),
        };

        self.rec
            .log(
                turret_path.as_str(),
                &rerun::Transform3D::from_translation_rotation(
                    [self.x.get::<meter>(), self.y.get::<meter>(), self.z.get::<meter>()],
                    rerun::Rotation3D::AxisAngle(rerun::components::RotationAxisAngle::new(
                        [0.0, 0.0, 1.0],
                        rerun::Angle::from_radians(yaw),
                    )),
                ),
            )
            .map_err(|e| CuError::from(format!("TurretVisualizer: failed to log transform: {e}")))?;

        self.rec
            .log(
                format!("{turret_path}/box").as_str(),
                &rerun::Boxes3D::from_half_sizes([[
                    self.length.get::<meter>() / 2.0,
                    self.width.get::<meter>() / 2.0,
                    self.height.get::<meter>() / 2.0,
                ]]),
            )
            .map_err(|e| CuError::from(format!("TurretVisualizer: failed to log box: {e}")))?;

        Ok(())
    }
}