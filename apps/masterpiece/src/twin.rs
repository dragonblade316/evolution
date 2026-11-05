use cu29::logstream::SessionRouterLimits;
use cu29::prelude::*;
use cu29_logstream_udp::CuUdpLogStreamConfig;
use masterpiece::constants;
#[cfg(not(feature = "pc"))]
use masterpiece::{MasterpieceRuntime as TwinRuntime, evo_visualizer as twin_visualizer};
#[cfg(feature = "pc")]
use masterpiece::{MasterpiecePcRuntime as TwinRuntime, evo_pc_visualizer as twin_visualizer};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const RECEIVE_BUFFER_BYTES: usize = 4 * 1024 * 1024;

fn main() -> CuResult<()> {
    let mut args = std::env::args().skip(1);
    let bind_address = args.next().unwrap_or_else(|| "0.0.0.0:9000".into());
    let archive = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("logs/masterpiece_live.copper"));
    let log_index = args.next().map(PathBuf::from);

    let stop = Arc::new(AtomicBool::new(false));
    ctrlc::set_handler({
        let stop = Arc::clone(&stop);
        move || stop.store(true, Ordering::Relaxed)
    })
    .map_err(|error| CuError::new_with_cause("failed to install Ctrl-C handler", error))?;

    let bind_address: SocketAddr = bind_address.parse().map_err(|error| {
        CuError::new_with_cause("invalid twin UDP receiver address", error)
    })?;
    let mut socket = CuUdpLogStreamConfig::new(bind_address);
    socket.recv_buffer_bytes = Some(RECEIVE_BUFFER_BYTES);
    let (_, receiver) = socket.open()?;

    let mut receiver_limits = SessionRouterLimits::default();
    receiver_limits.max_record_bytes = constants::TWIN_MAX_RECORD_BYTES;
    receiver_limits.finite_objects.max_object_bytes = constants::TWIN_MAX_OBJECT_BYTES;
    let twin = TwinRuntime::twin(receiver)
        .with_log_path(archive)
        .with_receiver_limits(receiver_limits)
        .with_section_size(constants::TWIN_MAX_RECORD_BYTES + 1024);
    twin_visualizer()?.run_twin(twin, log_index.as_deref(), stop.as_ref())?;
    Ok(())
}
