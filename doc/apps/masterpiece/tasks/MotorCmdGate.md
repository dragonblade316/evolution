---
task: MotorCmdGate
crate: masterpiece
kind: Task
subsystem: drive
input: "(MotorCMD, SuperState)"
output: "MotorCMD"
config: {}
---

# MotorCmdGate

Passes the motor command through when `SuperState` is not `Disabled`. Emits `MotorCMD::Stop` when the robot is disabled or either input is unavailable.
