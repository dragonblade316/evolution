---
task: JoySource
crate: Joy
kind: Source
subsystem: IO
input: "none"
output: "GamePadState"
config:
  deadband: "f32 (default 0.08, clamped ≤0.95)"
---

# JoySource

Reads an Xbox controller through `gilrs` and emits a `GamePadState` every cycle. Axes are deadbanded and rescaled so full stick throw reaches ±1; analog triggers are remapped to 0..1. Non-Xbox pads are ignored; if none is connected a zeroed state is emitted. Re-acquires the pad if it drops.