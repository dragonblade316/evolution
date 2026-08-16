use common::{DiffDriveSpeeds, TurretState};
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    reflect::Reflect,
    units::si::{
        angular_velocity::radian_per_second,
        f32::{Length, Velocity},
        length::meter,
        velocity::meter_per_second,
    },
    CuResult,
};
use moteus_bridge::messages::MoteusData;

// ---------------------------------------------------------------------------
// MoteusDiff — per-motor MoteusData → DiffDriveSpeeds
// ---------------------------------------------------------------------------

type LeftMoteusData = MoteusData;
type RightMoteusData = MoteusData;

/// Collects individual motor telemetry and produces diff-drive wheel speeds.
///
/// Config keys:
///   wheel_radius — wheel radius in meters (default: 0.1)
#[derive(Reflect)]
pub struct MoteusDiff {
    wheel_radius: Length,
}

impl Freezable for MoteusDiff {}

impl CuTask for MoteusDiff {
    type Input<'m> = input_msg!('m, LeftMoteusData, RightMoteusData);
    type Output<'m> = output_msg!(DiffDriveSpeeds);
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
        let (left_msg, right_msg) = *input;
        let (Some(left), Some(right)) = (left_msg.payload(), right_msg.payload()) else {
            return Ok(());
        };

        let r = self.wheel_radius.get::<meter>();
        output.set_payload(DiffDriveSpeeds {
            left: Velocity::new::<meter_per_second>(left.data.vel.get::<radian_per_second>() * r),
            right: Velocity::new::<meter_per_second>(right.data.vel.get::<radian_per_second>() * r),
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MoteusTurretStateAssembler — flywheel/turret MoteusData → TurretState
// ---------------------------------------------------------------------------

type FlywheelMoteusData = MoteusData;
type TurretMoteusData = MoteusData;

/// Combines flywheel and turret motor telemetry into the turret state consumed
/// by turret control tasks.
///
/// The flywheel speed comes from the flywheel motor velocity, and the turret
/// motor supplies the turret position.
#[derive(Reflect)]
pub struct MoteusTurretStateAssembler {}

impl Freezable for MoteusTurretStateAssembler {}

impl MoteusTurretStateAssembler {
    fn assemble(flywheel: &MoteusData, turret: &MoteusData) -> TurretState {
        TurretState {
            flywheel: flywheel.data.vel,
            position: turret.data.pos,
        }
    }
}

impl CuTask for MoteusTurretStateAssembler {
    type Input<'m> = input_msg!('m, FlywheelMoteusData, TurretMoteusData);
    type Output<'m> = output_msg!(TurretState);
    type Resources<'r> = ();

    fn new(
        _config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
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
        let (flywheel_msg, turret_msg) = *input;
        let (Some(flywheel), Some(turret)) = (flywheel_msg.payload(), turret_msg.payload()) else {
            return Ok(());
        };

        output.set_payload(Self::assemble(flywheel, turret));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cu29::units::si::{
        angle::degree,
        angular_velocity::revolution_per_minute,
        f32::{Angle, AngularVelocity},
    };

    #[test]
    fn assembles_turret_state_from_the_two_motor_messages() {
        let flywheel = MoteusData {
            data: common::MotorData {
                vel: AngularVelocity::new::<revolution_per_minute>(4_200.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let turret = MoteusData {
            data: common::MotorData {
                pos: Angle::new::<degree>(135.0),
                ..Default::default()
            },
            ..Default::default()
        };

        let state = MoteusTurretStateAssembler::assemble(&flywheel, &turret);

        assert_eq!(state.flywheel.get::<revolution_per_minute>(), 4_200.0);
        assert_eq!(state.position.get::<degree>(), 135.0);
    }
}
