---
task: BinaryServo
crate: rpsinks
kind: Sink
subsystem: IO
input: "bool"
output: "n/a"
config:
  frequency_hz: "f64 (default 50.0)"
  deployed_us: "f64 (default 1500.0)"
  undeployed_us: "f64 (default 1000.0)"
  channel: "u8 (default 0)"
---

# BinaryServo

Two-position PWM servo sink (rppal). On each `bool` payload it sets the pulse width to `deployed_us` (true) or `undeployed_us` (false). Disables PWM on `stop`.