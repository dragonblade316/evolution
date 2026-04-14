use bincode::{Decode, Encode};
use cu29::units::si::angular_velocity::radian_per_second;
use cu29::units::si::f32::*;
use cu29::{reflect::Reflect, units::si::velocity::meter_per_second};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, Reflect)]
pub struct ChassisSpeeds {
    pub x: Velocity,
    pub y: Velocity,
    pub theta: AngularVelocity,
}

impl Default for ChassisSpeeds {
    fn default() -> Self {
        Self {
            x: Velocity::new::<meter_per_second>(0.0),
            y: Velocity::new::<meter_per_second>(0.0),
            theta: AngularVelocity::new::<radian_per_second>(0.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, Reflect)]
pub struct DiffDriveSpeeds {
    pub left: AngularVelocity,
    pub right: AngularVelocity,
}

impl Default for DiffDriveSpeeds {
    fn default() -> Self {
        Self {
            left: AngularVelocity::new::<radian_per_second>(0.0),
            right: AngularVelocity::new::<radian_per_second>(0.0),
        }
    }
}


