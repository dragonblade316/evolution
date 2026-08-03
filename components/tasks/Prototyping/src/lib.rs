use common::{GamePadState, MotorCMD};
use cu29::{
    CuResult,
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    prelude::Reflect,
    units::si::{
        angle::radian,
        angular_velocity::revolution_per_minute,
        f32::{Angle, AngularVelocity},
    },
};

// ---------------------------------------------------------------------------
// ProtoVelocityControl — GamePadState → MotorCMD::Velocity
// ---------------------------------------------------------------------------

/// Maps left stick Y to a velocity command.
///
/// Config keys:
///   max_vel — peak velocity in rpm at full stick deflection (default: 10.0)
#[derive(Reflect)]
pub struct ProtoVelocityControl {
    max_vel: AngularVelocity,
}

impl Freezable for ProtoVelocityControl {}

impl CuTask for ProtoVelocityControl {
    type Input<'m> = input_msg!(GamePadState);
    type Output<'m> = output_msg!(MotorCMD);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let max_vel = AngularVelocity::new::<revolution_per_minute>(match config {
            Some(cfg) => cfg.get::<f32>("max_vel")?.unwrap_or(10.0),
            None => 10.0,
        });
        Ok(Self { max_vel })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let Some(pad) = input.payload() else {
            return Ok(());
        };

        output.set_payload(MotorCMD::Velocity(self.max_vel * pad.left_y, None));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ProtoPositionControl — GamePadState → MotorCMD::Position
// ---------------------------------------------------------------------------

/// Maps left stick Y to a position command.
///
/// Config keys:
///   max_pos — peak position in radians at full stick deflection (default: 1.0)
#[derive(Reflect)]
pub struct ProtoPositionControl {
    max_pos: Angle,
}

impl Freezable for ProtoPositionControl {}

impl CuTask for ProtoPositionControl {
    type Input<'m> = input_msg!(GamePadState);
    type Output<'m> = output_msg!(MotorCMD);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let max_pos = Angle::new::<radian>(match config {
            Some(cfg) => cfg.get::<f32>("max_pos")?.unwrap_or(1.0),
            None => 1.0,
        });
        Ok(Self { max_pos })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let Some(pad) = input.payload() else {
            return Ok(());
        };

        output.set_payload(MotorCMD::Position(self.max_pos * pad.left_y, None, None));
        Ok(())
    }
}
