use std::time::Duration;

use gilrs::{Axis, Button, Gamepad, GamepadId, Gilrs};
use tokio::sync::watch;

pub type PadState = common::GamePadState;

#[derive(Debug, Clone, Default)]
pub struct GamepadSnapshot {
    pub connected: bool,
    pub name: Option<String>,
    pub state: PadState,
}

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const DEADBAND: f32 = 0.08;
const MICROSOFT_VENDOR_ID: u16 = 0x045e;

pub(crate) fn is_xbox_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("xbox") || name.contains("x-box") || name.contains("xinput")
}

fn is_xbox_gamepad(gamepad: &Gamepad<'_>) -> bool {
    gamepad.vendor_id() == Some(MICROSOFT_VENDOR_ID)
        || is_xbox_name(gamepad.name())
        || is_xbox_name(gamepad.os_name())
}

fn find_xbox_gamepad(gilrs: &Gilrs) -> Option<GamepadId> {
    gilrs
        .gamepads()
        .find(|(_, gamepad)| gamepad.is_connected() && is_xbox_gamepad(gamepad))
        .map(|(id, _)| id)
}

pub(crate) fn apply_gamepad_deadband(value: f32) -> f32 {
    let magnitude = value.abs();
    if magnitude <= DEADBAND {
        return 0.0;
    }

    let scaled = (magnitude - DEADBAND) / (1.0 - DEADBAND);
    value.signum() * scaled.clamp(0.0, 1.0)
}

fn read_trigger_value(gamepad: &Gamepad<'_>, trigger: Button) -> f32 {
    gamepad
        .button_data(trigger)
        .map(|data| apply_gamepad_deadband(data.value().clamp(0.0, 1.0)))
        .unwrap_or_default()
}

fn read_gamepad_state(gamepad: &Gamepad<'_>) -> PadState {
    PadState {
        left_x: apply_gamepad_deadband(gamepad.value(Axis::LeftStickX)),
        left_y: apply_gamepad_deadband(gamepad.value(Axis::LeftStickY)),
        right_x: apply_gamepad_deadband(gamepad.value(Axis::RightStickX)),
        right_y: apply_gamepad_deadband(gamepad.value(Axis::RightStickY)),
        left_trigger: read_trigger_value(gamepad, Button::LeftTrigger2),
        right_trigger: read_trigger_value(gamepad, Button::RightTrigger2),
        left_shoulder: gamepad.is_pressed(Button::LeftTrigger),
        right_shoulder: gamepad.is_pressed(Button::RightTrigger),
        a: gamepad.is_pressed(Button::South),
        b: gamepad.is_pressed(Button::East),
        x: gamepad.is_pressed(Button::West),
        y: gamepad.is_pressed(Button::North),
        d_up: gamepad.is_pressed(Button::DPadUp),
        d_right: gamepad.is_pressed(Button::DPadRight),
        d_left: gamepad.is_pressed(Button::DPadLeft),
        d_down: gamepad.is_pressed(Button::DPadDown),
    }
}

pub fn runtime(pad_tx: watch::Sender<GamepadSnapshot>) {
    println!("[joy] thread started");
    let mut gilrs = match Gilrs::new() {
        Ok(gilrs) => gilrs,
        Err(err) => {
            eprintln!("[joy] failed to initialize gamepad subsystem: {err}");
            return;
        }
    };
    let mut active_id = find_xbox_gamepad(&gilrs);

    loop {
        while let Some(event) = gilrs.next_event() {
            if Some(event.id) == active_id && !gilrs.gamepad(event.id).is_connected() {
                println!("[joy] Xbox controller disconnected");
                active_id = None;
            } else if active_id.is_none() {
                let gamepad = gilrs.gamepad(event.id);
                if gamepad.is_connected() && is_xbox_gamepad(&gamepad) {
                    println!("[joy] using Xbox controller: {}", gamepad.name());
                    active_id = Some(event.id);
                }
            }
        }

        if active_id.is_some_and(|id| !gilrs.gamepad(id).is_connected()) {
            active_id = None;
        }

        if active_id.is_none() {
            active_id = find_xbox_gamepad(&gilrs);
        }

        let snapshot = active_id
            .filter(|id| gilrs.gamepad(*id).is_connected())
            .map(|id| {
                let gamepad = gilrs.gamepad(id);
                GamepadSnapshot {
                    connected: true,
                    name: Some(gamepad.name().to_owned()),
                    state: read_gamepad_state(&gamepad),
                }
            })
            .unwrap_or_default();

        if pad_tx.send(snapshot).is_err() {
            println!("[joy] zruntime gone, exiting");
            break;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}
