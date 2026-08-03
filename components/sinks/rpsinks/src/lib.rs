use cu29::cutask::{CuSinkTask, Freezable};
use cu29::prelude::*;
use cu29::reflect::Reflect;
use rppal::pwm::{Channel, Pwm};
use std::time::Duration;

const DEFAULT_FREQUENCY_HZ: f64 = 50.0;
const DEFAULT_DEPLOYED_US: f64 = 1500.0;
const DEFAULT_UNDEPLOYED_US: f64 = 1000.0;
const DEFAULT_CHANNEL: u8 = 0;

#[derive(Reflect)]
pub struct BinaryServo {
    pwm: Pwm,
    deployed_pulse: Duration,
    undeployed_pulse: Duration,
}

impl Freezable for BinaryServo {}

impl CuSinkTask for BinaryServo {
    type Input<'m> = input_msg!(bool);
    type Resources<'r> = ();

    fn new(
        config: Option<&ComponentConfig>,
        _resources: Self::Resources<'_>,
    ) -> CuResult<Self>
    where
        Self: Sized,
    {
        let (frequency, deployed_us, undeployed_us, channel) = match config {
            Some(cfg) => (
                cfg.get::<f64>("frequency_hz")?.unwrap_or(DEFAULT_FREQUENCY_HZ),
                cfg.get::<f64>("deployed_us")?.unwrap_or(DEFAULT_DEPLOYED_US),
                cfg.get::<f64>("undeployed_us")?.unwrap_or(DEFAULT_UNDEPLOYED_US),
                cfg.get::<u8>("channel")?.unwrap_or(DEFAULT_CHANNEL),
            ),
            None => (
                DEFAULT_FREQUENCY_HZ,
                DEFAULT_DEPLOYED_US,
                DEFAULT_UNDEPLOYED_US,
                DEFAULT_CHANNEL,
            ),
        };

        let ch = Channel::try_from(channel)
            .map_err(|e| CuError::from(format!("BinaryServo: invalid PWM channel: {e:?}")))?;

        let pwm = Pwm::with_frequency(
            ch,
            frequency,
            0.0,
            rppal::pwm::Polarity::Normal,
            true,
        )
        .map_err(|e| CuError::from(format!("BinaryServo: PWM init failed: {e:?}")))?;

        pwm.enable()
            .map_err(|e| CuError::from(format!("BinaryServo: PWM enable failed: {e:?}")))?;

        Ok(Self {
            pwm,
            deployed_pulse: Duration::from_micros(deployed_us as u64),
            undeployed_pulse: Duration::from_micros(undeployed_us as u64),
        })
    }

    fn process(&mut self, _ctx: &CuContext, input: &Self::Input<'_>) -> CuResult<()> {
        let Some(deployed) = input.payload() else {
            return Ok(());
        };

        let target = if *deployed {
            self.deployed_pulse
        } else {
            self.undeployed_pulse
        };

        self.pwm
            .set_pulse_width(target)
            .map_err(|e| CuError::from(format!("BinaryServo: set_pulse_width failed: {e:?}")))?;

        debug!(
            "BinaryServo: deployed={}, pulse_width={:?}",
            deployed, target
        );

        Ok(())
    }

    fn stop(&mut self, _ctx: &CuContext) -> CuResult<()> {
        self.pwm
            .disable()
            .map_err(|e| CuError::from(format!("BinaryServo: PWM disable failed: {e:?}")))?;
        Ok(())
    }
}