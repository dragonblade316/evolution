use std::collections::VecDeque;
use std::time::Instant;

use cu29::prelude::*;
use common::ChassisSpeeds;
use cu_apriltag::AprilTagDetections;
use cu_spatial_payloads::{Pose, Transform3D};
use cu29::cutask::{CuMsg, CuTask, Freezable};
use cu29::reflect::Reflect;

use cu29::units::si::angular_velocity::radian_per_second;
use cu29::units::si::f32::{AngularVelocity, Velocity};
use cu29::units::si::velocity::meter_per_second;
use cu29::{input_msg, output_msg};
use sophus::lie::{Isometry2F64, Rotation2};
use sophus::nalgebra::{Matrix3, Vector2, Vector3};

struct IEKF {
    pose: Isometry2F64,
    P: Matrix3<f64>,
    Q: Matrix3<f64>,
    R: Matrix3<f64>,
}

impl IEKF {
    fn predict(&mut self, twist: Vector3<f64>, dt: f64) {
        let exp_v = Isometry2F64::exp(twist * dt);
        self.pose = self.pose * exp_v;
        let ad = exp_v.inverse().adj();
        // TODO: this should be Addition, not multiplication:
        //   self.P = ad * self.P * ad.transpose() + self.Q * dt;
        self.P = ad * self.P * ad.transpose() + self.Q * dt;


    }
    fn mesurement(&mut self, measurement: Isometry2F64) {
        let err = (self.pose.inverse() * measurement).log();

        let H = Matrix3::<f64>::identity();
        let S = H * self.P * H.transpose() + self.R;

        //AI bc translating from nalgebra is pain:
        //original:  K = np.linalg.solve(S.T, (self.P @ H.T).T).T
        // 1. Compute (self.P @ H.T).T  ==>  H @ P.T (or just H @ P since P is symmetric)
        let num_matrix = &H * &self.P; // Note: (P @ H.T).T simplifies algebraically to H @ P

        // 2. Compute S.T (which is just S, since S is symmetric)
        let S_t = S.transpose();

        // 3. Solve the linear system S_t * X = num_matrix, then transpose the result
        let K = S_t.cholesky()
            .expect("Cholesky decomposition failed: S is not positive-definite")
            .solve(&num_matrix)
            .transpose();

        let delta = K * err;
        self.pose = self.pose * Isometry2F64::exp(delta);
        self.P = (Matrix3::<f64>::identity() - K * H) * self.P;
    }
}

struct TimeMachineIEKF {
    iekf: IEKF,
    //not technically needed but whatev
    last: Instant,
    buf: VecDeque<(Instant, (ChassisSpeeds, Isometry2F64, Matrix3<f64>))>
}

impl TimeMachineIEKF {
    fn update(&mut self, speeds: ChassisSpeeds, timestamp: Instant) {
        // sophus SE(2) tangent order is (theta, x, y) — rotation first.
        let twist = Vector3::new(
            speeds.theta.get::<radian_per_second>() as f64,
            speeds.x.get::<meter_per_second>() as f64,
            speeds.y.get::<meter_per_second>() as f64,
        );
        self.iekf.predict(twist, timestamp.duration_since(self.last).as_secs_f64());
        self.buf.push_back((timestamp, (speeds, self.iekf.pose, self.iekf.P)));

        //since we start
        for i in 0..self.buf.len() - 1 {
            if timestamp.duration_since(self.buf[i].0).as_secs_f64() > 1.5 {
                let _ = self.buf.pop_front();
            } else {
                //There should never be a situation where there is an entry newer further up in the list
                break
            }
        }
        self.last = timestamp;
    }

    fn update_vision(&mut self, pose: &Transform3D<f32>, timestamp: Instant) {
        let mat = pose.to_matrix();
        let x = mat[0][3] as f64;
        let y = mat[1][3] as f64;
        let theta = f64::atan2(mat[1][0] as f64, mat[0][0] as f64);
        let iso: Isometry2F64 = Isometry2F64::from_rotation_and_translation(
            Rotation2::rot(theta),
            Vector2::new(x, y),
        );

        let Some(pos) = self.buf.iter().position(|(t, _)| *t >= timestamp) else {
            return;
        };
        if pos == 0 {
            return;
        }
        let (t0, (_, iso0, P0)) = &self.buf[pos - 1];
        let (t1, (_, iso1, P1)) = &self.buf[pos];
        let total = t1.duration_since(*t0).as_secs_f64();
        let elapsed = timestamp.duration_since(*t0).as_secs_f64();
        let w = elapsed / total;
        let interpolated_pose = iso0.interpolate(iso1, w);
        let interpolated_P = Self::interpolate_cov(P0, P1, w);

        self.iekf.pose = interpolated_pose;
        self.iekf.P = interpolated_P;
        self.iekf.mesurement(iso);

        let mut prev_t = timestamp;
        for i in pos..self.buf.len() {
            let (t, (speeds, _, _)) = &self.buf[i];
            let dt = t.duration_since(prev_t).as_secs_f64();
            let twist = Vector3::new(
                speeds.theta.get::<radian_per_second>() as f64,
                speeds.x.get::<meter_per_second>() as f64,
                speeds.y.get::<meter_per_second>() as f64,
            );
            self.iekf.predict(twist, dt);
            prev_t = *t;
        }
    }

    fn interpolate_cov(P1: &Matrix3<f64>, P2: &Matrix3<f64>, t: f64) -> Matrix3<f64> {
        let L1 = P1.cholesky().unwrap().l();
        let L2 = P2.cholesky().unwrap().l();
        let L = L1 + (L2 - L1) * t;
        &L * L.transpose()
    }

    fn get_pose(&self) -> Pose<f64> {
        iso_to_pose(&self.iekf.pose)
    }

}

fn iso_to_pose(iso: &Isometry2F64) -> Pose<f64> {
    let mat = iso.matrix();
    let x = mat[(0, 2)];
    let y = mat[(1, 2)];
    let theta = iso.rotation().log()[0];
    let c = theta.cos();
    let s = theta.sin();
    Transform3D::from_matrix([
        [c, -s, 0.0, x],
        [s,  c, 0.0, y],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ])
}



#[derive(Reflect)]
pub struct IEKFPoseEstimator {
    tm: TimeMachineIEKF,
    speeds: ChassisSpeeds
}

impl Freezable for IEKFPoseEstimator {}

impl CuTask for IEKFPoseEstimator {
    type Input<'m> = input_msg!('m, ChassisSpeeds, AprilTagDetections);
    type Output<'m> = output_msg!(Pose<f64>, ChassisSpeeds);
    type Resources<'r> = ();

    fn new(
        _config: Option<&cu29::prelude::ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> cu29::CuResult<Self>
    where
        Self: Sized,
    {
        let iekf = IEKF {
            pose: Isometry2F64::identity(),
            P: Matrix3::<f64>::identity(),
            Q: Matrix3::<f64>::identity(),
            R: Matrix3::<f64>::identity(),
        };
        let tm = TimeMachineIEKF {
            iekf,
            last: std::time::Instant::now(),
            buf: VecDeque::new(),
        };
        let speeds = ChassisSpeeds {
            x: Velocity::new::<meter_per_second>(0.0),
            y: Velocity::new::<meter_per_second>(0.0),
            theta: AngularVelocity::new::<radian_per_second>(0.0),
        };
        Ok(Self { tm, speeds })
    }

    fn process(
        &mut self,
        ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> cu29::CuResult<()> {
        let now = std::time::Instant::now();
        debug!("hello world");

        if let Some(speeds) = input.0.payload() {
            self.tm.update(speeds.clone(), now);
            self.speeds = speeds.clone();
        }
        if let Some(detections) = input.1.payload() {
            let best = detections.filtered_by_decision_margin(0.0).next();
            if let Some((_id, pose, _margin)) = best {
                self.tm.update_vision(pose, now);
            }
        }

        output.0.set_payload(self.tm.get_pose());

        // Rotate robot-relative (vx, vy) into the field frame using the
        // current estimated heading. yaw rate is frame-invariant.
        let theta = self.tm.iekf.pose.rotation().log()[0] as f32;
        let (c, s) = (theta.cos(), theta.sin());
        let vx_r = self.speeds.x.get::<meter_per_second>();
        let vy_r = self.speeds.y.get::<meter_per_second>();
        let field_speeds = ChassisSpeeds {
            x: Velocity::new::<meter_per_second>(vx_r * c - vy_r * s),
            y: Velocity::new::<meter_per_second>(vx_r * s + vy_r * c),
            theta: self.speeds.theta,
        };
        output.1.set_payload(field_speeds);

        let mat = self.tm.iekf.pose.matrix();
        debug!(
            "IEKF pose: x={:.3}, y={:.3}, theta={:.3}",
            mat[(0, 2)],
            mat[(1, 2)],
            self.tm.iekf.pose.rotation().log()[0],
        );

        Ok(())
    }
}
