use core::f64;

use common::{ChassisSpeeds, DiffDriveSpeeds, GamePadState, MotorCMD};
use cu29::{
    CuResult, config::ComponentConfig, cutask::{CuMsg, CuTask, Freezable}, input_msg, output_msg, prelude::Reflect, units::si::{
        angular_velocity::radian_per_second,
        f32::{AngularVelocity, Length, Velocity},
        length::meter, velocity::meter_per_second,
    }
};

pub mod odom;

// ---------------------------------------------------------------------------
// DiffDriveKinematics — ChassisSpeeds → DiffDriveSpeeds
// ---------------------------------------------------------------------------

#[derive(Reflect)]
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
        const DEFAULT_WHEEL_RADIUS_METERS: f32 = 0.1;
        const DEFAULT_TRACKWIDTH: f32 = 0.3;

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
                    .unwrap_or(DEFAULT_TRACKWIDTH),
            ),
            None => Length::new::<meter>(DEFAULT_TRACKWIDTH),
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

        let left = Velocity::new::<meter_per_second>(
            cmd.x.raw() - (cmd.theta.raw() * self.trackwidth.raw()) / 2.0,
        );
        let right = Velocity::new::<meter_per_second>(
            cmd.x.raw() + (cmd.theta.raw() * self.trackwidth.raw()) / 2.0,
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
///   wheel_radius — wheel radius in meters (default: 0.1)
#[derive(Reflect)]
pub struct DiffDriveCmd {
    #[allow(dead_code)]
    wheel_radius: Length,
}

impl Freezable for DiffDriveCmd {}

pub type LeftMotorCMD = MotorCMD;
pub type RightMotorCMD = MotorCMD;

impl CuTask for DiffDriveCmd {
    type Input<'m> = input_msg!(DiffDriveSpeeds);
    type Output<'m> = output_msg!('m, LeftMotorCMD, RightMotorCMD);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let wheel_radius = Length::new::<meter>(match config {
            Some(cfg) => cfg.get::<f32>("wheel_radius")?.unwrap_or(0.1),
            None => 0.1,
        });
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

            fn solve(v: Velocity, r: Length) -> AngularVelocity {
                AngularVelocity::new::<radian_per_second>(v.get::<meter_per_second>() / r.get::<meter>())
            }

            left_out.set_payload(MotorCMD::Velocity(solve(speeds.left, self.wheel_radius), None));
            right_out.set_payload(MotorCMD::Velocity(solve(speeds.right, self.wheel_radius), None));
        }
        Ok(())
    }
}

///This is a task that takes in joy any outputs
#[derive(Reflect)]
pub struct DiffDriveDoubleStick {
    #[allow(dead_code)]
    max_wheel_vel: f32,
}

impl Freezable for DiffDriveDoubleStick {}

impl CuTask for DiffDriveDoubleStick {
    type Input<'m> = input_msg!(GamePadState);
    type Output<'m> = output_msg!(DiffDriveSpeeds);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let max_wheel_vel = match config {
            Some(cfg) => cfg.get::<f32>("max_wheel_vel")?.unwrap_or(1.0),
            None => 1.0,
        };
        Ok(Self { max_wheel_vel })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let data = input.payload().unwrap();

        output.set_payload(DiffDriveSpeeds { left: Velocity::new::<meter_per_second>(self.max_wheel_vel * data.left_y), right: Velocity::new::<meter_per_second>(self.max_wheel_vel * data.right_y)});
        Ok(())
    }
}
