---
task: AllianceTransformSelector
crate: allience_spesific_tasks
kind: Task
subsystem: field
input: "DSStatus"
output: "Transform3D<f64>"
config:
  red_transform: "required 4x4 f64 transform matrix"
  blue_transform: "required 4x4 f64 transform matrix"
---

# AllianceTransformSelector

Selects the configured field transform for the alliance in the incoming driver-station status. Both `ENABLED` and `DISABLED` statuses emit their alliance's transform; `DISCONNECTED` emits no payload.

Configure the node with two matrices accepted by `Transform3D::from_matrix`:

```ron
(
    id: "alliance_transform",
    type: "allience_spesific_tasks::AllianceTransformSelector",
    config: {
        "red_transform": [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        "blue_transform": [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    },
)
```

Wire `common::DSStatus` into the task and consume its `cu_spatial_payloads::Transform3D<f64>` output wherever an alliance-specific field target is required.
