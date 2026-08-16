---
task: TurretStateAssembler
crate: turret_tasks
kind: Task
subsystem: turret
input: "(AngularVelocity flywheel_speed, Angle turret_position)"
output: "TurretState"
config: {}
---

# TurretStateAssembler

Combines separate flywheel-speed and turret-position messages into a single `common::TurretState`. No-op if any payload is missing.
