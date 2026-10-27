use bno08x_rs::BNO08x;
use cu29::prelude::*;
use cu29::units::si::{angular_velocity::radian_per_second, f32::AngularVelocity};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

const DEFAULT_SPI_DEVICE: &str = "/dev/spidev0.0";
const DEFAULT_GPIO_CHIP: &str = "/dev/gpiochip0";
const DEFAULT_INT_LINE: u32 = 25;
const DEFAULT_RESET_LINE: u32 = 24;
const DEFAULT_REPORT_INTERVAL_MS: u32 = 10;

enum BnoCommand {
    Init(u16, Sender<Result<(), String>>),
    HandleMessages(usize, Sender<()>),
    ReadGyro(Sender<Result<[f32; 3], String>>),
}

#[derive(Reflect)]
#[reflect(from_reflect = false, no_field_bounds)]
pub struct Bno085 {
    #[reflect(ignore)]
    commands: Sender<BnoCommand>,
    report_interval_ms: u16,
}

impl Freezable for Bno085 {}

impl CuSrcTask for Bno085 {
    type Resources<'r> = ();
    type Output<'m> = output_msg!(AngularVelocity);

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let spi_device = config
            .map(|cfg| cfg.get::<String>("spi_device"))
            .transpose()?
            .flatten()
            .unwrap_or_else(|| DEFAULT_SPI_DEVICE.to_string());
        let gpio_chip = config
            .map(|cfg| cfg.get::<String>("gpio_chip"))
            .transpose()?
            .flatten()
            .unwrap_or_else(|| DEFAULT_GPIO_CHIP.to_string());
        let int_line = config
            .map(|cfg| cfg.get::<u32>("int_line"))
            .transpose()?
            .flatten()
            .unwrap_or(DEFAULT_INT_LINE);
        let reset_line = config
            .map(|cfg| cfg.get::<u32>("reset_line"))
            .transpose()?
            .flatten()
            .unwrap_or(DEFAULT_RESET_LINE);
        let report_interval_ms = config
            .map(|cfg| cfg.get::<u32>("report_interval_ms"))
            .transpose()?
            .flatten()
            .unwrap_or(DEFAULT_REPORT_INTERVAL_MS);
        let report_interval_ms = u16::try_from(report_interval_ms)
            .map_err(|_| CuError::from("BNO085 report_interval_ms must fit in u16"))?;

        let (commands, ready) =
            spawn_bno_worker(spi_device, gpio_chip, int_line, reset_line);
        ready
            .recv()
            .map_err(|error| {
                CuError::from(format!("BNO085 worker stopped during startup: {error}"))
            })?
            .map_err(CuError::from)?;

        Ok(Self {
            commands,
            report_interval_ms,
        })
    }

    fn start(&mut self, _ctx: &CuContext) -> CuResult<()> {
        let (reply, result) = channel();
        self.commands
            .send(BnoCommand::Init(self.report_interval_ms, reply))
            .map_err(|error| CuError::from(format!("BNO085 worker unavailable: {error}")))?;
        result
            .recv()
            .map_err(|error| CuError::from(format!("BNO085 worker stopped during init: {error}")))?
            .map_err(CuError::from)
    }

    fn preprocess(&mut self, _ctx: &CuContext) -> CuResult<()> {
        let (reply, result) = channel();
        self.commands
            .send(BnoCommand::HandleMessages(5, reply))
            .map_err(|error| CuError::from(format!("BNO085 worker unavailable: {error}")))?;
        result.recv().map_err(|error| {
            CuError::from(format!("BNO085 worker stopped while reading: {error}"))
        })
    }

    fn process(&mut self, _ctx: &CuContext, output: &mut Self::Output<'_>) -> CuResult<()> {
        let (reply, result) = channel();
        self.commands
            .send(BnoCommand::ReadGyro(reply))
            .map_err(|error| CuError::from(format!("BNO085 worker unavailable: {error}")))?;
        let gyro = result
            .recv()
            .map_err(|error| {
                CuError::from(format!("BNO085 worker stopped while reading gyro: {error}"))
            })?
            .map_err(CuError::from)?;
        output.set_payload(AngularVelocity::new::<radian_per_second>(gyro[2]));
        Ok(())
    }
}

fn spawn_bno_worker(
    spi_device: String,
    gpio_chip: String,
    int_line: u32,
    reset_line: u32,
) -> (Sender<BnoCommand>, Receiver<Result<(), String>>) {
    let (commands, requests) = channel();
    let (startup, ready) = channel();

    thread::spawn(move || {
        let mut imu = match BNO08x::new_spi(
            &spi_device,
            &gpio_chip,
            int_line,
            &gpio_chip,
            reset_line,
        ) {
            Ok(imu) => {
                let _ = startup.send(Ok(()));
                imu
            }
            Err(error) => {
                let _ = startup.send(Err(format!("BNO085 startup failed: {error}")));
                return;
            }
        };

        while let Ok(command) = requests.recv() {
            match command {
                BnoCommand::Init(report_interval_ms, reply) => {
                    let result = imu
                        .init()
                        .map_err(|error| format!("{error:?}"))
                        .and_then(|_| {
                            imu.enable_gyro(report_interval_ms)
                                .map_err(|error| format!("{error:?}"))
                        })
                        .and_then(|enabled| {
                            enabled
                                .then_some(())
                                .ok_or_else(|| "failed to enable gyroscope report".to_string())
                        });
                    let _ = reply.send(result);
                }
                BnoCommand::HandleMessages(timeout_ms, reply) => {
                    imu.handle_all_messages(timeout_ms);
                    let _ = reply.send(());
                }
                BnoCommand::ReadGyro(reply) => {
                    let _ = reply.send(imu.gyro().map_err(|error| format!("{error:?}")));
                }
            }
        }
    });

    (commands, ready)
}
