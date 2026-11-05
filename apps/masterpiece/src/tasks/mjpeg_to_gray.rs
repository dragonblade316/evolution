use cu29::prelude::*;
use cu_sensor_payloads::{CuImage, CuImageBufferFormat};
use zune_jpeg::{
    JpegDecoder,
    zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions},
};

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
        let (gray, width, height) = compressed.buffer_handle.with_inner(|bytes| {
            let options =
                DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
            let mut decoder =
                JpegDecoder::new_with_options(ZCursor::new(&bytes[..]), options);
            let gray = decoder
                .decode()
                .map_err(|error| CuError::from(format!("failed to decode MJPEG frame: {error}")))?;
            let (width, height) = decoder
                .dimensions()
                .ok_or_else(|| CuError::from("decoded MJPEG frame has no dimensions"))?;
            Ok::<_, CuError>((gray, width as u32, height as u32))
        })?;

        let format = CuImageBufferFormat {
            width,
            height,
            stride: width,
            pixel_format: *b"GRAY",
        };
        let mut image = CuImage::new(
            format,
            CuHandle::new_detached_with_mode(gray, cu29::config::HandleContent::None),
        );
        image.seq = compressed.seq;
        output.set_payload(image);
        Ok(())
    }
}
