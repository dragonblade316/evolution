use eframe::egui::{
    self, Color32, RectAlign, RichText, Sense, Stroke, StrokeKind, Vec2,
};

use crate::{GamepadSnapshot, PadState};

const GREEN: Color32 = Color32::from_rgb(46, 204, 113);
const MUTED: Color32 = Color32::from_rgb(148, 163, 184);
const PANEL_BACKGROUND: Color32 = Color32::from_rgb(36, 42, 52);
const ACTIVE_BUTTON: Color32 = Color32::from_rgb(96, 165, 250);

pub fn summary_button(ui: &mut egui::Ui, snapshot: &GamepadSnapshot) {
    let active = snapshot.connected && has_input_activity(&snapshot.state);
    let color = if snapshot.connected { GREEN } else { MUTED };
    let name = snapshot.name.as_deref().unwrap_or("NO XBOX CONTROLLER");
    let compact_name = if name.chars().count() > 22 {
        format!("{}…", name.chars().take(21).collect::<String>())
    } else {
        name.to_owned()
    };
    let label = if active {
        format!("● 🎮 {compact_name}")
    } else {
        format!("○ 🎮 {compact_name}")
    };

    let response = ui.add(
        egui::Button::new(RichText::new(label).strong().color(color))
            .fill(PANEL_BACKGROUND)
            .stroke(Stroke::new(1.0, color))
            .min_size(Vec2::new(180.0, 40.0)),
    );

    egui::Popup::menu(&response)
        .align(RectAlign::TOP_START)
        .width(340.0)
        .show(|ui| details(ui, snapshot));
}

fn details(ui: &mut egui::Ui, snapshot: &GamepadSnapshot) {
    ui.set_min_width(320.0);
    ui.heading(
        snapshot
            .name
            .as_deref()
            .unwrap_or("No accepted Xbox controller"),
    );
    ui.label(
        RichText::new(if snapshot.connected {
            "Connected"
        } else {
            "Disconnected"
        })
        .color(if snapshot.connected { GREEN } else { MUTED }),
    );
    ui.separator();

    if !snapshot.connected {
        ui.label("Connect an Xbox or XInput controller to view live inputs.");
        return;
    }

    ui.columns(2, |columns| {
        stick_plot(
            &mut columns[0],
            "LEFT STICK",
            snapshot.state.left_x,
            snapshot.state.left_y,
        );
        stick_plot(
            &mut columns[1],
            "RIGHT STICK",
            snapshot.state.right_x,
            snapshot.state.right_y,
        );
    });

    ui.add_space(6.0);
    trigger_bar(ui, "LEFT TRIGGER", snapshot.state.left_trigger);
    trigger_bar(ui, "RIGHT TRIGGER", snapshot.state.right_trigger);
    ui.add_space(6.0);

    egui::Grid::new("gamepad_buttons")
        .num_columns(4)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            button_indicator(ui, "LB", snapshot.state.left_shoulder);
            button_indicator(ui, "RB", snapshot.state.right_shoulder);
            button_indicator(ui, "A", snapshot.state.a);
            button_indicator(ui, "B", snapshot.state.b);
            ui.end_row();
            button_indicator(ui, "D↑", snapshot.state.d_up);
            button_indicator(ui, "D↓", snapshot.state.d_down);
            button_indicator(ui, "X", snapshot.state.x);
            button_indicator(ui, "Y", snapshot.state.y);
            ui.end_row();
            button_indicator(ui, "D←", snapshot.state.d_left);
            button_indicator(ui, "D→", snapshot.state.d_right);
        });
}

fn stick_plot(ui: &mut egui::Ui, label: &str, x: f32, y: f32) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(label).small().strong());
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(96.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 5.0, Color32::from_rgb(17, 24, 39));
        painter.rect_stroke(
            rect,
            5.0,
            Stroke::new(1.0, MUTED),
            StrokeKind::Inside,
        );
        painter.line_segment(
            [
                egui::pos2(rect.center().x, rect.top()),
                egui::pos2(rect.center().x, rect.bottom()),
            ],
            Stroke::new(1.0, Color32::from_gray(70)),
        );
        painter.line_segment(
            [
                egui::pos2(rect.left(), rect.center().y),
                egui::pos2(rect.right(), rect.center().y),
            ],
            Stroke::new(1.0, Color32::from_gray(70)),
        );

        let radius = rect.width() * 0.42;
        let position =
            rect.center() + egui::vec2(x.clamp(-1.0, 1.0), -y.clamp(-1.0, 1.0)) * radius;
        painter.circle_filled(position, 6.0, ACTIVE_BUTTON);
        response.on_hover_text(format!("x: {x:.2}\ny: {y:.2}"));
    });
}

fn trigger_bar(ui: &mut egui::Ui, label: &str, value: f32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).small());
        ui.add(
            egui::ProgressBar::new(value.clamp(0.0, 1.0))
                .desired_width(190.0)
                .desired_height(14.0)
                .fill(ACTIVE_BUTTON)
                .text(format!("{value:.2}")),
        );
    });
}

fn button_indicator(ui: &mut egui::Ui, label: &str, pressed: bool) {
    let color = if pressed { ACTIVE_BUTTON } else { MUTED };
    ui.label(RichText::new(format!("● {label}")).strong().color(color));
}

fn has_input_activity(state: &PadState) -> bool {
    state.left_x.abs() > 0.01
        || state.left_y.abs() > 0.01
        || state.right_x.abs() > 0.01
        || state.right_y.abs() > 0.01
        || state.left_trigger > 0.01
        || state.right_trigger > 0.01
        || state.left_shoulder
        || state.right_shoulder
        || state.a
        || state.b
        || state.x
        || state.y
        || state.d_up
        || state.d_down
        || state.d_left
        || state.d_right
}

#[cfg(test)]
mod tests {
    use super::has_input_activity;
    use crate::PadState;

    #[test]
    fn detects_gamepad_activity() {
        assert!(!has_input_activity(&PadState::default()));

        let mut state = PadState::default();
        state.a = true;
        assert!(has_input_activity(&state));

        state.a = false;
        state.left_x = 0.5;
        assert!(has_input_activity(&state));
    }
}
