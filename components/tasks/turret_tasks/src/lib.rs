use cu29::cutask::{CuMsg, CuTask, Freezable};
use cu29::reflect::Reflect;
use cu29::units::si::angle::degree;
use cu29::units::si::f32::Angle;
use cu29::{input_msg, output_msg, CuResult};

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
