---
task: ProtoPositionControl
crate: Prototyping
kind: Task
subsystem: drive
input: "GamePadState"
output: "MotorCMD"
config:
  max_pos: "f32 rad (default 1.0)"
---

# ProtoPositionControl

Teleop position prototype: maps left stick Y to `MotorCMD::Position(max_pos * left_y, None, None)`.