use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cu29::prelude::*;
use cu_sensor_payloads::{CuImage, CuImageBufferFormat};
use v4l::buffer::{Flags, Type};
use v4l::io::traits::CaptureStream;
use v4l::prelude::MmapStream;
use v4l::video::Capture;
use v4l::video::capture::Parameters;
use v4l::{Device, Format, FourCC};

#[derive(Clone, Copy)]
struct CameraConfig {
    device: usize,
    width: u32,
    height: u32,
    fps: u32,
    buffers: u32,
    timeout: Duration,
}

struct CapturedFrame {
    bytes: Vec<u8>,
    captured_at: CuTime,
    sequence: u64,
}

#[derive(Reflect)]
#[reflect(from_reflect = false)]
pub struct MjpegCameraSource {
    #[reflect(ignore)]
    config: CameraConfig,
    #[reflect(ignore)]
    format: Option<CuImageBufferFormat>,
    #[reflect(ignore)]
    latest: Arc<Mutex<Option<CapturedFrame>>>,
    #[reflect(ignore)]
    capture_error: Arc<Mutex<Option<String>>>,
    #[reflect(ignore)]
    stop: Arc<AtomicBool>,
    #[reflect(ignore)]
    worker: Option<JoinHandle<()>>,
}

impl Freezable for MjpegCameraSource {}

impl MjpegCameraSource {
    fn stop_worker(&mut self) -> CuResult<()> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| CuError::from("MJPEG capture thread panicked"))?;
        }
        Ok(())
    }
}

impl Drop for MjpegCameraSource {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}

impl CuSrcTask for MjpegCameraSource {
    type Output<'m> = output_msg!(CuImage<Vec<u8>>);
    type Resources<'r> = ();

    fn new(config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self> {
        let config = config.ok_or_else(|| CuError::from("MjpegCameraSource requires configuration"))?;
        let fourcc = config
            .get::<String>("fourcc")?
            .unwrap_or_else(|| "MJPG".to_string());
        if fourcc != "MJPG" {
            return Err(CuError::from("MjpegCameraSource requires fourcc MJPG"));
        }

        Ok(Self {
            config: CameraConfig {
                device: config.get::<u32>("device")?.unwrap_or(0) as usize,
                width: config.get::<u32>("width")?.unwrap_or(1280),
                height: config.get::<u32>("height")?.unwrap_or(720),
                fps: config.get::<u32>("fps")?.unwrap_or(10),
                buffers: config.get::<u32>("buffers")?.unwrap_or(4),
                timeout: Duration::from_millis(
                    config.get::<u32>("timeout_ms")?.unwrap_or(500) as u64,
                ),
            },
            format: None,
            latest: Arc::new(Mutex::new(None)),
            capture_error: Arc::new(Mutex::new(None)),
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        })
    }

    fn start(&mut self, ctx: &CuContext) -> CuResult<()> {
        self.stop.store(false, Ordering::Relaxed);
        *self
            .latest
            .lock()
            .map_err(|_| CuError::from("MJPEG frame lock poisoned"))? = None;
        *self
            .capture_error
            .lock()
            .map_err(|_| CuError::from("MJPEG error lock poisoned"))? = None;

        let config = self.config;
        let (device, format) = configure_camera(config).map_err(CuError::from)?;
        self.format = Some(CuImageBufferFormat {
            width: format.width,
            height: format.height,
            stride: format.stride,
            pixel_format: format.fourcc.repr,
        });

        let latest = Arc::clone(&self.latest);
        let capture_error = Arc::clone(&self.capture_error);
        let stop = Arc::clone(&self.stop);
        let clock = ctx.clock.clone();
        self.worker = Some(
            thread::Builder::new()
                .name("mjpeg-capture".to_string())
                .spawn(move || {
                    if let Err(error) = capture_frames(device, config, latest, stop, clock) {
                        eprintln!("MJPEG capture failed: {error}");
                        if let Ok(mut capture_error) = capture_error.lock() {
                            *capture_error = Some(error);
                        }
                    }
                })
                .map_err(|error| {
                    CuError::new_with_cause("failed to spawn MJPEG capture thread", error)
                })?,
        );
        eprintln!(
            "MJPEG camera started: {}x{} at {} FPS",
            format.width, format.height, self.config.fps
        );
        Ok(())
    }

    fn process(&mut self, _ctx: &CuContext, output: &mut Self::Output<'_>) -> CuResult<()> {
        output.clear_payload();

        if let Some(error) = self
            .capture_error
            .lock()
            .map_err(|_| CuError::from("MJPEG error lock poisoned"))?
            .as_ref()
        {
            return Err(CuError::from(error.clone()));
        }

        let frame = self
            .latest
            .lock()
            .map_err(|_| CuError::from("MJPEG frame lock poisoned"))?
            .take();
        let Some(frame) = frame else {
            return Ok(());
        };

        let mut image = CuImage::new(
            self.format
                .ok_or_else(|| CuError::from("MJPEG camera has not started"))?,
            CuHandle::new_detached(frame.bytes),
        );
        image.seq = frame.sequence;
        output.tov = Tov::Time(frame.captured_at);
        output.set_payload(image);
        Ok(())
    }

    fn stop(&mut self, _ctx: &CuContext) -> CuResult<()> {
        self.stop_worker()
    }
}

fn capture_frames(
    device: Device,
    config: CameraConfig,
    latest: Arc<Mutex<Option<CapturedFrame>>>,
    stop: Arc<AtomicBool>,
    clock: cu29::clock::RobotClock,
) -> Result<(), String> {
    let mut stream = MmapStream::with_buffers(&device, Type::VideoCapture, config.buffers)
        .map_err(|error| format!("failed to create MJPEG mmap stream: {error}"))?;
    stream.set_timeout(config.timeout);

    while !stop.load(Ordering::Relaxed) {
        let (bytes, metadata) = match stream.next() {
            Ok(frame) => frame,
            Err(error) => return Err(format!("failed to capture MJPEG frame: {error}")),
        };
        if metadata.bytesused == 0 || metadata.flags.contains(Flags::ERROR) {
            continue;
        }

        let length = metadata.bytesused as usize;
        if length > bytes.len() {
            return Err(format!(
                "MJPEG frame length {length} exceeds mmap buffer length {}",
                bytes.len()
            ));
        }

        let frame = CapturedFrame {
            bytes: bytes[..length].to_vec(),
            captured_at: clock.now(),
            sequence: metadata.sequence as u64,
        };
        *latest
            .lock()
            .map_err(|_| "MJPEG frame lock poisoned".to_string())? = Some(frame);
    }

    Ok(())
}

fn configure_camera(config: CameraConfig) -> Result<(Device, Format), String> {
    let device = Device::new(config.device)
        .map_err(|error| format!("failed to open V4L device {}: {error}", config.device))?;
    let requested_fourcc = FourCC::new(b"MJPG");
    let format = device
        .set_format(&Format::new(config.width, config.height, requested_fourcc))
        .map_err(|error| format!("failed to set MJPEG camera format: {error}"))?;
    if format.width != config.width
        || format.height != config.height
        || format.fourcc != requested_fourcc
    {
        return Err(format!(
            "camera negotiated {}x{} {} instead of {}x{} MJPG",
            format.width, format.height, format.fourcc, config.width, config.height
        ));
    }

    let parameters = device
        .set_params(&Parameters::with_fps(config.fps))
        .map_err(|error| format!("failed to set MJPEG camera frame rate: {error}"))?;
    if parameters.interval.numerator.saturating_mul(config.fps)
        != parameters.interval.denominator
    {
        return Err(format!(
            "camera negotiated frame interval {} instead of 1/{}",
            parameters.interval, config.fps
        ));
    }

    Ok((device, format))
}
