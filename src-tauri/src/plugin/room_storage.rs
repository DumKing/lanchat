use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

pub const MAX_COMMIT_PAYLOAD_BYTES: usize = 256 * 1024;
pub const MAX_CHECKPOINT_PAYLOAD_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ROOM_PAYLOAD_BYTES: i64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoomAdmission {
    Lobby,
    Closed,
}

impl RoomAdmission {
    fn as_str(self) -> &'static str {
        match self {
            Self::Lobby => "lobby",
            Self::Closed => "closed",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "lobby" => Ok(Self::Lobby),
            "closed" => Ok(Self::Closed),
            _ => Err("房间 admission 数据无效".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomMemberInput {
    pub peer_id: String,
    pub seat_index: i64,
    pub joined_at: i64,
}

pub type StoredRoomMember = RoomMemberInput;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRoomMetadataInput {
    pub plugin_id: String,
    pub room_id: String,
    pub game_id: String,
    pub owner_peer_id: String,
    pub max_players: i64,
    pub schema_version: i64,
    pub protocol_version: i64,
    pub room_version: i64,
    pub members: Vec<RoomMemberInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredRoomMetadata {
    pub plugin_id: String,
    pub room_id: String,
    pub game_id: String,
    pub owner_peer_id: String,
    pub admission: RoomAdmission,
    pub max_players: i64,
    pub schema_version: i64,
    pub protocol_version: i64,
    pub room_version: i64,
    pub revision: i64,
    pub terminal_commit_id: Option<String>,
    pub terminal_state_hash: Option<String>,
    pub closed_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendCommitInput {
    pub commit_id: String,
    pub command_id: String,
    pub base_revision: i64,
    pub protocol_version: i64,
    pub payload: Value,
    pub state_hash: String,
    pub terminal_result_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredCommit {
    pub commit_id: String,
    pub command_id: String,
    pub base_revision: i64,
    pub revision: i64,
    pub protocol_version: i64,
    pub payload: Value,
    pub state_hash: String,
    pub terminal_result_hash: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointInput {
    pub revision: i64,
    pub protocol_version: i64,
    pub payload: Value,
    pub state_hash: String,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredCheckpoint {
    pub revision: i64,
    pub protocol_version: i64,
    pub payload: Value,
    pub state_hash: String,
    pub checksum: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredRoom {
    pub metadata: StoredRoomMetadata,
    pub members: Vec<StoredRoomMember>,
    pub checkpoint: Option<StoredCheckpoint>,
    pub commits: Vec<StoredCommit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomRecovery {
    pub metadata: StoredRoomMetadata,
    pub members: Vec<StoredRoomMember>,
    pub head_revision: i64,
    pub checkpoint: Option<StoredCheckpoint>,
    pub commits: Vec<StoredCommit>,
}

pub struct PluginRoomStorage {
    connection: Mutex<Connection>,
}

impl PluginRoomStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let connection =
            Connection::open(path).map_err(|error| format!("打开插件房间数据库失败: {error}"))?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| format!("配置插件房间数据库超时失败: {error}"))?;
        connection
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;
                PRAGMA journal_mode = WAL;
                CREATE TABLE IF NOT EXISTS plugin_rooms (
                    plugin_id TEXT NOT NULL,
                    room_id TEXT NOT NULL,
                    game_id TEXT NOT NULL,
                    owner_peer_id TEXT NOT NULL,
                    admission TEXT NOT NULL CHECK (admission IN ('lobby', 'closed')),
                    max_players INTEGER NOT NULL CHECK (max_players > 0),
                    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
                    protocol_version INTEGER NOT NULL CHECK (protocol_version > 0),
                    room_version INTEGER NOT NULL CHECK (room_version > 0),
                    revision INTEGER NOT NULL CHECK (revision >= 0),
                    terminal_commit_id TEXT,
                    terminal_state_hash TEXT,
                    closed_at INTEGER,
                    updated_at INTEGER NOT NULL,
                    PRIMARY KEY (plugin_id, room_id)
                );
                CREATE TABLE IF NOT EXISTS plugin_room_members (
                    plugin_id TEXT NOT NULL,
                    room_id TEXT NOT NULL,
                    peer_id TEXT NOT NULL,
                    seat_index INTEGER NOT NULL CHECK (seat_index >= 0),
                    joined_at INTEGER NOT NULL,
                    PRIMARY KEY (plugin_id, room_id, peer_id),
                    UNIQUE (plugin_id, room_id, seat_index),
                    FOREIGN KEY (plugin_id, room_id) REFERENCES plugin_rooms(plugin_id, room_id) ON DELETE CASCADE
                );
                CREATE TABLE IF NOT EXISTS plugin_room_commits (
                    plugin_id TEXT NOT NULL,
                    room_id TEXT NOT NULL,
                    commit_id TEXT NOT NULL,
                    command_id TEXT NOT NULL,
                    base_revision INTEGER NOT NULL CHECK (base_revision >= 0),
                    revision INTEGER NOT NULL CHECK (revision > 0),
                    protocol_version INTEGER NOT NULL CHECK (protocol_version > 0),
                    payload_json TEXT NOT NULL,
                    state_hash TEXT NOT NULL,
                    terminal_result_hash TEXT,
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY (plugin_id, room_id, revision),
                    UNIQUE (plugin_id, room_id, commit_id),
                    UNIQUE (plugin_id, room_id, command_id),
                    FOREIGN KEY (plugin_id, room_id) REFERENCES plugin_rooms(plugin_id, room_id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_plugin_room_commits_recovery
                    ON plugin_room_commits(plugin_id, room_id, protocol_version, revision);
                CREATE TABLE IF NOT EXISTS plugin_room_checkpoints (
                    plugin_id TEXT NOT NULL,
                    room_id TEXT NOT NULL,
                    revision INTEGER NOT NULL CHECK (revision >= 0),
                    protocol_version INTEGER NOT NULL CHECK (protocol_version > 0),
                    payload_json TEXT NOT NULL,
                    state_hash TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY (plugin_id, room_id, revision),
                    FOREIGN KEY (plugin_id, room_id) REFERENCES plugin_rooms(plugin_id, room_id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_plugin_room_checkpoints_recovery
                    ON plugin_room_checkpoints(plugin_id, room_id, protocol_version, revision DESC);
                "#,
            )
            .map_err(|error| format!("初始化插件房间数据库失败: {error}"))?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn save_metadata(
        &self,
        input: SaveRoomMetadataInput,
        updated_at: i64,
    ) -> Result<StoredRoomMetadata, String> {
        validate_metadata(&input)?;
        let mut connection = self.lock()?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("启动房间事务失败: {error}"))?;
        let current = query_metadata(&transaction, &input.plugin_id, &input.room_id)?;
        if let Some(current) = &current {
            if input.room_version <= current.room_version {
                return Err("roomVersion 必须单调递增".into());
            }
            if input.owner_peer_id != current.owner_peer_id || input.game_id != current.game_id {
                return Err("不能修改房间的 gameId 或房主".into());
            }
            if input.protocol_version != current.protocol_version {
                return Err("不能修改房间协议版本".into());
            }
            transaction
                .execute(
                    "UPDATE plugin_rooms SET max_players=?1,schema_version=?2,room_version=?3,updated_at=?4 WHERE plugin_id=?5 AND room_id=?6",
                    params![input.max_players, input.schema_version, input.room_version, updated_at, input.plugin_id, input.room_id],
                )
                .map_err(|error| format!("更新插件房间失败: {error}"))?;
            transaction
                .execute(
                    "DELETE FROM plugin_room_members WHERE plugin_id=?1 AND room_id=?2",
                    params![input.plugin_id, input.room_id],
                )
                .map_err(|error| format!("更新房间成员失败: {error}"))?;
        } else {
            transaction
                .execute(
                    "INSERT INTO plugin_rooms (plugin_id,room_id,game_id,owner_peer_id,admission,max_players,schema_version,protocol_version,room_version,revision,updated_at) VALUES (?1,?2,?3,?4,'lobby',?5,?6,?7,?8,0,?9)",
                    params![input.plugin_id, input.room_id, input.game_id, input.owner_peer_id, input.max_players, input.schema_version, input.protocol_version, input.room_version, updated_at],
                )
                .map_err(|error| format!("创建插件房间失败: {error}"))?;
        }
        for member in &input.members {
            transaction
                .execute(
                    "INSERT INTO plugin_room_members (plugin_id,room_id,peer_id,seat_index,joined_at) VALUES (?1,?2,?3,?4,?5)",
                    params![input.plugin_id, input.room_id, member.peer_id, member.seat_index, member.joined_at],
                )
                .map_err(|_| "房间成员或席位重复".to_string())?;
        }
        let metadata = query_metadata(&transaction, &input.plugin_id, &input.room_id)?
            .ok_or_else(|| "房间保存后不存在".to_string())?;
        transaction
            .commit()
            .map_err(|error| format!("提交房间事务失败: {error}"))?;
        Ok(metadata)
    }

    pub fn set_admission(
        &self,
        plugin_id: &str,
        room_id: &str,
        caller_peer_id: &str,
        admission: RoomAdmission,
        room_version: i64,
        updated_at: i64,
    ) -> Result<StoredRoomMetadata, String> {
        if room_version <= 0 {
            return Err("roomVersion 必须为正数".into());
        }
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(db_transaction_error)?;
        let current = require_room(&transaction, plugin_id, room_id)?;
        require_owner(&current, caller_peer_id)?;
        if current.closed_at.is_some() && admission != RoomAdmission::Closed {
            return Err("房间已关闭".into());
        }
        if room_version <= current.room_version {
            return Err("roomVersion 必须单调递增".into());
        }
        transaction
            .execute(
                "UPDATE plugin_rooms SET admission=?1,room_version=?2,updated_at=?3 WHERE plugin_id=?4 AND room_id=?5",
                params![admission.as_str(), room_version, updated_at, plugin_id, room_id],
            )
            .map_err(|error| format!("更新房间开放状态失败: {error}"))?;
        let result = require_room(&transaction, plugin_id, room_id)?;
        transaction.commit().map_err(commit_transaction_error)?;
        Ok(result)
    }

    pub fn append_commit(
        &self,
        plugin_id: &str,
        room_id: &str,
        caller_peer_id: &str,
        input: AppendCommitInput,
        created_at: i64,
    ) -> Result<StoredCommit, String> {
        validate_non_empty("commitId", &input.commit_id)?;
        validate_non_empty("commandId", &input.command_id)?;
        validate_non_empty("stateHash", &input.state_hash)?;
        if input.base_revision < 0 || input.protocol_version <= 0 {
            return Err("baseRevision 必须非负且协议版本必须为正数".into());
        }
        let payload_json = serialize_payload(
            &input.payload,
            MAX_COMMIT_PAYLOAD_BYTES,
            "commit",
            "256 KiB",
        )?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(db_transaction_error)?;
        let room = require_room(&transaction, plugin_id, room_id)?;
        require_owner(&room, caller_peer_id)?;
        if input.protocol_version != room.protocol_version {
            return Err("commit 协议版本与房间不兼容".into());
        }
        if let Some(existing) =
            query_commit_by_command(&transaction, plugin_id, room_id, &input.command_id)?
        {
            transaction.commit().map_err(commit_transaction_error)?;
            return Ok(existing);
        }
        if room.closed_at.is_some() {
            return Err("房间已关闭，不能追加 commit".into());
        }
        if input.base_revision != room.revision {
            return Err(format!(
                "baseRevision 与当前 revision {} 不一致",
                room.revision
            ));
        }
        ensure_quota(
            &transaction,
            plugin_id,
            room_id,
            payload_json.len() as i64,
            None,
        )?;
        let revision = input.base_revision + 1;
        transaction
            .execute(
                "INSERT INTO plugin_room_commits (plugin_id,room_id,commit_id,command_id,base_revision,revision,protocol_version,payload_json,state_hash,terminal_result_hash,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![plugin_id, room_id, input.commit_id, input.command_id, input.base_revision, revision, input.protocol_version, payload_json, input.state_hash, input.terminal_result_hash, created_at],
            )
            .map_err(|error| {
                let message = error.to_string();
                if message.contains("commit_id") { "房间 commitId 已存在".into() } else { format!("保存房间 commit 失败: {error}") }
            })?;
        transaction
            .execute(
                "UPDATE plugin_rooms SET revision=?1,terminal_commit_id=CASE WHEN ?2 IS NULL THEN terminal_commit_id ELSE ?3 END,terminal_state_hash=CASE WHEN ?2 IS NULL THEN terminal_state_hash ELSE ?4 END,updated_at=?5 WHERE plugin_id=?6 AND room_id=?7",
                params![revision, input.terminal_result_hash, input.commit_id, input.state_hash, created_at, plugin_id, room_id],
            )
            .map_err(|error| format!("更新房间 head 失败: {error}"))?;
        let result = query_commit_by_command(&transaction, plugin_id, room_id, &input.command_id)?
            .ok_or_else(|| "commit 保存后不存在".to_string())?;
        transaction.commit().map_err(commit_transaction_error)?;
        Ok(result)
    }

    pub fn save_checkpoint(
        &self,
        plugin_id: &str,
        room_id: &str,
        caller_peer_id: &str,
        input: CheckpointInput,
        created_at: i64,
    ) -> Result<StoredCheckpoint, String> {
        validate_non_empty("stateHash", &input.state_hash)?;
        validate_non_empty("checksum", &input.checksum)?;
        if input.revision < 0 || input.protocol_version <= 0 {
            return Err("checkpoint revision 必须非负且协议版本必须为正数".into());
        }
        let payload_json = serialize_payload(
            &input.payload,
            MAX_CHECKPOINT_PAYLOAD_BYTES,
            "checkpoint",
            "2 MiB",
        )?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(db_transaction_error)?;
        let room = require_room(&transaction, plugin_id, room_id)?;
        require_owner(&room, caller_peer_id)?;
        if input.protocol_version != room.protocol_version {
            return Err("checkpoint 协议版本与房间不兼容".into());
        }
        if input.revision != room.revision {
            return Err(format!(
                "checkpoint revision 必须等于当前 revision {}",
                room.revision
            ));
        }
        if input.revision > 0 {
            let head_hash: String = transaction
                .query_row(
                    "SELECT state_hash FROM plugin_room_commits WHERE plugin_id=?1 AND room_id=?2 AND revision=?3",
                    params![plugin_id, room_id, input.revision],
                    |row| row.get(0),
                )
                .map_err(|_| "当前 head commit 不存在，无法保存 checkpoint".to_string())?;
            if input.state_hash != head_hash {
                return Err("checkpoint stateHash 与当前 head 不一致".into());
            }
        }
        let latest_revision: Option<i64> = transaction
            .query_row(
                "SELECT MAX(revision) FROM plugin_room_checkpoints WHERE plugin_id=?1 AND room_id=?2",
                params![plugin_id, room_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("读取 checkpoint revision 失败: {error}"))?;
        if latest_revision.is_some_and(|revision| input.revision <= revision) {
            return Err("旧 revision 的 checkpoint 不可覆盖".into());
        }
        ensure_quota(
            &transaction,
            plugin_id,
            room_id,
            payload_json.len() as i64,
            Some(input.revision),
        )?;
        transaction
            .execute(
                "INSERT INTO plugin_room_checkpoints (plugin_id,room_id,revision,protocol_version,payload_json,state_hash,checksum,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![plugin_id, room_id, input.revision, input.protocol_version, payload_json, input.state_hash, input.checksum, created_at],
            )
            .map_err(|error| format!("保存房间 checkpoint 失败: {error}"))?;
        transaction
            .execute(
                "DELETE FROM plugin_room_commits WHERE plugin_id=?1 AND room_id=?2 AND revision<=?3",
                params![plugin_id, room_id, input.revision],
            )
            .map_err(|error| format!("裁剪旧 commit 失败: {error}"))?;
        let result = query_checkpoint(&transaction, plugin_id, room_id, input.protocol_version)?
            .ok_or_else(|| "checkpoint 保存后不存在".to_string())?;
        transaction.commit().map_err(commit_transaction_error)?;
        Ok(result)
    }

    pub fn load(&self, plugin_id: &str, room_id: &str) -> Result<Option<StoredRoom>, String> {
        let connection = self.lock()?;
        let Some(metadata) = query_metadata(&connection, plugin_id, room_id)? else {
            return Ok(None);
        };
        let members = query_members(&connection, plugin_id, room_id)?;
        let checkpoint =
            query_checkpoint(&connection, plugin_id, room_id, metadata.protocol_version)?;
        let commits = query_commits_after(&connection, plugin_id, room_id, -1)?;
        Ok(Some(StoredRoom {
            metadata,
            members,
            checkpoint,
            commits,
        }))
    }

    pub fn recover(
        &self,
        plugin_id: &str,
        room_id: &str,
        protocol_version: i64,
        after_revision: i64,
    ) -> Result<RoomRecovery, String> {
        if protocol_version <= 0 || after_revision < 0 {
            return Err("恢复协议版本必须为正数，afterRevision 必须非负".into());
        }
        let connection = self.lock()?;
        let metadata = query_metadata(&connection, plugin_id, room_id)?
            .ok_or_else(|| "插件房间不存在".to_string())?;
        if metadata.protocol_version != protocol_version {
            return Err("恢复协议版本与房间不兼容".into());
        }
        let members = query_members(&connection, plugin_id, room_id)?;
        let checkpoint = query_checkpoint(&connection, plugin_id, room_id, protocol_version)?;
        let start_revision = checkpoint
            .as_ref()
            .map(|item| item.revision.max(after_revision))
            .unwrap_or(after_revision);
        let commits = query_commits_after(&connection, plugin_id, room_id, start_revision)?;
        let mut expected = start_revision + 1;
        for item in &commits {
            if item.revision != expected {
                return Err(format!("恢复 commit 不连续，缺少 revision {expected}"));
            }
            expected += 1;
        }
        if expected - 1 != metadata.revision {
            return Err("恢复数据未到达当前 head revision".into());
        }
        Ok(RoomRecovery {
            head_revision: metadata.revision,
            metadata,
            members,
            checkpoint,
            commits,
        })
    }

    pub fn close(
        &self,
        plugin_id: &str,
        room_id: &str,
        caller_peer_id: &str,
        closed_at: i64,
    ) -> Result<StoredRoomMetadata, String> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction().map_err(db_transaction_error)?;
        let room = require_room(&transaction, plugin_id, room_id)?;
        require_owner(&room, caller_peer_id)?;
        if room.closed_at.is_none() {
            transaction
                .execute(
                    "UPDATE plugin_rooms SET admission='closed',room_version=room_version+1,closed_at=?1,updated_at=?1 WHERE plugin_id=?2 AND room_id=?3",
                    params![closed_at, plugin_id, room_id],
                )
                .map_err(|error| format!("关闭插件房间失败: {error}"))?;
        }
        let result = require_room(&transaction, plugin_id, room_id)?;
        transaction.commit().map_err(commit_transaction_error)?;
        Ok(result)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.connection
            .lock()
            .map_err(|_| "插件房间数据库锁已损坏".to_string())
    }
}

fn validate_metadata(input: &SaveRoomMetadataInput) -> Result<(), String> {
    for (name, value) in [
        ("pluginId", &input.plugin_id),
        ("roomId", &input.room_id),
        ("gameId", &input.game_id),
        ("ownerPeerId", &input.owner_peer_id),
    ] {
        validate_non_empty(name, value)?;
    }
    if input.max_players <= 0
        || input.schema_version <= 0
        || input.protocol_version <= 0
        || input.room_version <= 0
    {
        return Err("maxPlayers、schemaVersion、protocolVersion 和 roomVersion 必须为正数".into());
    }
    if input.members.len() as i64 > input.max_players {
        return Err("房间成员数量超过 maxPlayers".into());
    }
    let mut peers = HashSet::new();
    let mut seats = HashSet::new();
    for member in &input.members {
        validate_non_empty("peerId", &member.peer_id)?;
        if member.seat_index < 0
            || member.seat_index >= input.max_players
            || !peers.insert(&member.peer_id)
            || !seats.insert(member.seat_index)
        {
            return Err("房间成员或席位重复，seatIndex 必须在 maxPlayers 范围内".into());
        }
    }
    if !peers.contains(&input.owner_peer_id) {
        return Err("房主必须在房间成员中".into());
    }
    Ok(())
}

fn validate_non_empty(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{name} 不能为空"))
    } else {
        Ok(())
    }
}

fn serialize_payload(
    value: &Value,
    maximum: usize,
    label: &str,
    limit: &str,
) -> Result<String, String> {
    let serialized = serde_json::to_string(value)
        .map_err(|error| format!("序列化 {label} payload 失败: {error}"))?;
    if serialized.len() > maximum {
        return Err(format!("{label} payload 不能超过 {limit}"));
    }
    Ok(serialized)
}

fn query_metadata(
    connection: &Connection,
    plugin_id: &str,
    room_id: &str,
) -> Result<Option<StoredRoomMetadata>, String> {
    connection
        .query_row(
            "SELECT plugin_id,room_id,game_id,owner_peer_id,admission,max_players,schema_version,protocol_version,room_version,revision,terminal_commit_id,terminal_state_hash,closed_at,updated_at FROM plugin_rooms WHERE plugin_id=?1 AND room_id=?2",
            params![plugin_id, room_id],
            metadata_from_row,
        )
        .optional()
        .map_err(|error| format!("读取插件房间失败: {error}"))?
        .map(|room| room.and_then(|room| Ok(room)))
        .transpose()
}

fn metadata_from_row(row: &Row<'_>) -> rusqlite::Result<Result<StoredRoomMetadata, String>> {
    let admission: String = row.get(4)?;
    Ok(
        RoomAdmission::parse(&admission).map(|admission| StoredRoomMetadata {
            plugin_id: row.get(0).unwrap_or_default(),
            room_id: row.get(1).unwrap_or_default(),
            game_id: row.get(2).unwrap_or_default(),
            owner_peer_id: row.get(3).unwrap_or_default(),
            admission,
            max_players: row.get(5).unwrap_or_default(),
            schema_version: row.get(6).unwrap_or_default(),
            protocol_version: row.get(7).unwrap_or_default(),
            room_version: row.get(8).unwrap_or_default(),
            revision: row.get(9).unwrap_or_default(),
            terminal_commit_id: row.get(10).unwrap_or_default(),
            terminal_state_hash: row.get(11).unwrap_or_default(),
            closed_at: row.get(12).unwrap_or_default(),
            updated_at: row.get(13).unwrap_or_default(),
        }),
    )
}

fn require_room(
    connection: &Connection,
    plugin_id: &str,
    room_id: &str,
) -> Result<StoredRoomMetadata, String> {
    query_metadata(connection, plugin_id, room_id)?.ok_or_else(|| "插件房间不存在".into())
}

fn require_owner(room: &StoredRoomMetadata, caller_peer_id: &str) -> Result<(), String> {
    if room.owner_peer_id == caller_peer_id {
        Ok(())
    } else {
        Err("只有房主可以执行此操作".into())
    }
}

fn query_members(
    connection: &Connection,
    plugin_id: &str,
    room_id: &str,
) -> Result<Vec<StoredRoomMember>, String> {
    let mut statement = connection
        .prepare("SELECT peer_id,seat_index,joined_at FROM plugin_room_members WHERE plugin_id=?1 AND room_id=?2 ORDER BY seat_index")
        .map_err(|error| format!("准备房间成员查询失败: {error}"))?;
    let rows = statement
        .query_map(params![plugin_id, room_id], |row| {
            Ok(StoredRoomMember {
                peer_id: row.get(0)?,
                seat_index: row.get(1)?,
                joined_at: row.get(2)?,
            })
        })
        .map_err(|error| format!("读取房间成员失败: {error}"))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析房间成员失败: {error}"))
}

fn commit_from_row(row: &Row<'_>) -> rusqlite::Result<StoredCommit> {
    let payload_json: String = row.get(5)?;
    let payload = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            payload_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(StoredCommit {
        commit_id: row.get(0)?,
        command_id: row.get(1)?,
        base_revision: row.get(2)?,
        revision: row.get(3)?,
        protocol_version: row.get(4)?,
        payload,
        state_hash: row.get(6)?,
        terminal_result_hash: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn query_commit_by_command(
    connection: &Connection,
    plugin_id: &str,
    room_id: &str,
    command_id: &str,
) -> Result<Option<StoredCommit>, String> {
    connection.query_row(
        "SELECT commit_id,command_id,base_revision,revision,protocol_version,payload_json,state_hash,terminal_result_hash,created_at FROM plugin_room_commits WHERE plugin_id=?1 AND room_id=?2 AND command_id=?3",
        params![plugin_id, room_id, command_id], commit_from_row,
    ).optional().map_err(|error| format!("读取幂等 commit 失败: {error}"))
}

fn query_commits_after(
    connection: &Connection,
    plugin_id: &str,
    room_id: &str,
    revision: i64,
) -> Result<Vec<StoredCommit>, String> {
    let mut statement = connection.prepare(
        "SELECT commit_id,command_id,base_revision,revision,protocol_version,payload_json,state_hash,terminal_result_hash,created_at FROM plugin_room_commits WHERE plugin_id=?1 AND room_id=?2 AND revision>?3 ORDER BY revision"
    ).map_err(|error| format!("准备恢复 commit 查询失败: {error}"))?;
    let rows = statement
        .query_map(params![plugin_id, room_id, revision], commit_from_row)
        .map_err(|error| format!("读取恢复 commit 失败: {error}"))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| format!("解析恢复 commit 失败: {error}"))
}

fn checkpoint_from_row(row: &Row<'_>) -> rusqlite::Result<StoredCheckpoint> {
    let payload_json: String = row.get(2)?;
    let payload = serde_json::from_str(&payload_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            payload_json.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })?;
    Ok(StoredCheckpoint {
        revision: row.get(0)?,
        protocol_version: row.get(1)?,
        payload,
        state_hash: row.get(3)?,
        checksum: row.get(4)?,
        created_at: row.get(5)?,
    })
}

fn query_checkpoint(
    connection: &Connection,
    plugin_id: &str,
    room_id: &str,
    protocol_version: i64,
) -> Result<Option<StoredCheckpoint>, String> {
    connection.query_row(
        "SELECT revision,protocol_version,payload_json,state_hash,checksum,created_at FROM plugin_room_checkpoints WHERE plugin_id=?1 AND room_id=?2 AND protocol_version=?3 ORDER BY revision DESC LIMIT 1",
        params![plugin_id, room_id, protocol_version], checkpoint_from_row,
    ).optional().map_err(|error| format!("读取恢复 checkpoint 失败: {error}"))
}

fn ensure_quota(
    transaction: &Transaction<'_>,
    plugin_id: &str,
    room_id: &str,
    added_bytes: i64,
    prune_through: Option<i64>,
) -> Result<(), String> {
    let commit_bytes: i64 = if let Some(revision) = prune_through {
        transaction.query_row("SELECT COALESCE(SUM(LENGTH(CAST(payload_json AS BLOB))),0) FROM plugin_room_commits WHERE plugin_id=?1 AND room_id=?2 AND revision>?3", params![plugin_id, room_id, revision], |row| row.get(0))
    } else {
        transaction.query_row("SELECT COALESCE(SUM(LENGTH(CAST(payload_json AS BLOB))),0) FROM plugin_room_commits WHERE plugin_id=?1 AND room_id=?2", params![plugin_id, room_id], |row| row.get(0))
    }.map_err(|error| format!("计算房间 commit 配额失败: {error}"))?;
    let checkpoint_bytes: i64 = transaction.query_row(
        "SELECT COALESCE(SUM(LENGTH(CAST(payload_json AS BLOB))),0) FROM plugin_room_checkpoints WHERE plugin_id=?1 AND room_id=?2",
        params![plugin_id, room_id], |row| row.get(0),
    ).map_err(|error| format!("计算房间 checkpoint 配额失败: {error}"))?;
    if commit_bytes + checkpoint_bytes + added_bytes > MAX_ROOM_PAYLOAD_BYTES {
        Err("单房间持久化 payload 总量不能超过 16 MiB".into())
    } else {
        Ok(())
    }
}

fn db_transaction_error(error: rusqlite::Error) -> String {
    format!("启动房间事务失败: {error}")
}

fn commit_transaction_error(error: rusqlite::Error) -> String {
    format!("提交房间事务失败: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::storage::PluginStorage;
    use tempfile::TempDir;

    fn storage() -> (TempDir, PluginRoomStorage) {
        let temp = tempfile::tempdir().unwrap();
        let storage = PluginRoomStorage::open(temp.path().join("plugins.sqlite3")).unwrap();
        (temp, storage)
    }

    fn metadata(plugin_id: &str, room_id: &str) -> SaveRoomMetadataInput {
        SaveRoomMetadataInput {
            plugin_id: plugin_id.into(),
            room_id: room_id.into(),
            game_id: "game".into(),
            owner_peer_id: "owner".into(),
            max_players: 4,
            schema_version: 1,
            protocol_version: 2,
            room_version: 1,
            members: vec![RoomMemberInput {
                peer_id: "owner".into(),
                seat_index: 0,
                joined_at: 10,
            }],
        }
    }

    fn commit(
        command_id: &str,
        base_revision: i64,
        payload: serde_json::Value,
    ) -> AppendCommitInput {
        AppendCommitInput {
            commit_id: format!("commit-{command_id}"),
            command_id: command_id.into(),
            base_revision,
            protocol_version: 2,
            payload,
            state_hash: format!("hash-{}", base_revision + 1),
            terminal_result_hash: None,
        }
    }

    #[test]
    fn creates_lobby_and_isolates_members_by_plugin() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("a", "same"), 100).unwrap();
        let mut other = metadata("b", "same");
        other.members.push(RoomMemberInput {
            peer_id: "guest".into(),
            seat_index: 1,
            joined_at: 11,
        });
        storage.save_metadata(other, 101).unwrap();

        let a = storage.load("a", "same").unwrap().unwrap();
        let b = storage.load("b", "same").unwrap().unwrap();
        assert_eq!(a.metadata.admission, RoomAdmission::Lobby);
        assert_eq!(a.metadata.revision, 0);
        assert_eq!(a.members.len(), 1);
        assert_eq!(b.members.len(), 2);
    }

    #[test]
    fn shares_database_path_through_an_independent_configured_connection() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("plugins.sqlite3");
        let plugin_storage = PluginStorage::open(&path).unwrap();
        let room_storage = PluginRoomStorage::open(&path).unwrap();

        plugin_storage
            .storage_set("p", "settings", &serde_json::json!({"sound": true}), 1)
            .unwrap();
        room_storage.save_metadata(metadata("p", "r"), 2).unwrap();

        let connection = room_storage.lock().unwrap();
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        let busy_timeout: i64 = connection
            .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode, "wal");
        assert_eq!(busy_timeout, 5_000);
    }

    #[test]
    fn metadata_room_version_is_monotonic_and_rolls_back_member_errors() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("p", "r"), 100).unwrap();
        let stale = storage.save_metadata(metadata("p", "r"), 101).unwrap_err();
        assert!(stale.contains("roomVersion"));
        let mut duplicate_seat = metadata("p", "r");
        duplicate_seat.room_version = 2;
        duplicate_seat.members.push(RoomMemberInput {
            peer_id: "guest".into(),
            seat_index: 0,
            joined_at: 12,
        });
        assert!(duplicate_seat.members.len() == 2);
        assert!(storage
            .save_metadata(duplicate_seat, 102)
            .unwrap_err()
            .contains("席位"));
        assert_eq!(
            storage
                .load("p", "r")
                .unwrap()
                .unwrap()
                .metadata
                .room_version,
            1
        );

        let mut out_of_range_seat = metadata("p", "r");
        out_of_range_seat.room_version = 2;
        out_of_range_seat.members.push(RoomMemberInput {
            peer_id: "guest".into(),
            seat_index: 4,
            joined_at: 12,
        });
        assert!(storage
            .save_metadata(out_of_range_seat, 103)
            .unwrap_err()
            .contains("maxPlayers"));
        assert_eq!(
            storage
                .load("p", "r")
                .unwrap()
                .unwrap()
                .metadata
                .room_version,
            1
        );
    }

    #[test]
    fn owner_guards_admission_commits_checkpoints_and_close() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("p", "r"), 100).unwrap();
        assert!(storage
            .set_admission("p", "r", "guest", RoomAdmission::Closed, 2, 101)
            .unwrap_err()
            .contains("房主"));
        assert!(storage
            .append_commit(
                "p",
                "r",
                "guest",
                commit("1", 0, serde_json::json!([])),
                102
            )
            .unwrap_err()
            .contains("房主"));
        assert!(storage
            .save_checkpoint(
                "p",
                "r",
                "guest",
                CheckpointInput {
                    revision: 0,
                    protocol_version: 2,
                    payload: serde_json::json!({}),
                    state_hash: "initial".into(),
                    checksum: "sum".into()
                },
                103
            )
            .unwrap_err()
            .contains("房主"));
        assert!(storage
            .close("p", "r", "guest", 104)
            .unwrap_err()
            .contains("房主"));
    }

    #[test]
    fn appends_contiguous_commits_idempotently_and_records_terminal_head() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("p", "r"), 100).unwrap();
        storage
            .set_admission("p", "r", "owner", RoomAdmission::Closed, 2, 101)
            .unwrap();
        let first = storage
            .append_commit(
                "p",
                "r",
                "owner",
                commit("1", 0, serde_json::json!([{"type":"start"}])),
                101,
            )
            .unwrap();
        assert_eq!(first.revision, 1);
        let retry = storage
            .append_commit(
                "p",
                "r",
                "owner",
                commit("1", 0, serde_json::json!([{"ignored":true}])),
                102,
            )
            .unwrap();
        assert_eq!(retry, first);
        let error = storage
            .append_commit(
                "p",
                "r",
                "owner",
                commit("bad", 0, serde_json::json!([])),
                103,
            )
            .unwrap_err();
        assert!(error.contains("baseRevision"));
        let mut terminal = commit("2", 1, serde_json::json!([{"type":"finish"}]));
        terminal.terminal_result_hash = Some("result-hash".into());
        let terminal = storage
            .append_commit("p", "r", "owner", terminal, 104)
            .unwrap();
        assert_eq!(terminal.revision, 2);
        let room = storage.load("p", "r").unwrap().unwrap();
        assert_eq!(
            room.metadata.terminal_commit_id.as_deref(),
            Some("commit-2")
        );
        assert_eq!(room.metadata.terminal_state_hash.as_deref(), Some("hash-2"));
        assert_eq!(room.commits.len(), 2);
    }

    #[test]
    fn rejects_protocol_mismatch_without_writing() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("p", "r"), 100).unwrap();
        let mut wrong = commit("1", 0, serde_json::json!([]));
        wrong.protocol_version = 3;
        assert!(storage
            .append_commit("p", "r", "owner", wrong, 101)
            .unwrap_err()
            .contains("协议版本"));
        assert_eq!(
            storage.load("p", "r").unwrap().unwrap().metadata.revision,
            0
        );
    }

    #[test]
    fn checkpoint_requires_current_revision_and_hash_then_prunes_recovery_prefix() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("p", "r"), 100).unwrap();
        storage
            .append_commit(
                "p",
                "r",
                "owner",
                commit("1", 0, serde_json::json!([1])),
                101,
            )
            .unwrap();
        storage
            .append_commit(
                "p",
                "r",
                "owner",
                commit("2", 1, serde_json::json!([2])),
                102,
            )
            .unwrap();
        let stale = CheckpointInput {
            revision: 1,
            protocol_version: 2,
            payload: serde_json::json!({"state":1}),
            state_hash: "hash-1".into(),
            checksum: "sum-1".into(),
        };
        assert!(storage
            .save_checkpoint("p", "r", "owner", stale, 103)
            .unwrap_err()
            .contains("当前 revision"));
        let wrong_hash = CheckpointInput {
            revision: 2,
            protocol_version: 2,
            payload: serde_json::json!({"state":2}),
            state_hash: "wrong".into(),
            checksum: "sum-2".into(),
        };
        assert!(storage
            .save_checkpoint("p", "r", "owner", wrong_hash, 104)
            .unwrap_err()
            .contains("stateHash"));
        let good = CheckpointInput {
            revision: 2,
            protocol_version: 2,
            payload: serde_json::json!({"state":2}),
            state_hash: "hash-2".into(),
            checksum: "sum-2".into(),
        };
        storage
            .save_checkpoint("p", "r", "owner", good, 105)
            .unwrap();
        storage
            .append_commit(
                "p",
                "r",
                "owner",
                commit("3", 2, serde_json::json!([3])),
                106,
            )
            .unwrap();

        let recovered = storage.recover("p", "r", 2, 0).unwrap();
        assert_eq!(recovered.checkpoint.unwrap().revision, 2);
        assert_eq!(
            recovered
                .commits
                .iter()
                .map(|item| item.revision)
                .collect::<Vec<_>>(),
            vec![3]
        );
    }

    #[test]
    fn initial_checkpoint_and_restart_recovery_work() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("plugins.sqlite3");
        {
            let storage = PluginRoomStorage::open(&path).unwrap();
            storage.save_metadata(metadata("p", "r"), 100).unwrap();
            storage
                .save_checkpoint(
                    "p",
                    "r",
                    "owner",
                    CheckpointInput {
                        revision: 0,
                        protocol_version: 2,
                        payload: serde_json::json!({"initial":true}),
                        state_hash: "initial".into(),
                        checksum: "sum-0".into(),
                    },
                    101,
                )
                .unwrap();
        }
        let reopened = PluginRoomStorage::open(&path).unwrap();
        let recovered = reopened.recover("p", "r", 2, 0).unwrap();
        assert_eq!(recovered.checkpoint.unwrap().state_hash, "initial");
    }

    #[test]
    fn close_is_owner_only_and_idempotent() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("p", "r"), 100).unwrap();
        let first = storage.close("p", "r", "owner", 110).unwrap();
        let second = storage.close("p", "r", "owner", 120).unwrap();
        assert_eq!(first.closed_at, Some(110));
        assert_eq!(second.closed_at, Some(110));
        assert!(storage
            .append_commit(
                "p",
                "r",
                "owner",
                commit("1", 0, serde_json::json!([])),
                121
            )
            .unwrap_err()
            .contains("已关闭"));
    }

    #[test]
    fn enforces_payload_limits_and_rolls_back_quota_failures() {
        let (_temp, storage) = storage();
        storage.save_metadata(metadata("p", "r"), 100).unwrap();
        let oversized_commit = serde_json::Value::String("x".repeat(MAX_COMMIT_PAYLOAD_BYTES));
        assert!(storage
            .append_commit("p", "r", "owner", commit("large", 0, oversized_commit), 101)
            .unwrap_err()
            .contains("256 KiB"));
        assert_eq!(
            storage.load("p", "r").unwrap().unwrap().metadata.revision,
            0
        );
        let oversized_checkpoint = CheckpointInput {
            revision: 0,
            protocol_version: 2,
            payload: serde_json::Value::String("x".repeat(MAX_CHECKPOINT_PAYLOAD_BYTES)),
            state_hash: "initial".into(),
            checksum: "sum".into(),
        };
        assert!(storage
            .save_checkpoint("p", "r", "owner", oversized_checkpoint, 102)
            .unwrap_err()
            .contains("2 MiB"));

        // JSON 字符串首尾各占一个引号，因此减 2 后序列化大小恰好为 256 KiB。
        for index in 0..64 {
            let input = commit(
                &format!("q{index}"),
                index,
                serde_json::Value::String("x".repeat(MAX_COMMIT_PAYLOAD_BYTES - 2)),
            );
            storage
                .append_commit("p", "r", "owner", input, 200 + index)
                .unwrap();
        }
        let before = storage.load("p", "r").unwrap().unwrap().metadata.revision;
        assert_eq!(before, 64, "恰好 16 MiB 的 payload 应被允许");
        let quota = commit("quota", before, serde_json::json!(null));
        assert!(storage
            .append_commit("p", "r", "owner", quota, 999)
            .unwrap_err()
            .contains("16 MiB"));
        assert_eq!(
            storage.load("p", "r").unwrap().unwrap().metadata.revision,
            before
        );

        storage
            .save_metadata(metadata("p", "unicode"), 1_000)
            .unwrap();
        for index in 0..64 {
            let input = commit(
                &format!("u{index}"),
                index,
                serde_json::Value::String("界".repeat(87_380)),
            );
            storage
                .append_commit("p", "unicode", "owner", input, 1_001 + index)
                .unwrap();
        }
        let unicode_quota = commit(
            "unicode-quota",
            64,
            serde_json::Value::String("x".repeat(127)),
        );
        assert!(storage
            .append_commit("p", "unicode", "owner", unicode_quota, 2_000)
            .unwrap_err()
            .contains("16 MiB"));
        assert_eq!(
            storage
                .load("p", "unicode")
                .unwrap()
                .unwrap()
                .metadata
                .revision,
            64
        );
    }
}
