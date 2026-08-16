
use cu29::{
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    units::si::{angular_velocity::revolution_per_minute, f32::AngularVelocity},
};
use common::{MotorCMD, TurretState};


use crate::messages::SuperState;

struct TurretCMD {
    max_turret_velocity: Option<AngularVelocity>,
    idle_flywheel_speed: AngularVelocity,
}

impl Freezable for TurretCMD {

}

type AngleMotorCmd = MotorCMD;
type FlywheelMotorCmd = MotorCMD;

impl CuTask for TurretCMD {

    type Input<'m> = input_msg!('m, SuperState, TurretState);
    type Output<'m> = output_msg!('m, FlywheelMotorCmd, AngleMotorCmd);
    type Resources<'r> = ();

    fn new(config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
        where
            Self: Sized {
                let (max_turret_velocity, idle_flywheel_speed) = match config {
                    Some(cfg) => (
                        cfg.get::<f32>("max_turret_velocity")?
                            .map(AngularVelocity::new::<revolution_per_minute>),
                        AngularVelocity::new::<revolution_per_minute>(
                            cfg.get::<f32>("idle_flywheel_speed")?.unwrap_or(0.0),
                        ),
                    ),
                    None => (None, AngularVelocity::new::<revolution_per_minute>(0.0)),
                };

                Ok(Self {
                    max_turret_velocity,
                    idle_flywheel_speed,
                })
    }

    fn process(
            &mut self,
            _ctx: &cu29::prelude::CuContext,
            input: &Self::Input<'_>,
            output: &mut Self::Output<'_>,
        ) -> cu29::CuResult<()> {
            let cmd = input.0.payload().unwrap_or(&SuperState::Disabled);
            let default_turret_state = TurretState::default();
            let turretd = input.1.payload().unwrap_or(&default_turret_state);
            match cmd {
                SuperState::Disabled => {
                    output.0.set_payload(MotorCMD::Stop);
                    output.1.set_payload(MotorCMD::Stop);
                },
                SuperState::Shooting | SuperState::WalkAndTalk => {
                    output.0.set_payload(MotorCMD::Velocity(turretd.flywheel, None));
                    output.1.set_payload(MotorCMD::Position(turretd.position, self.max_turret_velocity, None));
                }
                _ => {
                    output.0.set_payload(MotorCMD::Velocity(self.idle_flywheel_speed, None));
                    output.1.set_payload(MotorCMD::Position(turretd.position, self.max_turret_velocity, None));
                }
            };
            Ok(())
    }
}
