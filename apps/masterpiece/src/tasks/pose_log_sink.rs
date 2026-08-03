use common::ChassisSpeeds;
use cu29::cutask::{CuSinkTask, Freezable};
use cu29::prelude::*;
use cu29::units::si::{angular_velocity::radian_per_second, velocity::meter_per_second};
use cu_spatial_payloads::Pose;

#[derive(Reflect)]
pub struct PoseLogSink {}

impl Freezable for PoseLogSink {}

impl CuSinkTask for PoseLogSink {
    type Input<'m> = input_msg!(Pose<f64>);
    type Resources<'r> = ();

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn process(&mut self, _ctx: &CuContext, input: &Self::Input<'_>) -> CuResult<()> {
        if let Some(pose) = input.payload() {
            let mat = pose.to_matrix();
            debug!(
                "PoseLogSink received pose: x={:.3}, y={:.3}",
                mat[0][3], mat[1][3]
            );
        }
        Ok(())
    }
}

#[derive(Reflect)]
pub struct SpeedsLogSink {}

impl Freezable for SpeedsLogSink {}

impl CuSinkTask for SpeedsLogSink {
    type Input<'m> = input_msg!(ChassisSpeeds);
    type Resources<'r> = ();

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn process(&mut self, _ctx: &CuContext, input: &Self::Input<'_>) -> CuResult<()> {
        if let Some(speeds) = input.payload() {
            debug!(
                "SpeedsLogSink received speeds: x={:.3}, y={:.3}, theta={:.3}",
                speeds.x.get::<meter_per_second>(),
                speeds.y.get::<meter_per_second>(),
                speeds.theta.get::<radian_per_second>(),
            );
        }
        Ok(())
    }
}