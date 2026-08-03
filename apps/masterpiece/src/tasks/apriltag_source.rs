use cu29::prelude::*;
use cu_apriltag::AprilTagDetections;

#[derive(Default, Reflect)]
pub struct AprilTagSource {}

impl Freezable for AprilTagSource {}

impl CuSrcTask for AprilTagSource {
    type Resources<'r> = ();
    type Output<'m> = output_msg!(AprilTagDetections);

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn process(&mut self, _ctx: &CuContext, _output: &mut Self::Output<'_>) -> CuResult<()> {
        Ok(())
    }
}