pub mod messages;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cu29::units::si::angle::{revolution};
use cu29::units::si::angular_velocity::{revolution_per_second};
use cu29::units::si::f32::{Angle, AngularVelocity};
use cu29::units::si::torque::newton_meter;
use cu29::units::si::f32::Torque;
use cu29::{
    config::ComponentConfig,
    cutask::Freezable,
    prelude::{BridgeChannel, BridgeChannelConfig, BridgeChannelSet, CuBridge, CuContext},
    CuResult,
};
pub use messages::SharedBridgePayload;
use moteus::Transport;
use moteus::transport::Router;
use moteus::{BlockingController, transport::singleton::get_singleton_transport};
use cu29::prelude::*;
use messages::*;

struct MoteusMotor {
    ctrl: BlockingController<Arc<Mutex<Router>>>
}

struct MoteusChannelConfig<Id: Copy> {
    id: Id,
    can_id: usize
}

pub struct MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static

{
    motors: HashMap<u8, MoteusMotor>,
    tx_channels: Vec<MoteusChannelConfig<Tx::Id>>,
    rx_channels: Vec<MoteusChannelConfig<Rx::Id>>
}

impl<Tx, Rx> Freezable for MoteusBridge<Tx, Rx>
where
    Tx: BridgeChannelSet + 'static,
    Rx: BridgeChannelSet + 'static,
    Tx::Id: Send + Sync + 'static,
    Rx::Id: Send + Sync + 'static,
{}

impl CuBridge for MoteusBridge<Tx, Rx>
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
                })
            })
            .collect::<CuResult<Vec<_>>>()?;

        let rx_channels: Vec<MoteusChannelConfig<Rx::Id> = rx_channels
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
                })
            })
            .collect::<CuResult<Vec<_>>>()?;

        let mut can_ids = tx_channels
            .iter()
            .chain(rx_channels.iter())
            .map(|channel| channel.can_id as u8)
            .collect::<Vec<_>>();
        can_ids.sort_unstable();
        can_ids.dedup();

        let transport = get_singleton_transport(None)
            .map_err(|e| CuError::new_with_cause("Failed to get moteus transport", e))?;
        let motors = can_ids
            .into_iter()
            .map(|can_id| {
                (
                    can_id,
                    MoteusMotor {
                        ctrl: BlockingController::with_transport(can_id, transport.clone()),
                    },
                )
            })
            .collect();

        Ok(MoteusBridge::<Tx, Rx> {
            motors,
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
        let command = msg.downcast_ref::<MoteusCMD>()?;
        let channel_config = self
            .tx_channels
            .iter()
            .find(|configured| configured.id == channel.id)
            .ok_or_else(|| CuError::from("Moteus command channel is not configured"))?;
        if command.can_id as usize != channel_config.can_id {
            return Err(CuError::from(format!(
                "Moteus command CAN ID {} does not match channel CAN ID {}",
                command.can_id, channel_config.can_id
            )));
        }

        // Hardware command encoding remains in the existing Moteus integration.
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
        let motor = self
            .motors
            .get_mut(&can_id)
            .ok_or_else(|| CuError::from("Moteus status channel has no configured motor"))?;
        let result = motor
            .ctrl
            .query()
            .map_err(|e| CuError::from(format!("moteus query error on {can_id}: {e:?}")))?;

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
        for motor in self.motors.values_mut() {
            let _ = motor.ctrl.set_stop();
        }
        Ok(())
    }
}
