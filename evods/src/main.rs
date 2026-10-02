use std::{collections::BTreeMap, sync::Arc, time::Duration};

use ::cu29::cutask::CuMsg;
pub use evods::cu29;
use tokio::sync::{mpsc, watch};

mod joy;
mod ui;

pub use joy::{GamepadSnapshot, PadState};

/// Commands the UI (main/egui thread) sends down to `zruntime`.
#[derive(Debug)]
pub enum UiCommand {
    SetAllience(common::Allience),
    Enable,
    Disable,
    ESTOP,
}

/// Status / telemetry `zruntime` pushes back up to the UI.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RobotStatus {
    Connected,
    #[default]
    Disconnected,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequestedRobotState {
    #[default]
    Disabled,
    Enabled,
    EStopped,
}

#[derive(Debug, Clone)]
pub struct DriverStationSnapshot {
    pub robot: RobotStatus,
    pub requested_state: RequestedRobotState,
    pub allience: common::Allience,
    pub gamepad: GamepadSnapshot,
    pub telemetry: Arc<BTreeMap<String, serde_json::Value>>,
}

impl Default for DriverStationSnapshot {
    fn default() -> Self {
        Self {
            robot: RobotStatus::default(),
            requested_state: RequestedRobotState::default(),
            allience: common::Allience::BLUE,
            gamepad: GamepadSnapshot::default(),
            telemetry: Arc::default(),
        }
    }
}

fn decode_telemetry(payload: &[u8]) -> serde_json::Result<serde_json::Value> {
    serde_json::from_slice(payload)
}

fn robot_status_from_matching_subscriber(matching: bool) -> RobotStatus {
    if matching {
        RobotStatus::Connected
    } else {
        RobotStatus::Disconnected
    }
}

fn apply_ui_command(
    command: UiCommand,
    ds_state: &mut cu29::payloads::DsTxMsg,
    status: &mut DriverStationSnapshot,
) {
    match command {
        UiCommand::SetAllience(allience) => {
            ds_state.allience = allience.clone();
            status.allience = allience;
        }
        UiCommand::Enable
            if !ds_state.estop && status.robot == RobotStatus::Connected =>
        {
            ds_state.enabled = true;
            status.requested_state = RequestedRobotState::Enabled;
        }
        UiCommand::Enable => {}
        UiCommand::Disable => {
            ds_state.enabled = false;
            if !ds_state.estop {
                status.requested_state = RequestedRobotState::Disabled;
            }
        }
        UiCommand::ESTOP => {
            ds_state.enabled = false;
            ds_state.estop = true;
            status.requested_state = RequestedRobotState::EStopped;
        }
    }
}

fn zruntime(
    mut pad_rx: watch::Receiver<GamepadSnapshot>,
    mut cmd_rx: mpsc::Receiver<UiCommand>,
    status_tx: watch::Sender<DriverStationSnapshot>,
) {
    println!("[zruntime] building tokio runtime");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let mut ds_state = cu29::payloads::DsTxMsg::default();
    let mut status = DriverStationSnapshot::default();

    runtime.block_on(async move {
        println!("[zruntime] runtime running");
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let telsub = session.declare_subscriber("evods/tel/*").await.unwrap();
        let dspub = session.declare_publisher("evods/ds").await.unwrap();
        let ds_matching = dspub.matching_listener().await.unwrap();
        let mut ds_tx = tokio::time::interval(Duration::from_millis(20));
        ds_tx.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                Ok(()) = pad_rx.changed() => {
                    let gamepad = pad_rx.borrow_and_update().clone();
                    ds_state.gamepad = gamepad.state.clone();
                    status.gamepad = gamepad;
                    status_tx.send_replace(status.clone());
                }
                Some(cmd) = cmd_rx.recv() => {
                    apply_ui_command(cmd, &mut ds_state, &mut status);
                    status_tx.send_replace(status.clone());
                }
                tel = telsub.recv_async() => {
                    let sample = match tel {
                        Ok(sample) => sample,
                        Err(err) => {
                            eprintln!("[zruntime] telemetry subscriber closed: {err}");
                            status.robot = RobotStatus::Disconnected;
                            status_tx.send_replace(status.clone());
                            break;
                        }
                    };

                    let payload = sample.payload().to_bytes();
                    match decode_telemetry(payload.as_ref()) {
                        Ok(message) => {
                            Arc::make_mut(&mut status.telemetry)
                                .insert(sample.key_expr().to_string(), message);
                            status_tx.send_replace(status.clone());
                        }
                        Err(err) => eprintln!("[zruntime] invalid telemetry message: {err}"),
                    }
                }
                matching = ds_matching.recv_async() => {
                    let matching = match matching {
                        Ok(matching) => matching,
                        Err(err) => {
                            eprintln!("[zruntime] driver-station matching listener closed: {err}");
                            status.robot = RobotStatus::Disconnected;
                            status_tx.send_replace(status.clone());
                            break;
                        }
                    };
                    let robot = robot_status_from_matching_subscriber(matching.matching());
                    if robot != status.robot {
                        status.robot = robot;
                        status_tx.send_replace(status.clone());
                    }
                }
                _ = ds_tx.tick() => {
                    // TODO: replace this fixed-rate broadcast with the final DS transport scheduler.
                    ds_state.seq += 1;
                    match serde_json::to_vec(&CuMsg::new(Some(ds_state.clone()))) {
                        Ok(message) => {
                            if let Err(err) = dspub.put(message).await {
                                eprintln!("[zruntime] failed to send driver-station message: {err}");
                            }
                        }
                        Err(err) => eprintln!("[zruntime] failed to encode driver-station message: {err}"),
                    }
                }
            }
        }
    })
}

fn main() -> eframe::Result<()> {
    // Joystick latest-state channel.
    let (pad_tx, pad_rx) = watch::channel(GamepadSnapshot::default());

    // UI -> zruntime commands.
    let (cmd_tx, cmd_rx) = mpsc::channel::<UiCommand>(32);

    // zruntime -> UI status.
    let (status_tx, status_rx) = watch::channel(DriverStationSnapshot::default());

    // Spawn the two background workers up front, before the UI starts.
    std::thread::spawn(move || joy::runtime(pad_tx));
    std::thread::spawn(move || zruntime(pad_rx, cmd_rx, status_tx));

    ui::run(cmd_tx, status_rx)
}

#[cfg(test)]
mod tests;
