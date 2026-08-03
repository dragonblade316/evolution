use common::GamePadState;
use cu29::prelude::*;
use gilrs::{Axis, Button, Gamepad, Gilrs, GamepadId};

// ---------------------------------------------------------------------------
// JoySource — copper-rs source that reads joystick input
// ---------------------------------------------------------------------------

/// A copper-rs source task that reads gamepad / joystick state and outputs a
/// [`GamePadState`] message every cycle.
///
/// Uses [`gilrs`](https://crates.io/crates/gilrs) under the hood.  Only Xbox
/// controllers are accepted; if none is connected, a default (zeroed) state is
/// emitted.
///
/// Config keys:
///   deadband — stick axis deadzone in [0, 1) (default: 0.08)
#[derive(Reflect)]
pub struct JoySource {
    gilrs: Gilrs,
    /// The id of the Xbox gamepad we are tracking.
    active_id: Option<GamepadId>,
    /// Axis magnitude below this is treated as zero; remaining range is rescaled.
    deadband: f32,
}

impl Freezable for JoySource {}

const DEFAULT_DEADBAND: f32 = 0.08;

fn is_xbox_gamepad(gp: &Gamepad<'_>) -> bool {
    let name = gp.name().to_ascii_lowercase();
    name.contains("xbox") || name.contains("x-box") || name.contains("xinput")
}

fn find_xbox_gamepad(gilrs: &Gilrs) -> Option<GamepadId> {
    gilrs
        .gamepads()
        .find(|(_, gp)| gp.is_connected() && is_xbox_gamepad(gp))
        .map(|(id, _)| id)
}

/// Zero out values inside the deadband and rescale the rest to [-1, 1].
fn apply_deadband(value: f32, deadband: f32) -> f32 {
    let abs = value.abs();
    if abs <= deadband {
        return 0.0;
    }
    let sign = value.signum();
    let scaled = (abs - deadband) / (1.0 - deadband);
    sign * scaled.clamp(0.0, 1.0)
}

impl CuSrcTask for JoySource {
    type Resources<'r> = ();
    type Output<'m> = output_msg!(GamePadState);

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let gilrs = Gilrs::new().map_err(|e| {
            CuError::from(format!("Failed to initialize gilrs gamepad subsystem: {e}"))
        })?;
        let active_id = find_xbox_gamepad(&gilrs);
        let deadband = match config {
            Some(cfg) => cfg.get::<f32>("deadband")?.unwrap_or(DEFAULT_DEADBAND),
            None => DEFAULT_DEADBAND,
        }
        .clamp(0.0, 0.95);

        Ok(Self {
            gilrs,
            active_id,
            deadband,
        })
    }

    fn start(&mut self, _ctx: &CuContext) -> CuResult<()> {
        // Re-detect gamepad in case one was connected after construction.
        if self.active_id.is_none() {
            self.active_id = find_xbox_gamepad(&self.gilrs);
        }
        Ok(())
    }

    fn stop(&mut self, _ctx: &CuContext) -> CuResult<()> {
        Ok(())
    }

    fn process(&mut self, _ctx: &CuContext, output: &mut Self::Output<'_>) -> CuResult<()> {
        // --- drain and apply all pending events ---
        while let Some(event) = self.gilrs.next_event() {
            if self.active_id.is_none() {
                let gp = self.gilrs.gamepad(event.id);
                if is_xbox_gamepad(&gp) {
                    self.active_id = Some(event.id);
                }
            }
            if Some(event.id) != self.active_id {
                continue;
            }
            self.gilrs.update(&event);
        }

        // If the active pad dropped, try to reacquire an Xbox controller.
        if self.active_id.is_none() {
            self.active_id = find_xbox_gamepad(&self.gilrs);
        }

        let mut state = GamePadState::default();

        if let Some(id) = self.active_id {
            if self.gilrs.gamepad(id).is_connected() {
                let gp = self.gilrs.gamepad(id);

                // axes (deadbanded + rescaled so full throw still reaches ±1)
                let db = self.deadband;
                state.left_x = apply_deadband(gp.value(Axis::LeftStickX), db);
                state.left_y = apply_deadband(gp.value(Axis::LeftStickY), db);
                state.right_x = apply_deadband(gp.value(Axis::RightStickX), db);
                state.right_y = apply_deadband(gp.value(Axis::RightStickY), db);
                // gilrs maps analog triggers to -1..1; remap to 0..1
                state.left_trigger = apply_deadband((gp.value(Axis::LeftZ) + 1.0) / 2.0, db);
                state.right_trigger = apply_deadband((gp.value(Axis::RightZ) + 1.0) / 2.0, db);

                // shoulders
                state.left_shoulder = gp.is_pressed(Button::LeftTrigger);
                state.right_shoulder = gp.is_pressed(Button::RightTrigger);

                // face buttons
                state.a = gp.is_pressed(Button::South);
                state.b = gp.is_pressed(Button::East);
                state.x = gp.is_pressed(Button::North);
                state.y = gp.is_pressed(Button::West);

                // dpad
                state.d_up = gp.is_pressed(Button::DPadUp);
                state.d_down = gp.is_pressed(Button::DPadDown);
                state.d_left = gp.is_pressed(Button::DPadLeft);
                state.d_right = gp.is_pressed(Button::DPadRight);
            } else {
                self.active_id = None;
            }
        }

        // println!("joy: {:?}", state);

        output.set_payload(state);
        Ok(())
    }
}
