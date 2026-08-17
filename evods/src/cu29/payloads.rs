use common::{Allience, GamePadState};
use cu29::bincode::{Decode, Encode};
use cu29::reflect::Reflect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize, Reflect)]
pub struct DsTxMsg {
    /// Monotonic sequence number for ordering / freshness checks on the robot.
    pub seq: u64,
    pub allience: Allience,
    pub gamepad: GamePadState,
    pub enabled: bool,
    pub estop: bool,
}

impl Default for DsTxMsg {
    fn default() -> Self {
        Self {
            seq: 0,
            allience: Allience::BLUE,
            gamepad: GamePadState::default(),
            estop: false,
            enabled: false
        }
    }
}

#[derive(Debug, Clone, Default, Encode, Decode, Serialize, Deserialize, Reflect)]
pub struct DxRxMsg {
    /// Monotonic sequence number echoed from the robot (for loss detection / RTT).
    pub seq: u64,
}
