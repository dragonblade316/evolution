---
task: FieldOrientedDiffDrive
crate: field_oriented_diff_drive
kind: Task
subsystem: drive
input: "(ChassisSpeeds desired, ChassisSpeeds current)"
output: "ChassisSpeeds"
config:
  kp: "f32 (default 2.0)"
  ki: "f32 (default 0.1)"
  kd: "f32 (default 0.0)"
  output_limit: "f32 rad/s (default 5.0)"
  turn_slowdown: "f32 (default 0.5)"
---

# FieldOrientedDiffDrive

Field-oriented diff-drive controller. Computes heading error between desired and current global-frame velocity vectors (shortest path, wrapped), drives a PID to zero the error, and outputs robot-relative (forward, 0, theta_cmd). Forward speed is reduced as `|theta_cmd|` grows via `speed / (1 + turn_slowdown*|theta|)`.