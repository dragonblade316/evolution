use cu29::{CuError, cutask::{CuMsg, CuTask, Freezable}, input_msg, output_msg};
use common::GamePadState;


use crate::messages::SuperState;

struct S1Behavior {

}

impl Freezable for S1Behavior {

}

impl CuTask for S1Behavior {

    type Input<'m> = input_msg!('m, GamePadState, bool);
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

            let joy = input.0.payload().unwrap_or(todo!());
            let ready_to_fire = input.1.payload().unwrap_or(&false);

            if true {
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
            } else {
                output.set_payload(SuperState::Disabled);
                return Ok(());
            }
    }
}
