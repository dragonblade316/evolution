---
task: ProtoVelocityControl
crate: Prototyping
kind: Task
subsystem: drive
input: "GamePadState"
output: "MotorCMD"
config:
  max_vel: "f32 rpm (default 10.0)"
---

# ProtoVelocityControl

Teleop velocity prototype: maps left stick Y to `MotorCMD::Velocity(max_vel * left_y, None)`.