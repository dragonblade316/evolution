mod bottom_bar;
mod gamepad;
mod telemetry;

use std::time::Duration;

use eframe::egui;
use tokio::sync::{mpsc, watch};

use crate::{DriverStationSnapshot, UiCommand};
use bottom_bar::BottomBar;
use telemetry::TelemetryViewer;

pub fn run(
    cmd_tx: mpsc::Sender<UiCommand>,
    status_rx: watch::Receiver<DriverStationSnapshot>,
) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "evods",
        options,
        Box::new(move |_cc| Ok(Box::new(EvodsApp::new(cmd_tx, status_rx)))),
    )
}

struct EvodsApp {
    cmd_tx: mpsc::Sender<UiCommand>,
    status_rx: watch::Receiver<DriverStationSnapshot>,
    bottom_bar: BottomBar,
    telemetry: TelemetryViewer,
}

impl EvodsApp {
    fn new(
        cmd_tx: mpsc::Sender<UiCommand>,
        status_rx: watch::Receiver<DriverStationSnapshot>,
    ) -> Self {
        Self {
            cmd_tx,
            status_rx,
            bottom_bar: BottomBar::default(),
            telemetry: TelemetryViewer,
        }
    }
}

impl eframe::App for EvodsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let status = self.status_rx.borrow_and_update().clone();
        self.bottom_bar
            .handle_keyboard(ui.ctx(), &status, &self.cmd_tx);
        self.bottom_bar.show(ui, &status, &self.cmd_tx);

        egui::CentralPanel::default().show(ui, |ui| {
            self.telemetry.show(ui, &status.telemetry);
        });

        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }
}
