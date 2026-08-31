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

## Follow-up

When enabling Copper/Bevy reflection for `sim-debug`, keep the Sophus, nalgebra, and `Instant` runtime state opaque. Later, add a small reflectable debug-state struct containing only stable scalar/array values (for example, pose `x`, `y`, heading, and covariance entries), and expose that projection through the task's debug-state hooks.
