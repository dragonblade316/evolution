use cu29::{
    clock::{CuDuration, CuTime},
    prelude::*,
};
use common::{DSStatus, GamePadState};

use crate::cu29::payloads::{DsTxMsg, DxRxMsg};

#[derive(Reflect)]
pub struct DsRx {
    seq: u64,
    last: DsTxMsg,
    last_message_timestamp: CuTime,
}

impl Freezable for DsRx {}

impl CuTask for DsRx {
    type Input<'m> = input_msg!(DsTxMsg);
    type Output<'m> = output_msg!('m, DSStatus, GamePadState);
    type Resources<'r> = ();

    fn new(_config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
        where
            Self: Sized {
                Ok(Self {
                    seq: 0,
                    last: DsTxMsg::default(),
                    last_message_timestamp: CuTime(0)
                })
    }

    fn process(
            &mut self,
            ctx: &cu29::prelude::CuContext,
            input: &Self::Input<'_>,
            output: &mut Self::Output<'_>,
        ) -> cu29::CuResult<()> {
            let cmd = if let Some(cmd) = input.payload() {
                if cmd.seq <= self.seq {
                    return Err(CuError::new(199)
                        .add_cause("DsTxMsg received out of order; leaving the last command active"));
                }
                if !input.metadata.process_time.end.is_none() {
                    self.last_message_timestamp = input.metadata.process_time.end.unwrap();
                }
                self.last = cmd.clone();
                self.seq = cmd.seq;
                cmd.clone()
            } else {
                self.last.clone()
            };

            if ctx.now() - self.last_message_timestamp > CuDuration::from_millis(500) {
                output.0.set_payload(DSStatus::DISCONNECTED);
                return Ok(())
            }
            if cmd.estop {
                output.0.set_payload(DSStatus::ESTOPPED);
                return Ok(())
            }

            let status = match cmd.enabled {
                true => DSStatus::ENABLED(cmd.allience.clone()),
                false => DSStatus::DISABLED(cmd.allience.clone()),
            };

            output.0.set_payload(status);
            output.1.set_payload(cmd.gamepad.clone());

            Ok(())
    }
}

/// Acknowledges a driver-station command on the robot-to-driver-station
/// channel. The echoed sequence number lets the driver station detect loss and
/// measure round-trip time.
#[derive(Reflect)]
pub struct DsTx;

impl Freezable for DsTx {}

impl CuTask for DsTx {
    type Input<'m> = input_msg!(DsTxMsg);
    type Output<'m> = output_msg!(DxRxMsg);
    type Resources<'r> = ();

    fn new(
        _config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self)
    }

    fn process(
        &mut self,
        _ctx: &CuContext,
        input: &Self::Input<'_>,
        output: &mut Self::Output<'_>,
    ) -> CuResult<()> {
        match input.payload() {
            Some(command) => output.set_payload(DxRxMsg { seq: command.seq }),
            None => output.clear_payload(),
        }

        Ok(())
    }
}
