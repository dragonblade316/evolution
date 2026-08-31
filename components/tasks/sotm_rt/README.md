# sotm_rt

Shoot-on-the-move real-time targeting task. Given the robot's pose, a runtime target transform, and velocity, computes the turret angle and muzzle velocity required to hit the target.

## Config

```ron
// In copperconfig.ron, under the SOTM task node:
(
    turret_offset: [0.1, 0.0, 0.5],   // [x, y, z] relative to robot center
)
```

`turret_offset` is optional and defaults to `[0.1, 0.0, 0.5]`. The target is
provided at runtime as a `Transform3D<f64>` input, so no target position is
configured on the SOTM task.

## Input / Output

| Direction | Type | Description |
|---|---|---|
| Input | `(Pose<f64>, Transform3D<f64>, ChassisSpeeds)` | Robot pose, world-frame target transform, and velocity |
| Output | `(f64, f64)` | `(turret_angle, muzzle_velocity)` — turret angle in radians, muzzle velocity in m/s |

## Pipeline

```
robot Pose + target Transform3D + ChassisSpeeds
  → target translation becomes the world-frame goal position
  → turret world position & velocity (offset rotated by heading)
  → decompose into parallel/perpendicular axes vs goal
  → solve(dist_to_goal, dz, v_∥, v_⊥, heading, goal_direction)
       → solve_range → [muzzle_velocity_initial, tof]
       → solve_angle → (θ_lateral, r)
       → solve_range(r, dz) → [muzzle_velocity_final, _]
       → turret_angle = (goal_direction − heading) + θ_lateral
       → return (turret_angle, muzzle_velocity)
  → output: (turret_angle, muzzle_velocity)
```

### Key variables

| Variable | Meaning |
|---|---|
| `dist_to_goal` | Ground-plane distance from turret to goal |
| `dz` | Vertical difference (goal_z − turret_z) |
| `v_parallel` | Turret velocity along the toward-goal axis (positive = closing) |
| `v_perpendicular` | Turret velocity across the goal axis (lateral) |
| `heading` | Robot yaw in world frame |
| `goal_direction` | Angle from turret to goal in world frame |
| `angle_front_to_goal` | `goal_direction − heading` — base aim angle |
| `θ_lateral` | Lateral correction from `solve_angle` |
| `turret_angle` | `angle_front_to_goal + θ_lateral` — final turret aim |
| `muzzle_velocity` | Launch velocity from the second `solve_range` pass |

The target transform's translation supplies `[x, y, z]` in the world frame.
Its rotation is ignored because SOTM aims at the target point rather than an
orientation-dependent target surface.
