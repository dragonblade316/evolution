use bno08x_rs::{BNO08x, interface::{SpiInterface, gpio::{GpiodIn, GpiodOut}, spidev::SpiDevice}};
use cu29::cutask::{CuSrcTask, Freezable};
use cu29::prelude::ComponentConfig;
use cu29::units::si::f32::*;

const DEFAULT_SPI_DEVICE: &str = "/dev/spidev1.0";
const DEFAULT_IMU_INT: &str = "IMU_INT";
const DEFAULT_IMU_RST: &str = "IMU_RST";

struct Bno085 {
    imu: BNO08x<'static, SpiInterface<SpiDevice, GpiodIn, GpiodOut>>
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

                let mut imu = BNO08x::new_spi_from_symbol(
                        &spi_device,
                        &imu_int,
                        &imu_rst,
                    ).unwrap();

                Ok(Self {
                    imu
                })
    }

    fn start(&mut self, _ctx: &cu29::prelude::CuContext) -> cu29::CuResult<()> {
        self.imu.init().unwrap();
        Ok(())
    }

    fn preprocess(&mut self, _ctx: &cu29::prelude::CuContext) -> cu29::CuResult<()> {
        self.imu.handle_all_messages(5);
        Ok(())
    }


    fn process(
            &mut self,
            _ctx: &cu29::prelude::CuContext,
            output: &mut Self::Output<'_>,
        ) -> cu29::CuResult<()> {
            self.imu.gyro();

            Ok(())
    }
}
