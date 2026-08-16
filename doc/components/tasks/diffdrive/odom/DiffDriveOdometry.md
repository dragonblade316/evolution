---
task: DiffDriveOdometry
crate: diffdrive
module: odom
kind: Task
subsystem: localization
input: "DiffDriveSpeeds"
output: "ChassisSpeeds"
config:
  wheel_radius: "f32 m (default 0.1)"  # unused; speeds already linear
  trackwidth: "f32 m (default 0.3)"
  axle_offset: "f32 m (default 0.0)"
---

# DiffDriveOdometry

Forward diff-drive kinematics → chassis velocity at a control point. `v = (left+right)/2`, `omega = (right-left)/trackwidth`. When the control point is offset from the axle, a sideways component `vy = omega * axle_offset` is added so the frame is the robot center, not the axle.