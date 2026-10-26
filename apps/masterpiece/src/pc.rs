mod messages;
mod tasks;

use cu29::config::{
    LogStreamContinuousFecConfig, LogStreamDestinationConfig, LogStreamFecConfig,
    LogStreamLinkConfig, LogStreamObjectFecConfig, LogStreamRepairDensity, LogStreamRlcField,
    LogStreamTransportConfig,
};
use cu29::logstream::capture::CaptureDataSet;
use cu29::logstream::{
    CuStreamTx, CuStreamTxError, LogStreamPlan, StreamIdentity, new_session_id,
};
use cu29::prelude::*;
use masterpiece::bridges;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path;

const PREALLOCATED_STORAGE_SIZE: Option<usize> = Some(1024 * 1024 * 100);
const APP_NAME: &str = env!("CARGO_PKG_NAME");
const TWIN_ADDRESS: &str = "127.0.0.1:9000";

#[copper_runtime(config = "copperconfig.ron")]
struct MasterpieceApplication {}

#[derive(Debug)]
struct UdpSender {
    socket: UdpSocket,
    target: SocketAddr,
}

impl UdpSender {
    fn new(address: &str) -> CuResult<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|error| CuError::new_with_cause("failed to bind log stream sender", error))?;
        socket.set_nonblocking(true).map_err(|error| {
            CuError::new_with_cause("failed to make log stream sender nonblocking", error)
        })?;
        let target = address.parse().map_err(|error| {
            CuError::new_with_cause("invalid log stream destination address", error)
        })?;
        Ok(Self { socket, target })
    }
}

impl CuStreamTx for UdpSender {
    fn try_send(&mut self, packet: &[u8]) -> Result<(), CuStreamTxError> {
        match self.socket.send_to(packet, self.target) {
            Ok(length) if length == packet.len() => Ok(()),
            Ok(_) => Err(CuStreamTxError::Failed("partial UDP send")),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err(CuStreamTxError::WouldBlock)
            }
            Err(_) => Err(CuStreamTxError::Failed("UDP send failed")),
        }
    }
}

fn log_stream_config(identity: StreamIdentity) -> CuResult<cu29::logstream::LogStreamSenderConfig> {
    let destination = LogStreamDestinationConfig {
        feedback: None,
        id: "local_twin".into(),
        transport: LogStreamTransportConfig {
            type_: "UdpSender".into(),
            resource: "local_udp".into(),
        },
        link: LogStreamLinkConfig {
            mtu_bytes: 1200,
            bitrate_bps: 1_000_000_000,
            memory_budget_kib: 64 * 1024,
            max_latency_ms: 250,
            burst_packets: 512,
        },
        fec: LogStreamFecConfig {
            continuous: LogStreamContinuousFecConfig {
                field: LogStreamRlcField::Gf256,
                window_symbols: 64,
                repair_every_source_symbols: 4,
                repair_density: LogStreamRepairDensity::Full,
            },
            objects: LogStreamObjectFecConfig {
                max_object_bytes: 64 * 1024,
                repair_symbols_per_block: 8,
            },
        },
        recovery_interval: 100,
        max_record_bytes: 4 * 1024,
    };
    let plan = LogStreamPlan::resolve(&destination)
        .map_err(|error| CuError::from(format!("invalid log stream configuration: {error}")))?;
    #[cfg(feature = "sim-debug")]
    let schema = <masterpiece::MasterpiecePcDataSet as CaptureDataSet>::stream_schema();
    #[cfg(not(feature = "sim-debug"))]
    let schema = <pc::CuStampedDataSet as CaptureDataSet>::stream_schema();

    plan.sender_config(identity, schema)
        .map_err(|error| CuError::from(format!("failed to configure log stream: {error}")))
}

fn main() -> CuResult<()> {
    let logger_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("logs")
        .join(format!("{APP_NAME}_pc.copper"));
    if let Some(parent) = logger_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| CuError::new_with_cause("failed to create logs directory", error))?;
    }

    let identity = StreamIdentity {
        session_id: new_session_id(),
        sender_id: 0,
    };
    let application = pc::MasterpieceApplication::builder()
        .with_instance_id(identity.sender_id)
        .with_logstream(
            UdpSender::new(TWIN_ADDRESS)?,
            log_stream_config(identity)?,
        )
        .with_log_path(&logger_path, PREALLOCATED_STORAGE_SIZE)?
        .build()?;

    debug!("Logger created at {}.", &logger_path);
    debug!("Running PC mission with log stream at {TWIN_ADDRESS}.");
    application.run_until_shutdown()?;
    Ok(())
}
