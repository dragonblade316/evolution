---
task: SOTM
crate: sotm_rt
kind: Task
subsystem: turret
input: "(Pose<f64> robot, ChassisSpeeds)"
output: "(f64 turret_angle, f64 muzzle_velocity)"
config:
  goal_position: "[f64;3] (default [4.0, 2.0, 2.5])"
  turret_offset: "[f64;3] (default [0.1, 0.0, 0.5])"
---

# SOTM

Shooter On The Move solver. Computes turret yaw and muzzle velocity so a projectile (60° hood) lands on a moving target while accounting for turret offset and robot velocity. Pipeline: solve range/TOF via Gauss-Newton on an ODE IVP (`eqsolver` + `ivp`), compute lead angle, re-solve for muzzle velocity. Exposes `solve_muzzle_velocity` for the static `angleseek` analysis binary.