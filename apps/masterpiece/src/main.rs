mod messages;
mod tasks;

use cu29::prelude::*;
use masterpiece::bridges;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

const LOG_SLAB_SIZE: usize = 128 * 1024 * 1024;
const LOG_CAPACITY: usize = 20 * 1024 * 1024 * 1024;
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
    let logger = UnifiedLoggerBuilder::new()
        .write(true)
        .create(true)
        .file_base_name(&logger_path)
        .preallocated_size(LOG_SLAB_SIZE)
        .rollover(LOG_CAPACITY)
        .build()
        .expect("Failed to create bounded unified logger.");
    let UnifiedLogger::Write(logger) = logger else {
        panic!("Unified logger is not write-capable.");
    };

    debug!("Logger created at {}.", &logger_path);
    debug!("Creating application... ");
    let mut application = MasterpieceApplication::builder()
        .with_instance_id(0)
        .with_unified_logger(Arc::new(Mutex::new(logger)))
        .build()
        .expect("Failed to create application.");
    debug!("Streaming to twin at {}.", constants::TWIN_ADDRESS);
    debug!("Running... starting clock: {}.", application.clock().now());

    application.run().expect("Failed to run application.");
    debug!("End of program: {}.", application.clock().now());
    sleep(Duration::from_secs(1));
}
