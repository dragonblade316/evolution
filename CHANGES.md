# Changes Made During This Session

## Overview

This document summarizes all code changes across the `evosim` crate, the
`masterpiece` sim binary, and the `iekflocalizer`, `diffdrive`,
`diffdrive_odometry`, and `moteus_tasks` components.

---

## 1. evosim: speed storage converted to cu29 unit types

### Files
- `external/evosim/src/lib.rs`

### Summary

Replaced all bare `f32` speed and position fields with **cu29 unit types**:

- `Motor::current_speed` / `target_speed`: `f32` -> `AngularVelocity`
- `Motor::current_pos`: `Angle` (new field, see below)
- `Motor::acceleration`: `f32` -> `AngularAcceleration`
- `DiffDriveState::left_speed` / `right_speed`: `f32` -> `AngularVelocity`
- `DiffDriveState::left_pos` / `right_pos`: `Angle` (added)
- `DiffDriveState::x` / `y`: `f32` -> `Length`
- `DiffDriveState::theta`: `f32` -> `Angle`
- `DiffDrive` internal fields (`wheel_radius`, `track_width`, `axle_offset`,
  `x`, `y`, `theta`, `accel`): all converted to unit types (`Length`, `Angle`,
  `AngularAcceleration`).

All public API methods (`Motor::new`, `Motor::set_target`, `Motor::speed`,
`Motor::pos`, `DiffDrive::new`, `DiffDrive::set_wheel_targets`,
`DiffDrive::set_left_motor`, `DiffDrive::set_right_motor`,
`DiffDrive::motor_data`, etc.) now accept and return cu29 unit types.

All value extraction uses `.get::<unit>()` (never `.raw()`).

### Motor struct

The `Motor` was extended to track position in addition to velocity:

```rust
pub struct Motor {
    current_pos: Angle,
    target_pos: Angle,
    current_speed: AngularVelocity,
    target_speed: AngularVelocity,
    acceleration: AngularAcceleration,
    pos_ctl: bool,
}
```

`Motor::update()` now advances `current_pos` by `speed * dt` each tick and,
when `pos_ctl` is set, drives `current_pos` toward `target_pos` at constant
velocity instead of ramping speed.

### DiffDrive::update forward-Euler fix

Previously, `DiffDrive::update` advanced the motors first and then read the
**post-ramp** (end-of-step) wheel speeds for the elapsed step's kinematic
integration, which over-reports distance during transients. Changed to apply
the kinematic integration using the **start-of-step** wheel speeds (forward
Euler), consistent with how the IEKF integrates the same twisted.

---

## 2. masterpiece: sim.rs updated

### File
- `apps/masterpiece/src/sim.rs`

### Summary

Updated the sim binary to construct unit-typed values for `DiffDrive::new`
(`Length::new::<meter>()`, `AngularAcceleration::new::<…>()`) and to extract
values via `.get::<>()` for rerun logging and turret math.

Wheel radius / track width defaults set to **0.1 m / 0.3 m**.

---

## 3. Wheel radius / track width defaults normalized

### Files
- `apps/masterpiece/src/sim.rs`
- `components/tasks/diffdrive/src/lib.rs`
- `components/tasks/diffdrive_odometry/src/lib.rs`
- `components/tasks/moteus_tasks/src/lib.rs`

### Summary

The defaults were scattered (0.02 m / 0.05 m / 0.4 m depending on the
component) and inconsistent between the sim and the odometry/inversion tasks.
--

| Component             | Before (r / W) | After (r / W)   |
|-----------------------|----------------|----------------|
| sim (`evosim::DiffDrive`) | 0.05 / 0.4   | 0.1 / 0.3       |
| `diffdrive::DiffDriveDoubleStick` | 0.02 / 0.02 | 0.1 / 0.3 |
| `diffdrive::DiffDriveCmd` | 0.02 / 0.02 | 0.1 / 0.3      |
| `diffdrive_odometry`  | 0.02 / 0.02    | 0.1 / 0.3       |
| `moteus_tasks::MoteusDiff` | 0.05 / -   | 0.1 / -         |

Also fixed a **copy-paste bug** in `diffdrive/src/lib.rs` where the trackwidth
fallback used `DEFAULT_WHEEL_RADIUS_METERS` instead of `DEFAULT_TRACKWIDTH`.

(Rust config `apps/masterpiece/copperconfig.ron` does not currently override
these values, so the task code defaults take effect.)

---

## 4. diffdrive_odometry: double-conversion bug fix

### File
- `components/tasks/diffdrive_odometry/src/lib.rs`

### Summary

`DiffDriveOdometry::process` was multiplying its `DiffDriveSpeeds` input by
`wheel_radius * PI`. But `DiffDriveSpeeds` (produced by
`moteus_tasks::MoteusDiff`) is already in **m/s** — the motor's rad/s velocity
has already been converted to linear wheel velocity there.

So the odometry was double-converting: with r=0.1 that meant the output was
off by a factor of `0.1 * pi ~= 0.314`, explaining the IEKF appearing to
underestimate motion by ~3.18x.

**Before:**

```rust
let v_axle = (speeds.left.raw() + speeds.right.raw()) / 2.0
    * self.wheel_radius.raw()
    * std::f32::consts::PI;
let omega = (speeds.right.raw() - speeds.left.raw())
    * self.wheel_radius.raw()
    * std::f32::consts::PI
    / self.trackwidth.raw();
```

**After:**

```rust
// speeds.left/right are already linear wheel velocities (m/s).
let v_axle = (speeds.left.raw() + speeds.right.raw()) / 2.0;
let omega = (speeds.right.raw() - speeds.left.raw()) / self.trackwidth.raw();
```

`wheel_radius` is now unused in this struct (warning kept).

Also added a `println!` at the end of `DiffDriveOdometry::process` logging
the computed `x`/`y`/`theta` chassis speeds.

---

## 5. iekflocalizer: SE(2) twist ordering bug fix (root cause)

### File
- `components/tasks/iekflocalizer/src/lib.rs`

This was the most consequential fix.

### Root cause

The IEKF was assembling the SE(2) tangent (twist) vector as
`Vector3::new(x, y, theta)` — i.e. `(vx, vy, omega)`. But sophrinos's
`Isometry2F64::exp` expects the tangent vector in the order
**(theta, x, y)**, i.e. `(omega, vx, vy)` — rotation first.

See `sophus_lie-0.15.0/src/groups/isometry2.rs:70`:
```
exp(theta, nu) = ( exp_so2(theta),  V(theta) * nu )
```

With the old ordering, forward velocity was being fed into the **rotation**
channel: a 1 m/s forward command drove the sim to x=5.46 m but the IEKF to
**x=0** with `theta` winding up at -0.82 rad (≈ 5.5 mod 2pi — i.e. the 1 m/s
was being read as 1 rad/s of rotation).

### Fix

Swapped the twist assembly in both `TimeMachineIEKF::update` and
`TimeMachineIEKF::update_vision`:

**Before (both sites):**
```rust
let twist = Vector3::new(
    speeds.x.get::<meter_per_second>() as f64,
    speeds.y.get::<meter_per_second>() as f64,
    speeds.theta.get::<radian_per_second>() as f64,
);
```

**After:**
```rust
// sophus SE(2) tangent order is (theta, x, y) — rotation first.
let twist = Vector3::new(
    speeds.theta.get::<radian_per_second>() as f64,
    speeds.x.get::<meter_per_second>() as f64,
    speeds.y.get::<meter_per_second>() as f64,
);
```

---

## 6. iekflocalizer: known outstanding issues

### 6a. Predict covariance: multiply should be addition

`IEKF::predict` (line ~30) currently:

```rust
self.P = ad * self.P * ad.transpose() * self.Q * dt;
```

It **multiplies** by `Q * dt` instead of **adding** it. The IEKF prediction
should be:

```rust
self.P = ad * self.P * ad.transpose() + self.Q * dt;
```

This has been flagged with a `TODO` comment on the line. With the bug in
place, the covariance shrinks toward zero every step and process noise never
accumulates — measurement updates will eventually be ignored. Not yet
fixed; affects vision updates only, not dead-reckoning.

### 6b. Buffer underflow

`TimeMachineIEKF::update` (line ~75):

```rust
for i in 0..self.buf.len() - 1 {
```

This underflows to a huge range when `self.buf` is empty. Should be
`0..self.buf.len().saturating_sub(1)` or guarded with an `is_empty()` check.

### 6c. self.last timestamp tracking

`self.last` was set once in `new()` and never reassigned in `update()`, so
`dt` grew unboundedly. This was fixed by the user out-of-band; current code
correctly sets `self.last = timestamp;` at the end of `update()`.

---

## 7. diffdrive_odometry: debug print

Added at the end of `DiffDriveOdometry::process`:
```rust
println!("DiffDriveOdometry: x={:?}, y={:?}, theta={:?}", x, y, theta);
```

---

## Summary of bugs fixed

| Bug | Severity | Location |
|-----|----------|----------|
| IEKF SE(2) twist axis swap (forward vel became rotation) | Critical | `iekflocalizer/src/lib.rs` |
| Odometry double-conversion (× r × PI on already-linear input) | High | `diffdrive_odometry/src/lib.rs` |
| IEKF `self.last` never updated (dt grew without bound) | High | `iekflocalizer/src/lib.rs` (user) |
| Inconsistent wheel_radius / trackwidth defaults across components | Medium | multiple |
| Copy-paste: trackwidth fallback used wheel_radius constant | Low | `diffdrive/src/lib.rs` |
| Sim used end-of-step speed for elapsed step (over-reports transients) | Low | `evosim/src/lib.rs` |

## Outstanding (not fixed)

| Issue | Notes |
|-------|-------|
| IEKF predict covariance: `*` should be `+` | TODO comment added; affects vision updates |
| IEKF buffer prune loop underflow when empty | Minor; low-frequency path |