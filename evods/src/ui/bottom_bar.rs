use std::time::{Duration, Instant};

use common::Allience;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use tokio::sync::mpsc;

use crate::{DriverStationSnapshot, RequestedRobotState, RobotStatus, UiCommand};

use super::gamepad;

const BAR_HEIGHT: f32 = 68.0;
const BAR_BACKGROUND: Color32 = Color32::from_rgb(25, 29, 36);
const PANEL_BACKGROUND: Color32 = Color32::from_rgb(36, 42, 52);
const MUTED: Color32 = Color32::from_rgb(148, 163, 184);
const GREEN: Color32 = Color32::from_rgb(46, 204, 113);
const AMBER: Color32 = Color32::from_rgb(245, 158, 11);
const RED: Color32 = Color32::from_rgb(239, 68, 68);
const BLUE: Color32 = Color32::from_rgb(59, 130, 246);

struct CommandError {
    message: String,
    occurred_at: Instant,
}

#[derive(Default)]
pub struct BottomBar {
    command_error: Option<CommandError>,
}

impl BottomBar {
    pub fn handle_keyboard(
        &mut self,
        ctx: &egui::Context,
        status: &DriverStationSnapshot,
        cmd_tx: &mpsc::Sender<UiCommand>,
    ) {
        let command = ctx.input(|input| {
            shortcut_command(
                input.key_pressed(egui::Key::Space),
                input.key_pressed(egui::Key::Enter),
                status,
            )
        });

        if let Some(command) = command {
            self.send_command(cmd_tx, command);
        }
    }

    pub fn show(
        &mut self,
        root_ui: &mut egui::Ui,
        status: &DriverStationSnapshot,
        cmd_tx: &mpsc::Sender<UiCommand>,
    ) {
        self.expire_command_error();

        egui::Panel::bottom("driver_station_bottom_bar")
            .exact_size(BAR_HEIGHT)
            .frame(
                egui::Frame::NONE
                    .fill(BAR_BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(55, 65, 81))),
            )
            .show(root_ui, |ui| {
                ui.horizontal_centered(|ui| {
                    connection_status(ui, status.robot);
                    ui.separator();
                    gamepad::summary_button(ui, &status.gamepad);
                    ui.separator();
                    requested_state(ui, status.requested_state, self.command_error.as_ref());

                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            let estopped =
                                status.requested_state == RequestedRobotState::EStopped;
                            if ui
                                .add_enabled(
                                    !estopped,
                                    egui::Button::new(
                                        RichText::new("E-STOP")
                                            .strong()
                                            .color(Color32::WHITE),
                                    )
                                    .fill(RED)
                                    .stroke(Stroke::new(
                                        1.0,
                                        Color32::from_rgb(127, 29, 29),
                                    ))
                                    .min_size(Vec2::new(96.0, 38.0)),
                                )
                                .on_hover_text("Immediately latch emergency stop (Space)")
                                .clicked()
                            {
                                self.send_command(cmd_tx, UiCommand::ESTOP);
                            }

                            let enable_allowed = status.robot == RobotStatus::Connected
                                && status.requested_state != RequestedRobotState::EStopped;
                            let enable_active =
                                status.requested_state == RequestedRobotState::Enabled;
                            if ui
                                .add_enabled(
                                    enable_allowed,
                                    command_button("ENABLE", enable_active, GREEN)
                                        .min_size(Vec2::new(84.0, 38.0)),
                                )
                                .on_hover_text(if enable_allowed {
                                    "Request robot enable (Enter)"
                                } else {
                                    "Enable requires robot communications and a cleared e-stop"
                                })
                                .clicked()
                            {
                                self.send_command(cmd_tx, UiCommand::Enable);
                            }

                            let disable_active =
                                status.requested_state == RequestedRobotState::Disabled;
                            if ui
                                .add(
                                    command_button("DISABLE", disable_active, AMBER)
                                        .min_size(Vec2::new(84.0, 38.0)),
                                )
                                .on_hover_text("Request robot disable (Enter)")
                                .clicked()
                            {
                                self.send_command(cmd_tx, UiCommand::Disable);
                            }

                            ui.separator();
                            let (label, color, next_allience) = match &status.allience {
                                Allience::RED => ("ALLIANCE: RED", RED, Allience::BLUE),
                                Allience::BLUE => ("ALLIANCE: BLUE", BLUE, Allience::RED),
                            };
                            if ui
                                .add(
                                    command_button(label, true, color)
                                        .min_size(Vec2::new(120.0, 38.0)),
                                )
                                .on_hover_text("Change alliance")
                                .clicked()
                            {
                                self.send_command(cmd_tx, UiCommand::SetAllience(next_allience));
                            }
                        },
                    );
                });
            });
    }

    fn send_command(&mut self, cmd_tx: &mpsc::Sender<UiCommand>, command: UiCommand) {
        if let Err(err) = cmd_tx.try_send(command) {
            self.command_error = Some(CommandError {
                message: format!("Command not sent: {err}"),
                occurred_at: Instant::now(),
            });
        }
    }

    fn expire_command_error(&mut self) {
        if self
            .command_error
            .as_ref()
            .is_some_and(|error| error.occurred_at.elapsed() > Duration::from_secs(3))
        {
            self.command_error = None;
        }
    }
}

fn shortcut_command(
    space_pressed: bool,
    enter_pressed: bool,
    status: &DriverStationSnapshot,
) -> Option<UiCommand> {
    if space_pressed {
        return Some(UiCommand::ESTOP);
    }

    if !enter_pressed {
        return None;
    }

    match status.requested_state {
        RequestedRobotState::Enabled => Some(UiCommand::Disable),
        RequestedRobotState::Disabled if status.robot == RobotStatus::Connected => {
            Some(UiCommand::Enable)
        }
        RequestedRobotState::Disabled | RequestedRobotState::EStopped => None,
    }
}

fn connection_status(ui: &mut egui::Ui, robot: RobotStatus) {
    let (color, title, detail) = match robot {
        RobotStatus::Connected => (GREEN, "ROBOT", "ONLINE"),
        RobotStatus::Disconnected => (MUTED, "ROBOT", "OFFLINE"),
    };

    ui.horizontal(|ui| {
        ui.colored_label(color, "●");
        ui.vertical(|ui| {
            ui.label(RichText::new(title).strong().color(Color32::WHITE));
            ui.label(RichText::new(detail).small().color(color));
        });
    });
}

fn requested_state(
    ui: &mut egui::Ui,
    state: RequestedRobotState,
    command_error: Option<&CommandError>,
) {
    let (label, color) = match state {
        RequestedRobotState::Disabled => ("DISABLED", MUTED),
        RequestedRobotState::Enabled => ("ENABLED", GREEN),
        RequestedRobotState::EStopped => ("E-STOPPED", RED),
    };

    ui.vertical(|ui| {
        ui.label(RichText::new(label).strong().color(color));
        if let Some(error) = command_error {
            ui.label(RichText::new(&error.message).small().color(RED));
        } else {
            let detail = if state == RequestedRobotState::EStopped {
                "Restart required"
            } else {
                "Requested state"
            };
            ui.label(RichText::new(detail).small().color(MUTED));
        }
    });
}

fn command_button(
    label: &'static str,
    active: bool,
    active_color: Color32,
) -> egui::Button<'static> {
    let fill = if active { active_color } else { PANEL_BACKGROUND };
    let text_color = if active { Color32::BLACK } else { Color32::WHITE };

    egui::Button::new(RichText::new(label).strong().color(text_color))
        .fill(fill)
        .stroke(Stroke::new(1.0, active_color))
}

#[cfg(test)]
mod tests {
    use super::shortcut_command;
    use crate::{DriverStationSnapshot, RequestedRobotState, RobotStatus, UiCommand};

    #[test]
    fn space_always_requests_an_emergency_stop() {
        let status = DriverStationSnapshot::default();

        assert!(matches!(
            shortcut_command(true, true, &status),
            Some(UiCommand::ESTOP)
        ));
    }

    #[test]
    fn enter_toggles_between_enable_and_disable_when_safe() {
        let mut status = DriverStationSnapshot {
            robot: RobotStatus::Connected,
            ..Default::default()
        };

        assert!(matches!(
            shortcut_command(false, true, &status),
            Some(UiCommand::Enable)
        ));

        status.requested_state = RequestedRobotState::Enabled;
        assert!(matches!(
            shortcut_command(false, true, &status),
            Some(UiCommand::Disable)
        ));
    }

    #[test]
    fn enter_cannot_enable_without_communications_or_after_estop() {
        let mut status = DriverStationSnapshot::default();
        assert!(shortcut_command(false, true, &status).is_none());

        status.robot = RobotStatus::Connected;
        status.requested_state = RequestedRobotState::EStopped;
        assert!(shortcut_command(false, true, &status).is_none());
    }
}
