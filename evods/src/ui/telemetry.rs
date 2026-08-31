use std::collections::BTreeMap;

use eframe::egui::{self, Color32, RichText};
use serde_json::Value;

pub struct TelemetryViewer;

impl TelemetryViewer {
    pub fn show(&mut self, ui: &mut egui::Ui, telemetry: &BTreeMap<String, Value>) {
        ui.heading("Telemetry");
        ui.separator();

        if telemetry.is_empty() {
            ui.label("Waiting for JSON telemetry on evods/tel/*");
            return;
        }

        egui::ScrollArea::both().show(ui, |ui| {
            for (route, value) in telemetry {
                egui::CollapsingHeader::new(RichText::new(route).strong())
                    .default_open(true)
                    .show(ui, |ui| show_value(ui, value));
            }
        });
    }
}

fn show_value(ui: &mut egui::Ui, value: &Value) {
    match value {
        Value::Object(fields) => {
            for (name, value) in fields {
                show_entry(ui, name, value);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                show_entry(ui, &index.to_string(), value);
            }
        }
        _ => show_scalar(ui, value),
    }
}

fn show_entry(ui: &mut egui::Ui, name: &str, value: &Value) {
    match value {
        Value::Object(fields) if fields.is_empty() => {
            ui.label(format!("{name}: {{}}"));
        }
        Value::Array(values) if values.is_empty() => {
            ui.label(format!("{name}: []"));
        }
        Value::Object(_) | Value::Array(_) => {
            egui::CollapsingHeader::new(name)
                .default_open(true)
                .show(ui, |ui| show_value(ui, value));
        }
        _ => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{name}:")).strong());
                show_scalar(ui, value);
            });
        }
    }
}

fn show_scalar(ui: &mut egui::Ui, value: &Value) {
    match value {
        Value::Null => {
            ui.colored_label(Color32::GRAY, "null");
        }
        Value::Bool(value) => {
            ui.label(value.to_string());
        }
        Value::Number(value) => {
            ui.monospace(value.to_string());
        }
        Value::String(value) => {
            ui.label(value);
        }
        Value::Array(_) | Value::Object(_) => {}
    }
}
