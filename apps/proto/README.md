# proto

Single-motor prototyping app for real hardware.

Graph:

`Joy::JoySource` → `Prototyping::ProtoVelocityControl` → `moteus` (CAN id 1)

- Left stick Y → velocity, peak **50 rpm**
- Motor gains: **kp = 1.2**, **kd = 0.0**

```sh
cargo run -p proto
```
