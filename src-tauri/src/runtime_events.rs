use crate::debug_log::DebugLogEntry;
use crate::native_app::NativeEventBus;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone)]
pub enum NetworkEventSink {
    Tauri(AppHandle),
    Native(NativeEventBus),
}

impl From<AppHandle> for NetworkEventSink {
    fn from(value: AppHandle) -> Self {
        Self::Tauri(value)
    }
}

impl NetworkEventSink {
    pub fn native(events: NativeEventBus) -> Self {
        Self::Native(events)
    }

    pub fn emit<T: Serialize + Clone>(&self, name: &str, payload: T) -> Result<(), ()> {
        match self {
            Self::Tauri(app) => app.emit(name, payload).map_err(|_| ()),
            Self::Native(events) => {
                events.publish(name, serde_json::to_value(payload).map_err(|_| ())?);
                Ok(())
            }
        }
    }

    pub fn request_main_window(&self) {
        if let Self::Tauri(app) = self {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_skip_taskbar(false);
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
    }
}

pub trait DebugLogSink {
    fn emit_debug_log_entry(&self, entry: DebugLogEntry);
}

impl DebugLogSink for AppHandle {
    fn emit_debug_log_entry(&self, entry: DebugLogEntry) {
        self.emit("debug_log", entry).ok();
    }
}

impl DebugLogSink for NetworkEventSink {
    fn emit_debug_log_entry(&self, entry: DebugLogEntry) {
        self.emit("debug_log", entry).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::NetworkEventSink;
    use crate::native_app::NativeEventBus;
    use serde_json::json;

    #[test]
    fn native_sink_forwards_serialized_events_to_the_bus() {
        let bus = NativeEventBus::default();
        NetworkEventSink::native(bus.clone())
            .emit("message_received", json!({ "id": "message-1" }))
            .expect("event published");

        let events = bus.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "message_received");
        assert_eq!(events[0].payload["id"], "message-1");
    }
}
