use cu_apriltag::AprilTagDetections;
use cu_spatial_payloads::{Pose, Transform3D};
use cu29::prelude::*;
use cu29::units::si::angle::radian;
use cu29::units::si::f32::{Angle, Length};
use cu29::units::si::length::meter;
use serde::Deserialize;

#[derive(Deserialize)]
struct PoseConfig {
    translation_m: [f32; 3],
    rotation_radians: [f32; 3],
}

#[derive(Deserialize)]
struct TagConfig {
    id: usize,
    field_pose: PoseConfig,
}

#[derive(Deserialize)]
struct BoundsConfig {
    min_x_m: f32,
    max_x_m: f32,
    min_y_m: f32,
    max_y_m: f32,
}

#[derive(Deserialize)]
struct ProcessorConfig {
    tags: Vec<TagConfig>,
    robot_to_camera_pose: PoseConfig,
    min_decision_margin: f32,
    ground_z_m: f32,
    max_ground_distance_m: f32,
    #[serde(default)]
    bounds: Option<BoundsConfig>,
}

#[derive(Clone)]
struct KnownTag {
    id: usize,
    field_to_tag: Transform3D<f32>,
}

#[derive(Reflect, Clone)]
#[reflect(opaque, from_reflect = false, no_field_bounds)]
pub struct AprilTagPoseProcessor {
    tags: Vec<KnownTag>,
    robot_to_camera: Transform3D<f32>,
    min_decision_margin: f32,
    ground_z: f32,
    max_ground_distance: f32,
    bounds: Option<[f32; 4]>,
}

impl AprilTagPoseProcessor {
    fn robot_pose(
        &self,
        tag_id: usize,
        camera_to_tag: &Transform3D<f32>,
    ) -> Option<Pose<f32>> {
        let tag = self.tags.iter().find(|tag| tag.id == tag_id)?;
        Some(tag.field_to_tag * camera_to_tag.inverse() * self.robot_to_camera.inverse())
    }

    fn accepts(&self, pose: &Pose<f32>) -> bool {
        let [x, y, z] = pose.translation().map(|length| length.get::<meter>());
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {
            return false;
        }
        if (z - self.ground_z).abs() > self.max_ground_distance {
            return false;
        }
        match self.bounds {
            Some([min_x, max_x, min_y, max_y]) => {
                x >= min_x && x <= max_x && y >= min_y && y <= max_y
            }
            None => true,
        }
    }
}

impl Freezable for AprilTagPoseProcessor {}

impl CuTask for AprilTagPoseProcessor {
    type Resources<'r> = ();
    type Input<'m> = input_msg!(AprilTagDetections);
    type Output<'m> = output_msg!(Pose<f32>);

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let config = config
            .ok_or_else(|| CuError::from("AprilTagPoseProcessor requires configuration"))?;
        let config: ProcessorConfig = config.deserialize_into()?;

        Ok(Self {
            tags: config
                .tags
                .into_iter()
                .map(|tag| KnownTag {
                    id: tag.id,
                    field_to_tag: transform_from_pose(tag.field_pose),
                })
                .collect(),
            robot_to_camera: transform_from_pose(config.robot_to_camera_pose),
            min_decision_margin: config.min_decision_margin,
            ground_z: config.ground_z_m,
            max_ground_distance: config.max_ground_distance_m,
            bounds: config.bounds.map(|bounds| {
                [
                    bounds.min_x_m,
                    bounds.max_x_m,
                    bounds.min_y_m,
                    bounds.max_y_m,
                ]
            }),
        })
    }

    fn process(
        &mut self,
        _ctx: &CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        output.tov = input.tov;
        output.clear_payload();

        let Some(detections) = input.payload() else {
            return Ok(());
        };

        let best_pose = detections
            .filtered_by_decision_margin(self.min_decision_margin)
            .filter_map(|(id, camera_to_tag, margin)| {
                let pose = self.robot_pose(id, camera_to_tag)?;
                self.accepts(&pose).then_some((pose, margin))
            })
            .max_by(|(_, left_margin), (_, right_margin)| {
                left_margin.total_cmp(right_margin)
            });

        if let Some((pose, _)) = best_pose {
            output.set_payload(pose);
        }
        Ok(())
    }
}

fn transform_from_pose(config: PoseConfig) -> Transform3D<f32> {
    Transform3D::<f32>::from_translation_euler_xyz(
        config.translation_m.map(Length::new::<meter>),
        config.rotation_radians.map(Angle::new::<radian>),
    )
}
