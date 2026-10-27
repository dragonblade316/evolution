use common::MotorCMD;
use cu29::prelude::*;

use crate::messages::SuperState;

#[derive(Reflect)]
pub struct MotorCmdGate;

impl Freezable for MotorCmdGate {}

impl CuTask for MotorCmdGate {
    type Input<'m> = input_msg!('m, MotorCMD, SuperState);
    type Output<'m> = output_msg!(MotorCMD);
    type Resources<'r> = ();

    fn new(
        _config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self> {
        Ok(Self)
    }

    fn process(
        &mut self,
        _ctx: &CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let command = match (input.0.payload(), input.1.payload()) {
            (Some(command), Some(state)) if !matches!(state, SuperState::Disabled) => *command,
            _ => MotorCMD::Stop,
        };
        output.set_payload(command);
        Ok(())
    }
}
