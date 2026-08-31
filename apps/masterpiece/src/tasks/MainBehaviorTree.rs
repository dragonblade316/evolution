use cu29::{CuError, cutask::{CuMsg, CuTask, Freezable}, input_msg, output_msg};
use cu29::prelude::*;
use cu29::reflect::Reflect;
use common::{DSStatus, GamePadState};


use crate::{messages::SuperState};

#[derive(Reflect)]
pub struct S1Behavior {

}

impl Freezable for S1Behavior {

}

impl CuTask for S1Behavior {

    //The bool is is the turret is ready to fire.
    type Input<'m> = input_msg!('m, DSStatus, GamePadState, bool);
    type Output<'m> = output_msg!(SuperState);
    type Resources<'r> = ();

    fn new(_config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
        where
            Self: Sized {
                Ok(Self {})
    }

    fn process(
            &mut self,
            _ctx: &cu29::prelude::CuContext,
            input: &Self::Input<'_>,
            output: &mut Self::Output<'_>,
        ) -> cu29::CuResult<()> {

            let defaultds = DSStatus::DISCONNECTED;
            let defaultgp = GamePadState::default();


            let ds = input.0.payload().unwrap_or(&defaultds);
            let joy = input.1.payload().unwrap_or(&defaultgp);
            let ready_to_fire = input.2.payload().unwrap_or(&false);

            match ds {

                DSStatus::ENABLED(t) => {
                    if joy.right_shoulder && joy.left_shoulder {
                        if *ready_to_fire {
                            output.set_payload(SuperState::WalkAndTalk);
                            return Ok(());
                        } else {
                            output.set_payload(SuperState::Intaking);
                            return Ok(())
                        }
                    }
                    if joy.right_shoulder {
                        output.set_payload(SuperState::Intaking);
                        return Ok(())
                    }
                    if joy.left_shoulder {
                        if *ready_to_fire {
                            output.set_payload(SuperState::Shooting);
                            return Ok(());
                        } else {
                            output.set_payload(SuperState::Idle);
                            return Ok(());
                        }
                    }
                    if joy.y {
                        output.set_payload(SuperState::Outtaking);
                        return Ok(());
                    }
                    output.set_payload(SuperState::Idle);
                    return Ok(());
                }
                _ => {
                    output.set_payload(SuperState::Disabled);
                    return Ok(());
                }
            };
    }
}
