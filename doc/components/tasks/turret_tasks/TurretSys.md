---
task: TurretSys
crate: turret_tasks
kind: Task
subsystem: turret
input: "(TurretState current, Angle target, Velocity muzzle_velocity)"
output: "TurretState"
config:
  flywheel_radius: "f32 m (default 0.05)"
---

# TurretSys

Produces a desired `common::TurretState` from the current turret state, a target angle, and SOTM's `MuzzleVelocity` (`Velocity`, in m/s). The target angle becomes the state position. The muzzle velocity is converted to flywheel angular velocity as `muzzle_velocity / flywheel_radius`.

The task emits no payload until all three inputs are present. `flywheel_radius` must be greater than zero.
