use common::{MotorCMD, MotorData};
use cu29::units::si::angular_velocity::radian_per_second;
use cu29::units::si::f32::AngularVelocity;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Motor — constant acceleration toward a target speed
// ---------------------------------------------------------------------------

pub struct Motor {
    current_speed: f32, // rad/s
    target_speed: f32,  // rad/s
    acceleration: f32,  // rad/s² (always positive, direction from sign of diff)
}

impl Motor {
    pub fn new(acceleration: f32) -> Self {
        Self {
            current_speed: 0.0,
            target_speed: 0.0,
            acceleration,
        }
    }

    pub fn set_target(&mut self, speed: f32) {
        self.target_speed = speed;
    }

    pub fn speed(&self) -> f32 {
        self.current_speed
    }

    /// Step the motor toward target_speed at constant acceleration.
    /// Clamps exactly to target if the step would overshoot.
    pub fn update(&mut self, dt: f32) {
        let diff = self.target_speed - self.current_speed;
        if diff == 0.0 {
            return;
        }
        let max_step = self.acceleration * dt;
        if diff.abs() <= max_step {
            self.current_speed = self.target_speed;
        } else {
            self.current_speed += max_step * diff.signum();
        }
    }
}

// ---------------------------------------------------------------------------
// DiffDrive — 2-wheel differential drive kinematics
// ---------------------------------------------------------------------------

/// The true (noiseless) kinematic state of the diff-drive system.
#[derive(Debug, Clone)]
pub struct DiffDriveState {
    pub x: f32,
    pub y: f32,
    pub theta: f32,       // heading, radians
    pub left_speed: f32,  // rad/s
    pub right_speed: f32, // rad/s
}

pub struct DiffDrive {
    left_motor: Motor,
    right_motor: Motor,
    wheel_radius: f32,
    track_width: f32, // centre-to-centre distance between wheels
    /// Distance from axle to robot center. Positive = center ahead of axle.
    axle_offset: f32,
    x: f32,
    y: f32,
    theta: f32,
}

impl DiffDrive {
    pub fn new(
        wheel_radius: f32,
        track_width: f32,
        motor_acceleration: f32,
        axle_offset: f32,
    ) -> Self {
        Self {
            left_motor: Motor::new(motor_acceleration),
            right_motor: Motor::new(motor_acceleration),
            wheel_radius,
            track_width,
            axle_offset,
            x: 0.0,
            y: 0.0,
            theta: 0.0,
        }
    }

    pub fn set_wheel_targets(&mut self, left: f32, right: f32) {
        self.left_motor.set_target(left);
        self.right_motor.set_target(right);
    }

    pub fn set_wheel_target(&mut self, can_id: u8, speed: f32) {
        match can_id {
            1 => self.left_motor.set_target(speed),
            2 => self.right_motor.set_target(speed),
            _ => {}
        }
    }

    /// The exact pose and wheel speeds — internal ground truth.
    pub fn true_state(&self) -> DiffDriveState {
        DiffDriveState {
            x: self.x,
            y: self.y,
            theta: self.theta,
            left_speed: self.left_motor.speed(),
            right_speed: self.right_motor.speed(),
        }
    }

    /// State with simulated sensor noise applied.
    /// Currently returns the true state verbatim — noise model TBD.
    pub fn noisy_state(&self) -> DiffDriveState {
        self.true_state()
    }

    fn motor_data(speed: f32) -> MotorData {
            MotorData {
                vel: AngularVelocity::new::<radian_per_second>(speed),
                ..MotorData::default()
            }
        }

    /// Per-motor `MotorData` keyed by CAN ID (1 = left, 2 = right).
    pub fn motor_states(&self) -> HashMap<usize, MotorData> {
        let state = self.noisy_state();
        HashMap::from([
            (1, Self::motor_data(state.left_speed)),
            (2, Self::motor_data(state.right_speed)),
        ])
    }

    pub fn update(&mut self, dt: f32) {
        // 1. Advance motors
        self.left_motor.update(dt);
        self.right_motor.update(dt);

        // 2. Diff-drive kinematics at the axle.
        let vl = self.left_motor.speed() * self.wheel_radius;
        let vr = self.right_motor.speed() * self.wheel_radius;

        let v_axle = (vl + vr) / 2.0;
        let omega = (vr - vl) / self.track_width;

        // 3. Translate to robot center.  When the axle is offset from the
        //    center, a turning robot experiences a sideways velocity at the
        //    center: v_y = ω · axle_offset (robot frame).
        let v_x = v_axle * self.theta.cos() - omega * self.axle_offset * self.theta.sin();
        let v_y = v_axle * self.theta.sin() + omega * self.axle_offset * self.theta.cos();

        self.theta += omega * dt;
        self.x += v_x * dt;
        self.y += v_y * dt;
    }
}
