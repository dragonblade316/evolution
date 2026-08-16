---
task: TurretCMD
crate: masterpiece
kind: Task
subsystem: turret
input: "(SuperState, TurretState)"
output: "(MotorCMD flywheel, MotorCMD turret)"
config:
  max_turret_velocity: "optional f32 RPM"
  idle_flywheel_speed: "f32 RPM (default 0.0)"
---

# TurretCMD

Translates the high-level robot state and desired `TurretState` into flywheel and turret `MotorCMD` messages.

`max_turret_velocity` and `idle_flywheel_speed` are loaded and stored by the task for later use. They do not currently change the commands emitted by the task.
