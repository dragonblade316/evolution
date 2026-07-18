use core::f64;

use common::{ChassisSpeeds, DiffDriveSpeeds, MotorCMD};
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    units::si::{
        angular_velocity::radian_per_second,
        f32::{AngularVelocity, Length},
        length::meter,
        velocity::meter_per_second,
    },
    CuResult,
};

// ---------------------------------------------------------------------------
// DiffDriveKinematics — ChassisSpeeds → DiffDriveSpeeds
// ---------------------------------------------------------------------------

pub struct DiffDriveKinematics {
    trackwidth: Length,
    wheel_radius: Length,
}

impl Freezable for DiffDriveKinematics {}

impl CuTask for DiffDriveKinematics {
    type Input<'m> = input_msg!(ChassisSpeeds);
    type Output<'m> = output_msg!(DiffDriveSpeeds);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        const DEFAULT_WHEEL_RADIUS_METERS: f32 = 0.02;
        const DEFAULT_TRACKWIDTH: f32 = 0.02;

        let wheel_radius = match config {
            Some(cfg) => Length::new::<meter>(
                cfg.get::<f32>("wheel_radius")?
                    .unwrap_or(DEFAULT_WHEEL_RADIUS_METERS),
            ),
            None => Length::new::<meter>(DEFAULT_WHEEL_RADIUS_METERS),
        };

        let trackwidth = match config {
            Some(cfg) => Length::new::<meter>(
                cfg.get::<f32>("trackwidth")?
                    .unwrap_or(DEFAULT_WHEEL_RADIUS_METERS),
            ),
            None => Length::new::<meter>(DEFAULT_WHEEL_RADIUS_METERS),
        };

        Ok(Self {
            wheel_radius,
            trackwidth,
        })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let cmd = match input.payload() {
            Some(i) => i,
            None => return Ok(()),
        };

        let left = AngularVelocity::new::<radian_per_second>(
            (cmd.x.raw() - (cmd.theta.raw() * self.trackwidth.raw()) / 2.0)
                / (self.wheel_radius.raw() * std::f32::consts::PI),
        );
        let right = AngularVelocity::new::<radian_per_second>(
            (cmd.x.raw() + (cmd.theta.raw() * self.trackwidth.raw()) / 2.0)
                / (self.wheel_radius.raw() * std::f32::consts::PI),
        );

        output.set_payload(DiffDriveSpeeds { left, right });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DiffDriveCmd — DiffDriveSpeeds → per-motor MotorCMD
// ---------------------------------------------------------------------------

/// Converts diff-drive wheel speeds into per-motor velocity commands.
///
/// Config keys:
///   wheel_radius — wheel radius in meters (default: 0.05)
pub struct DiffDriveCmd {
    #[allow(dead_code)]
    wheel_radius: f32,
}

impl Freezable for DiffDriveCmd {}

impl CuTask for DiffDriveCmd {
    type Input<'m> = input_msg!(DiffDriveSpeeds);
    type Output<'m> = output_msg!('m, MotorCMD, MotorCMD);
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
        if let Some(speeds) = input.payload() {
            let (left_out, right_out) = output;
            left_out.set_payload(MotorCMD::Velocity(speeds.left, None));
            right_out.set_payload(MotorCMD::Velocity(speeds.right, None));
        }
        Ok(())
    }
}
