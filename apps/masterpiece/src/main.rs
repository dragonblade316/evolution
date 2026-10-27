mod messages;
mod tasks;

use cu29::logstream::{StreamIdentity, new_session_id};
use cu29::prelude::*;
use masterpiece::bridges;
use masterpiece::logstream::{UdpSender, sender_config};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

const PREALLOCATED_STORAGE_SIZE: Option<usize> = Some(1024 * 1024 * 100);
const APP_NAME: &str = env!("CARGO_PKG_NAME");

#[copper_runtime(config = "copperconfig.ron")]
struct MasterpieceApplication {}

fn main() {
    let logger_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("logs")
        .join(format!("{APP_NAME}.copper"));
    if let Some(parent) = logger_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).expect("Failed to create logs directory");
        }
    }
    debug!("Logger created at {}.", &logger_path);
    debug!("Creating application... ");
    let identity = StreamIdentity {
        session_id: new_session_id(),
        sender_id: 0,
    };
    let mut application = MasterpieceApplication::builder()
        .with_instance_id(identity.sender_id)
        .with_logstream(
            UdpSender::new(constants::TWIN_ADDRESS).expect("Failed to create twin UDP sender."),
            sender_config::<default::CuStampedDataSet>(
                identity,
                constants::TWIN_MAX_RECORD_BYTES,
            )
            .expect("Failed to configure twin log stream."),
        )
        .with_log_path(&logger_path, PREALLOCATED_STORAGE_SIZE)
        .expect("Failed to setup logger.")
        .build()
        .expect("Failed to create application.");
    debug!("Streaming to twin at {}.", constants::TWIN_ADDRESS);
    debug!("Running... starting clock: {}.", application.clock().now());

    application.run().expect("Failed to run application.");
    debug!("End of program: {}.", application.clock().now());
    sleep(Duration::from_secs(1));
}
