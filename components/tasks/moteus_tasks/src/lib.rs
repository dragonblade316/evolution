use common::{DiffDriveSpeeds, TurretState};
use cu29::{
    config::ComponentConfig,
    cutask::{CuMsg, CuTask, Freezable},
    input_msg, output_msg,
    reflect::Reflect,
    units::si::{
        angular_velocity::radian_per_second,
        f32::{
            Angle, AngularAcceleration, AngularVelocity, Length, Torque, Velocity,
        },
        length::meter,
        velocity::meter_per_second,
    },
    CuResult,
};
use cu29::prelude::*;
use moteus_bridge::messages::MoteusData;

// ---------------------------------------------------------------------------
// MoteusDataSplitter — MoteusData → individually wireable telemetry fields
// ---------------------------------------------------------------------------

/// Semantic aliases make fields with the same underlying Rust type distinct
/// in Copper graph configuration while remaining directly compatible with
/// consumers of those underlying types.
pub type MoteusCanId = u8;
pub type MoteusPosition = Angle;
pub type MoteusVelocity = AngularVelocity;
pub type MoteusAcceleration = Option<AngularAcceleration>;
pub type MoteusTorque = Option<Torque>;
pub type MoteusTemperature = f32;
pub type MoteusVoltage = f32;
pub type MoteusFault = i8;

/// Splits one moteus telemetry message into independently wireable outputs.
#[derive(Reflect)]
pub struct MoteusDataSplitter {}

impl Freezable for MoteusDataSplitter {}

impl CuTask for MoteusDataSplitter {
    type Input<'m> = input_msg!(MoteusData);
    type Output<'m> = output_msg!(
        'm,
        MoteusCanId,
        MoteusPosition,
        MoteusVelocity,
        MoteusAcceleration,
        MoteusTorque,
        MoteusTemperature,
        MoteusVoltage,
        MoteusFault
    );
    type Resources<'r> = ();

    fn new(
        _config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let Some(data) = input.payload() else {
            return Ok(());
        };

        output.0.set_payload(data.canid);
        output.1.set_payload(data.data.pos);
        output.2.set_payload(data.data.vel);
        output.3.set_payload(data.data.accel);
        output.4.set_payload(data.data.torque);
        output.5.set_payload(data.temp);
        output.6.set_payload(data.voltage);
        output.7.set_payload(data.fault);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MoteusDiff — per-motor MoteusData → DiffDriveSpeeds
// ---------------------------------------------------------------------------

type LeftMoteusData = MoteusData;
type RightMoteusData = MoteusData;

/// Collects individual motor telemetry and produces diff-drive wheel speeds.
///
/// Config keys:
///   wheel_radius — wheel radius in meters (default: 0.1)
#[derive(Reflect)]
pub struct MoteusDiff {
    wheel_radius: Length,
}

impl Freezable for MoteusDiff {}

impl CuTask for MoteusDiff {
    type Input<'m> = input_msg!('m, LeftMoteusData, RightMoteusData);
    type Output<'m> = output_msg!(DiffDriveSpeeds);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let wheel_radius = Length::new::<meter>(match config {
            Some(cfg) => cfg.get::<f32>("wheel_radius")?.unwrap_or(0.1),
            None => 0.1,
        });
        Ok(Self { wheel_radius })
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (left_msg, right_msg) = *input;
        let (Some(left), Some(right)) = (left_msg.payload(), right_msg.payload()) else {
            return Ok(());
        };

        let r = self.wheel_radius.get::<meter>();
        output.set_payload(DiffDriveSpeeds {
            left: Velocity::new::<meter_per_second>(left.data.vel.get::<radian_per_second>() * r),
            right: Velocity::new::<meter_per_second>(right.data.vel.get::<radian_per_second>() * r),
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MoteusTurretStateAssembler — flywheel/turret MoteusData → TurretState
// ---------------------------------------------------------------------------

type FlywheelMoteusData = MoteusData;
type TurretMoteusData = MoteusData;

/// Combines flywheel and turret motor telemetry into the turret state consumed
/// by turret control tasks.
///
/// The flywheel speed comes from the flywheel motor velocity, and the turret
/// motor supplies the turret position.
#[derive(Reflect)]
pub struct MoteusTurretStateAssembler {}

impl Freezable for MoteusTurretStateAssembler {}

impl MoteusTurretStateAssembler {
    fn assemble(flywheel: &MoteusData, turret: &MoteusData) -> TurretState {
        TurretState {
            flywheel: flywheel.data.vel,
            position: turret.data.pos,
        }
    }
}

impl CuTask for MoteusTurretStateAssembler {
    type Input<'m> = input_msg!('m, FlywheelMoteusData, TurretMoteusData);
    type Output<'m> = output_msg!(TurretState);
    type Resources<'r> = ();

    fn new(
        _config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn process(
        &mut self,
        _ctx: &cu29::prelude::CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        let (flywheel_msg, turret_msg) = *input;
        let (Some(flywheel), Some(turret)) = (flywheel_msg.payload(), turret_msg.payload()) else {
            return Ok(());
        };

        output.set_payload(Self::assemble(flywheel, turret));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cu29::units::si::{
        angle::degree,
        angular_acceleration::radian_per_second_squared,
        angular_velocity::revolution_per_minute,
        f32::{Angle, AngularAcceleration, AngularVelocity, Torque},
        torque::newton_meter,
    };

    fn run_splitter(
        input: CuMsg<MoteusData>,
    ) -> <MoteusDataSplitter as CuTask>::Output<'static> {
        let mut splitter = MoteusDataSplitter::new(None, ()).unwrap();
        let (ctx, _mock) = cu29::prelude::CuContext::new_mock_clock();
        let mut output = Default::default();
        splitter.process(&ctx, &input, &mut output).unwrap();
        output
    }

    #[test]
    fn splits_all_moteus_data_fields() {
        let input = CuMsg::new(Some(MoteusData {
            canid: 4,
            data: common::MotorData {
                pos: Angle::new::<degree>(135.0),
                vel: AngularVelocity::new::<revolution_per_minute>(4_200.0),
                accel: Some(AngularAcceleration::new::<radian_per_second_squared>(12.5)),
                torque: Some(Torque::new::<newton_meter>(3.25)),
            },
            temp: 41.5,
            voltage: 23.8,
            fault: -2,
        }));

        let output = run_splitter(input);

        assert_eq!(output.0.payload(), Some(&4));
        assert_eq!(output.1.payload().unwrap().get::<degree>(), 135.0);
        assert_eq!(
            output.2.payload().unwrap().get::<revolution_per_minute>(),
            4_200.0
        );
        assert_eq!(
            output.3.payload().unwrap().unwrap().get::<radian_per_second_squared>(),
            12.5
        );
        assert_eq!(
            output.4.payload().unwrap().unwrap().get::<newton_meter>(),
            3.25
        );
        assert_eq!(output.5.payload(), Some(&41.5));
        assert_eq!(output.6.payload(), Some(&23.8));
        assert_eq!(output.7.payload(), Some(&-2));
    }

    #[test]
    fn emits_no_outputs_without_an_input_payload() {
        let output = run_splitter(CuMsg::new(None));

        assert!(output.0.payload().is_none());
        assert!(output.1.payload().is_none());
        assert!(output.2.payload().is_none());
        assert!(output.3.payload().is_none());
        assert!(output.4.payload().is_none());
        assert!(output.5.payload().is_none());
        assert!(output.6.payload().is_none());
        assert!(output.7.payload().is_none());
    }

    #[test]
    fn emits_explicit_none_for_unavailable_optional_telemetry() {
        let output = run_splitter(CuMsg::new(Some(MoteusData::default())));

        assert_eq!(output.3.payload(), Some(&None));
        assert_eq!(output.4.payload(), Some(&None));
    }

    #[test]
    fn assembles_turret_state_from_the_two_motor_messages() {
        let flywheel = MoteusData {
            data: common::MotorData {
                vel: AngularVelocity::new::<revolution_per_minute>(4_200.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let turret = MoteusData {
            data: common::MotorData {
                pos: Angle::new::<degree>(135.0),
                ..Default::default()
            },
            ..Default::default()
        };

        let state = MoteusTurretStateAssembler::assemble(&flywheel, &turret);

        assert_eq!(state.flywheel.get::<revolution_per_minute>(), 4_200.0);
        assert_eq!(state.position.get::<degree>(), 135.0);
    }
}
