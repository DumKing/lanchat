use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use uuid::Uuid;

const MAX_EVENTS_PER_INSTANCE: usize = 256;
const MAX_REQUESTS_PER_SECOND: u32 = 120;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PluginBounds {
    pub fn validate(self) -> Result<Self, String> {
        let values = [self.x, self.y, self.width, self.height];
        if values.iter().any(|value| !value.is_finite()) || self.width < 1.0 || self.height < 1.0 {
            return Err("插件视口尺寸无效".to_string());
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedPluginInstance {
    pub plugin_id: String,
    pub instance_id: String,
    pub instance_token: String,
    pub feature_code: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PluginInstanceRecord {
    pub plugin_id: String,
    pub version: String,
    pub instance_id: String,
    pub instance_token: String,
    pub webview_label: String,
    pub feature_code: String,
    pub capabilities: Vec<String>,
    pub crash_count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRuntimeEvent {
    pub sequence: u64,
    pub event: String,
    pub payload: Value,
}

struct RateWindow {
    started_at: Instant,
    count: u32,
}

struct PendingRequest {
    instance_token: String,
    sender: oneshot::Sender<Value>,
}

#[derive(Default)]
pub struct PluginRuntime {
    instances: Mutex<HashMap<String, PluginInstanceRecord>>,
    token_index: Mutex<HashMap<String, String>>,
    events: Mutex<HashMap<String, VecDeque<PluginRuntimeEvent>>>,
    next_event_sequence: Mutex<HashMap<String, u64>>,
    request_windows: Mutex<HashMap<String, RateWindow>>,
    pending_requests: Mutex<HashMap<String, PendingRequest>>,
}

impl PluginRuntime {
    pub fn prepare_instance(
        &self,
        plugin_id: &str,
        version: &str,
        feature_code: &str,
        capabilities: Vec<String>,
    ) -> Result<StartedPluginInstance, String> {
        if let Some(existing) = self.instance_for_plugin(plugin_id)? {
            return Ok(existing.into());
        }
        let instance_id = Uuid::new_v4().to_string();
        let instance_token = Uuid::new_v4().to_string();
        let record = PluginInstanceRecord {
            plugin_id: plugin_id.to_string(),
            version: version.to_string(),
            instance_id: instance_id.clone(),
            instance_token: instance_token.clone(),
            webview_label: format!("plugin-{}", instance_id),
            feature_code: feature_code.to_string(),
            capabilities,
            crash_count: 0,
        };
        self.instances
            .lock()
            .map_err(|_| "插件运行时实例锁已损坏".to_string())?
            .insert(instance_id.clone(), record.clone());
        self.token_index
            .lock()
            .map_err(|_| "插件运行时令牌锁已损坏".to_string())?
            .insert(instance_token, instance_id);
        Ok(record.into())
    }

    pub fn instance(&self, instance_id: &str) -> Result<Option<PluginInstanceRecord>, String> {
        Ok(self.instances.lock().map_err(|_| "插件运行时实例锁已损坏".to_string())?.get(instance_id).cloned())
    }

    pub fn instance_for_token(&self, token: &str) -> Result<Option<PluginInstanceRecord>, String> {
        let instance_id = self.token_index.lock().map_err(|_| "插件运行时令牌锁已损坏".to_string())?.get(token).cloned();
        instance_id.map(|value| self.instance(&value)).transpose().map(|value| value.flatten())
    }

    pub fn instance_for_plugin(&self, plugin_id: &str) -> Result<Option<PluginInstanceRecord>, String> {
        Ok(self.instances.lock().map_err(|_| "插件运行时实例锁已损坏".to_string())?.values().find(|item| item.plugin_id == plugin_id).cloned())
    }

    pub fn instance_for_plugin_webview(&self, webview_label: &str) -> Result<Option<PluginInstanceRecord>, String> {
        Ok(self.instances.lock().map_err(|_| "插件运行时实例锁已损坏".to_string())?.values()
            .find(|item| item.webview_label == webview_label).cloned())
    }

    pub fn remove_instance(&self, instance_id: &str) -> Result<Option<PluginInstanceRecord>, String> {
        let record = self.instances.lock().map_err(|_| "插件运行时实例锁已损坏".to_string())?.remove(instance_id);
        if let Some(record) = &record {
            self.token_index.lock().map_err(|_| "插件运行时令牌锁已损坏".to_string())?.remove(&record.instance_token);
            self.events.lock().map_err(|_| "插件事件队列锁已损坏".to_string())?.remove(instance_id);
            self.next_event_sequence.lock().map_err(|_| "插件事件序号锁已损坏".to_string())?.remove(instance_id);
            self.request_windows.lock().map_err(|_| "插件限流锁已损坏".to_string())?.remove(instance_id);
            self.cancel_pending_for_token(&record.instance_token)?;
        }
        Ok(record)
    }

    pub fn plugin_instance_ids(&self, plugin_id: &str) -> Result<Vec<String>, String> {
        Ok(self.instances.lock().map_err(|_| "插件运行时实例锁已损坏".to_string())?.values()
            .filter(|item| item.plugin_id == plugin_id).map(|item| item.instance_id.clone()).collect())
    }

    pub fn record_crash(&self, instance_id: &str) -> Result<u32, String> {
        let mut instances = self.instances.lock().map_err(|_| "插件运行时实例锁已损坏".to_string())?;
        let instance = instances.get_mut(instance_id).ok_or_else(|| "插件实例已经停止".to_string())?;
        instance.crash_count = instance.crash_count.saturating_add(1);
        Ok(instance.crash_count)
    }

    pub fn enqueue_event(&self, instance_id: &str, event: String, payload: Value) -> Result<u64, String> {
        if self.instance(instance_id)?.is_none() {
            return Err("插件实例已经停止".to_string());
        }
        let sequence = {
            let mut sequences = self.next_event_sequence.lock().map_err(|_| "插件事件序号锁已损坏".to_string())?;
            let next = sequences.entry(instance_id.to_string()).or_insert(0);
            *next += 1;
            *next
        };
        let mut events = self.events.lock().map_err(|_| "插件事件队列锁已损坏".to_string())?;
        let queue = events.entry(instance_id.to_string()).or_default();
        queue.push_back(PluginRuntimeEvent { sequence, event, payload });
        while queue.len() > MAX_EVENTS_PER_INSTANCE {
            queue.pop_front();
        }
        Ok(sequence)
    }

    pub fn poll_events(&self, token: &str, after_sequence: u64) -> Result<Vec<PluginRuntimeEvent>, String> {
        let instance = self.instance_for_token(token)?.ok_or_else(|| "插件实例已经失效".to_string())?;
        Ok(self.events.lock().map_err(|_| "插件事件队列锁已损坏".to_string())?
            .get(&instance.instance_id).map(|queue| queue.iter().filter(|item| item.sequence > after_sequence).cloned().collect()).unwrap_or_default())
    }

    pub fn begin_request(&self, token: &str, request_id: &str) -> Result<oneshot::Receiver<Value>, String> {
        let instance = self.instance_for_token(token)?.ok_or_else(|| "插件实例已经失效".to_string())?;
        let now = Instant::now();
        let mut windows = self.request_windows.lock().map_err(|_| "插件限流锁已损坏".to_string())?;
        let window = windows.entry(instance.instance_id).or_insert(RateWindow { started_at: now, count: 0 });
        if now.duration_since(window.started_at) >= Duration::from_secs(1) {
            window.started_at = now;
            window.count = 0;
        }
        window.count += 1;
        if window.count > MAX_REQUESTS_PER_SECOND {
            return Err("RATE_LIMITED".to_string());
        }
        drop(windows);
        let key = format!("{token}:{request_id}");
        let (sender, receiver) = oneshot::channel();
        let replaced = self.pending_requests.lock().map_err(|_| "插件请求锁已损坏".to_string())?
            .insert(key, PendingRequest { instance_token: token.to_string(), sender });
        if replaced.is_some() {
            return Err("插件请求 ID 重复".to_string());
        }
        Ok(receiver)
    }

    pub fn resolve_request(&self, token: &str, request_id: &str, response: Value) -> Result<(), String> {
        let key = format!("{token}:{request_id}");
        let pending = self.pending_requests.lock().map_err(|_| "插件请求锁已损坏".to_string())?.remove(&key)
            .ok_or_else(|| "插件请求不存在或已经超时".to_string())?;
        pending.sender.send(response).map_err(|_| "插件请求已经取消".to_string())
    }

    pub fn cancel_request(&self, token: &str, request_id: &str) {
        if let Ok(mut pending) = self.pending_requests.lock() {
            pending.remove(&format!("{token}:{request_id}"));
        }
    }

    fn cancel_pending_for_token(&self, token: &str) -> Result<(), String> {
        self.pending_requests.lock().map_err(|_| "插件请求锁已损坏".to_string())?.retain(|_, value| value.instance_token != token);
        Ok(())
    }
}

impl From<PluginInstanceRecord> for StartedPluginInstance {
    fn from(value: PluginInstanceRecord) -> Self {
        Self {
            plugin_id: value.plugin_id,
            instance_id: value.instance_id,
            instance_token: value.instance_token,
            feature_code: value.feature_code,
            capabilities: value.capabilities,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revoking_instance_invalidates_token_and_events() {
        let runtime = PluginRuntime::default();
        let instance = runtime.prepare_instance("com.lanchat.gomoku", "0.8.0", "gomoku", vec!["rooms.read".into()]).unwrap();
        runtime.enqueue_event(&instance.instance_id, "plugin.enter".into(), serde_json::json!({})).unwrap();
        assert_eq!(runtime.poll_events(&instance.instance_token, 0).unwrap().len(), 1);
        runtime.remove_instance(&instance.instance_id).unwrap();
        assert!(runtime.poll_events(&instance.instance_token, 0).is_err());
    }

    #[tokio::test]
    async fn matches_bridge_response_to_request() {
        let runtime = PluginRuntime::default();
        let instance = runtime.prepare_instance("com.lanchat.gomoku", "0.8.0", "gomoku", vec![]).unwrap();
        let receiver = runtime.begin_request(&instance.instance_token, "request-1").unwrap();
        runtime.resolve_request(&instance.instance_token, "request-1", serde_json::json!({"ok": true})).unwrap();
        assert_eq!(receiver.await.unwrap(), serde_json::json!({"ok": true}));
    }

    #[test]
    fn counts_plugin_crashes_per_instance() {
        let runtime = PluginRuntime::default();
        let instance = runtime.prepare_instance("com.lanchat.gomoku", "0.8.0", "gomoku", vec![]).unwrap();
        assert_eq!(runtime.record_crash(&instance.instance_id).unwrap(), 1);
        assert_eq!(runtime.record_crash(&instance.instance_id).unwrap(), 2);
    }
}
