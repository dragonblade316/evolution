---
task: MoteusTurretStateAssembler
crate: moteus_tasks
kind: Task
subsystem: turret
input: "(MoteusData flywheel, MoteusData turret)"
output: "TurretState"
config: {}
---

# MoteusTurretStateAssembler

Combines telemetry from the flywheel and turret Moteus motors into one `common::TurretState`.

| Input | Output field |
| --- | --- |
| Flywheel `MoteusData.data.vel` | `flywheel` |
| Turret `MoteusData.data.pos` | `position` |

Wire the flywheel and turret `moteus_bridge::messages::MoteusData` channels to the task in that order. It emits no payload until both channels provide telemetry.
