use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub struct NativeNetworkEvent {
    pub name: String,
    pub payload: Value,
}

#[derive(Clone, Default)]
pub struct NativeEventBus {
    events: Arc<Mutex<VecDeque<NativeNetworkEvent>>>,
}

impl NativeEventBus {
    pub fn publish(&self, name: impl Into<String>, payload: Value) {
        if let Ok(mut events) = self.events.lock() {
            events.push_back(NativeNetworkEvent {
                name: name.into(),
                payload,
            });
        }
    }

    pub fn drain(&self) -> Vec<NativeNetworkEvent> {
        let Ok(mut events) = self.events.lock() else {
            return Vec::new();
        };
        events.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::NativeEventBus;
    use serde_json::json;

    #[test]
    fn drains_events_in_publish_order() {
        let bus = NativeEventBus::default();
        bus.publish("peer_online", json!({ "deviceId": "AA-BB" }));
        bus.publish("message_received", json!({ "id": "message-1" }));

        let events = bus.drain();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, "peer_online");
        assert_eq!(events[1].payload["id"], "message-1");
        assert!(bus.drain().is_empty());
    }
}
