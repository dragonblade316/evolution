---
task: DiffDriveKinematics
crate: accel_limiter
kind: Task
subsystem: drive
input: "(ChassisSpeeds cmd, ChassisSpeeds current)"
output: "ChassisSpeeds"
config:
  accel_limit: "f32 m/s² (default 0.5)"
---

# accel_limiter / DiffDriveKinematics

Rate-limits a desired chassis command against the current measured chassis velocity. Each axis (x, y, theta) is clamped so its change per tick never exceeds `accel_limit * dt`. First tick passes the command through (no prior time). If either payload is missing, emits nothing.