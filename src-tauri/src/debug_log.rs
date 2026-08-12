use crate::runtime_events::DebugLogSink;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DebugLogEntry {
    pub ts: i64,
    pub level: String,
    pub scope: String,
    pub message: String,
    pub detail: Option<String>,
}

pub fn emit_debug_log<T: DebugLogSink>(
    app: &T,
    level: impl Into<String>,
    scope: impl Into<String>,
    message: impl Into<String>,
    detail: Option<String>,
) {
    let entry = DebugLogEntry {
        ts: chrono::Utc::now().timestamp_millis(),
        level: level.into(),
        scope: scope.into(),
        message: message.into(),
        detail,
    };
    app.emit_debug_log_entry(entry);
}
