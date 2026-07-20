mod channel_crypto;
mod debug_log;
mod desktop_pet;
mod desktop_pet_runtime;
mod file_server;
mod identity;
mod network;
mod protocol;
mod storage;

#[cfg(test)]
mod desktop_pet_tests;

use channel_crypto::{generate_channel_key, CHANNEL_KEY_VERSION};
use desktop_pet::{
    DesktopPetManager, DesktopPetPackage, DesktopPetRegistrySnapshot, DesktopPetSettings,
    PetPackageSource, PetResourceRoot, PetStatePlaybackConfig,
};
use desktop_pet_runtime::{DesktopPetController, DesktopPetRuntimeState};
use file_server::FileServer;
use network::Network;
use protocol::GameFrame;
use protocol::MessageRecallFrame;
use protocol::{
    AdminAlertModeFrame, AdminDiscoModeFrame, QuickAlertFeedbackFrame, QuickAlertFrame,
    QuickAlertTrustResetFrame,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use storage::{
    ChannelMember, ChannelMemberSeed, Conversation, Message, MessageType, Peer, Profile, Storage,
    DEFAULT_GROUP_ID,
};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, LogicalSize, Manager, Size, State, UserAttentionType, WindowEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use uuid::Uuid;

pub fn run_desktop_pet_process() {
    desktop_pet_runtime::run_desktop_pet_process();
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

struct AppState {
    storage: Arc<Storage>,
    network: Network,
    file_server: FileServer,
    tray: Arc<Mutex<TrayState>>,
    desktop_pet_controller: DesktopPetController,
    desktop_pet: DesktopPetManager,
    desktop_pet_stop_hotkey: Arc<Mutex<Option<Shortcut>>>,
}

const TRAY_NORMAL_ICON: &[u8] = include_bytes!("../icons/32x32.png");
const TRAY_ALERT_ICON: &[u8] = include_bytes!("../icons/tray-alert.png");

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

#[cfg(test)]
mod platform_info_tests {
    use super::*;

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
        if approx_bytes > 500 * 1024 {
            return Err("头像图片不能超过 500KB".to_string());
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
) -> Result<Peer, String> {
    let nickname = nickname.trim().to_string();
    if target_device_id.trim().is_empty() {
        return Err("请选择要修改的设备".to_string());
    }
    if nickname.is_empty() {
        return Err("昵称不能为空".to_string());
    }
    state
        .network
        .send_admin_nickname(app, target_device_id.clone(), nickname.clone())
        .await?;
    let mut peer = state
        .storage
        .get_peer(&target_device_id)?
        .ok_or_else(|| "设备已发送修改，但本地列表未找到该设备".to_string())?;
    peer.nickname = nickname;
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
    state.storage.list_conversations()
}

#[tauri::command]
fn list_channel_members(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ChannelMember>, String> {
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
    let channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    if !can_manage_private_channel(&channel.owner_device_id, &profile.device_id, super_admin) {
        return Err("只有频道群主或超管可以邀请成员".to_string());
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
async fn set_private_channel_member_muted(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    member_device_id: String,
    muted: bool,
    super_admin: bool,
) -> Result<Vec<ChannelMember>, String> {
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
    let channel = state
        .storage
        .get_private_channel(&conversation_id)?
        .ok_or_else(|| "请选择私有频道".to_string())?;
    let profile = state.storage.get_or_create_profile()?;
    if !can_manage_private_channel(&channel.owner_device_id, &profile.device_id, super_admin) {
        return Err("只有频道群主或超管可以邀请成员".to_string());
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
    state
        .network
        .broadcast_channel_notice(app, conversation_id, notice)
        .await
}

#[tauri::command]
fn list_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<Message>, String> {
    state.storage.list_messages(&conversation_id)
}

#[tauri::command]
async fn send_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
) -> Result<Message, String> {
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
        created_at: now,
    };
    state.network.send_message(app, message.clone()).await?;
    Ok(message)
}

#[tauri::command]
fn save_system_notice(
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
) -> Result<Message, String> {
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
async fn send_voice_message(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    file_name: String,
    bytes: Vec<u8>,
    duration_ms: u64,
) -> Result<Message, String> {
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
async fn send_quick_alert(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    content: String,
    mode: Option<String>,
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
        content: {
            let text = content.trim();
            if text.is_empty() {
                "呱呱~呱~~".to_string()
            } else {
                text.chars().take(120).collect()
            }
        },
        mode,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    state
        .network
        .broadcast_quick_alert(app, frame.clone())
        .await?;
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
        .send_admin_disco_mode(
            app.clone(),
            target_device_id,
            duration_ms.unwrap_or(120_000),
        )
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
    let settings = state.desktop_pet.update_settings(settings)?;
    state.desktop_pet_controller.set_enabled(settings.enabled);
    state
        .desktop_pet_controller
        .set_package(state.desktop_pet.selected_package());
    app.emit("desktop_pet_selected", &settings).ok();
    Ok(settings)
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
            std::thread::sleep(Duration::from_secs(2));
            if manager.refresh_if_changed() {
                controller.set_package(manager.selected_package());
                app.emit("desktop_pet_registry_changed", manager.snapshot())
                    .ok();
            }
        })
        .ok();
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

fn parse_desktop_pet_stop_hotkey(value: &str) -> Result<Shortcut, String> {
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
fn register_desktop_pet_stop_hotkey(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    hotkey: String,
) -> Result<(), String> {
    let mut current = state
        .desktop_pet_stop_hotkey
        .lock()
        .map_err(|_| "快捷键状态锁定失败".to_string())?;
    if let Some(previous) = current.take() {
        let _ = app.global_shortcut().unregister(previous);
    }
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        return Ok(());
    }
    let shortcut = parse_desktop_pet_stop_hotkey(hotkey)?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|err| format!("注册停止蹦迪快捷键失败：{err}"))?;
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
                    format!("{}：{} 条未读", item.title, item.count)
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
            set_tray_icon(&tray, false)?;
        }
    }
    Ok(())
}

fn set_tray_icon(tray: &tauri::tray::TrayIcon, alert: bool) -> Result<(), String> {
    let bytes = if alert {
        TRAY_ALERT_ICON
    } else {
        TRAY_NORMAL_ICON
    };
    let image = Image::from_bytes(bytes).map_err(|err| format!("读取托盘图标失败：{err}"))?;
    tray.set_icon(Some(image))
        .map_err(|err| format!("更新托盘图标失败：{err}"))
}

fn start_tray_blinker(app: tauri::AppHandle, tray_state: Arc<Mutex<TrayState>>) {
    std::thread::spawn(move || {
        let mut alert = false;
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
                alert = !alert;
                let _ = set_tray_icon(&tray, alert);
            } else if alert {
                alert = false;
                let _ = set_tray_icon(&tray, false);
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
    let mut builder = TrayIconBuilder::with_id("main-tray")
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
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
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
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            let _ = show_main_window(app, None);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        let _ = app.emit("desktop_pet_stop_hotkey_received", ());
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
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
            let network = Network::new(storage.clone());
            network.start(app.handle().clone())?;
            let file_server = FileServer::new();
            file_server.start();
            let user_pet_root = app_dir.join("desktop-pets");
            let builtin_pet_root = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| app_dir.clone())
                .join("desktop-pets");
            let portable_pet_root = std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent.join("desktop-pets")))
                .unwrap_or_else(|| app_dir.join("portable-desktop-pets"));
            let desktop_pet = DesktopPetManager::new(
                vec![
                    PetResourceRoot::new(builtin_pet_root, PetPackageSource::BuiltIn),
                    PetResourceRoot::new(portable_pet_root, PetPackageSource::Portable),
                    PetResourceRoot::new(user_pet_root, PetPackageSource::User),
                ],
                app_dir.join("desktop-pets"),
                app_dir.join("desktop-pet-settings.json"),
            );
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
                desktop_pet_stop_hotkey: Arc::new(Mutex::new(None)),
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
            get_profile,
            update_profile,
            list_peers,
            delete_peer,
            admin_rename_peer,
            connect_peer,
            list_conversations,
            list_channel_members,
            create_private_channel,
            invite_private_channel_members,
            remove_private_channel_member,
            set_private_channel_member_muted,
            dissolve_private_channel,
            admin_mute_channel_member,
            build_private_channel_invite_card,
            accept_private_channel_invite,
            is_channel_muted,
            broadcast_channel_notice,
            list_messages,
            send_message,
            save_system_notice,
            recall_message,
            send_file_message,
            send_voice_message,
            send_game_frame,
            send_quick_alert,
            send_quick_alert_feedback,
            send_quick_alert_trust_reset,
            send_admin_disco_mode,
            send_admin_alert_mode,
            list_desktop_pets,
            refresh_desktop_pets,
            import_desktop_pet,
            remove_desktop_pet,
            select_desktop_pet,
            get_desktop_pet_settings,
            update_desktop_pet_settings,
            update_desktop_pet_playback_config,
            open_desktop_pet_folder,
            set_desktop_pet_enabled,
            update_desktop_pet_state,
            register_desktop_pet_stop_hotkey,
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
