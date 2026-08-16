---
task: TurretVisualizer
crate: evoviz
kind: Sink
subsystem: IO
input: "Angle"
output: "n/a"
config:
  parent: "String (optional; e.g. \"robot\" to attach turret under the robot frame)"
  x: "f32 m (default 0.0)"
  y: "f32 m (default 0.0)"
  z: "f32 m (default 0.0)"
  length: "f32 m (default 0.3)"
  width: "f32 m (default 0.15)"
  height: "f32 m (default 0.1)"
---

# TurretVisualizer

General rerun visualizer that draws the turret as a 3D box rotated by the input `Angle`. Spawns a rerun viewer on `new`. When `parent` is set (e.g. `"robot"`), the turret logs at `{parent}/turret` so rerun parents it under the robot's transform — `x`/`y`/`z` then become the turret's offset from the robot center in the robot's local frame. Without `parent`, the turret logs standalone at `turret` and `x`/`y`/`z` are world coordinates.