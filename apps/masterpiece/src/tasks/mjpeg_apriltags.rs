use cu29::bincode::{Decode, Encode};
use cu29::prelude::*;
use cu_apriltag::{AprilTagDetections, AprilTags};
use cu_sensor_payloads::CuImage;

use super::MjpegToGray;

const KEYFRAME_CAPACITY_RESERVATION: usize = 32 * 1024;

#[derive(Reflect)]
#[reflect(from_reflect = false)]
pub struct MjpegAprilTags {
    #[reflect(ignore)]
    grayscale: MjpegToGray,
    #[reflect(ignore)]
    detector: AprilTags,
    #[reflect(ignore)]
    keyframe_capacity_reservation: Vec<u8>,
}

impl Freezable for MjpegAprilTags {
    fn freeze<E: cu29::bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), cu29::bincode::error::EncodeError> {
        self.keyframe_capacity_reservation.encode(encoder)
    }

    fn thaw<D: cu29::bincode::de::Decoder>(
        &mut self,
        decoder: &mut D,
    ) -> Result<(), cu29::bincode::error::DecodeError> {
        self.keyframe_capacity_reservation = Vec::<u8>::decode(decoder)?;
        Ok(())
    }
}

impl CuTask for MjpegAprilTags {
    type Input<'m> = input_msg!(CuImage<Vec<u8>>);
    type Output<'m> = output_msg!(AprilTagDetections);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self> {
        Ok(Self {
            grayscale: <MjpegToGray as CuTask>::new(None, ())?,
            detector: <AprilTags as CuTask>::new(config, ())?,
            keyframe_capacity_reservation: vec![0; KEYFRAME_CAPACITY_RESERVATION],
        })
    }

    fn process(
        &mut self,
        ctx: &CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let mut grayscale = CuMsg::<CuImage<Vec<u8>>>::default();
        self.grayscale.process(ctx, input, &mut grayscale)?;
        self.detector.process(ctx, &grayscale, output)
    }
}
