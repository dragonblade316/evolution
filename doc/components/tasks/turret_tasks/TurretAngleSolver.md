---
task: TurretAngleSolver
crate: turret_tasks
kind: Task
subsystem: turret
input: "(Angle turret_current, Angle target)"
output: "Angle"
config:
  min_deg: "f32 (default -180)"
  max_deg: "f32 (default 180)"
---

# TurretAngleSolver

Wraps a target turret angle into the configured min/max travel range. Normalizes the target into 0..360°, considers the -360° alias, and picks whichever (target or alias) is both valid and closest to the current angle. Emits nothing if neither is reachable.