use super::manifest::parse_and_validate_manifest;
use super::runtime::{PluginBounds, StartedPluginInstance};
use crate::AppState;
use serde_json::{json, Value};
use std::path::Path;
use tauri::{LogicalPosition, LogicalSize, Manager, State, WebviewBuilder, WebviewUrl};

fn bridge_bootstrap_script(instance: &StartedPluginInstance) -> String {
    let token = serde_json::to_string(&instance.instance_token).unwrap();
    let script = r#"
(() => {
  const token = __TOKEN__;
  const bridge = async (path, payload) => {
    const response = await fetch(`lanchat-plugin-bridge://localhost/${path}`, {
      method: 'POST', headers: { 'Content-Type': 'text/plain;charset=UTF-8' }, body: JSON.stringify(payload)
    });
    return response.json();
  };
  try { Object.defineProperty(window, '__TAURI_INTERNALS__', { value: undefined, configurable: false }); } catch {}
  try { Object.defineProperty(window, '__TAURI__', { value: undefined, configurable: false }); } catch {}
  let serial = 0;
  let lastSequence = 0;
  let stopped = false;
  const listeners = new Map();
  const deliver = (item) => {
    lastSequence = Math.max(lastSequence, item.sequence || 0);
    for (const callback of listeners.get(item.event) || []) {
      try { callback(item.payload); } catch (error) { console.error(error); }
    }
    if (item.event === 'plugin.out' && item.payload?.isTerminated) stopped = true;
  };
  window.addEventListener('__lanchat_host_event__', (event) => deliver(event.detail));
  const subscribe = (event, callback) => {
    const set = listeners.get(event) || new Set();
    set.add(callback); listeners.set(event, set);
    return () => { set.delete(callback); if (!set.size) listeners.delete(event); };
  };
  const request = async (method, params = {}) => {
    if (stopped) throw new Error('PLUGIN_STOPPED');
    const response = await bridge('request', {
      id: `${Date.now()}-${++serial}`, apiVersion: '1.0', instanceToken: token, method, params
    });
    if (!response || !response.ok) {
      const error = new Error(response?.error?.message || '插件调用失败');
      error.code = response?.error?.code || 'INTERNAL_ERROR';
      throw error;
    }
    return response.result;
  };
  const poll = async () => {
    while (!stopped) {
      try {
        const events = await bridge('events', { instanceToken: token, afterSequence: lastSequence });
        for (const item of events || []) deliver(item);
      } catch { stopped = true; break; }
      await new Promise((resolve) => setTimeout(resolve, 120));
    }
  };
  const api = {
    apiVersion: '1.0',
    app: { getVersion: () => request('app.getVersion'), getInstance: () => request('app.getInstance'), close: () => request('app.close') },
    events: {
      onPluginEnter: (cb) => subscribe('plugin.enter', cb), onPluginOut: (cb) => subscribe('plugin.out', cb),
      onThemeChanged: (cb) => subscribe('theme.changed', cb), onNetworkChanged: (cb) => subscribe('network.changed', cb),
      onRoomEvent: (cb) => subscribe('room.event', cb), onVisibilityChanged: (cb) => subscribe('visibility.changed', cb)
    },
    devices: { list: () => request('devices.list') },
    chat: { send: (input) => request('chat.send', input) },
    rooms: {
      list: (input = {}) => request('rooms.list', input), create: (input) => request('rooms.create', input),
      join: (input) => request('rooms.join', input), leave: (input) => request('rooms.leave', input),
      send: (input) => request('rooms.send', input), snapshot: (input) => request('rooms.snapshot', input)
    },
    leaderboard: { list: (input) => request('leaderboard.list', input), submit: (input) => request('leaderboard.submit', input) },
    storage: {
      get: (key) => request('storage.get', { key }), set: (key, value) => request('storage.set', { key, value }),
      delete: (key) => request('storage.delete', { key }), keys: () => request('storage.keys')
    },
    theme: { current: () => request('theme.current') },
    ui: {
      notify: (input) => request('ui.notify', input), confirm: (input) => request('ui.confirm', input),
      pickFile: (input = {}) => request('ui.pickFile', input)
    },
    logger: {}
  };
  for (const level of ['debug', 'info', 'warn', 'error']) api.logger[level] = (message, details) => request('logger.write', { level, message, details });
  Object.defineProperty(window, 'lanchat', { value: Object.freeze(api), configurable: false, writable: false });
  const reportCrash = (reason) => { void bridge('crash', { instanceToken: token, reason: String(reason || 'unknown').slice(0, 500) }); };
  window.addEventListener('error', (event) => reportCrash(event.error?.message || event.message));
  window.addEventListener('unhandledrejection', (event) => reportCrash(event.reason?.message || event.reason));
  void poll();
})();
"#;
    script.replace("__TOKEN__", &token)
}

fn plugin_url(plugin_id: &str, version: &str, entry: &str) -> Result<WebviewUrl, String> {
    let value = format!("lanchat-plugin://localhost/{plugin_id}/{version}/{entry}");
    Ok(WebviewUrl::External(value.parse().map_err(|error| format!("插件入口 URL 无效: {error}"))?))
}

#[tauri::command]
pub async fn start_plugin_instance(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
    feature_code: String,
    bounds: PluginBounds,
) -> Result<StartedPluginInstance, String> {
    let bounds = bounds.validate()?;
    let installation = state.plugin_storage.get(&plugin_id)?.ok_or_else(|| "插件尚未安装".to_string())?;
    if !installation.enabled {
        return Err("插件尚未启用".to_string());
    }
    let install_path = state.plugin_storage.version_install_path(&plugin_id, &installation.active_version)?
        .ok_or_else(|| "插件安装目录不存在".to_string())?;
    let bytes = std::fs::read(Path::new(&install_path).join("plugin.json")).map_err(|error| format!("读取插件清单失败: {error}"))?;
    let manifest = parse_and_validate_manifest(&bytes, env!("CARGO_PKG_VERSION"))?;
    if manifest.id != plugin_id || manifest.version != installation.active_version {
        return Err("插件清单与安装状态不一致".to_string());
    }
    let capabilities = installation.granted_capabilities.into_iter()
        .filter(|capability| manifest.capabilities.contains(capability)).collect::<Vec<_>>();
    let instance = state.plugin_runtime.prepare_instance(&plugin_id, &manifest.version, &feature_code, capabilities)?;
    let record = state.plugin_runtime.instance(&instance.instance_id)?.ok_or_else(|| "插件实例创建失败".to_string())?;
    if let Some(webview) = app.get_webview(&record.webview_label) {
        webview.set_position(LogicalPosition::new(bounds.x, bounds.y)).map_err(|error| error.to_string())?;
        webview.set_size(LogicalSize::new(bounds.width, bounds.height)).map_err(|error| error.to_string())?;
        webview.show().map_err(|error| error.to_string())?;
        return Ok(instance);
    }
    let main_window = app.get_window("main").ok_or_else(|| "主窗口不存在".to_string())?;
    let webview_data_dir = app.path().app_local_data_dir()
        .map_err(|error| format!("定位插件 WebView 数据目录失败: {error}"))?
        .join("plugin-webviews")
        .join(&plugin_id);
    let allowed_plugin_id = plugin_id.clone();
    let allowed_version = manifest.version.clone();
    let builder = WebviewBuilder::new(&record.webview_label, plugin_url(&plugin_id, &manifest.version, &manifest.entry)?)
        .initialization_script(bridge_bootstrap_script(&instance))
        .data_directory(webview_data_dir)
        .devtools(cfg!(debug_assertions))
        .on_navigation(move |url| {
            let path = url.path();
            (url.scheme() == "lanchat-plugin" || url.host_str() == Some("lanchat-plugin.localhost"))
                && path.contains(&allowed_plugin_id) && path.contains(&allowed_version)
        });
    if let Err(error) = main_window.add_child(
        builder,
        LogicalPosition::new(bounds.x, bounds.y),
        LogicalSize::new(bounds.width, bounds.height),
    ) {
        let _ = state.plugin_runtime.remove_instance(&instance.instance_id);
        return Err(format!("创建插件 WebView 失败: {error}"));
    }
    Ok(instance)
}

#[tauri::command]
pub fn set_plugin_instance_bounds(app: tauri::AppHandle, state: State<'_, AppState>, instance_id: String, bounds: PluginBounds) -> Result<(), String> {
    let bounds = bounds.validate()?;
    let record = state.plugin_runtime.instance(&instance_id)?.ok_or_else(|| "插件实例已经停止".to_string())?;
    let webview = app.get_webview(&record.webview_label).ok_or_else(|| "插件 WebView 不存在".to_string())?;
    webview.set_position(LogicalPosition::new(bounds.x, bounds.y)).map_err(|error| error.to_string())?;
    webview.set_size(LogicalSize::new(bounds.width, bounds.height)).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_plugin_instance_visible(app: tauri::AppHandle, state: State<'_, AppState>, instance_id: String, visible: bool) -> Result<(), String> {
    let record = state.plugin_runtime.instance(&instance_id)?.ok_or_else(|| "插件实例已经停止".to_string())?;
    let webview = app.get_webview(&record.webview_label).ok_or_else(|| "插件 WebView 不存在".to_string())?;
    if visible { webview.show() } else { webview.hide() }.map_err(|error| error.to_string())?;
    state.plugin_runtime.enqueue_event(&instance_id, "visibility.changed".into(), json!(visible))?;
    Ok(())
}

pub fn destroy_plugin_instance_inner(app: &tauri::AppHandle, state: &AppState, instance_id: &str, reason: &str) -> Result<(), String> {
    let Some(record) = state.plugin_runtime.instance(instance_id)? else { return Ok(()); };
    if let Some(webview) = app.get_webview(&record.webview_label) {
        let payload = serde_json::to_string(&json!({
            "sequence": 0,
            "event": "plugin.out",
            "payload": { "reason": reason, "isTerminated": true }
        })).unwrap_or_else(|_| "{}".to_string());
        let _ = webview.eval(format!("window.dispatchEvent(new CustomEvent('__lanchat_host_event__', {{ detail: {payload} }}));"));
        let _ = webview.close();
    }
    state.plugin_runtime.remove_instance(instance_id)?;
    Ok(())
}

pub fn destroy_plugin_instances(app: &tauri::AppHandle, state: &AppState, plugin_id: &str, reason: &str) -> Result<(), String> {
    for instance_id in state.plugin_runtime.plugin_instance_ids(plugin_id)? {
        destroy_plugin_instance_inner(app, state, &instance_id, reason)?;
    }
    Ok(())
}

#[tauri::command]
pub fn destroy_plugin_instance(app: tauri::AppHandle, state: State<'_, AppState>, instance_id: String, reason: String) -> Result<(), String> {
    destroy_plugin_instance_inner(&app, &state, &instance_id, &reason)
}

#[tauri::command]
pub fn emit_plugin_runtime_event(state: State<'_, AppState>, instance_id: String, event: String, payload: Value) -> Result<u64, String> {
    state.plugin_runtime.enqueue_event(&instance_id, event, payload)
}

#[tauri::command]
pub fn resolve_plugin_bridge_request(state: State<'_, AppState>, instance_token: String, request_id: String, response: Value) -> Result<(), String> {
    state.plugin_runtime.resolve_request(&instance_token, &request_id, response)
}

#[tauri::command]
pub fn plugin_private_storage_get(state: State<'_, AppState>, plugin_id: String, key: String) -> Result<Option<Value>, String> {
    state.plugin_storage.storage_get(&plugin_id, &key)
}

#[tauri::command]
pub fn plugin_private_storage_set(state: State<'_, AppState>, plugin_id: String, key: String, value: Value) -> Result<(), String> {
    state.plugin_storage.storage_set(&plugin_id, &key, &value, chrono::Utc::now().timestamp_millis())
}

#[tauri::command]
pub fn plugin_private_storage_delete(state: State<'_, AppState>, plugin_id: String, key: String) -> Result<(), String> {
    state.plugin_storage.storage_delete(&plugin_id, &key)
}

#[tauri::command]
pub fn plugin_private_storage_keys(state: State<'_, AppState>, plugin_id: String) -> Result<Vec<String>, String> {
    state.plugin_storage.storage_keys(&plugin_id)
}
