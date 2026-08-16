//! Tasks whose behavior varies by the driver station alliance.

use common::{Allience, DSStatus};
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg, CuError, CuResult,
};
use cu_spatial_payloads::Transform3D;

/// Selects a field transform based on the alliance reported by the driver station.
///
/// Required component configuration:
///
/// ```ron
/// config: {
///     "red_transform": [[1.0, 0.0, 0.0, 0.0], ...],
///     "blue_transform": [[1.0, 0.0, 0.0, 0.0], ...],
/// }
/// ```
///
/// Each value is a 4 by 4 transformation matrix accepted by
/// [`Transform3D::from_matrix`]. Disabled and enabled statuses select their recorded
/// alliance; a disconnected status produces no output.
pub struct AllianceTransformSelector {
    red_transform: Transform3D<f64>,
    blue_transform: Transform3D<f64>,
}

impl AllianceTransformSelector {
    fn transform_for_status(&self, status: &DSStatus) -> Option<Transform3D<f64>> {
        match status {
            DSStatus::DISCONNECTED => None,
            DSStatus::DISABLED(Allience::RED) | DSStatus::ENABLED(Allience::RED) => {
                Some(self.red_transform)
            }
            DSStatus::DISABLED(Allience::BLUE) | DSStatus::ENABLED(Allience::BLUE) => {
                Some(self.blue_transform)
            }
        }
    }
}

impl Freezable for AllianceTransformSelector {}

impl CuTask for AllianceTransformSelector {
    type Input<'m> = input_msg!(DSStatus);
    type Output<'m> = output_msg!(Transform3D<f64>);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        let config = config.ok_or_else(|| CuError::from("Missing alliance transform configuration"))?;
        let red_transform = read_transform(config, "red_transform")?;
        let blue_transform = read_transform(config, "blue_transform")?;

        Ok(Self {
            red_transform,
            blue_transform,
        })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let status = match input.payload() {
            Some(status) => status,
            None => return Ok(()),
        };

        if let Some(transform) = self.transform_for_status(status) {
            output.set_payload(transform);
        }

        Ok(())
    }
}

fn read_transform(config: &ComponentConfig, key: &str) -> CuResult<Transform3D<f64>> {
    let matrix = config
        .get_value::<[[f64; 4]; 4]>(key)?
        .ok_or_else(|| CuError::from(format!("Missing required configuration key: {key}")))?;

    Ok(Transform3D::from_matrix(matrix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selector() -> AllianceTransformSelector {
        AllianceTransformSelector {
            red_transform: Transform3D::from_matrix([
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 2.0],
                [0.0, 0.0, 1.0, 3.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
            blue_transform: Transform3D::from_matrix([
                [1.0, 0.0, 0.0, 4.0],
                [0.0, 1.0, 0.0, 5.0],
                [0.0, 0.0, 1.0, 6.0],
                [0.0, 0.0, 0.0, 1.0],
            ]),
        }
    }

    #[test]
    fn red_statuses_select_the_red_transform() {
        let selector = selector();

        for status in [DSStatus::ENABLED(Allience::RED), DSStatus::DISABLED(Allience::RED)] {
            assert_eq!(
                selector.transform_for_status(&status).unwrap().to_matrix(),
                selector.red_transform.to_matrix()
            );
        }
    }

    #[test]
    fn blue_statuses_select_the_blue_transform() {
        let selector = selector();

        for status in [DSStatus::ENABLED(Allience::BLUE), DSStatus::DISABLED(Allience::BLUE)] {
            assert_eq!(
                selector.transform_for_status(&status).unwrap().to_matrix(),
                selector.blue_transform.to_matrix()
            );
        }
    }

    #[test]
    fn disconnected_status_selects_no_transform() {
        assert!(selector()
            .transform_for_status(&DSStatus::DISCONNECTED)
            .is_none());
    }
}
