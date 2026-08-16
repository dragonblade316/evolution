---
task: MuzzleVelocityTurretStateAssembler
crate: turret_tasks
kind: Task
subsystem: turret
input: "(Velocity muzzle_velocity, Angle turret_position)"
output: "TurretState"
config:
  flywheel_radius: "f32 m (default 0.05)"
---

# MuzzleVelocityTurretStateAssembler

Combines a SOTM-compatible muzzle `Velocity` (m/s) and turret position into `common::TurretState`. It converts muzzle velocity to the flywheel angular-speed setpoint using `muzzle_velocity / flywheel_radius`.

The task emits no payload until both inputs are present. `flywheel_radius` must be greater than zero.
