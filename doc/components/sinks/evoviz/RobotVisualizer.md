---
task: RobotVisualizer
crate: evoviz
kind: Sink
subsystem: IO
input: "Pose<f64>"
output: "n/a"
config:
  length: "f32 m (default 0.5)"
  width: "f32 m (default 0.5)"
  height: "f32 m (default 0.2)"
---

# RobotVisualizer

General rerun visualizer that draws the robot as a 3D box. Spawns a rerun viewer on `new`. Each cycle logs the input `Pose<f64>` as a transform at `robot`, and a `Boxes3D` (half-extents = length/width/height ÷ 2) at `robot/box`. Yaw is extracted from the pose's rotation matrix. Configure the box size via `length`/`width`/`height` (meters).