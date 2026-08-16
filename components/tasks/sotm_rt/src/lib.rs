use common::ChassisSpeeds;
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    CuResult,
};
use cu29::units::si::angle::radian;
use cu29::units::si::f32::{Angle, Velocity};
use cu29::units::si::velocity::meter_per_second;
use cu_spatial_payloads::Pose;

use std::f64::consts::PI;
use eqsolver::nalgebra::DVector;
use ivp::prelude::*;

/// Turret azimuth measured clockwise/counterclockwise from "straight
/// ahead" of the robot (0 rad = muzzle points along the robot's +x axis).
/// Carries the same SI radian storage as [`turret_tasks::TurretAngle`] so
/// downstream consumers (e.g. [`turret_tasks::TurretAngleSolver`]) can wire
/// straight in.
pub type TurretAngle = Angle;

/// Muzzle speed along the firing axis. SI meters/second.
pub type MuzzleVelocity = Velocity;

struct Ball {}
impl SecondOrderSystem for Ball {
    fn acceleration(&self, t: f64, q: &[f64], a: &mut [f64]) {
        a[0] = 0.0;
        a[1] = -9.81;
    }
}

struct SOTM {
    /// 3D position of the goal in world frame [x, y, z]
    goal_position: [f64; 3],
    /// 3D position of the turret relative to the robot center [x, y, z]
    turret_offset: [f64; 3],
}

impl SOTM {

    ///qt: The position of the target relative to the robot. [x, y] (typically only x will change since it is the distance of the robot from the goal)
    ///vcx: the velocity that the robot is moving toward the goal.
    ///angle_deg: launch (hood) angle above the horizontal, in degrees.
    ///Returns: Some([velocity_out, tof]) on convergence, or None if the shot is
    ///unsolvable for the given angle/distance (e.g. too close to the goal).
    fn solve_range(qt: [f64; 2], vcx: f64, angle_deg: f64) -> Option<[f64; 2]> {
        let q0 = [0.0,0.0];

        let target_objective = |guess: DVector<f64>| {
            let vx = guess[0] * f64::cos(angle_deg * (PI/180.0));
            let vy = guess[0] * f64::sin(angle_deg * (PI/180.0));
            let tof = guess[1];

            let v0 = [vx+vcx, vy];

            if tof <= 0.0 {
                return DVector::from_vec(vec![1e6, 1e6, 1e6]);
            }

            let sol = Ivp::second_order(&Ball {}, 0.0, tof, &q0, &v0).solve().unwrap();

            let y = sol.y.last().unwrap();
            let qx = y[0];
            let qy = y[1];

            let error_x = qx - qt[0];
            let error_y = qy - qt[1];
            let random_con = error_x * error_y;

            DVector::from_vec(vec!{error_x, error_y, random_con})
        };

        let solver = eqsolver::multivariable::GaussNewtonFD::new(target_objective);
        let sol = solver.solve(DVector::from_vec(vec![1.0, 1.0])).ok()?;
        Some([sol[0], sol[1]])
    }

    ///v: Lateral velocity of the robot relative to the goal
    ///x: Distance of the robot from the goal
    ///tof: Estimated time of flight
    fn solve_angle(v: f64, x: f64, tof: f64) -> (f64, f64) {
        let L = v*tof;
        let theta = f64::atan2(L, x);
        let R = f64::sqrt(L*L + x*x);
        (theta, R)
    }

    ///dist_to_goal: Ground-plane distance from the turret to the goal.
    ///dz: Vertical difference from the turret to the goal (goal_z - turret_z).
    ///v_parallel: Turret velocity along the toward-goal axis (positive = closing distance).
    ///v_perpendicular: Turret velocity across the goal axis (lateral).
    ///heading: Robot yaw in world frame (radians).
    ///goal_direction: Angle from turret to goal in world frame (radians).
    ///Returns: (turret_angle, muzzle_velocity)
    fn solve(dist_to_goal: f64, dz: f64, v_parallel: f64, v_perpendicular: f64, heading: f64, goal_direction: f64) -> (f64, f64) {
        // Angle from the front of the robot to the goal
        let angle_front_to_goal = goal_direction - heading;

        let qt = [dist_to_goal, dz];
        let [_velocity, tof] = Self::solve_range(qt, v_parallel, 60.0).unwrap();
        let (theta, r) = Self::solve_angle(v_perpendicular, dist_to_goal, tof);
        let qt = [r, dz];
        let [velocity_out, _] = Self::solve_range(qt, v_parallel, 60.0).unwrap();

        let turret_angle = angle_front_to_goal + theta;
        (turret_angle, velocity_out)
    }
}

impl Freezable for SOTM {

}

impl CuTask for SOTM {
    type Input<'m> = input_msg!((Pose<f64>, ChassisSpeeds));

    type Output<'m> = output_msg!('m, TurretAngle, MuzzleVelocity);

    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized {
        const DEFAULT_GOAL_POSITION: [f64; 3] = [4.0, 2.0, 2.5];
        const DEFAULT_TURRET_OFFSET: [f64; 3] = [0.1, 0.0, 0.5];

        let goal_position = config
            .and_then(|cfg| cfg.get_value::<[f64; 3]>("goal_position").ok().flatten())
            .unwrap_or(DEFAULT_GOAL_POSITION);

        let turret_offset = config
            .and_then(|cfg| cfg.get_value::<[f64; 3]>("turret_offset").ok().flatten())
            .unwrap_or(DEFAULT_TURRET_OFFSET);

        Ok(Self { goal_position, turret_offset })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let input_msg = input;
        let (robot_pose, robot_speeds) = match input_msg.payload() {
            Some(p) => p,
            None => return Ok(()),
        };

        // --- Robot state ---
        let robot_translation = robot_pose.translation();
        let robot_x = robot_translation[0].raw();
        let robot_y = robot_translation[1].raw();
        let robot_z = robot_translation[2].raw();

// Extract yaw from the 3x3 rotation matrix. `Pose::rotation()` returns the
// raw dimensionless rotation matrix as `[[T; 3]; 3]` (see cu-spatial-payloads
// 1.1.0 docs), so the element access is already an `f64` — no `.raw()` needed.
let rot = robot_pose.rotation();
let heading = f64::atan2(rot[1][0], rot[0][0]);

        let robot_vx = robot_speeds.x.raw() as f64;
        let robot_vy = robot_speeds.y.raw() as f64;
        let robot_omega = robot_speeds.theta.raw() as f64;

        // --- Turret pose (world frame) ---
        // Rotate the turret offset from body frame into world frame
        let [tx, ty, tz] = self.turret_offset;
        let (sin_h, cos_h) = heading.sin_cos();
        let offset_world_x = tx * cos_h - ty * sin_h;
        let offset_world_y = tx * sin_h + ty * cos_h;

        let turret_x = robot_x + offset_world_x;
        let turret_y = robot_y + offset_world_y;
        let turret_z = robot_z + tz;

        // --- Turret velocity (world frame) ---
        // v_turret = v_robot + omega × r_offset
        let turret_vx = robot_vx - robot_omega * offset_world_y;
        let turret_vy = robot_vy + robot_omega * offset_world_x;

        // --- Decompose into parallel/perpendicular axes relative to goal ---
        // Vector from turret to goal (ground plane)
        let dx = self.goal_position[0] - turret_x;
        let dy = self.goal_position[1] - turret_y;
        let dist_to_goal = f64::hypot(dx, dy);

        // Unit vectors: toward-goal (parallel) and perpendicular (CCW 90°)
        let (ux, uy) = if dist_to_goal > 0.0 {
            (dx / dist_to_goal, dy / dist_to_goal)
        } else {
            (1.0, 0.0)
        };
        let (px, py) = (-uy, ux);

        // Turret velocity projected onto these axes
        let v_parallel = turret_vx * ux + turret_vy * uy;   // positive = toward goal
        let v_perpendicular = turret_vx * px + turret_vy * py;

        // Vertical difference from turret to goal
        let dz = self.goal_position[2] - turret_z;

        // Direction from turret to goal in world frame
        let goal_direction = f64::atan2(dy, dx);

        let (turret_angle, muzzle_velocity) = Self::solve(
            dist_to_goal, dz, v_parallel, v_perpendicular, heading, goal_direction,
        );

        // Wrap the f64 solver outputs into the typed SI unit exports expected
        // by the rest of the graph (`TurretAngle` matches the `Angle` consumed
        // by turret_tasks::TurretAngleSolver; `MuzzleVelocity` matches the
        // `Velocity` used by DiffDriveSpeeds). Truncate f64→f32 here: the
        // impact of the last 7+ digits is well below motor encoder noise.
        let (turret_angle_out, muzzle_velocity_out) = output;
        turret_angle_out.set_payload(TurretAngle::new::<radian>(turret_angle as f32));
        muzzle_velocity_out
            .set_payload(MuzzleVelocity::new::<meter_per_second>(muzzle_velocity as f32));

        Ok(())
    }
}

/// Public analysis API used by the `angleseek` binary.
///
/// For a static shooter (no robot motion) the lateral-correction pass in
/// `SOTM::solve` is a no-op (`v_perpendicular = 0` ⇒ `theta = 0`, `r = x`),
/// so the muzzle velocity for a given `(distance, dz, hood_angle)` reduces to
/// a single `solve_range` call. This wrapper exposes that directly.
///
/// `max_velocity` is the physically plausible ceiling (m/s). Solutions above
/// this — or below `MIN_VELOCITY` — are treated as unsolvable, which happens
/// near the singularity where the solver converges on absurd values.
const MIN_VELOCITY: f64 = 0.1;

pub fn solve_muzzle_velocity(
    distance: f64,
    dz: f64,
    hood_angle_deg: f64,
    max_velocity: f64,
) -> Option<f64> {
    SOTM::solve_range([distance, dz], 0.0, hood_angle_deg)
        .map(|[v, _]| v)
        .filter(|&v| v >= MIN_VELOCITY && v <= max_velocity)
}
