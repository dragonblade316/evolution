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

Bidirectional copper bridge to moteus motors over the singleton CAN transport. Each Copper loop batches all RX queries in `preprocess` and caches their responses for `receive`. `send` translates `MotorCMD` (Position/Velocity; SI rad → moteus rev), applies per-TX-channel limits/gains, and queues it for one batched transport cycle in `postprocess`. `Stop` is queued with the other commands and `MotorCMD::Torque` currently errors. `receive` packs cached position/velocity/torque/temp/voltage/fault/mode into `MoteusData`; shutdown sends one batch of stop commands to all TX motors.