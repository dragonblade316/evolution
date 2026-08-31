---
task: MoteusDataSplitter
crate: moteus_tasks
kind: Task
subsystem: moteus
input: "MoteusData"
output: "(MoteusCanId, MoteusPosition, MoteusVelocity, MoteusAcceleration, MoteusTorque, MoteusTemperature, MoteusVoltage, MoteusFault)"
config: {}
---

# MoteusDataSplitter

Splits one `moteus_bridge::messages::MoteusData` payload into independently
wireable Copper messages without changing units or values.

| Output order | Output type | Source field |
| --- | --- | --- |
| 1 | `MoteusCanId` (`u8`) | `canid` |
| 2 | `MoteusPosition` (`Angle`) | `data.pos` |
| 3 | `MoteusVelocity` (`AngularVelocity`) | `data.vel` |
| 4 | `MoteusAcceleration` (`Option<AngularAcceleration>`) | `data.accel` |
| 5 | `MoteusTorque` (`Option<Torque>`) | `data.torque` |
| 6 | `MoteusTemperature` (`f32`) | `temp` |
| 7 | `MoteusVoltage` (`f32`) | `voltage` |
| 8 | `MoteusFault` (`i8`) | `fault` |

The public aliases distinguish outputs with identical underlying Rust types in
Copper configuration while remaining compatible with tasks that consume those
underlying types. A missing input produces no outputs. A present message emits
all outputs, including explicit `None` values for unavailable acceleration or
torque.

Copper requires every declared output to appear in the graph. Connect any
unused output to `dst: "__nc__"`, preserving the declared output order in the
connection list.
