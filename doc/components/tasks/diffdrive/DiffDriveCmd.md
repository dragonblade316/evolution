---
task: DiffDriveCmd
crate: diffdrive
kind: Task
subsystem: drive
input: "DiffDriveSpeeds"
output: "(LeftMotorCMD, RightMotorCMD)"
config:
  wheel_radius: "f32 m (default 0.1)"
---

# DiffDriveCmd

Turns diff-drive wheel linear velocities into per-motor velocity commands. Each wheel speed is divided by `wheel_radius` to get angular velocity, then emitted as `MotorCMD::Velocity(omega, None)`.