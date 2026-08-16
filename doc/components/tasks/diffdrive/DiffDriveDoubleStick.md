---
task: DiffDriveDoubleStick
crate: diffdrive
kind: Task
subsystem: drive
input: "GamePadState"
output: "DiffDriveSpeeds"
config:
  max_wheel_vel: "f32 (default 1.0)"
---

# DiffDriveDoubleStick

Direct tank-style teleop mapping: left stick Y → left wheel, right stick Y → right wheel, each scaled by `max_wheel_vel` (m/s).