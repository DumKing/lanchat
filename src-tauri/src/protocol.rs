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
    Ping,
    Pong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloFrame {
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub protocol_version: u16,
    pub listen_port: u16,
    #[serde(default = "default_client_kind")]
    pub client_kind: String,
    #[serde(default = "default_supports_chat")]
    pub supports_chat: bool,
    pub public_key: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerStatusFrame {
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub address: Option<String>,
    pub protocol_version: u16,
    pub listen_port: u16,
    #[serde(default = "default_client_kind")]
    pub client_kind: String,
    #[serde(default = "default_supports_chat")]
    pub supports_chat: bool,
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
            created_at: 1_756_000_000,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert!(encoded.ends_with('\n'));
        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }

    #[test]
    fn hello_contains_protocol_identity() {
        let frame = WireFrame::Hello(HelloFrame {
            device_id: "device-a".to_string(),
            nickname: "LanChat A".to_string(),
            avatar: Some("data:image/png;base64,abc".to_string()),
            protocol_version: 1,
            listen_port: 18145,
            client_kind: "full".to_string(),
            supports_chat: true,
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
            created_at: 130,
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
    fn peer_status_round_trips() {
        let frame = WireFrame::PeerStatus(PeerStatusFrame {
            device_id: "aa:bb:cc:dd:ee:ff".to_string(),
            nickname: "Alice".to_string(),
            avatar: Some("A".to_string()),
            address: Some("192.168.1.20".to_string()),
            protocol_version: 1,
            listen_port: 18145,
            client_kind: "full".to_string(),
            supports_chat: true,
            updated_at: 10,
        });

        let encoded = encode_frame(&frame).expect("frame should encode");

        assert_eq!(decode_frame(&encoded).expect("frame should decode"), frame);
    }
}
