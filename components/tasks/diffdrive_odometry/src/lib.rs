use common::{ChassisSpeeds, DiffDriveSpeeds};
use cu29::prelude::*;
use cu29::{
    clock::CuTime,
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    reflect::Reflect,
    units::si::{
        angular_velocity::radian_per_second,
        f32::{AngularVelocity, Length, Velocity},
        length::meter,
        velocity::meter_per_second,
    },
    CuError, CuResult,
};
use kfilter::kalman::{KalmanFilter, KalmanLinearNoInput, KalmanPredict, KalmanUpdate};
use kfilter::measurement::{LinearMeasurement, Measurement};
use nalgebra::{Matrix1, Vector1};

#[derive(Reflect)]
pub struct DiffDriveOdometry {
    trackwidth: Length,
    wheel_radius: Length,
    /// Distance from the axle to the robot's center (control point).
    /// Positive = center is ahead of the axle (axle at rear).
    /// Default: 0 (center on axle).
    axle_offset: Length,
}

impl Freezable for DiffDriveOdometry {}

impl CuTask for DiffDriveOdometry {
    type Input<'m> = input_msg!(DiffDriveSpeeds);
    type Output<'m> = output_msg!(ChassisSpeeds);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        const DEFAULT_WHEEL_RADIUS_METERS: f32 = 0.1;
        const DEFAULT_TRACKWIDTH_METERS: f32 = 0.3;
        const DEFAULT_AXLE_OFFSET_METERS: f32 = 0.0;

        let wheel_radius = match config {
            Some(cfg) => Length::new::<meter>(
                cfg.get::<f32>("wheel_radius")?
                    .unwrap_or(DEFAULT_WHEEL_RADIUS_METERS),
            ),
            None => Length::new::<meter>(DEFAULT_WHEEL_RADIUS_METERS),
        };

        let trackwidth = match config {
            Some(cfg) => Length::new::<meter>(
                cfg.get::<f32>("trackwidth")?
                    .unwrap_or(DEFAULT_TRACKWIDTH_METERS),
            ),
            None => Length::new::<meter>(DEFAULT_TRACKWIDTH_METERS),
        };

        let axle_offset = match config {
            Some(cfg) => Length::new::<meter>(
                cfg.get::<f32>("axle_offset")?
                    .unwrap_or(DEFAULT_AXLE_OFFSET_METERS),
            ),
            None => Length::new::<meter>(DEFAULT_AXLE_OFFSET_METERS),
        };

        Ok(Self {
            wheel_radius,
            trackwidth,
            axle_offset,
        })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let speeds = match input.payload() {
            Some(s) => s,
            None => return Ok(()),
        };

        // Linear and angular velocity at the axle.
        // speeds.left/right are already linear wheel velocities (m/s).
        let v_axle = (speeds.left.raw() + speeds.right.raw()) / 2.0;
        let omega = (speeds.right.raw() - speeds.left.raw())
            / self.trackwidth.raw();

        // Translate to the robot's control point (center).
        // When turning, a point offset from the axle experiences a
        // sideways velocity component: v_y = ω · axle_offset.
        let x = Velocity::new::<meter_per_second>(v_axle);
        let y = Velocity::new::<meter_per_second>(omega * self.axle_offset.raw());
        let theta = AngularVelocity::new::<radian_per_second>(omega);

        output.set_payload(ChassisSpeeds { x, y, theta });
        debug!("DiffDriveOdometry: x={:?}, y={:?}, theta={:?}", x, y, theta);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// DiffDriveOmegaFusion — fuse a chassis-derived yaw rate with an IMU yaw rate
// via a 1D Kalman filter.
//
//   Input 0: ChassisSpeeds            (REQUIRED every cycle — missing => Err)
//   Input 1: AngularVelocity (IMU)     (optional; fused when present)
//
//   Output:  ChassisSpeeds whose `x`/`y` are taken verbatim from the chassis
//            input and whose `theta` is the Kalman-fused yaw rate.
//
// State model: constant angular velocity (F = 1). Process noise Q is rescaled
// per-tick by `process_var * dt`. Two measurements, each a direct observation
// of `omega` with its own variance (R), are applied sequentially; that is the
// standard way kfilter handles multiple rate sensors with one state.
//
// Config keys (all optional):
//   - process_var (f32, default 1.0):  process noise variance per second [(rad/s)^2/s]
//   - chassis_var  (f32, default 0.05): measurement variance of chassis-derived omega [(rad/s)^2]
//   - imu_var      (f32, default 0.001): measurement variance of IMU yaw rate [(rad/s)^2]
// ---------------------------------------------------------------------------
#[derive(Reflect)]
pub struct DiffDriveOmegaFusion {
    /// 1-state Kalman filter estimating the robot yaw rate (omega), in rad/s.
    /// Generic over <T=f32, N=1, U=0>; system is a `LinearNoInputSystem`.
    kf: KalmanLinearNoInput<f32, 1>,
    /// H = 1, R = chassis_var (fixed measurement object; `z` updated per-tick).
    chassis_meas: LinearMeasurement<f32, 1, 1>,
    /// H = 1, R = imu_var (fixed measurement object; `z` updated per-tick).
    imu_meas: LinearMeasurement<f32, 1, 1>,
    /// Per-second process noise variance, scaled by `dt` to recover Q each tick.
    process_var: f32,
    /// Timestamp of the previous `process()` call; `None` on the first tick.
    last_time: Option<CuTime>,
}

impl Freezable for DiffDriveOmegaFusion {}

impl CuTask for DiffDriveOmegaFusion {
    type Input<'m> = input_msg!('m, ChassisSpeeds, AngularVelocity);
    type Output<'m> = output_msg!(ChassisSpeeds);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        const DEFAULT_PROCESS_VAR: f32 = 1.0;
        const DEFAULT_CHASSIS_VAR: f32 = 0.05;
        const DEFAULT_IMU_VAR: f32 = 1.0e-3;

        let (process_var, chassis_var, imu_var) = match config {
            Some(cfg) => (
                cfg.get::<f32>("process_var")?.unwrap_or(DEFAULT_PROCESS_VAR),
                cfg.get::<f32>("chassis_var")?.unwrap_or(DEFAULT_CHASSIS_VAR),
                cfg.get::<f32>("imu_var")?.unwrap_or(DEFAULT_IMU_VAR),
            ),
            None => (DEFAULT_PROCESS_VAR, DEFAULT_CHASSIS_VAR, DEFAULT_IMU_VAR),
        };

        // F = 1 (constant-omega model); Q is supplied here and rescaled per-tick
        // below; initial state x0 = 0; initial covariance P0 = process_var (loose
        // prior so the first measurement dominates quickly).
        let kf = <KalmanLinearNoInput<f32, 1>>::new(
            Matrix1::new(1.0),            // F
            Matrix1::new(process_var),    // Q (rescaled each tick by `process_var * dt`)
            Vector1::new(0.0),            // x0 (rad/s)
            Matrix1::new(process_var),    // P0
        );

        let chassis_meas = LinearMeasurement::new(
            Matrix1::new(1.0),            // H
            Matrix1::new(chassis_var),    // R
            Vector1::new(0.0),            // z (set per-tick)
        );
        let imu_meas = LinearMeasurement::new(
            Matrix1::new(1.0),
            Matrix1::new(imu_var),
            Vector1::new(0.0),
        );

        Ok(Self {
            kf,
            chassis_meas,
            imu_meas,
            process_var,
            last_time: None,
        })
    }

    fn process(
        &mut self,
        ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (chassis_msg, imu_msg) = *input;

        // Requirement: a chassis speed MUST be present, otherwise signal the
        // copper runtime and refuse to emit anything.
        let chassis = chassis_msg.payload().ok_or_else(|| {
            CuError::from(
                "DiffDriveOmegaFusion: chassis speeds payload is missing; \
                 refusing to emit a fused estimate",
            )
        })?;

        // Predict: drive the constant-omega state forward, with Q scaled by dt.
        let now = ctx.now();
        let dt_s = match self.last_time {
            Some(last) => {
                let dt_ns = (now - last).as_nanos();
                if dt_ns > 0 {
                    dt_ns as f32 / 1_000_000_000.0
                } else {
                    0.0
                }
            }
            None => 0.0,
        };
        *self.kf.system_mut().covariance_mut() = Matrix1::new(self.process_var * dt_s);
        let _ = self
            .kf
            .predict()
            .map_err(|e| CuError::from(format!("DiffDriveOmegaFusion: KF predict failed: {e:?}")))?;
        self.last_time = Some(now);

        // Update #1: chassis-derived yaw rate (ω from differential kinematics).
        let omega_chassis = chassis.theta.get::<radian_per_second>();
        self.chassis_meas.set_measurement(Vector1::new(omega_chassis));
        let _ = self
            .kf
            .update(&self.chassis_meas)
            .map_err(|e| CuError::from(format!("DiffDriveOmegaFusion: chassis update failed: {e:?}")))?;

        // Update #2: IMU yaw rate, when available (optional).
        let mut omega_imu: Option<f32> = None;
        if let Some(imu_omega) = imu_msg.payload() {
            let v = imu_omega.get::<radian_per_second>();
            omega_imu = Some(v);
            self.imu_meas.set_measurement(Vector1::new(v));
            let _ = self
                .kf
                .update(&self.imu_meas)
                .map_err(|e| CuError::from(format!("DiffDriveOmegaFusion: imu update failed: {e:?}")))?;
        }

        // Emit the chassis speed with the fused angular velocity; x/y are
        // frame-independent under SE(2) translation so they pass through.
        let omega_fused = self.kf.state()[0];
        let merged = ChassisSpeeds {
            x: chassis.x,
            y: chassis.y,
            theta: AngularVelocity::new::<radian_per_second>(omega_fused),
        };
        output.set_payload(merged);
        debug!(
            "DiffDriveOmegaFusion: omega_chassis={:.3}, omega_imu={:?}, omega_fused={:.3} rad/s",
            omega_chassis, omega_imu, omega_fused
        );
        Ok(())
    }
}
