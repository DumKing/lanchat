use crate::file_server::FileMeta;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum WireFrame {
    Hello(HelloFrame),
    PeerStatus(PeerStatusFrame),
    ChatMessage(ChatMessageFrame),
    Ack(AckFrame),
    Game(GameFrame),
    PrivateChannelInvite(PrivateChannelInviteFrame),
    ChannelNotice(ChannelNoticeFrame),
    MessageRecall(MessageRecallFrame),
    QuickAlert(QuickAlertFrame),
    QuickAlertFeedback(QuickAlertFeedbackFrame),
    QuickAlertTrustReset(QuickAlertTrustResetFrame),
    AdminNickname(AdminNicknameFrame),
    AdminChannelControl(AdminChannelControlFrame),
    AdminDiscoMode(AdminDiscoModeFrame),
    AdminAlertMode(AdminAlertModeFrame),
    CallSignal(CallSignalFrame),
    Nudge(NudgeFrame),
    AdminAlertPushPolicy(AdminAlertPushPolicyFrame),
    AdminNotification(AdminNotificationFrame),
    AdminNotificationSubmission(AdminNotificationSubmissionFrame),
    AdminNotificationDecision(AdminNotificationDecisionFrame),
    FacePersonPolicy(FacePersonPolicyFrame),
    FaceMonitorPolicy(FaceMonitorPolicyFrame),
    CameraFaceAlert(CameraFaceAlertFrame),
    CameraFaceAlertFeedback(CameraFaceAlertFeedbackFrame),
    AdminRemoteUpdate(AdminRemoteUpdateFrame),
    Ping,
    Pong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloFrame {
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub nickname_locked: bool,
    pub protocol_version: u16,
    pub listen_port: u16,
    #[serde(default = "default_client_kind")]
    pub client_kind: String,
    #[serde(default = "default_supports_chat")]
    pub supports_chat: bool,
    #[serde(default)]
    pub build_version: String,
    #[serde(default)]
    pub build_timestamp: i64,
    pub public_key: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerStatusFrame {
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    #[serde(default)]
    pub includes_avatar: bool,
    #[serde(default)]
    pub nickname_locked: bool,
    pub address: Option<String>,
    pub protocol_version: u16,
    pub listen_port: u16,
    #[serde(default = "default_client_kind")]
    pub client_kind: String,
    #[serde(default = "default_supports_chat")]
    pub supports_chat: bool,
    #[serde(default)]
    pub build_version: String,
    #[serde(default)]
    pub build_timestamp: i64,
    pub updated_at: i64,
}

fn default_client_kind() -> String {
    "full".to_string()
}

fn default_supports_chat() -> bool {
    true
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageFrame {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_device_id: String,
    pub content: String,
    pub message_type: String,
    pub file_meta: Option<FileMeta>,
    #[serde(default)]
    pub encrypted: bool,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub key_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation: Option<SimulationMeta>,
    pub created_at: i64,
}

/// 由本机超级管理员代为发布时的可追溯操作信息。
/// 业务展示仍使用消息中的 sender_device_id，审计与可选标签使用本结构。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationMeta {
    pub operator_device_id: String,
    pub operator_nickname: String,
    pub display_label: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckFrame {
    pub message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminNicknameFrame {
    pub target_device_id: String,
    pub nickname: String,
    #[serde(default)]
    pub nickname_locked: Option<bool>,
    #[serde(default)]
    pub use_system_username: bool,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminChannelControlFrame {
    pub target_device_id: String,
    pub channel_id: String,
    pub action: String,
    pub muted: bool,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelMemberFrame {
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateChannelInviteFrame {
    pub channel_id: String,
    pub title: String,
    pub owner_device_id: String,
    pub owner_nickname: String,
    pub channel_key: String,
    pub key_version: u32,
    pub members: Vec<ChannelMemberFrame>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelNoticeFrame {
    pub conversation_id: String,
    pub notice: String,
    pub updated_by_device_id: String,
    pub updated_by_nickname: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRecallFrame {
    pub message_id: String,
    pub conversation_id: String,
    pub sender_device_id: String,
    pub recalled_at: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAlertFrame {
    pub alert_id: String,
    pub sender_device_id: String,
    pub sender_nickname: String,
    #[serde(default)]
    pub sender_address: Option<String>,
    pub content: String,
    #[serde(default = "default_alert_mode")]
    pub mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation: Option<SimulationMeta>,
    pub created_at: i64,
}

fn default_alert_mode() -> String {
    "normal".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAlertFeedbackFrame {
    pub alert_id: String,
    pub alert_sender_device_id: String,
    pub responder_device_id: String,
    pub responder_nickname: String,
    pub result: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickAlertTrustResetFrame {
    pub target_device_id: String,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallSignalFrame {
    pub call_id: String,
    pub sender_device_id: String,
    pub sender_nickname: String,
    /// offer | answer | ice_candidate | hangup | reject
    pub kind: String,
    #[serde(default)]
    pub media: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NudgeFrame {
    pub nudge_id: String,
    pub sender_device_id: String,
    pub sender_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminAlertPushPolicyFrame {
    /// A concrete device id, or * for all online devices.
    pub target_device_id: String,
    /// 0-100. Alerts below the configured credibility do not trigger external group bots.
    pub min_credibility: u8,
    /// When enabled, only a later administrator policy can change this threshold.
    #[serde(default)]
    pub min_credibility_locked: bool,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminDiscoModeFrame {
    pub target_device_id: String,
    pub duration_ms: u64,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminAlertModeFrame {
    pub target_device_id: String,
    pub mode: String,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminRemoteUpdateFrame {
    pub command_id: String,
    pub target_device_id: String,
    pub target_version: String,
    #[serde(default)]
    pub package: Option<FileMeta>,
    #[serde(default)]
    pub package_sha256: Option<String>,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

/// The reference image is transferred through the issuer's LAN file server,
/// keeping the JSON-lines control channel small and bounded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FacePersonPolicyFrame {
    pub person_id: String,
    pub display_name: String,
    #[serde(default)]
    pub photo_url: Option<String>,
    #[serde(default)]
    pub photo_urls: Vec<String>,
    #[serde(default)]
    pub photo_sha256: Option<String>,
    #[serde(default)]
    pub photo_sha256s: Vec<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    pub enabled: bool,
    pub version: i64,
    /// upsert | disable | delete
    pub action: String,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub issued_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaceMonitorPolicyFrame {
    /// * targets all currently online devices, otherwise a concrete device id.
    pub target_device_id: String,
    pub min_confidence: u8,
    #[serde(default = "default_body_min_confidence")]
    pub body_min_confidence: u8,
    #[serde(default = "default_face_monitor_sample_fps")]
    pub sample_fps: u8,
    pub consecutive_hits: u8,
    /// Legacy fallback used by clients older than the split-cooldown protocol.
    pub cooldown_seconds: u32,
    #[serde(default)]
    pub face_cooldown_seconds: u32,
    #[serde(default)]
    pub body_cooldown_seconds: u32,
    #[serde(default)]
    pub settings_locked: bool,
    pub version: i64,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub issued_at: i64,
}

fn default_body_min_confidence() -> u8 {
    68
}

fn default_face_monitor_sample_fps() -> u8 {
    2
}

/// A metadata-only automatic alert. It intentionally contains no frame, image
/// URL, embedding, or any other camera evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraFaceAlertFrame {
    pub alert_id: String,
    pub source_kind: String,
    pub source_device_id: String,
    pub source_nickname: String,
    #[serde(default)]
    pub source_address: Option<String>,
    pub person_id: String,
    pub person_name: String,
    pub confidence: u8,
    #[serde(default = "default_confirmed_recognition_level")]
    pub recognition_level: String,
    #[serde(default)]
    pub face_confidence: Option<u8>,
    #[serde(default)]
    pub body_confidence: Option<u8>,
    pub consecutive_hits: u8,
    pub policy_version: i64,
    pub created_at: i64,
}

fn default_confirmed_recognition_level() -> String {
    "confirmed".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraFaceAlertFeedbackFrame {
    pub alert_id: String,
    pub source_device_id: String,
    pub responder_device_id: String,
    pub responder_nickname: String,
    /// real | false
    pub result: String,
    pub created_at: i64,
}

/// 超管下发给指定设备的通知。确认型通知由目标设备提交后，再由超管审核放行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminNotificationFrame {
    pub notification_id: String,
    pub target_device_id: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub template: String,
    #[serde(default)]
    pub support_url: Option<String>,
    /// dismissible | requires_confirmation
    pub display_mode: String,
    #[serde(default)]
    pub deadline_at: Option<i64>,
    /// auto_release | manual_review | keep_locked
    #[serde(default)]
    pub timeout_policy: String,
    #[serde(default)]
    pub force_open_main_window: bool,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminNotificationSubmissionFrame {
    pub notification_id: String,
    pub target_device_id: String,
    pub submitted_by_device_id: String,
    pub submitted_by_nickname: String,
    pub submitted_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminNotificationDecisionFrame {
    pub notification_id: String,
    pub target_device_id: String,
    /// approved | rejected | revoked
    pub decision: String,
    pub decided_by_device_id: String,
    pub decided_by_nickname: String,
    pub decided_at: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameFrame {
    pub frame_id: String,
    pub game: String,
    pub room_id: String,
    pub sender_device_id: String,
    pub sender_nickname: String,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created_at: i64,
}

pub fn encode_frame(frame: &WireFrame) -> Result<String, serde_json::Error> {
    serde_json::to_string(frame).map(|mut line| {
        line.push('\n');
        line
    })
}

pub fn decode_frame(input: &str) -> Result<WireFrame, serde_json::Error> {
    serde_json::from_str(input.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_round_trips_as_json_line() {
        let frame = WireFrame::ChatMessage(ChatMessageFrame {
            message_id: "msg-1".to_string(),
            conversation_id: "peer-device".to_string(),
            sender_device_id: "local-device".to_string(),
            content: "你好，局域网".to_string(),
            message_type: "text".to_string(),
            file_meta: None,
            encrypted: false,
            nonce: None,
            key_version: None,
            simulation: None,
            created_at: 1_756_000_000,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert!(encoded.ends_with('\n'));
        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn face_person_policy_round_trips_without_embedding_photo_bytes() {
        let frame = WireFrame::FacePersonPolicy(FacePersonPolicyFrame {
            person_id: "person-1".to_string(),
            display_name: "测试人员".to_string(),
            photo_url: Some("http://192.168.1.2:1234/files/a/photo.jpg".to_string()),
            photo_urls: vec![],
            photo_sha256: Some("abc".to_string()),
            photo_sha256s: vec![],
            expires_at: None,
            enabled: true,
            version: 2,
            action: "upsert".to_string(),
            issued_by_device_id: "admin".to_string(),
            issued_by_nickname: "管理员".to_string(),
            issued_at: 10,
        });
        let encoded = encode_frame(&frame).expect("face frame encodes");
        assert!(!encoded.contains("photo_base64"));
        assert_eq!(decode_frame(&encoded).expect("face frame decodes"), frame);
    }

    #[test]
    fn face_monitor_policy_round_trips() {
        let frame = WireFrame::FaceMonitorPolicy(FaceMonitorPolicyFrame {
            target_device_id: "*".to_string(),
            min_confidence: 82,
            body_min_confidence: 72,
            sample_fps: 3,
            consecutive_hits: 3,
            cooldown_seconds: 60,
            face_cooldown_seconds: 45,
            body_cooldown_seconds: 90,
            settings_locked: true,
            version: 4,
            issued_by_device_id: "admin".to_string(),
            issued_by_nickname: "管理员".to_string(),
            issued_at: 10,
        });
        let encoded = encode_frame(&frame).expect("policy frame encodes");
        assert_eq!(decode_frame(&encoded).expect("policy frame decodes"), frame);
    }

    #[test]
    fn camera_face_alert_round_trips_without_camera_evidence() {
        let frame = WireFrame::CameraFaceAlert(CameraFaceAlertFrame {
            alert_id: "face-alert-1".to_string(),
            source_kind: "camera_face".to_string(),
            source_device_id: "device-a".to_string(),
            source_nickname: "摄像头设备".to_string(),
            source_address: Some("192.168.1.9".to_string()),
            person_id: "person-1".to_string(),
            person_name: "指定人员".to_string(),
            confidence: 91,
            recognition_level: "confirmed".to_string(),
            face_confidence: Some(91),
            body_confidence: None,
            consecutive_hits: 2,
            policy_version: 3,
            created_at: 10,
        });
        let encoded = encode_frame(&frame).expect("automatic alert encodes");
        assert!(!encoded.contains("photo"));
        assert!(!encoded.contains("embedding"));
        assert_eq!(
            decode_frame(&encoded).expect("automatic alert decodes"),
            frame
        );
    }

    #[test]
    fn hello_contains_protocol_identity() {
        let frame = WireFrame::Hello(HelloFrame {
            device_id: "device-a".to_string(),
            nickname: "LanChat A".to_string(),
            avatar: Some("data:image/png;base64,abc".to_string()),
            nickname_locked: false,
            protocol_version: 1,
            listen_port: 18145,
            client_kind: "full".to_string(),
            supports_chat: true,
            build_version: "0.3.0+1".to_string(),
            build_timestamp: 1,
            public_key: None,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");
        let decoded = decode_frame(&encoded).expect("frame should decode");

        assert_eq!(decoded, frame);
    }
    #[test]
    fn game_frame_round_trips() {
        let frame = WireFrame::Game(GameFrame {
            frame_id: "frame-1".to_string(),
            game: "doudizhu".to_string(),
            room_id: "room-1".to_string(),
            sender_device_id: "device-a".to_string(),
            sender_nickname: "Alice".to_string(),
            kind: "room_snapshot".to_string(),
            payload: serde_json::json!({ "players": 3, "phase": "playing" }),
            created_at: 20,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn admin_nickname_round_trips() {
        let frame = WireFrame::AdminNickname(AdminNicknameFrame {
            target_device_id: "aa:bb:cc:dd:ee:ff".to_string(),
            nickname: "新昵称".to_string(),
            nickname_locked: Some(true),
            use_system_username: false,
            issued_by_device_id: "11:22:33:44:55:66".to_string(),
            issued_by_nickname: "管理员".to_string(),
            created_at: 30,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn private_channel_invite_round_trips() {
        let frame = WireFrame::PrivateChannelInvite(PrivateChannelInviteFrame {
            channel_id: "private-1".to_string(),
            title: "午休私有频道".to_string(),
            owner_device_id: "owner-1".to_string(),
            owner_nickname: "群主".to_string(),
            channel_key: "key".to_string(),
            key_version: 1,
            members: vec![ChannelMemberFrame {
                device_id: "peer-1".to_string(),
                nickname: "成员".to_string(),
                avatar: None,
            }],
            created_at: 100,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }
    #[test]
    fn channel_notice_round_trips() {
        let frame = WireFrame::ChannelNotice(ChannelNoticeFrame {
            conversation_id: "lan-room".to_string(),
            notice: "今天下午一起联机".to_string(),
            updated_by_device_id: "admin-1".to_string(),
            updated_by_nickname: "管理员".to_string(),
            updated_at: 120,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }
    #[test]
    fn quick_alert_round_trips() {
        let frame = WireFrame::QuickAlert(QuickAlertFrame {
            alert_id: "alert-1".to_string(),
            sender_device_id: "device-a".to_string(),
            sender_nickname: "Alice".to_string(),
            sender_address: Some("192.168.1.23".to_string()),
            content: "快捷告警".to_string(),
            mode: "disco".to_string(),
            simulation: None,
            created_at: 130,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn call_signal_round_trips() {
        let frame = WireFrame::CallSignal(CallSignalFrame {
            call_id: "call-1".to_string(),
            sender_device_id: "device-a".to_string(),
            sender_nickname: "Alice".to_string(),
            kind: "offer".to_string(),
            media: "video".to_string(),
            payload: serde_json::json!({ "type": "offer", "sdp": "v=0" }),
            created_at: 131,
        });

        assert_eq!(decode_frame(&encode_frame(&frame).unwrap()).unwrap(), frame);
    }

    #[test]
    fn nudge_round_trips() {
        let frame = WireFrame::Nudge(NudgeFrame {
            nudge_id: "nudge-1".to_string(),
            sender_device_id: "device-a".to_string(),
            sender_nickname: "Alice".to_string(),
            created_at: 132,
        });

        assert_eq!(decode_frame(&encode_frame(&frame).unwrap()).unwrap(), frame);
    }

    #[test]
    fn admin_alert_push_policy_round_trips() {
        let frame = WireFrame::AdminAlertPushPolicy(AdminAlertPushPolicyFrame {
            target_device_id: "*".to_string(),
            min_credibility: 50,
            min_credibility_locked: true,
            issued_by_device_id: "admin-1".to_string(),
            issued_by_nickname: "管理员".to_string(),
            created_at: 132,
        });

        assert_eq!(decode_frame(&encode_frame(&frame).unwrap()).unwrap(), frame);
    }

    #[test]
    fn simulation_metadata_round_trips() {
        let frame = WireFrame::ChatMessage(ChatMessageFrame {
            message_id: "simulated-1".to_string(),
            conversation_id: "lan-room".to_string(),
            sender_device_id: "device-simulated".to_string(),
            content: "模拟消息".to_string(),
            message_type: "text".to_string(),
            file_meta: None,
            encrypted: false,
            nonce: None,
            key_version: None,
            simulation: Some(SimulationMeta {
                operator_device_id: "device-admin".to_string(),
                operator_nickname: "超级管理员".to_string(),
                display_label: true,
                created_at: 140,
            }),
            created_at: 140,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn quick_alert_feedback_round_trips() {
        let frame = WireFrame::QuickAlertFeedback(QuickAlertFeedbackFrame {
            alert_id: "alert-1".to_string(),
            alert_sender_device_id: "device-a".to_string(),
            responder_device_id: "device-b".to_string(),
            responder_nickname: "Bob".to_string(),
            result: "real".to_string(),
            created_at: 140,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn quick_alert_trust_reset_round_trips() {
        let frame = WireFrame::QuickAlertTrustReset(QuickAlertTrustResetFrame {
            target_device_id: "device-a".to_string(),
            issued_by_device_id: "admin-1".to_string(),
            issued_by_nickname: "管理员".to_string(),
            created_at: 150,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn admin_disco_mode_round_trips() {
        let frame = WireFrame::AdminDiscoMode(AdminDiscoModeFrame {
            target_device_id: "device-a".to_string(),
            duration_ms: 120_000,
            issued_by_device_id: "admin-1".to_string(),
            issued_by_nickname: "管理员".to_string(),
            created_at: 160,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn admin_alert_mode_round_trips() {
        let frame = WireFrame::AdminAlertMode(AdminAlertModeFrame {
            target_device_id: "device-a".to_string(),
            mode: "disco".to_string(),
            issued_by_device_id: "admin-1".to_string(),
            issued_by_nickname: "管理员".to_string(),
            created_at: 170,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn admin_notification_frames_round_trip() {
        let notification = WireFrame::AdminNotification(AdminNotificationFrame {
            notification_id: "notice-1".to_string(),
            target_device_id: "device-a".to_string(),
            title: "请确认".to_string(),
            content: "请完成本次操作后提交确认".to_string(),
            template: "support".to_string(),
            support_url: Some("https://example.test/qrcode.png".to_string()),
            display_mode: "requires_confirmation".to_string(),
            deadline_at: Some(200),
            timeout_policy: "manual_review".to_string(),
            force_open_main_window: true,
            issued_by_device_id: "admin-1".to_string(),
            issued_by_nickname: "管理员".to_string(),
            created_at: 100,
        });
        assert_eq!(
            decode_frame(&encode_frame(&notification).unwrap()).unwrap(),
            notification
        );

        let decision = WireFrame::AdminNotificationDecision(AdminNotificationDecisionFrame {
            notification_id: "notice-1".to_string(),
            target_device_id: "device-a".to_string(),
            decision: "approved".to_string(),
            decided_by_device_id: "admin-1".to_string(),
            decided_by_nickname: "管理员".to_string(),
            decided_at: 220,
        });
        assert_eq!(
            decode_frame(&encode_frame(&decision).unwrap()).unwrap(),
            decision
        );
    }
    #[test]
    fn peer_status_round_trips() {
        let frame = WireFrame::PeerStatus(PeerStatusFrame {
            device_id: "aa:bb:cc:dd:ee:ff".to_string(),
            nickname: "Alice".to_string(),
            avatar: Some("A".to_string()),
            includes_avatar: true,
            nickname_locked: true,
            address: Some("192.168.1.20".to_string()),
            protocol_version: 1,
            listen_port: 18145,
            client_kind: "full".to_string(),
            supports_chat: true,
            build_version: "0.3.0+10".to_string(),
            build_timestamp: 10,
            updated_at: 10,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }
}
