import math

import numpy as np

# Definition for a sensor-fusion differential-drive robot.
# The simulator implements these objects and injects one DifferentialDriveStateEstimator
# instance into RobotPolicy.step.
#
# class Pose2d:
#     x: float
#     y: float
#     yaw: float
#
# class AprilTagPoseEstimate:
#     tag_id: int
#     timestamp: float
#     pose: Pose2d
#     distance_m: float
#     bearing_rad: float
#     position_std_m: float
#     yaw_std_rad: float
#     ambiguity: float
#
# class DifferentialDriveStateEstimator(MobileRobot):
#     wheel_base_m: float = 0.2667
#     wheel_radius_m: float = 0.0660
#     ticks_per_rev: int = 392
#     dt: float = 0.05
#
#     def get_encoder_values() -> tuple[int, int]:
#         # Cumulative left and right encoder ticks since episode start.
#         # Ticks are quantized from the executed wheel motion.
#
#     def gyro() -> float:
#         # Angular velocity about the robot vertical axis.
#         # Challenge contracts declare whether yaw rate is exact, biased, or noisy.
#
#     def april_tag_measurements() -> tuple[AprilTagPoseEstimate, ...]:
#         # New field-relative robot-pose estimates delivered by the front camera this
#         # timestep.
#         # Measurements are intermittent, delayed, and include distance- and
#         # view-angle-dependent noise plus an ambiguity score.
#
#     def submit_odometry(pose: Pose2d) -> None:
#         # Pose estimate in the episode-start frame, where the robot begins at x=0, y=0,
#         # yaw=0.
#         # Scoring compares each submitted estimate against hidden ground truth for the
#         # same timestep.


class SO2:
    def __init__(self, theta):
        self.theta = theta

    def exp(theta):
        return SO2(theta)

    def log(self):
        return self.theta


class SE2:
    def __init__(self, rot, translation):
        self.translation = np.asarray(translation, dtype=float)
        self.rot = rot

    def exp(vec):
        vec = np.asarray(vec, dtype=float).reshape(3)
        v = vec[:2]
        theta = vec[2]
        R = SO2.exp(theta)
        if abs(theta) < 1e-7:
            t = v
        else:
            V = np.array(
                [
                    [np.sin(theta) / theta, -(1 - np.cos(theta)) / theta],
                    [(1 - np.cos(theta)) / theta, np.sin(theta) / theta],
                ]
            )
            t = V @ v
        return SE2(t, R)

    def as_matrix(self) -> "np.ndarray":

        R = self.rot.as_matrix()
        t = self.translation
        M = np.eye(3)
        M[:2, :2] = R
        M[:2, 2] = t
        return M

    @staticmethod
    def identity() -> "SE2":
        """Return the identity transformation."""
        return SE2(SO2.exp(0.0), np.zeros(2))

    @classmethod
    def from_matrix(cls, m: "np.ndarray") -> "SE2":

        R = SO2.from_matrix(m[:2, :2])
        t = m[:2, 2]
        return cls(R, t)

    def __mul__(self, other: "SE2") -> "SE2":
        """Group composition."""
        R1 = self.rot.as_matrix()
        R2 = other.rot.as_matrix()
        R = SO2.from_matrix(R1 @ R2)
        t = self.translation + R1 @ other.translation
        return SE2(R, t)

    def inverse(self) -> "SE2":
        """Return the inverse transformation."""
        R_inv = self.rot.as_matrix().T
        R = SO2.from_matrix(R_inv)
        t = -(R_inv @ self.translation)
        return SE2(R, t)


def adjoint_se2(g: SE2) -> "np.ndarray":
    """Return the adjoint matrix of an SE2 element."""
    R = g.rot.as_matrix()
    x, y = g.translation
    adj = np.eye(3)
    adj[:2, :2] = R
    adj[0, 2] = -y
    adj[1, 2] = x
    return adj


class Odom:
    def __init__(self):
        # self.x = 0.0
        # self.y = 0.0
        self.yaw = 0.0
        self.last_right = 0.0
        self.last_left = 0.0

    def step(self, robot):
        left_ticks, right_ticks = robot.get_encoder_values()
        yaw_rate = robot.gyro()

        # Calculate wheel distances using self.last_left and self.last_right
        left = (((left_ticks - self.last_left) / 392) * 2 * math.pi * 0.066) / 0.05
        right = (((right_ticks - self.last_right) / 392) * 2 * math.pi * 0.066) / 0.05

        # Update yaw using self
        self.yaw = self.yaw + (yaw_rate * 0.05)
        V = (left + right) / 2

        # Added math. prefix to cos and sin
        # self.x = self.x + (V * math.cos(self.yaw) * 0.05)
        # self.y = self.y + (V * math.sin(self.yaw) * 0.05)

        self.x = V * math.cos(self.yaw)
        self.y = V * math.sin(self.yaw)

        # Update the history for the next step
        self.last_left = left_ticks
        self.last_right = right_ticks

        return np.array([(V * math.cos(self.yaw)), (V * math.sin(self.yaw)), yaw_rate])
        # Submit the updated pose


class iekf:
    def __init__(self):
        dim = 3
        self.pose = SE2.identity()
        self.P = np.eye(dim) * 1e-3
        self.Q = np.eye(dim) * 1e-4
        self.R = np.eye(dim) * 1e-2

    def predict(self, v, dt):
        self.pose = self.pose * SE2.exp(v * dt)
        A = adjoint_se2(SE2.exp(v * dt))
        self.P = Ad @ self.P @ Ad.T + self.Q * dt * dt

    def mesurement(measurement):
        err = (self.pose.inverse() * measurement).log()
        S = self.P + self.R
        K = np.linalg.solve(S.T, self.P.T).T
        delta = K @ err
        self.pose = self.pose * SE2.exp(delta)
        self.P = (np.eye(self.P.shape[0]) - K) @ self.P


class RobotPolicy:
    def __init__(self):
        # Initialize variables as object attributes
        self.odom = Odom()
        self.iekf = iekf()

    def step(self, robot):
        left_ticks, right_ticks = robot.get_encoder_values()
        yaw_rate = robot.gyro()
        tag_measurements = robot.april_tag_measurements()

        self.iekf.predict(self.odom.step(robot))

        for i in tag_measurements:
            p = i.pose
            m = np.array([p.x, p.y, p.yaw])
            self.iekf.measurement(m)

        t = self.iekf.translation
        r = self.iekf.rot

        robot.submit_odometry(Pose2d(t[0], t[1], r))
