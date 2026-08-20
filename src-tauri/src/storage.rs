use crate::file_server::FileMeta;
use crate::identity::{normalize_device_id, resolve_device_id, resolve_profile_device_id};
use crate::network::local_ip_address;
use crate::protocol::{
    AdminNotificationDecisionFrame, AdminNotificationFrame, CameraFaceAlertFeedbackFrame,
    CameraFaceAlertFrame, FaceMonitorPolicyFrame, FacePersonPolicyFrame, SimulationMeta,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

pub const DEFAULT_GROUP_ID: &str = "lan-room";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub device_id: String,
    pub nickname: String,
    pub listen_port: u16,
    pub avatar: Option<String>,
    pub nickname_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Peer {
    pub device_id: String,
    pub nickname: String,
    pub note: Option<String>,
    pub avatar: Option<String>,
    pub address: String,
    pub port: u16,
    pub online: bool,
    pub last_seen_at: i64,
    pub client_kind: String,
    pub supports_chat: bool,
    pub nickname_locked: bool,
    pub build_version: String,
    pub build_timestamp: i64,
}

impl Peer {
    pub fn supports_full_features(&self) -> bool {
        self.supports_chat
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub kind: ConversationKind,
    pub peer_device_id: Option<String>,
    pub updated_at: i64,
    pub unread_count: u32,
    pub is_private: bool,
    pub owner_device_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Direct,
    Group,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelMemberSeed {
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelMember {
    pub channel_id: String,
    pub device_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub online: bool,
    pub last_seen_at: i64,
    pub is_owner: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateChannelRecord {
    pub id: String,
    pub title: String,
    pub owner_device_id: String,
    pub owner_nickname: String,
    pub channel_key: String,
    pub key_version: u32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacePersonRecord {
    pub person_id: String,
    pub display_name: String,
    pub photo_url: Option<String>,
    #[serde(default)]
    pub photo_urls: Vec<String>,
    pub photo_sha256: Option<String>,
    pub expires_at: Option<i64>,
    pub enabled: bool,
    pub version: i64,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub issued_at: i64,
    pub deleted_at: Option<i64>,
    #[serde(skip_serializing)]
    pub embedding: Option<Vec<u8>>,
    pub embedding_model_version: Option<String>,
    #[serde(default)]
    pub has_embedding: bool,
    #[serde(default)]
    pub has_body_embedding: bool,
    #[serde(default)]
    pub sample_count: u32,
}

#[derive(Debug, Clone)]
pub struct FacePersonSampleRecord {
    pub sample_id: String,
    pub person_id: String,
    pub photo_url: String,
    pub photo_sha256: Option<String>,
    pub embedding: Option<Vec<u8>>,
    pub embedding_model_version: Option<String>,
    pub body_embedding: Option<Vec<u8>>,
    pub body_embedding_model_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceMonitorPolicyRecord {
    pub target_device_id: String,
    pub min_confidence: u8,
    pub body_min_confidence: u8,
    pub sample_fps: u8,
    pub consecutive_hits: u8,
    pub cooldown_seconds: u32,
    pub face_cooldown_seconds: u32,
    pub body_cooldown_seconds: u32,
    pub settings_locked: bool,
    pub version: i64,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub issued_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraFaceAlertRecord {
    pub alert_id: String,
    pub source_kind: String,
    pub source_device_id: String,
    pub source_nickname: String,
    pub source_address: Option<String>,
    pub person_id: String,
    pub person_name: String,
    pub confidence: u8,
    pub recognition_level: String,
    pub face_confidence: Option<u8>,
    pub body_confidence: Option<u8>,
    pub consecutive_hits: u8,
    pub policy_version: i64,
    pub created_at: i64,
    pub feedback_real: u32,
    pub feedback_false: u32,
    pub local_feedback: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub sender_device_id: String,
    pub content: String,
    pub message_type: MessageType,
    pub file_meta: Option<FileMeta>,
    pub status: MessageStatus,
    pub simulation: Option<SimulationMeta>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationAudit {
    pub id: String,
    pub operator_device_id: String,
    pub operator_nickname: String,
    pub simulated_device_id: String,
    pub action_kind: String,
    pub target_id: Option<String>,
    pub display_label: bool,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminNotificationRecord {
    pub notification_id: String,
    pub target_device_id: String,
    pub title: String,
    pub content: String,
    pub template: String,
    pub support_url: Option<String>,
    pub display_mode: String,
    pub deadline_at: Option<i64>,
    pub timeout_policy: String,
    pub force_open_main_window: bool,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub created_at: i64,
    pub status: String,
    pub submitted_at: Option<i64>,
    pub decided_at: Option<i64>,
    pub decision_by_device_id: Option<String>,
    pub decision_by_nickname: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Text,
    File,
    Voice,
    System,
}

impl MessageType {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            MessageType::Text => "text",
            MessageType::File => "file",
            MessageType::Voice => "voice",
            MessageType::System => "system",
        }
    }

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "file" => MessageType::File,
            "voice" => MessageType::Voice,
            "system" => MessageType::System,
            _ => MessageType::Text,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Sending,
    Sent,
    Delivered,
    Failed,
}

impl MessageStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            MessageStatus::Sending => "sending",
            MessageStatus::Sent => "sent",
            MessageStatus::Delivered => "delivered",
            MessageStatus::Failed => "failed",
        }
    }

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "sending" => MessageStatus::Sending,
            "delivered" => MessageStatus::Delivered,
            "failed" => MessageStatus::Failed,
            _ => MessageStatus::Sent,
        }
    }
}

pub struct Storage {
    conn: Mutex<Connection>,
}

impl Storage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("创建数据目录失败：{err}"))?;
        }
        let conn = Connection::open(path).map_err(|err| format!("打开本地数据库失败：{err}"))?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init()?;
        Ok(storage)
    }

    fn init(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS profile (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                device_id TEXT NOT NULL,
                nickname TEXT NOT NULL,
                listen_port INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS peers (
                device_id TEXT PRIMARY KEY,
                nickname TEXT NOT NULL,
                address TEXT NOT NULL,
                port INTEGER NOT NULL,
                online INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS device_notes (
                device_id TEXT PRIMARY KEY,
                note TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                kind TEXT NOT NULL,
                peer_device_id TEXT,
                updated_at INTEGER NOT NULL,
                unread_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                sender_device_id TEXT NOT NULL,
                content TEXT NOT NULL,
                status TEXT NOT NULL,
                simulation_operator_device_id TEXT,
                simulation_operator_nickname TEXT,
                simulation_display_label INTEGER,
                simulation_created_at INTEGER,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS simulation_audits (
                id TEXT PRIMARY KEY,
                operator_device_id TEXT NOT NULL,
                operator_nickname TEXT NOT NULL,
                simulated_device_id TEXT NOT NULL,
                action_kind TEXT NOT NULL,
                target_id TEXT,
                display_label INTEGER NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS admin_notifications (
                notification_id TEXT PRIMARY KEY,
                target_device_id TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                template TEXT NOT NULL DEFAULT '',
                support_url TEXT,
                display_mode TEXT NOT NULL,
                deadline_at INTEGER,
                timeout_policy TEXT NOT NULL DEFAULT 'manual_review',
                force_open_main_window INTEGER NOT NULL DEFAULT 0,
                issued_by_device_id TEXT NOT NULL,
                issued_by_nickname TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                submitted_at INTEGER,
                decided_at INTEGER,
                decision_by_device_id TEXT,
                decision_by_nickname TEXT
            );
            CREATE TABLE IF NOT EXISTS private_channels (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                owner_device_id TEXT NOT NULL,
                owner_nickname TEXT NOT NULL,
                channel_key TEXT NOT NULL,
                key_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS channel_members (
                channel_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                nickname TEXT NOT NULL,
                avatar TEXT,
                invited_at INTEGER NOT NULL,
                PRIMARY KEY(channel_id, device_id)
            );
            CREATE TABLE IF NOT EXISTS channel_mutes (
                channel_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                muted INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY(channel_id, device_id)
            );
            CREATE TABLE IF NOT EXISTS face_people (
                person_id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                photo_url TEXT,
                photo_sha256 TEXT,
                expires_at INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                version INTEGER NOT NULL,
                issued_by_device_id TEXT NOT NULL,
                issued_by_nickname TEXT NOT NULL,
                issued_at INTEGER NOT NULL,
                deleted_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS face_person_samples (
                sample_id TEXT PRIMARY KEY,
                person_id TEXT NOT NULL,
                photo_url TEXT NOT NULL,
                photo_sha256 TEXT,
                embedding BLOB,
                embedding_model_version TEXT,
                body_embedding BLOB,
                body_embedding_model_version TEXT,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(person_id) REFERENCES face_people(person_id)
            );
            CREATE TABLE IF NOT EXISTS face_monitor_policies (
                target_device_id TEXT PRIMARY KEY,
                min_confidence INTEGER NOT NULL,
                body_min_confidence INTEGER NOT NULL DEFAULT 0,
                sample_fps INTEGER NOT NULL DEFAULT 0,
                consecutive_hits INTEGER NOT NULL,
                cooldown_seconds INTEGER NOT NULL,
                face_cooldown_seconds INTEGER NOT NULL DEFAULT 0,
                body_cooldown_seconds INTEGER NOT NULL DEFAULT 0,
                settings_locked INTEGER NOT NULL DEFAULT 0,
                version INTEGER NOT NULL,
                issued_by_device_id TEXT NOT NULL,
                issued_by_nickname TEXT NOT NULL,
                issued_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS camera_face_alerts (
                alert_id TEXT PRIMARY KEY,
                source_device_id TEXT NOT NULL,
                source_nickname TEXT NOT NULL,
                source_address TEXT,
                person_id TEXT NOT NULL,
                person_name TEXT NOT NULL,
                confidence INTEGER NOT NULL,
                recognition_level TEXT NOT NULL DEFAULT 'confirmed',
                face_confidence INTEGER,
                body_confidence INTEGER,
                consecutive_hits INTEGER NOT NULL,
                policy_version INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS camera_face_alert_feedbacks (
                alert_id TEXT NOT NULL,
                responder_device_id TEXT NOT NULL,
                responder_nickname TEXT NOT NULL,
                result TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY(alert_id, responder_device_id)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conversation_created
                ON messages(conversation_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_face_people_active
                ON face_people(enabled, expires_at, deleted_at);
            CREATE INDEX IF NOT EXISTS idx_face_person_samples_person
                ON face_person_samples(person_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_camera_face_alerts_created
                ON camera_face_alerts(created_at DESC);
            ",
        )
        .map_err(|err| format!("初始化本地数据库失败：{err}"))?;
        ensure_column(
            &conn,
            "messages",
            "message_type",
            "TEXT NOT NULL DEFAULT 'text'",
        )?;
        ensure_column(&conn, "messages", "file_name", "TEXT")?;
        ensure_column(&conn, "messages", "file_size", "INTEGER")?;
        ensure_column(&conn, "messages", "file_url", "TEXT")?;
        ensure_column(&conn, "messages", "file_mime_type", "TEXT")?;
        ensure_column(&conn, "messages", "file_duration_ms", "INTEGER")?;
        ensure_column(&conn, "messages", "simulation_operator_device_id", "TEXT")?;
        ensure_column(&conn, "messages", "simulation_operator_nickname", "TEXT")?;
        ensure_column(&conn, "messages", "simulation_display_label", "INTEGER")?;
        ensure_column(&conn, "messages", "simulation_created_at", "INTEGER")?;
        ensure_column(&conn, "face_people", "embedding", "BLOB")?;
        ensure_column(&conn, "face_people", "embedding_model_version", "TEXT")?;
        ensure_column(&conn, "face_person_samples", "body_embedding", "BLOB")?;
        ensure_column(
            &conn,
            "face_person_samples",
            "body_embedding_model_version",
            "TEXT",
        )?;
        ensure_column(
            &conn,
            "camera_face_alerts",
            "source_kind",
            "TEXT NOT NULL DEFAULT 'camera_face_presence'",
        )?;
        ensure_column(
            &conn,
            "camera_face_alerts",
            "recognition_level",
            "TEXT NOT NULL DEFAULT 'confirmed'",
        )?;
        ensure_column(&conn, "camera_face_alerts", "face_confidence", "INTEGER")?;
        ensure_column(&conn, "camera_face_alerts", "body_confidence", "INTEGER")?;
        ensure_column(
            &conn,
            "face_monitor_policies",
            "body_min_confidence",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "face_monitor_policies",
            "sample_fps",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "face_monitor_policies",
            "settings_locked",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "face_monitor_policies",
            "face_cooldown_seconds",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "face_monitor_policies",
            "body_cooldown_seconds",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(&conn, "profile", "avatar", "TEXT")?;
        ensure_column(
            &conn,
            "profile",
            "nickname_locked",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(&conn, "peers", "avatar", "TEXT")?;
        ensure_column(
            &conn,
            "peers",
            "client_kind",
            "TEXT NOT NULL DEFAULT 'full'",
        )?;
        ensure_column(
            &conn,
            "peers",
            "supports_chat",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        ensure_column(
            &conn,
            "peers",
            "nickname_locked",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(&conn, "peers", "build_version", "TEXT NOT NULL DEFAULT ''")?;
        ensure_column(
            &conn,
            "peers",
            "build_timestamp",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "channel_members",
            "muted",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            &conn,
            "admin_notifications",
            "force_open_main_window",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        crate::vision::storage::initialize(&conn)?;
        Ok(())
    }

    pub fn vision_schema_version(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.query_row(
            "SELECT MAX(version) FROM vision_schema_migrations",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(|error| format!("读取视觉识别数据库版本失败：{error}"))?
        .ok_or_else(|| "视觉识别数据库版本缺失".to_string())
    }

    pub fn legacy_face_alert_count(&self) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.query_row("SELECT COUNT(*) FROM camera_face_alerts", [], |row| row.get(0))
            .map_err(|error| format!("读取历史视觉告警失败：{error}"))
    }

    pub fn record_vision_remote_command(
        &self,
        issuer_device_id: &str,
        target_device_id: &str,
        command_id: &str,
        nonce: &str,
        processed_at: i64,
        result_json: &str,
    ) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|error| format!("开始远程视觉命令事务失败：{error}"))?;
        let result = (|| {
            let existing = conn
                .query_row(
                    "SELECT result_json FROM vision_remote_command_nonces WHERE issuer_device_id=?1 AND target_device_id=?2 AND command_id=?3",
                    (issuer_device_id, target_device_id, command_id),
                    |row| row.get::<_, String>(0),
                )
                .ok();
            if let Some(result) = existing {
                return Ok(result);
            }
            conn.execute(
                "INSERT INTO vision_remote_command_nonces(nonce,command_id,issuer_device_id,target_device_id,expires_at,result_json,processed_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                (nonce, command_id, issuer_device_id, target_device_id, processed_at + 300, result_json, processed_at),
            )
            .map_err(|error| format!("写入远程视觉命令收据失败：{error}"))?;
            Ok(result_json.to_string())
        })();
        match result {
            Ok(result) => {
                conn.execute_batch("COMMIT")
                    .map_err(|error| format!("提交远程视觉命令事务失败：{error}"))?;
                Ok(result)
            }
            Err(error) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    /// 可重复执行的兼容迁移。后续 V5 特征重算只读取新表，旧表保留一个兼容周期。
    pub fn migrate_legacy_vision_data(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        crate::vision::storage::copy_legacy_reference_images(&conn)
    }

    pub fn vision_reference_image_count(&self, person_id: &str) -> Result<i64, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.query_row(
            "SELECT COUNT(*) FROM person_reference_images_v2 WHERE person_id=?1",
            params![person_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取视觉参考图数量失败：{error}"))
    }

    pub fn upsert_face_person(
        &self,
        frame: &FacePersonPolicyFrame,
    ) -> Result<FacePersonRecord, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let current = conn
            .query_row(
                "SELECT version FROM face_people WHERE person_id=?1",
                params![frame.person_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|err| format!("读取人员规则版本失败：{err}"))?;
        if current.is_some_and(|version| version > frame.version) {
            return Self::read_face_person(&conn, &frame.person_id);
        }
        let action = frame.action.trim().to_ascii_lowercase();
        let enabled = frame.enabled && action == "upsert";
        let deleted_at = (action == "delete").then(|| chrono::Utc::now().timestamp_millis());
        conn.execute(
            "INSERT INTO face_people (person_id,display_name,photo_url,photo_sha256,expires_at,enabled,version,issued_by_device_id,issued_by_nickname,issued_at,deleted_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(person_id) DO UPDATE SET display_name=excluded.display_name,photo_url=excluded.photo_url,photo_sha256=excluded.photo_sha256,expires_at=excluded.expires_at,enabled=excluded.enabled,version=excluded.version,issued_by_device_id=excluded.issued_by_device_id,issued_by_nickname=excluded.issued_by_nickname,issued_at=excluded.issued_at,deleted_at=excluded.deleted_at",
            params![frame.person_id, frame.display_name, frame.photo_url, frame.photo_sha256, frame.expires_at, enabled as i32, frame.version, frame.issued_by_device_id, frame.issued_by_nickname, frame.issued_at, deleted_at],
        ).map_err(|err| format!("保存人员规则失败：{err}"))?;
        if action == "upsert" && !frame.photo_urls.is_empty() {
            let samples = frame
                .photo_urls
                .iter()
                .take(12)
                .enumerate()
                .map(|(index, photo_url)| FacePersonSampleRecord {
                    sample_id: format!("{}-v{}-{}", frame.person_id, frame.version, index),
                    person_id: frame.person_id.clone(),
                    photo_url: photo_url.clone(),
                    photo_sha256: frame.photo_sha256s.get(index).cloned(),
                    embedding: None,
                    embedding_model_version: None,
                    body_embedding: None,
                    body_embedding_model_version: None,
                })
                .collect::<Vec<_>>();
            drop(conn);
            self.replace_face_person_samples(&frame.person_id, &samples)?;
            let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
            return Self::read_face_person(&conn, &frame.person_id);
        }
        Self::read_face_person(&conn, &frame.person_id)
    }

    pub fn list_face_people(&self) -> Result<Vec<FacePersonRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn.prepare("SELECT person_id,display_name,photo_url,photo_sha256,expires_at,enabled,version,issued_by_device_id,issued_by_nickname,issued_at,deleted_at,embedding,embedding_model_version,(SELECT COUNT(*) FROM face_person_samples s WHERE s.person_id=face_people.person_id),(SELECT COUNT(*) FROM face_person_samples s WHERE s.person_id=face_people.person_id AND s.body_embedding IS NOT NULL) FROM face_people WHERE deleted_at IS NULL ORDER BY issued_at DESC")
            .map_err(|err| format!("读取人员规则失败：{err}"))?;
        let rows = stmt
            .query_map([], Self::face_person_from_row)
            .map_err(|err| format!("读取人员规则失败：{err}"))?;
        let mut records = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("读取人员规则失败：{err}"))?;
        drop(stmt);
        for record in &mut records {
            record.photo_urls = Self::face_person_photo_urls(&conn, record)?;
        }
        Ok(records)
    }

    pub fn replace_face_person_samples(
        &self,
        person_id: &str,
        samples: &[FacePersonSampleRecord],
    ) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let transaction = conn
            .transaction()
            .map_err(|err| format!("保存人员样本失败：{err}"))?;
        transaction
            .execute(
                "DELETE FROM face_person_samples WHERE person_id=?1",
                params![person_id],
            )
            .map_err(|err| format!("清理旧人员样本失败：{err}"))?;
        for sample in samples {
            transaction.execute(
                "INSERT INTO face_person_samples (sample_id,person_id,photo_url,photo_sha256,embedding,embedding_model_version,body_embedding,body_embedding_model_version,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![sample.sample_id, sample.person_id, sample.photo_url, sample.photo_sha256, sample.embedding, sample.embedding_model_version, sample.body_embedding, sample.body_embedding_model_version, chrono::Utc::now().timestamp_millis()],
            ).map_err(|err| format!("保存人员样本失败：{err}"))?;
        }
        transaction
            .commit()
            .map_err(|err| format!("提交人员样本失败：{err}"))
    }

    pub fn list_face_person_samples(
        &self,
        person_id: &str,
    ) -> Result<Vec<FacePersonSampleRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn.prepare("SELECT sample_id,person_id,photo_url,photo_sha256,embedding,embedding_model_version,body_embedding,body_embedding_model_version FROM face_person_samples WHERE person_id=?1 ORDER BY created_at ASC")
            .map_err(|err| format!("读取人员样本失败：{err}"))?;
        let rows = stmt
            .query_map(params![person_id], |row| {
                Ok(FacePersonSampleRecord {
                    sample_id: row.get(0)?,
                    person_id: row.get(1)?,
                    photo_url: row.get(2)?,
                    photo_sha256: row.get(3)?,
                    embedding: row.get(4)?,
                    embedding_model_version: row.get(5)?,
                    body_embedding: row.get(6)?,
                    body_embedding_model_version: row.get(7)?,
                })
            })
            .map_err(|err| format!("读取人员样本失败：{err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("读取人员样本失败：{err}"))
    }

    /// 仅在本机更新人脸特征，特征不参与局域网同步。
    pub fn update_face_person_embedding(
        &self,
        person_id: &str,
        embedding: Option<Vec<u8>>,
        model_version: Option<String>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let changed = conn.execute(
            "UPDATE face_people SET embedding=?1, embedding_model_version=?2 WHERE person_id=?3",
            params![embedding.as_deref(), model_version, person_id],
        ).map_err(|err| format!("保存人脸特征失败：{err}"))?;
        if changed == 0 {
            return Err("未找到识别人员".to_string());
        }
        Ok(())
    }

    /// A target device can always stop monitoring a person locally. This is not
    /// broadcast, so it cannot alter another device's authorised rule set.
    pub fn delete_face_person_local(&self, person_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let changed = conn
            .execute(
                "UPDATE face_people SET enabled=0, deleted_at=?1 WHERE person_id=?2",
                params![chrono::Utc::now().timestamp_millis(), person_id],
            )
            .map_err(|err| format!("删除本机识别人员失败：{err}"))?;
        if changed == 0 {
            return Err("未找到识别人员".to_string());
        }
        Ok(())
    }

    pub fn upsert_face_monitor_policy(
        &self,
        frame: &FaceMonitorPolicyFrame,
    ) -> Result<FaceMonitorPolicyRecord, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let current = conn
            .query_row(
                "SELECT version FROM face_monitor_policies WHERE target_device_id=?1",
                params![frame.target_device_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|err| format!("读取识别策略版本失败：{err}"))?;
        if current.is_some_and(|version| version > frame.version) {
            return Self::read_face_monitor_policy(&conn, &frame.target_device_id);
        }
        let legacy_cooldown = frame.cooldown_seconds.clamp(5, 86_400);
        let face_cooldown = if frame.face_cooldown_seconds == 0 {
            legacy_cooldown
        } else {
            frame.face_cooldown_seconds.clamp(5, 86_400)
        };
        let body_cooldown = if frame.body_cooldown_seconds == 0 {
            legacy_cooldown
        } else {
            frame.body_cooldown_seconds.clamp(5, 86_400)
        };
        conn.execute(
            "INSERT INTO face_monitor_policies (target_device_id,min_confidence,body_min_confidence,sample_fps,consecutive_hits,cooldown_seconds,face_cooldown_seconds,body_cooldown_seconds,settings_locked,version,issued_by_device_id,issued_by_nickname,issued_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(target_device_id) DO UPDATE SET min_confidence=excluded.min_confidence,body_min_confidence=excluded.body_min_confidence,sample_fps=excluded.sample_fps,consecutive_hits=excluded.consecutive_hits,cooldown_seconds=excluded.cooldown_seconds,face_cooldown_seconds=excluded.face_cooldown_seconds,body_cooldown_seconds=excluded.body_cooldown_seconds,settings_locked=excluded.settings_locked,version=excluded.version,issued_by_device_id=excluded.issued_by_device_id,issued_by_nickname=excluded.issued_by_nickname,issued_at=excluded.issued_at",
            params![frame.target_device_id, frame.min_confidence.clamp(1, 100), frame.body_min_confidence.clamp(1, 100), frame.sample_fps.clamp(1, 5), frame.consecutive_hits.clamp(1, 20), legacy_cooldown, face_cooldown, body_cooldown, frame.settings_locked as i32, frame.version, frame.issued_by_device_id, frame.issued_by_nickname, frame.issued_at],
        ).map_err(|err| format!("保存识别策略失败：{err}"))?;
        Self::read_face_monitor_policy(&conn, &frame.target_device_id)
    }

    pub fn effective_face_monitor_policy(
        &self,
        device_id: &str,
    ) -> Result<Option<FaceMonitorPolicyRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.query_row(
            "SELECT target_device_id,min_confidence,body_min_confidence,sample_fps,consecutive_hits,cooldown_seconds,face_cooldown_seconds,body_cooldown_seconds,settings_locked,version,issued_by_device_id,issued_by_nickname,issued_at FROM face_monitor_policies WHERE target_device_id IN (?1, '*') ORDER BY CASE WHEN target_device_id=?1 THEN 0 ELSE 1 END LIMIT 1",
            params![device_id],
            Self::face_monitor_policy_from_row,
        ).optional().map_err(|err| format!("读取识别策略失败：{err}"))
    }

    fn read_face_person(conn: &Connection, person_id: &str) -> Result<FacePersonRecord, String> {
        let mut record = conn
            .query_row("SELECT person_id,display_name,photo_url,photo_sha256,expires_at,enabled,version,issued_by_device_id,issued_by_nickname,issued_at,deleted_at,embedding,embedding_model_version,(SELECT COUNT(*) FROM face_person_samples s WHERE s.person_id=face_people.person_id),(SELECT COUNT(*) FROM face_person_samples s WHERE s.person_id=face_people.person_id AND s.body_embedding IS NOT NULL) FROM face_people WHERE person_id=?1", params![person_id], Self::face_person_from_row)
            .map_err(|err| format!("读取人员规则失败：{err}"))?;
        record.photo_urls = Self::face_person_photo_urls(conn, &record)?;
        Ok(record)
    }

    fn face_person_photo_urls(
        conn: &Connection,
        record: &FacePersonRecord,
    ) -> Result<Vec<String>, String> {
        let mut stmt = conn
            .prepare("SELECT photo_url FROM face_person_samples WHERE person_id=?1 ORDER BY created_at ASC, rowid ASC")
            .map_err(|err| format!("读取人员样本照片失败：{err}"))?;
        let rows = stmt
            .query_map(params![record.person_id], |row| row.get::<_, String>(0))
            .map_err(|err| format!("读取人员样本照片失败：{err}"))?;
        let mut urls = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("读取人员样本照片失败：{err}"))?;
        if urls.is_empty() {
            if let Some(photo_url) = record.photo_url.as_ref().filter(|value| !value.is_empty()) {
                urls.push(photo_url.clone());
            }
        }
        let mut seen = HashSet::new();
        urls.retain(|url| seen.insert(url.clone()));
        Ok(urls)
    }

    fn face_person_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FacePersonRecord> {
        let embedding: Option<Vec<u8>> = row.get(11)?;
        Ok(FacePersonRecord {
            person_id: row.get(0)?,
            display_name: row.get(1)?,
            photo_url: row.get(2)?,
            photo_urls: Vec::new(),
            photo_sha256: row.get(3)?,
            expires_at: row.get(4)?,
            enabled: row.get::<_, i32>(5)? != 0,
            version: row.get(6)?,
            issued_by_device_id: row.get(7)?,
            issued_by_nickname: row.get(8)?,
            issued_at: row.get(9)?,
            deleted_at: row.get(10)?,
            has_embedding: embedding.is_some(),
            embedding,
            embedding_model_version: row.get(12)?,
            sample_count: row.get::<_, i64>(13)? as u32,
            has_body_embedding: row.get::<_, i64>(14)? > 0,
        })
    }

    fn read_face_monitor_policy(
        conn: &Connection,
        target_device_id: &str,
    ) -> Result<FaceMonitorPolicyRecord, String> {
        conn.query_row("SELECT target_device_id,min_confidence,body_min_confidence,sample_fps,consecutive_hits,cooldown_seconds,face_cooldown_seconds,body_cooldown_seconds,settings_locked,version,issued_by_device_id,issued_by_nickname,issued_at FROM face_monitor_policies WHERE target_device_id=?1", params![target_device_id], Self::face_monitor_policy_from_row)
            .map_err(|err| format!("读取识别策略失败：{err}"))
    }

    fn face_monitor_policy_from_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<FaceMonitorPolicyRecord> {
        Ok(FaceMonitorPolicyRecord {
            target_device_id: row.get(0)?,
            min_confidence: row.get(1)?,
            body_min_confidence: {
                let value = row.get::<_, u8>(2)?;
                if value == 0 {
                    row.get::<_, u8>(1)?.max(68)
                } else {
                    value
                }
            },
            sample_fps: {
                let value = row.get::<_, u8>(3)?;
                if value == 0 {
                    2
                } else {
                    value.clamp(1, 5)
                }
            },
            consecutive_hits: row.get(4)?,
            cooldown_seconds: row.get(5)?,
            face_cooldown_seconds: {
                let value = row.get::<_, u32>(6)?;
                if value == 0 {
                    row.get(5)?
                } else {
                    value
                }
            },
            body_cooldown_seconds: {
                let value = row.get::<_, u32>(7)?;
                if value == 0 {
                    row.get(5)?
                } else {
                    value
                }
            },
            settings_locked: row.get::<_, i32>(8)? != 0,
            version: row.get(9)?,
            issued_by_device_id: row.get(10)?,
            issued_by_nickname: row.get(11)?,
            issued_at: row.get(12)?,
        })
    }

    pub fn upsert_camera_face_alert(
        &self,
        frame: &CameraFaceAlertFrame,
    ) -> Result<CameraFaceAlertRecord, String> {
        if !matches!(
            frame.source_kind.as_str(),
            "camera_face" | "camera_face_presence" | "camera_person"
        ) {
            return Err("自动识别告警来源无效".to_string());
        }
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "INSERT INTO camera_face_alerts (alert_id,source_kind,source_device_id,source_nickname,source_address,person_id,person_name,confidence,recognition_level,face_confidence,body_confidence,consecutive_hits,policy_version,created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
             ON CONFLICT(alert_id) DO NOTHING",
            params![frame.alert_id, frame.source_kind, frame.source_device_id, frame.source_nickname, frame.source_address, frame.person_id, frame.person_name, frame.confidence.min(100), frame.recognition_level, frame.face_confidence.map(|v| v.min(100)), frame.body_confidence.map(|v| v.min(100)), frame.consecutive_hits.max(1), frame.policy_version, frame.created_at],
        ).map_err(|err| format!("保存自动识别告警失败：{err}"))?;
        Self::read_camera_face_alert(&conn, &frame.alert_id)
    }

    pub fn list_camera_face_alerts_for_responder(
        &self,
        limit: usize,
        responder_device_id: &str,
    ) -> Result<Vec<CameraFaceAlertRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn.prepare("SELECT a.alert_id,a.source_kind,a.source_device_id,a.source_nickname,a.source_address,a.person_id,a.person_name,a.confidence,a.recognition_level,a.face_confidence,a.body_confidence,a.consecutive_hits,a.policy_version,a.created_at,COALESCE(SUM(CASE WHEN f.result='real' THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN f.result='false' THEN 1 ELSE 0 END),0),MAX(CASE WHEN f.responder_device_id=?1 THEN f.result END) FROM camera_face_alerts a LEFT JOIN camera_face_alert_feedbacks f ON f.alert_id=a.alert_id GROUP BY a.alert_id ORDER BY a.created_at DESC LIMIT ?2")
            .map_err(|err| format!("读取自动识别告警失败：{err}"))?;
        let rows = stmt
            .query_map(
                params![responder_device_id, limit.clamp(1, 200) as i64],
                Self::camera_face_alert_from_row,
            )
            .map_err(|err| format!("读取自动识别告警失败：{err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("读取自动识别告警失败：{err}"))
    }

    pub fn clear_camera_face_alerts(&self) -> Result<(), String> {
        let mut conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let transaction = conn
            .transaction()
            .map_err(|err| format!("开始清空识别率排行榜失败：{err}"))?;
        transaction
            .execute("DELETE FROM camera_face_alert_feedbacks", [])
            .map_err(|err| format!("清空识别反馈失败：{err}"))?;
        transaction
            .execute("DELETE FROM camera_face_alerts", [])
            .map_err(|err| format!("清空识别告警失败：{err}"))?;
        transaction
            .commit()
            .map_err(|err| format!("提交识别率排行榜清理失败：{err}"))
    }

    pub fn upsert_camera_face_alert_feedback(
        &self,
        frame: &CameraFaceAlertFeedbackFrame,
    ) -> Result<CameraFaceAlertRecord, String> {
        if !matches!(frame.result.as_str(), "real" | "false") {
            return Err("自动识别告警反馈无效".to_string());
        }
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute("INSERT INTO camera_face_alert_feedbacks (alert_id,responder_device_id,responder_nickname,result,created_at) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(alert_id,responder_device_id) DO UPDATE SET responder_nickname=excluded.responder_nickname,result=excluded.result,created_at=excluded.created_at", params![frame.alert_id, frame.responder_device_id, frame.responder_nickname, frame.result, frame.created_at])
            .map_err(|err| format!("保存自动识别告警反馈失败：{err}"))?;
        Self::read_camera_face_alert(&conn, &frame.alert_id)
    }

    fn read_camera_face_alert(
        conn: &Connection,
        alert_id: &str,
    ) -> Result<CameraFaceAlertRecord, String> {
        conn.query_row("SELECT a.alert_id,a.source_kind,a.source_device_id,a.source_nickname,a.source_address,a.person_id,a.person_name,a.confidence,a.recognition_level,a.face_confidence,a.body_confidence,a.consecutive_hits,a.policy_version,a.created_at,COALESCE(SUM(CASE WHEN f.result='real' THEN 1 ELSE 0 END),0),COALESCE(SUM(CASE WHEN f.result='false' THEN 1 ELSE 0 END),0),CAST(NULL AS TEXT) FROM camera_face_alerts a LEFT JOIN camera_face_alert_feedbacks f ON f.alert_id=a.alert_id WHERE a.alert_id=?1 GROUP BY a.alert_id", params![alert_id], Self::camera_face_alert_from_row)
            .map_err(|err| format!("读取自动识别告警失败：{err}"))
    }

    fn camera_face_alert_from_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<CameraFaceAlertRecord> {
        Ok(CameraFaceAlertRecord {
            alert_id: row.get(0)?,
            source_kind: row.get(1)?,
            source_device_id: row.get(2)?,
            source_nickname: row.get(3)?,
            source_address: row.get(4)?,
            person_id: row.get(5)?,
            person_name: row.get(6)?,
            confidence: row.get(7)?,
            recognition_level: row.get(8)?,
            face_confidence: row.get(9)?,
            body_confidence: row.get(10)?,
            consecutive_hits: row.get(11)?,
            policy_version: row.get(12)?,
            created_at: row.get(13)?,
            feedback_real: row.get(14)?,
            feedback_false: row.get(15)?,
            local_feedback: row.get(16)?,
        })
    }

    pub fn get_or_create_profile(&self) -> Result<Profile, String> {
        if let Some(mut profile) = self.get_profile()? {
            let resolved_id = resolve_device_id();
            let desired_id = resolve_profile_device_id(&profile.device_id, &resolved_id);
            if desired_id != profile.device_id {
                self.migrate_profile_device_identity(&profile.device_id, &desired_id)?;
                profile.device_id = desired_id;
            }
            self.repair_legacy_private_channel_memberships(&profile, &local_ip_address())?;
            return Ok(profile);
        }

        let profile = Profile {
            device_id: resolve_device_id(),
            nickname: default_nickname(),
            listen_port: 18145,
            avatar: None,
            nickname_locked: false,
        };
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "INSERT INTO profile (id, device_id, nickname, listen_port, avatar, nickname_locked) VALUES (1, ?1, ?2, ?3, ?4, ?5)",
            params![profile.device_id, profile.nickname, profile.listen_port, profile.avatar, if profile.nickname_locked { 1 } else { 0 }],
        )
        .map_err(|err| format!("保存本机身份失败：{err}"))?;
        drop(conn);
        self.ensure_default_group(0)?;
        self.repair_legacy_private_channel_memberships(&profile, &local_ip_address())?;
        Ok(profile)
    }

    fn repair_legacy_private_channel_memberships(
        &self,
        profile: &Profile,
        local_address: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT cm.channel_id, cm.device_id, p.address
                 FROM channel_members cm
                 LEFT JOIN peers p ON replace(replace(lower(p.device_id), ':', ''), '-', '') = replace(replace(lower(cm.device_id), ':', ''), '-', '')
                 WHERE replace(replace(lower(cm.device_id), ':', ''), '-', '') <> replace(replace(lower(?1), ':', ''), '-', '')
                   AND NOT EXISTS (
                     SELECT 1 FROM channel_members current
                     WHERE current.channel_id = cm.channel_id
                       AND replace(replace(lower(current.device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')
                   )",
            )
            .map_err(|err| format!("读取历史频道成员失败：{err}"))?;
        let candidates = stmt
            .query_map(params![profile.device_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|err| format!("读取历史频道成员失败：{err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析历史频道成员失败：{err}"))?;
        drop(stmt);
        drop(conn);

        let mut mac_matches: HashMap<String, Vec<String>> = HashMap::new();
        let mut ip_matches: HashMap<String, Vec<String>> = HashMap::new();
        for (channel_id, device_id, address) in candidates {
            if normalize_device_id(&device_id) == normalize_device_id(&profile.device_id) {
                mac_matches.entry(channel_id).or_default().push(device_id);
            } else if !local_address.trim().is_empty()
                && local_address != "127.0.0.1"
                && address.as_deref() == Some(local_address)
            {
                ip_matches.entry(channel_id).or_default().push(device_id);
            }
        }
        let mut legacy_ids = HashSet::new();
        for (channel_id, ids) in mac_matches {
            if ids.len() == 1 {
                legacy_ids.insert(ids[0].clone());
                ip_matches.remove(&channel_id);
            }
        }
        for ids in ip_matches.values().filter(|ids| ids.len() == 1) {
            legacy_ids.insert(ids[0].clone());
        }
        for legacy_id in legacy_ids {
            self.migrate_profile_device_identity(&legacy_id, &profile.device_id)?;
        }
        Ok(())
    }

    pub fn migrate_profile_device_identity(
        &self,
        old_device_id: &str,
        new_device_id: &str,
    ) -> Result<(), String> {
        let old_device_id = old_device_id.trim().to_ascii_lowercase();
        let new_device_id = normalize_device_id(new_device_id);
        if old_device_id.is_empty() || new_device_id.is_empty() || old_device_id == new_device_id {
            return Ok(());
        }
        let mut conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let tx = conn
            .transaction()
            .map_err(|err| format!("创建身份迁移事务失败：{err}"))?;
        tx.execute(
            "DELETE FROM channel_members
             WHERE replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')
               AND EXISTS (
                 SELECT 1 FROM channel_members current
                 WHERE current.channel_id = channel_members.channel_id
                   AND replace(replace(lower(current.device_id), ':', ''), '-', '') = replace(replace(lower(?2), ':', ''), '-', '')
               )",
            params![&old_device_id, &new_device_id],
        )
        .map_err(|err| format!("合并频道成员身份失败：{err}"))?;
        tx.execute(
            "DELETE FROM channel_mutes
             WHERE replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')
               AND EXISTS (
                 SELECT 1 FROM channel_mutes current
                 WHERE current.channel_id = channel_mutes.channel_id
                   AND replace(replace(lower(current.device_id), ':', ''), '-', '') = replace(replace(lower(?2), ':', ''), '-', '')
               )",
            params![&old_device_id, &new_device_id],
        )
        .map_err(|err| format!("合并频道禁言身份失败：{err}"))?;
        for statement in [
            "UPDATE channel_members SET device_id = ?2 WHERE replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')",
            "UPDATE private_channels SET owner_device_id = ?2 WHERE replace(replace(lower(owner_device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')",
            "UPDATE channel_mutes SET device_id = ?2 WHERE replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')",
            "UPDATE messages SET sender_device_id = ?2 WHERE replace(replace(lower(sender_device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')",
            "UPDATE conversations SET peer_device_id = ?2 WHERE replace(replace(lower(peer_device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')",
            "UPDATE profile SET device_id = ?2 WHERE id = 1 AND replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?1), ':', ''), '-', '')",
        ] {
            tx.execute(statement, params![&old_device_id, &new_device_id])
                .map_err(|err| format!("迁移本机设备标识失败：{err}"))?;
        }
        tx.commit()
            .map_err(|err| format!("提交本机设备标识迁移失败：{err}"))?;
        Ok(())
    }

    pub fn get_profile(&self) -> Result<Option<Profile>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.query_row(
            "SELECT device_id, nickname, listen_port, avatar, nickname_locked FROM profile WHERE id = 1",
            [],
            |row| {
                Ok(Profile {
                    device_id: row.get(0)?,
                    nickname: row.get(1)?,
                    listen_port: row.get::<_, i64>(2)? as u16,
                    avatar: row.get(3)?,
                    nickname_locked: row.get::<_, i64>(4)? == 1,
                })
            },
        )
        .optional()
        .map_err(|err| format!("读取本机身份失败：{err}"))
    }

    pub fn update_profile(
        &self,
        nickname: &str,
        listen_port: u16,
        avatar: Option<String>,
    ) -> Result<Profile, String> {
        let mut profile = self.get_or_create_profile()?;
        let next_nickname = nickname.trim().to_string();
        if profile.nickname_locked && next_nickname != profile.nickname {
            return Err("管理员已禁止本机修改昵称".to_string());
        }
        profile.nickname = next_nickname;
        profile.listen_port = listen_port;
        profile.avatar = avatar.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "UPDATE profile SET nickname = ?1, listen_port = ?2, avatar = ?3 WHERE id = 1",
            params![profile.nickname, profile.listen_port, profile.avatar],
        )
        .map_err(|err| format!("更新本机资料失败：{err}"))?;
        Ok(profile)
    }

    pub fn apply_admin_nickname(
        &self,
        nickname: &str,
        nickname_locked: Option<bool>,
    ) -> Result<Profile, String> {
        let mut profile = self.get_or_create_profile()?;
        profile.nickname = nickname.trim().to_string();
        if let Some(locked) = nickname_locked {
            profile.nickname_locked = locked;
        }
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "UPDATE profile SET nickname = ?1, nickname_locked = ?2 WHERE id = 1",
            params![
                profile.nickname,
                if profile.nickname_locked { 1 } else { 0 }
            ],
        )
        .map_err(|err| format!("应用管理员昵称失败：{err}"))?;
        Ok(profile)
    }

    pub fn upsert_peer(&self, peer: &Peer) -> Result<(), String> {
        let normalized = Peer {
            device_id: normalize_device_id(&peer.device_id),
            nickname: peer.nickname.trim().to_string(),
            note: peer.note.clone(),
            avatar: peer.avatar.clone(),
            address: peer.address.trim().to_string(),
            port: peer.port,
            online: peer.online,
            last_seen_at: peer.last_seen_at,
            client_kind: normalize_client_kind(&peer.client_kind),
            supports_chat: peer.supports_chat,
            nickname_locked: peer.nickname_locked,
            build_version: peer.build_version.trim().to_string(),
            build_timestamp: peer.build_timestamp,
        };
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let duplicate_ids = find_duplicate_peer_ids(&conn, &normalized)?;
        for duplicate_id in duplicate_ids {
            conn.execute(
                "DELETE FROM peers WHERE device_id = ?1",
                params![&duplicate_id],
            )
            .map_err(|err| format!("清理重复设备失败：{err}"))?;
            conn.execute(
                "DELETE FROM conversations WHERE kind = 'direct' AND id = ?1",
                params![duplicate_id],
            )
            .map_err(|err| format!("清理重复会话失败：{err}"))?;
        }
        conn.execute(
            "
            INSERT INTO peers (device_id, nickname, avatar, address, port, online, last_seen_at, client_kind, supports_chat, nickname_locked, build_version, build_timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(device_id) DO UPDATE SET
                nickname = excluded.nickname,
                avatar = COALESCE(excluded.avatar, peers.avatar),
                address = excluded.address,
                port = excluded.port,
                online = excluded.online,
                last_seen_at = excluded.last_seen_at,
                client_kind = excluded.client_kind,
                supports_chat = excluded.supports_chat,
                nickname_locked = excluded.nickname_locked,
                build_version = excluded.build_version,
                build_timestamp = excluded.build_timestamp
            ",
            params![
                normalized.device_id,
                normalized.nickname,
                normalized.avatar,
                normalized.address,
                normalized.port,
                if normalized.online { 1 } else { 0 },
                normalized.last_seen_at,
                normalized.client_kind,
                if normalized.supports_chat { 1 } else { 0 },
                if normalized.nickname_locked { 1 } else { 0 },
                normalized.build_version,
                normalized.build_timestamp
            ],
        )
        .map_err(|err| format!("保存局域网设备失败：{err}"))?;
        drop(conn);
        if normalized.supports_chat {
            self.ensure_direct_conversation(&normalized)
        } else {
            Ok(())
        }
    }

    pub fn update_peer_avatar(
        &self,
        device_id: &str,
        avatar: Option<String>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "UPDATE peers SET avatar = ?1 WHERE device_id = ?2",
            params![avatar, normalize_device_id(device_id)],
        )
        .map_err(|err| format!("更新设备头像失败：{err}"))?;
        Ok(())
    }

    pub fn get_peer(&self, device_id: &str) -> Result<Option<Peer>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.query_row(
            "SELECT p.device_id, p.nickname, dn.note, p.avatar, p.address, p.port, p.online, p.last_seen_at, p.client_kind, p.supports_chat, p.nickname_locked, p.build_version, p.build_timestamp
             FROM peers p LEFT JOIN device_notes dn ON lower(dn.device_id) = lower(p.device_id)
             WHERE p.device_id = ?1",
            params![normalize_device_id(device_id)],
            |row| {
                Ok(Peer {
                    device_id: row.get(0)?,
                    nickname: row.get(1)?,
                    note: row.get(2)?,
                    avatar: row.get(3)?,
                    address: row.get(4)?,
                    port: row.get::<_, i64>(5)? as u16,
                    online: row.get::<_, i64>(6)? == 1,
                    last_seen_at: row.get(7)?,
                    client_kind: row.get(8)?,
                    supports_chat: row.get::<_, i64>(9)? == 1,
                    nickname_locked: row.get::<_, i64>(10)? == 1,
                    build_version: row.get(11)?,
                    build_timestamp: row.get(12)?,
                })
            },
        )
        .optional()
        .map_err(|err| format!("读取局域网设备失败：{err}"))
    }

    pub fn list_peers(&self) -> Result<Vec<Peer>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT p.device_id, p.nickname, dn.note, p.avatar, p.address, p.port, p.online, p.last_seen_at, p.client_kind, p.supports_chat, p.nickname_locked, p.build_version, p.build_timestamp
                 FROM peers p LEFT JOIN device_notes dn ON lower(dn.device_id) = lower(p.device_id)
                 ORDER BY p.online DESC,
                          CASE WHEN trim(COALESCE(dn.note, '')) <> '' THEN 0 ELSE 1 END,
                          lower(COALESCE(NULLIF(trim(dn.note), ''), p.nickname)), lower(p.device_id)",
            )
            .map_err(|err| format!("读取局域网设备失败：{err}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Peer {
                    device_id: row.get(0)?,
                    nickname: row.get(1)?,
                    note: row.get(2)?,
                    avatar: row.get(3)?,
                    address: row.get(4)?,
                    port: row.get::<_, i64>(5)? as u16,
                    online: row.get::<_, i64>(6)? == 1,
                    last_seen_at: row.get(7)?,
                    client_kind: row.get(8)?,
                    supports_chat: row.get::<_, i64>(9)? == 1,
                    nickname_locked: row.get::<_, i64>(10)? == 1,
                    build_version: row.get(11)?,
                    build_timestamp: row.get(12)?,
                })
            })
            .map_err(|err| format!("读取局域网设备失败：{err}"))?;
        let peers = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析局域网设备失败：{err}"))?;
        Ok(dedupe_peer_list(peers))
    }

    pub fn delete_peer(&self, device_id: &str) -> Result<(), String> {
        let normalized_id = normalize_device_id(device_id);
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "DELETE FROM peers WHERE device_id = ?1",
            params![&normalized_id],
        )
        .map_err(|err| format!("删除设备失败：{err}"))?;
        conn.execute(
            "DELETE FROM conversations WHERE kind = 'direct' AND id = ?1",
            params![normalized_id],
        )
        .map_err(|err| format!("删除设备会话失败：{err}"))?;
        conn.execute(
            "DELETE FROM device_notes WHERE lower(device_id) = lower(?1)",
            params![normalized_id],
        )
        .map_err(|err| format!("删除设备备注失败：{err}"))?;
        Ok(())
    }

    pub fn update_peer_note(&self, device_id: &str, note: &str) -> Result<(), String> {
        let device_id = normalize_device_id(device_id);
        let note = note.trim();
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let exists = conn
            .query_row(
                "SELECT COUNT(1) FROM peers WHERE lower(device_id) = lower(?1)",
                params![device_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|err| format!("检查设备备注失败：{err}"))?
            > 0;
        if !exists {
            return Err("未找到该设备，无法保存备注".to_string());
        }
        if note.is_empty() {
            conn.execute(
                "DELETE FROM device_notes WHERE lower(device_id) = lower(?1)",
                params![device_id],
            )
            .map_err(|err| format!("清除设备备注失败：{err}"))?;
        } else {
            conn.execute(
                "INSERT INTO device_notes (device_id, note, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(device_id) DO UPDATE SET note = excluded.note, updated_at = excluded.updated_at",
                params![device_id, note, chrono::Utc::now().timestamp_millis()],
            )
            .map_err(|err| format!("保存设备备注失败：{err}"))?;
        }
        Ok(())
    }

    pub fn mark_stale_peers_offline(&self, older_than: i64) -> Result<Vec<String>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn
            .prepare("SELECT device_id FROM peers WHERE online = 1 AND last_seen_at < ?1")
            .map_err(|err| format!("读取超时设备失败：{err}"))?;
        let ids = stmt
            .query_map(params![older_than], |row| row.get::<_, String>(0))
            .map_err(|err| format!("读取超时设备失败：{err}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析超时设备失败：{err}"))?;
        drop(stmt);
        for id in &ids {
            conn.execute(
                "UPDATE peers SET online = 0 WHERE device_id = ?1",
                params![id],
            )
            .map_err(|err| format!("更新设备离线状态失败：{err}"))?;
        }
        Ok(ids)
    }
    pub fn list_conversations(&self) -> Result<Vec<Conversation>, String> {
        self.ensure_default_group(chrono::Utc::now().timestamp_millis())?;
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT c.id, c.title, c.kind, c.peer_device_id, c.updated_at, c.unread_count,
                        pc.owner_device_id, CASE WHEN pc.id IS NULL THEN 0 ELSE 1 END
                 FROM conversations c
                 LEFT JOIN private_channels pc ON pc.id = c.id
                 ORDER BY c.updated_at DESC, c.title ASC",
            )
            .map_err(|err| format!("读取会话失败：{err}"))?;
        let rows = stmt
            .query_map([], |row| {
                let kind: String = row.get(2)?;
                let is_private = row.get::<_, i64>(7)? == 1;
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    kind: if kind == "group" {
                        ConversationKind::Group
                    } else {
                        ConversationKind::Direct
                    },
                    peer_device_id: row.get(3)?,
                    updated_at: row.get(4)?,
                    unread_count: row.get::<_, i64>(5)? as u32,
                    is_private,
                    owner_device_id: row.get(6)?,
                })
            })
            .map_err(|err| format!("读取会话失败：{err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析会话失败：{err}"))
    }

    pub fn save_message(&self, message: &Message) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "
            INSERT INTO messages (id, conversation_id, sender_device_id, content, message_type, file_name, file_size, file_url, file_mime_type, file_duration_ms, status, simulation_operator_device_id, simulation_operator_nickname, simulation_display_label, simulation_created_at, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT(id) DO UPDATE SET status = excluded.status
            ",
            params![
                message.id,
                message.conversation_id,
                message.sender_device_id,
                message.content,
                message.message_type.as_str(),
                message.file_meta.as_ref().map(|meta| meta.name.as_str()),
                message.file_meta.as_ref().map(|meta| meta.size as i64),
                message.file_meta.as_ref().map(|meta| meta.url.as_str()),
                message.file_meta.as_ref().and_then(|meta| meta.mime_type.as_deref()),
                message.file_meta.as_ref().and_then(|meta| meta.duration_ms.map(|value| value as i64)),
                message.status.as_str(),
                message.simulation.as_ref().map(|meta| meta.operator_device_id.as_str()),
                message.simulation.as_ref().map(|meta| meta.operator_nickname.as_str()),
                message.simulation.as_ref().map(|meta| i64::from(meta.display_label)),
                message.simulation.as_ref().map(|meta| meta.created_at),
                message.created_at
            ],
        )
        .map_err(|err| format!("保存消息失败：{err}"))?;
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![message.created_at, message.conversation_id],
        )
        .ok();
        Ok(())
    }

    pub fn save_simulation_audit(&self, audit: &SimulationAudit) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "INSERT INTO simulation_audits (id, operator_device_id, operator_nickname, simulated_device_id, action_kind, target_id, display_label, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![audit.id, audit.operator_device_id, audit.operator_nickname, audit.simulated_device_id, audit.action_kind, audit.target_id, i64::from(audit.display_label), audit.content, audit.created_at],
        )
        .map_err(|err| format!("保存模拟操作审计失败：{err}"))?;
        Ok(())
    }

    pub fn upsert_admin_notification(
        &self,
        frame: &AdminNotificationFrame,
    ) -> Result<AdminNotificationRecord, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "INSERT INTO admin_notifications (notification_id, target_device_id, title, content, template, support_url, display_mode, deadline_at, timeout_policy, force_open_main_window, issued_by_device_id, issued_by_nickname, created_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'pending')
             ON CONFLICT(notification_id) DO UPDATE SET title=excluded.title, content=excluded.content, support_url=excluded.support_url, deadline_at=excluded.deadline_at, timeout_policy=excluded.timeout_policy, force_open_main_window=excluded.force_open_main_window",
            params![frame.notification_id, frame.target_device_id, frame.title, frame.content, frame.template, frame.support_url, frame.display_mode, frame.deadline_at, frame.timeout_policy, if frame.force_open_main_window { 1 } else { 0 }, frame.issued_by_device_id, frame.issued_by_nickname, frame.created_at],
        ).map_err(|err| format!("保存超管通知失败：{err}"))?;
        Self::read_admin_notification(&conn, &frame.notification_id)
    }

    pub fn list_admin_notifications(&self) -> Result<Vec<AdminNotificationRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        self.resolve_admin_notification_timeouts_locked(&conn)?;
        let mut stmt = conn.prepare("SELECT notification_id,target_device_id,title,content,template,support_url,display_mode,deadline_at,timeout_policy,force_open_main_window,issued_by_device_id,issued_by_nickname,created_at,status,submitted_at,decided_at,decision_by_device_id,decision_by_nickname FROM admin_notifications ORDER BY created_at DESC")
            .map_err(|err| format!("读取超管通知失败：{err}"))?;
        let rows = stmt
            .query_map([], Self::admin_notification_from_row)
            .map_err(|err| format!("读取超管通知失败：{err}"))?;
        let records = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析超管通知失败：{err}"))?;
        Ok(records)
    }

    pub fn submit_admin_notification(
        &self,
        notification_id: &str,
        device_id: &str,
        nickname: &str,
        submitted_at: i64,
    ) -> Result<AdminNotificationRecord, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        self.resolve_admin_notification_timeouts_locked(&conn)?;
        let record = Self::read_admin_notification(&conn, notification_id)?;
        if normalize_device_id(&record.target_device_id) != normalize_device_id(device_id) {
            return Err("该通知不属于本机".to_string());
        }
        if !matches!(
            record.status.as_str(),
            "pending" | "rejected" | "expired_locked"
        ) {
            return Err("该通知当前不能提交确认".to_string());
        }
        conn.execute("UPDATE admin_notifications SET status='submitted', submitted_at=?1 WHERE notification_id=?2", params![submitted_at, notification_id])
            .map_err(|err| format!("提交通知确认失败：{err}"))?;
        let _ = nickname;
        Self::read_admin_notification(&conn, notification_id)
    }

    pub fn decide_admin_notification(
        &self,
        frame: &AdminNotificationDecisionFrame,
    ) -> Result<AdminNotificationRecord, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let status = match frame.decision.as_str() {
            "approved" => "approved",
            "rejected" => "rejected",
            "revoked" => "revoked",
            _ => return Err("无效的通知审核结果".to_string()),
        };
        conn.execute("UPDATE admin_notifications SET status=?1, decided_at=?2, decision_by_device_id=?3, decision_by_nickname=?4 WHERE notification_id=?5", params![status, frame.decided_at, frame.decided_by_device_id, frame.decided_by_nickname, frame.notification_id])
            .map_err(|err| format!("更新通知审核结果失败：{err}"))?;
        Self::read_admin_notification(&conn, &frame.notification_id)
    }

    fn resolve_admin_notification_timeouts_locked(&self, conn: &Connection) -> Result<(), String> {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute("UPDATE admin_notifications SET status=CASE WHEN timeout_policy='auto_release' THEN 'expired_released' ELSE 'expired_locked' END WHERE deadline_at IS NOT NULL AND deadline_at <= ?1 AND status IN ('pending','submitted','rejected')", params![now])
            .map_err(|err| format!("处理通知超时失败：{err}"))?;
        Ok(())
    }

    fn read_admin_notification(
        conn: &Connection,
        notification_id: &str,
    ) -> Result<AdminNotificationRecord, String> {
        conn.query_row("SELECT notification_id,target_device_id,title,content,template,support_url,display_mode,deadline_at,timeout_policy,force_open_main_window,issued_by_device_id,issued_by_nickname,created_at,status,submitted_at,decided_at,decision_by_device_id,decision_by_nickname FROM admin_notifications WHERE notification_id=?1", params![notification_id], Self::admin_notification_from_row)
            .map_err(|err| format!("读取超管通知失败：{err}"))
    }

    fn admin_notification_from_row(
        row: &rusqlite::Row<'_>,
    ) -> rusqlite::Result<AdminNotificationRecord> {
        Ok(AdminNotificationRecord {
            notification_id: row.get(0)?,
            target_device_id: row.get(1)?,
            title: row.get(2)?,
            content: row.get(3)?,
            template: row.get(4)?,
            support_url: row.get(5)?,
            display_mode: row.get(6)?,
            deadline_at: row.get(7)?,
            timeout_policy: row.get(8)?,
            force_open_main_window: row.get::<_, i64>(9)? != 0,
            issued_by_device_id: row.get(10)?,
            issued_by_nickname: row.get(11)?,
            created_at: row.get(12)?,
            status: row.get(13)?,
            submitted_at: row.get(14)?,
            decided_at: row.get(15)?,
            decision_by_device_id: row.get(16)?,
            decision_by_nickname: row.get(17)?,
        })
    }

    pub fn update_message_status(
        &self,
        message_id: &str,
        status: MessageStatus,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "UPDATE messages SET status = ?1 WHERE id = ?2",
            params![status.as_str(), message_id],
        )
        .map_err(|err| format!("更新消息状态失败：{err}"))?;
        Ok(())
    }

    pub fn update_message_after_recall(&self, message_id: &str) -> Result<Option<Message>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "UPDATE messages SET content = '消息已撤回', message_type = 'system', file_name = NULL, file_size = NULL, file_url = NULL, file_mime_type = NULL, file_duration_ms = NULL, status = 'delivered' WHERE id = ?1",
            params![message_id],
        )
        .map_err(|err| format!("撤回消息失败：{err}"))?;
        conn.query_row(
            "SELECT id, conversation_id, sender_device_id, content, message_type, file_name, file_size, file_url, file_mime_type, file_duration_ms, status, simulation_operator_device_id, simulation_operator_nickname, simulation_display_label, simulation_created_at, created_at
             FROM messages WHERE id = ?1",
            params![message_id],
            |row| {
                let message_type: String = row.get(4)?;
                let status: String = row.get(10)?;
                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    sender_device_id: row.get(2)?,
                    content: row.get(3)?,
                    message_type: MessageType::from_str(&message_type),
                    file_meta: None,
                    status: MessageStatus::from_str(&status),
                    simulation: simulation_meta_from_row(row, 11)?,
                    created_at: row.get(15)?,
                })
            },
        )
        .optional()
        .map_err(|err| format!("读取撤回消息失败：{err}"))
    }

    pub fn list_messages_page(
        &self,
        conversation_id: &str,
        before_created_at: Option<i64>,
        limit: i64,
    ) -> Result<Vec<Message>, String> {
        let limit = limit.clamp(1, 100);
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, conversation_id, sender_device_id, content, message_type, file_name, file_size, file_url, file_mime_type, file_duration_ms, status, simulation_operator_device_id, simulation_operator_nickname, simulation_display_label, simulation_created_at, created_at
                 FROM (SELECT id, conversation_id, sender_device_id, content, message_type, file_name, file_size, file_url, file_mime_type, file_duration_ms, status, simulation_operator_device_id, simulation_operator_nickname, simulation_display_label, simulation_created_at, created_at
                       FROM messages WHERE conversation_id = ?1 AND (?2 IS NULL OR created_at < ?2) ORDER BY created_at DESC LIMIT ?3)
                 ORDER BY created_at ASC",
            )
            .map_err(|err| format!("读取消息失败：{err}"))?;
        let rows = stmt
            .query_map(params![conversation_id, before_created_at, limit], |row| {
                let message_type: String = row.get(4)?;
                let status: String = row.get(10)?;
                let file_name: Option<String> = row.get(5)?;
                let file_size: Option<i64> = row.get(6)?;
                let file_url: Option<String> = row.get(7)?;
                let file_mime_type: Option<String> = row.get(8)?;
                let file_duration_ms: Option<i64> = row.get(9)?;
                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    sender_device_id: row.get(2)?,
                    content: row.get(3)?,
                    message_type: MessageType::from_str(&message_type),
                    file_meta: match (file_name, file_size, file_url) {
                        (Some(name), Some(size), Some(url)) => Some(FileMeta {
                            name,
                            size: size as u64,
                            url,
                            mime_type: file_mime_type,
                            duration_ms: file_duration_ms.map(|value| value as u64),
                        }),
                        _ => None,
                    },
                    status: MessageStatus::from_str(&status),
                    simulation: simulation_meta_from_row(row, 11)?,
                    created_at: row.get(15)?,
                })
            })
            .map_err(|err| format!("读取消息失败：{err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析消息失败：{err}"))
    }

    #[cfg(test)]
    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<Message>, String> {
        self.list_messages_page(conversation_id, None, 500)
    }

    pub fn get_conversation(&self, conversation_id: &str) -> Result<Option<Conversation>, String> {
        Ok(self
            .list_conversations()?
            .into_iter()
            .find(|conversation| conversation.id == conversation_id))
    }

    pub fn upsert_private_channel(
        &self,
        channel_id: &str,
        title: &str,
        owner_device_id: &str,
        owner_nickname: &str,
        channel_key: &str,
        key_version: u32,
        members: &[ChannelMemberSeed],
        updated_at: i64,
    ) -> Result<Conversation, String> {
        let channel_id = channel_id.trim();
        let title = title.trim();
        let owner_device_id = normalize_device_id(owner_device_id);
        let owner_nickname = owner_nickname.trim();
        if channel_id.is_empty() {
            return Err("频道 ID 不能为空".to_string());
        }
        if title.is_empty() {
            return Err("频道名称不能为空".to_string());
        }
        if owner_device_id.is_empty() {
            return Err("频道群主不能为空".to_string());
        }
        if channel_key.trim().is_empty() {
            return Err("频道密钥不能为空".to_string());
        }

        let mut unique_members: HashMap<String, ChannelMemberSeed> = HashMap::new();
        unique_members.insert(
            owner_device_id.clone(),
            ChannelMemberSeed {
                device_id: owner_device_id.clone(),
                nickname: owner_nickname.to_string(),
                avatar: None,
            },
        );
        for member in members {
            let device_id = normalize_device_id(&member.device_id);
            if device_id.is_empty() {
                continue;
            }
            unique_members.insert(
                device_id.clone(),
                ChannelMemberSeed {
                    device_id,
                    nickname: member.nickname.trim().to_string(),
                    avatar: member.avatar.clone(),
                },
            );
        }

        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "
            INSERT INTO private_channels (id, title, owner_device_id, owner_nickname, channel_key, key_version, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                owner_device_id = excluded.owner_device_id,
                owner_nickname = excluded.owner_nickname,
                channel_key = excluded.channel_key,
                key_version = excluded.key_version,
                updated_at = excluded.updated_at
            ",
            params![
                channel_id,
                title,
                owner_device_id,
                owner_nickname,
                channel_key.trim(),
                key_version as i64,
                updated_at
            ],
        )
        .map_err(|err| format!("保存私有频道失败：{err}"))?;
        conn.execute(
            "
            INSERT INTO conversations (id, title, kind, peer_device_id, updated_at, unread_count)
            VALUES (?1, ?2, 'group', NULL, ?3, 0)
            ON CONFLICT(id) DO UPDATE SET title = excluded.title, updated_at = excluded.updated_at
            ",
            params![channel_id, title, updated_at],
        )
        .map_err(|err| format!("保存私有频道会话失败：{err}"))?;
        for member in unique_members.values() {
            let nickname = if member.nickname.trim().is_empty() {
                "局域网成员"
            } else {
                member.nickname.trim()
            };
            conn.execute(
                "
                INSERT INTO channel_members (channel_id, device_id, nickname, avatar, invited_at)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(channel_id, device_id) DO UPDATE SET
                    nickname = excluded.nickname,
                    avatar = excluded.avatar,
                    invited_at = excluded.invited_at
                ",
                params![
                    channel_id,
                    member.device_id,
                    nickname,
                    member.avatar,
                    updated_at
                ],
            )
            .map_err(|err| format!("保存私有频道成员失败：{err}"))?;
        }
        drop(conn);
        self.get_conversation(channel_id)?
            .ok_or_else(|| "私有频道创建后未找到会话".to_string())
    }

    pub fn get_private_channel(
        &self,
        channel_id: &str,
    ) -> Result<Option<PrivateChannelRecord>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.query_row(
            "SELECT id, title, owner_device_id, owner_nickname, channel_key, key_version, updated_at FROM private_channels WHERE id = ?1",
            params![channel_id],
            |row| {
                Ok(PrivateChannelRecord {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    owner_device_id: row.get(2)?,
                    owner_nickname: row.get(3)?,
                    channel_key: row.get(4)?,
                    key_version: row.get::<_, i64>(5)? as u32,
                    updated_at: row.get(6)?,
                })
            },
        )
        .optional()
        .map_err(|err| format!("读取私有频道失败：{err}"))
    }

    pub fn private_channel_key(&self, channel_id: &str) -> Result<Option<(String, u32)>, String> {
        Ok(self
            .get_private_channel(channel_id)?
            .map(|channel| (channel.channel_key, channel.key_version)))
    }

    pub fn list_channel_members(&self, channel_id: &str) -> Result<Vec<ChannelMember>, String> {
        let channel = self.get_private_channel(channel_id)?;
        let owner_device_id = channel
            .as_ref()
            .map(|item| normalize_device_id(&item.owner_device_id))
            .unwrap_or_default();
        let local_device_id = self
            .get_profile()?
            .map(|profile| normalize_device_id(&profile.device_id))
            .unwrap_or_default();
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT cm.channel_id, cm.device_id, cm.nickname, cm.avatar, cm.muted,
                        CASE WHEN replace(replace(lower(cm.device_id), ':', ''), '-', '') = replace(replace(lower(?3), ':', ''), '-', '') THEN 1 ELSE COALESCE(p.online, 0) END,
                        CASE WHEN replace(replace(lower(cm.device_id), ':', ''), '-', '') = replace(replace(lower(?3), ':', ''), '-', '') THEN ?4 ELSE COALESCE(p.last_seen_at, cm.invited_at) END
                 FROM channel_members cm
                 LEFT JOIN peers p ON replace(replace(lower(p.device_id), ':', ''), '-', '') = replace(replace(lower(cm.device_id), ':', ''), '-', '')
                 WHERE cm.channel_id = ?1
                 ORDER BY CASE WHEN replace(replace(lower(cm.device_id), ':', ''), '-', '') = replace(replace(lower(?2), ':', ''), '-', '') THEN 0 ELSE 1 END, cm.nickname ASC",
            )
            .map_err(|err| format!("读取频道成员失败：{err}"))?;
        let rows = stmt
            .query_map(
                params![channel_id, owner_device_id, local_device_id, now],
                |row| {
                    let device_id: String = row.get(1)?;
                    Ok(ChannelMember {
                        channel_id: row.get(0)?,
                        device_id: device_id.clone(),
                        nickname: row.get(2)?,
                        avatar: row.get(3)?,
                        muted: row.get::<_, i64>(4)? == 1,
                        online: row.get::<_, i64>(5)? == 1,
                        last_seen_at: row.get(6)?,
                        is_owner: normalize_device_id(&device_id) == owner_device_id,
                    })
                },
            )
            .map_err(|err| format!("读取频道成员失败：{err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析频道成员失败：{err}"))
    }

    pub fn remove_private_channel_member(
        &self,
        channel_id: &str,
        member_device_id: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "DELETE FROM channel_members WHERE channel_id = ?1 AND replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?2), ':', ''), '-', '')",
            params![channel_id, normalize_device_id(member_device_id)],
        )
        .map_err(|err| format!("移除频道成员失败：{err}"))?;
        Ok(())
    }

    pub fn add_private_channel_member(
        &self,
        channel_id: &str,
        member: &ChannelMemberSeed,
        updated_at: i64,
    ) -> Result<(), String> {
        let channel_id = channel_id.trim();
        let device_id = normalize_device_id(&member.device_id);
        if channel_id.is_empty() {
            return Err("频道 ID 不能为空".to_string());
        }
        if device_id.is_empty() {
            return Err("频道成员不能为空".to_string());
        }
        let nickname = if member.nickname.trim().is_empty() {
            "局域网成员"
        } else {
            member.nickname.trim()
        };
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM private_channels WHERE id = ?1",
                params![channel_id],
                |row| row.get(0),
            )
            .map_err(|err| format!("检查私有频道失败：{err}"))?;
        if exists == 0 {
            return Err("私有频道不存在".to_string());
        }
        conn.execute(
            "
            INSERT INTO channel_members (channel_id, device_id, nickname, avatar, invited_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(channel_id, device_id) DO UPDATE SET
                nickname = excluded.nickname,
                avatar = excluded.avatar,
                invited_at = excluded.invited_at
            ",
            params![
                channel_id,
                device_id,
                nickname,
                member.avatar.clone(),
                updated_at
            ],
        )
        .map_err(|err| format!("保存私有频道成员失败：{err}"))?;
        conn.execute(
            "UPDATE private_channels SET updated_at = ?1 WHERE id = ?2",
            params![updated_at, channel_id],
        )
        .ok();
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![updated_at, channel_id],
        )
        .ok();
        Ok(())
    }
    pub fn set_private_channel_member_muted(
        &self,
        channel_id: &str,
        member_device_id: &str,
        muted: bool,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "UPDATE channel_members SET muted = ?1 WHERE channel_id = ?2 AND replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?3), ':', ''), '-', '')",
            params![if muted { 1 } else { 0 }, channel_id, normalize_device_id(member_device_id)],
        )
        .map_err(|err| format!("更新频道成员禁言失败：{err}"))?;
        Ok(())
    }

    pub fn is_private_channel_member_muted(
        &self,
        channel_id: &str,
        member_device_id: &str,
    ) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let muted = conn
            .query_row(
                "SELECT muted FROM channel_members WHERE channel_id = ?1 AND replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?2), ':', ''), '-', '')",
                params![channel_id, normalize_device_id(member_device_id)],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|err| format!("检查频道成员禁言失败：{err}"))?
            .unwrap_or(0);
        Ok(muted == 1)
    }

    pub fn set_channel_mute(
        &self,
        channel_id: &str,
        device_id: &str,
        muted: bool,
        updated_at: i64,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "INSERT INTO channel_mutes (channel_id, device_id, muted, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(channel_id, device_id) DO UPDATE SET muted = excluded.muted, updated_at = excluded.updated_at",
            params![channel_id, normalize_device_id(device_id), if muted { 1 } else { 0 }, updated_at],
        )
        .map_err(|err| format!("保存频道禁言失败：{err}"))?;
        Ok(())
    }

    pub fn is_channel_muted(&self, channel_id: &str, device_id: &str) -> Result<bool, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let muted = conn
            .query_row(
                "SELECT muted FROM channel_mutes WHERE channel_id = ?1 AND lower(device_id) = lower(?2)",
                params![channel_id, normalize_device_id(device_id)],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|err| format!("检查频道禁言失败：{err}"))?
            .unwrap_or(0);
        Ok(muted == 1)
    }

    pub fn delete_private_channel(&self, channel_id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "DELETE FROM channel_members WHERE channel_id = ?1",
            params![channel_id],
        )
        .map_err(|err| format!("删除频道成员失败：{err}"))?;
        conn.execute(
            "DELETE FROM private_channels WHERE id = ?1",
            params![channel_id],
        )
        .map_err(|err| format!("删除私有频道失败：{err}"))?;
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            params![channel_id],
        )
        .ok();
        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            params![channel_id],
        )
        .map_err(|err| format!("删除频道会话失败：{err}"))?;
        Ok(())
    }

    pub fn is_private_channel_member(
        &self,
        channel_id: &str,
        device_id: &str,
    ) -> Result<bool, String> {
        let normalized = normalize_device_id(device_id);
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(1) FROM channel_members WHERE channel_id = ?1 AND replace(replace(lower(device_id), ':', ''), '-', '') = replace(replace(lower(?2), ':', ''), '-', '')",
                params![channel_id, normalized],
                |row| row.get(0),
            )
            .map_err(|err| format!("检查频道成员失败：{err}"))?;
        Ok(count > 0)
    }
    fn ensure_default_group(&self, updated_at: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "
            INSERT INTO conversations (id, title, kind, peer_device_id, updated_at, unread_count)
            VALUES (?1, '局域网频道', 'group', NULL, ?2, 0)
            ON CONFLICT(id) DO NOTHING
            ",
            params![DEFAULT_GROUP_ID, updated_at],
        )
        .map_err(|err| format!("初始化默认群聊失败：{err}"))?;
        Ok(())
    }

    fn ensure_direct_conversation(&self, peer: &Peer) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        conn.execute(
            "
            INSERT INTO conversations (id, title, kind, peer_device_id, updated_at, unread_count)
            VALUES (?1, ?2, 'direct', ?1, ?3, 0)
            ON CONFLICT(id) DO UPDATE SET title = excluded.title
            ",
            params![peer.device_id, peer.nickname, peer.last_seen_at],
        )
        .map_err(|err| format!("保存会话失败：{err}"))?;
        Ok(())
    }
}

fn simulation_meta_from_row(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<Option<SimulationMeta>> {
    let operator_device_id: Option<String> = row.get(offset)?;
    let operator_nickname: Option<String> = row.get(offset + 1)?;
    let display_label: Option<i64> = row.get(offset + 2)?;
    let created_at: Option<i64> = row.get(offset + 3)?;
    match (
        operator_device_id,
        operator_nickname,
        display_label,
        created_at,
    ) {
        (
            Some(operator_device_id),
            Some(operator_nickname),
            Some(display_label),
            Some(created_at),
        ) => Ok(Some(SimulationMeta {
            operator_device_id,
            operator_nickname,
            display_label: display_label != 0,
            created_at,
        })),
        _ => Ok(None),
    }
}

fn find_duplicate_peer_ids(conn: &Connection, peer: &Peer) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT device_id FROM peers
             WHERE device_id <> ?1 AND (lower(device_id) = lower(?1) OR (address = ?2 AND port = ?3))",
        )
        .map_err(|err| format!("检查重复设备失败：{err}"))?;
    let rows = stmt
        .query_map(params![&peer.device_id, &peer.address, peer.port], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|err| format!("检查重复设备失败：{err}"))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("解析重复设备失败：{err}"))
}

fn dedupe_peer_list(peers: Vec<Peer>) -> Vec<Peer> {
    let mut seen_ids = HashSet::new();
    let mut seen_endpoints = HashSet::new();
    let mut result = Vec::new();
    for peer in peers {
        let id_key = normalize_device_id(&peer.device_id);
        let endpoint_key = format!("{}:{}", peer.address, peer.port);
        if seen_ids.contains(&id_key) || seen_endpoints.contains(&endpoint_key) {
            continue;
        }
        seen_ids.insert(id_key);
        seen_endpoints.insert(endpoint_key);
        result.push(peer);
    }
    result
}

fn normalize_client_kind(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        _ => "full".to_string(),
    }
}
pub fn system_login_nickname() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "LanChat 用户".to_string())
}

fn default_nickname() -> String {
    system_login_nickname()
}
fn ensure_column(
    conn: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|err| format!("检查数据库字段失败：{err}"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| format!("检查数据库字段失败：{err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("检查数据库字段失败：{err}"))?;
    if !columns.iter().any(|name| name == column) {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )
        .map_err(|err| format!("升级数据库字段失败：{err}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_profile_once() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");

        let first = storage.get_or_create_profile().expect("profile");
        let second = storage.get_or_create_profile().expect("profile");

        assert_eq!(first.device_id, second.device_id);
        assert!(!first.nickname.is_empty());
        assert!(first.listen_port > 0);
    }

    #[test]
    fn upserts_peer_and_lists_latest_value() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");

        storage
            .upsert_peer(&Peer {
                device_id: "peer-1".to_string(),
                nickname: "第一台".to_string(),
                note: None,
                avatar: None,
                address: "192.168.1.11".to_string(),
                port: 18145,
                online: true,
                last_seen_at: 10,
                client_kind: "full".to_string(),
                supports_chat: true,
                nickname_locked: false,
                build_version: "0.3.0+10".to_string(),
                build_timestamp: 10,
            })
            .expect("peer saved");
        storage
            .upsert_peer(&Peer {
                device_id: "peer-1".to_string(),
                nickname: "改名后".to_string(),
                note: None,
                avatar: Some("A".to_string()),
                address: "192.168.1.12".to_string(),
                port: 18146,
                online: true,
                last_seen_at: 20,
                client_kind: "full".to_string(),
                supports_chat: true,
                nickname_locked: true,
                build_version: "0.3.0+20".to_string(),
                build_timestamp: 20,
            })
            .expect("peer updated");

        let peers = storage.list_peers().expect("peers");
        let peer = storage
            .get_peer("peer-1")
            .expect("peer")
            .expect("peer exists");

        assert_eq!(1, peers.len());
        assert_eq!("改名后", peers[0].nickname);
        assert_eq!(Some("A".to_string()), peers[0].avatar);
        assert!(peers[0].nickname_locked);
        assert_eq!("0.3.0+20", peers[0].build_version);
        assert_eq!(20, peers[0].build_timestamp);
        assert_eq!("192.168.1.12", peers[0].address);
        assert_eq!(18146, peer.port);

        let mut heartbeat = peer.clone();
        heartbeat.avatar = None;
        heartbeat.last_seen_at = 30;
        storage.upsert_peer(&heartbeat).expect("heartbeat saved");
        assert_eq!(
            Some("A".to_string()),
            storage
                .get_peer("peer-1")
                .expect("peer")
                .expect("peer exists")
                .avatar
        );

        storage
            .update_peer_avatar("peer-1", None)
            .expect("avatar cleared");
        storage.upsert_peer(&heartbeat).expect("heartbeat saved");
        assert_eq!(
            None,
            storage
                .get_peer("peer-1")
                .expect("peer")
                .expect("peer exists")
                .avatar
        );
    }

    #[test]
    fn face_monitor_policy_prefers_device_override_and_keeps_newest_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open(&temp.path().join("lanchat.sqlite3")).expect("storage opens");
        storage
            .upsert_face_monitor_policy(&FaceMonitorPolicyFrame {
                target_device_id: "*".to_string(),
                min_confidence: 70,
                body_min_confidence: 69,
                sample_fps: 2,
                consecutive_hits: 2,
                cooldown_seconds: 60,
                face_cooldown_seconds: 0,
                body_cooldown_seconds: 0,
                settings_locked: false,
                version: 1,
                issued_by_device_id: "admin".to_string(),
                issued_by_nickname: "管理员".to_string(),
                issued_at: 1,
            })
            .expect("global policy");
        let migrated = storage
            .effective_face_monitor_policy("legacy-device")
            .expect("legacy policy read")
            .expect("legacy policy exists");
        assert_eq!(migrated.face_cooldown_seconds, 60);
        assert_eq!(migrated.body_cooldown_seconds, 60);
        storage
            .upsert_face_monitor_policy(&FaceMonitorPolicyFrame {
                target_device_id: "device-a".to_string(),
                min_confidence: 88,
                body_min_confidence: 76,
                sample_fps: 4,
                consecutive_hits: 3,
                cooldown_seconds: 90,
                face_cooldown_seconds: 45,
                body_cooldown_seconds: 120,
                settings_locked: true,
                version: 2,
                issued_by_device_id: "admin".to_string(),
                issued_by_nickname: "管理员".to_string(),
                issued_at: 2,
            })
            .expect("device policy");
        let effective = storage
            .effective_face_monitor_policy("device-a")
            .expect("policy read")
            .expect("policy exists");
        assert_eq!(effective.min_confidence, 88);
        assert_eq!(effective.body_min_confidence, 76);
        assert_eq!(effective.sample_fps, 4);
        assert_eq!(effective.face_cooldown_seconds, 45);
        assert_eq!(effective.body_cooldown_seconds, 120);
        assert!(effective.settings_locked);
        storage
            .upsert_face_monitor_policy(&FaceMonitorPolicyFrame {
                target_device_id: "device-a".to_string(),
                min_confidence: 10,
                body_min_confidence: 20,
                sample_fps: 1,
                consecutive_hits: 1,
                cooldown_seconds: 5,
                face_cooldown_seconds: 5,
                body_cooldown_seconds: 5,
                settings_locked: false,
                version: 1,
                issued_by_device_id: "old".to_string(),
                issued_by_nickname: "旧管理员".to_string(),
                issued_at: 0,
            })
            .expect("old policy ignored");
        assert_eq!(
            storage
                .effective_face_monitor_policy("device-a")
                .expect("policy read")
                .expect("policy exists")
                .min_confidence,
            88
        );
    }

    #[test]
    fn older_face_person_frame_does_not_override_newer_rule() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open(&temp.path().join("lanchat.sqlite3")).expect("storage opens");
        let mut frame = FacePersonPolicyFrame {
            person_id: "person-1".to_string(),
            display_name: "新名称".to_string(),
            photo_url: None,
            photo_urls: vec![],
            photo_sha256: None,
            photo_sha256s: vec![],
            expires_at: None,
            enabled: true,
            version: 2,
            action: "upsert".to_string(),
            issued_by_device_id: "admin".to_string(),
            issued_by_nickname: "管理员".to_string(),
            issued_at: 2,
        };
        storage
            .upsert_face_person(&frame)
            .expect("new person saved");
        frame.display_name = "旧名称".to_string();
        frame.version = 1;
        assert_eq!(
            storage
                .upsert_face_person(&frame)
                .expect("old frame ignored")
                .display_name,
            "新名称"
        );
    }

    #[test]
    fn deleted_face_person_is_not_returned_in_visible_list() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open(&temp.path().join("lanchat.sqlite3")).expect("storage opens");
        storage
            .upsert_face_person(&FacePersonPolicyFrame {
                person_id: "person-1".to_string(),
                display_name: "临时人员".to_string(),
                photo_url: None,
                photo_urls: vec![],
                photo_sha256: None,
                photo_sha256s: vec![],
                expires_at: None,
                enabled: true,
                version: 1,
                action: "upsert".to_string(),
                issued_by_device_id: "local".to_string(),
                issued_by_nickname: "本机".to_string(),
                issued_at: 1,
            })
            .expect("person saved");
        storage
            .delete_face_person_local("person-1")
            .expect("person deleted");
        assert!(storage.list_face_people().expect("visible list").is_empty());
    }

    #[test]
    fn face_people_store_embedding_and_model_version() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open(&temp.path().join("lanchat.sqlite3")).expect("storage opens");
        storage
            .upsert_face_person(&FacePersonPolicyFrame {
                person_id: "person-embedding".to_string(),
                display_name: "特征人员".to_string(),
                photo_url: None,
                photo_urls: vec![],
                photo_sha256: None,
                photo_sha256s: vec![],
                expires_at: None,
                enabled: true,
                version: 1,
                action: "upsert".to_string(),
                issued_by_device_id: "local".to_string(),
                issued_by_nickname: "本机".to_string(),
                issued_at: 1,
            })
            .expect("person saved");
        storage
            .update_face_person_embedding(
                "person-embedding",
                Some(vec![1u8, 2, 3]),
                Some("v1".to_string()),
            )
            .expect("embedding saved");
        let person = storage
            .list_face_people()
            .expect("visible list")
            .pop()
            .expect("person present");
        assert_eq!(person.embedding.as_deref(), Some([1u8, 2, 3].as_slice()));
        assert_eq!(person.embedding_model_version.as_deref(), Some("v1"));
        // 远端同步 upsert 不应覆盖本机已提取的特征。
        storage
            .upsert_face_person(&FacePersonPolicyFrame {
                person_id: "person-embedding".to_string(),
                display_name: "特征人员改名".to_string(),
                photo_url: None,
                photo_urls: vec![],
                photo_sha256: None,
                photo_sha256s: vec![],
                expires_at: None,
                enabled: true,
                version: 2,
                action: "upsert".to_string(),
                issued_by_device_id: "remote".to_string(),
                issued_by_nickname: "远端".to_string(),
                issued_at: 2,
            })
            .expect("person re-synced");
        let person = storage
            .list_face_people()
            .expect("visible list")
            .pop()
            .expect("person present");
        assert_eq!(person.display_name, "特征人员改名");
        assert_eq!(person.embedding.as_deref(), Some([1u8, 2, 3].as_slice()));
        assert_eq!(person.embedding_model_version.as_deref(), Some("v1"));
        // 提取失败时清空特征。
        storage
            .update_face_person_embedding("person-embedding", None, None)
            .expect("embedding cleared");
        let person = storage
            .list_face_people()
            .expect("visible list")
            .pop()
            .expect("person present");
        assert!(person.embedding.is_none());
        assert!(person.embedding_model_version.is_none());
    }

    #[test]
    fn face_person_keeps_multiple_reference_samples() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open(&temp.path().join("lanchat.sqlite3")).expect("storage opens");
        storage
            .upsert_face_person(&FacePersonPolicyFrame {
                person_id: "person-samples".to_string(),
                display_name: "多角度人员".to_string(),
                photo_url: Some("first.jpg".to_string()),
                photo_urls: vec!["first.jpg".to_string(), "side.jpg".to_string()],
                photo_sha256: None,
                photo_sha256s: vec![],
                expires_at: None,
                enabled: true,
                version: 1,
                action: "upsert".to_string(),
                issued_by_device_id: "local".to_string(),
                issued_by_nickname: "本机".to_string(),
                issued_at: 1,
            })
            .expect("person saved");
        let person = storage
            .list_face_people()
            .expect("visible list")
            .pop()
            .expect("person present");
        assert_eq!(person.sample_count, 2);
        assert_eq!(
            person.photo_urls,
            vec!["first.jpg".to_string(), "side.jpg".to_string()]
        );
        assert_eq!(
            storage
                .list_face_person_samples("person-samples")
                .expect("samples read")
                .len(),
            2
        );
    }

    #[test]
    fn camera_face_alert_list_restores_feedback_for_current_device() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        storage
            .upsert_camera_face_alert(&CameraFaceAlertFrame {
                alert_id: "face-alert-1".to_string(),
                source_kind: "camera_face".to_string(),
                source_device_id: "source-device".to_string(),
                source_nickname: "来源设备".to_string(),
                source_address: Some("192.168.1.8".to_string()),
                person_id: "person-1".to_string(),
                person_name: "测试人员".to_string(),
                confidence: 88,
                recognition_level: "confirmed".to_string(),
                face_confidence: Some(88),
                body_confidence: None,
                consecutive_hits: 2,
                policy_version: 1,
                created_at: 100,
            })
            .expect("alert saved");
        storage
            .upsert_camera_face_alert_feedback(&CameraFaceAlertFeedbackFrame {
                alert_id: "face-alert-1".to_string(),
                source_device_id: "source-device".to_string(),
                responder_device_id: "local-device".to_string(),
                responder_nickname: "本机".to_string(),
                result: "real".to_string(),
                created_at: 200,
            })
            .expect("feedback saved");
        drop(storage);

        let reopened = Storage::open(&db_path).expect("storage reopens");

        let local_records = reopened
            .list_camera_face_alerts_for_responder(100, "local-device")
            .expect("local records");
        let other_records = reopened
            .list_camera_face_alerts_for_responder(100, "other-device")
            .expect("other records");

        assert_eq!(local_records[0].local_feedback.as_deref(), Some("real"));
        assert_eq!(other_records[0].local_feedback, None);
    }

    #[test]
    fn clear_camera_face_alerts_removes_alerts_and_feedbacks() {
        let temp = tempfile::tempdir().expect("tempdir");
        let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
        storage
            .upsert_camera_face_alert(&CameraFaceAlertFrame {
                alert_id: "face-alert-clear".to_string(),
                source_kind: "camera_face".to_string(),
                source_device_id: "source-device".to_string(),
                source_nickname: "来源设备".to_string(),
                source_address: None,
                person_id: "person-1".to_string(),
                person_name: "测试人员".to_string(),
                confidence: 86,
                recognition_level: "confirmed".to_string(),
                face_confidence: Some(86),
                body_confidence: None,
                consecutive_hits: 2,
                policy_version: 1,
                created_at: 100,
            })
            .expect("alert saved");
        storage
            .upsert_camera_face_alert_feedback(&CameraFaceAlertFeedbackFrame {
                alert_id: "face-alert-clear".to_string(),
                source_device_id: "source-device".to_string(),
                responder_device_id: "local-device".to_string(),
                responder_nickname: "本机".to_string(),
                result: "real".to_string(),
                created_at: 200,
            })
            .expect("feedback saved");

        storage.clear_camera_face_alerts().expect("records cleared");

        assert!(storage
            .list_camera_face_alerts_for_responder(100, "local-device")
            .expect("records read")
            .is_empty());
    }

    #[test]
    fn saves_and_lists_messages_by_conversation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        storage
            .upsert_peer(&Peer {
                device_id: "peer-1".to_string(),
                nickname: "同事".to_string(),
                note: None,
                avatar: None,
                address: "192.168.1.11".to_string(),
                port: 18145,
                online: true,
                last_seen_at: 10,
                client_kind: "full".to_string(),
                supports_chat: true,
                nickname_locked: false,
                build_version: "0.3.0+10".to_string(),
                build_timestamp: 10,
            })
            .expect("peer");

        storage
            .save_message(&Message {
                id: "msg-1".to_string(),
                conversation_id: "peer-1".to_string(),
                sender_device_id: "me".to_string(),
                content: "第一条".to_string(),
                message_type: MessageType::Text,
                file_meta: None,
                status: MessageStatus::Sent,
                simulation: None,
                created_at: 10,
            })
            .expect("message saved");
        storage
            .save_message(&Message {
                id: "msg-2".to_string(),
                conversation_id: "peer-1".to_string(),
                sender_device_id: "peer-1".to_string(),
                content: "第二条".to_string(),
                message_type: MessageType::Text,
                file_meta: None,
                status: MessageStatus::Delivered,
                simulation: Some(SimulationMeta {
                    operator_device_id: "admin-1".to_string(),
                    operator_nickname: "超级管理员".to_string(),
                    display_label: true,
                    created_at: 20,
                }),
                created_at: 20,
            })
            .expect("message saved");

        let messages = storage.list_messages("peer-1").expect("messages");

        assert_eq!(2, messages.len());
        assert_eq!("第一条", messages[0].content);
        assert_eq!(MessageStatus::Delivered, messages[1].status);
        assert_eq!(
            Some("admin-1"),
            messages[1]
                .simulation
                .as_ref()
                .map(|item| item.operator_device_id.as_str())
        );
    }

    #[test]
    fn private_channel_members_and_key_round_trip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        let members = vec![
            ChannelMemberSeed {
                device_id: "owner-1".to_string(),
                nickname: "群主".to_string(),
                avatar: Some("O".to_string()),
            },
            ChannelMemberSeed {
                device_id: "peer-1".to_string(),
                nickname: "成员".to_string(),
                avatar: None,
            },
        ];

        storage
            .upsert_private_channel(
                "private-1",
                "午休私有频道",
                "owner-1",
                "群主",
                "test-key",
                1,
                &members,
                100,
            )
            .expect("private channel saved");

        let conversations = storage.list_conversations().expect("conversations");
        let conversation = conversations
            .iter()
            .find(|item| item.id == "private-1")
            .expect("conversation exists");
        let key = storage
            .private_channel_key("private-1")
            .expect("key")
            .expect("key exists");
        let listed_members = storage.list_channel_members("private-1").expect("members");

        assert_eq!("午休私有频道", conversation.title);
        assert!(conversation.is_private);
        assert_eq!(
            Some("owner-1".to_string()),
            conversation.owner_device_id.clone()
        );
        assert_eq!(("test-key".to_string(), 1), key);
        assert_eq!(2, listed_members.len());
        assert!(storage
            .is_private_channel_member("private-1", "peer-1")
            .expect("member"));
    }

    #[test]
    fn migrates_private_channel_membership_when_profile_id_changes_to_mac() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        storage
            .upsert_private_channel(
                "private-migration",
                "旧频道",
                "legacy-device-id",
                "我",
                "test-key",
                1,
                &[ChannelMemberSeed {
                    device_id: "legacy-device-id".to_string(),
                    nickname: "我".to_string(),
                    avatar: None,
                }],
                100,
            )
            .expect("private channel saved");

        storage
            .migrate_profile_device_identity("legacy-device-id", "aa-bb-cc-dd-ee-ff")
            .expect("profile identity migrated");

        assert!(storage
            .is_private_channel_member("private-migration", "AA:BB:CC:DD:EE:FF")
            .expect("current mac remains a channel member"));
        let members = storage
            .list_channel_members("private-migration")
            .expect("members listed");
        assert_eq!("aa:bb:cc:dd:ee:ff", members[0].device_id);
    }

    #[test]
    fn recognizes_legacy_mac_separator_variants_as_the_local_channel_member() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        let profile = storage.get_or_create_profile().expect("profile");
        let current_id = "aa:bb:cc:dd:ee:ff";
        {
            let conn = storage.conn.lock().expect("connection");
            conn.execute(
                "UPDATE profile SET device_id = ?1 WHERE id = 1",
                params![current_id],
            )
            .expect("profile device id updated");
        }
        storage
            .upsert_private_channel(
                "private-mac-separator",
                "格式兼容频道",
                current_id,
                &profile.nickname,
                "test-key",
                1,
                &[ChannelMemberSeed {
                    device_id: current_id.to_string(),
                    nickname: profile.nickname.clone(),
                    avatar: None,
                }],
                100,
            )
            .expect("private channel saved");
        {
            let conn = storage.conn.lock().expect("connection");
            conn.execute(
                "UPDATE channel_members SET device_id = 'aa-bb-cc-dd-ee-ff' WHERE channel_id = 'private-mac-separator'",
                [],
            )
            .expect("legacy member id updated");
        }

        assert!(storage
            .is_private_channel_member("private-mac-separator", current_id)
            .expect("separator variant is treated as the current member"));
        let member = storage
            .list_channel_members("private-mac-separator")
            .expect("members")
            .into_iter()
            .next()
            .expect("self member");
        assert!(member.online, "self must be rendered online");
    }

    #[test]
    fn repairs_legacy_private_channel_membership_by_matching_local_ip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        storage
            .upsert_peer(&Peer {
                device_id: "uuid_legacy_local".to_string(),
                nickname: "旧本机".to_string(),
                note: None,
                avatar: None,
                address: "192.168.50.8".to_string(),
                port: 18145,
                online: false,
                last_seen_at: 1,
                client_kind: "full".to_string(),
                supports_chat: true,
                nickname_locked: false,
                build_version: "0.1.0".to_string(),
                build_timestamp: 0,
            })
            .expect("legacy peer saved");
        storage
            .upsert_private_channel(
                "private-ip-migration",
                "历史频道",
                "uuid_legacy_local",
                "旧本机",
                "test-key",
                1,
                &[ChannelMemberSeed {
                    device_id: "uuid_legacy_local".to_string(),
                    nickname: "旧本机".to_string(),
                    avatar: None,
                }],
                100,
            )
            .expect("private channel saved");
        let profile = Profile {
            device_id: "aa:bb:cc:dd:ee:ff".to_string(),
            nickname: "新本机".to_string(),
            listen_port: 18145,
            avatar: None,
            nickname_locked: false,
        };

        storage
            .repair_legacy_private_channel_memberships(&profile, "192.168.50.8")
            .expect("legacy membership repaired");

        assert!(storage
            .is_private_channel_member("private-ip-migration", &profile.device_id)
            .expect("current profile is a channel member"));
    }

    #[test]
    fn stores_device_note_and_orders_online_peers_before_offline_peers() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        for (index, (id, nickname, online, last_seen_at)) in [
            ("aa:bb:cc:dd:ee:03", "Gamma", true, 300),
            ("aa:bb:cc:dd:ee:01", "Zeta", false, 100),
            ("aa:bb:cc:dd:ee:02", "Beta", false, 900),
        ]
        .into_iter()
        .enumerate()
        {
            storage
                .upsert_peer(&Peer {
                    device_id: id.to_string(),
                    nickname: nickname.to_string(),
                    note: None,
                    avatar: None,
                    address: format!("192.168.1.{}", index + 2),
                    port: 18145 + index as u16,
                    online,
                    last_seen_at,
                    client_kind: "full".to_string(),
                    supports_chat: true,
                    nickname_locked: false,
                    build_version: "0.3.3".to_string(),
                    build_timestamp: 0,
                })
                .expect("peer saved");
        }
        storage
            .update_peer_note("aa:bb:cc:dd:ee:01", "阿尔法")
            .expect("note saved");

        let first = storage.list_peers().expect("peers listed");
        assert_eq!(
            vec![
                "aa:bb:cc:dd:ee:03",
                "aa:bb:cc:dd:ee:01",
                "aa:bb:cc:dd:ee:02"
            ],
            first
                .iter()
                .map(|peer| peer.device_id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(Some("阿尔法".to_string()), first[1].note);

        let mut refreshed = first[2].clone();
        refreshed.online = false;
        refreshed.last_seen_at = 9_999;
        storage
            .upsert_peer(&refreshed)
            .expect("heartbeat refresh saved");
        let second = storage.list_peers().expect("peers listed again");
        assert_eq!(
            vec![
                "aa:bb:cc:dd:ee:03",
                "aa:bb:cc:dd:ee:01",
                "aa:bb:cc:dd:ee:02"
            ],
            second
                .iter()
                .map(|peer| peer.device_id.as_str())
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn marks_stale_peers_offline() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");
        storage
            .upsert_peer(&Peer {
                device_id: "peer-1".to_string(),
                nickname: "同事".to_string(),
                note: None,
                avatar: None,
                address: "192.168.1.11".to_string(),
                port: 18145,
                online: true,
                last_seen_at: 10,
                client_kind: "full".to_string(),
                supports_chat: true,
                nickname_locked: false,
                build_version: "0.3.0+10".to_string(),
                build_timestamp: 10,
            })
            .expect("peer");

        let offline = storage.mark_stale_peers_offline(20).expect("marked");
        let peer = storage.get_peer("peer-1").expect("peer").expect("exists");

        assert_eq!(vec!["peer-1".to_string()], offline);
        assert!(!peer.online);
    }

    #[test]
    fn unsupported_peer_is_listed_as_device_but_not_as_chat_conversation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("lanchat.sqlite3");
        let storage = Storage::open(&db_path).expect("storage opens");

        storage
            .upsert_peer(&Peer {
                device_id: "limited-1".to_string(),
                nickname: "受限设备".to_string(),
                note: None,
                avatar: None,
                address: "192.168.1.50".to_string(),
                port: 18145,
                online: true,
                last_seen_at: 10,
                client_kind: "unknown".to_string(),
                supports_chat: false,
                nickname_locked: false,
                build_version: "0.3.0+10".to_string(),
                build_timestamp: 10,
            })
            .expect("limited peer saved");

        let listed_peers = storage.list_peers().expect("peers");
        assert_eq!(1, listed_peers.len());
        assert_eq!("full", listed_peers[0].client_kind);
        assert!(!listed_peers[0].supports_chat);
        assert!(!storage
            .list_conversations()
            .expect("conversations")
            .iter()
            .any(|conversation| conversation.id == "limited-1"));
    }
}
