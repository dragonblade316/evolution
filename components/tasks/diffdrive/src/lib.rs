use core::f64;

use common::{ChassisSpeeds, DiffDriveSpeeds};
use cu29::{config::Value, cutask::{CuMsg, CuTask, Freezable}, input_msg, output_msg, units::si::{angular_velocity::radian_per_second, f32::{AngularVelocity, Length, Velocity}, length::meter, velocity::meter_per_second}};



struct DiffDriveKinematics {
    trackwidth: Length,
    wheel_radius: Length
}

impl Freezable for DiffDriveKinematics {}

impl CuTask for DiffDriveKinematics {
    type Input<'m> = input_msg!(ChassisSpeeds);
    type Output<'m> = output_msg!(DiffDriveSpeeds);
    type Resources<'r> = ();
    
    fn new(config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
        where
            Self: Sized {

        const DEFAULT_WHEEL_RADIUS_METERS: f32 = 0.02;
        const DEFAULT_TRACKWIDTH: f32 = 0.02;

        let wheel_radius = match config {
            Some(cfg) => Length::new::<meter>(cfg.get::<f32>("wheel_radius")?.unwrap_or(DEFAULT_WHEEL_RADIUS_METERS)),
            None => Length::new::<meter>(DEFAULT_WHEEL_RADIUS_METERS)
        };

        let trackwidth = match config {
            Some(cfg) => Length::new::<meter>(cfg.get::<f32>("trackwidth")?.unwrap_or(DEFAULT_WHEEL_RADIUS_METERS)),
            None => Length::new::<meter>(DEFAULT_WHEEL_RADIUS_METERS)
        };


        Ok(Self {
            wheel_radius,
            trackwidth
        })
    }

    fn process(
            &mut self,
            _ctx: &cu29::prelude::CuContext,
            input: &Self::Input<'_>,
            output: &mut Self::Output<'_>,
        ) -> cu29::CuResult<()> {

        let cmd = match input.payload() {
            Some(i) => i,
            None => return Ok(())
        };

        //I hate this
        let left = AngularVelocity::new::<radian_per_second>((cmd.x.raw() - (cmd.theta.raw() * self.trackwidth.raw())/2.0) / (self.wheel_radius.raw() * std::f32::consts::PI));
        let right = AngularVelocity::new::<radian_per_second>((cmd.x.raw() + (cmd.theta.raw() * self.trackwidth.raw())/2.0) / (self.wheel_radius.raw() * std::f32::consts::PI));
        
        let drivespeeds = DiffDriveSpeeds {
            left,
            right
        };

        output.set_payload(drivespeeds);

        Ok(())
    }
}

struct DiffDriveOdometry;
