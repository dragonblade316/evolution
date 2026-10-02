use cu29::logstream::{CuStreamRx, CuStreamRxError};
use cu29::prelude::*;
use masterpiece::{MasterpieceRuntime, evo_visualizer};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug)]
struct UdpReceiver {
    socket: UdpSocket,
    receive_buffer: Vec<u8>,
}

impl UdpReceiver {
    fn bind(address: &str) -> CuResult<Self> {
        let socket = UdpSocket::bind(address)
            .map_err(|error| CuError::new_with_cause("failed to bind twin UDP receiver", error))?;
        socket.set_nonblocking(true).map_err(|error| {
            CuError::new_with_cause("failed to make twin UDP receiver nonblocking", error)
        })?;
        Ok(Self {
            socket,
            receive_buffer: vec![0; u16::MAX as usize + 1],
        })
    }
}

impl CuStreamRx for UdpReceiver {
    fn try_recv(
        &mut self,
        packet: &mut [u8],
    ) -> Result<Option<usize>, CuStreamRxError> {
        match self.socket.recv_from(&mut self.receive_buffer) {
            Ok((length, _)) if length > packet.len() => {
                Err(CuStreamRxError::BufferTooSmall { needed: length })
            }
            Ok((length, _)) => {
                packet[..length].copy_from_slice(&self.receive_buffer[..length]);
                Ok(Some(length))
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(_) => Err(CuStreamRxError::Failed("UDP receive failed")),
        }
    }
}

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

    let receiver = UdpReceiver::bind(&bind_address)?;
    let twin = MasterpieceRuntime::twin(receiver).with_log_path(archive);
    evo_visualizer()?.run_twin(twin, log_index.as_deref(), stop.as_ref())?;
    Ok(())
}
