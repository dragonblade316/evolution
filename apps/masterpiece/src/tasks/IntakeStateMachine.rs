
use cu29::{clock::{CuDuration, CuInstant, Instant}, cutask::{CuMsg, CuTask, Freezable}, input_msg, output_msg, units::si::{angular_velocity::revolution_per_minute, f32::AngularVelocity}};
use common::{GamePadState, MotorCMD};


use crate::{messages::SuperState};

enum IntakeState {
    IDLE,
    PASS,
    RETRACT,
    INTAKING,
    OUTTAKING
}

struct S1IntakeStateMachine {
    state: IntakeState,
    retract_time: Instant,
    intake_speed: AngularVelocity,
}

impl Freezable for S1IntakeStateMachine{

}

impl S1IntakeStateMachine {
    fn retract(&mut self, ctx: &cu29::prelude::CuContext) {
        self.retract_time = CuInstant::now();
        self.state = IntakeState::RETRACT;
    }
}

impl CuTask for S1IntakeStateMachine {
    type Input<'m> = input_msg!(SuperState);
    type Output<'m> = output_msg!('m, MotorCMD, bool);
    type Resources<'r> = ();

    fn new(config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
        where
            Self: Sized {
                const DEFAULT_INTAKE_SPEED_RPM: f32 = 100.0;

                let intake_speed = match config {
                    Some(cfg) => match cfg.get::<f32>("intake_speed")? {
                        Some(speed_rpm) => AngularVelocity::new::<revolution_per_minute>(speed_rpm),
                        None => AngularVelocity::new::<revolution_per_minute>(DEFAULT_INTAKE_SPEED_RPM),
                    },
                    None => AngularVelocity::new::<revolution_per_minute>(DEFAULT_INTAKE_SPEED_RPM),
                };

                Ok(Self {
                    state: IntakeState::IDLE,
                    retract_time: CuInstant::now(),
                    intake_speed,
                })
    }

    fn process(
            &mut self,
            ctx: &cu29::prelude::CuContext,
            input: &Self::Input<'_>,
            output: &mut Self::Output<'_>,
        ) -> cu29::CuResult<()> {
            let movecmd = MotorCMD::Velocity(self.intake_speed, None);
            let revcmd = MotorCMD::Velocity(-self.intake_speed, None);
            let stopcmd = MotorCMD::Velocity(AngularVelocity::new::<revolution_per_minute>(0.0), None);


            let cmd = input.payload().unwrap_or(&SuperState::Disabled);

            match self.state {
                IntakeState::IDLE => {
                    output.0.set_payload(stopcmd);
                    output.1.set_payload(true);
                    match cmd {
                        SuperState::Shooting | SuperState::WalkAndTalk => self.state = IntakeState::PASS,
                        SuperState::Intaking => self.state = IntakeState::INTAKING,
                        SuperState::Outtaking => self.state = IntakeState::OUTTAKING,
                        _ => {}
                    }
                },
                IntakeState::INTAKING => {
                    output.0.set_payload(movecmd);
                    output.1.set_payload(true);
                    match cmd {
                        SuperState::Shooting | SuperState::WalkAndTalk => self.state = IntakeState::PASS,
                        SuperState::Intaking => {},
                        _ => self.state = IntakeState::IDLE,
                    }
                },
                IntakeState::PASS => {
                    output.0.set_payload(movecmd);
                    output.1.set_payload(false);
                    match cmd {
                        SuperState::Shooting | SuperState::WalkAndTalk => {},
                        _ => self.retract(ctx),
                    }
                },
                IntakeState::OUTTAKING => {
                    output.0.set_payload(revcmd);
                    output.1.set_payload(true);
                    match cmd {
                        SuperState::Outtaking => {},
                        _ => self.state = IntakeState::IDLE
                    }
                }
                IntakeState::RETRACT => {
                    output.0.set_payload(revcmd);
                    output.1.set_payload(true);
                    if CuInstant::now() - self.retract_time > CuDuration::from_millis(500) {
                        self.state = IntakeState::IDLE;
                    }
                }
            }
            return Ok(())
    }
}
