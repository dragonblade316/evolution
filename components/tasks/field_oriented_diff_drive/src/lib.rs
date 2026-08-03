use common::ChassisSpeeds;
use cu29::{
    CuError, CuResult, config::ComponentConfig, cutask::{CuMsg, CuTask, Freezable}, input_msg, output_msg, units::si::{
        angular_velocity::radian_per_second,
        f32::{AngularVelocity, Velocity},
        velocity::meter_per_second,
    }
};
use pid::Pid;

//note: this is one of the few times I think the AI has writen better code than I would have. Turns out detailed prompts are good.

/// Field-oriented diff-drive controller.
///
/// Compares a desired global-frame velocity to the robot's current
/// global-frame velocity and outputs a robot-relative command: forward
/// speed in X plus a rotational correction in theta.
///
/// A PID controller steers the robot toward the desired heading.
/// Forward speed is the desired speed magnitude, scaled down as the
/// rotational correction increases so the robot doesn't waste energy
/// pushing in the wrong direction while it turns.
///
/// Config keys (all optional):
///   kp             — proportional gain          (default: 2.0)
///   ki             — integral gain              (default: 0.1)
///   kd             — derivative gain            (default: 0.0)
///   output_limit   — max angular velocity rad/s (default: 5.0)
///   turn_slowdown  — forward reduction factor   (default: 0.5)
pub struct FieldOrientedDiffDrive {
    pid: Pid<f32>,
    turn_slowdown: f32,
}

impl Freezable for FieldOrientedDiffDrive {}

impl CuTask for FieldOrientedDiffDrive {
    type Input<'m> = input_msg!('m, ChassisSpeeds, ChassisSpeeds);
    //                          ^- desired      ^- current
    type Output<'m> = output_msg!(ChassisSpeeds);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        let kp: f32 = config_get(config, "kp", 2.0)?;
        let ki: f32 = config_get(config, "ki", 0.1)?;
        let kd: f32 = config_get(config, "kd", 0.0)?;
        let output_limit: f32 = config_get(config, "output_limit", 5.0)?;
        let turn_slowdown: f32 = config_get(config, "turn_slowdown", 0.5)?;


        let mut pid = Pid::new(0.0f32, output_limit);
        pid.p(kp, output_limit).i(ki, output_limit).d(kd, output_limit);

        Ok(Self { pid, turn_slowdown })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (desired_msg, current_msg) = *input;
        let desired = match desired_msg.payload() {
            Some(p) => p,
            None => return Ok(()),
        };
        let current = match current_msg.payload() {
            Some(p) => p,
            None => return Ok(()),
        };

        // Angles from velocity vectors in the global frame.
        let desired_angle = f32::atan2(desired.y.raw(), desired.x.raw());
        let current_angle = f32::atan2(current.y.raw(), current.x.raw());

        // Shortest-path error in [-π, π].
        let angle_error = {
            let raw = desired_angle - current_angle;
            f32::atan2(f32::sin(raw), f32::cos(raw))
        };

        // PID: setpoint=0, measurement = -error → output drives error→0.
        let ctrl = self.pid.next_control_output(-angle_error);
        let theta_cmd = ctrl.output;

        // Desired speed magnitude.
        let speed = f32::hypot(desired.x.raw(), desired.y.raw());

        // Reduce forward speed as turning intensity increases.
        // When straight (theta_cmd ≈ 0), forward = full speed.
        // When turning hard, forward speed drops to avoid wasting
        // traction pushing forward while the robot rotates in place.
        let forward = speed / (1.0 + self.turn_slowdown * theta_cmd.abs());

        output.set_payload(ChassisSpeeds {
            x: Velocity::new::<meter_per_second>(forward),
            y: Velocity::new::<meter_per_second>(0.0),
            theta: AngularVelocity::new::<radian_per_second>(theta_cmd),
        });
        Ok(())
    }
}

fn config_get<T: std::str::FromStr>(
    config: Option<&ComponentConfig>,
    key: &str,
    default: T,
) -> CuResult<T> {
    match config {
        Some(cfg) => match cfg.get::<String>(key)? {
            Some(s) => s
                .parse()
                .map_err(|_| CuError::from(format!("Invalid {key}"))),
            None => Ok(default),
        },
        None => Ok(default),
    }
}
