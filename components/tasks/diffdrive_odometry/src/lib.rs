use common::{ChassisSpeeds, DiffDriveSpeeds};
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    units::si::{
        angular_velocity::radian_per_second,
        f32::{AngularVelocity, Length, Velocity},
        length::meter,
        velocity::meter_per_second,
    },
    CuError, CuResult,
};

struct DiffDriveOdometry {
    trackwidth: Length,
    wheel_radius: Length,
    /// Distance from the axle to the robot's center (control point).
    /// Positive = center is ahead of the axle (axle at rear).
    /// Default: 0 (center on axle).
    axle_offset: Length,
}

impl Freezable for DiffDriveOdometry {}

impl CuTask for DiffDriveOdometry {
    type Input<'m> = input_msg!(DiffDriveSpeeds);
    type Output<'m> = output_msg!(ChassisSpeeds);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        const DEFAULT_WHEEL_RADIUS_METERS: f32 = 0.02;
        const DEFAULT_TRACKWIDTH_METERS: f32 = 0.02;
        const DEFAULT_AXLE_OFFSET_METERS: f32 = 0.0;

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
                    .unwrap_or(DEFAULT_TRACKWIDTH_METERS),
            ),
            None => Length::new::<meter>(DEFAULT_TRACKWIDTH_METERS),
        };

        let axle_offset = match config {
            Some(cfg) => Length::new::<meter>(
                cfg.get::<f32>("axle_offset")?
                    .unwrap_or(DEFAULT_AXLE_OFFSET_METERS),
            ),
            None => Length::new::<meter>(DEFAULT_AXLE_OFFSET_METERS),
        };

        Ok(Self {
            wheel_radius,
            trackwidth,
            axle_offset,
        })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let speeds = match input.payload() {
            Some(s) => s,
            None => return Ok(()),
        };

        // Linear and angular velocity at the axle.
        let v_axle = (speeds.left.raw() + speeds.right.raw()) / 2.0
            * self.wheel_radius.raw()
            * std::f32::consts::PI;
        let omega = (speeds.right.raw() - speeds.left.raw())
            * self.wheel_radius.raw()
            * std::f32::consts::PI
            / self.trackwidth.raw();

        // Translate to the robot's control point (center).
        // When turning, a point offset from the axle experiences a
        // sideways velocity component: v_y = ω · axle_offset.
        let x = Velocity::new::<meter_per_second>(v_axle);
        let y = Velocity::new::<meter_per_second>(omega * self.axle_offset.raw());
        let theta = AngularVelocity::new::<radian_per_second>(omega);

        output.set_payload(ChassisSpeeds { x, y, theta });
        Ok(())
    }
}
