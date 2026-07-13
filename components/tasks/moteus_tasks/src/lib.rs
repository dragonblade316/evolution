use common::{DiffDriveSpeeds, MotorCMD, MotorData};
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    CuError, CuResult,
};
use moteus_bridge::messages::{MoteusCMD, MoteusData};


// ---------------------------------------------------------------------------
// MoteusDiff — DiffDriveSpeeds → MoteusCMD (per-motor commands)
// ---------------------------------------------------------------------------

/// Converts diff-drive wheel speeds into per-motor CAN commands.
///
/// Config keys:
///   left_can_id   — CAN ID of the left wheel motor   (default: "1")
///   right_can_id  — CAN ID of the right wheel motor  (default: "2")
///   max_torque    — optional torque limit (Nm)        (default: none)
pub struct MoteusDiff {
    left_can_id: u8,
    right_can_id: u8,
    max_torque: Option<f32>,
}

impl Freezable for MoteusDiff {}

impl CuTask for MoteusDiff {
    type Input<'m> = input_msg!(DiffDriveSpeeds);
    type Output<'m> = output_msg!(MoteusCMD, MoteusCMD);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        let left_can_id: u8 = match config {
            Some(cfg) => cfg
                .get::<String>("left_can_id")?
                .unwrap_or_else(|| "1".to_string())
                .parse()
                .map_err(|_| CuError::from("Invalid left_can_id"))?,
            None => 1,
        };

        let right_can_id: u8 = match config {
            Some(cfg) => cfg
                .get::<String>("right_can_id")?
                .unwrap_or_else(|| "2".to_string())
                .parse()
                .map_err(|_| CuError::from("Invalid right_can_id"))?,
            None => 2,
        };

        let max_torque: Option<f32> = match config {
            Some(cfg) => cfg
                .get::<String>("max_torque")?
                .map(|s| {
                    s.parse()
                        .map_err(|_| CuError::from("Invalid max_torque"))
                })
                .transpose()?,
            None => None,
        };

        Ok(Self {
            left_can_id,
            right_can_id,
            max_torque,
        })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        if let Some(speeds) = input.payload() {
            let (left_out, right_out) = output;

            left_out.set_payload(MoteusCMD {
                can_id: self.left_can_id,
                cmd: MotorCMD::Velocity(speeds.left, None),
            });

            right_out.set_payload(MoteusCMD {
                can_id: self.right_can_id,
                cmd: MotorCMD::Velocity(speeds.right, None),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MoteusTelemetry — MoteusData + MoteusData → DiffDriveSpeeds
// ---------------------------------------------------------------------------

/// Extracts left/right wheel velocities from motor telemetry.
///
/// Takes one status message from each wheel channel and extracts their
/// velocities. The channel paths identify the motors, so no CAN-ID lookup is
/// needed in this task.
pub struct MoteusTelemetry;

impl Freezable for MoteusTelemetry {}

impl CuTask for MoteusTelemetry {
    type Input<'m> = input_msg!(MoteusData, MoteusData);
    type Output<'m> = output_msg!(DiffDriveSpeeds);
    type Resources<'r> = ();

    fn new(
        _config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self)
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
