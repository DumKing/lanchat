//! 视觉域的 SQLite V5 迁移与持久化接口。
//!
//! V5 只追加数据，不会修改或删除旧 `face_*` 表。这样在升级中断或新运行时
//! 回滚时，旧识别路径仍然可以读取完整历史数据。

use rusqlite::{backup::Backup, Connection};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const VISION_SCHEMA_VERSION: i64 = 5;

/// 只追加视觉域表。旧 face_people、face_person_samples 与告警表保留给兼容读取层。
pub fn initialize(conn: &Connection) -> Result<(), String> {
    maybe_backup_legacy_database(conn)?;
    conn.execute_batch("PRAGMA foreign_keys = ON; BEGIN IMMEDIATE;")
        .map_err(|error| format!("开始视觉识别数据库迁移失败：{error}"))?;

    let result = (|| {
        conn.execute_batch(
            "
        CREATE TABLE IF NOT EXISTS vision_schema_migrations (
            version INTEGER PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS vision_people_v2 (
            person_id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            expires_at INTEGER,
            version INTEGER NOT NULL,
            issued_by_device_id TEXT NOT NULL,
            issued_by_nickname TEXT NOT NULL,
            issued_at INTEGER NOT NULL,
            deleted_at INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
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
            UNIQUE(person_id, sha256),
            FOREIGN KEY(person_id) REFERENCES vision_people_v2(person_id)
        );
        CREATE TABLE IF NOT EXISTS vision_embedding_spaces (
            embedding_space_id TEXT PRIMARY KEY,
            profile_id TEXT NOT NULL,
            profile_version TEXT NOT NULL,
            modality TEXT NOT NULL,
            semantics_json TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS person_embeddings_v2 (
            embedding_id TEXT PRIMARY KEY,
            person_id TEXT NOT NULL,
            reference_image_id TEXT,
            embedding_space_id TEXT NOT NULL,
            modality TEXT NOT NULL,
            feature_role TEXT NOT NULL DEFAULT 'reference',
            vector_blob BLOB NOT NULL,
            quality_score REAL,
            source_kind TEXT NOT NULL DEFAULT 'legacy',
            state TEXT NOT NULL DEFAULT 'ready',
            valid_until INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            FOREIGN KEY(person_id) REFERENCES vision_people_v2(person_id),
            FOREIGN KEY(reference_image_id) REFERENCES person_reference_images_v2(id),
            FOREIGN KEY(embedding_space_id) REFERENCES vision_embedding_spaces(embedding_space_id)
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
        CREATE TABLE IF NOT EXISTS vision_alert_events (
            alert_id TEXT PRIMARY KEY,
            source_kind TEXT NOT NULL,
            source_device_id TEXT NOT NULL,
            source_nickname TEXT NOT NULL,
            source_address TEXT,
            person_id TEXT,
            person_name_snapshot TEXT,
            decision TEXT NOT NULL,
            match_score INTEGER NOT NULL,
            face_score INTEGER,
            body_score INTEGER,
            consecutive_hits INTEGER NOT NULL DEFAULT 1,
            policy_version INTEGER NOT NULL DEFAULT 0,
            legacy_source_alert_id TEXT UNIQUE,
            created_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS vision_alert_feedbacks (
            alert_id TEXT NOT NULL,
            responder_device_id TEXT NOT NULL,
            responder_nickname TEXT NOT NULL,
            result TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY(alert_id, responder_device_id),
            FOREIGN KEY(alert_id) REFERENCES vision_alert_events(alert_id)
        );
        CREATE UNIQUE INDEX IF NOT EXISTS uq_vision_remote_command_target
            ON vision_remote_command_nonces(issuer_device_id, target_device_id, command_id);
        CREATE INDEX IF NOT EXISTS idx_vision_remote_nonce_expiry
            ON vision_remote_command_nonces(expires_at);
        CREATE INDEX IF NOT EXISTS idx_vision_reference_images_person
            ON person_reference_images_v2(person_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_vision_embeddings_person_space
            ON person_embeddings_v2(person_id, embedding_space_id, modality);
        CREATE INDEX IF NOT EXISTS idx_vision_alert_events_created
            ON vision_alert_events(created_at DESC);
        ",
        )
        .map_err(|error| format!("初始化视觉识别数据库失败：{error}"))?;
        copy_legacy_people(conn)?;
        copy_legacy_reference_images(conn)?;
        copy_legacy_embeddings(conn)?;
        copy_legacy_alert_history(conn)?;
        verify_integrity(conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO vision_schema_migrations(version, checksum, applied_at) VALUES (?1, ?2, strftime('%s','now'))",
            (VISION_SCHEMA_VERSION, "vision-v5-initial"),
        )
        .map_err(|error| format!("记录视觉识别数据库版本失败：{error}"))?;
        Ok(())
    })();

    match result {
        Ok(()) => conn
            .execute_batch("COMMIT")
            .map_err(|error| format!("提交视觉识别数据库迁移失败：{error}")),
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
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

pub fn copy_legacy_people(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO vision_people_v2(
            person_id,display_name,enabled,expires_at,version,issued_by_device_id,
            issued_by_nickname,issued_at,deleted_at,created_at,updated_at
         )
         SELECT person_id,display_name,enabled,expires_at,version,issued_by_device_id,
                issued_by_nickname,issued_at,deleted_at,issued_at,issued_at
         FROM face_people",
        [],
    )
    .map_err(|error| format!("复制旧视觉人员失败：{error}"))?;
    Ok(())
}

fn copy_legacy_embeddings(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO vision_embedding_spaces(
            embedding_space_id,profile_id,profile_version,modality,semantics_json,created_at
         )
         SELECT DISTINCT
            'legacy:face:' || COALESCE(NULLIF(embedding_model_version, ''), 'unknown'),
            'legacy',
            COALESCE(NULLIF(embedding_model_version, ''), 'unknown'),
            'face',
            '{\"kind\":\"legacy\",\"modality\":\"face\"}',
            strftime('%s','now')
         FROM face_person_samples
         WHERE embedding IS NOT NULL",
        [],
    )
    .map_err(|error| format!("创建旧人脸特征空间失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO vision_embedding_spaces(
            embedding_space_id,profile_id,profile_version,modality,semantics_json,created_at
         )
         SELECT DISTINCT
            'legacy:body:' || COALESCE(NULLIF(body_embedding_model_version, ''), 'unknown'),
            'legacy',
            COALESCE(NULLIF(body_embedding_model_version, ''), 'unknown'),
            'body',
            '{\"kind\":\"legacy\",\"modality\":\"body\"}',
            strftime('%s','now')
         FROM face_person_samples
         WHERE body_embedding IS NOT NULL",
        [],
    )
    .map_err(|error| format!("创建旧人体特征空间失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO vision_embedding_spaces(
            embedding_space_id,profile_id,profile_version,modality,semantics_json,created_at
         )
         SELECT DISTINCT
            'legacy:face:' || COALESCE(NULLIF(embedding_model_version, ''), 'unknown'),
            'legacy',
            COALESCE(NULLIF(embedding_model_version, ''), 'unknown'),
            'face',
            '{\"kind\":\"legacy\",\"modality\":\"face\"}',
            strftime('%s','now')
         FROM face_people
         WHERE embedding IS NOT NULL",
        [],
    )
    .map_err(|error| format!("创建旧人员原型特征空间失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO person_embeddings_v2(
            embedding_id,person_id,reference_image_id,embedding_space_id,modality,feature_role,
            vector_blob,source_kind,state,created_at,updated_at
         )
         SELECT 'legacy:face:' || sample_id,person_id,'legacy:' || sample_id,
                'legacy:face:' || COALESCE(NULLIF(embedding_model_version, ''), 'unknown'),
                'face','reference',embedding,'legacy','ready',created_at,created_at
         FROM face_person_samples WHERE embedding IS NOT NULL",
        [],
    )
    .map_err(|error| format!("复制旧人脸特征失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO person_embeddings_v2(
            embedding_id,person_id,reference_image_id,embedding_space_id,modality,feature_role,
            vector_blob,source_kind,state,created_at,updated_at
         )
         SELECT 'legacy:body:' || sample_id,person_id,'legacy:' || sample_id,
                'legacy:body:' || COALESCE(NULLIF(body_embedding_model_version, ''), 'unknown'),
                'body','reference',body_embedding,'legacy','ready',created_at,created_at
         FROM face_person_samples WHERE body_embedding IS NOT NULL",
        [],
    )
    .map_err(|error| format!("复制旧人体特征失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO person_embeddings_v2(
            embedding_id,person_id,reference_image_id,embedding_space_id,modality,feature_role,
            vector_blob,source_kind,state,created_at,updated_at
         )
         SELECT 'legacy:prototype:' || person_id,person_id,NULL,
                'legacy:face:' || COALESCE(NULLIF(embedding_model_version, ''), 'unknown'),
                'face','person_prototype',embedding,'legacy','ready',issued_at,issued_at
         FROM face_people WHERE embedding IS NOT NULL",
        [],
    )
    .map_err(|error| format!("复制旧人员原型特征失败：{error}"))?;
    Ok(())
}

fn copy_legacy_alert_history(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO vision_alert_events(
            alert_id,source_kind,source_device_id,source_nickname,source_address,person_id,
            person_name_snapshot,decision,match_score,face_score,body_score,consecutive_hits,
            policy_version,legacy_source_alert_id,created_at
         )
         SELECT alert_id,source_kind,source_device_id,source_nickname,source_address,person_id,
                person_name,recognition_level,confidence,face_confidence,body_confidence,
                consecutive_hits,policy_version,alert_id,created_at
         FROM camera_face_alerts",
        [],
    )
    .map_err(|error| format!("复制旧视觉告警失败：{error}"))?;
    conn.execute(
        "INSERT OR IGNORE INTO vision_alert_feedbacks(
            alert_id,responder_device_id,responder_nickname,result,created_at
         )
         SELECT alert_id,responder_device_id,responder_nickname,result,created_at
         FROM camera_face_alert_feedbacks",
        [],
    )
    .map_err(|error| format!("复制旧视觉告警反馈失败：{error}"))?;
    Ok(())
}

pub fn verify_integrity(conn: &Connection) -> Result<(), String> {
    let integrity = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map_err(|error| format!("检查视觉识别数据库完整性失败：{error}"))?;
    if integrity != "ok" {
        return Err(format!("视觉识别数据库完整性校验失败：{integrity}"));
    }
    let mut statement = conn
        .prepare("PRAGMA foreign_key_check")
        .map_err(|error| format!("检查视觉识别外键失败：{error}"))?;
    let has_violation = statement
        .query([])
        .map_err(|error| format!("读取视觉识别外键检查失败：{error}"))?
        .next()
        .map_err(|error| format!("读取视觉识别外键检查失败：{error}"))?
        .is_some();
    if has_violation {
        return Err("视觉识别数据库外键校验失败".to_string());
    }
    Ok(())
}

fn maybe_backup_legacy_database(conn: &Connection) -> Result<(), String> {
    if vision_schema_exists(conn)? || !legacy_data_exists(conn)? {
        return Ok(());
    }
    let Some(source_path) = main_database_path(conn)? else {
        return Ok(());
    };
    let backup_path = backup_path_for(&source_path);
    if backup_path.exists() {
        return Ok(());
    }
    let mut backup_conn = Connection::open(&backup_path)
        .map_err(|error| format!("创建视觉识别迁移备份失败：{error}"))?;
    let backup = Backup::new(conn, &mut backup_conn)
        .map_err(|error| format!("初始化视觉识别在线备份失败：{error}"))?;
    backup
        .run_to_completion(32, Duration::from_millis(5), None)
        .map_err(|error| format!("执行视觉识别在线备份失败：{error}"))
}

fn vision_schema_exists(conn: &Connection) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='vision_schema_migrations')",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|error| format!("检查视觉识别迁移状态失败：{error}"))
}

fn legacy_data_exists(conn: &Connection) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM face_people LIMIT 1)
         OR EXISTS(SELECT 1 FROM face_person_samples LIMIT 1)
         OR EXISTS(SELECT 1 FROM camera_face_alerts LIMIT 1)",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(|error| format!("检查旧视觉数据失败：{error}"))
}

fn main_database_path(conn: &Connection) -> Result<Option<PathBuf>, String> {
    let mut statement = conn
        .prepare("PRAGMA database_list")
        .map_err(|error| format!("读取视觉识别数据库路径失败：{error}"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| format!("读取视觉识别数据库路径失败：{error}"))?;
    while let Some(row) = rows
        .next()
        .map_err(|error| format!("读取视觉识别数据库路径失败：{error}"))?
    {
        let name: String = row
            .get(1)
            .map_err(|error| format!("读取视觉识别数据库路径失败：{error}"))?;
        if name == "main" {
            let path: String = row
                .get(2)
                .map_err(|error| format!("读取视觉识别数据库路径失败：{error}"))?;
            return Ok((!path.is_empty()).then(|| PathBuf::from(path)));
        }
    }
    Ok(None)
}

fn backup_path_for(source_path: &Path) -> PathBuf {
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("lanchat");
    source_path.with_file_name(format!("{stem}.vision-v5-backup.sqlite3"))
}
