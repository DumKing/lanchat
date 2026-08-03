use crate::file_server::FileMeta;
use crate::identity::{normalize_device_id, resolve_device_id, resolve_profile_device_id};
use crate::network::local_ip_address;
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
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub sender_device_id: String,
    pub content: String,
    pub message_type: MessageType,
    pub file_meta: Option<FileMeta>,
    pub status: MessageStatus,
    pub created_at: i64,
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
                created_at INTEGER NOT NULL
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
            CREATE INDEX IF NOT EXISTS idx_messages_conversation_created
                ON messages(conversation_id, created_at);
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
        Ok(())
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
            INSERT INTO messages (id, conversation_id, sender_device_id, content, message_type, file_name, file_size, file_url, file_mime_type, file_duration_ms, status, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
            "SELECT id, conversation_id, sender_device_id, content, message_type, file_name, file_size, file_url, file_mime_type, file_duration_ms, status, created_at
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
                    created_at: row.get(11)?,
                })
            },
        )
        .optional()
        .map_err(|err| format!("读取撤回消息失败：{err}"))
    }

    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<Message>, String> {
        let conn = self.conn.lock().map_err(|_| "数据库锁已损坏".to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT id, conversation_id, sender_device_id, content, message_type, file_name, file_size, file_url, file_mime_type, file_duration_ms, status, created_at
                 FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|err| format!("读取消息失败：{err}"))?;
        let rows = stmt
            .query_map(params![conversation_id], |row| {
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
                    created_at: row.get(11)?,
                })
            })
            .map_err(|err| format!("读取消息失败：{err}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| format!("解析消息失败：{err}"))
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
                created_at: 20,
            })
            .expect("message saved");

        let messages = storage.list_messages("peer-1").expect("messages");

        assert_eq!(2, messages.len());
        assert_eq!("第一条", messages[0].content);
        assert_eq!(MessageStatus::Delivered, messages[1].status);
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
    fn stores_device_note_and_keeps_peer_order_stable_across_heartbeats() {
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
                "aa:bb:cc:dd:ee:01",
                "aa:bb:cc:dd:ee:02",
                "aa:bb:cc:dd:ee:03"
            ],
            first
                .iter()
                .map(|peer| peer.device_id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(Some("阿尔法".to_string()), first[0].note);

        let mut refreshed = first[2].clone();
        refreshed.online = false;
        refreshed.last_seen_at = 9_999;
        storage
            .upsert_peer(&refreshed)
            .expect("heartbeat refresh saved");
        let second = storage.list_peers().expect("peers listed again");
        assert_eq!(
            vec![
                "aa:bb:cc:dd:ee:01",
                "aa:bb:cc:dd:ee:02",
                "aa:bb:cc:dd:ee:03"
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
