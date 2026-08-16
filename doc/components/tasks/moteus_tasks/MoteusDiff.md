---
task: MoteusDiff
crate: moteus_tasks
kind: Task
subsystem: drive
input: "(LeftMoteusData, RightMoteusData)"
output: "DiffDriveSpeeds"
config:
  wheel_radius: "f32 m (default 0.1)"
---

# MoteusDiff

Aggregates per-motor moteus telemetry into diff-drive wheel speeds. Each motor's angular velocity (rad/s) is multiplied by `wheel_radius` to produce linear wheel velocity (m/s). No-op if either payload is missing.