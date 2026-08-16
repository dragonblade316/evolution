---
task: IEKFPoseEstimator
crate: iekflocalizer
kind: Task
subsystem: localization
input: "(ChassisSpeeds, AprilTagDetections)"  # detections optional
output: "(Pose<f64>, ChassisSpeeds field-frame)"
config: {}
---

# IEKFPoseEstimator

Iterated Extended Kalman Filter on SE(2) estimating robot pose. Predicts from chassis twist; on each AprilTag detection (filtered by decision margin, best tag taken) it rolls the state buffer back, interpolates pose+covariance to the measurement time, applies the update, and replays forward. Output 0 is the pose; output 1 is chassis speeds rotated into the field frame using the estimated heading.