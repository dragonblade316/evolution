use cu29::{cutask::CuSinkTask, input_msg};



struct EvoRerun {
}

impl CuSinkTask for EvoRerun {
    type Input<'m> = input_msg!();
    type Resources<'r> = ();

    fn new(_config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
        where
            Self: Sized {

    }
}
