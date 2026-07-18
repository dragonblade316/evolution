pub mod messages;

use std::collections::HashMap;
use std::f32;
use std::sync::{Arc, Mutex};

use common::MotorCMD;
use cu29::units::si::angle::{radian, revolution};
use cu29::units::si::angular_velocity::{radian_per_second, revolution_per_second};
use cu29::units::si::f32::{Angle, AngularVelocity};
use cu29::units::si::torque::newton_meter;
use cu29::units::si::f32::Torque;
use cu29::{
    config::ComponentConfig,
    cutask::Freezable,
    prelude::{BridgeChannel, BridgeChannelConfig, BridgeChannelSet, CuBridge, CuContext},
    CuResult,
};
use moteus::Transport;
use moteus::command::PositionCommand;
use moteus::transport::Router;
use moteus::{BlockingController, transport::singleton::get_singleton_transport};
use cu29::prelude::*;
use messages::*;

#[derive(Debug)]
struct MoteusMotor {
    ctrl: BlockingController<Arc<Mutex<Router>>>
}

struct MoteusChannelConfig<Id: Copy> {
    id: Id,
    can_id: usize,
    motor: MoteusMotor
}

pub struct MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static

{
    tx_channels: Vec<MoteusChannelConfig<Tx::Id>>,
    rx_channels: Vec<MoteusChannelConfig<Rx::Id>>
}

impl<Tx, Rx> MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{
    fn cmd(pos: Option<Angle>, vel: Option<AngularVelocity>, torque: Option<Torque>) -> PositionCommand {
        PositionCommand::new()
            .position(match pos {
                Some(a) => a.get::<radian>(),
                None => f32::NAN
            })
            .velocity(match vel {
                Some(a) => a.get::<radian_per_second>(),
                None => f32::NAN
            })
    }
}



impl<Tx, Rx> Freezable for MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{}

impl<Tx, Rx> CuBridge for MoteusBridge<Tx, Rx>
    where
        Tx: BridgeChannelSet + 'static,
        Rx: BridgeChannelSet + 'static,
        Tx::Id: Send + Sync + 'static,
        Rx::Id: Send + Sync + 'static
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


        let transport = get_singleton_transport(None).map_err(|e| CuError::new_with_cause("Failed to get moteus transport", e))?;
        let tx_channels: Vec<MoteusChannelConfig<<Self::Tx as BridgeChannelSet>::Id>> = tx_channels
            .iter()
            .map(|channel| {
                let can_id = channel
                    .config
                    .as_ref()
                    .ok_or_else(|| CuError::from("Moteus command channel is missing config"))?
                    .get::<String>("can_id")?
                    .ok_or_else(|| CuError::from("Moteus command channel is missing can_id"))?
                    .parse::<usize>()
                    .map_err(|_| CuError::from("Invalid Moteus command can_id"))?;
                Ok(MoteusChannelConfig {
                    id: channel.channel.id,
                    can_id,
                    motor: MoteusMotor {
                        ctrl: BlockingController::with_transport(can_id as u8, transport.clone()),
                    },
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
                })
            })
            .collect::<CuResult<Vec<_>>>()?;

        let transport = get_singleton_transport(None).map_err(|e| CuError::new_with_cause("Failed to get moteus transport", e))?;

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
            let cmd = match cmd {
                MotorCMD::Position(pos, vel, torque) => Self::cmd(Some(*pos), *vel, *torque),
                MotorCMD::Velocity(vel, torque) => Self::cmd(None, Some(*vel), *torque),
                MotorCMD::Torque(torque) => return Err(CuError::from("Torque commands are not supported by bridge")),
                MotorCMD::Stop => {
                    let _ = channel_config.motor.ctrl.set_stop();
                    return Ok(());
                }
            };
            channel_config.motor.ctrl.set_position(cmd);
        }
        Ok(())
    }

    fn receive<'a, Payload>(
        &mut self,
        _ctx: &CuContext,
        channel: &'static BridgeChannel<<Self::Rx as BridgeChannelSet>::Id, Payload>,
        msg: &mut CuMsg<Payload>,
    ) -> CuResult<()>
    where
        Payload: CuMsgPayload + 'a,
    {
        let channel_config = self
            .rx_channels
            .iter()
            .find(|configured| configured.id == channel.id)
            .ok_or_else(|| CuError::from("Moteus status channel is not configured"))?;
        let can_id = channel_config.can_id as u8;

        let result = channel_config.motor.ctrl.query()?;

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
