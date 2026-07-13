use cu29::bincode::{Decode, Encode};
use moteus::Mode;
use serde::{Deserialize, Serialize};

// Shared bridge message type
#[derive(Default, Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub struct SharedBridgePayload {
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct MoteusCMD {
    pub can_id: u8,
    pub cmd: common::MotorCMD,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct MoteusData {
    pub canid: u8,
    pub data: common::MotorData,
    pub temp: f32,
    pub voltage: f32,
    pub fault: i8
}
