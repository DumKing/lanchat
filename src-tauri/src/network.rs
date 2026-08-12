use crate::channel_crypto::{decrypt_channel_content, encrypt_channel_content};
use crate::debug_log::emit_debug_log;
use crate::runtime_events::NetworkEventSink;
use crate::desktop_pet::DesktopPetManager;
use crate::identity::normalize_device_id;
use crate::protocol::{
    decode_frame, encode_frame, AckFrame, AdminAlertModeFrame, AdminAlertPushPolicyFrame,
    AdminChannelControlFrame, AdminDiscoModeFrame, AdminNicknameFrame, CallSignalFrame,
    ChannelMemberFrame, ChannelNoticeFrame, ChatMessageFrame, GameFrame, HelloFrame, NudgeFrame,
    PeerStatusFrame, PrivateChannelInviteFrame, QuickAlertFeedbackFrame, QuickAlertFrame,
    QuickAlertTrustResetFrame, WireFrame,
};
use crate::storage::{
    system_login_nickname, ChannelMemberSeed, Message, MessageStatus, MessageType, Peer, Profile,
    Storage, DEFAULT_GROUP_ID,
};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;
use uuid::Uuid;

const SERVICE_TYPE: &str = "_lanchat._tcp.local.";
const PROTOCOL_VERSION: u16 = 1;
const BUILD_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+",
    env!("LANCHAT_BUILD_TIMESTAMP")
);
const STATUS_BROADCAST_SECONDS: u64 = 30;
const UDP_PRESENCE_PORT: u16 = 18146;
const PEER_OFFLINE_TIMEOUT_MS: i64 = 75_000;
const OFFLINE_SWEEP_SECONDS: u64 = 15;

fn build_timestamp() -> i64 {
    env!("LANCHAT_BUILD_TIMESTAMP").parse().unwrap_or(0)
}

#[derive(Clone)]
struct PeerSender {
    id: String,
    sender: mpsc::Sender<WireFrame>,
}

type PeerSenders = Arc<Mutex<HashMap<String, PeerSender>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedChatPayload {
    content: String,
    message_type: String,
    file_meta: Option<crate::file_server::FileMeta>,
}

#[derive(Clone)]
pub struct Network {
    storage: Arc<Storage>,
    senders: PeerSenders,
    client_kind: String,
    supports_chat: bool,
    desktop_pet: Option<DesktopPetManager>,
}

impl Network {
    #[allow(dead_code)]
    pub fn new(storage: Arc<Storage>) -> Self {
        Self::new_with_client_kind(storage, "full")
    }

    pub fn new_with_client_kind(storage: Arc<Storage>, client_kind: &str) -> Self {
        let client_kind = normalize_client_kind(client_kind);
        Self {
            storage,
            senders: Arc::new(Mutex::new(HashMap::new())),
            client_kind,
            supports_chat: true,
            desktop_pet: None,
        }
    }

    pub fn new_with_desktop_pet(storage: Arc<Storage>, desktop_pet: DesktopPetManager) -> Self {
        let mut network = Self::new(storage);
        network.desktop_pet = Some(desktop_pet);
        network
    }

    pub fn supports_chat(&self) -> bool {
        self.supports_chat
    }

    pub fn start(&self, app: AppHandle) -> Result<(), String> {
        self.start_with_events(app.into())
    }

    pub fn start_native(&self, events: NetworkEventSink) -> Result<(), String> {
        self.start_with_events(events)
    }

    fn start_with_events(&self, app: NetworkEventSink) -> Result<(), String> {
        emit_debug_log(&app, "info", "network", "启动网络模块", None);
        let network = self.clone();
        let app_for_listener = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = network.start_tcp_listener(app_for_listener.clone()).await {
                emit_debug_log(
                    &app_for_listener,
                    "error",
                    "tcp",
                    "TCP 监听失败",
                    Some(err.clone()),
                );
                eprintln!("LanChat TCP listener failed: {err}");
            }
        });

        let network_for_mdns = self.clone();
        let app_for_mdns = app.clone();
        std::thread::spawn(move || {
            if let Err(err) = start_mdns(app_for_mdns.clone(), network_for_mdns) {
                emit_debug_log(
                    &app_for_mdns,
                    "error",
                    "mdns",
                    "mDNS 启动失败",
                    Some(err.clone()),
                );
                eprintln!("LanChat mDNS failed: {err}");
            }
        });

        let network_for_udp = self.clone();
        let app_for_udp = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(err) = network_for_udp
                .start_udp_presence_listener(app_for_udp.clone())
                .await
            {
                emit_debug_log(
                    &app_for_udp,
                    "error",
                    "udp",
                    "UDP 在线监听失败",
                    Some(err.clone()),
                );
                eprintln!("LanChat UDP presence failed: {err}");
            }
        });

        let network_for_status = self.clone();
        let app_for_status = app.clone();
        tauri::async_runtime::spawn(async move {
            network_for_status
                .start_status_broadcast(app_for_status)
                .await;
        });

        let network_for_offline = self.clone();
        let app_for_offline = app.clone();
        tauri::async_runtime::spawn(async move {
            network_for_offline
                .start_offline_sweeper(app_for_offline)
                .await;
        });
        Ok(())
    }

    fn status_frame(&self, profile: &Profile) -> PeerStatusFrame {
        status_frame_with_capabilities(profile, &self.client_kind, self.supports_chat)
    }

    fn status_frame_without_avatar(&self, profile: &Profile) -> PeerStatusFrame {
        PeerStatusFrame {
            avatar: None,
            includes_avatar: false,
            ..self.status_frame(profile)
        }
    }

    fn store_peer_update(&self, peer: Peer, includes_avatar: bool) -> Result<Peer, String> {
        if includes_avatar || peer.avatar.is_some() {
            self.storage
                .update_peer_avatar(&peer.device_id, peer.avatar.clone())?;
        }
        self.storage.upsert_peer(&peer)?;
        self.storage
            .get_peer(&peer.device_id)?
            .ok_or_else(|| "读取更新后的设备资料失败".to_string())
    }

    pub async fn connect_peer(
        &self,
        app: impl Into<NetworkEventSink>,
        address: String,
        port: u16,
    ) -> Result<Peer, String> {
        let app = app.into();
        emit_debug_log(
            &app,
            "info",
            "tcp",
            "尝试连接设备",
            Some(format!("{address}:{port}")),
        );
        let stream = TcpStream::connect(format!("{address}:{port}"))
            .await
            .map_err(|err| {
                let message = format!("连接 {address}:{port} 失败：{err}");
                emit_debug_log(&app, "warn", "tcp", "连接设备失败", Some(message.clone()));
                message
            })?;
        self.attach_stream(app, stream, Some((address, port))).await
    }

    pub fn broadcast_profile_status(
        &self,
        app: AppHandle,
        profile: &Profile,
    ) -> Result<(), String> {
        let frame = WireFrame::PeerStatus(self.status_frame(profile));
        let senders = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?;
        let mut pushed = 0;
        for sender in senders.values() {
            if sender.sender.try_send(frame.clone()).is_ok() {
                pushed += 1;
            }
        }
        emit_debug_log(
            &app,
            "info",
            "presence",
            "本机资料变更已推送",
            Some(format!("{pushed} 个连接")),
        );
        Ok(())
    }

    pub async fn send_game_frame(
        &self,
        app: AppHandle,
        target_device_id: Option<String>,
        frame: GameFrame,
    ) -> Result<(), String> {
        if !self.supports_chat {
            return Err("当前客户端不支持游戏".to_string());
        }
        let wire_frame = WireFrame::Game(frame.clone());
        let delivered =
            if let Some(peer_id) = target_device_id.filter(|value| !value.trim().is_empty()) {
                self.require_full_feature_peer(&peer_id, "游戏")?;
                self.send_direct_frame(app.clone(), &peer_id, wire_frame)
                    .await?
            } else {
                let senders = self
                    .senders
                    .lock()
                    .map_err(|_| "连接表已损坏".to_string())?;
                let mut delivered = false;
                for (peer_id, sender) in senders.iter() {
                    if !self.peer_supports_full_features(peer_id)? {
                        continue;
                    }
                    delivered |= sender.sender.try_send(wire_frame.clone()).is_ok();
                }
                delivered
            };

        if !delivered {
            emit_debug_log(
                &app,
                "warn",
                "game",
                "游戏消息未送达",
                Some(format!("{} {}", frame.game, frame.kind)),
            );
            return Err("没有可用连接，游戏消息未送达".to_string());
        }

        emit_debug_log(
            &app,
            "info",
            "game",
            "游戏消息已发送",
            Some(format!("{} {}", frame.game, frame.kind)),
        );
        Ok(())
    }
    pub async fn send_admin_nickname(
        &self,
        app: AppHandle,
        target_device_id: String,
        nickname: String,
        nickname_locked: Option<bool>,
        use_system_username: bool,
    ) -> Result<(), String> {
        let target_device_id = normalize_device_id(&target_device_id);
        let nickname = nickname.trim().to_string();
        if nickname.is_empty() && !use_system_username {
            return Err("昵称不能为空".to_string());
        }
        let profile = self.storage.get_or_create_profile()?;
        let frame = WireFrame::AdminNickname(AdminNicknameFrame {
            target_device_id: target_device_id.clone(),
            nickname: nickname.clone(),
            nickname_locked,
            use_system_username,
            issued_by_device_id: profile.device_id.clone(),
            issued_by_nickname: profile.nickname.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        });
        let delivered = self
            .send_direct_frame(app.clone(), &target_device_id, frame)
            .await?;
        if !delivered {
            emit_debug_log(
                &app,
                "warn",
                "admin",
                "超管昵称修改未送达",
                Some(target_device_id),
            );
            return Err("没有可用连接，超管昵称修改未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "admin",
            "超管昵称修改已发送",
            Some(format!(
                "{} -> {}",
                target_device_id,
                if use_system_username {
                    "电脑登录用户名".to_string()
                } else {
                    nickname
                }
            )),
        );
        Ok(())
    }
    fn broadcast_private_channel_snapshot(
        &self,
        app: impl Into<NetworkEventSink>,
        channel_id: &str,
    ) -> Result<(), String> {
        let app = app.into();
        let Some(channel) = self.storage.get_private_channel(channel_id)? else {
            return Ok(());
        };
        let profile = self.storage.get_or_create_profile()?;
        let members = self.storage.list_channel_members(channel_id)?;
        let frame = WireFrame::PrivateChannelInvite(PrivateChannelInviteFrame {
            channel_id: channel.id.clone(),
            title: channel.title.clone(),
            owner_device_id: channel.owner_device_id.clone(),
            owner_nickname: channel.owner_nickname.clone(),
            channel_key: channel.channel_key.clone(),
            key_version: channel.key_version,
            members: members
                .iter()
                .map(|member| ChannelMemberFrame {
                    device_id: member.device_id.clone(),
                    nickname: member.nickname.clone(),
                    avatar: member.avatar.clone(),
                })
                .collect(),
            created_at: chrono::Utc::now().timestamp_millis(),
        });
        let senders = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?;
        for member in members {
            if normalize_device_id(&member.device_id) == normalize_device_id(&profile.device_id) {
                continue;
            }
            if let Some(sender) = senders.get(&normalize_device_id(&member.device_id)) {
                sender.sender.try_send(frame.clone()).ok();
            }
        }
        emit_debug_log(
            &app,
            "info",
            "channel",
            "私有频道成员快照已广播",
            Some(channel.title),
        );
        Ok(())
    }
    pub async fn send_admin_channel_control(
        &self,
        app: AppHandle,
        target_device_id: String,
        channel_id: String,
        action: String,
        muted: bool,
    ) -> Result<(), String> {
        let target_device_id = normalize_device_id(&target_device_id);
        let profile = self.storage.get_or_create_profile()?;
        let frame = WireFrame::AdminChannelControl(AdminChannelControlFrame {
            target_device_id: target_device_id.clone(),
            channel_id: channel_id.clone(),
            action: action.clone(),
            muted,
            issued_by_device_id: profile.device_id.clone(),
            issued_by_nickname: profile.nickname.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        });
        let delivered = self
            .send_direct_frame(app.clone(), &target_device_id, frame)
            .await?;
        if !delivered {
            emit_debug_log(
                &app,
                "warn",
                "admin",
                "频道管控未送达",
                Some(format!("{} {}", channel_id, action)),
            );
            return Err("没有可用连接，频道管控未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "admin",
            "频道管控已发送",
            Some(format!("{} {}", channel_id, action)),
        );
        Ok(())
    }

    pub async fn send_admin_disco_mode(
        &self,
        app: AppHandle,
        target_device_id: String,
        duration_ms: u64,
    ) -> Result<AdminDiscoModeFrame, String> {
        let target_device_id = normalize_device_id(&target_device_id);
        let duration_ms = duration_ms.clamp(10_000, 300_000);
        let profile = self.storage.get_or_create_profile()?;
        let frame = AdminDiscoModeFrame {
            target_device_id: target_device_id.clone(),
            duration_ms,
            issued_by_device_id: profile.device_id.clone(),
            issued_by_nickname: profile.nickname.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        if target_device_id == normalize_device_id(&profile.device_id) {
            emit_debug_log(
                &app,
                "info",
                "admin",
                "本机蹦迪模式已触发",
                Some(format!("{}ms", duration_ms)),
            );
            return Ok(frame);
        }
        let delivered = self
            .send_direct_frame(
                app.clone(),
                &target_device_id,
                WireFrame::AdminDiscoMode(frame.clone()),
            )
            .await?;
        if !delivered {
            emit_debug_log(
                &app,
                "warn",
                "admin",
                "蹦迪模式未送达",
                Some(target_device_id),
            );
            return Err("没有可用连接，蹦迪模式未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "admin",
            "蹦迪模式已发送",
            Some(format!("{} {}ms", target_device_id, duration_ms)),
        );
        Ok(frame)
    }

    pub async fn send_admin_alert_mode(
        &self,
        app: AppHandle,
        target_device_id: String,
        mode: String,
    ) -> Result<AdminAlertModeFrame, String> {
        let target_device_id = normalize_device_id(&target_device_id);
        let mode = normalize_alert_mode(&mode);
        let profile = self.storage.get_or_create_profile()?;
        let frame = AdminAlertModeFrame {
            target_device_id: target_device_id.clone(),
            mode: mode.clone(),
            issued_by_device_id: profile.device_id.clone(),
            issued_by_nickname: profile.nickname.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        if target_device_id == normalize_device_id(&profile.device_id) {
            emit_debug_log(&app, "info", "admin", "本机报警模式已修改", Some(mode));
            return Ok(frame);
        }
        let delivered = self
            .send_direct_frame(
                app.clone(),
                &target_device_id,
                WireFrame::AdminAlertMode(frame.clone()),
            )
            .await?;
        if !delivered {
            emit_debug_log(
                &app,
                "warn",
                "admin",
                "报警模式未送达",
                Some(target_device_id),
            );
            return Err("没有可用连接，报警模式未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "admin",
            "报警模式已发送",
            Some(format!("{} {}", target_device_id, mode)),
        );
        Ok(frame)
    }

    pub async fn send_call_signal(
        &self,
        app: AppHandle,
        target_device_id: String,
        frame: CallSignalFrame,
    ) -> Result<(), String> {
        let target_device_id = normalize_device_id(&target_device_id);
        self.require_full_feature_peer(&target_device_id, "语音/视频通话")?;
        if !self
            .send_direct_frame(
                app.clone(),
                &target_device_id,
                WireFrame::CallSignal(frame.clone()),
            )
            .await?
        {
            return Err("对方不在线或通话信令未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "call",
            "通话信令已发送",
            Some(format!("{} {}", target_device_id, frame.kind)),
        );
        Ok(())
    }

    pub async fn send_nudge(
        &self,
        app: AppHandle,
        target_device_id: String,
    ) -> Result<NudgeFrame, String> {
        let target_device_id = normalize_device_id(&target_device_id);
        self.require_full_feature_peer(&target_device_id, "抖一抖")?;
        let profile = self.storage.get_or_create_profile()?;
        let frame = NudgeFrame {
            nudge_id: Uuid::new_v4().to_string(),
            sender_device_id: profile.device_id,
            sender_nickname: profile.nickname,
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        if !self
            .send_direct_frame(
                app.clone(),
                &target_device_id,
                WireFrame::Nudge(frame.clone()),
            )
            .await?
        {
            return Err("对方不在线，抖一抖未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "nudge",
            "抖一抖已发送",
            Some(target_device_id),
        );
        Ok(frame)
    }

    pub async fn send_admin_alert_push_policy(
        &self,
        app: AppHandle,
        target_device_id: String,
        min_credibility: u8,
        min_credibility_locked: bool,
    ) -> Result<AdminAlertPushPolicyFrame, String> {
        let target_device_id = target_device_id.trim().to_string();
        let profile = self.storage.get_or_create_profile()?;
        let frame = AdminAlertPushPolicyFrame {
            target_device_id: target_device_id.clone(),
            min_credibility,
            min_credibility_locked,
            issued_by_device_id: profile.device_id.clone(),
            issued_by_nickname: profile.nickname.clone(),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        let wire = WireFrame::AdminAlertPushPolicy(frame.clone());
        let delivered = if target_device_id == "*" {
            let peers = self.storage.list_peers()?;
            let mut delivered = false;
            for peer in peers.into_iter().filter(|peer| {
                peer.online
                    && peer.supports_full_features()
                    && normalize_device_id(&peer.device_id)
                        != normalize_device_id(&profile.device_id)
            }) {
                delivered |= self
                    .send_direct_frame(app.clone(), &peer.device_id, wire.clone())
                    .await?;
            }
            delivered
        } else if normalize_device_id(&target_device_id) == normalize_device_id(&profile.device_id)
        {
            true
        } else {
            self.send_direct_frame(app.clone(), &target_device_id, wire)
                .await?
        };
        if !delivered
            && target_device_id != "*"
            && target_device_id != normalize_device_id(&profile.device_id)
        {
            return Err("没有可用连接，推送阈值未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "admin",
            "狼来了推送阈值已下发",
            Some(format!(
                "{} {min_credibility} locked={min_credibility_locked}",
                target_device_id
            )),
        );
        Ok(frame)
    }

    pub async fn send_admin_notification_frame(
        &self,
        app: AppHandle,
        target_device_id: &str,
        frame: WireFrame,
    ) -> Result<(), String> {
        let target_device_id = normalize_device_id(target_device_id);
        let delivered = self
            .send_direct_frame(app.clone(), &target_device_id, frame)
            .await?;
        if !delivered {
            return Err("目标设备当前不在线，通知未送达".to_string());
        }
        emit_debug_log(
            &app,
            "info",
            "admin",
            "超管通知已直连送达",
            Some(target_device_id),
        );
        Ok(())
    }

    pub async fn broadcast_channel_notice(
        &self,
        app: AppHandle,
        conversation_id: String,
        notice: String,
    ) -> Result<(), String> {
        let conversation_id = conversation_id.trim().to_string();
        if conversation_id.is_empty() {
            return Err("请选择频道".to_string());
        }
        let profile = self.storage.get_or_create_profile()?;
        let frame = WireFrame::ChannelNotice(ChannelNoticeFrame {
            conversation_id: conversation_id.clone(),
            notice: notice.trim().to_string(),
            updated_by_device_id: profile.device_id.clone(),
            updated_by_nickname: profile.nickname.clone(),
            updated_at: chrono::Utc::now().timestamp_millis(),
        });
        let mut target_ids = Vec::new();
        if self
            .storage
            .private_channel_key(&conversation_id)?
            .is_some()
        {
            target_ids = self
                .storage
                .list_channel_members(&conversation_id)?
                .into_iter()
                .map(|member| normalize_device_id(&member.device_id))
                .filter(|device_id| device_id != &normalize_device_id(&profile.device_id))
                .collect();
        }

        let senders = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?;
        let mut delivered = 0;
        if target_ids.is_empty() && conversation_id == DEFAULT_GROUP_ID {
            for sender in senders.values() {
                if sender.sender.try_send(frame.clone()).is_ok() {
                    delivered += 1;
                }
            }
        } else {
            for target_id in &target_ids {
                if let Some(sender) = senders.get(target_id) {
                    if sender.sender.try_send(frame.clone()).is_ok() {
                        delivered += 1;
                    }
                }
            }
        }
        emit_debug_log(
            &app,
            "info",
            "channel",
            "频道公告已广播",
            Some(format!("{conversation_id} delivered={delivered}")),
        );
        Ok(())
    }

    pub async fn broadcast_message_recall(
        &self,
        app: AppHandle,
        frame: crate::protocol::MessageRecallFrame,
    ) -> Result<(), String> {
        let wire_frame = WireFrame::MessageRecall(frame.clone());
        let mut delivered = false;
        if frame.conversation_id == DEFAULT_GROUP_ID
            || self
                .storage
                .private_channel_key(&frame.conversation_id)?
                .is_some()
        {
            let senders = self
                .senders
                .lock()
                .map_err(|_| "连接表已损坏".to_string())?;
            for sender in senders.values() {
                delivered |= sender.sender.try_send(wire_frame.clone()).is_ok();
            }
        } else {
            delivered = self
                .send_direct_frame(app.clone(), &frame.conversation_id, wire_frame)
                .await?;
        }
        emit_debug_log(
            &app,
            if delivered { "info" } else { "warn" },
            "message",
            "消息撤回已广播",
            Some(format!("{} delivered={delivered}", frame.message_id)),
        );
        Ok(())
    }

    pub async fn broadcast_quick_alert(
        &self,
        app: impl Into<NetworkEventSink>,
        frame: QuickAlertFrame,
    ) -> Result<(), String> {
        let app = app.into();
        let wire_frame = WireFrame::QuickAlert(frame.clone());
        let senders = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?;
        let mut delivered = 0;
        for sender in senders.values() {
            if sender.sender.try_send(wire_frame.clone()).is_ok() {
                delivered += 1;
            }
        }
        emit_debug_log(
            &app,
            if delivered > 0 { "info" } else { "warn" },
            "alert",
            "快捷告警已广播",
            Some(format!("{} delivered={delivered}", frame.alert_id)),
        );
        Ok(())
    }

    pub async fn broadcast_quick_alert_feedback(
        &self,
        app: impl Into<NetworkEventSink>,
        frame: QuickAlertFeedbackFrame,
    ) -> Result<(), String> {
        let app = app.into();
        let wire_frame = WireFrame::QuickAlertFeedback(frame.clone());
        let senders = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?;
        let mut delivered = 0;
        for sender in senders.values() {
            if sender.sender.try_send(wire_frame.clone()).is_ok() {
                delivered += 1;
            }
        }
        emit_debug_log(
            &app,
            if delivered > 0 { "info" } else { "warn" },
            "alert",
            "快捷告警反馈已广播",
            Some(format!(
                "{} {} delivered={delivered}",
                frame.alert_id, frame.result
            )),
        );
        Ok(())
    }

    pub async fn broadcast_quick_alert_trust_reset(
        &self,
        app: AppHandle,
        frame: QuickAlertTrustResetFrame,
    ) -> Result<(), String> {
        let wire_frame = WireFrame::QuickAlertTrustReset(frame.clone());
        let senders = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?;
        let mut delivered = 0;
        for sender in senders.values() {
            if sender.sender.try_send(wire_frame.clone()).is_ok() {
                delivered += 1;
            }
        }
        emit_debug_log(
            &app,
            if delivered > 0 { "info" } else { "warn" },
            "alert",
            "告警可信度重置已广播",
            Some(format!("{} delivered={delivered}", frame.target_device_id)),
        );
        Ok(())
    }

    pub async fn send_message(
        &self,
        app: impl Into<NetworkEventSink>,
        message: Message,
    ) -> Result<(), String> {
        let app = app.into();
        if !self.supports_chat {
            return Err("当前客户端不支持聊天、频道和文件消息".to_string());
        }
        let authorization_device_id = message
            .simulation
            .as_ref()
            .map(|meta| meta.operator_device_id.as_str())
            .unwrap_or(message.sender_device_id.as_str());
        let private_channel_key = self.storage.private_channel_key(&message.conversation_id)?;
        if let Some(_) = private_channel_key.as_ref() {
            if !self
                .storage
                .is_private_channel_member(&message.conversation_id, authorization_device_id)?
            {
                return Err("你不是该私有频道成员，不能发送消息".to_string());
            }
            if self.storage.is_private_channel_member_muted(
                &message.conversation_id,
                authorization_device_id,
            )? {
                return Err("你已被群主禁言，暂不能在该频道发言".to_string());
            }
        } else if message.conversation_id == DEFAULT_GROUP_ID {
            if self
                .storage
                .is_channel_muted(DEFAULT_GROUP_ID, authorization_device_id)?
            {
                return Err("你已被超管禁言，暂不能在公共频道发言".to_string());
            }
        } else {
            let peer = self
                .storage
                .get_peer(&message.conversation_id)?
                .ok_or_else(|| "未找到该设备，无法发送私聊消息".to_string())?;
            if !peer.supports_full_features() {
                emit_debug_log(
                    &app,
                    "warn",
                    "message",
                    "阻止向不支持聊天的设备发送私聊",
                    Some(peer.device_id.clone()),
                );
                return Err("该设备不支持聊天".to_string());
            }
            if !peer.online {
                emit_debug_log(
                    &app,
                    "warn",
                    "message",
                    "阻止发送离线私聊",
                    Some(peer.device_id.clone()),
                );
                return Err("对方已离线，不能发送私聊消息".to_string());
            }
        }
        self.storage.save_message(&message)?;
        app.emit("message_status_changed", &message).ok();

        let mut frame = ChatMessageFrame {
            message_id: message.id.clone(),
            conversation_id: message.conversation_id.clone(),
            sender_device_id: message.sender_device_id.clone(),
            content: message.content.clone(),
            message_type: message.message_type.as_str().to_string(),
            file_meta: message.file_meta.clone(),
            encrypted: false,
            nonce: None,
            key_version: None,
            simulation: message.simulation.clone(),
            created_at: message.created_at,
        };

        let mut broadcast = message.conversation_id == DEFAULT_GROUP_ID;
        if let Some((channel_key, key_version)) = private_channel_key {
            let protected_payload = serde_json::to_string(&EncryptedChatPayload {
                content: message.content.clone(),
                message_type: message.message_type.as_str().to_string(),
                file_meta: message.file_meta.clone(),
            })
            .map_err(|err| format!("准备私有频道消息失败：{err}"))?;
            let encrypted = encrypt_channel_content(&channel_key, &protected_payload)?;
            frame.content = encrypted.content;
            frame.message_type = "encrypted".to_string();
            frame.file_meta = None;
            frame.encrypted = true;
            frame.nonce = Some(encrypted.nonce);
            frame.key_version = Some(key_version);
            broadcast = true;
        }
        let wire_frame = WireFrame::ChatMessage(frame);

        let mut delivered = false;
        if broadcast {
            let senders = self
                .senders
                .lock()
                .map_err(|_| "连接表已损坏".to_string())?;
            for (peer_id, sender) in senders.iter() {
                if !self.peer_supports_full_features(peer_id)? {
                    continue;
                }
                delivered |= sender.sender.try_send(wire_frame.clone()).is_ok();
            }
        } else {
            delivered = self
                .send_direct_frame(app.clone(), &message.conversation_id, wire_frame)
                .await?;
        }

        let status = if delivered {
            MessageStatus::Sent
        } else {
            MessageStatus::Failed
        };
        self.storage
            .update_message_status(&message.id, status.clone())?;
        let mut updated = message;
        updated.status = status;
        app.emit("message_status_changed", &updated).ok();
        Ok(())
    }

    async fn send_direct_frame(
        &self,
        app: impl Into<NetworkEventSink>,
        peer_device_id: &str,
        frame: WireFrame,
    ) -> Result<bool, String> {
        let app = app.into();
        let peer_device_id = normalize_device_id(peer_device_id);
        if let Some(sender) = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?
            .get(&peer_device_id)
            .cloned()
        {
            return Ok(sender.sender.try_send(frame).is_ok());
        }

        if let Some(peer) = self.storage.get_peer(&peer_device_id)? {
            let connected = self
                .connect_peer(app, peer.address.clone(), peer.port)
                .await
                .map(|_| true)
                .unwrap_or(false);
            if connected {
                if let Some(sender) = self
                    .senders
                    .lock()
                    .map_err(|_| "连接表已损坏".to_string())?
                    .get(&peer_device_id)
                    .cloned()
                {
                    return Ok(sender.sender.try_send(frame).is_ok());
                }
            }
        }

        Ok(false)
    }

    fn peer_supports_full_features(&self, peer_device_id: &str) -> Result<bool, String> {
        Ok(self
            .storage
            .get_peer(peer_device_id)?
            .map(|peer| peer.supports_full_features())
            .unwrap_or(false))
    }

    fn require_full_feature_peer(
        &self,
        peer_device_id: &str,
        capability: &str,
    ) -> Result<(), String> {
        let peer = self
            .storage
            .get_peer(peer_device_id)?
            .ok_or_else(|| "未找到该设备".to_string())?;
        if peer.supports_full_features() {
            Ok(())
        } else {
            Err(format!("该设备不支持{capability}"))
        }
    }

    async fn start_tcp_listener(&self, app: NetworkEventSink) -> Result<(), String> {
        let profile = self.storage.get_or_create_profile()?;
        let listener = TcpListener::bind(("0.0.0.0", profile.listen_port))
            .await
            .map_err(|err| format!("监听端口 {} 失败：{err}", profile.listen_port))?;
        emit_debug_log(
            &app,
            "info",
            "tcp",
            "TCP 监听已启动",
            Some(format!("0.0.0.0:{}", profile.listen_port)),
        );

        loop {
            let (stream, addr) = listener
                .accept()
                .await
                .map_err(|err| format!("接受连接失败：{err}"))?;
            let app_for_peer = app.clone();
            let network = self.clone();
            tauri::async_runtime::spawn(async move {
                let address = addr.ip().to_string();
                emit_debug_log(
                    &app_for_peer,
                    "info",
                    "tcp",
                    "收到 TCP 连接",
                    Some(format!("{}", addr)),
                );
                if let Err(err) = network
                    .attach_stream(app_for_peer, stream, Some((address, addr.port())))
                    .await
                {
                    eprintln!("LanChat peer attach failed: {err}");
                }
            });
        }
    }

    async fn attach_stream(
        &self,
        app: NetworkEventSink,
        stream: TcpStream,
        remote_hint: Option<(String, u16)>,
    ) -> Result<Peer, String> {
        let profile = self.storage.get_or_create_profile()?;
        let (reader, mut writer) = stream.into_split();
        let hello = WireFrame::Hello(HelloFrame {
            device_id: profile.device_id.clone(),
            nickname: profile.nickname.clone(),
            avatar: profile.avatar.clone(),
            nickname_locked: profile.nickname_locked,
            protocol_version: PROTOCOL_VERSION,
            listen_port: profile.listen_port,
            client_kind: self.client_kind.clone(),
            supports_chat: self.supports_chat,
            build_version: BUILD_VERSION.to_string(),
            build_timestamp: build_timestamp(),
            public_key: None,
        });
        writer
            .write_all(
                encode_frame(&hello)
                    .map_err(|err| format!("编码握手失败：{err}"))?
                    .as_bytes(),
            )
            .await
            .map_err(|err| format!("发送握手失败：{err}"))?;

        let mut lines = BufReader::new(reader).lines();
        let first_line = lines
            .next_line()
            .await
            .map_err(|err| format!("读取握手失败：{err}"))?
            .ok_or_else(|| "对方未发送握手".to_string())?;
        let remote_hello =
            match decode_frame(&first_line).map_err(|err| format!("解析握手失败：{err}"))? {
                WireFrame::Hello(frame) => frame,
                _ => return Err("对方握手格式不正确".to_string()),
            };

        let (address, fallback_port) =
            remote_hint.unwrap_or_else(|| ("0.0.0.0".to_string(), remote_hello.listen_port));
        let peer = Peer {
            device_id: normalize_device_id(&remote_hello.device_id),
            nickname: remote_hello.nickname.clone(),
            note: None,
            avatar: remote_hello.avatar.clone(),
            address,
            port: if remote_hello.listen_port > 0 {
                remote_hello.listen_port
            } else {
                fallback_port
            },
            online: true,
            last_seen_at: chrono::Utc::now().timestamp_millis(),
            client_kind: remote_hello.client_kind.clone(),
            supports_chat: remote_hello.supports_chat,
            nickname_locked: remote_hello.nickname_locked,
            build_version: remote_hello.build_version.clone(),
            build_timestamp: remote_hello.build_timestamp,
        };
        let peer = self.store_peer_update(peer, true)?;
        emit_debug_log(
            &app,
            "info",
            "peer",
            "TCP 握手完成，设备在线",
            Some(format!("{} {}:{}", peer.nickname, peer.address, peer.port)),
        );
        app.emit("peer_online", &peer).ok();

        // A disconnected peer must not be able to grow the process indefinitely.
        let (tx, mut rx) = mpsc::channel::<WireFrame>(128);
        let connection_id = Uuid::new_v4().to_string();
        self.senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?
            .insert(
                peer.device_id.clone(),
                PeerSender {
                    id: connection_id.clone(),
                    sender: tx.clone(),
                },
            );
        tx.try_send(WireFrame::PeerStatus(self.status_frame(&profile)))
            .ok();

        let writer_app = app.clone();
        let peer_id = peer.device_id.clone();
        let senders = self.senders.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(frame) = rx.recv().await {
                match encode_frame(&frame) {
                    Ok(line) => {
                        if writer.write_all(line.as_bytes()).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            if let Ok(mut locked) = senders.lock() {
                let should_remove = locked
                    .get(&peer_id)
                    .map(|current| current.id == connection_id)
                    .unwrap_or(false);
                if should_remove {
                    locked.remove(&peer_id);
                }
            }
            emit_debug_log(
                &writer_app,
                "warn",
                "tcp",
                "TCP 连接关闭",
                Some(peer_id.clone()),
            );
            writer_app.emit("peer_connection_closed", peer_id).ok();
        });

        let read_storage = self.storage.clone();
        let read_desktop_pet = self.desktop_pet.clone();
        let read_network = self.clone();
        let read_app = app.clone();
        let local_device_id = profile.device_id.clone();
        let peer_address = peer.address.clone();
        let read_supports_chat = self.supports_chat;
        let ack_sender = self
            .senders
            .lock()
            .map_err(|_| "连接表已损坏".to_string())?
            .get(&peer.device_id)
            .map(|value| value.sender.clone());
        tauri::async_runtime::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                match decode_frame(&line) {
                    Ok(WireFrame::ChatMessage(frame)) => {
                        if !read_supports_chat {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "client",
                                "当前客户端已忽略聊天消息",
                                Some(frame.message_id),
                            );
                            continue;
                        }
                        let conversation_id = inbound_conversation_id(&frame, &local_device_id);
                        let mut content = frame.content.clone();
                        let mut message_type =
                            crate::storage::MessageType::from_str(&frame.message_type);
                        let mut file_meta = frame.file_meta.clone();
                        if frame.encrypted {
                            let Some((channel_key, _)) = read_storage
                                .private_channel_key(&frame.conversation_id)
                                .ok()
                                .flatten()
                            else {
                                emit_debug_log(
                                    &read_app,
                                    "warn",
                                    "channel",
                                    "丢弃未知私有频道密文",
                                    Some(frame.conversation_id.clone()),
                                );
                                continue;
                            };
                            let Some(nonce) = frame.nonce.clone() else {
                                emit_debug_log(
                                    &read_app,
                                    "warn",
                                    "channel",
                                    "丢弃缺少 nonce 的私有频道消息",
                                    Some(frame.conversation_id.clone()),
                                );
                                continue;
                            };
                            let Ok(decrypted) =
                                decrypt_channel_content(&channel_key, &nonce, &frame.content)
                            else {
                                emit_debug_log(
                                    &read_app,
                                    "warn",
                                    "channel",
                                    "私有频道消息解密失败",
                                    Some(frame.conversation_id.clone()),
                                );
                                continue;
                            };
                            let Ok(payload) =
                                serde_json::from_str::<EncryptedChatPayload>(&decrypted)
                            else {
                                emit_debug_log(
                                    &read_app,
                                    "warn",
                                    "channel",
                                    "私有频道消息载荷无效",
                                    Some(frame.conversation_id.clone()),
                                );
                                continue;
                            };
                            content = payload.content;
                            message_type =
                                crate::storage::MessageType::from_str(&payload.message_type);
                            file_meta = payload.file_meta;
                        } else if frame.conversation_id != DEFAULT_GROUP_ID
                            && read_storage
                                .private_channel_key(&frame.conversation_id)
                                .ok()
                                .flatten()
                                .is_some()
                        {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "channel",
                                "丢弃未加密的私有频道消息",
                                Some(frame.conversation_id.clone()),
                            );
                            continue;
                        }
                        let message = Message {
                            id: frame.message_id.clone(),
                            conversation_id,
                            sender_device_id: frame.sender_device_id.clone(),
                            content,
                            message_type,
                            file_meta,
                            status: MessageStatus::Delivered,
                            simulation: frame.simulation.clone(),
                            created_at: frame.created_at,
                        };
                        if read_storage.save_message(&message).is_ok() {
                            read_app.emit("message_received", &message).ok();
                        }
                        if let Some(sender) = &ack_sender {
                            sender
                                .try_send(WireFrame::Ack(AckFrame {
                                    message_id: frame.message_id,
                                }))
                                .ok();
                        }
                    }
                    Ok(WireFrame::PeerStatus(frame)) => {
                        if normalize_device_id(&frame.device_id) != local_device_id {
                            let includes_avatar = frame.includes_avatar || frame.avatar.is_some();
                            let peer = peer_from_status(frame, peer_address.clone());
                            if let Ok(peer) = read_network.store_peer_update(peer, includes_avatar)
                            {
                                emit_debug_log(
                                    &read_app,
                                    "info",
                                    "peer",
                                    "收到 TCP 在线状态",
                                    Some(format!(
                                        "{} {}:{}",
                                        peer.nickname, peer.address, peer.port
                                    )),
                                );
                                read_app.emit("peer_online", &peer).ok();
                            }
                        }
                    }
                    Ok(WireFrame::Game(frame)) => {
                        if !read_supports_chat {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "client",
                                "当前客户端已忽略游戏消息",
                                Some(format!("{} {}", frame.game, frame.kind)),
                            );
                            continue;
                        }
                        read_app.emit("game_frame_received", frame).ok();
                    }
                    Ok(WireFrame::PrivateChannelInvite(frame)) => {
                        if !read_supports_chat {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "client",
                                "当前客户端已忽略私有频道邀请",
                                Some(frame.channel_id),
                            );
                            continue;
                        }
                        let local_id = normalize_device_id(&local_device_id);
                        if !frame
                            .members
                            .iter()
                            .any(|member| normalize_device_id(&member.device_id) == local_id)
                        {
                            continue;
                        }
                        let members = frame
                            .members
                            .iter()
                            .map(|member| ChannelMemberSeed {
                                device_id: member.device_id.clone(),
                                nickname: member.nickname.clone(),
                                avatar: member.avatar.clone(),
                            })
                            .collect::<Vec<_>>();
                        match read_storage.upsert_private_channel(
                            &frame.channel_id,
                            &frame.title,
                            &frame.owner_device_id,
                            &frame.owner_nickname,
                            &frame.channel_key,
                            frame.key_version,
                            &members,
                            frame.created_at,
                        ) {
                            Ok(conversation) => {
                                emit_debug_log(
                                    &read_app,
                                    "info",
                                    "channel",
                                    "收到私有频道邀请",
                                    Some(frame.title.clone()),
                                );
                                read_app.emit("private_channel_invited", &conversation).ok();
                            }
                            Err(err) => emit_debug_log(
                                &read_app,
                                "warn",
                                "channel",
                                "保存私有频道邀请失败",
                                Some(err),
                            ),
                        }
                    }
                    Ok(WireFrame::ChannelNotice(frame)) => {
                        if !read_supports_chat {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "client",
                                "当前客户端已忽略频道公告",
                                Some(frame.conversation_id),
                            );
                            continue;
                        }
                        emit_debug_log(
                            &read_app,
                            "info",
                            "channel",
                            "收到频道公告更新",
                            Some(format!(
                                "{} by {}",
                                frame.conversation_id, frame.updated_by_nickname
                            )),
                        );
                        read_app.emit("channel_notice_updated", frame).ok();
                    }
                    Ok(WireFrame::MessageRecall(frame)) => {
                        if !read_supports_chat {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "client",
                                "当前客户端已忽略消息撤回",
                                Some(frame.message_id),
                            );
                            continue;
                        }
                        match read_storage.update_message_after_recall(&frame.message_id) {
                            Ok(Some(message)) => {
                                emit_debug_log(
                                    &read_app,
                                    "info",
                                    "message",
                                    "收到消息撤回",
                                    Some(frame.message_id),
                                );
                                read_app.emit("message_recalled", message).ok();
                            }
                            Ok(None) => {}
                            Err(err) => emit_debug_log(
                                &read_app,
                                "warn",
                                "message",
                                "处理消息撤回失败",
                                Some(err),
                            ),
                        }
                    }
                    Ok(WireFrame::QuickAlert(frame)) => {
                        emit_debug_log(
                            &read_app,
                            "warn",
                            "alert",
                            "收到快捷告警",
                            Some(format!("{} {}", frame.sender_nickname, frame.alert_id)),
                        );
                        read_app.emit("quick_alert_received", frame).ok();
                    }
                    Ok(WireFrame::QuickAlertFeedback(frame)) => {
                        emit_debug_log(
                            &read_app,
                            "info",
                            "alert",
                            "收到快捷告警反馈",
                            Some(format!(
                                "{} {} {}",
                                frame.alert_id, frame.responder_nickname, frame.result
                            )),
                        );
                        read_app.emit("quick_alert_feedback_received", frame).ok();
                    }
                    Ok(WireFrame::QuickAlertTrustReset(frame)) => {
                        emit_debug_log(
                            &read_app,
                            "warn",
                            "alert",
                            "收到告警可信度重置",
                            Some(format!(
                                "{} {}",
                                frame.target_device_id, frame.issued_by_nickname
                            )),
                        );
                        read_app
                            .emit("quick_alert_trust_reset_received", frame)
                            .ok();
                    }
                    Ok(WireFrame::CallSignal(frame)) => {
                        if !read_supports_chat {
                            continue;
                        }
                        emit_debug_log(
                            &read_app,
                            "info",
                            "call",
                            "收到通话信令",
                            Some(format!("{} {}", frame.sender_nickname, frame.kind)),
                        );
                        read_app.emit("call_signal_received", frame).ok();
                    }
                    Ok(WireFrame::Nudge(frame)) => {
                        if !read_supports_chat {
                            continue;
                        }
                        emit_debug_log(
                            &read_app,
                            "info",
                            "nudge",
                            "收到抖一抖",
                            Some(frame.sender_nickname.clone()),
                        );
                        read_app.emit("nudge_received", frame).ok();
                    }
                    Ok(WireFrame::AdminAlertPushPolicy(frame)) => {
                        if frame.target_device_id == "*"
                            || normalize_device_id(&frame.target_device_id) == local_device_id
                        {
                            if let Some(desktop_pet) = &read_desktop_pet {
                                let mut settings = desktop_pet.settings();
                                settings.external_push_min_credibility = frame.min_credibility;
                                settings.external_push_min_credibility_locked =
                                    frame.min_credibility_locked;
                                if let Err(err) = desktop_pet.update_settings(settings) {
                                    emit_debug_log(
                                        &read_app,
                                        "error",
                                        "admin",
                                        "保存狼来了推送阈值失败",
                                        Some(err),
                                    );
                                }
                            }
                            emit_debug_log(
                                &read_app,
                                "info",
                                "admin",
                                "收到狼来了推送阈值",
                                Some(frame.min_credibility.to_string()),
                            );
                            read_app
                                .emit("admin_alert_push_policy_received", frame)
                                .ok();
                        }
                    }
                    Ok(WireFrame::AdminNickname(frame)) => {
                        if normalize_device_id(&frame.target_device_id) == local_device_id {
                            let requested = if frame.use_system_username {
                                system_login_nickname()
                            } else {
                                frame.nickname.trim().to_string()
                            };
                            if !requested.is_empty() {
                                if let Ok(updated) = read_storage
                                    .apply_admin_nickname(&requested, frame.nickname_locked)
                                {
                                    emit_debug_log(
                                        &read_app,
                                        "info",
                                        "admin",
                                        "收到超管昵称修改",
                                        Some(format!(
                                            "{} 设置为 {}{}{}",
                                            frame.issued_by_nickname,
                                            updated.nickname,
                                            if frame.use_system_username {
                                                "（电脑登录用户名）"
                                            } else {
                                                ""
                                            },
                                            if updated.nickname_locked {
                                                "，并禁止本机修改昵称"
                                            } else {
                                                ""
                                            }
                                        )),
                                    );
                                    read_app.emit("profile_updated", &updated).ok();
                                    if let Some(sender) = &ack_sender {
                                        sender
                                            .try_send(WireFrame::PeerStatus(
                                                read_network.status_frame(&updated),
                                            ))
                                            .ok();
                                    }
                                }
                            }
                        }
                    }
                    Ok(WireFrame::AdminDiscoMode(frame)) => {
                        if normalize_device_id(&frame.target_device_id) == local_device_id {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "admin",
                                "收到蹦迪模式",
                                Some(format!(
                                    "{} {}ms",
                                    frame.issued_by_nickname, frame.duration_ms
                                )),
                            );
                            read_app.emit("admin_disco_mode_received", frame).ok();
                        }
                    }
                    Ok(WireFrame::AdminAlertMode(frame)) => {
                        if normalize_device_id(&frame.target_device_id) == local_device_id {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "admin",
                                "收到报警模式下发",
                                Some(format!("{} {}", frame.issued_by_nickname, frame.mode)),
                            );
                            read_app.emit("admin_alert_mode_received", frame).ok();
                        }
                    }
                    Ok(WireFrame::AdminNotification(frame)) => {
                        if normalize_device_id(&frame.target_device_id) == local_device_id {
                            match read_storage.upsert_admin_notification(&frame) {
                                Ok(record) => {
                                    if record.force_open_main_window {
                                        read_app.request_main_window();
                                        emit_debug_log(
                                            &read_app,
                                            "warn",
                                            "admin",
                                            "超管通知强制打开主窗口",
                                            Some(record.title.clone()),
                                        );
                                    }
                                    emit_debug_log(
                                        &read_app,
                                        "warn",
                                        "admin",
                                        "收到超管通知",
                                        Some(record.title.clone()),
                                    );
                                    read_app.emit("admin_notification_received", record).ok();
                                }
                                Err(error) => emit_debug_log(
                                    &read_app,
                                    "error",
                                    "admin",
                                    "保存超管通知失败",
                                    Some(error),
                                ),
                            }
                        }
                    }
                    Ok(WireFrame::AdminNotificationSubmission(frame)) => {
                        match read_storage.submit_admin_notification(
                            &frame.notification_id,
                            &frame.target_device_id,
                            &frame.submitted_by_nickname,
                            frame.submitted_at,
                        ) {
                            Ok(record) => {
                                emit_debug_log(
                                    &read_app,
                                    "info",
                                    "admin",
                                    "收到通知完成提交",
                                    Some(frame.notification_id),
                                );
                                read_app
                                    .emit("admin_notification_submission_received", record)
                                    .ok();
                            }
                            Err(error) => emit_debug_log(
                                &read_app,
                                "warn",
                                "admin",
                                "处理通知完成提交失败",
                                Some(error),
                            ),
                        }
                    }
                    Ok(WireFrame::AdminNotificationDecision(frame)) => {
                        if normalize_device_id(&frame.target_device_id) == local_device_id {
                            match read_storage.decide_admin_notification(&frame) {
                                Ok(record) => {
                                    emit_debug_log(
                                        &read_app,
                                        "info",
                                        "admin",
                                        "收到通知审核结果",
                                        Some(record.status.clone()),
                                    );
                                    read_app
                                        .emit("admin_notification_decision_received", record)
                                        .ok();
                                }
                                Err(error) => emit_debug_log(
                                    &read_app,
                                    "error",
                                    "admin",
                                    "处理通知审核结果失败",
                                    Some(error),
                                ),
                            }
                        }
                    }
                    Ok(WireFrame::AdminChannelControl(frame)) => {
                        if !read_supports_chat {
                            emit_debug_log(
                                &read_app,
                                "warn",
                                "client",
                                "当前客户端已忽略频道管控",
                                Some(frame.channel_id),
                            );
                            continue;
                        }
                        if normalize_device_id(&frame.target_device_id) == local_device_id {
                            let action = frame.action.as_str();
                            let result = match action {
                                "mute" | "unmute" if frame.channel_id == DEFAULT_GROUP_ID => {
                                    read_storage.set_channel_mute(
                                        &frame.channel_id,
                                        &local_device_id,
                                        frame.muted,
                                        frame.created_at,
                                    )
                                }
                                "mute" | "unmute" => read_storage.set_private_channel_member_muted(
                                    &frame.channel_id,
                                    &local_device_id,
                                    frame.muted,
                                ),
                                "join" => {
                                    let requester_id =
                                        normalize_device_id(&frame.issued_by_device_id);
                                    let known_peer =
                                        read_storage.get_peer(&requester_id).ok().flatten();
                                    let nickname = known_peer
                                        .as_ref()
                                        .map(|peer| peer.nickname.clone())
                                        .unwrap_or_else(|| frame.issued_by_nickname.clone());
                                    let avatar = known_peer.and_then(|peer| peer.avatar);
                                    let notice = format!("{nickname} 加入了群聊");
                                    match read_storage.add_private_channel_member(
                                        &frame.channel_id,
                                        &ChannelMemberSeed {
                                            device_id: requester_id,
                                            nickname,
                                            avatar,
                                        },
                                        frame.created_at,
                                    ) {
                                        Ok(()) => {
                                            if let Ok(message) = save_system_notice(
                                                &read_storage,
                                                &frame.channel_id,
                                                notice,
                                            ) {
                                                read_app.emit("message_received", &message).ok();
                                            }
                                            Ok(())
                                        }
                                        Err(err) => Err(err),
                                    }
                                }
                                "remove" | "dissolve" => {
                                    read_storage.delete_private_channel(&frame.channel_id)
                                }
                                "leave" => {
                                    let leaving_id = normalize_device_id(&frame.issued_by_device_id);
                                    let notice = format!("{} 退出了群聊", frame.issued_by_nickname);
                                    read_storage
                                        .remove_private_channel_member(&frame.channel_id, &leaving_id)
                                        .map(|()| {
                                            if let Ok(message) = save_system_notice(
                                                &read_storage,
                                                &frame.channel_id,
                                                notice,
                                            ) {
                                                read_app.emit("message_received", &message).ok();
                                            }
                                        })
                                }
                                _ => Ok(()),
                            };
                            match result {
                                Ok(()) => {
                                    emit_debug_log(
                                        &read_app,
                                        "info",
                                        "admin",
                                        "收到频道管控",
                                        Some(format!("{} {}", frame.channel_id, frame.action)),
                                    );
                                    read_app
                                        .emit("private_channel_changed", &frame.channel_id)
                                        .ok();
                                    if frame.action == "join" {
                                        let _ = read_network.broadcast_private_channel_snapshot(
                                            read_app.clone(),
                                            &frame.channel_id,
                                        );
                                    }
                                }
                                Err(err) => emit_debug_log(
                                    &read_app,
                                    "warn",
                                    "admin",
                                    "频道管控执行失败",
                                    Some(err),
                                ),
                            }
                        }
                    }
                    Ok(WireFrame::Ack(frame)) => {
                        if read_storage
                            .update_message_status(&frame.message_id, MessageStatus::Delivered)
                            .is_ok()
                        {
                            read_app.emit("message_status_changed", frame).ok();
                        }
                    }
                    Ok(WireFrame::Ping) => {
                        if let Some(sender) = &ack_sender {
                            sender.try_send(WireFrame::Pong).ok();
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(peer)
    }

    async fn start_udp_presence_listener(&self, app: NetworkEventSink) -> Result<(), String> {
        let socket = UdpSocket::bind(("0.0.0.0", UDP_PRESENCE_PORT))
            .await
            .map_err(|err| format!("监听 UDP 在线广播端口 {UDP_PRESENCE_PORT} 失败：{err}"))?;
        emit_debug_log(
            &app,
            "info",
            "udp",
            "UDP 在线监听已启动",
            Some(format!("0.0.0.0:{UDP_PRESENCE_PORT}")),
        );
        let mut buf = vec![0_u8; 4096];
        loop {
            let (size, addr) = socket
                .recv_from(&mut buf)
                .await
                .map_err(|err| format!("接收 UDP 在线广播失败：{err}"))?;
            let Ok(text) = std::str::from_utf8(&buf[..size]) else {
                continue;
            };
            let Ok(WireFrame::PeerStatus(frame)) = decode_frame(text) else {
                continue;
            };
            let Ok(profile) = self.storage.get_or_create_profile() else {
                continue;
            };
            if normalize_device_id(&frame.device_id) == profile.device_id {
                continue;
            }
            let includes_avatar = frame.includes_avatar || frame.avatar.is_some();
            let peer = peer_from_status(frame, addr.ip().to_string());
            emit_debug_log(
                &app,
                "info",
                "udp",
                "收到 UDP 在线广播",
                Some(format!(
                    "{} {}:{} from {}",
                    peer.nickname, peer.address, peer.port, addr
                )),
            );
            if let Ok(peer) = self.store_peer_update(peer, includes_avatar) {
                app.emit("peer_online", &peer).ok();
                let should_connect = self
                    .senders
                    .lock()
                    .map(|senders| !senders.contains_key(&peer.device_id))
                    .unwrap_or(false);
                if should_connect {
                    let connect_network = self.clone();
                    let connect_app = app.clone();
                    let connect_address = peer.address.clone();
                    let connect_port = peer.port;
                    tauri::async_runtime::spawn(async move {
                        if let Err(err) = connect_network
                            .connect_peer(
                                connect_app.clone(),
                                connect_address.clone(),
                                connect_port,
                            )
                            .await
                        {
                            emit_debug_log(
                                &connect_app,
                                "warn",
                                "tcp",
                                "UDP 发现后自动连接失败",
                                Some(format!("{}:{} {err}", connect_address, connect_port)),
                            );
                        }
                    });
                }
            }
        }
    }
    async fn start_offline_sweeper(&self, app: NetworkEventSink) {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(OFFLINE_SWEEP_SECONDS));
        loop {
            interval.tick().await;
            let cutoff = chrono::Utc::now().timestamp_millis() - PEER_OFFLINE_TIMEOUT_MS;
            let Ok(ids) = self.storage.mark_stale_peers_offline(cutoff) else {
                continue;
            };
            if let Ok(mut senders) = self.senders.lock() {
                for id in &ids {
                    senders.remove(id);
                }
            }
            for id in ids {
                emit_debug_log(
                    &app,
                    "warn",
                    "peer",
                    "设备在线超时，标记离线",
                    Some(id.clone()),
                );
                app.emit("peer_offline", id).ok();
            }
        }
    }
    async fn start_status_broadcast(&self, app: NetworkEventSink) {
        let udp_socket = UdpSocket::bind(("0.0.0.0", 0)).await.ok();
        if let Some(socket) = &udp_socket {
            match socket.set_broadcast(true) {
                Ok(()) => emit_debug_log(
                    &app,
                    "info",
                    "presence",
                    "UDP 在线广播已启用",
                    Some(format!("255.255.255.255:{UDP_PRESENCE_PORT}")),
                ),
                Err(err) => emit_debug_log(
                    &app,
                    "warn",
                    "presence",
                    "UDP 在线广播启用失败",
                    Some(err.to_string()),
                ),
            }
        } else {
            emit_debug_log(
                &app,
                "warn",
                "presence",
                "UDP 在线广播 socket 创建失败",
                Some("将仅通过已连接 TCP 推送在线状态".to_string()),
            );
        }
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(STATUS_BROADCAST_SECONDS));
        let mut first_broadcast = true;
        loop {
            interval.tick().await;
            let Ok(profile) = self.storage.get_or_create_profile() else {
                continue;
            };
            let udp_frame = WireFrame::PeerStatus(self.status_frame_without_avatar(&profile));
            let tcp_frame = WireFrame::PeerStatus(if first_broadcast {
                first_broadcast = false;
                self.status_frame(&profile)
            } else {
                self.status_frame_without_avatar(&profile)
            });
            if let Some(socket) = &udp_socket {
                if let Ok(line) = encode_frame(&udp_frame) {
                    if let Err(err) = socket
                        .send_to(line.as_bytes(), ("255.255.255.255", UDP_PRESENCE_PORT))
                        .await
                    {
                        emit_debug_log(
                            &app,
                            "warn",
                            "presence",
                            "UDP 在线状态广播失败",
                            Some(err.to_string()),
                        );
                        eprintln!("LanChat UDP broadcast failed: {err}");
                    } else {
                        emit_debug_log(
                            &app,
                            "info",
                            "presence",
                            "UDP 在线状态已广播",
                            Some(format!(
                                "{} {} port={}",
                                profile.device_id, profile.nickname, profile.listen_port
                            )),
                        );
                    }
                }
            }
            let mut pushed = 0;
            if let Ok(senders) = self.senders.lock() {
                for sender in senders.values() {
                    if sender.sender.try_send(tcp_frame.clone()).is_ok() {
                        pushed += 1;
                    }
                }
            }
            emit_debug_log(
                &app,
                "info",
                "presence",
                "TCP 在线状态已推送",
                Some(format!("{pushed} 个连接")),
            );
        }
    }
}

fn status_frame_with_capabilities(
    profile: &Profile,
    client_kind: &str,
    supports_chat: bool,
) -> PeerStatusFrame {
    PeerStatusFrame {
        device_id: profile.device_id.clone(),
        nickname: profile.nickname.clone(),
        avatar: profile.avatar.clone(),
        includes_avatar: true,
        address: None,
        protocol_version: PROTOCOL_VERSION,
        listen_port: profile.listen_port,
        nickname_locked: profile.nickname_locked,
        client_kind: normalize_client_kind(client_kind),
        supports_chat,
        build_version: BUILD_VERSION.to_string(),
        build_timestamp: build_timestamp(),
        updated_at: chrono::Utc::now().timestamp_millis(),
    }
}

fn normalize_alert_mode(mode: &str) -> String {
    if mode.trim().eq_ignore_ascii_case("disco") {
        "disco".to_string()
    } else {
        "normal".to_string()
    }
}

fn peer_from_status(frame: PeerStatusFrame, source_address: String) -> Peer {
    peer_from_status_at(frame, source_address, chrono::Utc::now().timestamp_millis())
}

fn peer_from_status_at(frame: PeerStatusFrame, source_address: String, seen_at: i64) -> Peer {
    Peer {
        device_id: normalize_device_id(&frame.device_id),
        nickname: frame.nickname,
        note: None,
        avatar: frame.avatar,
        nickname_locked: frame.nickname_locked,
        address: source_address,
        port: frame.listen_port,
        online: true,
        last_seen_at: seen_at,
        client_kind: frame.client_kind,
        supports_chat: frame.supports_chat,
        build_version: frame.build_version,
        build_timestamp: frame.build_timestamp,
    }
}

fn normalize_client_kind(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        _ => "full".to_string(),
    }
}

fn save_system_notice(
    storage: &Storage,
    conversation_id: &str,
    content: String,
) -> Result<Message, String> {
    let message = Message {
        id: Uuid::new_v4().to_string(),
        conversation_id: conversation_id.to_string(),
        sender_device_id: "system".to_string(),
        content,
        message_type: MessageType::System,
        file_meta: None,
        status: MessageStatus::Delivered,
        simulation: None,
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    storage.save_message(&message)?;
    Ok(message)
}

fn inbound_conversation_id(frame: &ChatMessageFrame, local_device_id: &str) -> String {
    if frame.conversation_id == DEFAULT_GROUP_ID {
        DEFAULT_GROUP_ID.to_string()
    } else if frame.conversation_id == local_device_id {
        frame.sender_device_id.clone()
    } else {
        frame.conversation_id.clone()
    }
}

fn mdns_instance_name(device_id: &str) -> String {
    let compact: String = device_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect();
    if compact.is_empty() {
        "lanchat-node".to_string()
    } else {
        format!("lc-{compact}")
    }
}

fn start_mdns(app: NetworkEventSink, network: Network) -> Result<(), String> {
    let profile = network.storage.get_or_create_profile()?;
    let mdns = ServiceDaemon::new().map_err(|err| format!("启动 mDNS 失败：{err}"))?;
    register_mdns_service(&mdns, &profile, &network.client_kind, network.supports_chat)?;
    let receiver = mdns
        .browse(SERVICE_TYPE)
        .map_err(|err| format!("启动 mDNS 浏览失败：{err}"))?;

    while let Ok(event) = receiver.recv() {
        if let ServiceEvent::ServiceResolved(info) = event {
            let props = info.get_properties();
            let Some(device_id) = props.get("device_id").map(|v| v.val_str()) else {
                continue;
            };
            if normalize_device_id(device_id) == profile.device_id {
                continue;
            }
            let nickname = props
                .get("nickname")
                .map(|v| v.val_str())
                .unwrap_or("局域网用户")
                .to_string();
            let client_kind = props
                .get("client_kind")
                .map(|v| normalize_client_kind(v.val_str()))
                .unwrap_or_else(|| "full".to_string());
            let supports_chat = props
                .get("supports_chat")
                .map(|v| v.val_str() != "false")
                .unwrap_or(true);
            let address = info
                .get_addresses()
                .iter()
                .find(|addr| addr.is_ipv4())
                .or_else(|| info.get_addresses().iter().next())
                .map(|addr| addr.to_string())
                .unwrap_or_else(|| "0.0.0.0".to_string());
            let peer = Peer {
                device_id: normalize_device_id(device_id),
                nickname,
                note: None,
                avatar: None,
                address: address.clone(),
                port: info.get_port(),
                online: true,
                last_seen_at: chrono::Utc::now().timestamp_millis(),
                client_kind,
                supports_chat,
                nickname_locked: false,
                build_version: String::new(),
                build_timestamp: 0,
            };
            if let Ok(peer) = network.store_peer_update(peer, false) {
                emit_debug_log(
                    &app,
                    "info",
                    "mdns",
                    "mDNS 发现设备",
                    Some(format!("{} {}:{}", peer.nickname, peer.address, peer.port)),
                );
                app.emit("peer_online", &peer).ok();
                let connect_network = network.clone();
                let connect_app = app.clone();
                let connect_address = peer.address.clone();
                let connect_port = peer.port;
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = connect_network
                        .connect_peer(connect_app.clone(), connect_address.clone(), connect_port)
                        .await
                    {
                        emit_debug_log(
                            &connect_app,
                            "warn",
                            "tcp",
                            "mDNS 发现后自动连接失败",
                            Some(format!("{}:{} {err}", connect_address, connect_port)),
                        );
                    }
                });
            }
        }
    }
    Ok(())
}

fn register_mdns_service(
    mdns: &ServiceDaemon,
    profile: &Profile,
    client_kind: &str,
    supports_chat: bool,
) -> Result<(), String> {
    let ip = local_ip_address();
    let host_name = format!("{}.local.", mdns_instance_name(&profile.device_id));
    let instance_name = mdns_instance_name(&profile.device_id);
    let properties = [
        ("device_id", profile.device_id.as_str()),
        ("nickname", profile.nickname.as_str()),
        ("protocol_version", "1"),
        ("client_kind", client_kind),
        (
            "supports_chat",
            if supports_chat { "true" } else { "false" },
        ),
    ];
    let service = ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name,
        &host_name,
        ip.as_str(),
        profile.listen_port,
        &properties[..],
    )
    .map_err(|err| format!("创建 mDNS 服务失败：{err}"))?;
    mdns.register(service)
        .map_err(|err| format!("注册 mDNS 服务失败：{err}"))
}

pub(crate) fn local_ip_address() -> String {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return "127.0.0.1".to_string();
    };
    let wireless_ip = interfaces
        .iter()
        .filter(|iface| !iface.is_loopback())
        .filter(|iface| is_wireless_interface(iface))
        .find_map(interface_ipv4);
    wireless_ip
        .or_else(|| {
            interfaces
                .iter()
                .filter(|iface| !iface.is_loopback())
                .find_map(interface_ipv4)
        })
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

fn interface_ipv4(iface: &if_addrs::Interface) -> Option<String> {
    match iface.addr.ip() {
        std::net::IpAddr::V4(ip) => Some(ip.to_string()),
        std::net::IpAddr::V6(_) => None,
    }
}

fn is_wireless_interface(iface: &if_addrs::Interface) -> bool {
    let name = iface.name.to_ascii_lowercase();
    if name.contains("wlan")
        || name.contains("wi-fi")
        || name.contains("wifi")
        || name.contains("wireless")
        || name.contains("802.11")
    {
        return true;
    }
    #[cfg(windows)]
    {
        let adapter_name = iface.adapter_name.to_ascii_lowercase();
        adapter_name.contains("wlan")
            || adapter_name.contains("wi-fi")
            || adapter_name.contains("wifi")
            || adapter_name.contains("wireless")
            || adapter_name.contains("802.11")
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incoming_direct_message_is_stored_under_sender_conversation() {
        let frame = ChatMessageFrame {
            message_id: "msg-1".to_string(),
            conversation_id: "local-device".to_string(),
            sender_device_id: "peer-device".to_string(),
            content: "hello".to_string(),
            message_type: "text".to_string(),
            file_meta: None,
            encrypted: false,
            nonce: None,
            key_version: None,
            simulation: None,
            created_at: 10,
        };

        assert_eq!(
            "peer-device",
            inbound_conversation_id(&frame, "local-device")
        );
    }

    #[test]
    fn incoming_group_message_keeps_group_conversation() {
        let frame = ChatMessageFrame {
            message_id: "msg-1".to_string(),
            conversation_id: DEFAULT_GROUP_ID.to_string(),
            sender_device_id: "peer-device".to_string(),
            content: "hello".to_string(),
            message_type: "text".to_string(),
            file_meta: None,
            encrypted: false,
            nonce: None,
            key_version: None,
            simulation: None,
            created_at: 10,
        };

        assert_eq!(
            DEFAULT_GROUP_ID,
            inbound_conversation_id(&frame, "local-device")
        );
    }

    #[test]
    fn mdns_instance_name_is_short_and_stable() {
        let name = mdns_instance_name(
            "mac_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );

        assert!(name.len() <= 15);
        assert_eq!(name, mdns_instance_name("mac_0123456789abcdef"));
    }

    #[test]
    fn local_ip_address_returns_some_address() {
        assert!(!local_ip_address().is_empty());
    }
    #[test]
    fn peer_status_uses_source_address_and_local_seen_time() {
        let peer = peer_from_status_at(
            PeerStatusFrame {
                device_id: "peer-b".to_string(),
                nickname: "B".to_string(),
                avatar: None,
                includes_avatar: false,
                nickname_locked: false,
                address: Some("10.0.0.99".to_string()),
                protocol_version: 1,
                listen_port: 18145,
                client_kind: "full".to_string(),
                supports_chat: true,
                build_version: "0.3.0+1".to_string(),
                build_timestamp: 1,
                updated_at: 1,
            },
            "192.168.1.22".to_string(),
            30,
        );

        assert_eq!("192.168.1.22", peer.address);
        assert_eq!(30, peer.last_seen_at);
    }
}
