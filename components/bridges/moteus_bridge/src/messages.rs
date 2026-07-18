use cu29::bincode::{Decode, Encode};
use moteus::Mode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct MoteusData {
    pub canid: u8,
    pub data: common::MotorData,
    pub temp: f32,
    pub voltage: f32,
    pub fault: i8
}

impl Default for MoteusData {
    fn default() -> Self {
        Self {
            canid: 0,
            data: common::MotorData::default(),
            temp: 0.0,
            voltage: 0.0,
            fault: 0,
        }
    }
}
