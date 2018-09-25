use log;
use log::{Level, Log, Metadata, Record};

use EditorConnection;

/// A `Log` implementation that sends all incoming logs to the editor, which may allow more
/// interactive filtering.
pub struct EditorLogger {
    editor_connection: EditorConnection,
}

impl EditorLogger {
    /// Construct a logger that sends logs to the given editor.
    pub fn new(editor_connection: EditorConnection) -> Self {
        Self { editor_connection }
    }

    /// Start this logger if no current logger is set.
    pub fn start(self) {
        log::set_max_level(log::LevelFilter::max());
        log::set_boxed_logger(Box::new(self))
            .unwrap_or_else(|_| warn!("Logger already set. The editor will not receive any logs."));
    }
}

impl Log for EditorLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        self.editor_connection
            .send_message("log", SerializableLogRecord::from(record));
    }

    fn flush(&self) {}
}

#[derive(Debug, Serialize)]
enum SerializableLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<Level> for SerializableLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Error => SerializableLevel::Error,
            Level::Warn => SerializableLevel::Warn,
            Level::Info => SerializableLevel::Info,
            Level::Debug => SerializableLevel::Debug,
            Level::Trace => SerializableLevel::Trace,
        }
    }
}

#[derive(Debug, Serialize)]
struct SerializableLogRecord {
    level: SerializableLevel,
    target: String,
    module: Option<String>,
    file: Option<String>,
    line: Option<u32>,
    message: String,
}

impl<'a> From<&'a Record<'a>> for SerializableLogRecord {
    fn from(record: &Record) -> Self {
        Self {
            level: record.level().into(),
            target: record.target().to_owned(),
            module: record.module_path().map(|s| s.to_owned()),
            file: record.file().map(|s| s.to_owned()),
            line: record.line(),
            message: format!("{}", record.args()),
        }
    }
}
