---
task: SOTM
crate: sotm_rt
kind: Task
subsystem: turret
input: "(Pose<f64> robot, Transform3D<f64> target, ChassisSpeeds)"
output: "(f64 turret_angle, f64 muzzle_velocity)"
config:
  turret_offset: "[f64;3] (default [0.1, 0.0, 0.5])"
---

# SOTM

Shooter On The Move solver. Computes turret yaw and muzzle velocity so a projectile (60° hood) lands on a moving target while accounting for turret offset and robot velocity. The target is supplied at runtime as a world-frame `Transform3D<f64>`; its translation is used as the target point and its rotation is ignored. Pipeline: solve range/TOF via Gauss-Newton on an ODE IVP (`eqsolver` + `ivp`), compute lead angle, re-solve for muzzle velocity. Exposes `solve_muzzle_velocity` for the static `angleseek` analysis binary.
