use crate::Visualizer;
use cu29::prelude::{
    CopperListTuple, CuError, CuPayloadRawBytes, CuResult, UnifiedLogType, UnifiedLogger,
    UnifiedLoggerBuilder, UnifiedLoggerIOReader,
};
use cu29_export::{copperlists_reader, structlog_reader};
use std::path::Path;

pub fn from_log<P>(
    log_base: impl AsRef<Path>,
    log_index: impl AsRef<Path>,
    task_path: &str,
) -> CuResult<Visualizer>
where
    P: CopperListTuple + CuPayloadRawBytes + 'static,
{
    let log_base = log_base.as_ref();
    cu29::logcodec::seed_effective_config_from_log::<P>(log_base)?;

    let interned_strings = cu29_intern_strs::read_interned_strings(log_index.as_ref()).map_err(
        |error| {
            CuError::new_with_cause(
                "failed to read structured log string index",
                std::io::Error::other(error.to_string()),
            )
        },
    )?;
    let visualizer = Visualizer::spawn()?;

    let copperlist_reader = open_reader(log_base, UnifiedLogType::CopperList)?;
    for entry in copperlists_reader::<P>(copperlist_reader) {
        visualizer.log_copperlist(&entry, task_path)?;
    }

    let text_path = format!("{}/logs", task_path.trim_end_matches('/'));
    let structured_log_reader = open_reader(log_base, UnifiedLogType::StructuredLogLine)?;
    for entry in structlog_reader(structured_log_reader) {
        visualizer.log_text(&entry?, &text_path, &interned_strings)?;
    }

    Ok(visualizer)
}

fn open_reader(log_base: &Path, log_type: UnifiedLogType) -> CuResult<UnifiedLoggerIOReader> {
    let logger = UnifiedLoggerBuilder::new()
        .file_base_name(log_base)
        .build()
        .map_err(|error| CuError::new_with_cause("failed to open Copper log", error))?;

    match logger {
        UnifiedLogger::Read(logger) => Ok(UnifiedLoggerIOReader::new(logger, log_type)),
        UnifiedLogger::Write(_) => Err(CuError::from("expected a read-only Copper log")),
    }
}
