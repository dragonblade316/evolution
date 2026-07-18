use cu29::bincode::{Decode, Encode};
use cu29::units::si::angular_velocity::radian_per_second;
use cu29::units::si::f32::*;
use cu29::units::si::f32::Torque;
use cu29::{reflect::Reflect, units::si::velocity::meter_per_second};
use serde::{Deserialize, Serialize};


//kinematics
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

//motors
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, Reflect)]
pub enum MotorCMD {
    Position(
        Angle,
        Option<AngularVelocity>,
        Option<Torque>,
    ),
    Velocity(AngularVelocity, Option<Torque>),
    // Acceleration(AngularAcceleration, Option<Torque>),
    Torque(Torque),

    Stop,
}

impl Default for MotorCMD {
    fn default() -> Self {
        Self::Stop
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, Reflect)]
pub struct MotorData {
    pub pos: Angle,
    pub vel: AngularVelocity,
    pub accel: Option<AngularAcceleration>,
    pub torque: Option<Torque>,
}

impl Default for MotorData {
    fn default() -> Self {
        Self {
            pos: Angle::default(),
            vel: AngularVelocity::default(),
            accel: None,
            torque: None,
        }
    }
}

//DS
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, Reflect)]
pub enum Allience {
    RED,
    BLUE,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, Reflect)]
pub enum DSStatus {
    DISCONNECTED,
    DISABLED(Allience),
    ENABLED(Allience),
}
