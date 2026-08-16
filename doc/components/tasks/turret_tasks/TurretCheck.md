---
task: TurretCheck
crate: turret_tasks
kind: Task
subsystem: turret
input: "(TurretState current, TurretState desired)"
output: "bool"
config:
  angle_tol: "f32 deg (default 2.0)"
  velocity_tol: "f32 rpm (default 10.0)"
---

# TurretCheck

Tolerance gate comparing current turret state to desired. Outputs `true` when both the position error (deg) and flywheel-velocity error (rad/s) are within their respective tolerances; `false` otherwise. No-op if either payload is missing.