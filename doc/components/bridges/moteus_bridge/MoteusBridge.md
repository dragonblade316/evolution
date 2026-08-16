---
task: MoteusBridge
crate: moteus_bridge
kind: Bridge
subsystem: IO
tx: "MotorCMD (per channel)"
rx: "MoteusData (per channel)"
config:
  can_id: "String (per channel, required)"
  kp: "f32 (optional, kp_scale)"
  kd: "f32 (optional, kd_scale)"
  max_vel: "f32 rev/s (optional, velocity_limit)"
  max_accel: "f32 rev/s² (optional, accel_limit)"
  max_torque: "f32 N·m (optional, maximum_torque)"
  position_min: "f32 rev (optional, clamp)"
  position_max: "f32 rev (optional, clamp)"
---

# MoteusBridge

Bidirectional copper bridge to moteus motors over the singleton CAN transport. `send` translates `MotorCMD` (Position/Velocity; SI rad → moteus rev) into a `PositionCommand`, applies per-TX-channel limits/gains, and writes it. `Stop` and `MotorCMD::Torque` are handled (torque currently errors). `receive` queries each RX motor and packs position/velocity/torque/temp/voltage/fault into `MoteusData`. `stop` issues `set_stop` to all TX motors.