use bincode::{Decode, Encode};
use cu29::prelude::*;
use serde::{Deserialize, Serialize};

// Define a message type
#[derive(Default, Debug, Clone, Encode, Decode, Serialize, Deserialize, Reflect)]
pub struct MyPayload {
    pub value: i32,
}

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize, Reflect)]
pub enum SuperState {
    Disabled,
    Idle,
    Intaking,
    Outtaking,
    Shooting,
    WalkAndTalk, //A nickname for the state where the robot intakes and shoots at the same time.
    Climb
}

impl Default for SuperState {
    fn default() -> Self {
        SuperState::Disabled
    }
}
