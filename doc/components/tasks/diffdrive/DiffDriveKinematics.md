---
task: DiffDriveKinematics
crate: diffdrive
kind: Task
subsystem: drive
input: "ChassisSpeeds"
output: "DiffDriveSpeeds"
config:
  trackwidth: "f32 m (default 0.3)"
---

# diffdrive / DiffDriveKinematics

Inverse diff-drive kinematics: converts a robot-relative `ChassisSpeeds` (vx, omega) into left/right wheel linear velocities using `trackwidth`. `left = vx - omega*trackwidth/2`, `right = vx + omega*trackwidth/2`.