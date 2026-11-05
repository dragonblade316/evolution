use cu29::prelude::*;
use cu_sensor_payloads::{CuImage, CuImageBufferFormat};
use image::ImageFormat;

#[derive(Reflect)]
pub struct MjpegToGray;

impl Freezable for MjpegToGray {}

impl CuTask for MjpegToGray {
    type Input<'m> = input_msg!(CuImage<Vec<u8>>);
    type Output<'m> = output_msg!(CuImage<Vec<u8>>);
    type Resources<'r> = ();

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self> {
        Ok(Self)
    }

    fn process(
        &mut self,
        _ctx: &CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        output.tov = input.tov;
        output.clear_payload();

        let Some(compressed) = input.payload() else {
            return Ok(());
        };

        compressed.mark_touched();
        let gray = compressed
            .buffer_handle
            .with_inner(|bytes| image::load_from_memory_with_format(bytes, ImageFormat::Jpeg))
            .map_err(|error| CuError::new_with_cause("failed to decode MJPEG frame", error))?
            .into_luma8();

        let format = CuImageBufferFormat {
            width: gray.width(),
            height: gray.height(),
            stride: gray.width(),
            pixel_format: *b"GRAY",
        };
        let mut image = CuImage::new(format, CuHandle::new_detached(gray.into_raw()));
        image.seq = compressed.seq;
        output.set_payload(image);
        Ok(())
    }
}
