//! 视觉识别域的版本化公共类型。
//!
//! 新运行时尚未接管旧 `face_monitor` 路径前，所有新状态与 DTO 先在本模块
//! 收敛，避免再向旧模块增加无法迁移的公共契约。

use serde::{Deserialize, Serialize};

/// Runtime 生命周期与采样/性能状态正交；不要把用户暂停混入生命周期枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisionLifecycleState {
    Disabled,
    Initializing,
    Ready,
    RebuildingSession,
    RollingBack,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisionSamplingState {
    Running,
    PausedByUser,
    PausedByResourceConflict,
    Starved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VisionPerformanceState {
    Normal,
    Degraded,
    Recovering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisionRuntimeSnapshot {
    pub lifecycle: VisionLifecycleState,
    pub sampling: VisionSamplingState,
    pub performance: VisionPerformanceState,
    pub active_profile_id: Option<String>,
    pub active_profile_version: Option<String>,
    pub revision: u64,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisionModality {
    Face,
    Body,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityDecision {
    ConfirmedFace,
    ConfirmedFusion,
    ProbableBody,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionEvidence {
    pub modality: VisionModality,
    pub embedding_space_id: String,
    pub raw_similarity: f32,
    pub normalized_match_score: f32,
    pub second_best_similarity: Option<f32>,
    pub margin: Option<f32>,
    pub quality_score: f32,
    pub consecutive_hits: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecognitionResult {
    pub stream_id: String,
    pub stream_generation: u64,
    pub frame_id: u64,
    pub track_id: String,
    pub person_id: Option<String>,
    pub decision: IdentityDecision,
    pub evidence: Vec<RecognitionEvidence>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProfileActivationState {
    PendingValidation,
    SmokeTesting,
    Benchmarking,
    AwaitingUserConfirmation,
    RebuildingEmbeddings,
    Paused,
    WarmingUp,
    Switching,
    Active,
    RolledBack,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileActivation {
    pub activation_id: String,
    pub revision: u64,
    pub from_profile_id: Option<String>,
    pub from_profile_version: Option<String>,
    pub to_profile_id: String,
    pub to_profile_version: String,
    pub target_spaces: Vec<String>,
    pub state: ProfileActivationState,
    pub embedding_job_id: Option<String>,
    pub progress: u8,
    pub error_code: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
}

/// 只持久化用户主动暂停。资源争用和饥饿采样都要在重启后重新取得摄像头流。
pub fn restore_sampling_state(
    user_paused: bool,
    _previous_sampling: VisionSamplingState,
) -> VisionSamplingState {
    if user_paused {
        VisionSamplingState::PausedByUser
    } else {
        VisionSamplingState::Running
    }
}
