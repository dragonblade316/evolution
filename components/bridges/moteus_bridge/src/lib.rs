pub mod messages;

use std::f32;
use std::sync::{Arc, Mutex};

use common::MotorCMD;
use cu29::prelude::*;
use cu29::units::si::angle::{radian, revolution};
use cu29::units::si::angular_velocity::{radian_per_second, revolution_per_second};
use cu29::units::si::f32::Torque;
use cu29::units::si::f32::{Angle, AngularVelocity};
use cu29::units::si::torque::newton_meter;
use cu29::{
    CuResult,
    config::ComponentConfig,
    cutask::Freezable,
    prelude::{BridgeChannel, BridgeChannelConfig, BridgeChannelSet, CuBridge, CuContext},
};
use moteus::command::PositionCommand;
use moteus::transport::Router;
use moteus::transport::singleton::get_singleton_transport;
use moteus::{BlockingController, Mode};

#[derive(Debug)]
struct MoteusMotor {
    ctrl: BlockingController<Arc<Mutex<Router>>>,
}

/// Optional per-TX-channel limits/gains applied to every outgoing position command.
///
/// Only fields present in the channel config are set on the moteus command; unset
/// fields are left as `None` so they are not serialized on the wire.
///
/// Config keys (all optional f32, moteus-native units unless noted):
/// - `kp`            → `kp_scale` (dimensionless scale)
/// - `kd`            → `kd_scale` (dimensionless scale)
/// - `max_vel`       → `velocity_limit` (rev/s)
/// - `max_accel`     → `accel_limit` (rev/s²)
/// - `max_torque`    → `maximum_torque` (N·m)
/// - `position_min`  → clamps commanded position (rev); not a separate wire field
/// - `position_max`  → clamps commanded position (rev); not a separate wire field
#[derive(Debug, Clone, Default, Reflect)]
struct TxCommandConfig {
    kp: Option<f32>,
    kd: Option<f32>,
    max_vel: Option<f32>,
    max_accel: Option<f32>,
    max_torque: Option<f32>,
    position_min: Option<f32>,
    position_max: Option<f32>,
}

impl TxCommandConfig {
    fn from_config(cfg: &ComponentConfig) -> CuResult<Self> {
        Ok(Self {
            kp: cfg.get::<f32>("kp")?,
            kd: cfg.get::<f32>("kd")?,
            max_vel: cfg.get::<f32>("max_vel")?,
            max_accel: cfg.get::<f32>("max_accel")?,
            max_torque: cfg.get::<f32>("max_torque")?,
            position_min: cfg.get::<f32>("position_min")?,
            position_max: cfg.get::<f32>("position_max")?,
        })
    }

    /// Apply configured fields onto a command. Unset options are left untouched
    /// so they are not included in the serialized moteus frame.
    fn apply(&self, mut cmd: PositionCommand) -> PositionCommand {
        // Position min/max are not separate moteus command fields; clamp the
        // finite position setpoint when either bound is configured.
        if let Some(pos) = cmd.position.filter(|p| p.is_finite()) {
            if self.position_min.is_some() || self.position_max.is_some() {
                let mut p = pos;
                if let Some(min) = self.position_min {
                    p = p.max(min);
                }
                if let Some(max) = self.position_max {
                    p = p.min(max);
                }
                cmd = cmd.position(p);
            }
        }

        if let Some(v) = self.kp {
            cmd = cmd.kp_scale(v);
        }
        if let Some(v) = self.kd {
            cmd = cmd.kd_scale(v);
        }
        if let Some(v) = self.max_vel {
            cmd = cmd.velocity_limit(v);
        }
        if let Some(v) = self.max_accel {
            cmd = cmd.accel_limit(v);
        }
        if let Some(v) = self.max_torque {
            cmd = cmd.maximum_torque(v);
        }
        cmd
    }
}

#[derive(Debug)]
struct MoteusChannelConfig<Id: Copy> {
    id: Id,
    can_id: usize,
    motor: MoteusMotor,
    /// Present only for TX channels.
    tx_cmd: TxCommandConfig,
    last_fault: i8,
    last_mode: Option<Mode>,
}

#[derive(Reflect)]
#[reflect(from_reflect = false, no_field_bounds, type_path = false)]
pub struct MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{
    #[reflect(ignore)]
    tx_channels: Vec<MoteusChannelConfig<Tx::Id>>,
    #[reflect(ignore)]
    rx_channels: Vec<MoteusChannelConfig<Rx::Id>>,
}


impl<Tx, Rx> cu29::reflect::TypePath for MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{
    fn type_path() -> &'static str {
        "moteus_bridge::MoteusBridge"
    }

    fn short_type_path() -> &'static str {
        "MoteusBridge"
    }

    fn type_ident() -> Option<&'static str> {
        Some("MoteusBridge")
    }

    fn crate_name() -> Option<&'static str> {
        Some("moteus_bridge")
    }

    fn module_path() -> Option<&'static str> {
        Some("moteus_bridge")
    }
}

impl<Tx, Rx> MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{
    fn cmd(
        pos: Option<Angle>,
        vel: Option<AngularVelocity>,
        torque: Option<Torque>,
    ) -> PositionCommand {
        // Moteus protocol uses revolutions / rev/s; MotorCMD carries SI rad / rad/s.
        let mut cmd = PositionCommand::new()
            .position(match pos {
                Some(a) => a.get::<revolution>(),
                None => f32::NAN,
            })
            .velocity(match vel {
                Some(a) => a.get::<revolution_per_second>(),
                None => f32::NAN,
            });
        if let Some(t) = torque {
            cmd = cmd.feedforward_torque(t.get::<newton_meter>());
        }
        cmd
    }
}

impl<Tx, Rx> Freezable for MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{
}

impl<Tx, Rx> CuBridge for MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{
    type Resources<'r> = ();
    type Tx = Tx;
    type Rx = Rx;

    fn new(
        _config: Option<&ComponentConfig>,
        tx_channels: &[BridgeChannelConfig<<Self::Tx as BridgeChannelSet>::Id>],
        rx_channels: &[BridgeChannelConfig<<Self::Rx as BridgeChannelSet>::Id>],
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        let transport = get_singleton_transport(None)
            .map_err(|e| CuError::new_with_cause("Failed to get moteus transport", e))?;
        let tx_channels: Vec<MoteusChannelConfig<<Self::Tx as BridgeChannelSet>::Id>> = tx_channels
            .iter()
            .map(|channel| {
                let cfg = channel
                    .config
                    .as_ref()
                    .ok_or_else(|| CuError::from("Moteus command channel is missing config"))?;
                let can_id = cfg
                    .get::<String>("can_id")?
                    .ok_or_else(|| CuError::from("Moteus command channel is missing can_id"))?
                    .parse::<usize>()
                    .map_err(|_| CuError::from("Invalid Moteus command can_id"))?;
                let tx_cmd = TxCommandConfig::from_config(cfg)?;
                Ok(MoteusChannelConfig {
                    id: channel.channel.id,
                    can_id,
                    motor: MoteusMotor {
                        ctrl: BlockingController::with_transport(can_id as u8, transport.clone()),
                    },
                    tx_cmd,
                    last_fault: 0,
                    last_mode: None,
                })
            })
            .collect::<CuResult<Vec<_>>>()?;

        let rx_channels: Vec<MoteusChannelConfig<Rx::Id>> = rx_channels
            .iter()
            .map(|channel| {
                let can_id = channel
                    .config
                    .as_ref()
                    .ok_or_else(|| CuError::from("Moteus status channel is missing config"))?
                    .get::<String>("can_id")?
                    .ok_or_else(|| CuError::from("Moteus status channel is missing can_id"))?
                    .parse::<usize>()
                    .map_err(|_| CuError::from("Invalid Moteus status can_id"))?;
                Ok(MoteusChannelConfig {
                    id: channel.channel.id,
                    can_id,
                    motor: MoteusMotor {
                        ctrl: BlockingController::with_transport(can_id as u8, transport.clone()),
                    },
                    tx_cmd: TxCommandConfig::default(),
                    last_fault: 0,
                    last_mode: None,
                })
            })
            .collect::<CuResult<Vec<_>>>()?;

        Ok(MoteusBridge::<Tx, Rx> {
            tx_channels,
            rx_channels,
        })
    }

    fn send<'a, Payload>(
        &mut self,
        _ctx: &CuContext,
        channel: &'static BridgeChannel<<Self::Tx as BridgeChannelSet>::Id, Payload>,
        msg: &CuMsg<Payload>,
    ) -> CuResult<()>
    where
        Payload: CuMsgPayload + 'a,
    {
        let channel_config = self
            .tx_channels
            .iter_mut()
            .find(|configured| configured.id == channel.id)
            .ok_or_else(|| CuError::from("Moteus status channel is not configured"))?;

        let cmd_msg: &CuMsg<MotorCMD> = msg.downcast_ref()?;
        if let Some(cmd) = cmd_msg.payload() {
            // debug!(
            //     "moteus send can_id={}: {:?}",
            //     channel_config.can_id, cmd
            // );
            let cmd = match cmd {
                MotorCMD::Position(pos, vel, torque) => Self::cmd(Some(*pos), *vel, *torque),
                MotorCMD::Velocity(vel, torque) => Self::cmd(None, Some(*vel), *torque),
                MotorCMD::Torque(_torque) => {
                    return Err(CuError::from("Torque commands are not supported by bridge"));
                }
                MotorCMD::Stop => {
                    let _ = channel_config.motor.ctrl.set_stop();
                    return Ok(());
                }
            };
            let cmd = channel_config.tx_cmd.apply(cmd);
            // debug!("moteus position_cmd can_id={}", channel_config.can_id);
            channel_config
                .motor
                .ctrl
                .set_position_no_query(&cmd)
                .map_err(|error| CuError::new_with_cause("Moteus position command failed", error))?;
        }
        Ok(())
    }

    fn receive<'a, Payload>(
        &mut self,
        ctx: &CuContext,
        channel: &'static BridgeChannel<<Self::Rx as BridgeChannelSet>::Id, Payload>,
        msg: &mut CuMsg<Payload>,
    ) -> CuResult<()>
    where
        Payload: CuMsgPayload + 'a,
    {
        let channel_config = self
            .rx_channels
            .iter_mut()
            .find(|configured| configured.id == channel.id)
            .ok_or_else(|| CuError::from("Moteus status channel is not configured"))?;
        let can_id = channel_config.can_id as u8;

        let result = channel_config
            .motor
            .ctrl
            .query()
            .map_err(|e| CuError::new_with_cause("Moteus query failed", e))?;

        if channel_config.last_mode != Some(result.mode) {
            let mode = result.mode.to_string();
            info!(
                ctx,
                "Moteus CAN ID {} mode changed to {}",
                can_id,
                mode
            );
            channel_config.last_mode = Some(result.mode);
        }

        if result.fault != 0 && result.fault != channel_config.last_fault {
            error!(
                ctx,
                "Moteus fault detected on CAN ID {}: fault code {}",
                can_id,
                result.fault
            );
        }
        channel_config.last_fault = result.fault;

        msg.downcast_mut()?.set_payload(messages::MoteusData {
            canid: can_id,
            data: common::MotorData {
                pos: Angle::new::<revolution>(result.position),
                vel: AngularVelocity::new::<revolution_per_second>(result.velocity),
                accel: None,
                torque: Some(Torque::new::<newton_meter>(result.torque)),
            },
            temp: result.temperature,
            voltage: result.voltage,
            fault: result.fault,
        });
        Ok(())
    }

    fn stop(&mut self, _ctx: &CuContext) -> CuResult<()> {
        //motors registered to tx channels are the only ones that should ever be moving.
        for c in self.tx_channels.iter_mut() {
            let _ = c.motor.ctrl.set_stop();
        }
        Ok(())
    }
}
