---
task: DiffDriveOmegaFusion
crate: diffdrive
module: odom
kind: Task
subsystem: localization
input: "(ChassisSpeeds, AngularVelocity IMU)"  # IMU optional
output: "ChassisSpeeds"
config:
  process_var: "f32 (default 1.0)"
  chassis_var: "f32 (default 0.05)"
  imu_var: "f32 (default 0.001)"
---

# DiffDriveOmegaFusion

1-state Kalman filter fusing chassis-derived yaw rate (required) with IMU yaw rate (optional). Constant-omega model (F=1); Q rescaled by `process_var*dt` each tick. Two sequential updates with separate measurement variances. Output = chassis x/y passthrough + fused theta.