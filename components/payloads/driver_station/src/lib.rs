//! Driver station wire messages exchanged with the robot over zenoh.
//!
//! The DS <-> robot protocol is intentionally a single `DsTx` (DS -> robot) and
//! single `DsRx` (robot -> DS) message so that on the robot side each maps to one
//! copper-rs rx/tx channel. These types double as copper-rs payloads: they derive
//! `Encode`/`Decode` (cu-bincode) and `Reflect` in addition to `serde` so they can
//! cross zenoh as JSON here while remaining usable as copper payloads elsewhere.
//!
//! Existing shared protocol types (`GamePadState`, `DSStatus`, `Allience`) remain
//! in the `common` crate; this module references them via `common::` directly.

use common::{Allience, DSStatus, GamePadState};
use cu29::bincode::{Decode, Encode};
use cu29::reflect::Reflect;
use serde::{Deserialize, Serialize};

/// Discrete commands the driver station can issue to the robot, distinct from
/// the continuously-streamed `DSStatus` / gamepad telemetry carried in `DsTx`.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize, Reflect)]
pub enum DsCommand {
    /// Enable the robot under the given alliance.
    Enable(Allience),
    /// Disable the robot (safe state, motors coast/brake per robot policy).
    Disable,
    /// Emergency stop — drop everything immediately.
    Estop,
    /// Change alliance without toggling enable.
    Alliance(Allience),
}

/// All telemetry flowing DS -> robot, serialized as JSON over zenoh key `ds/tx`.
///
/// Sent at a fixed cadence (~50 Hz) by the zenoh hub thread. The robot side
/// subscribes and folds the latest sample into its control loop.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize, Reflect)]
pub struct DsTx {
    /// Monotonic sequence number for ordering / freshness checks on the robot.
    pub seq: u32,
    /// Current driver station logical status (enabled/disabled + alliance).
    pub status: DSStatus,
    /// Latest gamepad snapshot captured by the gamepad thread.
    pub gamepad: GamePadState,
    /// Latched emergency-stop flag. Once set, stays set until DS resets it.
    pub estop: bool,
    /// A discrete command to apply this tick, if any. `None` means teleop steady-state.
    pub command: Option<DsCommand>,
}

impl Default for DsTx {
    fn default() -> Self {
        Self {
            seq: 0,
            status: DSStatus::DISCONNECTED,
            gamepad: GamePadState::default(),
            estop: false,
            command: None,
        }
    }
}

/// All telemetry flowing robot -> DS, serialized as JSON over zenoh key `ds/rx`.
///
/// Published by the robot; the DS zenoh hub turns each received sample into an
/// `Update::Telemetry` event forwarded to the UI thread.
#[derive(Debug, Clone, Default, Encode, Decode, Serialize, Deserialize, Reflect)]
pub struct DsRx {
    /// Monotonic sequence number echoed from the robot (for loss detection / RTT).
    pub seq: u32,
    /// Whether the robot considers itself connected to a controller / capable of motion.
    pub connected: bool,
    /// Battery voltage in volts (e.g. 12.4). UI surfaces this prominently.
    pub battery: f32,
}

/// Zenoh key expression for the DS -> robot channel.
pub const TX_KEY: &str = "ds/tx";
/// Zenoh key expression for the robot -> DS channel.
pub const RX_KEY: &str = "ds/rx";