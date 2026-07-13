# sotm_rt

Shoot-on-the-move real-time targeting task. Given the robot's pose and velocity, computes the turret angle and muzzle velocity required to hit the goal.

## Config

```ron
// In copperconfig.ron, under the SOTM task node:
(
    goal_position: [4.0, 2.0, 2.5],   // [x, y, z] in world frame
    turret_offset: [0.1, 0.0, 0.5],   // [x, y, z] relative to robot center
)
```

Both keys are optional `[f64; 3]` arrays. Falls back to the defaults shown above.

## Input / Output

| Direction | Type | Description |
|---|---|---|
| Input | `(Pose<f64>, ChassisSpeeds)` | Robot pose and velocity |
| Output | `(f64, f64)` | `(turret_angle, muzzle_velocity)` — turret angle in radians, muzzle velocity in m/s |

## Pipeline

```
robot Pose + ChassisSpeeds
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
