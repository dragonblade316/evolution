use bno08x_rs::BNO08x;
use cu29::cutask::{CuSrcTask, Freezable};
use cu29::prelude::*;
use cu29::prelude::ComponentConfig;
use cu29::reflect::Reflect;
use cu29::units::si::f32::*;
use std::sync::mpsc::{channel, Sender};
use std::thread;

const DEFAULT_SPI_DEVICE: &str = "/dev/spidev1.0";
const DEFAULT_IMU_INT: &str = "IMU_INT";
const DEFAULT_IMU_RST: &str = "IMU_RST";

// BNO08x contains non-Send callback state, so keep the driver on its own worker thread.
enum BnoCommand {
    Init(Sender<Result<(), String>>),
    HandleMessages(usize, Sender<()>),
    ReadGyro(Sender<()>),
}

#[derive(Reflect)]
#[reflect(from_reflect = false, no_field_bounds)]
struct Bno085 {
    #[reflect(ignore)]
    commands: Sender<BnoCommand>,
}

impl Freezable for Bno085 {

}

impl CuSrcTask for Bno085 {
    type Output<'m> = AngularVelocity;
    type Resources<'r> = ();


    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
        where
            Self: Sized {
                let spi_device = match config {
                    Some(cfg) => cfg.get::<String>("spi_device")?.unwrap_or_else(|| DEFAULT_SPI_DEVICE.to_string()),
                    None => DEFAULT_SPI_DEVICE.to_string(),
                };
                let imu_int = match config {
                    Some(cfg) => cfg.get::<String>("imu_int")?.unwrap_or_else(|| DEFAULT_IMU_INT.to_string()),
                    None => DEFAULT_IMU_INT.to_string(),
                };
                let imu_rst = match config {
                    Some(cfg) => cfg.get::<String>("imu_rst")?.unwrap_or_else(|| DEFAULT_IMU_RST.to_string()),
                    None => DEFAULT_IMU_RST.to_string(),
                };

                let (commands, ready) = spawn_bno_worker(spi_device, imu_int, imu_rst)?;
                ready.recv().map_err(|e| {
                    CuError::from(format!("BNO085 worker stopped during startup: {e}"))
                })??;

                Ok(Self {
                    commands,
                })
    }

    fn start(&mut self, _ctx: &cu29::prelude::CuContext) -> cu29::CuResult<()> {
        let (reply, result) = channel();
        self.commands
            .send(BnoCommand::Init(reply))
            .map_err(|e| CuError::from(format!("BNO085 worker unavailable: {e}")))?;
        result
            .recv()
            .map_err(|e| CuError::from(format!("BNO085 worker stopped during init: {e}")))?
            .map_err(CuError::from)
    }

    fn preprocess(&mut self, _ctx: &cu29::prelude::CuContext) -> cu29::CuResult<()> {
        let (reply, result) = channel();
        self.commands
            .send(BnoCommand::HandleMessages(5, reply))
            .map_err(|e| CuError::from(format!("BNO085 worker unavailable: {e}")))?;
        result
            .recv()
            .map_err(|e| CuError::from(format!("BNO085 worker stopped while reading: {e}")))
    }


    fn process(
            &mut self,
            _ctx: &cu29::prelude::CuContext,
        output: &mut Self::Output<'_>,
    ) -> cu29::CuResult<()> {
            let (reply, result) = channel();
            self.commands
                .send(BnoCommand::ReadGyro(reply))
                .map_err(|e| CuError::from(format!("BNO085 worker unavailable: {e}")))?;
            result
                .recv()
                .map_err(|e| CuError::from(format!("BNO085 worker stopped while reading gyro: {e}")))
    }
}

fn spawn_bno_worker(
    spi_device: String,
    imu_int: String,
    imu_rst: String,
) -> cu29::CuResult<(Sender<BnoCommand>, std::sync::mpsc::Receiver<Result<(), String>>)> {
    let (commands, requests) = channel();
    let (startup, ready) = channel();

    thread::spawn(move || {
        let mut imu = match BNO08x::new_spi_from_symbol(&spi_device, &imu_int, &imu_rst) {
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
                BnoCommand::Init(reply) => {
                    let _ = reply.send(imu.init().map_err(|error| format!("{error:?}")));
                }
                BnoCommand::HandleMessages(timeout_ms, reply) => {
                    imu.handle_all_messages(timeout_ms);
                    let _ = reply.send(());
                }
                BnoCommand::ReadGyro(reply) => {
                    let _ = imu.gyro();
                    let _ = reply.send(());
                }
            }
        }
    });

    Ok((commands, ready))
}
