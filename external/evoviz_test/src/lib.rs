mod log_file;

pub use log_file::from_log;

use cu29::{
    bevy_reflect::{PartialReflect, ReflectRef},
    prelude::{
        CopperList, CopperListTuple, CuError, CuLogEntry, CuLogLevel, CuPayloadRawBytes, CuResult,
        ErasedCuStampedDataSet, rebuild_logline,
    },
};
use rerun::{RecordingStream, RecordingStreamBuilder, Scalars, TextLog, TextLogLevel};

pub struct Visualizer {
    stream: RecordingStream,
}

impl Visualizer {
    pub fn spawn() -> CuResult<Self> {
        let stream = RecordingStreamBuilder::new("evoviz")
            .spawn()
            .map_err(|error| CuError::new_with_cause("failed to spawn visualizer", error))?;
        Ok(Self { stream })
    }

    pub fn log_copperlist<P>(
        &self,
        entry: &CopperList<P>,
        task_path: &str,
    ) -> CuResult<()>
    where
        P: CopperListTuple + CuPayloadRawBytes + 'static,
    {
        self.stream
            .set_time_sequence("copperlist", entry.id as i64);

        for (task_id, message) in P::get_all_task_ids().iter().zip(entry.cumsgs()) {
            if let Some(payload) = message.payload_reflect() {
                let path = join_path(task_path, task_id);
                log_reflected(&self.stream, &path, payload.as_partial_reflect())?;
            }
        }

        Ok(())
    }

    pub fn log_text(
        &self,
        entry: &CuLogEntry,
        task_path: &str,
        interned_strings: &[String],
    ) -> CuResult<()> {
        let line = rebuild_logline(interned_strings, entry)
            .unwrap_or_else(|_| fallback_logline(interned_strings, entry));
        self.stream.set_time(
            "copper_time",
            std::time::Duration::from_nanos(entry.time.as_nanos()),
        );
        self.stream
            .log(
                task_path,
                &TextLog::new(line).with_level(rerun_log_level(entry.level)),
            )
            .map_err(|error| CuError::new_with_cause("failed to log structured text", error))
    }
}

fn log_reflected(
    stream: &RecordingStream,
    path: &str,
    value: &dyn PartialReflect,
) -> CuResult<()> {
    match value.reflect_ref() {
        ReflectRef::Struct(value) => {
            for index in 0..value.field_len() {
                if let Some(field) = value.field_at(index) {
                    let name = value.name_at(index).unwrap_or("unknown");
                    log_reflected(stream, &join_path(path, name), field)?;
                }
            }
        }
        ReflectRef::TupleStruct(value) => {
            for index in 0..value.field_len() {
                if let Some(field) = value.field(index) {
                    log_reflected(stream, &join_path(path, &index.to_string()), field)?;
                }
            }
        }
        ReflectRef::Tuple(value) => {
            for index in 0..value.field_len() {
                if let Some(field) = value.field(index) {
                    log_reflected(stream, &join_path(path, &index.to_string()), field)?;
                }
            }
        }
        ReflectRef::List(value) => {
            for (index, item) in value.iter().enumerate() {
                log_reflected(stream, &join_path(path, &index.to_string()), item)?;
            }
        }
        ReflectRef::Array(value) => {
            for (index, item) in value.iter().enumerate() {
                log_reflected(stream, &join_path(path, &index.to_string()), item)?;
            }
        }
        ReflectRef::Map(value) => {
            for (key, item) in value.iter() {
                log_reflected(stream, &join_path(path, &reflect_key(key)), item)?;
            }
        }
        ReflectRef::Set(value) => {
            for (index, item) in value.iter().enumerate() {
                log_reflected(stream, &join_path(path, &index.to_string()), item)?;
            }
        }
        ReflectRef::Enum(value) => {
            let path = join_path(path, value.variant_name());
            for index in 0..value.field_len() {
                if let Some(field) = value.field_at(index) {
                    let name = value
                        .name_at(index)
                        .map(str::to_owned)
                        .unwrap_or_else(|| index.to_string());
                    log_reflected(stream, &join_path(&path, &name), field)?;
                }
            }
        }
        ReflectRef::Opaque(_) => {
            if let Some(scalar) = reflected_scalar(value) {
                stream
                    .log(path, &Scalars::new([scalar]))
                    .map_err(|error| CuError::new_with_cause("failed to log scalar", error))?;
            }
        }
    }

    Ok(())
}

fn reflected_scalar(value: &dyn PartialReflect) -> Option<f64> {
    macro_rules! scalar {
        ($type:ty) => {
            if let Some(value) = value.try_downcast_ref::<$type>() {
                return Some(*value as f64);
            }
        };
    }

    if let Some(value) = value.try_downcast_ref::<bool>() {
        return Some(u8::from(*value) as f64);
    }

    scalar!(u8);
    scalar!(u16);
    scalar!(u32);
    scalar!(u64);
    scalar!(u128);
    scalar!(usize);
    scalar!(i8);
    scalar!(i16);
    scalar!(i32);
    scalar!(i64);
    scalar!(i128);
    scalar!(isize);
    scalar!(f32);
    scalar!(f64);

    None
}

fn reflect_key(value: &dyn PartialReflect) -> String {
    if let Some(value) = value.try_downcast_ref::<String>() {
        return value.clone();
    }
    if let Some(value) = value.try_downcast_ref::<char>() {
        return value.to_string();
    }

    macro_rules! key {
        ($type:ty) => {
            if let Some(value) = value.try_downcast_ref::<$type>() {
                return value.to_string();
            }
        };
    }

    key!(bool);
    key!(u8);
    key!(u16);
    key!(u32);
    key!(u64);
    key!(u128);
    key!(usize);
    key!(i8);
    key!(i16);
    key!(i32);
    key!(i64);
    key!(i128);
    key!(isize);

    format!("{value:?}")
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}/{child}")
    }
}

fn fallback_logline(interned_strings: &[String], entry: &CuLogEntry) -> String {
    let message = interned_strings
        .get(entry.msg_index as usize)
        .map(String::as_str)
        .unwrap_or("<missing log message>");
    let params = entry
        .params
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");

    format!("{} [{:?}]: {message} | {params}", entry.time, entry.level)
}

fn rerun_log_level(level: CuLogLevel) -> &'static str {
    match level {
        CuLogLevel::Debug => TextLogLevel::DEBUG,
        CuLogLevel::Info => TextLogLevel::INFO,
        CuLogLevel::Warning => TextLogLevel::WARN,
        CuLogLevel::Error => TextLogLevel::ERROR,
        CuLogLevel::Critical => TextLogLevel::CRITICAL,
    }
}
