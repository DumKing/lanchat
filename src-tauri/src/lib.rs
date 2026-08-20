mod channel_crypto;
mod debug_log;
mod desktop_pet;
mod desktop_pet_runtime;
mod face_monitor;
mod file_server;
mod identity;
mod network;
mod protocol;
mod storage;
mod vision;

#[cfg(test)]
mod desktop_pet_tests;

use channel_crypto::{generate_channel_key, CHANNEL_KEY_VERSION};
use chrono::TimeZone;
use debug_log::emit_debug_log;
use desktop_pet::{
    DesktopPetManager, DesktopPetPackage, DesktopPetRegistrySnapshot, DesktopPetSettings,
    ExternalPushConfig, PetPackageSource, PetResourceRoot, PetStatePlaybackConfig,
};
use desktop_pet_runtime::{DesktopPetController, DesktopPetRuntimeState};
use face_monitor::{
    dynamic_embedding_bytes, dynamic_embedding_from_bytes, embedding_bytes, embedding_from_bytes,
    FaceMatch, FaceMonitorLocalSettings, FaceMonitorRuntime, FaceMonitorStatus, PersonTemplate,
    PERSON_REID_DIM,
};
use file_server::FileServer;
use fs2::FileExt;
use network::{local_ip_address, Network};
use protocol::MessageRecallFrame;
use protocol::{
    AdminAlertModeFrame, AdminAlertPushPolicyFrame, AdminDiscoModeFrame,
    AdminNotificationDecisionFrame, AdminNotificationFrame, AdminNotificationSubmissionFrame,
    AdminRemoteUpdateFrame, QuickAlertFeedbackFrame, QuickAlertFrame, QuickAlertTrustResetFrame,
    SimulationMeta,
};
use protocol::{CallSignalFrame, GameFrame};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use storage::{
    AdminNotificationRecord, CameraFaceAlertRecord, ChannelMember, ChannelMemberSeed, Conversation,
    FaceMonitorPolicyRecord, FacePersonRecord, FacePersonSampleRecord, Message, MessageType, Peer,
    Profile, SimulationAudit, Storage, DEFAULT_GROUP_ID,
};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    Emitter, LogicalSize, Manager, PhysicalPosition, Position, Size, State, UserAttentionType,
    WindowEvent,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use vision::worker::{decode_raw_frame, encode_frame_as_jpeg, LatestFrameMailbox, VisionWorker};
use vision::{
    model_manager::{download_and_install, fetch_official_catalog, VisionCatalogProfile},
    runtime::VisionRuntimeState,
    types::{VisionModelProfileSummary, VisionRuntimeDiagnostics, VisionRuntimeSnapshot},
};

#[cfg(target_os = "windows")]
use winreg::enums::HKEY_CURRENT_USER;
#[cfg(target_os = "windows")]
use winreg::RegKey;

const UPDATE_REPOSITORY: &str = "DumKing/lanchat";
const UPDATE_API_URL: &str = "https://api.github.com/repos/DumKing/lanchat/releases/latest";
const UPDATE_METADATA_ASSET: &str = "lanchat-update.json";
const UPDATE_GITHUB_TOKEN_SERVICE: &str = "com.lanchat.desktop.update";
const UPDATE_GITHUB_TOKEN_ACCOUNT: &str = "github-token";
const LOCAL_BUILD_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("LANCHAT_BUILD_TIMESTAMP")
);

const PREVIEW_MEDIA_CACHE_MAX_BYTES: usize = 30 * 1024 * 1024;
const PREVIEW_MEDIA_CACHE_TOTAL_LIMIT_BYTES: u64 = 300 * 1024 * 1024;
const SUPER_ADMIN_PASSWORD_MD5: &str = "D7B9AF919901FA1598BDC21465E3EB3F";
static MANAGED_UPDATE_PROXY_ENV: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn normalize_update_proxy(value: &str) -> Option<String> {
    let raw = value.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("direct") {
        return None;
    }
    let mut fallback = None;
    for item in raw
        .split(';')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (scheme, endpoint) = item
            .split_once('=')
            .map(|(scheme, endpoint)| (Some(scheme.trim().to_ascii_lowercase()), endpoint.trim()))
            .unwrap_or((None, item));
        if endpoint.is_empty() {
            continue;
        }
        let normalized = if endpoint.contains("://") {
            endpoint.to_string()
        } else {
            format!("http://{endpoint}")
        };
        match scheme.as_deref() {
            Some("https") => return Some(normalized),
            Some("http") | None => fallback.get_or_insert(normalized),
            _ => fallback.get_or_insert(normalized),
        };
    }
    fallback
}

fn environment_update_proxy() -> Option<String> {
    ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok())
        .and_then(|value| normalize_update_proxy(&value))
}

#[cfg(target_os = "windows")]
fn windows_system_update_proxy() -> Option<String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .ok()?;
    let enabled = key.get_value::<u32, _>("ProxyEnable").unwrap_or(0);
    if enabled == 0 {
        return None;
    }
    key.get_value::<String, _>("ProxyServer")
        .ok()
        .and_then(|value| normalize_update_proxy(&value))
}

#[cfg(not(target_os = "windows"))]
fn windows_system_update_proxy() -> Option<String> {
    None
}

fn current_update_proxy() -> Option<String> {
    windows_system_update_proxy().or_else(environment_update_proxy)
}

fn configure_update_http_client(client: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    let client = client.tcp_nodelay(true).pool_max_idle_per_host(8);
    match current_update_proxy().and_then(|proxy| reqwest::Proxy::all(proxy).ok()) {
        Some(proxy) => client.proxy(proxy),
        None => client,
    }
}

fn update_http_client() -> reqwest::Client {
    configure_update_http_client(reqwest::Client::builder())
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

fn update_github_token_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(UPDATE_GITHUB_TOKEN_SERVICE, UPDATE_GITHUB_TOKEN_ACCOUNT)
        .map_err(|error| format!("访问系统凭据库失败：{error}"))
}

fn read_update_github_token() -> Option<String> {
    let token = update_github_token_entry().ok()?.get_password().ok()?;
    let token = token.trim().to_string();
    (!token.is_empty()).then_some(token)
}

fn authorized_update_request(client: &reqwest::Client, url: &str) -> reqwest::RequestBuilder {
    let request = client.get(url).header("User-Agent", "LanChat");
    match read_update_github_token() {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateGithubTokenInfo {
    configured: bool,
    masked_value: Option<String>,
}

fn update_github_token_info() -> UpdateGithubTokenInfo {
    let token = read_update_github_token();
    let masked_value = token.as_ref().map(|value| {
        let suffix = value
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        format!("已配置（末四位：{suffix}）")
    });
    UpdateGithubTokenInfo {
        configured: token.is_some(),
        masked_value,
    }
}

/// Tauri updater creates a new reqwest client for every check/download. Refreshing
/// these process-local variables immediately before that work lets it follow a
/// proxy changed while LanChat is already running.
#[tauri::command]
fn refresh_update_proxy() -> Result<(), String> {
    let managed = MANAGED_UPDATE_PROXY_ENV.get_or_init(|| Mutex::new(None));
    let mut managed = managed
        .lock()
        .map_err(|_| "更新代理配置状态不可用".to_string())?;
    let detected = windows_system_update_proxy();

    match detected {
        Some(proxy) => {
            std::env::set_var("HTTPS_PROXY", &proxy);
            std::env::set_var("https_proxy", &proxy);
            std::env::set_var("HTTP_PROXY", &proxy);
            std::env::set_var("http_proxy", &proxy);
            *managed = Some(proxy);
        }
        None => {
            if let Some(previous) = managed.take() {
                for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
                    if std::env::var(&key).ok().as_deref() == Some(previous.as_str()) {
                        std::env::remove_var(key);
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewMediaCacheInfo {
    directory: String,
    file_count: u64,
    total_bytes: u64,
}

#[cfg(test)]
mod preview_media_cache_tests {
    use super::*;

    #[test]
    fn preview_cache_filename_uses_only_a_safe_image_extension() {
        assert_eq!(
            preview_cache_file_name("message-1", "photo.JPG"),
            "message-1.jpg"
        );
        assert_eq!(
            preview_cache_file_name("message:2", "unknown.exe"),
            "message_2.bin"
        );
    }
}

static LANCHAT_INSTANCE_LOCK: OnceLock<File> = OnceLock::new();

pub fn run_desktop_pet_process() {
    desktop_pet_runtime::run_desktop_pet_process();
}

fn push_unique_pet_root(
    roots: &mut Vec<PetResourceRoot>,
    seen: &mut Vec<PathBuf>,
    path: PathBuf,
    source: PetPackageSource,
) {
    if seen.iter().any(|current| current == &path) {
        return;
    }
    seen.push(path.clone());
    roots.push(PetResourceRoot::new(path, source));
}

fn desktop_pet_resource_roots(app: &tauri::App, app_dir: &Path) -> Vec<PetResourceRoot> {
    let mut roots = Vec::new();
    let mut seen = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        push_unique_pet_root(
            &mut roots,
            &mut seen,
            resource_dir.join("desktop-pets"),
            PetPackageSource::BuiltIn,
        );
        push_unique_pet_root(
            &mut roots,
            &mut seen,
            resource_dir.join("resources").join("desktop-pets"),
            PetPackageSource::BuiltIn,
        );
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_unique_pet_root(
                &mut roots,
                &mut seen,
                parent.join("desktop-pets"),
                PetPackageSource::Portable,
            );
            push_unique_pet_root(
                &mut roots,
                &mut seen,
                parent.join("resources").join("desktop-pets"),
                PetPackageSource::Portable,
            );
        }
    }
    push_unique_pet_root(
        &mut roots,
        &mut seen,
        app_dir.join("desktop-pets"),
        PetPackageSource::User,
    );
    roots
}

fn shared_desktop_pet_app_dir(app_dir: &Path) -> PathBuf {
    if app_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("com.lanchat.desktop"))
    {
        return app_dir.to_path_buf();
    }
    app_dir
        .parent()
        .map(|parent| parent.join("com.lanchat.desktop"))
        .unwrap_or_else(|| app_dir.to_path_buf())
}

fn alert_mode_label(mode: &str) -> &'static str {
    if mode.eq_ignore_ascii_case("disco") {
        "蹦迪报警"
    } else {
        "普通报警"
    }
}

fn format_alert_time(timestamp_millis: i64) -> String {
    chrono::Local
        .timestamp_millis_opt(timestamp_millis)
        .single()
        .unwrap_or_else(chrono::Local::now)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

fn render_external_push_content(template: &str, frame: &QuickAlertFrame) -> String {
    let content = template
        .replace("{mode}", alert_mode_label(&frame.mode))
        .replace("{content}", &frame.content)
        .replace("{time}", &format_alert_time(frame.created_at));
    content.trim().to_string()
}

fn render_external_push_alert_text(template: &str, frame: &QuickAlertFrame) -> String {
    let source_ip = frame.sender_address.as_deref().unwrap_or("未知 IP");
    let source = format!("来源：{}（{}）", frame.sender_nickname, source_ip);
    let content = render_external_push_content(template, frame);
    if content.is_empty() {
        source
    } else {
        format!("{content}\n{source}")
    }
}

async fn send_external_push_alert(
    config: ExternalPushConfig,
    frame: QuickAlertFrame,
) -> Result<(), String> {
    if !config.enabled {
        return Ok(());
    }
    let webhook = config.webhook.trim();
    if webhook.is_empty() {
        return Ok(());
    }
    if !webhook.starts_with("https://") {
        return Err("外部推送机器人 Webhook 必须使用 https:// 地址".to_string());
    }
    let content = render_external_push_alert_text(&config.template, &frame);
    let payload = external_push_payload(&config.kind, config.mention_all, &content);
    post_external_push_webhook(&config.name, webhook, &payload).await
}

/// 钉钉/企业微信机器人 text 消息体，狼来了与人脸识别告警共用。
fn external_push_payload(kind: &str, mention_all: bool, content: &str) -> serde_json::Value {
    if kind == "dingtalk" {
        serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": content,
            },
            "at": {
                "isAtAll": mention_all,
            }
        })
    } else {
        let mentioned_list = if mention_all {
            vec!["@all"]
        } else {
            Vec::new()
        };
        serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": content,
                "mentioned_list": mentioned_list,
            }
        })
    }
}

async fn post_external_push_webhook(
    name: &str,
    webhook: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(webhook)
        .json(payload)
        .send()
        .await
        .map_err(|error| format!("外部推送「{name}」发送失败：{error}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|_| String::new());
    if !status.is_success() {
        return Err(format!("外部推送「{name}」发送失败：HTTP {status}"));
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
        let code = value
            .get("errcode")
            .and_then(|item| item.as_i64())
            .unwrap_or(0);
        if code != 0 {
            let message = value
                .get("errmsg")
                .and_then(|item| item.as_str())
                .unwrap_or("未知错误");
            return Err(format!("外部推送「{name}」发送失败：{message}"));
        }
    }
    Ok(())
}

/// 人物识别告警的推送文案：明确区分人脸确认和人体特征疑似命中。
fn render_camera_face_push_text(frame: &protocol::CameraFaceAlertFrame) -> String {
    let source_ip = frame.source_address.as_deref().unwrap_or("未知 IP");
    let (title, action) = if frame.recognition_level == "suspected" {
        ("人体特征告警", "通过人体特征疑似检测到")
    } else {
        ("人脸确认告警", "通过人脸确认检测到")
    };
    format!(
        "[{title}] {}°C\n{} 【{}】 在【{}】附近游荡\n来源：{}（{}）",
        frame.confidence,
        action,
        frame.person_name,
        frame.source_nickname,
        frame.source_nickname,
        source_ip,
    )
}

/// 人脸识别告警的独立外部推送：不走可信度阈值，不复用狼来了模板。
async fn send_camera_face_external_push(
    config: ExternalPushConfig,
    frame: protocol::CameraFaceAlertFrame,
) -> Result<(), String> {
    if !config.enabled {
        return Ok(());
    }
    let webhook = config.webhook.trim();
    if webhook.is_empty() {
        return Ok(());
    }
    if !webhook.starts_with("https://") {
        return Err("外部推送机器人 Webhook 必须使用 https:// 地址".to_string());
    }
    let content = render_camera_face_push_text(&frame);
    // 人物识别告警固定提醒全员；使用机器人协议字段，不把“@所有人”拼进正文。
    let payload = external_push_payload(&config.kind, true, &content);
    post_external_push_webhook(&config.name, webhook, &payload).await
}

#[cfg(test)]
mod camera_face_push_tests {
    use super::*;

    #[test]
    fn camera_face_push_text_names_person_and_source() {
        let frame = protocol::CameraFaceAlertFrame {
            alert_id: "alert-1".to_string(),
            source_kind: "camera_face".to_string(),
            source_device_id: "device-1".to_string(),
            source_nickname: "监控机".to_string(),
            source_address: Some("192.168.1.10".to_string()),
            person_id: "person-1".to_string(),
            person_name: "张三".to_string(),
            confidence: 87,
            recognition_level: "confirmed".to_string(),
            face_confidence: Some(87),
            body_confidence: None,
            consecutive_hits: 2,
            policy_version: 3,
            created_at: 1_723_000_000_000,
        };
        let text = render_camera_face_push_text(&frame);
        assert!(text.starts_with(
            "[人脸确认告警] 87°C\n通过人脸确认检测到 【张三】 在【监控机】附近游荡\n"
        ));
        assert!(text.contains("来源：监控机（192.168.1.10）"));
        assert!(!text.contains("@所有人"));
        assert_eq!(
            external_push_payload("wechat_work", true, &text)["text"]["mentioned_list"][0],
            "@all"
        );
        assert_eq!(
            external_push_payload("dingtalk", true, &text)["at"]["isAtAll"],
            true
        );
    }

    #[test]
    fn camera_body_push_text_keeps_suspected_semantics() {
        let frame = protocol::CameraFaceAlertFrame {
            alert_id: "alert-body-1".to_string(),
            source_kind: "camera_face".to_string(),
            source_device_id: "device-1".to_string(),
            source_nickname: "监控机".to_string(),
            source_address: Some("192.168.1.10".to_string()),
            person_id: "person-1".to_string(),
            person_name: "张三".to_string(),
            confidence: 76,
            recognition_level: "suspected".to_string(),
            face_confidence: None,
            body_confidence: Some(76),
            consecutive_hits: 2,
            policy_version: 3,
            created_at: 1_723_000_000_000,
        };

        let text = render_camera_face_push_text(&frame);

        assert!(text.starts_with(
            "[人体特征告警] 76°C\n通过人体特征疑似检测到 【张三】 在【监控机】附近游荡\n"
        ));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PrivateChannelInviteCardPayload {
    channel_id: String,
    title: String,
    owner_device_id: String,
    owner_nickname: String,
    channel_key: String,
    key_version: u32,
    members: Vec<ChannelMemberSeed>,
    created_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrayAttentionItem {
    id: String,
    kind: String,
    title: String,
    count: u32,
}

#[derive(Default)]
struct TrayState {
    latest_target: Option<TrayAttentionItem>,
    items: Vec<TrayAttentionItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformInfo {
    os: &'static str,
    windows_firewall_repair_supported: bool,
    desktop_pet_supported: bool,
    global_shortcut_requires_permission: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppVersionInfo {
    version: String,
    build_version: String,
    build_timestamp: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateDownloadLinks {
    windows_portable: Option<String>,
    windows_portable_sha256: Option<String>,
    windows_installer: Option<String>,
    macos_dmg: Option<String>,
    release_page: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckResult {
    repository: String,
    current: AppVersionInfo,
    latest_version: String,
    latest_build: Option<String>,
    title: String,
    notes: String,
    release_url: String,
    downloads: UpdateDownloadLinks,
    update_available: bool,
    force: bool,
    min_supported_version: Option<String>,
    force_required: bool,
    checked_at: i64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    html_url: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Default, Deserialize)]
struct ReleaseUpdateMetadata {
    version: Option<String>,
    build: Option<String>,
    force: Option<bool>,
    min_supported_version: Option<String>,
    title: Option<String>,
    notes: Option<String>,
    downloads: Option<ReleaseDownloadMetadata>,
}

#[derive(Debug, Default, Deserialize)]
struct ReleaseDownloadMetadata {
    windows_portable: Option<String>,
    windows_portable_sha256: Option<String>,
    windows_installer: Option<String>,
    macos_dmg: Option<String>,
    release_page: Option<String>,
}

struct AppState {
    storage: Arc<Storage>,
    network: Network,
    file_server: FileServer,
    tray: Arc<Mutex<TrayState>>,
    desktop_pet_controller: DesktopPetController,
    desktop_pet: DesktopPetManager,
    desktop_pet_send_hotkey: Arc<Mutex<Option<Shortcut>>>,
    desktop_pet_stop_hotkey: Arc<Mutex<Option<Shortcut>>>,
    super_admin_session: Arc<Mutex<bool>>,
    face_monitor: Arc<FaceMonitorRuntime>,
    vision_mailbox: Arc<LatestFrameMailbox>,
    vision_runtime: Arc<VisionRuntimeState>,
    vision_model_catalog: Arc<Mutex<Vec<VisionCatalogProfile>>>,
    vision_model_root: PathBuf,
    // 由状态持有，确保应用生命周期内只有一个视觉推理 Worker。
    _vision_worker: VisionWorker,
}

fn ensure_full_client(state: &AppState, capability: &str) -> Result<(), String> {
    if state.network.supports_chat() {
        Ok(())
    } else {
        Err(format!("当前客户端不支持{capability}"))
    }
}

const TRAY_NORMAL_ICON: &[u8] = include_bytes!("../icons/32x32.png");

fn platform_info_value() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS,
        windows_firewall_repair_supported: cfg!(target_os = "windows"),
        desktop_pet_supported: cfg!(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )),
        global_shortcut_requires_permission: cfg!(target_os = "macos"),
    }
}

#[tauri::command]
fn get_platform_info() -> PlatformInfo {
    platform_info_value()
}

#[tauri::command]
fn get_face_monitor_status(state: State<'_, AppState>) -> FaceMonitorStatus {
    state.face_monitor.status()
}

#[tauri::command]
fn get_vision_runtime_snapshot(state: State<'_, AppState>) -> VisionRuntimeSnapshot {
    state.vision_runtime.snapshot()
}

#[tauri::command]
fn set_vision_runtime_paused(
    state: State<'_, AppState>,
    paused: bool,
) -> Result<VisionRuntimeSnapshot, String> {
    if paused {
        state.vision_runtime.pause_by_user();
    } else {
        state.vision_runtime.resume_by_user();
    }
    let persisted = state.vision_runtime.persisted_state();
    state.storage.save_vision_runtime_state(&persisted)?;
    Ok(persisted.snapshot)
}

#[tauri::command]
fn update_face_monitor_local_settings(
    state: State<'_, AppState>,
    mut settings: FaceMonitorLocalSettings,
) -> Result<FaceMonitorLocalSettings, String> {
    settings = settings.normalized();
    let profile = state.storage.get_or_create_profile()?;
    if let Some(policy) = state
        .storage
        .effective_face_monitor_policy(&profile.device_id)?
    {
        let is_new_policy = policy.version > settings.applied_policy_version;
        if is_new_policy || policy.settings_locked {
            settings.face_min_confidence = policy.min_confidence;
            settings.body_min_confidence = policy.body_min_confidence;
            settings.sample_fps = policy.sample_fps;
            settings.consecutive_hits = policy.consecutive_hits;
        }
        if is_new_policy {
            settings.face_cooldown_seconds = policy.face_cooldown_seconds;
            settings.body_cooldown_seconds = policy.body_cooldown_seconds;
            settings.applied_policy_version = policy.version;
        }
    }
    let saved = state.face_monitor.update_settings(settings);
    if saved.enabled {
        state.vision_runtime.resume_by_user();
    } else {
        state.vision_runtime.pause_by_user();
    }
    state
        .vision_runtime
        .mark_model_availability(state.face_monitor.status().model_ready);
    state
        .storage
        .save_vision_runtime_state(&state.vision_runtime.persisted_state())?;
    Ok(saved)
}

fn resolved_face_monitor_policy(
    device_id: &str,
    local: &FaceMonitorLocalSettings,
    remote: Option<FaceMonitorPolicyRecord>,
) -> FaceMonitorPolicyRecord {
    let Some(mut policy) = remote else {
        return FaceMonitorPolicyRecord {
            target_device_id: device_id.to_string(),
            min_confidence: local.face_min_confidence,
            body_min_confidence: local.body_min_confidence,
            sample_fps: local.sample_fps,
            consecutive_hits: local.consecutive_hits,
            cooldown_seconds: local.face_cooldown_seconds,
            face_cooldown_seconds: local.face_cooldown_seconds,
            body_cooldown_seconds: local.body_cooldown_seconds,
            settings_locked: false,
            version: local.applied_policy_version,
            issued_by_device_id: "local".to_string(),
            issued_by_nickname: "本机设置".to_string(),
            issued_at: 0,
        };
    };
    let remote_not_applied = policy.version > local.applied_policy_version;
    if !policy.settings_locked && !remote_not_applied {
        policy.min_confidence = local.face_min_confidence;
        policy.body_min_confidence = local.body_min_confidence;
        policy.sample_fps = local.sample_fps;
        policy.consecutive_hits = local.consecutive_hits;
    }
    if !remote_not_applied {
        // 冷却时间永远允许本机调整，不参与超管锁定。
        policy.cooldown_seconds = local.face_cooldown_seconds;
        policy.face_cooldown_seconds = local.face_cooldown_seconds;
        policy.body_cooldown_seconds = local.body_cooldown_seconds;
    }
    policy
}

#[cfg(test)]
mod face_monitor_policy_resolution_tests {
    use super::*;

    fn local_settings(applied_policy_version: i64) -> FaceMonitorLocalSettings {
        FaceMonitorLocalSettings {
            enabled: true,
            face_recognition_enabled: true,
            body_recognition_enabled: true,
            device_id: None,
            pause_during_call: false,
            sample_fps: 3,
            face_min_confidence: 61,
            body_min_confidence: 71,
            consecutive_hits: 2,
            face_cooldown_seconds: 45,
            body_cooldown_seconds: 75,
            applied_policy_version,
        }
    }

    fn remote_policy(settings_locked: bool) -> FaceMonitorPolicyRecord {
        FaceMonitorPolicyRecord {
            target_device_id: "device-a".to_string(),
            min_confidence: 82,
            body_min_confidence: 77,
            sample_fps: 5,
            consecutive_hits: 4,
            cooldown_seconds: 120,
            face_cooldown_seconds: 120,
            body_cooldown_seconds: 180,
            settings_locked,
            version: 10,
            issued_by_device_id: "admin".to_string(),
            issued_by_nickname: "管理员".to_string(),
            issued_at: 10,
        }
    }

    #[test]
    fn unlocked_applied_policy_allows_local_strategy_overrides() {
        let resolved = resolved_face_monitor_policy(
            "device-a",
            &local_settings(10),
            Some(remote_policy(false)),
        );
        assert_eq!(resolved.min_confidence, 61);
        assert_eq!(resolved.body_min_confidence, 71);
        assert_eq!(resolved.sample_fps, 3);
        assert_eq!(resolved.consecutive_hits, 2);
        assert_eq!(resolved.face_cooldown_seconds, 45);
        assert_eq!(resolved.body_cooldown_seconds, 75);
    }

    #[test]
    fn locked_policy_keeps_local_cooldown_editable() {
        let resolved = resolved_face_monitor_policy(
            "device-a",
            &local_settings(10),
            Some(remote_policy(true)),
        );
        assert_eq!(resolved.min_confidence, 82);
        assert_eq!(resolved.body_min_confidence, 77);
        assert_eq!(resolved.sample_fps, 5);
        assert_eq!(resolved.consecutive_hits, 4);
        assert_eq!(resolved.face_cooldown_seconds, 45);
        assert_eq!(resolved.body_cooldown_seconds, 75);
    }

    #[test]
    fn newer_remote_policy_is_applied_before_local_acknowledgement() {
        let resolved = resolved_face_monitor_policy(
            "device-a",
            &local_settings(9),
            Some(remote_policy(false)),
        );
        assert_eq!(resolved.min_confidence, 82);
        assert_eq!(resolved.body_min_confidence, 77);
        assert_eq!(resolved.sample_fps, 5);
        assert_eq!(resolved.consecutive_hits, 4);
        assert_eq!(resolved.face_cooldown_seconds, 120);
        assert_eq!(resolved.body_cooldown_seconds, 180);
    }
}

/// 旧识别器的兼容处理路径。
///
/// Raw 帧由专用 Worker 转为 JPEG 后进入这里，因此前端不再承担编码和大对象传输。
async fn process_face_monitor_frame(
    app: tauri::AppHandle,
    state: &AppState,
    bytes: &[u8],
) -> Result<Option<CameraFaceAlertRecord>, String> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let local_settings = state.face_monitor.settings();
    if !local_settings.enabled
        || (!local_settings.face_recognition_enabled && !local_settings.body_recognition_enabled)
    {
        state.face_monitor.prepare_match_frame(&[]);
        return Ok(None);
    }
    // 对应识别模型未就绪或无可用录入人员时不产生任何告警事件与数据，
    // 错误原因通过 get_face_monitor_status 由设置页展示。
    let runtime_status = state.face_monitor.status();
    let face_ready = local_settings.face_recognition_enabled && runtime_status.recognizer_ready;
    let body_ready = local_settings.body_recognition_enabled
        && runtime_status.person_detector_ready
        && runtime_status.person_recognizer_ready;
    if !face_ready && !body_ready {
        return Ok(None);
    }
    let templates = load_recognition_templates(state)?;
    if templates.is_empty() {
        return Ok(None);
    }
    let Some(recognition) = state.face_monitor.recognize_frame(bytes, &templates)? else {
        state.face_monitor.prepare_match_frame(&[]);
        return Ok(None);
    };
    if recognition.matches.is_empty() {
        state.face_monitor.prepare_match_frame(&[]);
        return Ok(None);
    }
    let profile = state.storage.get_or_create_profile()?;
    let remote_policy = state
        .storage
        .effective_face_monitor_policy(&profile.device_id)?;
    let policy = resolved_face_monitor_policy(&profile.device_id, &local_settings, remote_policy);
    let now = chrono::Utc::now().timestamp_millis();
    let mut eligible: std::collections::HashMap<String, FaceMatch> =
        std::collections::HashMap::new();
    for matched in recognition.matches.into_iter().filter(|item| {
        let enabled = if item.recognition_level == "suspected" {
            local_settings.body_recognition_enabled
        } else {
            local_settings.face_recognition_enabled
        };
        let min_confidence = if item.recognition_level == "suspected" {
            policy.body_min_confidence
        } else {
            policy.min_confidence
        };
        enabled && item.confidence >= min_confidence
    }) {
        eligible
            .entry(matched.person_id.clone())
            .and_modify(|current| {
                let matched_rank = if matched.recognition_level == "confirmed" {
                    2
                } else {
                    1
                };
                let current_rank = if current.recognition_level == "confirmed" {
                    2
                } else {
                    1
                };
                if matched_rank > current_rank
                    || (matched_rank == current_rank && matched.confidence > current.confidence)
                {
                    *current = matched.clone();
                }
            })
            .or_insert(matched);
    }
    let candidate_keys = eligible
        .values()
        .map(|matched| {
            format!(
                "camera-person:{}:{}",
                matched.recognition_level, matched.person_id
            )
        })
        .collect::<Vec<_>>();
    state.face_monitor.prepare_match_frame(&candidate_keys);
    let mut first_record = None;
    for matched in eligible.into_values() {
        let min_confidence = if matched.recognition_level == "suspected" {
            policy.body_min_confidence
        } else {
            policy.min_confidence
        };
        let cooldown_seconds = if matched.recognition_level == "suspected" {
            policy.body_cooldown_seconds
        } else {
            policy.face_cooldown_seconds
        };
        let gate_key = format!(
            "camera-person:{}:{}",
            matched.recognition_level, matched.person_id
        );
        if !state.face_monitor.accept_match(
            &gate_key,
            matched.confidence,
            min_confidence,
            policy.consecutive_hits,
            cooldown_seconds,
            now,
        ) {
            continue;
        }
        let record =
            publish_camera_face_alert(app.clone(), state, &profile, &policy, &matched, now).await?;
        if first_record.is_none() {
            first_record = Some(record);
        }
    }
    Ok(first_record)
}

/// 旧版 JPEG 接口保留到兼容切换完成，内部与新 Worker 共用同一套识别逻辑。
#[tauri::command]
async fn submit_face_monitor_frame(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    bytes: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<Option<CameraFaceAlertRecord>, String> {
    let _ = (width, height);
    process_face_monitor_frame(app, &state, &bytes).await
}

/// 新视觉运行时的轻量入口：仅验证二进制 Envelope 并覆盖上一帧。
/// 真正推理由专用 Worker 消费该邮箱，命令线程不执行模型与数据库读取。
#[tauri::command]
fn submit_vision_frame_raw(state: State<'_, AppState>, frame: Vec<u8>) -> Result<(), String> {
    let frame = decode_raw_frame(&frame)?;
    state
        .vision_runtime
        .mark_model_availability(state.face_monitor.status().model_ready);
    state.vision_mailbox.submit(frame);
    state.vision_runtime.record_frame_accepted(
        state.vision_mailbox.dropped_frames(),
        state.vision_mailbox.queue_depth(),
        state.vision_mailbox.pending_frame_bytes(),
        state.vision_mailbox.stream_reset_count(),
    );
    Ok(())
}

#[tauri::command]
fn get_vision_runtime_diagnostics(state: State<'_, AppState>) -> VisionRuntimeDiagnostics {
    state.vision_runtime.diagnostics()
}

fn vision_model_profiles(state: &AppState) -> Result<Vec<VisionModelProfileSummary>, String> {
    let snapshot = state.vision_runtime.snapshot();
    let mut profiles = state.storage.list_vision_model_profiles()?;
    profiles.insert(
        0,
        VisionModelProfileSummary {
            profile_id: "baseline".to_string(),
            profile_version: state
                .face_monitor
                .status()
                .model_version
                .unwrap_or_else(|| "1.0.0".to_string()),
            display_name: "内置基础模型".to_string(),
            tier: "low_resource".to_string(),
            installed: state.face_monitor.status().model_assets_ready,
            active: snapshot.active_profile_id.as_deref() == Some("baseline"),
            compatible: state.face_monitor.status().model_assets_ready,
            compatibility_reason: state.face_monitor.status().last_error,
            downloadable: false,
            package_size_bytes: 0,
            restart_required: false,
        },
    );
    let catalog = state
        .vision_model_catalog
        .lock()
        .map_err(|_| "视觉模型目录状态不可用".to_string())?
        .clone();
    for profile in catalog {
        if profiles.iter().any(|item| {
            item.profile_id == profile.profile_id && item.profile_version == profile.profile_version
        }) {
            continue;
        }
        profiles.push(VisionModelProfileSummary {
            profile_id: profile.profile_id,
            profile_version: profile.profile_version,
            display_name: profile.display_name,
            tier: profile.tier,
            installed: false,
            active: false,
            compatible: true,
            compatibility_reason: None,
            downloadable: true,
            package_size_bytes: profile.package_size_bytes,
            restart_required: true,
        });
    }
    Ok(profiles)
}

#[tauri::command]
fn list_vision_model_profiles(
    state: State<'_, AppState>,
) -> Result<Vec<VisionModelProfileSummary>, String> {
    vision_model_profiles(&state)
}

#[tauri::command]
async fn refresh_vision_model_catalog(
    state: State<'_, AppState>,
) -> Result<Vec<VisionModelProfileSummary>, String> {
    let catalog = fetch_official_catalog(&update_http_client()).await?;
    let mut target = state
        .vision_model_catalog
        .lock()
        .map_err(|_| "视觉模型目录状态不可用".to_string())?;
    *target = catalog.profiles;
    drop(target);
    vision_model_profiles(&state)
}

#[tauri::command]
async fn install_vision_model_profile(
    state: State<'_, AppState>,
    profile_id: String,
    profile_version: String,
) -> Result<Vec<VisionModelProfileSummary>, String> {
    let profile = state
        .vision_model_catalog
        .lock()
        .map_err(|_| "视觉模型目录状态不可用".to_string())?
        .iter()
        .find(|item| item.profile_id == profile_id && item.profile_version == profile_version)
        .cloned()
        .ok_or_else(|| "VISION_MODEL_PROFILE_NOT_IN_CATALOG".to_string())?;
    let installed =
        download_and_install(&update_http_client(), &profile, &state.vision_model_root).await?;
    let summary = VisionModelProfileSummary {
        profile_id: installed.profile.profile_id.clone(),
        profile_version: installed.profile.profile_version.clone(),
        display_name: installed.profile.display_name.clone(),
        tier: installed.profile.tier.clone(),
        installed: true,
        active: false,
        compatible: true,
        compatibility_reason: None,
        downloadable: true,
        package_size_bytes: installed.bytes,
        restart_required: true,
    };
    state.storage.upsert_vision_model_profile(
        &summary,
        &installed.manifest_json,
        &installed.install_dir,
    )?;
    vision_model_profiles(&state)
}

#[tauri::command]
fn activate_vision_model_profile(
    state: State<'_, AppState>,
    profile_id: String,
    profile_version: String,
) -> Result<Vec<VisionModelProfileSummary>, String> {
    state
        .storage
        .activate_vision_model_profile(&profile_id, &profile_version)?;
    vision_model_profiles(&state)
}

/// 加载启用中录入人员的特征模板：优先用版本匹配的已存特征，
/// 版本不一致时用参考照片重新提取并落库；照片不可读或无人脸时清空特征并跳过。
fn load_recognition_templates(state: &AppState) -> Result<Vec<PersonTemplate>, String> {
    let model_version = state
        .face_monitor
        .status()
        .model_version
        .unwrap_or_default();
    let mut templates = Vec::new();
    for person in state.storage.list_face_people()? {
        if !person.enabled || person.deleted_at.is_some() {
            continue;
        }
        let mut samples = state.storage.list_face_person_samples(&person.person_id)?;
        // 旧版本的一张参考照片在首次读取时自动迁为一条样本，已有人员无需重新录入。
        if samples.is_empty() {
            if let Some(photo_url) = person.photo_url.clone() {
                samples.push(FacePersonSampleRecord {
                    sample_id: format!("{}-legacy", person.person_id),
                    person_id: person.person_id.clone(),
                    photo_url,
                    photo_sha256: person.photo_sha256.clone(),
                    embedding: person.embedding.clone(),
                    embedding_model_version: person.embedding_model_version.clone(),
                    body_embedding: None,
                    body_embedding_model_version: None,
                });
            }
        }
        let mut face_embeddings = Vec::new();
        let mut body_embeddings = Vec::new();
        let mut refreshed_samples = Vec::new();
        let mut samples_dirty = false;
        for mut sample in samples {
            let bytes = std::fs::read(&sample.photo_url).ok();
            let face_embedding =
                if sample.embedding_model_version.as_deref() == Some(model_version.as_str()) {
                    sample
                        .embedding
                        .as_deref()
                        .and_then(|value| embedding_from_bytes(value).ok())
                } else {
                    None
                };
            let face_embedding = face_embedding.or_else(|| {
                let generated = bytes
                    .as_deref()
                    .and_then(|value| state.face_monitor.embedding_from_photo_bytes(value).ok());
                if generated.is_some() {
                    samples_dirty = true;
                }
                generated
            });
            let body_embedding =
                if sample.body_embedding_model_version.as_deref() == Some(model_version.as_str()) {
                    sample
                        .body_embedding
                        .as_deref()
                        .and_then(|value| dynamic_embedding_from_bytes(value, PERSON_REID_DIM).ok())
                } else {
                    None
                };
            let body_embedding = body_embedding.or_else(|| {
                let generated = bytes.as_deref().and_then(|value| {
                    state
                        .face_monitor
                        .body_embedding_from_photo_bytes(value)
                        .ok()
                });
                if generated.is_some() {
                    samples_dirty = true;
                }
                generated
            });
            if body_embedding.is_none() && sample.body_embedding.is_some() {
                sample.body_embedding = None;
                sample.body_embedding_model_version = None;
                samples_dirty = true;
            }
            if let Some(embedding) = face_embedding {
                sample.embedding = Some(embedding_bytes(&embedding));
                sample.embedding_model_version = Some(model_version.clone());
                face_embeddings.push(embedding);
            }
            if let Some(embedding) = body_embedding {
                sample.body_embedding = Some(dynamic_embedding_bytes(&embedding));
                sample.body_embedding_model_version = Some(model_version.clone());
                body_embeddings.push(embedding);
            }
            if sample.embedding.is_none() && sample.body_embedding.is_none() {
                continue;
            }
            refreshed_samples.push(sample);
        }
        if !refreshed_samples.is_empty() {
            if samples_dirty || person.sample_count == 0 {
                state
                    .storage
                    .replace_face_person_samples(&person.person_id, &refreshed_samples)
                    .ok();
                let first_face = refreshed_samples
                    .iter()
                    .find_map(|sample| sample.embedding.clone());
                state
                    .storage
                    .update_face_person_embedding(
                        &person.person_id,
                        first_face,
                        Some(model_version.clone()),
                    )
                    .ok();
            }
            templates.push(PersonTemplate {
                person_id: person.person_id,
                display_name: person.display_name,
                face_embeddings,
                body_embeddings,
            });
        } else {
            state
                .storage
                .update_face_person_embedding(&person.person_id, None, None)
                .ok();
        }
    }
    Ok(templates)
}

#[tauri::command]
fn list_face_people(state: State<'_, AppState>) -> Result<Vec<FacePersonRecord>, String> {
    state.storage.list_face_people()
}

#[tauri::command]
fn delete_face_person_local(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    person_id: String,
) -> Result<(), String> {
    let samples = state.storage.list_face_person_samples(person_id.trim())?;
    state.storage.delete_face_person_local(person_id.trim())?;
    let reference_root = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("读取应用数据目录失败：{err}"))?
        .join("face-reference-uploads");
    let reference_root = std::fs::canonicalize(&reference_root).ok();
    for sample in samples {
        let path = std::path::PathBuf::from(sample.photo_url);
        let can_remove = match (reference_root.as_ref(), std::fs::canonicalize(&path).ok()) {
            (Some(root), Some(candidate)) => candidate.starts_with(root),
            _ => false,
        };
        if can_remove {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

#[tauri::command]
fn save_face_reference_photo(app: tauri::AppHandle, bytes: Vec<u8>) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("参考照片内容为空".to_string());
    }
    image::load_from_memory(&bytes).map_err(|err| format!("参考照片无法解码：{err}"))?;
    let root = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("读取应用数据目录失败：{err}"))?
        .join("face-reference-uploads");
    std::fs::create_dir_all(&root).map_err(|err| format!("创建参考照片目录失败：{err}"))?;
    let path = root.join(format!("{}.jpg", Uuid::new_v4()));
    std::fs::write(&path, bytes).map_err(|err| format!("保存参考照片失败：{err}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn create_local_face_person(
    state: State<'_, AppState>,
    person_id: String,
    display_name: String,
    photo_paths: Vec<String>,
) -> Result<FacePersonRecord, String> {
    let profile = state.storage.get_or_create_profile()?;
    let person_id = person_id.trim();
    let display_name = display_name.trim();
    let photo_paths = photo_paths
        .into_iter()
        .filter(|path| !path.trim().is_empty())
        .take(vision::storage::MAX_REFERENCE_IMAGES)
        .collect::<Vec<_>>();
    if person_id.is_empty() || display_name.is_empty() {
        return Err("请填写人员名称并提供参考照片".to_string());
    }
    vision::storage::validate_new_reference_image_count(photo_paths.len()).map_err(|_| {
        format!(
            "请提供 {} 至 {} 张参考照片",
            vision::storage::MIN_REFERENCE_IMAGES,
            vision::storage::MAX_REFERENCE_IMAGES
        )
    })?;
    if !state.face_monitor.status().recognizer_ready {
        return Err("识别模型未安装，暂时无法录入识别人员".to_string());
    }
    let mut samples = Vec::new();
    for photo_path in &photo_paths {
        let bytes = std::fs::read(photo_path).map_err(|err| format!("读取参考照片失败：{err}"))?;
        image::load_from_memory(&bytes).map_err(|err| format!("参考照片无法解码：{err}"))?;
        let analysis = state.face_monitor.analyze_reference_photo(&bytes)?;
        vision::storage::validate_reference_subject_count(analysis.detected_subject_count)
            .map_err(|_| "参考照片中检测到多个人，请单独上传目标人员照片".to_string())?;
        let face_embedding = state.face_monitor.embedding_from_photo_bytes(&bytes).ok();
        let body_embedding = state
            .face_monitor
            .body_embedding_from_photo_bytes(&bytes)
            .ok();
        if face_embedding.is_none() && body_embedding.is_none() {
            return Err("参考照片中未检测到可用的人脸或人物".to_string());
        }
        samples.push(FacePersonSampleRecord {
            sample_id: Uuid::new_v4().to_string(),
            person_id: person_id.to_string(),
            photo_url: photo_path.clone(),
            photo_sha256: Some(hex::encode(sha2::Sha256::digest(&bytes))),
            embedding: face_embedding.as_ref().map(embedding_bytes),
            embedding_model_version: face_embedding
                .as_ref()
                .and(state.face_monitor.status().model_version.clone()),
            body_embedding: body_embedding
                .as_ref()
                .map(|value| dynamic_embedding_bytes(value)),
            body_embedding_model_version: body_embedding
                .as_ref()
                .and(state.face_monitor.status().model_version.clone()),
        });
    }
    let model_version = state.face_monitor.status().model_version;
    let record = state
        .storage
        .upsert_face_person(&protocol::FacePersonPolicyFrame {
            person_id: person_id.to_string(),
            display_name: display_name.to_string(),
            photo_url: Some(photo_paths[0].clone()),
            photo_urls: vec![],
            photo_sha256: samples[0].photo_sha256.clone(),
            expires_at: None,
            enabled: true,
            photo_sha256s: vec![],
            version: chrono::Utc::now().timestamp_millis(),
            action: "upsert".to_string(),
            issued_by_device_id: profile.device_id,
            issued_by_nickname: "本机录入".to_string(),
            issued_at: chrono::Utc::now().timestamp_millis(),
        })?;
    state
        .storage
        .replace_face_person_samples(&record.person_id, &samples)?;
    state.storage.update_face_person_embedding(
        &record.person_id,
        samples[0].embedding.clone(),
        model_version,
    )?;
    state
        .storage
        .list_face_people()?
        .into_iter()
        .find(|person| person.person_id == record.person_id)
        .ok_or_else(|| "保存人员样本后无法读取人员".to_string())
}

#[tauri::command]
fn get_effective_face_monitor_policy(
    state: State<'_, AppState>,
) -> Result<Option<FaceMonitorPolicyRecord>, String> {
    let profile = state.storage.get_or_create_profile()?;
    let remote = state
        .storage
        .effective_face_monitor_policy(&profile.device_id)?;
    Ok(Some(resolved_face_monitor_policy(
        &profile.device_id,
        &state.face_monitor.settings(),
        remote,
    )))
}

#[tauri::command]
fn list_camera_face_alerts(
    state: State<'_, AppState>,
) -> Result<Vec<CameraFaceAlertRecord>, String> {
    let profile = state.storage.get_or_create_profile()?;
    state
        .storage
        .list_camera_face_alerts_for_responder(100, &profile.device_id)
}

#[tauri::command]
fn clear_camera_face_alerts(state: State<'_, AppState>) -> Result<(), String> {
    state.storage.clear_camera_face_alerts()
}

#[tauri::command]
async fn send_camera_face_alert_feedback(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    alert_id: String,
    source_device_id: String,
    result: String,
) -> Result<CameraFaceAlertRecord, String> {
    let result = result.trim().to_ascii_lowercase();
    if !matches!(result.as_str(), "real" | "false") {
        return Err("反馈结果无效".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let frame = protocol::CameraFaceAlertFeedbackFrame {
        alert_id,
        source_device_id,
        responder_device_id: profile.device_id,
        responder_nickname: profile.nickname,
        result,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let mut record = state.storage.upsert_camera_face_alert_feedback(&frame)?;
    record.local_feedback = Some(frame.result.clone());
    state
        .network
        .broadcast_camera_face_alert_feedback(app.clone(), frame)
        .await?;
    app.emit("camera_face_alert_feedback_received", &record)
        .ok();
    Ok(record)
}

/// 识别命中后发布具名告警：落库、局域网广播、通知前端，与狼来了告警链路完全分离。
async fn publish_camera_face_alert(
    app: tauri::AppHandle,
    state: &AppState,
    profile: &storage::Profile,
    policy: &FaceMonitorPolicyRecord,
    matched: &FaceMatch,
    now: i64,
) -> Result<CameraFaceAlertRecord, String> {
    let frame = protocol::CameraFaceAlertFrame {
        // 保留旧来源值，确保旧客户端仍能接收；识别分级由 recognition_level 表达。
        alert_id: Uuid::new_v4().to_string(),
        source_kind: "camera_face".to_string(),
        source_device_id: profile.device_id.clone(),
        source_nickname: profile.nickname.clone(),
        source_address: Some(local_ip_address()),
        person_id: matched.person_id.clone(),
        person_name: matched.display_name.clone(),
        confidence: matched.confidence,
        recognition_level: matched.recognition_level.clone(),
        face_confidence: matched.face_confidence,
        body_confidence: matched.body_confidence,
        consecutive_hits: policy.consecutive_hits,
        policy_version: policy.version,
        created_at: now,
    };
    let record = state.storage.upsert_camera_face_alert(&frame)?;
    state
        .network
        .broadcast_camera_face_alert(app.clone(), frame.clone())
        .await?;
    app.emit("camera_face_alert_received", &record).ok();
    // 人脸识别告警的独立外部推送：不走可信度阈值，只在产生端执行。
    let settings = state.desktop_pet.settings();
    if settings.external_push_enabled {
        for config in settings
            .external_push_configs
            .into_iter()
            .filter(|item| item.enabled && !item.webhook.trim().is_empty())
        {
            let notify_frame = frame.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = send_camera_face_external_push(config, notify_frame).await {
                    eprintln!("{error}");
                }
            });
        }
    }
    Ok(record)
}

#[tauri::command]
async fn send_face_monitor_policy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    min_confidence: u8,
    body_min_confidence: u8,
    sample_fps: u8,
    consecutive_hits: u8,
    face_cooldown_seconds: u32,
    body_cooldown_seconds: u32,
    settings_locked: bool,
    version: i64,
) -> Result<FaceMonitorPolicyRecord, String> {
    ensure_super_admin_session(&state)?;
    let target = target_device_id.trim();
    if target.is_empty() {
        return Err("请选择策略接收设备".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let frame = protocol::FaceMonitorPolicyFrame {
        target_device_id: target.to_string(),
        min_confidence: min_confidence.clamp(1, 100),
        body_min_confidence: body_min_confidence.clamp(1, 100),
        sample_fps: sample_fps.clamp(1, 5),
        consecutive_hits: consecutive_hits.clamp(1, 20),
        cooldown_seconds: face_cooldown_seconds.clamp(5, 86_400),
        face_cooldown_seconds: face_cooldown_seconds.clamp(5, 86_400),
        body_cooldown_seconds: body_cooldown_seconds.clamp(5, 86_400),
        settings_locked,
        version,
        issued_by_device_id: profile.device_id.clone(),
        issued_by_nickname: profile.nickname.clone(),
        issued_at: chrono::Utc::now().timestamp_millis(),
    };
    if target == "*" {
        for peer in state
            .storage
            .list_peers()?
            .into_iter()
            .filter(|peer| peer.online)
        {
            let _ = state
                .network
                .send_face_monitor_policy(app.clone(), &peer.device_id, frame.clone())
                .await;
        }
    } else if target != profile.device_id {
        if !state
            .network
            .send_face_monitor_policy(app.clone(), target, frame.clone())
            .await?
        {
            return Err("目标设备不在线，识别策略未送达".to_string());
        }
    }
    let record = state.storage.upsert_face_monitor_policy(&frame)?;
    app.emit("face_monitor_policy_received", &record).ok();
    Ok(record)
}

#[tauri::command]
async fn send_face_person_policy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    person_id: String,
    display_name: String,
    photo_paths: Vec<String>,
    expires_at: Option<i64>,
    enabled: bool,
    action: String,
    version: i64,
) -> Result<FacePersonRecord, String> {
    ensure_super_admin_session(&state)?;
    let target = target_device_id.trim();
    let person_id = person_id.trim();
    let display_name = display_name.trim();
    let action = action.trim().to_ascii_lowercase();
    if target.is_empty() || person_id.is_empty() || display_name.is_empty() {
        return Err("请填写人员名称、人员标识和下发目标".to_string());
    }
    if !matches!(action.as_str(), "upsert" | "disable" | "delete") {
        return Err("人员规则操作无效".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let (photo_url, photo_urls, photo_sha256, photo_sha256s) = if action == "upsert" {
        let mut urls = Vec::new();
        let mut hashes = Vec::new();
        for raw_path in photo_paths
            .into_iter()
            .filter(|path| !path.trim().is_empty())
            .take(12)
        {
            let path = PathBuf::from(raw_path);
            let metadata =
                std::fs::metadata(&path).map_err(|err| format!("读取人员照片失败：{err}"))?;
            if !metadata.is_file() {
                return Err("人员参考照片必须是本地文件".to_string());
            }
            let bytes = std::fs::read(&path).map_err(|err| format!("读取人员照片失败：{err}"))?;
            let meta = state.file_server.share_file_with_options(
                path,
                Some("image/*".to_string()),
                None,
            )?;
            urls.push(meta.url);
            hashes.push(hex::encode(sha2::Sha256::digest(bytes)));
        }
        if urls.is_empty() {
            return Err("请选择至少一张人员参考照片".to_string());
        }
        (urls.first().cloned(), urls, hashes.first().cloned(), hashes)
    } else {
        (None, vec![], None, vec![])
    };
    let frame = protocol::FacePersonPolicyFrame {
        person_id: person_id.to_string(),
        display_name: display_name.to_string(),
        photo_url,
        photo_urls,
        photo_sha256,
        photo_sha256s,
        expires_at,
        enabled,
        version: version.max(1),
        action,
        issued_by_device_id: profile.device_id.clone(),
        issued_by_nickname: profile.nickname.clone(),
        issued_at: chrono::Utc::now().timestamp_millis(),
    };
    if target == "*" {
        for peer in state
            .storage
            .list_peers()?
            .into_iter()
            .filter(|peer| peer.online)
        {
            let _ = state
                .network
                .send_face_person_policy(app.clone(), &peer.device_id, frame.clone())
                .await;
        }
    } else if target != profile.device_id
        && !state
            .network
            .send_face_person_policy(app.clone(), target, frame.clone())
            .await?
    {
        return Err("目标设备不在线，人员照片未送达".to_string());
    }
    let record = state.storage.upsert_face_person(&frame)?;
    app.emit("face_person_policy_received", &record).ok();
    Ok(record)
}

fn local_app_version_info() -> AppVersionInfo {
    AppVersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_version: LOCAL_BUILD_VERSION.to_string(),
        build_timestamp: env!("LANCHAT_BUILD_TIMESTAMP").parse().unwrap_or(0),
    }
}

fn normalize_version(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('v')
        .split('+')
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn version_segments(value: &str) -> Vec<u64> {
    normalize_version(value)
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
                .parse::<u64>()
                .unwrap_or(0)
        })
        .collect()
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left = version_segments(left);
    let right = version_segments(right);
    let max_len = left.len().max(right.len()).max(1);
    for index in 0..max_len {
        let a = *left.get(index).unwrap_or(&0);
        let b = *right.get(index).unwrap_or(&0);
        match a.cmp(&b) {
            std::cmp::Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    std::cmp::Ordering::Equal
}

fn find_release_asset<'a>(
    release: &'a GithubRelease,
    predicate: impl Fn(&str) -> bool,
) -> Option<&'a GithubReleaseAsset> {
    release
        .assets
        .iter()
        .find(|asset| predicate(&asset.name.to_ascii_lowercase()))
}

async fn fetch_release_metadata(
    client: &reqwest::Client,
    release: &GithubRelease,
) -> Option<ReleaseUpdateMetadata> {
    let asset = find_release_asset(release, |name| name == UPDATE_METADATA_ASSET)?;
    let response = authorized_update_request(client, &asset.browser_download_url)
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<ReleaseUpdateMetadata>().await.ok()
}

fn asset_url(release: &GithubRelease, predicate: impl Fn(&str) -> bool) -> Option<String> {
    find_release_asset(release, predicate).map(|asset| asset.browser_download_url.clone())
}

async fn build_update_result(release: GithubRelease) -> UpdateCheckResult {
    let client = update_http_client();
    let metadata = fetch_release_metadata(&client, &release)
        .await
        .unwrap_or_default();
    let latest_version = metadata
        .version
        .clone()
        .unwrap_or_else(|| normalize_version(&release.tag_name));
    let release_page = metadata
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.release_page.clone())
        .unwrap_or_else(|| release.html_url.clone());
    let windows_portable = metadata
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.windows_portable.clone())
        .or_else(|| {
            asset_url(&release, |name| {
                name.contains("windows")
                    && (name.contains("portable") || name.contains("green"))
                    && name.ends_with(".zip")
            })
        });
    let windows_portable_sha256 = metadata
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.windows_portable_sha256.clone());
    let windows_installer = metadata
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.windows_installer.clone())
        .or_else(|| {
            asset_url(&release, |name| {
                (name.ends_with(".exe") || name.ends_with(".msi")) && !name.contains("lite")
            })
        });
    let macos_dmg = metadata
        .downloads
        .as_ref()
        .and_then(|downloads| downloads.macos_dmg.clone())
        .or_else(|| asset_url(&release, |name| name.ends_with(".dmg")));
    let current = local_app_version_info();
    let update_available =
        compare_versions(&latest_version, &current.version) == std::cmp::Ordering::Greater;
    let min_supported_version = metadata.min_supported_version.clone();
    let force = metadata.force.unwrap_or(false);
    let force_required = force && update_available;

    UpdateCheckResult {
        repository: UPDATE_REPOSITORY.to_string(),
        current,
        latest_version,
        latest_build: metadata.build.clone(),
        title: metadata
            .title
            .clone()
            .or_else(|| release.name.clone())
            .unwrap_or_else(|| release.tag_name.clone()),
        notes: metadata
            .notes
            .clone()
            .or_else(|| release.body.clone())
            .unwrap_or_default(),
        release_url: release.html_url.clone(),
        downloads: UpdateDownloadLinks {
            windows_portable,
            windows_portable_sha256,
            windows_installer,
            macos_dmg,
            release_page,
        },
        update_available,
        force,
        min_supported_version,
        force_required,
        checked_at: chrono::Utc::now().timestamp_millis(),
    }
}

#[tauri::command]
fn get_app_version_info() -> AppVersionInfo {
    local_app_version_info()
}

#[tauri::command]
async fn check_for_update() -> Result<UpdateCheckResult, String> {
    let client = update_http_client();
    let release = authorized_update_request(&client, UPDATE_API_URL)
        .send()
        .await
        .map_err(|err| format!("检查更新失败：{err}"))?
        .error_for_status()
        .map_err(|err| format!("检查更新失败：{err}"))?
        .json::<GithubRelease>()
        .await
        .map_err(|err| format!("解析更新信息失败：{err}"))?;
    Ok(build_update_result(release).await)
}

#[tauri::command]
fn get_update_github_token_info() -> UpdateGithubTokenInfo {
    update_github_token_info()
}

#[tauri::command]
fn save_update_github_token(token: String) -> Result<UpdateGithubTokenInfo, String> {
    let token = token.trim();
    if token.len() < 20 || token.chars().any(char::is_whitespace) {
        return Err("GitHub Token 格式不正确，请粘贴有效的 Personal Access Token".to_string());
    }
    update_github_token_entry()?
        .set_password(token)
        .map_err(|error| format!("保存 GitHub Token 失败：{error}"))?;
    Ok(update_github_token_info())
}

#[tauri::command]
fn clear_update_github_token() -> Result<UpdateGithubTokenInfo, String> {
    let entry = update_github_token_entry()?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(update_github_token_info()),
        Err(error) => Err(format!("清除 GitHub Token 失败：{error}")),
    }
}

#[tauri::command]
fn open_update_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://github.com/")
        || url.starts_with("https://api.github.com/")
        || url.starts_with("https://objects.githubusercontent.com/"))
    {
        return Err("更新链接不合法".to_string());
    }
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|error| format!("打开更新页面失败：{error}"))
}

fn running_portable_root() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let root = executable.parent()?.to_path_buf();
    if root.join("README.txt").is_file() && root.join("lanchat.exe").is_file() {
        Some(root)
    } else {
        None
    }
}

#[tauri::command]
fn is_portable_runtime() -> bool {
    cfg!(target_os = "windows") && running_portable_root().is_some()
}

fn normalized_remote_update_version(value: &str) -> Result<String, String> {
    let value = normalize_version(value);
    if value.is_empty()
        || value.len() > 32
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '+')
        })
    {
        return Err("目标版本格式不正确".to_string());
    }
    Ok(value)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("读取更新包失败：{error}"))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("读取更新包失败：{error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn is_allowed_remote_update_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if url.scheme() == "https"
        && matches!(
            url.host_str(),
            Some("github.com" | "api.github.com" | "objects.githubusercontent.com")
        )
    {
        return true;
    }
    if url.scheme() != "http" {
        return false;
    }
    url.host_str()
        .and_then(|host| host.parse::<IpAddr>().ok())
        .is_some_and(|address| match address {
            IpAddr::V4(address) => {
                address.is_private() || address.is_loopback() || address.is_link_local()
            }
            IpAddr::V6(address) => address.is_loopback() || address.is_unique_local(),
        })
}

async fn download_remote_update_package(
    url: &str,
    file_name: &str,
    expected_sha256: Option<&str>,
) -> Result<PathBuf, String> {
    const MAX_UPDATE_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    if !is_allowed_remote_update_url(url) {
        return Err("远程更新包地址不合法".to_string());
    }
    let safe_name = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("lanchat-update.exe");
    let update_root = std::env::temp_dir().join(format!("lanchat-admin-update-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&update_root)
        .map_err(|error| format!("创建更新临时目录失败：{error}"))?;
    let target = update_root.join(safe_name);
    let temporary = target.with_extension("downloading");
    let client = update_http_client();
    let mut request = client.get(url).header("User-Agent", "LanChat");
    if url.starts_with("https://github.com/")
        || url.starts_with("https://api.github.com/")
        || url.starts_with("https://objects.githubusercontent.com/")
    {
        if let Some(token) = read_update_github_token() {
            request = request.bearer_auth(token);
        }
    }
    let mut response = request
        .send()
        .await
        .map_err(|error| format!("下载远程更新包失败：{error}"))?
        .error_for_status()
        .map_err(|error| format!("下载远程更新包失败：{error}"))?;
    if response.content_length().unwrap_or(0) > MAX_UPDATE_PACKAGE_BYTES {
        return Err("远程更新包超过 2GB，已取消下载".to_string());
    }
    let mut file = tokio::fs::File::create(&temporary)
        .await
        .map_err(|error| format!("创建更新包文件失败：{error}"))?;
    let mut downloaded = 0_u64;
    let mut hasher = sha2::Sha256::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("读取远程更新包失败：{error}"))?
    {
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > MAX_UPDATE_PACKAGE_BYTES {
            drop(file);
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err("远程更新包超过 2GB，已取消下载".to_string());
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|error| format!("写入远程更新包失败：{error}"))?;
    }
    file.flush()
        .await
        .map_err(|error| format!("完成远程更新包失败：{error}"))?;
    drop(file);
    if let Some(expected) = expected_sha256 {
        let expected = expected.trim().to_ascii_lowercase();
        if expected.len() != 64 || !expected.chars().all(|value| value.is_ascii_hexdigit()) {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err("远程更新命令缺少有效的 SHA-256 校验值".to_string());
        }
        if hex::encode(hasher.finalize()) != expected {
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err("远程更新包校验失败，已取消安装".to_string());
        }
    }
    tokio::fs::rename(&temporary, &target)
        .await
        .map_err(|error| format!("完成远程更新包失败：{error}"))?;
    Ok(target)
}

#[cfg(target_os = "windows")]
fn schedule_remote_update_install(app: &tauri::AppHandle, package: &Path) -> Result<(), String> {
    let current_executable =
        std::env::current_exe().map_err(|error| format!("读取当前程序路径失败：{error}"))?;
    let update_root = package
        .parent()
        .ok_or_else(|| "更新包路径无效".to_string())?;
    let script = update_root.join("install-update.ps1");
    let extension = package
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let process_id = std::process::id();
    let install_command = match extension.as_str() {
        "exe" => format!(
            "Start-Process -FilePath '{}' -ArgumentList '/S' -Wait",
            package.to_string_lossy().replace('\'', "''")
        ),
        "msi" => format!(
            "Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i','{}','/qn','/norestart') -Wait",
            package.to_string_lossy().replace('\'', "''")
        ),
        "zip" => {
            let root = running_portable_root()
                .ok_or_else(|| "ZIP 更新包只能用于绿色版 LanChat".to_string())?;
            format!(
                "$staging=Join-Path '{}' 'staging'; Expand-Archive -LiteralPath '{}' -DestinationPath $staging -Force; $payload=Get-ChildItem -LiteralPath $staging -Directory | Select-Object -First 1; if($null -eq $payload){{throw '更新包结构无效'}}; Copy-Item -Path (Join-Path $payload.FullName '*') -Destination '{}' -Recurse -Force",
                update_root.to_string_lossy().replace('\'', "''"),
                package.to_string_lossy().replace('\'', "''"),
                root.to_string_lossy().replace('\'', "''")
            )
        }
        _ => return Err("仅支持 EXE、MSI 或 ZIP 更新包".to_string()),
    };
    let script_text = format!(
        "$ErrorActionPreference='Stop'\n$deadline=(Get-Date).AddSeconds(60)\nwhile(Get-Process -Id {process_id} -ErrorAction SilentlyContinue){{if((Get-Date)-ge $deadline){{throw '等待 LanChat 退出超时'}};Start-Sleep -Milliseconds 250}}\n{install_command}\nStart-Process -FilePath '{}'\n",
        current_executable.to_string_lossy().replace('\'', "''")
    );
    std::fs::write(&script, script_text)
        .map_err(|error| format!("生成远程更新脚本失败：{error}"))?;
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script)
        .spawn()
        .map_err(|error| format!("启动远程更新程序失败：{error}"))?;
    app.exit(0);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn schedule_remote_update_install(_app: &tauri::AppHandle, _package: &Path) -> Result<(), String> {
    Err("定向自动安装当前仅支持 Windows".to_string())
}

#[tauri::command]
async fn send_admin_remote_update(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    target_version: String,
    package_path: Option<String>,
) -> Result<AdminRemoteUpdateFrame, String> {
    ensure_super_admin_session(&state)?;
    let target_device_id = target_device_id.trim().to_ascii_lowercase();
    if target_device_id.is_empty() {
        return Err("请选择要强制更新的在线设备".to_string());
    }
    let target_version = normalized_remote_update_version(&target_version)?;
    let target_peer = state
        .storage
        .get_peer(&target_device_id)?
        .filter(|peer| peer.online)
        .ok_or_else(|| "目标设备不在线，无法下发强制更新".to_string())?;
    if target_peer.device_id == state.storage.get_or_create_profile()?.device_id {
        return Err("不能向本机下发远程强制更新".to_string());
    }
    let (package, package_sha256) = match package_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        Some(path) => {
            let path = PathBuf::from(path);
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !matches!(extension.as_str(), "exe" | "msi" | "zip") {
                return Err("更新包仅支持 EXE、MSI 或 ZIP".to_string());
            }
            let hash = sha256_file(&path)?;
            (Some(state.file_server.share_file(path)?), Some(hash))
        }
        None => (None, None),
    };
    let profile = state.storage.get_or_create_profile()?;
    let frame = AdminRemoteUpdateFrame {
        command_id: Uuid::new_v4().to_string(),
        target_device_id: target_device_id.clone(),
        target_version,
        package,
        package_sha256,
        issued_by_device_id: profile.device_id,
        issued_by_nickname: profile.nickname,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    if !state
        .network
        .send_admin_remote_update(app, &target_device_id, frame.clone())
        .await?
    {
        return Err("远程强制更新未送达，请确认目标设备在线".to_string());
    }
    Ok(frame)
}

#[tauri::command]
async fn execute_admin_remote_update(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    command: AdminRemoteUpdateFrame,
) -> Result<(), String> {
    let profile = state.storage.get_or_create_profile()?;
    if command.target_device_id != profile.device_id {
        return Err("远程更新目标与本机不匹配".to_string());
    }
    let target_version = normalized_remote_update_version(&command.target_version)?;
    if target_version == local_app_version_info().version {
        return Ok(());
    }
    let (url, file_name, expected_sha256) = if let Some(package) = command.package {
        let expected = command
            .package_sha256
            .as_deref()
            .ok_or_else(|| "局域网更新包缺少 SHA-256 校验值".to_string())?;
        (package.url, package.name, Some(expected.to_string()))
    } else {
        let tag = format!("v{target_version}");
        let api_url =
            format!("https://api.github.com/repos/{UPDATE_REPOSITORY}/releases/tags/{tag}");
        let client = update_http_client();
        let release = authorized_update_request(&client, &api_url)
            .send()
            .await
            .map_err(|error| format!("查询指定版本失败：{error}"))?
            .error_for_status()
            .map_err(|error| format!("查询指定版本失败：{error}"))?
            .json::<GithubRelease>()
            .await
            .map_err(|error| format!("解析指定版本失败：{error}"))?;
        let update = build_update_result(release).await;
        let url = if is_portable_runtime() {
            update.downloads.windows_portable
        } else {
            update.downloads.windows_installer
        }
        .ok_or_else(|| "指定版本没有适用于当前 Windows 客户端的安装包".to_string())?;
        let file_name = reqwest::Url::parse(&url)
            .ok()
            .and_then(|url| url.path_segments()?.next_back().map(str::to_string))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| {
                if is_portable_runtime() {
                    "lanchat-update.zip"
                } else {
                    "lanchat-update.exe"
                }
                .to_string()
            });
        let hash = if is_portable_runtime() {
            update.downloads.windows_portable_sha256
        } else {
            None
        };
        (url, file_name, hash)
    };
    let package =
        download_remote_update_package(&url, &file_name, expected_sha256.as_deref()).await?;
    schedule_remote_update_install(&app, &package)
}

#[tauri::command]
async fn install_portable_update(
    app: tauri::AppHandle,
    download_url: String,
    sha256: String,
) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, download_url, sha256);
        return Err("绿色版自动更新暂仅支持 Windows".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let root = running_portable_root().ok_or_else(|| "当前不是绿色版运行环境".to_string())?;
        let download_url = download_url.trim().to_string();
        if !(download_url.starts_with("https://github.com/")
            || download_url.starts_with("https://objects.githubusercontent.com/"))
        {
            return Err("绿色版更新链接不合法".to_string());
        }
        let sha256 = sha256.trim().to_ascii_lowercase();
        if sha256.len() != 64 || !sha256.chars().all(|value| value.is_ascii_hexdigit()) {
            return Err("绿色版更新缺少有效的 SHA-256 校验值".to_string());
        }
        let bytes = update_http_client()
            .get(&download_url)
            .header("User-Agent", "LanChat")
            .send()
            .await
            .map_err(|error| format!("下载绿色版更新失败：{error}"))?
            .error_for_status()
            .map_err(|error| format!("下载绿色版更新失败：{error}"))?
            .bytes()
            .await
            .map_err(|error| format!("读取绿色版更新失败：{error}"))?;
        let actual = hex::encode(sha2::Sha256::digest(&bytes));
        if actual != sha256 {
            return Err("绿色版更新校验失败，已取消替换".to_string());
        }
        let update_root = std::env::temp_dir().join(format!("lanchat-update-{}", Uuid::new_v4()));
        let app_process_id = std::process::id();
        std::fs::create_dir_all(&update_root)
            .map_err(|error| format!("创建更新临时目录失败：{error}"))?;
        let archive = update_root.join("lanchat-update.zip");
        let script = update_root.join("apply-update.ps1");
        std::fs::write(&archive, bytes).map_err(|error| format!("保存绿色版更新失败：{error}"))?;
        let script_text = format!(
            r#"
$ErrorActionPreference = 'Stop'
$root = '{root}'
$archive = '{archive}'
$appProcessId = {app_process_id}
$staging = Join-Path (Split-Path $archive -Parent) 'staging'
$backup = Join-Path (Split-Path $archive -Parent) 'backup'
try {{
  $deadline = (Get-Date).AddSeconds(45)
  while (Get-Process -Id $appProcessId -ErrorAction SilentlyContinue) {{
    if ((Get-Date) -ge $deadline) {{ throw '等待旧版 LanChat 退出超时' }}
    Start-Sleep -Milliseconds 250
  }}
  Expand-Archive -LiteralPath $archive -DestinationPath $staging -Force
  $payload = Get-ChildItem -LiteralPath $staging -Directory | Select-Object -First 1
  if ($null -eq $payload) {{ throw '更新包结构无效' }}
  New-Item -ItemType Directory -Force $backup | Out-Null
  Copy-Item -Path (Join-Path $root '*') -Destination $backup -Recurse -Force
  Copy-Item -Path (Join-Path $payload.FullName '*') -Destination $root -Recurse -Force
  Start-Process -FilePath (Join-Path $root 'lanchat.exe')
}} catch {{
  if (Test-Path -LiteralPath $backup) {{ Copy-Item -Path (Join-Path $backup '*') -Destination $root -Recurse -Force }}
}}
"#,
            root = root.to_string_lossy().replace('\'', "''"),
            archive = archive.to_string_lossy().replace('\'', "''"),
            app_process_id = app_process_id
        );
        std::fs::write(&script, script_text)
            .map_err(|error| format!("生成绿色版更新脚本失败：{error}"))?;
        std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(&script)
            .spawn()
            .map_err(|error| format!("启动绿色版更新器失败：{error}"))?;
        app.exit(0);
        Ok(())
    }
}

#[cfg(test)]
mod platform_info_tests {
    use super::*;

    #[test]
    fn normalizes_windows_system_proxy_values_for_the_updater() {
        assert_eq!(
            normalize_update_proxy("http=127.0.0.1:7890;https=127.0.0.1:7891"),
            Some("http://127.0.0.1:7891".to_string())
        );
        assert_eq!(
            normalize_update_proxy("127.0.0.1:7890"),
            Some("http://127.0.0.1:7890".to_string())
        );
        assert_eq!(
            normalize_update_proxy("socks5://127.0.0.1:1080"),
            Some("socks5://127.0.0.1:1080".to_string())
        );
    }

    #[test]
    fn platform_info_matches_compiled_target() {
        let info = platform_info_value();

        assert!(!info.os.is_empty());
        assert_eq!(
            info.windows_firewall_repair_supported,
            cfg!(target_os = "windows")
        );
        assert_eq!(
            info.global_shortcut_requires_permission,
            cfg!(target_os = "macos")
        );
    }
}

fn can_manage_private_channel(
    channel_owner_device_id: &str,
    profile_device_id: &str,
    super_admin: bool,
) -> bool {
    super_admin || channel_owner_device_id == profile_device_id
}

#[tauri::command]
fn get_profile(state: State<'_, AppState>) -> Result<Profile, String> {
    state.storage.get_or_create_profile()
}

#[tauri::command]
fn update_profile(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    nickname: String,
    listen_port: u16,
    avatar: Option<String>,
) -> Result<Profile, String> {
    let nickname = nickname.trim();
    if nickname.is_empty() {
        return Err("昵称不能为空".to_string());
    }
    if let Some(value) = avatar
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let encoded = value
            .split_once(',')
            .map(|(_, payload)| payload)
            .unwrap_or(value);
        let approx_bytes = encoded.len().saturating_mul(3) / 4;
        if approx_bytes > 5 * 1024 * 1024 {
            return Err("头像图片不能超过 5M".to_string());
        }
    }
    let profile = state
        .storage
        .update_profile(nickname, listen_port, avatar)?;
    state.network.broadcast_profile_status(app, &profile)?;
    Ok(profile)
}

#[tauri::command]
fn list_peers(state: State<'_, AppState>) -> Result<Vec<Peer>, String> {
    state.storage.list_peers()
}
#[tauri::command]
fn update_peer_note(
    state: State<'_, AppState>,
    device_id: String,
    note: String,
) -> Result<Peer, String> {
    state.storage.update_peer_note(&device_id, &note)?;
    state
        .storage
        .get_peer(&device_id)?
        .ok_or_else(|| "保存备注后未找到设备".to_string())
}
#[tauri::command]
fn delete_peer(state: State<'_, AppState>, device_id: String) -> Result<(), String> {
    if device_id.trim().is_empty() {
        return Err("请选择要删除的设备".to_string());
    }
    state.storage.delete_peer(&device_id)
}

#[tauri::command]
async fn admin_rename_peer(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    nickname: String,
    nickname_locked: Option<bool>,
    use_system_username: Option<bool>,
) -> Result<Peer, String> {
    let nickname = nickname.trim().to_string();
    let use_system_username = use_system_username.unwrap_or(false);
    if target_device_id.trim().is_empty() {
        return Err("请选择要修改的设备".to_string());
    }
    if nickname.is_empty() && !use_system_username {
        return Err("昵称不能为空".to_string());
    }
    state
        .network
        .send_admin_nickname(
            app,
            target_device_id.clone(),
            nickname.clone(),
            nickname_locked,
            use_system_username,
        )
        .await?;
    let mut peer = state
        .storage
        .get_peer(&target_device_id)?
        .ok_or_else(|| "设备已发送修改，但本地列表未找到该设备".to_string())?;
    if !use_system_username {
        peer.nickname = nickname;
    }
    if let Some(locked) = nickname_locked {
        peer.nickname_locked = locked;
    }
    peer.last_seen_at = chrono::Utc::now().timestamp_millis();
    state.storage.upsert_peer(&peer)?;
    Ok(peer)
}

#[tauri::command]
async fn connect_peer(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    address: String,
    port: u16,
) -> Result<Peer, String> {
    if address.trim().is_empty() {
        return Err("请输入对方 IP 地址".to_string());
    }
    state
        .network
        .connect_peer(app, address.trim().to_string(), port)
        .await
}

#[tauri::command]
fn list_conversations(state: State<'_, AppState>) -> Result<Vec<Conversation>, String> {
    ensure_full_client(&state, "聊天和频道")?;
    state.storage.list_conversations()
}

#[tauri::command]
fn list_channel_members(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ChannelMember>, String> {
    ensure_full_client(&state, "频道成员")?;
    if conversation_id.trim().is_empty() {
        return Err("请选择频道".to_string());
    }
    state.storage.list_channel_members(&conversation_id)
}

#[tauri::command]
async fn create_private_channel(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
    title: String,
    _member_device_ids: Vec<String>,
) -> Result<Conversation, String> {
    ensure_full_client(&state, "创建私有频道")?;
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err("频道名称不能为空".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let members = vec![ChannelMemberSeed {
        device_id: profile.device_id.clone(),
        nickname: profile.nickname.clone(),
        avatar: profile.avatar.clone(),
    }];
    let channel_id = format!("private-{}", Uuid::new_v4());
    let now = chrono::Utc::now().timestamp_millis();
    let conversation = state.storage.upsert_private_channel(
        &channel_id,
        &title,
        &profile.device_id,
        &profile.nickname,
        &generate_channel_key(),
        CHANNEL_KEY_VERSION,
        &members,
        now,
    )?;
    Ok(conversation)
}

#[tauri::command]
async fn invite_private_channel_members(
    _app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    member_device_ids: Vec<String>,
    super_admin: bool,
) -> Result<Vec<ChannelMember>, String> {
    ensure_full_client(&state, "邀请频道成员")?;
    let _channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    let is_member = state
        .storage
        .list_channel_members(&conversation_id)?
        .iter()
        .any(|member| member.device_id == profile.device_id);
    if !super_admin && !is_member {
        return Err("只有频道成员或超管可以邀请成员".to_string());
    }
    let _ = member_device_ids;
    state.storage.list_channel_members(&conversation_id)
}

#[tauri::command]
async fn remove_private_channel_member(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    member_device_id: String,
    super_admin: bool,
) -> Result<Vec<ChannelMember>, String> {
    ensure_full_client(&state, "频道成员管理")?;
    let channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    if !can_manage_private_channel(&channel.owner_device_id, &profile.device_id, super_admin) {
        return Err("只有频道群主或超管可以移除成员".to_string());
    }
    if member_device_id == profile.device_id || member_device_id == channel.owner_device_id {
        return Err("不能移除频道群主".to_string());
    }
    state
        .storage
        .remove_private_channel_member(&conversation_id, &member_device_id)?;
    let _ = state
        .network
        .send_admin_channel_control(
            app,
            member_device_id,
            conversation_id.clone(),
            "remove".to_string(),
            false,
        )
        .await;
    state.storage.list_channel_members(&conversation_id)
}

#[tauri::command]
async fn leave_private_channel(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<(), String> {
    ensure_full_client(&state, "退出频道")?;
    let channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    if profile.device_id == channel.owner_device_id {
        return Err("群主不能退出频道，请使用解散频道".to_string());
    }
    if !state
        .storage
        .is_private_channel_member(&conversation_id, &profile.device_id)?
    {
        return Err("你不是该频道成员".to_string());
    }
    state
        .network
        .send_admin_channel_control(
            app,
            channel.owner_device_id.clone(),
            conversation_id.clone(),
            "leave".to_string(),
            false,
        )
        .await?;
    state.storage.delete_private_channel(&conversation_id)
}

#[tauri::command]
async fn set_private_channel_member_muted(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    member_device_id: String,
    muted: bool,
    super_admin: bool,
) -> Result<Vec<ChannelMember>, String> {
    ensure_full_client(&state, "频道禁言")?;
    let channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    if !can_manage_private_channel(&channel.owner_device_id, &profile.device_id, super_admin) {
        return Err("只有频道群主或超管可以设置禁言".to_string());
    }
    if member_device_id == profile.device_id || member_device_id == channel.owner_device_id {
        return Err("不能禁言频道群主".to_string());
    }
    state
        .storage
        .set_private_channel_member_muted(&conversation_id, &member_device_id, muted)?;
    let action = if muted { "mute" } else { "unmute" }.to_string();
    let _ = state
        .network
        .send_admin_channel_control(
            app,
            member_device_id,
            conversation_id.clone(),
            action,
            muted,
        )
        .await;
    state.storage.list_channel_members(&conversation_id)
}

#[tauri::command]
async fn dissolve_private_channel(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    super_admin: bool,
) -> Result<(), String> {
    ensure_full_client(&state, "解散频道")?;
    let channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    if !can_manage_private_channel(&channel.owner_device_id, &profile.device_id, super_admin) {
        return Err("只有频道群主或超管可以解散频道".to_string());
    }
    let members = state.storage.list_channel_members(&conversation_id)?;
    for member in members {
        if member.device_id == profile.device_id {
            continue;
        }
        let _ = state
            .network
            .send_admin_channel_control(
                app.clone(),
                member.device_id,
                conversation_id.clone(),
                "dissolve".to_string(),
                false,
            )
            .await;
    }
    state.storage.delete_private_channel(&conversation_id)
}

#[tauri::command]
async fn admin_mute_channel_member(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    target_device_id: String,
    muted: bool,
) -> Result<(), String> {
    ensure_full_client(&state, "频道管控")?;
    let profile = state.storage.get_or_create_profile()?;
    if target_device_id == profile.device_id {
        return Err("不能禁言自己".to_string());
    }
    let now = chrono::Utc::now().timestamp_millis();
    state
        .storage
        .set_channel_mute(&conversation_id, &target_device_id, muted, now)?;
    let action = if muted { "mute" } else { "unmute" }.to_string();
    state
        .network
        .send_admin_channel_control(app, target_device_id, conversation_id, action, muted)
        .await
}

#[tauri::command]
fn build_private_channel_invite_card(
    state: State<'_, AppState>,
    conversation_id: String,
    super_admin: bool,
) -> Result<PrivateChannelInviteCardPayload, String> {
    ensure_full_client(&state, "频道邀请")?;
    let channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    let is_member = state
        .storage
        .list_channel_members(&conversation_id)?
        .iter()
        .any(|member| member.device_id == profile.device_id);
    if !super_admin && !is_member {
        return Err("只有频道成员或超管可以邀请成员".to_string());
    }
    let members = state
        .storage
        .list_channel_members(&conversation_id)?
        .into_iter()
        .map(|member| ChannelMemberSeed {
            device_id: member.device_id,
            nickname: member.nickname,
            avatar: member.avatar,
        })
        .collect::<Vec<_>>();
    Ok(PrivateChannelInviteCardPayload {
        channel_id: channel.id,
        title: channel.title,
        owner_device_id: channel.owner_device_id,
        owner_nickname: channel.owner_nickname,
        channel_key: channel.channel_key,
        key_version: channel.key_version,
        members,
        created_at: chrono::Utc::now().timestamp_millis(),
    })
}

#[tauri::command]
async fn accept_private_channel_invite(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    invite: PrivateChannelInviteCardPayload,
) -> Result<Conversation, String> {
    ensure_full_client(&state, "加入私有频道")?;
    if invite.channel_id.trim().is_empty() || invite.title.trim().is_empty() {
        return Err("频道邀请无效".to_string());
    }
    if invite.channel_key.trim().is_empty() {
        return Err("频道邀请缺少密钥".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let now = chrono::Utc::now().timestamp_millis();
    let updated_at = if invite.created_at > 0 {
        invite.created_at
    } else {
        now
    };
    let mut members = if invite.members.is_empty() {
        vec![ChannelMemberSeed {
            device_id: invite.owner_device_id.clone(),
            nickname: invite.owner_nickname.clone(),
            avatar: None,
        }]
    } else {
        invite.members.clone()
    };
    members.push(ChannelMemberSeed {
        device_id: profile.device_id.clone(),
        nickname: profile.nickname.clone(),
        avatar: profile.avatar.clone(),
    });
    let conversation = state.storage.upsert_private_channel(
        &invite.channel_id,
        &invite.title,
        &invite.owner_device_id,
        &invite.owner_nickname,
        &invite.channel_key,
        invite.key_version,
        &members,
        updated_at,
    )?;
    if invite.owner_device_id != profile.device_id {
        let _ = state
            .network
            .send_admin_channel_control(
                app,
                invite.owner_device_id.clone(),
                invite.channel_id.clone(),
                "join".to_string(),
                false,
            )
            .await;
    }
    Ok(conversation)
}

#[tauri::command]
fn is_channel_muted(state: State<'_, AppState>, conversation_id: String) -> Result<bool, String> {
    let profile = state.storage.get_or_create_profile()?;
    state
        .storage
        .is_channel_muted(&conversation_id, &profile.device_id)
}

#[tauri::command]
async fn broadcast_channel_notice(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    notice: String,
) -> Result<(), String> {
    ensure_full_client(&state, "频道公告")?;
    state
        .network
        .broadcast_channel_notice(app, conversation_id, notice)
        .await
}

#[tauri::command]
fn list_messages(
    state: State<'_, AppState>,
    conversation_id: String,
    before_created_at: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<Message>, String> {
    ensure_full_client(&state, "聊天记录")?;
    state
        .storage
        .list_messages_page(&conversation_id, before_created_at, limit.unwrap_or(60))
}

#[tauri::command]
async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
) -> Result<Message, String> {
    ensure_full_client(&state, "聊天")?;
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("消息内容不能为空".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let now = chrono::Utc::now().timestamp_millis();
    let message = Message {
        id: Uuid::new_v4().to_string(),
        conversation_id: if conversation_id.trim().is_empty() {
            DEFAULT_GROUP_ID.to_string()
        } else {
            conversation_id
        },
        sender_device_id: profile.device_id,
        content,
        message_type: MessageType::Text,
        file_meta: None,
        status: storage::MessageStatus::Sending,
        simulation: None,
        created_at: now,
    };
    state.network.send_message(app, message.clone()).await?;
    Ok(message)
}

#[tauri::command]
fn authenticate_super_admin(state: State<'_, AppState>, password: String) -> Result<bool, String> {
    let actual = format!("{:x}", md5::compute(password.as_bytes()));
    if !actual.eq_ignore_ascii_case(SUPER_ADMIN_PASSWORD_MD5) {
        return Err("验证失败".to_string());
    }
    *state
        .super_admin_session
        .lock()
        .map_err(|_| "超级管理员会话状态异常".to_string())? = true;
    Ok(true)
}

#[tauri::command]
fn clear_super_admin_session(state: State<'_, AppState>) -> Result<(), String> {
    *state
        .super_admin_session
        .lock()
        .map_err(|_| "超级管理员会话状态异常".to_string())? = false;
    Ok(())
}

#[tauri::command]
fn is_super_admin_authenticated(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(*state
        .super_admin_session
        .lock()
        .map_err(|_| "超级管理员会话状态异常".to_string())?)
}

fn ensure_super_admin_session(state: &AppState) -> Result<(), String> {
    if *state
        .super_admin_session
        .lock()
        .map_err(|_| "超级管理员会话状态异常".to_string())?
    {
        Ok(())
    } else {
        Err("需要超级管理员权限".to_string())
    }
}

#[tauri::command]
async fn simulate_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    simulated_device_id: String,
    conversation_id: String,
    content: String,
    display_simulation_label: bool,
) -> Result<Message, String> {
    ensure_full_client(&state, "超管模拟发送")?;
    ensure_super_admin_session(&state)?;
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("消息内容不能为空".to_string());
    }
    let simulated = state
        .storage
        .get_peer(&simulated_device_id)?
        .ok_or_else(|| "只能选择已发现的设备作为模拟身份".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    let now = chrono::Utc::now().timestamp_millis();
    let conversation_id = if conversation_id.trim().is_empty() {
        DEFAULT_GROUP_ID.to_string()
    } else {
        conversation_id
    };
    let simulation = SimulationMeta {
        operator_device_id: profile.device_id.clone(),
        operator_nickname: profile.nickname.clone(),
        display_label: display_simulation_label,
        created_at: now,
    };
    let message = Message {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.clone(),
        sender_device_id: simulated.device_id.clone(),
        content: content.clone(),
        message_type: MessageType::Text,
        file_meta: None,
        status: storage::MessageStatus::Sending,
        simulation: Some(simulation),
        created_at: now,
    };
    state.network.send_message(app, message.clone()).await?;
    state.storage.save_simulation_audit(&SimulationAudit {
        id: Uuid::new_v4().to_string(),
        operator_device_id: profile.device_id,
        operator_nickname: profile.nickname,
        simulated_device_id: simulated.device_id,
        action_kind: "message".to_string(),
        target_id: Some(conversation_id),
        display_label: display_simulation_label,
        content,
        created_at: now,
    })?;
    Ok(message)
}

#[tauri::command]
fn save_system_notice(
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
) -> Result<Message, String> {
    ensure_full_client(&state, "系统聊天通知")?;
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err("系统通知不能为空".to_string());
    }
    let now = chrono::Utc::now().timestamp_millis();
    let message = Message {
        id: Uuid::new_v4().to_string(),
        conversation_id: if conversation_id.trim().is_empty() {
            DEFAULT_GROUP_ID.to_string()
        } else {
            conversation_id
        },
        sender_device_id: "system".to_string(),
        content,
        message_type: MessageType::System,
        file_meta: None,
        status: storage::MessageStatus::Delivered,
        simulation: None,
        created_at: now,
    };
    state.storage.save_message(&message)?;
    Ok(message)
}

#[tauri::command]
async fn recall_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    message_id: String,
) -> Result<Message, String> {
    ensure_full_client(&state, "消息撤回")?;
    let message = state
        .storage
        .update_message_after_recall(&message_id)?
        .ok_or_else(|| "未找到要撤回的消息".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    state
        .network
        .broadcast_message_recall(
            app,
            MessageRecallFrame {
                message_id: message.id.clone(),
                conversation_id: message.conversation_id.clone(),
                sender_device_id: profile.device_id,
                recalled_at: chrono::Utc::now().timestamp_millis(),
            },
        )
        .await?;
    Ok(message)
}

#[tauri::command]
async fn send_file_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    path: String,
) -> Result<Message, String> {
    ensure_full_client(&state, "文件消息")?;
    let file_meta = state
        .file_server
        .share_file(std::path::PathBuf::from(path))?;
    send_rich_message(
        app,
        state,
        conversation_id,
        MessageType::File,
        format!("文件：{}", file_meta.name),
        Some(file_meta),
    )
    .await
}

#[tauri::command]
async fn send_pasted_image_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    file_name: String,
    bytes: Vec<u8>,
    mime_type: String,
) -> Result<Message, String> {
    ensure_full_client(&state, "图片消息")?;
    const MAX_PASTE_IMAGE_BYTES: usize = 20 * 1024 * 1024;
    if bytes.is_empty() {
        return Err("粘贴的图片为空".to_string());
    }
    if bytes.len() > MAX_PASTE_IMAGE_BYTES {
        return Err("粘贴图片不能超过 20MB".to_string());
    }
    let safe_mime = mime_type.trim();
    if !safe_mime.starts_with("image/") {
        return Err("只能粘贴图片发送".to_string());
    }
    let safe_name = if file_name.trim().is_empty() {
        "paste-image.png".to_string()
    } else {
        file_name.trim().replace(['\\', '/'], "_")
    };
    let dir = std::env::temp_dir().join("lanchat-paste-images");
    std::fs::create_dir_all(&dir).map_err(|err| format!("创建图片缓存目录失败：{err}"))?;
    let path = dir.join(format!("{}-{}", Uuid::new_v4(), safe_name));
    std::fs::write(&path, bytes).map_err(|err| format!("保存粘贴图片失败：{err}"))?;
    let file_meta =
        state
            .file_server
            .share_file_with_options(path, Some(safe_mime.to_string()), None)?;
    send_rich_message(
        app,
        state,
        conversation_id,
        MessageType::File,
        "图片消息".to_string(),
        Some(file_meta),
    )
    .await
}

fn preview_media_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| format!("读取图片缓存目录失败：{err}"))?;
    Ok(root.join("cache").join("preview-media"))
}

fn preview_cache_file_name(message_id: &str, file_name: &str) -> String {
    let safe_id: String = message_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| {
            matches!(
                value.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
            )
        })
        .unwrap_or_else(|| "bin".to_string());
    format!(
        "{}.{extension}",
        if safe_id.is_empty() {
            "preview"
        } else {
            &safe_id
        }
    )
}

fn preview_media_cache_info(app: &tauri::AppHandle) -> Result<PreviewMediaCacheInfo, String> {
    let directory = preview_media_cache_dir(app)?;
    let mut file_count = 0_u64;
    let mut total_bytes = 0_u64;
    if directory.exists() {
        for entry in
            std::fs::read_dir(&directory).map_err(|err| format!("读取图片缓存失败：{err}"))?
        {
            let entry = entry.map_err(|err| format!("读取图片缓存项失败：{err}"))?;
            let metadata = entry
                .metadata()
                .map_err(|err| format!("读取图片缓存信息失败：{err}"))?;
            if metadata.is_file() {
                file_count += 1;
                total_bytes += metadata.len();
            }
        }
    }
    Ok(PreviewMediaCacheInfo {
        directory: directory.to_string_lossy().to_string(),
        file_count,
        total_bytes,
    })
}

fn enforce_preview_media_cache_limit(directory: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    let mut total_bytes = 0_u64;
    if !directory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory).map_err(|err| format!("读取图片缓存失败：{err}"))?
    {
        let entry = entry.map_err(|err| format!("读取图片缓存项失败：{err}"))?;
        let metadata = entry
            .metadata()
            .map_err(|err| format!("读取图片缓存信息失败：{err}"))?;
        if metadata.is_file() {
            total_bytes += metadata.len();
            files.push((
                metadata
                    .modified()
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                metadata.len(),
                entry.path(),
            ));
        }
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, size, path) in files {
        if total_bytes <= PREVIEW_MEDIA_CACHE_TOTAL_LIMIT_BYTES {
            break;
        }
        std::fs::remove_file(&path).map_err(|err| format!("清理过期图片缓存失败：{err}"))?;
        total_bytes = total_bytes.saturating_sub(size);
    }
    Ok(())
}

fn touch_preview_media_cache_file(path: &Path) {
    if let Ok(file) = OpenOptions::new().write(true).open(path) {
        let _ =
            file.set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::now()));
    }
}

#[tauri::command]
async fn cache_preview_media(
    app: tauri::AppHandle,
    message_id: String,
    url: String,
    file_name: String,
) -> Result<String, String> {
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("图片预览地址无效".to_string());
    }
    let directory = preview_media_cache_dir(&app)?;
    std::fs::create_dir_all(&directory).map_err(|err| format!("创建图片缓存目录失败：{err}"))?;
    let target = directory.join(preview_cache_file_name(&message_id, &file_name));
    if target.is_file() {
        touch_preview_media_cache_file(&target);
        return Ok(target.to_string_lossy().to_string());
    }
    let response = reqwest::get(url)
        .await
        .map_err(|err| format!("下载图片预览失败：{err}"))?;
    if !response.status().is_success() {
        return Err(format!("下载图片预览失败：HTTP {}", response.status()));
    }
    if response.content_length().unwrap_or(0) > PREVIEW_MEDIA_CACHE_MAX_BYTES as u64 {
        return Err("图片预览超过 30MB，未缓存到本机".to_string());
    }
    let temporary = target.with_extension("downloading");
    let mut file = tokio::fs::File::create(&temporary)
        .await
        .map_err(|err| format!("创建图片缓存失败：{err}"))?;
    let mut downloaded = 0usize;
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|err| format!("读取图片预览失败：{err}"))?
    {
        downloaded = downloaded.saturating_add(chunk.len());
        if downloaded > PREVIEW_MEDIA_CACHE_MAX_BYTES {
            drop(file);
            let _ = tokio::fs::remove_file(&temporary).await;
            return Err("图片预览超过 30MB，未缓存到本机".to_string());
        }
        file.write_all(&chunk)
            .await
            .map_err(|err| format!("写入图片缓存失败：{err}"))?;
    }
    file.flush()
        .await
        .map_err(|err| format!("完成图片缓存失败：{err}"))?;
    drop(file);
    std::fs::rename(&temporary, &target).map_err(|err| format!("完成图片缓存失败：{err}"))?;
    enforce_preview_media_cache_limit(&directory)?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
fn get_preview_media_cache_info(app: tauri::AppHandle) -> Result<PreviewMediaCacheInfo, String> {
    preview_media_cache_info(&app)
}

#[tauri::command]
fn clear_preview_media_cache(app: tauri::AppHandle) -> Result<PreviewMediaCacheInfo, String> {
    let directory = preview_media_cache_dir(&app)?;
    if directory.exists() {
        for entry in
            std::fs::read_dir(&directory).map_err(|err| format!("读取图片缓存失败：{err}"))?
        {
            let entry = entry.map_err(|err| format!("读取图片缓存项失败：{err}"))?;
            if entry.path().is_file() {
                std::fs::remove_file(entry.path())
                    .map_err(|err| format!("清理图片缓存失败：{err}"))?;
            }
        }
    }
    preview_media_cache_info(&app)
}

#[tauri::command]
async fn send_voice_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    file_name: String,
    bytes: Vec<u8>,
    duration_ms: u64,
) -> Result<Message, String> {
    ensure_full_client(&state, "语音消息")?;
    const MAX_VOICE_MS: u64 = 60_000;
    const MAX_VOICE_BYTES: usize = 8 * 1024 * 1024;
    if duration_ms == 0 || duration_ms > MAX_VOICE_MS {
        return Err("语音长度需在 1 秒到 60 秒之间".to_string());
    }
    if bytes.is_empty() || bytes.len() > MAX_VOICE_BYTES {
        return Err("语音文件过大，请控制在 8MB 内".to_string());
    }
    let safe_name = if file_name.trim().is_empty() {
        "voice.webm"
    } else {
        file_name.trim()
    };
    let dir = std::env::temp_dir().join("lanchat-voice");
    std::fs::create_dir_all(&dir).map_err(|err| format!("创建语音缓存目录失败：{err}"))?;
    let path = dir.join(format!(
        "{}-{}",
        Uuid::new_v4(),
        safe_name.replace(['\\', '/'], "_")
    ));
    std::fs::write(&path, bytes).map_err(|err| format!("保存语音失败：{err}"))?;
    let file_meta = state.file_server.share_file_with_options(
        path,
        Some("audio/webm".to_string()),
        Some(duration_ms),
    )?;
    send_rich_message(
        app,
        state,
        conversation_id,
        MessageType::Voice,
        "语音消息".to_string(),
        Some(file_meta),
    )
    .await
}

async fn send_rich_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    message_type: MessageType,
    content: String,
    file_meta: Option<file_server::FileMeta>,
) -> Result<Message, String> {
    let profile = state.storage.get_or_create_profile()?;
    let now = chrono::Utc::now().timestamp_millis();
    let message = Message {
        id: Uuid::new_v4().to_string(),
        conversation_id: if conversation_id.trim().is_empty() {
            DEFAULT_GROUP_ID.to_string()
        } else {
            conversation_id
        },
        sender_device_id: profile.device_id,
        content,
        message_type,
        file_meta,
        status: storage::MessageStatus::Sending,
        simulation: None,
        created_at: now,
    };
    state.network.send_message(app, message.clone()).await?;
    Ok(message)
}
#[tauri::command]
async fn send_game_frame(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: Option<String>,
    frame: GameFrame,
) -> Result<(), String> {
    state
        .network
        .send_game_frame(app, target_device_id, frame)
        .await
}

#[tauri::command]
async fn send_call_signal(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    frame: CallSignalFrame,
) -> Result<(), String> {
    let profile = state.storage.get_or_create_profile()?;
    if frame.sender_device_id != profile.device_id {
        return Err("通话信令身份校验失败".to_string());
    }
    state
        .network
        .send_call_signal(app, target_device_id, frame)
        .await
}

#[tauri::command]
async fn send_quick_alert(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    content: String,
    mode: Option<String>,
    sender_credibility: Option<u8>,
) -> Result<QuickAlertFrame, String> {
    let profile = state.storage.get_or_create_profile()?;
    let mode = if mode
        .as_deref()
        .unwrap_or("normal")
        .eq_ignore_ascii_case("disco")
    {
        "disco".to_string()
    } else {
        "normal".to_string()
    };
    let frame = QuickAlertFrame {
        alert_id: Uuid::new_v4().to_string(),
        sender_device_id: profile.device_id,
        sender_nickname: profile.nickname,
        sender_address: Some(local_ip_address()),
        content: {
            let text = content.trim();
            if text.is_empty() {
                "呱呱~呱~~".to_string()
            } else {
                text.chars().take(120).collect()
            }
        },
        mode,
        simulation: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    state
        .network
        .broadcast_quick_alert(app.clone(), frame.clone())
        .await?;
    let settings = state.desktop_pet.settings();
    let credibility = sender_credibility.unwrap_or(100).min(100);
    if settings.external_push_enabled && credibility >= settings.external_push_min_credibility {
        for config in settings
            .external_push_configs
            .into_iter()
            .filter(|item| item.enabled && !item.webhook.trim().is_empty())
        {
            let notify_frame = frame.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = send_external_push_alert(config, notify_frame).await {
                    eprintln!("{error}");
                }
            });
        }
    } else if settings.external_push_enabled {
        emit_debug_log(
            &app,
            "info",
            "alert",
            "告警可信度低于外部群推送阈值，已跳过",
            Some(format!(
                "{credibility} < {}",
                settings.external_push_min_credibility
            )),
        );
    }
    Ok(frame)
}

#[tauri::command]
async fn simulate_quick_alert(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    simulated_device_id: String,
    content: String,
    mode: Option<String>,
    display_simulation_label: bool,
) -> Result<QuickAlertFrame, String> {
    ensure_super_admin_session(&state)?;
    let simulated = state
        .storage
        .get_peer(&simulated_device_id)?
        .ok_or_else(|| "只能选择已发现的设备作为模拟身份".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    let now = chrono::Utc::now().timestamp_millis();
    let mode = if mode
        .as_deref()
        .unwrap_or("normal")
        .eq_ignore_ascii_case("disco")
    {
        "disco".to_string()
    } else {
        "normal".to_string()
    };
    let content = {
        let text = content.trim();
        if text.is_empty() {
            "呱呱~呱~~".to_string()
        } else {
            text.chars().take(120).collect()
        }
    };
    let frame = QuickAlertFrame {
        alert_id: Uuid::new_v4().to_string(),
        sender_device_id: simulated.device_id.clone(),
        sender_nickname: simulated.nickname.clone(),
        sender_address: Some(simulated.address.clone()),
        content: content.clone(),
        mode: mode.clone(),
        simulation: Some(SimulationMeta {
            operator_device_id: profile.device_id.clone(),
            operator_nickname: profile.nickname.clone(),
            display_label: display_simulation_label,
            created_at: now,
        }),
        created_at: now,
    };
    state
        .network
        .broadcast_quick_alert(app, frame.clone())
        .await?;
    let settings = state.desktop_pet.settings();
    if settings.external_push_enabled {
        for config in settings
            .external_push_configs
            .into_iter()
            .filter(|item| item.enabled && !item.webhook.trim().is_empty())
        {
            let notify_frame = frame.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = send_external_push_alert(config, notify_frame).await {
                    eprintln!("{error}");
                }
            });
        }
    }
    state.storage.save_simulation_audit(&SimulationAudit {
        id: Uuid::new_v4().to_string(),
        operator_device_id: profile.device_id,
        operator_nickname: profile.nickname,
        simulated_device_id: simulated.device_id,
        action_kind: format!("alert:{mode}"),
        target_id: None,
        display_label: display_simulation_label,
        content,
        created_at: now,
    })?;
    Ok(frame)
}

#[tauri::command]
async fn send_quick_alert_feedback(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    alert_id: String,
    alert_sender_device_id: String,
    result: String,
) -> Result<QuickAlertFeedbackFrame, String> {
    let result = result.trim().to_lowercase();
    if result != "real" && result != "false" {
        return Err("请选择告警反馈：真实或误报".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let frame = QuickAlertFeedbackFrame {
        alert_id,
        alert_sender_device_id,
        responder_device_id: profile.device_id,
        responder_nickname: profile.nickname,
        result,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    state
        .network
        .broadcast_quick_alert_feedback(app, frame.clone())
        .await?;
    Ok(frame)
}

#[tauri::command]
async fn send_quick_alert_trust_reset(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
) -> Result<QuickAlertTrustResetFrame, String> {
    let target_device_id = target_device_id.trim().to_lowercase();
    if target_device_id.is_empty() {
        return Err("请选择要清空可信度的设备".to_string());
    }
    let profile = state.storage.get_or_create_profile()?;
    let frame = QuickAlertTrustResetFrame {
        target_device_id,
        issued_by_device_id: profile.device_id,
        issued_by_nickname: profile.nickname,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    state
        .network
        .broadcast_quick_alert_trust_reset(app.clone(), frame.clone())
        .await?;
    app.emit("quick_alert_trust_reset_received", &frame).ok();
    Ok(frame)
}

#[tauri::command]
async fn send_admin_disco_mode(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    duration_ms: Option<u64>,
) -> Result<AdminDiscoModeFrame, String> {
    let target_device_id = target_device_id.trim().to_lowercase();
    if target_device_id.is_empty() {
        return Err("请选择要下发蹦迪模式的设备".to_string());
    }
    let frame = state
        .network
        .send_admin_disco_mode(app.clone(), target_device_id, duration_ms.unwrap_or(60_000))
        .await?;
    app.emit("admin_disco_mode_received", &frame).ok();
    Ok(frame)
}

#[tauri::command]
async fn send_admin_alert_mode(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    mode: String,
) -> Result<AdminAlertModeFrame, String> {
    let target_device_id = target_device_id.trim().to_lowercase();
    if target_device_id.is_empty() {
        return Err("请选择要下发报警模式的设备".to_string());
    }
    let frame = state
        .network
        .send_admin_alert_mode(app.clone(), target_device_id, mode)
        .await?;
    app.emit("admin_alert_mode_received", &frame).ok();
    Ok(frame)
}

#[tauri::command]
async fn send_admin_alert_push_policy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
    min_credibility: u8,
    min_credibility_locked: bool,
) -> Result<AdminAlertPushPolicyFrame, String> {
    ensure_super_admin_session(&state)?;
    let target = target_device_id.trim();
    if target.is_empty() {
        return Err("请选择要下发推送配置的设备".to_string());
    }
    let frame = state
        .network
        .send_admin_alert_push_policy(
            app.clone(),
            target.to_string(),
            min_credibility.min(100),
            min_credibility_locked,
        )
        .await?;
    if frame.target_device_id == "*"
        || frame.target_device_id == state.storage.get_or_create_profile()?.device_id
    {
        let mut settings = state.desktop_pet.settings();
        settings.external_push_min_credibility = frame.min_credibility;
        settings.external_push_min_credibility_locked = frame.min_credibility_locked;
        state.desktop_pet.update_settings(settings)?;
    }
    app.emit("admin_alert_push_policy_received", &frame).ok();
    Ok(frame)
}

#[tauri::command]
async fn send_nudge(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: String,
) -> Result<protocol::NudgeFrame, String> {
    let target_device_id = target_device_id.trim();
    if target_device_id.is_empty() {
        return Err("请选择在线设备".to_string());
    }
    state
        .network
        .send_nudge(app, target_device_id.to_string())
        .await
}

#[tauri::command]
fn reveal_and_shake_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let needs_attention =
        window.is_minimized().unwrap_or(false) || !window.is_visible().unwrap_or(true);
    if needs_attention {
        show_main_window(&app, None)?;
    }
    let origin = window
        .outer_position()
        .map_err(|err| format!("读取窗口位置失败：{err}"))?;
    tauri::async_runtime::spawn(async move {
        for offset in [0, 12, -12, 12, -12, 0, 0, 12, -12, 12, -12, 0] {
            let _ = window.set_position(Position::Physical(PhysicalPosition::new(
                origin.x + offset,
                origin.y,
            )));
            tokio::time::sleep(std::time::Duration::from_millis(68)).await;
        }
    });
    Ok(())
}

#[tauri::command]
fn list_admin_notifications(
    state: State<'_, AppState>,
) -> Result<Vec<AdminNotificationRecord>, String> {
    state.storage.list_admin_notifications()
}

#[tauri::command]
async fn send_admin_notification(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_device_id: Option<String>,
    target_scope: Option<String>,
    title: String,
    content: String,
    template: Option<String>,
    support_url: Option<String>,
    display_mode: String,
    deadline_at: Option<i64>,
    timeout_policy: String,
    force_open_main_window: bool,
) -> Result<Vec<AdminNotificationRecord>, String> {
    ensure_super_admin_session(&state)?;
    let profile = state.storage.get_or_create_profile()?;
    let target_scope = target_scope.unwrap_or_else(|| "device".to_string());
    let target_device_id = target_device_id.unwrap_or_default().trim().to_lowercase();
    let title = title.trim().chars().take(60).collect::<String>();
    let content = content.trim().chars().take(1000).collect::<String>();
    if title.is_empty() || content.is_empty() {
        return Err("请填写通知标题和内容".to_string());
    }
    let display_mode = if display_mode == "requires_confirmation" {
        "requires_confirmation"
    } else {
        "dismissible"
    }
    .to_string();
    let timeout_policy = match timeout_policy.as_str() {
        "auto_release" | "keep_locked" => timeout_policy,
        _ => "manual_review".to_string(),
    };
    let targets = if target_scope == "all_online" {
        state
            .storage
            .list_peers()?
            .into_iter()
            .filter(|peer| peer.online)
            .map(|peer| peer.device_id)
            .collect::<Vec<_>>()
    } else if target_device_id.is_empty() {
        return Err("请选择要通知的设备".to_string());
    } else {
        vec![target_device_id]
    };
    if targets.is_empty() {
        return Err("当前没有可通知的在线设备".to_string());
    }
    let mut delivered = Vec::new();
    for target_device_id in targets {
        let frame = AdminNotificationFrame {
            notification_id: Uuid::new_v4().to_string(),
            target_device_id: target_device_id.clone(),
            title: title.clone(),
            content: content.clone(),
            template: template.clone().unwrap_or_default(),
            support_url: support_url.clone().filter(|value| !value.trim().is_empty()),
            display_mode: display_mode.clone(),
            deadline_at,
            timeout_policy: timeout_policy.clone(),
            force_open_main_window,
            issued_by_device_id: profile.device_id.clone(),
            issued_by_nickname: profile.nickname.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        match state
            .network
            .send_admin_notification_frame(
                app.clone(),
                &target_device_id,
                protocol::WireFrame::AdminNotification(frame.clone()),
            )
            .await
        {
            Ok(()) => delivered.push(state.storage.upsert_admin_notification(&frame)?),
            Err(error) if target_scope == "all_online" => {
                emit_debug_log(&app, "warn", "admin", "全员通知部分设备未送达", Some(error))
            }
            Err(error) => return Err(error),
        }
    }
    if delivered.is_empty() {
        return Err("通知未送达任何在线设备".to_string());
    }
    Ok(delivered)
}

#[tauri::command]
async fn submit_admin_notification(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    notification_id: String,
) -> Result<AdminNotificationRecord, String> {
    let profile = state.storage.get_or_create_profile()?;
    let submitted_at = chrono::Utc::now().timestamp_millis();
    let record = state.storage.submit_admin_notification(
        &notification_id,
        &profile.device_id,
        &profile.nickname,
        submitted_at,
    )?;
    let frame = AdminNotificationSubmissionFrame {
        notification_id: record.notification_id.clone(),
        target_device_id: profile.device_id.clone(),
        submitted_by_device_id: profile.device_id,
        submitted_by_nickname: profile.nickname,
        submitted_at,
    };
    state
        .network
        .send_admin_notification_frame(
            app,
            &record.issued_by_device_id,
            protocol::WireFrame::AdminNotificationSubmission(frame),
        )
        .await?;
    Ok(record)
}

#[tauri::command]
async fn decide_admin_notification(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    notification_id: String,
    decision: String,
) -> Result<AdminNotificationRecord, String> {
    ensure_super_admin_session(&state)?;
    let profile = state.storage.get_or_create_profile()?;
    let records = state.storage.list_admin_notifications()?;
    let original = records
        .into_iter()
        .find(|item| item.notification_id == notification_id)
        .ok_or_else(|| "未找到超管通知".to_string())?;
    if original.issued_by_device_id != profile.device_id {
        return Err("只能审核本机下发的通知".to_string());
    }
    let frame = AdminNotificationDecisionFrame {
        notification_id,
        target_device_id: original.target_device_id.clone(),
        decision,
        decided_by_device_id: profile.device_id,
        decided_by_nickname: profile.nickname,
        decided_at: chrono::Utc::now().timestamp_millis(),
    };
    let record = state.storage.decide_admin_notification(&frame)?;
    state
        .network
        .send_admin_notification_frame(
            app,
            &original.target_device_id,
            protocol::WireFrame::AdminNotificationDecision(frame),
        )
        .await?;
    Ok(record)
}

#[tauri::command]
fn list_desktop_pets(state: State<'_, AppState>) -> Result<DesktopPetRegistrySnapshot, String> {
    Ok(state.desktop_pet.snapshot())
}

#[tauri::command]
fn refresh_desktop_pets(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<DesktopPetRegistrySnapshot, String> {
    state.desktop_pet.refresh();
    state
        .desktop_pet_controller
        .set_package(state.desktop_pet.selected_package());
    let snapshot = state.desktop_pet.snapshot();
    app.emit("desktop_pet_registry_changed", &snapshot).ok();
    Ok(snapshot)
}

#[tauri::command]
fn import_desktop_pet(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    source_path: String,
) -> Result<DesktopPetPackage, String> {
    let package = state
        .desktop_pet
        .import_package(std::path::Path::new(source_path.trim()))?;
    let snapshot = state.desktop_pet.snapshot();
    app.emit("desktop_pet_registry_changed", snapshot).ok();
    Ok(package)
}

#[tauri::command]
fn remove_desktop_pet(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    pet_id: String,
) -> Result<(), String> {
    state.desktop_pet.remove_user_package(pet_id.trim())?;
    state
        .desktop_pet_controller
        .set_package(state.desktop_pet.selected_package());
    app.emit("desktop_pet_registry_changed", state.desktop_pet.snapshot())
        .ok();
    Ok(())
}

#[tauri::command]
fn select_desktop_pet(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    pet_id: String,
) -> Result<DesktopPetSettings, String> {
    let settings = state.desktop_pet.select(pet_id.trim())?;
    state
        .desktop_pet_controller
        .set_package(state.desktop_pet.selected_package());
    app.emit("desktop_pet_selected", &settings).ok();
    Ok(settings)
}

#[tauri::command]
fn get_desktop_pet_settings(state: State<'_, AppState>) -> Result<DesktopPetSettings, String> {
    Ok(state.desktop_pet.settings())
}

#[tauri::command]
fn update_desktop_pet_settings(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: DesktopPetSettings,
) -> Result<DesktopPetSettings, String> {
    let current = state.desktop_pet.settings();
    if current.external_push_min_credibility_locked
        && (settings.external_push_min_credibility != current.external_push_min_credibility
            || !settings.external_push_min_credibility_locked)
    {
        return Err("管理员已禁止本机修改告警可信度阈值".to_string());
    }
    let settings = state.desktop_pet.update_settings(settings)?;
    state.desktop_pet_controller.set_enabled(settings.enabled);
    state
        .desktop_pet_controller
        .set_package(state.desktop_pet.selected_package());
    app.emit("desktop_pet_selected", &settings).ok();
    Ok(settings)
}

#[tauri::command]
fn apply_admin_alert_push_policy(
    state: State<'_, AppState>,
    min_credibility: u8,
    min_credibility_locked: bool,
) -> Result<DesktopPetSettings, String> {
    let mut settings = state.desktop_pet.settings();
    settings.external_push_min_credibility = min_credibility.min(100);
    settings.external_push_min_credibility_locked = min_credibility_locked;
    state.desktop_pet.update_settings(settings)
}

#[tauri::command]
fn update_desktop_pet_playback_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    pet_id: String,
    configs: HashMap<String, PetStatePlaybackConfig>,
) -> Result<DesktopPetPackage, String> {
    let package = state
        .desktop_pet
        .update_playback_configs(pet_id.trim(), configs)?;
    state
        .desktop_pet_controller
        .set_package(state.desktop_pet.selected_package());
    app.emit("desktop_pet_registry_changed", state.desktop_pet.snapshot())
        .ok();
    Ok(package)
}

#[tauri::command]
fn open_desktop_pet_folder(state: State<'_, AppState>) -> Result<(), String> {
    std::fs::create_dir_all(state.desktop_pet.user_root())
        .map_err(|error| format!("创建桌宠资源目录失败：{error}"))?;
    tauri_plugin_opener::open_path(state.desktop_pet.user_root(), None::<&str>)
        .map_err(|error| format!("打开桌宠资源目录失败：{error}"))
}

#[tauri::command]
fn set_desktop_pet_enabled(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut settings = state.desktop_pet.settings();
    settings.enabled = enabled;
    state.desktop_pet.update_settings(settings)?;
    state.desktop_pet_controller.set_enabled(enabled);
    Ok(())
}

#[tauri::command]
fn update_desktop_pet_state(
    state: State<'_, AppState>,
    pet_state: DesktopPetRuntimeState,
) -> Result<(), String> {
    state.desktop_pet_controller.update(pet_state);
    Ok(())
}

fn start_desktop_pet_watcher(
    app: tauri::AppHandle,
    manager: DesktopPetManager,
    controller: DesktopPetController,
) {
    std::thread::Builder::new()
        .name("lanchat-desktop-pet-watcher".to_string())
        .spawn(move || loop {
            if manager.refresh_if_changed() {
                controller.set_package(manager.selected_package());
                app.emit("desktop_pet_registry_changed", manager.snapshot())
                    .ok();
            }
            std::thread::sleep(Duration::from_secs(2));
        })
        .ok();
}

fn acquire_lanchat_instance_lock() -> Result<(), String> {
    let lock_dir = std::env::temp_dir().join("lanchat");
    std::fs::create_dir_all(&lock_dir).map_err(|err| format!("创建实例锁目录失败：{err}"))?;
    let lock_path = lock_dir.join("lanchat-app.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .map_err(|err| format!("打开实例锁失败：{err}"))?;
    file.try_lock_exclusive()
        .map_err(|_| "LanChat Full/Lite 已有一个版本正在运行".to_string())?;
    let _ = LANCHAT_INSTANCE_LOCK.set(file);
    Ok(())
}

fn show_instance_lock_message(message: &str) {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                hwnd: *mut std::ffi::c_void,
                lp_text: *const u16,
                lp_caption: *const u16,
                u_type: u32,
            ) -> i32;
        }

        const MB_OK: u32 = 0x0000_0000;
        const MB_ICONINFORMATION: u32 = 0x0000_0040;
        const MB_SETFOREGROUND: u32 = 0x0001_0000;

        fn wide(value: &str) -> Vec<u16> {
            OsStr::new(value).encode_wide().chain(Some(0)).collect()
        }

        let text = wide(message);
        let caption = wide("LanChat");
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND,
            );
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        eprintln!("{message}");
    }
}

fn parse_shortcut_code(value: &str) -> Option<Code> {
    let normalized = match value.trim() {
        key if key.len() == 1 && key.chars().all(|ch| ch.is_ascii_alphabetic()) => {
            format!("Key{}", key.to_ascii_uppercase())
        }
        key if key.len() == 1 && key.chars().all(|ch| ch.is_ascii_digit()) => {
            format!("Digit{key}")
        }
        "Esc" => "Escape".to_string(),
        "Spacebar" => "Space".to_string(),
        "ArrowLeft" | "Left" => "ArrowLeft".to_string(),
        "ArrowRight" | "Right" => "ArrowRight".to_string(),
        "ArrowUp" | "Up" => "ArrowUp".to_string(),
        "ArrowDown" | "Down" => "ArrowDown".to_string(),
        "-" => "Minus".to_string(),
        "=" => "Equal".to_string(),
        "," => "Comma".to_string(),
        "." => "Period".to_string(),
        "/" => "Slash".to_string(),
        "`" => "Backquote".to_string(),
        "[" => "BracketLeft".to_string(),
        "]" => "BracketRight".to_string(),
        "\\" => "Backslash".to_string(),
        ";" => "Semicolon".to_string(),
        "'" => "Quote".to_string(),
        key => key.to_string(),
    };
    Code::from_str(&normalized).ok()
}

fn parse_desktop_pet_hotkey(value: &str) -> Result<Shortcut, String> {
    let mut modifiers = Modifiers::empty();
    let mut code = None;
    for part in value
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "meta" | "cmd" | "command" | "win" | "super" => modifiers |= Modifiers::SUPER,
            _ => code = parse_shortcut_code(part),
        }
    }
    let Some(code) = code else {
        return Err("请设置包含普通按键的快捷键".to_string());
    };
    Ok(Shortcut::new(Some(modifiers), code))
}

#[tauri::command]
fn register_desktop_pet_send_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<(), String> {
    register_desktop_pet_hotkey(
        &app,
        &state.desktop_pet_send_hotkey,
        &hotkey,
        "注册发送告警快捷键失败",
    )
}

#[tauri::command]
fn register_desktop_pet_stop_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<(), String> {
    register_desktop_pet_hotkey(
        &app,
        &state.desktop_pet_stop_hotkey,
        &hotkey,
        "注册停止提醒快捷键失败",
    )
}

fn register_desktop_pet_hotkey(
    app: &tauri::AppHandle,
    holder: &Arc<Mutex<Option<Shortcut>>>,
    hotkey: &str,
    error_prefix: &str,
) -> Result<(), String> {
    let mut current = holder
        .lock()
        .map_err(|_| "快捷键状态锁定失败".to_string())?;
    if let Some(previous) = current.take() {
        let _ = app.global_shortcut().unregister(previous);
    }
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        return Ok(());
    }
    let shortcut = parse_desktop_pet_hotkey(hotkey)?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|err| format!("{error_prefix}：{err}"))?;
    *current = Some(shortcut);
    Ok(())
}

fn show_main_window(
    app: &tauri::AppHandle,
    target: Option<TrayAttentionItem>,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window
        .set_skip_taskbar(false)
        .map_err(|err| format!("恢复任务栏图标失败：{err}"))?;
    window
        .show()
        .map_err(|err| format!("显示窗口失败：{err}"))?;
    window
        .unminimize()
        .map_err(|err| format!("恢复最小化窗口失败：{err}"))?;
    window
        .set_focus()
        .map_err(|err| format!("聚焦窗口失败：{err}"))?;
    if let Some(target) = target {
        let _ = app.emit("tray_open_target", target);
    }
    Ok(())
}

fn hide_main_window_to_tray(app: &tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window
        .set_skip_taskbar(true)
        .map_err(|err| format!("隐藏任务栏图标失败：{err}"))?;
    window
        .hide()
        .map_err(|err| format!("隐藏到托盘失败：{err}"))
}

fn quit_lanchat(app: &tauri::AppHandle) -> ! {
    if let Some(state) = app.try_state::<AppState>() {
        state.desktop_pet_controller.shutdown();
    }
    app.exit(0);
    std::process::exit(0);
}

fn rebuild_tray_menu(
    app: &tauri::AppHandle,
    items: &[TrayAttentionItem],
) -> Result<Menu<tauri::Wry>, String> {
    let menu = Menu::new(app).map_err(|err| format!("创建托盘菜单失败：{err}"))?;
    let open = MenuItemBuilder::with_id("tray-open-latest", "打开 LanChat")
        .build(app)
        .map_err(|err| format!("创建托盘菜单失败：{err}"))?;
    menu.append(&open)
        .map_err(|err| format!("创建托盘菜单失败：{err}"))?;

    if !items.is_empty() {
        let separator =
            PredefinedMenuItem::separator(app).map_err(|err| format!("创建托盘菜单失败：{err}"))?;
        menu.append(&separator)
            .map_err(|err| format!("创建托盘菜单失败：{err}"))?;
        for (index, item) in items.iter().take(8).enumerate() {
            let label = if item.kind == "game" {
                format!("游戏待操作：{}", item.title)
            } else {
                format!("{}  {} 条", item.title, item.count)
            };
            let menu_item = MenuItemBuilder::with_id(format!("tray-target-{index}"), label)
                .build(app)
                .map_err(|err| format!("创建托盘菜单失败：{err}"))?;
            menu.append(&menu_item)
                .map_err(|err| format!("创建托盘菜单失败：{err}"))?;
        }
    }

    let separator =
        PredefinedMenuItem::separator(app).map_err(|err| format!("创建托盘菜单失败：{err}"))?;
    let quit = MenuItemBuilder::with_id("tray-quit", "退出")
        .build(app)
        .map_err(|err| format!("创建托盘菜单失败：{err}"))?;
    menu.append(&separator)
        .map_err(|err| format!("创建托盘菜单失败：{err}"))?;
    menu.append(&quit)
        .map_err(|err| format!("创建托盘菜单失败：{err}"))?;
    Ok(menu)
}

fn update_tray_visuals(app: &tauri::AppHandle, state: &TrayState) -> Result<(), String> {
    let total: u32 = state.items.iter().map(|item| item.count.max(1)).sum();
    let tooltip = if state.items.is_empty() {
        "LanChat".to_string()
    } else {
        let lines = state
            .items
            .iter()
            .take(8)
            .map(|item| {
                if item.kind == "game" {
                    format!("{}：待操作", item.title)
                } else {
                    format!("{}：{}条未读消息", item.title, item.count)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("LanChat · {total} 条提醒\n{lines}")
    };
    if let Some(tray) = app.tray_by_id("main-tray") {
        let menu = rebuild_tray_menu(app, &state.items)?;
        tray.set_tooltip(Some(tooltip))
            .map_err(|err| format!("更新托盘提示失败：{err}"))?;
        tray.set_menu(Some(menu))
            .map_err(|err| format!("更新托盘菜单失败：{err}"))?;
        if state.items.is_empty() {
            set_tray_icon_blank(&tray, false)?;
        }
    }
    Ok(())
}

fn set_tray_icon_blank(tray: &tauri::tray::TrayIcon, blank: bool) -> Result<(), String> {
    if blank {
        return tray
            .set_icon(None)
            .map_err(|err| format!("清空托盘图标失败：{err}"));
    }
    let image =
        Image::from_bytes(TRAY_NORMAL_ICON).map_err(|err| format!("读取托盘图标失败：{err}"))?;
    tray.set_icon(Some(image))
        .map_err(|err| format!("恢复托盘图标失败：{err}"))
}

fn start_tray_blinker(app: tauri::AppHandle, tray_state: Arc<Mutex<TrayState>>) {
    std::thread::spawn(move || {
        let mut hidden = false;
        loop {
            std::thread::sleep(Duration::from_millis(700));
            let has_attention = tray_state
                .lock()
                .map(|state| !state.items.is_empty())
                .unwrap_or(false);
            let Some(tray) = app.tray_by_id("main-tray") else {
                continue;
            };
            if has_attention {
                hidden = !hidden;
                let _ = set_tray_icon_blank(&tray, hidden);
            } else if hidden {
                hidden = false;
                let _ = set_tray_icon_blank(&tray, false);
            }
        }
    });
}

#[tauri::command]
fn hide_to_tray(app: tauri::AppHandle) -> Result<(), String> {
    hide_main_window_to_tray(&app)
}

#[tauri::command]
fn start_main_window_drag(app: tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window
        .start_dragging()
        .map_err(|err| format!("拖动窗口失败：{err}"))
}

#[tauri::command]
fn minimize_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window
        .minimize()
        .map_err(|err| format!("最小化窗口失败：{err}"))
}

#[tauri::command]
fn toggle_main_window_maximized(app: tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    if window
        .is_maximized()
        .map_err(|err| format!("读取窗口状态失败：{err}"))?
    {
        window
            .unmaximize()
            .map_err(|err| format!("还原窗口失败：{err}"))
    } else {
        window
            .maximize()
            .map_err(|err| format!("最大化窗口失败：{err}"))
    }
}

#[tauri::command]
fn show_from_tray(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let target = state
        .tray
        .lock()
        .map_err(|_| "托盘状态读取失败".to_string())?
        .latest_target
        .clone();
    show_main_window(&app, target)
}

#[tauri::command]
fn update_tray_attention(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    items: Vec<TrayAttentionItem>,
) -> Result<(), String> {
    let mut tray = state
        .tray
        .lock()
        .map_err(|_| "托盘状态更新失败".to_string())?;
    tray.latest_target = items.first().cloned();
    tray.items = items;
    update_tray_visuals(&app, &tray)?;
    if !tray.items.is_empty() {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.request_user_attention(Some(UserAttentionType::Critical));
        }
    }
    Ok(())
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    quit_lanchat(&app);
}
#[tauri::command]
fn repair_windows_firewall() -> Result<String, String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err("当前平台不需要 Windows 网络修复。macOS 请在系统设置中允许 LanChat 访问本地网络，并确认防火墙没有阻止传入连接。".to_string());
    }

    #[cfg(target_os = "windows")]
    {
        let exe = std::env::current_exe().map_err(|err| format!("读取程序路径失败：{err}"))?;
        let exe_path = exe.to_string_lossy().replace('\'', "''");
        let script_path =
            std::env::temp_dir().join(format!("lanchat-firewall-repair-{}.ps1", Uuid::new_v4()));
        let script = format!(
            r#"$ErrorActionPreference = "Stop"
$current = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($current)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {{
  Write-Host "请以管理员身份运行网络修复。"
  Read-Host "按 Enter 退出"
  exit 1
}}
$exePath = '{exe_path}'
$profiles = "Domain,Private,Public"
$rules = @(
  @{{ Name = "LanChat App Inbound"; Params = @{{ Program = $exePath }} }},
  @{{ Name = "LanChat TCP Chat 18145"; Params = @{{ Protocol = "TCP"; LocalPort = 18145 }} }},
  @{{ Name = "LanChat UDP Presence 18146"; Params = @{{ Protocol = "UDP"; LocalPort = 18146 }} }},
  @{{ Name = "LanChat mDNS UDP 5353"; Params = @{{ Protocol = "UDP"; LocalPort = 5353 }} }}
)
foreach ($rule in $rules) {{
  Get-NetFirewallRule -DisplayName $rule.Name -ErrorAction SilentlyContinue | Remove-NetFirewallRule
  $params = @{{
    DisplayName = $rule.Name
    Direction = "Inbound"
    Action = "Allow"
    Profile = $profiles
  }}
  foreach ($key in $rule.Params.Keys) {{
    $params[$key] = $rule.Params[$key]
  }}
  New-NetFirewallRule @params | Out-Null
}}
Write-Host "LanChat 网络修复完成。"
Write-Host "已放行 Domain / Private / Public：LanChat.exe、TCP 18145、UDP 18146、UDP 5353。"
Read-Host "按 Enter 退出"
"#
        );
        std::fs::write(&script_path, script)
            .map_err(|err| format!("创建网络修复脚本失败：{err}"))?;
        let script_arg = format!(
            "-NoProfile -ExecutionPolicy Bypass -File \"{}\"",
            script_path.to_string_lossy()
        );
        std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "Start-Process",
                "PowerShell",
                "-Verb",
                "RunAs",
                "-ArgumentList",
                &script_arg,
            ])
            .spawn()
            .map_err(|err| format!("打开管理员网络修复失败：{err}"))?;
        Ok("已打开 Windows 管理员授权窗口，请点击“是”完成网络修复".to_string())
    }
}
fn setup_tray(app: &tauri::App, tray_state: Arc<Mutex<TrayState>>) -> Result<(), String> {
    let menu = rebuild_tray_menu(app.handle(), &[])?;
    let icon =
        Image::from_bytes(TRAY_NORMAL_ICON).map_err(|err| format!("读取托盘图标失败：{err}"))?;
    let builder = TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("LanChat")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event({
            let tray_state = tray_state.clone();
            move |app, event| {
                let id = event.id().as_ref().to_string();
                if id == "tray-quit" {
                    quit_lanchat(app);
                }
                let target = if id == "tray-open-latest" {
                    tray_state
                        .lock()
                        .ok()
                        .and_then(|state| state.latest_target.clone())
                } else if let Some(index) = id
                    .strip_prefix("tray-target-")
                    .and_then(|value| value.parse::<usize>().ok())
                {
                    tray_state
                        .lock()
                        .ok()
                        .and_then(|state| state.items.get(index).cloned())
                } else {
                    None
                };
                if id == "tray-open-latest" || id.starts_with("tray-target-") {
                    let _ = show_main_window(app, target);
                }
            }
        })
        .on_tray_icon_event({
            let tray_state = tray_state.clone();
            move |tray, event| match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
                | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    let target = tray_state
                        .lock()
                        .ok()
                        .and_then(|state| state.latest_target.clone());
                    let _ = show_main_window(tray.app_handle(), target);
                }
                _ => {}
            }
        });
    builder
        .build(app)
        .map_err(|err| format!("创建系统托盘失败：{err}"))?;
    Ok(())
}

fn reset_main_window(app: &tauri::App) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    window
        .set_fullscreen(false)
        .map_err(|err| format!("恢复窗口全屏状态失败：{err}"))?;
    window
        .unmaximize()
        .map_err(|err| format!("恢复窗口最大化状态失败：{err}"))?;
    window
        .set_size(Size::Logical(LogicalSize {
            width: 1180.0,
            height: 760.0,
        }))
        .map_err(|err| format!("设置默认窗口大小失败：{err}"))?;
    window
        .center()
        .map_err(|err| format!("窗口居中失败：{err}"))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = acquire_lanchat_instance_lock() {
        eprintln!("{error}");
        show_instance_lock_message(&error);
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = show_main_window(app, None);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let state = app.state::<AppState>();
                    let is_send = state
                        .desktop_pet_send_hotkey
                        .lock()
                        .ok()
                        .and_then(|current| *current)
                        .is_some_and(|registered| registered == *shortcut);
                    let is_stop = state
                        .desktop_pet_stop_hotkey
                        .lock()
                        .ok()
                        .and_then(|current| *current)
                        .is_some_and(|registered| registered == *shortcut);
                    if is_stop {
                        state.desktop_pet_controller.stop_alert_visuals();
                        let _ = app.emit("desktop_pet_stop_hotkey_received", ());
                    } else if is_send {
                        let _ = app.emit("desktop_pet_send_hotkey_received", ());
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            reset_main_window(app)?;
            let tray_state = Arc::new(Mutex::new(TrayState::default()));
            setup_tray(app, tray_state.clone())?;
            start_tray_blinker(app.handle().clone(), tray_state.clone());
            let app_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                std::env::current_dir()
                    .expect("current dir")
                    .join(".lanchat")
            });
            let storage = Arc::new(
                Storage::open(app_dir.join("lanchat.sqlite3"))
                    .map_err(|err| format!("初始化本地存储失败：{err}"))?,
            );
            storage.get_or_create_profile()?;
            let desktop_pet_app_dir = shared_desktop_pet_app_dir(&app_dir);
            let pet_roots = desktop_pet_resource_roots(app, &desktop_pet_app_dir);
            let desktop_pet = DesktopPetManager::new_lazy(
                pet_roots,
                desktop_pet_app_dir.join("desktop-pets"),
                desktop_pet_app_dir.join("desktop-pet-settings.json"),
            );
            let network = Network::new_with_desktop_pet(storage.clone(), desktop_pet.clone());
            network.start(app.handle().clone())?;
            let file_server = FileServer::new();
            file_server.start();
            let persisted_vision_runtime = storage.load_vision_runtime_state()?;
            let selected_model = storage.active_vision_model_install_path()?;
            let mut face_model_dirs = selected_model
                .as_ref()
                .map(|(_, _, path)| vec![path.clone()])
                .unwrap_or_default();
            if let Ok(resource_dir) = app.path().resource_dir() {
                face_model_dirs.push(resource_dir);
            }
            let face_monitor = Arc::new(FaceMonitorRuntime::from_candidate_dirs(face_model_dirs));
            let vision_mailbox = Arc::new(LatestFrameMailbox::default());
            let vision_runtime = Arc::new(VisionRuntimeState::restore(persisted_vision_runtime));
            let face_monitor_status = face_monitor.status();
            vision_runtime.mark_model_availability(face_monitor_status.model_ready);
            if face_monitor_status.model_ready {
                if let Some((profile_id, profile_version, _)) =
                    selected_model.filter(|(_, version, _)| {
                        face_monitor_status.model_version.as_deref() == Some(version.as_str())
                    })
                {
                    vision_runtime.set_active_profile(profile_id, profile_version);
                } else {
                    vision_runtime.set_active_profile(
                        "baseline",
                        face_monitor_status
                            .model_version
                            .unwrap_or_else(|| "1.0.0".to_string()),
                    );
                }
            }
            storage.save_vision_runtime_state(&vision_runtime.persisted_state())?;
            let worker_app = app.handle().clone();
            let worker_runtime = vision_runtime.clone();
            let mut worker_active_stream: Option<(String, u64)> = None;
            let vision_worker = VisionWorker::start(vision_mailbox.clone(), move |frame| {
                let started_at = std::time::Instant::now();
                let state = worker_app.state::<AppState>();
                let stream_key = (frame.stream_id.clone(), frame.stream_generation);
                if worker_active_stream.as_ref() != Some(&stream_key) {
                    // 新流不能继承旧摄像头的连续命中和短时追踪状态。
                    state.face_monitor.prepare_match_frame(&[]);
                    worker_active_stream = Some(stream_key);
                }
                let encoded = match encode_frame_as_jpeg(&frame) {
                    Ok(encoded) => encoded,
                    Err(error) => {
                        emit_debug_log(
                            &worker_app,
                            "warn",
                            "vision",
                            "视觉帧兼容编码失败，已跳过该帧",
                            Some(error),
                        );
                        worker_runtime.record_processing_failure("VISION_FRAME_ENCODING_FAILED");
                        return;
                    }
                };
                let result = tauri::async_runtime::block_on(process_face_monitor_frame(
                    worker_app.clone(),
                    &state,
                    &encoded,
                ));
                if let Err(error) = result {
                    emit_debug_log(
                        &worker_app,
                        "warn",
                        "vision",
                        "视觉 Worker 处理帧失败",
                        Some(error),
                    );
                    worker_runtime.record_processing_failure("VISION_INFERENCE_FAILED");
                } else {
                    worker_runtime.record_processing_duration(started_at.elapsed());
                }
            });
            let desktop_pet_controller = DesktopPetController::start(app.handle().clone());
            let pet_settings = desktop_pet.settings();
            desktop_pet_controller.set_enabled(pet_settings.enabled);
            desktop_pet_controller.set_package(desktop_pet.selected_package());
            start_desktop_pet_watcher(
                app.handle().clone(),
                desktop_pet.clone(),
                desktop_pet_controller.clone(),
            );
            app.manage(AppState {
                storage,
                network,
                file_server,
                tray: tray_state,
                desktop_pet_controller,
                desktop_pet,
                desktop_pet_send_hotkey: Arc::new(Mutex::new(None)),
                desktop_pet_stop_hotkey: Arc::new(Mutex::new(None)),
                super_admin_session: Arc::new(Mutex::new(false)),
                face_monitor,
                vision_mailbox,
                vision_runtime,
                vision_model_catalog: Arc::new(Mutex::new(Vec::new())),
                vision_model_root: app_dir.join("vision-models"),
                _vision_worker: vision_worker,
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = hide_main_window_to_tray(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_platform_info,
            get_face_monitor_status,
            get_vision_runtime_snapshot,
            set_vision_runtime_paused,
            update_face_monitor_local_settings,
            submit_face_monitor_frame,
            submit_vision_frame_raw,
            get_vision_runtime_diagnostics,
            list_vision_model_profiles,
            refresh_vision_model_catalog,
            install_vision_model_profile,
            activate_vision_model_profile,
            list_face_people,
            delete_face_person_local,
            save_face_reference_photo,
            create_local_face_person,
            get_effective_face_monitor_policy,
            send_face_monitor_policy,
            send_face_person_policy,
            list_camera_face_alerts,
            clear_camera_face_alerts,
            send_camera_face_alert_feedback,
            get_app_version_info,
            refresh_update_proxy,
            check_for_update,
            get_update_github_token_info,
            save_update_github_token,
            clear_update_github_token,
            is_portable_runtime,
            install_portable_update,
            send_admin_remote_update,
            execute_admin_remote_update,
            authenticate_super_admin,
            clear_super_admin_session,
            is_super_admin_authenticated,
            open_update_url,
            get_profile,
            update_profile,
            list_peers,
            update_peer_note,
            delete_peer,
            admin_rename_peer,
            connect_peer,
            list_conversations,
            list_channel_members,
            create_private_channel,
            invite_private_channel_members,
            remove_private_channel_member,
            leave_private_channel,
            set_private_channel_member_muted,
            dissolve_private_channel,
            admin_mute_channel_member,
            build_private_channel_invite_card,
            accept_private_channel_invite,
            is_channel_muted,
            broadcast_channel_notice,
            list_messages,
            send_message,
            simulate_message,
            save_system_notice,
            recall_message,
            send_file_message,
            send_pasted_image_message,
            cache_preview_media,
            get_preview_media_cache_info,
            clear_preview_media_cache,
            send_voice_message,
            send_game_frame,
            send_call_signal,
            send_quick_alert,
            simulate_quick_alert,
            send_quick_alert_feedback,
            send_quick_alert_trust_reset,
            send_admin_disco_mode,
            send_admin_alert_mode,
            send_admin_alert_push_policy,
            send_nudge,
            reveal_and_shake_main_window,
            list_admin_notifications,
            send_admin_notification,
            submit_admin_notification,
            decide_admin_notification,
            list_desktop_pets,
            refresh_desktop_pets,
            import_desktop_pet,
            remove_desktop_pet,
            select_desktop_pet,
            get_desktop_pet_settings,
            update_desktop_pet_settings,
            apply_admin_alert_push_policy,
            update_desktop_pet_playback_config,
            open_desktop_pet_folder,
            set_desktop_pet_enabled,
            update_desktop_pet_state,
            register_desktop_pet_stop_hotkey,
            register_desktop_pet_send_hotkey,
            start_main_window_drag,
            minimize_main_window,
            toggle_main_window_maximized,
            hide_to_tray,
            show_from_tray,
            update_tray_attention,
            quit_app,
            repair_windows_firewall
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
