use crate::AppState;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Emitter, Manager, UriSchemeContext, UriSchemeResponder};

const PLUGIN_CSP: &str = "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self' data:; connect-src lanchat-plugin-bridge: http://lanchat-plugin-bridge.localhost https://lanchat-plugin-bridge.localhost; media-src 'self' blob:; object-src 'none'; frame-src 'none'; child-src 'none'; base-uri 'none'; form-action 'none'";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ForwardedBridgeRequest {
    instance_id: String,
    instance_token: String,
    plugin_id: String,
    feature_code: String,
    capabilities: Vec<String>,
    request: Value,
}

pub fn plugin_protocol_response(context: UriSchemeContext<'_, tauri::Wry>, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    match read_plugin_asset(&context, request.uri().path()) {
        Ok((path, bytes)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type(&path))
            .header(header::CONTENT_SECURITY_POLICY, PLUGIN_CSP)
            .header("X-Content-Type-Options", "nosniff")
            .header("Cache-Control", "no-store")
            .body(bytes)
            .unwrap(),
        Err((status, message)) => Response::builder()
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .header(header::CONTENT_SECURITY_POLICY, PLUGIN_CSP)
            .body(message.into_bytes())
            .unwrap(),
    }
}

pub fn plugin_bridge_protocol(
    context: UriSchemeContext<'_, tauri::Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    let webview_label = context.webview_label().to_string();
    let path = request.uri().path().to_string();
    let body = request.body().clone();
    tauri::async_runtime::spawn(async move {
        let response = handle_bridge_request(app, &webview_label, &path, &body).await;
        responder.respond(response);
    });
}

async fn handle_bridge_request(
    app: tauri::AppHandle,
    webview_label: &str,
    path: &str,
    body: &[u8],
) -> Response<Vec<u8>> {
    let result = async {
        let state = app.try_state::<AppState>().ok_or_else(|| "插件运行时尚未就绪".to_string())?;
        let instance = state.plugin_runtime.instance_for_plugin_webview(webview_label)?
            .ok_or_else(|| "此 WebView 没有插件桥访问权".to_string())?;
        let payload: Value = serde_json::from_slice(body).map_err(|_| "插件桥请求格式无效".to_string())?;
        if path.ends_with("/crash") || path == "/crash" {
            let token = payload.get("instanceToken").and_then(Value::as_str).ok_or_else(|| "缺少实例令牌".to_string())?;
            if token != instance.instance_token { return Err("插件实例令牌无效".to_string()); }
            let crash_count = state.plugin_runtime.record_crash(&instance.instance_id)?;
            let reason = payload.get("reason").and_then(Value::as_str).unwrap_or("unknown");
            let _ = app.emit_to("main", "plugin-runtime-crashed", json!({
                "pluginId": instance.plugin_id,
                "instanceId": instance.instance_id,
                "crashCount": crash_count,
                "reason": reason,
            }));
            return Ok(json!({ "crashCount": crash_count }));
        }
        if path.ends_with("/events") || path == "/events" {
            let token = payload.get("instanceToken").and_then(Value::as_str).ok_or_else(|| "缺少实例令牌".to_string())?;
            if token != instance.instance_token { return Err("插件实例令牌无效".to_string()); }
            let after = payload.get("afterSequence").and_then(Value::as_u64).unwrap_or(0);
            return serde_json::to_value(state.plugin_runtime.poll_events(token, after)?).map_err(|error| error.to_string());
        }
        if !(path.ends_with("/request") || path == "/request") { return Err("未知插件桥路径".to_string()); }
        let request_id = payload.get("id").and_then(Value::as_str).ok_or_else(|| "插件请求缺少 ID".to_string())?.to_string();
        let token = payload.get("instanceToken").and_then(Value::as_str).ok_or_else(|| "插件请求缺少实例令牌".to_string())?.to_string();
        if token != instance.instance_token { return Err("插件实例令牌无效".to_string()); }
        let receiver = match state.plugin_runtime.begin_request(&token, &request_id) {
            Ok(value) => value,
            Err(error) if error == "RATE_LIMITED" => return Ok(json!({ "id": request_id, "ok": false, "error": { "code": "RATE_LIMITED", "message": "插件调用过于频繁，请稍后重试", "retryable": true } })),
            Err(error) => return Err(error),
        };
        let forwarded = ForwardedBridgeRequest {
            instance_id: instance.instance_id,
            instance_token: instance.instance_token.clone(),
            plugin_id: instance.plugin_id,
            feature_code: instance.feature_code,
            capabilities: instance.capabilities,
            request: payload,
        };
        app.emit_to("main", "plugin-bridge-requested", forwarded).map_err(|error| format!("转发插件请求失败: {error}"))?;
        match tokio::time::timeout(Duration::from_secs(10), receiver).await {
            Ok(Ok(response)) => Ok(response),
            _ => {
                state.plugin_runtime.cancel_request(&token, &request_id);
                Ok(json!({ "id": request_id, "ok": false, "error": { "code": "HOST_TIMEOUT", "message": "宿主处理插件请求超时", "retryable": true } }))
            }
        }
    }.await;
    bridge_json_response(match result {
        Ok(value) => value,
        Err(message) => json!({ "ok": false, "error": { "code": "BAD_REQUEST", "message": message, "retryable": false } }),
    })
}

fn bridge_json_response(value: Value) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json; charset=utf-8")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type")
        .header("Cache-Control", "no-store")
        .body(serde_json::to_vec(&value).unwrap_or_else(|_| b"{}".to_vec()))
        .unwrap()
}

fn read_plugin_asset(context: &UriSchemeContext<'_, tauri::Wry>, uri_path: &str) -> Result<(PathBuf, Vec<u8>), (StatusCode, String)> {
    let state = context.app_handle().try_state::<AppState>().ok_or((StatusCode::SERVICE_UNAVAILABLE, "插件运行时尚未就绪".to_string()))?;
    let instance = state.plugin_runtime.instance_for_plugin_webview(context.webview_label())
        .map_err(|message| (StatusCode::INTERNAL_SERVER_ERROR, message))?
        .ok_or((StatusCode::FORBIDDEN, "此 WebView 没有插件资源访问权".to_string()))?;
    let segments = safe_segments(uri_path).map_err(|message| (StatusCode::BAD_REQUEST, message))?;
    if segments.len() < 3 || segments[0] != instance.plugin_id || segments[1] != instance.version {
        return Err((StatusCode::FORBIDDEN, "插件资源路径与实例不匹配".to_string()));
    }
    let install_path = state.plugin_storage.version_install_path(&instance.plugin_id, &instance.version)
        .map_err(|message| (StatusCode::INTERNAL_SERVER_ERROR, message))?
        .ok_or((StatusCode::NOT_FOUND, "插件安装目录不存在".to_string()))?;
    let root = PathBuf::from(install_path);
    let relative = segments[2..].iter().fold(PathBuf::new(), |path, segment| path.join(segment));
    let path = root.join(relative);
    let canonical_root = root.canonicalize().map_err(|_| (StatusCode::NOT_FOUND, "插件安装目录不存在".to_string()))?;
    let canonical_path = path.canonicalize().map_err(|_| (StatusCode::NOT_FOUND, "插件资源不存在".to_string()))?;
    if !canonical_path.starts_with(&canonical_root) || !canonical_path.is_file() {
        return Err((StatusCode::FORBIDDEN, "插件资源越界".to_string()));
    }
    let bytes = std::fs::read(&canonical_path).map_err(|_| (StatusCode::NOT_FOUND, "插件资源读取失败".to_string()))?;
    Ok((canonical_path, bytes))
}

fn safe_segments(uri_path: &str) -> Result<Vec<String>, String> {
    let decoded = uri_path.trim_start_matches('/');
    let path = Path::new(decoded);
    if decoded.is_empty() || decoded.contains('\\') || path.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err("插件资源路径无效".to_string());
    }
    Ok(decoded.split('/').map(str::to_string).collect())
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase().as_str() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_and_maps_content_types() {
        assert!(safe_segments("/com.lanchat.test/0.8.0/dist/index.html").is_ok());
        assert!(safe_segments("/com.lanchat.test/0.8.0/../secret").is_err());
        assert_eq!(content_type(Path::new("bundle.js")), "text/javascript; charset=utf-8");
    }
}
