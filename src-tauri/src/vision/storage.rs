//! 视觉域的 SQLite V5 迁移与持久化接口。

use rusqlite::Connection;

pub const VISION_SCHEMA_VERSION: i64 = 5;

/// 只追加视觉域表。旧 face_people、face_person_samples 与告警表保留给兼容读取层。
pub fn initialize(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS vision_schema_migrations (
            version INTEGER PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS person_reference_images_v2 (
            id TEXT PRIMARY KEY,
            person_id TEXT NOT NULL,
            file_path TEXT NOT NULL,
            sha256 TEXT NOT NULL,
            quality_score REAL,
            face_quality_score REAL,
            body_quality_score REAL,
            face_usage_enabled INTEGER NOT NULL DEFAULT 1,
            body_usage_enabled INTEGER NOT NULL DEFAULT 1,
            body_sample_kind TEXT,
            body_weight REAL NOT NULL DEFAULT 1.0,
            body_weight_decay_at INTEGER,
            body_expires_at INTEGER,
            detected_subject_count INTEGER NOT NULL DEFAULT 1,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(person_id, sha256)
        );
        CREATE TABLE IF NOT EXISTS vision_model_profiles (
            profile_id TEXT NOT NULL,
            profile_version TEXT NOT NULL,
            display_name TEXT NOT NULL,
            tier TEXT NOT NULL,
            manifest_json TEXT NOT NULL,
            source_kind TEXT NOT NULL,
            install_state TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 0,
            is_last_known_good INTEGER NOT NULL DEFAULT 0,
            installed_at INTEGER,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY(profile_id, profile_version)
        );
        CREATE TABLE IF NOT EXISTS vision_background_jobs (
            job_id TEXT PRIMARY KEY,
            activation_id TEXT,
            revision INTEGER NOT NULL DEFAULT 0,
            job_kind TEXT NOT NULL,
            state TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            cursor_json TEXT,
            completed_items INTEGER NOT NULL DEFAULT 0,
            total_items INTEGER NOT NULL DEFAULT 0,
            error_code TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS vision_profile_activations (
            activation_id TEXT PRIMARY KEY,
            revision INTEGER NOT NULL DEFAULT 0,
            to_profile_id TEXT NOT NULL,
            to_profile_version TEXT NOT NULL,
            state TEXT NOT NULL,
            embedding_job_id TEXT,
            progress INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS vision_runtime_state (
            singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
            revision INTEGER NOT NULL,
            active_profile_id TEXT,
            active_profile_version TEXT,
            lifecycle TEXT NOT NULL,
            sampling_state TEXT NOT NULL,
            performance_state TEXT NOT NULL,
            user_paused INTEGER NOT NULL DEFAULT 0,
            consecutive_failure_count INTEGER NOT NULL DEFAULT 0,
            last_error_code TEXT,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS vision_remote_command_state (
            issuer_device_id TEXT NOT NULL,
            target_device_id TEXT NOT NULL,
            highest_revision INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY(issuer_device_id, target_device_id)
        );
        CREATE TABLE IF NOT EXISTS vision_remote_command_nonces (
            nonce TEXT PRIMARY KEY,
            command_id TEXT NOT NULL,
            issuer_device_id TEXT NOT NULL,
            target_device_id TEXT NOT NULL,
            expires_at INTEGER NOT NULL,
            result_json TEXT NOT NULL,
            processed_at INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS uq_vision_remote_command_target
            ON vision_remote_command_nonces(issuer_device_id, target_device_id, command_id);
        CREATE INDEX IF NOT EXISTS idx_vision_remote_nonce_expiry
            ON vision_remote_command_nonces(expires_at);
        ",
    )
    .map_err(|error| format!("初始化视觉识别数据库失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO vision_schema_migrations(version, checksum, applied_at) VALUES (?1, ?2, strftime('%s','now'))",
        (VISION_SCHEMA_VERSION, "vision-v5-initial"),
    )
    .map_err(|error| format!("记录视觉识别数据库版本失败：{error}"))?;
    Ok(())
}

/// 从旧 `face_person_samples` 复制，而不是移动，确保迁移中断时旧运行时仍可工作。
pub fn copy_legacy_reference_images(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO person_reference_images_v2(
            id,person_id,file_path,sha256,created_at,updated_at
         )
         SELECT
            'legacy:' || sample_id,
            person_id,
            photo_url,
            COALESCE(NULLIF(photo_sha256, ''), photo_url),
            created_at,
            created_at
         FROM face_person_samples",
        [],
    )
    .map_err(|error| format!("复制旧视觉参考图失败：{error}"))?;
    Ok(())
}
