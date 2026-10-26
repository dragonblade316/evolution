use cu29::prelude::*;
use cu_sensor_payloads::CuImage;

#[derive(Reflect)]
pub struct NullCameraSource;

impl Freezable for NullCameraSource {}

impl CuSrcTask for NullCameraSource {
    type Output<'m> = output_msg!(CuImage<Vec<u8>>);
    type Resources<'r> = ();

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self> {
        Ok(Self)
    }

    fn process(&mut self, _ctx: &CuContext, output: &mut Self::Output<'_>) -> CuResult<()> {
        output.clear_payload();
        Ok(())
    }
}
