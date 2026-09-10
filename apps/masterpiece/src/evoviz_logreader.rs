mod messages;
mod tasks;

use cu29::prelude::*;
use evoviz_test::from_log;
use std::path::Path;

gen_cumsgs!("copperconfig.ron");

fn main() {
    let log_base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("logs/cu-test.copper");
    let log_index = Path::new(env!("LOG_INDEX_DIR"))
        .ancestors()
        .nth(3)
        .expect("invalid Cargo output directory")
        .join("cu29_log_index");

    let _visualizer = from_log::<CuStampedDataSet>(&log_base, &log_index, "masterpiece")
        .expect("failed to visualize Copper log");
}
