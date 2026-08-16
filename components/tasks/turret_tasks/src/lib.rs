use cu29::cutask::{CuMsg, CuTask, Freezable};
use cu29::reflect::Reflect;
use cu29::units::si::angle::degree;
use cu29::units::si::{
    angular_velocity::{radian_per_second, revolution_per_minute},
    f32::{Angle, AngularVelocity, Length, Velocity},
    length::meter,
    velocity::meter_per_second,
};
use cu29::{input_msg, output_msg, CuError, CuResult};

type TurretAngle = Angle;
type TargetAngle = Angle;

#[derive(Reflect)]
struct TurretAngleSolver {
    min: Angle,
    max: Angle,
}

impl Freezable for TurretAngleSolver {}

impl CuTask for TurretAngleSolver {
    type Input<'m> = input_msg!('m, TurretAngle, TargetAngle);
    type Output<'m> = output_msg!(Angle);
    type Resources<'r> = ();

    fn new(
        config: Option<&cu29::prelude::ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        const DEFAULT_MIN_DEG: f32 = -180.0;
        const DEFAULT_MAX_DEG: f32 = 180.0;

        let (min_deg, max_deg) = match config {
            Some(cfg) => (
                cfg.get::<f32>("min_deg")?.unwrap_or(DEFAULT_MIN_DEG),
                cfg.get::<f32>("max_deg")?.unwrap_or(DEFAULT_MAX_DEG),
            ),
            None => (DEFAULT_MIN_DEG, DEFAULT_MAX_DEG),
        };
        Ok(Self {
            min: Angle::new::<degree>(min_deg),
            max: Angle::new::<degree>(max_deg),
        })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (current, target) = *input;
        let (Some(current_angle), Some(target_angle)) = (current.payload(), target.payload())
        else {
            return Ok(());
        };

        let mut target_deg = target_angle.get::<degree>();

        // Normalize into 0-360 range
        while target_deg >= 360.0 {
            target_deg -= 360.0;
        }
        while target_deg < 0.0 {
            target_deg += 360.0;
        }

        let alt_deg = target_deg - 360.0;

        let target_valid = target_deg >= self.min.get::<degree>() && target_deg <= self.max.get::<degree>();
        let alt_valid = alt_deg >= self.min.get::<degree>() && alt_deg <= self.max.get::<degree>();

        let solved_deg = match (target_valid, alt_valid) {
            (true, true) => {
                let cur_deg = current_angle.get::<degree>();
                let d_target = (target_deg - cur_deg).abs();
                let d_alt = (alt_deg - cur_deg).abs();
                if d_target <= d_alt {
                    target_deg
                } else {
                    alt_deg
                }
            }
            (true, false) => target_deg,
            (false, true) => alt_deg,
            (false, false) => return Ok(()),
        };

        output.set_payload(Angle::new::<degree>(solved_deg));
        Ok(())
    }
}


struct TurretCheck {
    angle_tol: Angle,
    velocity_tol: AngularVelocity,
}

impl Freezable for TurretCheck {}

type DesiredState = common::TurretState;
type CurrentState = common::TurretState;

impl CuTask for TurretCheck {
    type Input<'m> = input_msg!('m, CurrentState, DesiredState);
    type Output<'m> = output_msg!(bool);
    type Resources<'r> = ();

    fn new(config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
        where
            Self: Sized {
        const DEFAULT_ANGLE_TOL_DEG: f32 = 2.0;
        const DEFAULT_VELOCITY_TOL_RPM: f32 = 10.0;

        let (angle_tol_deg, velocity_tol_rpm) = match config {
            Some(cfg) => (
                cfg.get::<f32>("angle_tol")?.unwrap_or(DEFAULT_ANGLE_TOL_DEG),
                cfg.get::<f32>("velocity_tol")?.unwrap_or(DEFAULT_VELOCITY_TOL_RPM),
            ),
            None => (DEFAULT_ANGLE_TOL_DEG, DEFAULT_VELOCITY_TOL_RPM),
        };

        Ok(Self {
            angle_tol: Angle::new::<degree>(angle_tol_deg),
            velocity_tol: AngularVelocity::new::<revolution_per_minute>(velocity_tol_rpm),
        })
    }

    fn process<'i, 'o>(
            &mut self,
            _ctx: &cu29::prelude::CuContext,
            input: &Self::Input<'i>,
            output: &mut Self::Output<'o>,
        ) -> CuResult<()> {
            let (current, desired) = *input;
            let (Some(current), Some(desired)) = (current.payload(), desired.payload())
            else {
                return Ok(());
            };

            let angle_err = (current.position.get::<degree>() - desired.position.get::<degree>()).abs();
            let velocity_err = (current.flywheel.get::<revolution_per_minute>() - desired.flywheel.get::<revolution_per_minute>()).abs();

            let in_tol = angle_err <= self.angle_tol.get::<degree>()
                && velocity_err <= self.velocity_tol.get::<revolution_per_minute>();

            output.set_payload(in_tol);

            Ok(())
    }
}

#[derive(Reflect)]
pub struct TurretStateAssembler {}

impl Freezable for TurretStateAssembler {}

type FlywheelSpeed = AngularVelocity;
type TurretPosition = Angle;

impl CuTask for TurretStateAssembler {
    type Input<'m> = input_msg!('m, FlywheelSpeed, TurretPosition);
    type Output<'m> = output_msg!(common::TurretState);
    type Resources<'r> = ();

    fn new(_config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (flywheel_msg, position_msg) = *input;
        let (Some(flywheel), Some(position)) = (flywheel_msg.payload(), position_msg.payload())
        else {
            return Ok(());
        };

        output.set_payload(common::TurretState {
            flywheel: *flywheel,
            position: *position,
        });
        Ok(())
    }
}

/// Assembles a turret state from a muzzle velocity and a turret position.
///
/// The muzzle velocity is converted to flywheel angular velocity with
/// `omega = muzzle_velocity / flywheel_radius`.
#[derive(Reflect)]
pub struct MuzzleVelocityTurretStateAssembler {
    flywheel_radius: Length,
}

type MuzzleVelocity = Velocity;

impl Freezable for MuzzleVelocityTurretStateAssembler {}

impl MuzzleVelocityTurretStateAssembler {
    fn assemble(
        muzzle_velocity: MuzzleVelocity,
        turret_position: TurretPosition,
        flywheel_radius: Length,
    ) -> common::TurretState {
        common::TurretState {
            flywheel: AngularVelocity::new::<radian_per_second>(
                muzzle_velocity.get::<meter_per_second>() / flywheel_radius.get::<meter>(),
            ),
            position: turret_position,
        }
    }
}

impl CuTask for MuzzleVelocityTurretStateAssembler {
    type Input<'m> = input_msg!('m, MuzzleVelocity, TurretPosition);
    type Output<'m> = output_msg!(common::TurretState);
    type Resources<'r> = ();

    fn new(
        config: Option<&cu29::prelude::ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        const DEFAULT_FLYWHEEL_RADIUS_M: f32 = 0.05;

        let flywheel_radius_m = match config {
            Some(cfg) => cfg
                .get::<f32>("flywheel_radius")?
                .unwrap_or(DEFAULT_FLYWHEEL_RADIUS_M),
            None => DEFAULT_FLYWHEEL_RADIUS_M,
        };
        if flywheel_radius_m <= 0.0 {
            return Err(CuError::from("flywheel_radius must be greater than zero"));
        }

        Ok(Self {
            flywheel_radius: Length::new::<meter>(flywheel_radius_m),
        })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (muzzle_velocity_msg, turret_position_msg) = *input;
        let (Some(muzzle_velocity), Some(turret_position)) =
            (muzzle_velocity_msg.payload(), turret_position_msg.payload())
        else {
            return Ok(());
        };

        output.set_payload(Self::assemble(
            *muzzle_velocity,
            *turret_position,
            self.flywheel_radius,
        ));
        Ok(())
    }
}
