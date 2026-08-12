use std::time::Duration;

use eframe::egui;
use tokio::sync::{mpsc, watch};

/// Latest joystick snapshot. Written by `joy_runtime` (std thread), read by
/// `zruntime` (tokio thread) via a `tokio::sync::watch`.
pub type PadState = common::GamePadState;

/// Commands the UI (main/egui thread) sends down to `zruntime`.
#[derive(Debug)]
pub enum UiCommand {
    /// Placeholder — replace with real commands later (enable, disable, mode, …).
    Ping,
}

/// Status / telemetry `zruntime` pushes back up to the UI.
#[derive(Debug)]
pub enum RobotStatus {
    /// Placeholder — replace with real status later (connection, voltage, …).
    Pong,
}

/// Runs on a plain OS thread with no async runtime.
///
/// Polls the gamepad and publishes the latest snapshot over the `watch`
/// channel. `watch::Sender::send` is sync, so no tokio runtime is needed here.
fn joy_runtime(pad_tx: watch::Sender<PadState>) {
    println!("[joy] thread started");
    let state = PadState::default();
    loop {
        // TODO: gilrs polling -> fill `state`.
        if pad_tx.send(state.clone()).is_err() {
            // All receivers dropped -> zruntime exited; stop polling.
            println!("[joy] zruntime gone, exiting");
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Runs on its own OS thread hosting a tokio runtime.
///
/// Reads the latest joystick snapshot from the `watch` receiver, handles UI
/// commands coming down from the egui thread, and pushes status updates back
/// up to it.
fn zruntime(
    mut pad_rx: watch::Receiver<PadState>,
    mut cmd_rx: mpsc::Receiver<UiCommand>,
    status_tx: mpsc::Sender<RobotStatus>,
) {
    println!("[zruntime] building tokio runtime");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");



    runtime.block_on(async move {
        println!("[zruntime] runtime running");
        // TODO: zenoh session + subscriber setup.
        let session = zenoh::open(zenoh::Config::default()).await.unwrap();
        let telsub = session.declare_subscriber("evods/tel").await.unwrap();

        loop {
            tokio::select! {
                Ok(()) = pad_rx.changed() => {
                    let _latest = pad_rx.borrow().clone();
                }
                Some(cmd) = cmd_rx.recv() => {
                }
                tel = telsub.recv_async() => {
                }

            }
        }
    })
}

fn main() -> eframe::Result<()> {
    // Joystick latest-state channel.
    let (pad_tx, pad_rx) = watch::channel(PadState::default());

    // UI -> zruntime commands.
    let (cmd_tx, cmd_rx) = mpsc::channel::<UiCommand>(32);

    // zruntime -> UI status.
    let (status_tx, status_rx) = mpsc::channel::<RobotStatus>(32);

    zenoh::

    // Spawn the two background workers up front, before the UI starts.
    std::thread::spawn(move || joy_runtime(pad_tx));
    std::thread::spawn(move || zruntime(pad_rx, cmd_rx, status_tx));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "evods",
        options,
        Box::new(move |_cc| Ok(Box::new(EvodsApp::new(cmd_tx, status_rx)))),
    )
}

struct EvodsApp {
    /// Sends commands down to `zruntime`. `try_send` is non-blocking and needs
    /// no tokio runtime, so it's safe to call from the egui thread.
    cmd_tx: mpsc::Sender<UiCommand>,
    /// Drains status updates from `zruntime`. `try_recv` is non-blocking.
    status_rx: mpsc::Receiver<RobotStatus>,
    /// Most recent status, shown in the UI.
    last_status: Option<RobotStatus>,
}

impl EvodsApp {
    fn new(cmd_tx: mpsc::Sender<UiCommand>, status_rx: mpsc::Receiver<RobotStatus>) -> Self {
        Self {
            cmd_tx,
            status_rx,
            last_status: None,
        }
    }
}

impl eframe::App for EvodsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drain any pending status updates from zruntime (non-blocking).
        while let Ok(status) = self.status_rx.try_recv() {
            self.last_status = Some(status);
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("evods");
            ui.label("driver station ui — todo");

            ui.separator();

            if ui.button("Ping zruntime").clicked() {
                // Non-blocking send from the UI thread.
                let _ = self.cmd_tx.try_send(UiCommand::Ping);
            }

            ui.separator();
            ui.label(format!(
                "last status: {:?}",
                self.last_status.as_ref().map(|s| s).map(|s| format!("{s:?}")).unwrap_or_default()
            ));
        });
    }
}
