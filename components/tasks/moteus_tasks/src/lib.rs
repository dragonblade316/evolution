use common::DiffDriveSpeeds;
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    CuResult,
};
use moteus_bridge::messages::MoteusData;

// ---------------------------------------------------------------------------
// MoteusDiff — per-motor MoteusData → DiffDriveSpeeds
// ---------------------------------------------------------------------------

/// Collects individual motor telemetry and produces diff-drive wheel speeds.
///
/// Config keys:
///   wheel_radius — wheel radius in meters (default: 0.05)
pub struct MoteusDiff {
    #[allow(dead_code)]
    wheel_radius: f32,
}

impl Freezable for MoteusDiff {}

impl CuTask for MoteusDiff {
    type Input<'m> = input_msg!('m, MoteusData, MoteusData);
    type Output<'m> = output_msg!(DiffDriveSpeeds);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let wheel_radius = match config {
            Some(cfg) => cfg.get::<f32>("wheel_radius")?.unwrap_or(0.05),
            None => 0.05,
        };
        Ok(Self { wheel_radius })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (left_msg, right_msg) = *input;
        let (Some(left), Some(right)) = (left_msg.payload(), right_msg.payload()) else {
            return Ok(());
        };

        output.set_payload(DiffDriveSpeeds {
            left: left.data.vel,
            right: right.data.vel,
        });
        Ok(())
    }
}
