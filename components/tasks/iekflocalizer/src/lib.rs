use common::ChassisSpeeds;
use cu_apriltag::AprilTagDetections;
use cu_spatial_payloads::{Pose, Transform3D};
use cu29::cutask::{CuMsg, CuTask, Freezable};

use cu29::{input_msg, output_msg};
use sophus::lie::Isometry2F64;
use sophus::nalgebra::{Vector3, Matrix3};

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
        self.P = ad * self.P * ad.transpose() * self.Q * dt;


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

impl Freezable for IEKF {}

impl CuTask for IEKF {
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
        let pose: Isometry2F64 = Isometry2F64::identity();
        let P = Matrix3::<f64>::identity();
        let Q = Matrix3::<f64>::identity();
        let R = Matrix3::<f64>::identity();
        Ok(Self { pose, P, Q, R })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> cu29::CuResult<()> {
        Ok(())
    }
}
