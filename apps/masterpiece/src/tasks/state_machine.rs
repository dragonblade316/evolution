use cu29::prelude::*;

use crate::messages::{MyPayload, SuperState};

// Defines a processing task
#[derive(Reflect)]
pub struct MyTask {
    // if you add some task state here, you need to implement the Freezable trait
}

// Needs to be fully implemented if you want to have a stateful task.
impl Freezable for MyTask {}

impl CuTask for MyTask {
    type Resources<'r> = ();
    type Input<'m> = ();
    type Output<'m> = output_msg!(SuperState);

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        // add the task state initialization here
        Ok(Self {})
    }

    // don't forget the other lifecycle methods if you need them: start, stop, preprocess, postprocess

    fn process(
        &mut self,
        _ctx: &CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        //Check if the current state is still valid based on sensors and ds status.
        //Check to see if the driver has triggered a state change.
        //Change internal state.
        //Broadcast the state.
        Ok(()) // outputs another message for downstream
    }
}
