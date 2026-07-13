use common::ChassisSpeeds;
use cu29::{
    clock::CuTime,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    units::si::{
        acceleration::meter_per_second_squared,
        angular_velocity::radian_per_second,
        f32::{Acceleration, AngularVelocity, Velocity},
        velocity::meter_per_second,
    },
};

struct DiffDriveKinematics {
    accel_limit: Acceleration,
    last_time: Option<CuTime>,
}

impl Freezable for DiffDriveKinematics {}

impl CuTask for DiffDriveKinematics {
    type Input<'m> = input_msg!('m, ChassisSpeeds, ChassisSpeeds);
    type Output<'m> = output_msg!(ChassisSpeeds);
    type Resources<'r> = ();

    fn new(
        config: Option<&cu29::prelude::ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> cu29::CuResult<Self>
    where
        Self: Sized,
    {
        const DEFAULT_ACCEL_LIMIT_MPS2: f32 = 0.50;

        let accel_limit = match config {
            Some(cfg) => Acceleration::new::<meter_per_second_squared>(
                cfg.get::<f32>("accel_limit")?.unwrap_or(DEFAULT_ACCEL_LIMIT_MPS2),
            ),
            None => {
                Acceleration::new::<meter_per_second_squared>(DEFAULT_ACCEL_LIMIT_MPS2)
            }
        };

        Ok(Self {
            accel_limit,
            last_time: None,
        })
    }

    fn process(
        &mut self,
        ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> cu29::CuResult<()> {
        let (cmd_msg, current_msg) = *input;
        let cmd = match cmd_msg.payload() {
            Some(p) => p,
            None => return Ok(()),
        };
        let current = match current_msg.payload() {
            Some(p) => p,
            None => return Ok(()),
        };

        let now = ctx.now();

        let limited = if let Some(last) = self.last_time {
            let dt_ns = (now - last).as_nanos();
            if dt_ns > 0 {
                let dt_s = dt_ns as f32 / 1_000_000_000.0;
                let max_delta = self.accel_limit.raw() * dt_s;

                let mut limited = cmd.clone();
                limited.x = Velocity::new::<meter_per_second>(clamp_axis(
                    cmd.x.raw(),
                    current.x.raw(),
                    max_delta,
                ));
                limited.y = Velocity::new::<meter_per_second>(clamp_axis(
                    cmd.y.raw(),
                    current.y.raw(),
                    max_delta,
                ));
                limited.theta = AngularVelocity::new::<radian_per_second>(clamp_axis(
                    cmd.theta.raw(),
                    current.theta.raw(),
                    max_delta,
                ));
                limited
            } else {
                cmd.clone()
            }
        } else {
            cmd.clone()
        };

        output.set_payload(limited);
        self.last_time = Some(now);

        Ok(())
    }
}

fn clamp_axis(desired: f32, current: f32, max_delta: f32) -> f32 {
    let error = desired - current;
    current + error.clamp(-max_delta, max_delta)
}
