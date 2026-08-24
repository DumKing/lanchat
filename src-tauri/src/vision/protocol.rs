//! 视觉域的版本化局域网策略帧。
//!
//! 本帧只承载策略与执行意图，模型文件仍通过受控下载链路获取，绝不写入
//! mDNS 或普通在线状态广播。接收方先持久化幂等收据，提交事务后才调度后台任务。

use crate::protocol::FaceMonitorPolicyFrame;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const VISION_POLICY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionPolicyFrame {
    pub schema_version: u16,
    pub command_id: String,
    pub nonce: String,
    pub target_device_id: String,
    pub issued_by_device_id: String,
    pub issued_by_nickname: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub revision: i64,
    /// policy_patch | profile_activation | profile_install | runtime_control
    pub operation: String,
    pub payload: serde_json::Value,
}

impl VisionPolicyFrame {
    pub fn new(
        issuer_device_id: impl Into<String>,
        target_device_id: impl Into<String>,
        operation: impl Into<String>,
        revision: i64,
        issued_at: i64,
        expires_at: i64,
    ) -> Self {
        Self {
            schema_version: VISION_POLICY_SCHEMA_VERSION,
            command_id: Uuid::new_v4().to_string(),
            nonce: Uuid::new_v4().to_string(),
            target_device_id: target_device_id.into(),
            issued_by_device_id: issuer_device_id.into(),
            issued_by_nickname: String::new(),
            issued_at,
            expires_at,
            revision,
            operation: operation.into(),
            payload: serde_json::Value::Object(Default::default()),
        }
    }
}

pub fn validate_policy_frame(frame: &VisionPolicyFrame, now: i64) -> Result<(), String> {
    if frame.schema_version != VISION_POLICY_SCHEMA_VERSION {
        return Err("VISION_POLICY_SCHEMA_UNSUPPORTED".to_string());
    }
    if frame.command_id.trim().is_empty()
        || frame.nonce.trim().is_empty()
        || frame.target_device_id.trim().is_empty()
        || frame.issued_by_device_id.trim().is_empty()
        || frame.operation.trim().is_empty()
        || frame.revision <= 0
    {
        return Err("VISION_POLICY_INVALID".to_string());
    }
    if frame.expires_at <= now {
        return Err("VISION_POLICY_EXPIRED".to_string());
    }
    if !frame.payload.is_object() {
        return Err("VISION_POLICY_PAYLOAD_INVALID".to_string());
    }
    Ok(())
}

/// 旧协议只映射既有摄像头阈值和开关，不能伪造 Profile/模型版本等新字段。
pub fn legacy_face_monitor_patch(legacy: &FaceMonitorPolicyFrame) -> VisionPolicyFrame {
    let mut frame = VisionPolicyFrame::new(
        legacy.issued_by_device_id.clone(),
        legacy.target_device_id.clone(),
        "policy_patch",
        legacy.version,
        legacy.issued_at,
        legacy.issued_at.saturating_add(24 * 60 * 60 * 1_000),
    );
    frame.issued_by_nickname = legacy.issued_by_nickname.clone();
    frame.payload = serde_json::json!({
        "faceMinScore": legacy.min_confidence,
        "bodyMinScore": legacy.body_min_confidence,
        "sampleFps": legacy.sample_fps,
        "consecutiveHits": legacy.consecutive_hits,
        "faceCooldownSeconds": legacy.face_cooldown_seconds,
        "bodyCooldownSeconds": legacy.body_cooldown_seconds,
        "settingsLocked": legacy.settings_locked,
    });
    frame
}
