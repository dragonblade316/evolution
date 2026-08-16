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
    pub left: Velocity,
    pub right: Velocity,
}

impl Default for DiffDriveSpeeds {
    fn default() -> Self {
        Self {
            left: Velocity::new::<meter_per_second>(0.0),
            right: Velocity::new::<meter_per_second>(0.0),
        }
    }
}

//motors
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Encode, Decode, Reflect)]
pub enum MotorCMD {
    Position(
        Angle,
        Option<AngularVelocity>,
        Option<Torque>,
    ),
    Velocity(AngularVelocity, Option<Torque>),
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

impl Default for DSStatus {
    fn default() -> Self {
        Self::DISCONNECTED
    }
}

/// Snapshot of a gamepad's state, intended as a copper-rs payload.
///
/// NOTE: menu buttons (Start / Select / Home) and stick buttons (LS / RS)
/// are intentionally omitted — they are not currently needed.
#[derive(Debug, Clone, Default, Encode, Decode, Serialize, Deserialize, Reflect)]
pub struct GamePadState {
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
    pub right_trigger: f32,
    pub left_trigger: f32,

    pub left_shoulder: bool,
    pub right_shoulder: bool,

    pub a: bool,
    pub x: bool,
    pub y: bool,
    pub b: bool,

    pub d_up: bool,
    pub d_right: bool,
    pub d_left: bool,
    pub d_down: bool,
}


#[derive(Debug, Clone, Default, Encode, Decode, Serialize, Deserialize, Reflect)]
pub struct TurretState {
    pub flywheel: AngularVelocity,
    pub position: Angle,
}
