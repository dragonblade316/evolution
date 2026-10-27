use cu29::cutask::{CuSinkTask, Freezable};
use cu29::prelude::*;
use cu29::reflect::Reflect;
use rppal::pwm::{Channel, Polarity, Pwm};
use std::time::Duration;

const DEFAULT_FREQUENCY_HZ: f64 = 50.0;
const DEFAULT_DEPLOYED_US: f64 = 1500.0;
const DEFAULT_UNDEPLOYED_US: f64 = 1000.0;
const DEFAULT_CHANNEL: u8 = 0;

#[derive(Reflect)]
#[reflect(from_reflect = false, no_field_bounds)]
pub struct BinaryServo {
    #[reflect(ignore)]
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

        if !frequency.is_finite() || frequency <= 0.0 {
            return Err("BinaryServo: frequency_hz must be finite and greater than zero".into());
        }
        if !deployed_us.is_finite()
            || !undeployed_us.is_finite()
            || deployed_us < 0.0
            || undeployed_us < 0.0
        {
            return Err("BinaryServo: pulse widths must be finite and non-negative".into());
        }

        let period = Duration::from_secs_f64(1.0 / frequency);
        let deployed_pulse = Duration::from_secs_f64(deployed_us / 1_000_000.0);
        let undeployed_pulse = Duration::from_secs_f64(undeployed_us / 1_000_000.0);
        if deployed_pulse > period || undeployed_pulse > period {
            return Err("BinaryServo: pulse width cannot exceed the PWM period".into());
        }

        let channel = Channel::try_from(channel)
            .map_err(|error| CuError::from(format!("BinaryServo: invalid PWM channel: {error:?}")))?;
        let pwm = Pwm::with_period(
            channel,
            period,
            undeployed_pulse,
            Polarity::Normal,
            true,
        )
        .map_err(|error| CuError::from(format!("BinaryServo: PWM init failed: {error:?}")))?;

        Ok(Self {
            pwm,
            deployed_pulse,
            undeployed_pulse,
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
            .map_err(|error| CuError::from(format!("BinaryServo: set_pulse_width failed: {error:?}")))?;

        debug!(
            "BinaryServo: deployed={}, pulse_width={:?}",
            deployed, target
        );

        Ok(())
    }

    fn stop(&mut self, _ctx: &CuContext) -> CuResult<()> {
        self.pwm
            .disable()
            .map_err(|error| CuError::from(format!("BinaryServo: PWM disable failed: {error:?}")))?;
        Ok(())
    }
}
