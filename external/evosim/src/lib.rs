use common::MotorData;
use cu29::units::si::angular_acceleration::radian_per_second_squared;
use cu29::units::si::angular_velocity::radian_per_second;
use cu29::units::si::angle::radian;
use cu29::units::si::f32::{Angle, AngularAcceleration, AngularVelocity, Length, Velocity};
use cu29::units::si::length::meter;
use cu29::units::si::velocity::meter_per_second;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Motor — constant acceleration toward a target speed
// ---------------------------------------------------------------------------

pub struct Motor {
    current_pos: Angle,                  // rad
    target_pos: Angle,
    current_speed: AngularVelocity,      // rad/s
    target_speed: AngularVelocity,        // rad/s
    acceleration: AngularAcceleration,    // rad/s² (always positive, direction from sign of diff)
    pos_ctl: bool
}

impl Motor {
    pub fn new() -> Self {
        Self {
            current_pos: Angle::new::<radian>(0.0),
            target_pos: Angle::new::<radian>(0.0),
            current_speed: AngularVelocity::new::<radian_per_second>(0.0),
            target_speed: AngularVelocity::new::<radian_per_second>(0.0),
            acceleration: AngularAcceleration::new::<radian_per_second_squared>(0.0),
            pos_ctl: false
        }
    }


    pub fn set_pos_target(&mut self, pos: Angle, vel: AngularVelocity) {
           self.target_pos = pos;
           self.current_speed = vel;
    }

    pub fn set_vel_target(&mut self, speed: AngularVelocity, accel: AngularAcceleration) {
        self.target_speed = speed;
        self.acceleration = accel;
    }

    pub fn speed(&self) -> AngularVelocity {
        self.current_speed
    }

    pub fn pos(&self) -> Angle {
        self.current_pos
    }

    /// Step the motor toward target_speed at constant acceleration.
    /// Clamps exactly to target if the step would overshoot.
    pub fn update(&mut self, dt: f32) {
        if self.pos_ctl {
            let diff = self.target_pos.get::<radian>() - self.current_pos.get::<radian>();
            let max_step = self.current_speed.get::<radian_per_second>() * dt;
            if diff.abs() <= max_step {
                self.current_pos = self.target_pos;
            } else {
                self.current_pos = Angle::new::<radian>(
                    self.current_pos.get::<radian>() + max_step * diff.signum(),
                );
            }
        } else {
            let diff = self.target_speed.get::<radian_per_second>() - self.current_speed.get::<radian_per_second>();
            let max_step = self.acceleration.get::<radian_per_second_squared>() * dt;
            // Integrate wheel position with the start-of-step speed (forward Euler)
            // so the chassis dead-reckoning matches the IEKF's model.
            let step_pos = self.current_speed.get::<radian_per_second>() * dt;
            if diff.abs() <= max_step {
                self.current_speed = self.target_speed;
            } else {
                self.current_speed = AngularVelocity::new::<radian_per_second>(
                    self.current_speed.get::<radian_per_second>() + max_step * diff.signum(),
                );
            }
            self.current_pos = Angle::new::<radian>(
                self.current_pos.get::<radian>() + step_pos,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// DiffDrive — 2-wheel differential drive kinematics
// ---------------------------------------------------------------------------

/// The true (noiseless) kinematic state of the diff-drive system.
#[derive(Debug, Clone)]
pub struct DiffDriveState {
    pub x: Length,
    pub y: Length,
    pub theta: Angle,                  // heading, radians
    pub left_pos: Angle,               // rad
    pub right_pos: Angle,              // rad
    pub left_speed: AngularVelocity,  // rad/s
    pub right_speed: AngularVelocity, // rad/s
}

pub struct DiffDrive {
    left_motor: Motor,
    right_motor: Motor,
    wheel_radius: Length,
    track_width: Length, // centre-to-centre distance between wheels
    /// Distance from axle to robot center. Positive = center ahead of axle.
    axle_offset: Length,
    accel: AngularAcceleration,
    x: Length,
    y: Length,
    theta: Angle,
}

impl DiffDrive {
    pub fn new(
        wheel_radius: Length,
        track_width: Length,
        motor_acceleration: AngularAcceleration,
        axle_offset: Length,
    ) -> Self {
        Self {
            left_motor: Motor::new(),
            right_motor: Motor::new(),
            wheel_radius,
            track_width,
            axle_offset,
            accel: motor_acceleration,
            x: Length::new::<meter>(0.0),
            y: Length::new::<meter>(0.0),
            theta: Angle::new::<radian>(0.0),
        }
    }

    pub fn set_wheel_targets(&mut self, left: AngularVelocity, right: AngularVelocity) {
        self.set_left_motor(left);
        self.set_right_motor(right);
    }

    pub fn set_left_motor(&mut self, speed: AngularVelocity) {
        self.left_motor.set_vel_target(speed, self.accel);
    }

    pub fn set_right_motor(&mut self, speed: AngularVelocity) {
        self.right_motor.set_vel_target(speed, self.accel);
    }

    /// The exact pose and wheel speeds — internal ground truth.
    pub fn true_state(&self) -> DiffDriveState {
        DiffDriveState {
            x: self.x,
            y: self.y,
            theta: self.theta,
            left_pos: self.left_motor.pos(),
            right_pos: self.right_motor.pos(),
            left_speed: self.left_motor.speed(),
            right_speed: self.right_motor.speed(),
        }
    }

    /// State with simulated sensor noise applied.
    /// Currently returns the true state verbatim — noise model TBD.
    pub fn noisy_state(&self) -> DiffDriveState {
        self.true_state()
    }

    fn motor_data(pos: Angle, speed: AngularVelocity) -> MotorData {
            MotorData {
                pos,
                vel: speed,
                ..MotorData::default()
            }
        }

    /// Per-motor `MotorData` keyed by CAN ID (1 = left, 2 = right).
    pub fn motor_states(&self) -> HashMap<usize, MotorData> {
        let state = self.noisy_state();
        HashMap::from([
            (1, Self::motor_data(self.left_motor.pos(), state.left_speed)),
            (2, Self::motor_data(self.right_motor.pos(), state.right_speed)),
        ])
    }

    pub fn update(&mut self, dt: f32) {
        // 1. Capture the start-of-step wheel speeds (forward Euler): these are
        //    the speeds that were actually in effect over the elapsed dt.
        let vl = self.left_motor.speed().get::<radian_per_second>() * self.wheel_radius.get::<meter>();
        let vr = self.right_motor.speed().get::<radian_per_second>() * self.wheel_radius.get::<meter>();

        // 2. Advance motors (ramps speed toward target for the *next* step).
        self.left_motor.update(dt);
        self.right_motor.update(dt);

        let v_axle = (vl + vr) / 2.0;
        let omega = (vr - vl) / self.track_width.get::<meter>();

        // 3. Translate to robot center.  When the axle is offset from the
        //    center, a turning robot experiences a sideways velocity at the
        //    center: v_y = ω · axle_offset (robot frame).
        let theta_rad = self.theta.get::<radian>();
        let v_x = v_axle * theta_rad.cos() - omega * self.axle_offset.get::<meter>() * theta_rad.sin();
        let v_y = v_axle * theta_rad.sin() + omega * self.axle_offset.get::<meter>() * theta_rad.cos();

        self.theta = Angle::new::<radian>(theta_rad + omega * dt);
        self.x = Length::new::<meter>(self.x.get::<meter>() + v_x * dt);
        self.y = Length::new::<meter>(self.y.get::<meter>() + v_y * dt);
    }
}
