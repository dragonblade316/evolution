use cu29::config::{
    LogStreamContinuousFecConfig, LogStreamDestinationConfig, LogStreamFecConfig,
    LogStreamLinkConfig, LogStreamObjectFecConfig, LogStreamRepairDensity, LogStreamRlcField,
    LogStreamTransportConfig,
};
use cu29::logstream::capture::CaptureDataSet;
use cu29::logstream::{
    CuStreamTx, CuStreamTxError, LogStreamPlan, LogStreamSenderConfig, StreamIdentity,
};
use cu29::prelude::*;
use std::net::{SocketAddr, UdpSocket};

#[derive(Debug)]
pub struct UdpSender {
    socket: UdpSocket,
    target: SocketAddr,
}

impl UdpSender {
    pub fn new(address: &str) -> CuResult<Self> {
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

pub fn sender_config<P: CaptureDataSet>(
    identity: StreamIdentity,
    max_record_bytes: usize,
) -> CuResult<LogStreamSenderConfig> {
    let destination = LogStreamDestinationConfig {
        feedback: None,
        id: "twin".into(),
        transport: LogStreamTransportConfig {
            type_: "UdpSender".into(),
            resource: "udp".into(),
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
        max_record_bytes: max_record_bytes as u64,
    };
    let plan = LogStreamPlan::resolve(&destination)
        .map_err(|error| CuError::from(format!("invalid log stream configuration: {error}")))?;

    plan.sender_config(identity, P::stream_schema())
        .map_err(|error| CuError::from(format!("failed to configure log stream: {error}")))
}
