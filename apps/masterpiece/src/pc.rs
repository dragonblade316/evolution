mod messages;
mod tasks;

use cu29::logstream::{StreamIdentity, new_session_id};
use cu29::prelude::*;
use masterpiece::bridges;
use masterpiece::logstream::{UdpSender, sender_config};
use std::path::Path;

const PREALLOCATED_STORAGE_SIZE: Option<usize> = Some(1024 * 1024 * 100);
const APP_NAME: &str = env!("CARGO_PKG_NAME");

#[copper_runtime(config = "copperconfig.ron")]
struct MasterpieceApplication {}

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
    #[cfg(feature = "sim-debug")]
    let stream_config = sender_config::<masterpiece::MasterpiecePcDataSet>(
        identity,
        constants::TWIN_MAX_RECORD_BYTES,
    )?;
    #[cfg(not(feature = "sim-debug"))]
    let stream_config =
        sender_config::<pc::CuStampedDataSet>(identity, constants::TWIN_MAX_RECORD_BYTES)?;
    let application = pc::MasterpieceApplication::builder()
        .with_instance_id(identity.sender_id)
        .with_logstream(UdpSender::new(constants::TWIN_ADDRESS)?, stream_config)
        .with_log_path(&logger_path, PREALLOCATED_STORAGE_SIZE)?
        .build()?;

    debug!("Logger created at {}.", &logger_path);
    debug!(
        "Running PC mission with log stream at {}.",
        constants::TWIN_ADDRESS
    );
    application.run_until_shutdown()?;
    Ok(())
}
