// How you can tell that I use AI in a repo is that the AI actually writes tests.

use super::{
    apply_ui_command, decode_telemetry, joy::apply_gamepad_deadband, joy::is_xbox_name,
    robot_status_from_matching_subscriber, DriverStationSnapshot, RequestedRobotState,
    RobotStatus, UiCommand,
};
use crate::cu29::payloads::DsTxMsg;

#[test]
fn decodes_valid_telemetry_json() {
    let message = decode_telemetry(
        br#"{"pose":[[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]]}"#,
    )
    .expect("telemetry should decode");

    assert!(message.get("pose").is_some());
}

#[test]
fn rejects_malformed_telemetry_json() {
    assert!(decode_telemetry(b"not json").is_err());
}

#[test]
fn accepts_any_json_schema() {
    assert!(decode_telemetry(br#"{"unexpected":42}"#).is_ok());
}

#[test]
fn recognizes_xbox_controller_names() {
    assert!(is_xbox_name("Xbox Wireless Controller"));
    assert!(is_xbox_name("X-Box 360 pad"));
    assert!(is_xbox_name("XInput Controller"));
    assert!(!is_xbox_name("Sony DualSense Wireless Controller"));
}

#[test]
fn applies_gamepad_deadband_and_rescales_axes() {
    assert_eq!(apply_gamepad_deadband(0.05), 0.0);
    assert_eq!(apply_gamepad_deadband(-0.05), 0.0);
    assert_eq!(apply_gamepad_deadband(1.0), 1.0);
    assert_eq!(apply_gamepad_deadband(-1.0), -1.0);
}

#[test]
fn applies_enable_and_disable_commands() {
    let mut ds_state = DsTxMsg::default();
    let mut status = DriverStationSnapshot::default();
    status.robot = RobotStatus::Connected;

    apply_ui_command(UiCommand::Enable, &mut ds_state, &mut status);
    assert!(ds_state.enabled);
    assert_eq!(status.requested_state, RequestedRobotState::Enabled);

    apply_ui_command(UiCommand::Disable, &mut ds_state, &mut status);
    assert!(!ds_state.enabled);
    assert_eq!(status.requested_state, RequestedRobotState::Disabled);
}

#[test]
fn emergency_stop_is_latched() {
    let mut ds_state = DsTxMsg::default();
    let mut status = DriverStationSnapshot::default();
    status.robot = RobotStatus::Connected;

    apply_ui_command(UiCommand::Enable, &mut ds_state, &mut status);
    apply_ui_command(UiCommand::ESTOP, &mut ds_state, &mut status);
    apply_ui_command(UiCommand::Disable, &mut ds_state, &mut status);
    apply_ui_command(UiCommand::Enable, &mut ds_state, &mut status);

    assert!(ds_state.estop);
    assert!(!ds_state.enabled);
    assert_eq!(status.requested_state, RequestedRobotState::EStopped);
}

#[test]
fn enable_requires_robot_communications() {
    let mut ds_state = DsTxMsg::default();
    let mut status = DriverStationSnapshot::default();

    apply_ui_command(UiCommand::Enable, &mut ds_state, &mut status);

    assert!(!ds_state.enabled);
    assert_eq!(status.requested_state, RequestedRobotState::Disabled);
}

#[test]
fn matching_command_subscriber_reports_robot_connection() {
    assert_eq!(
        robot_status_from_matching_subscriber(false),
        RobotStatus::Disconnected
    );
    assert_eq!(
        robot_status_from_matching_subscriber(true),
        RobotStatus::Connected
    );
}
