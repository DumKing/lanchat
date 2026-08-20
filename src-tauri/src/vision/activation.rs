//! 可恢复的模型 Profile 激活状态机。
//!
//! 激活工作在模型加载、特征重算和预热完成前不修改当前 Profile。只有
//! `Switching` 是不可取消的提交点，故预热失败可无损回滚到 LKG/当前模型。

use crate::vision::types::{ProfileActivation, ProfileActivationState};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveProfile {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundJobState {
    Queued,
    Running,
    Finished,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundJob {
    pub job_id: String,
    pub activation_id: String,
    pub state: BackgroundJobState,
}

impl BackgroundJob {
    pub fn new(job_id: impl Into<String>, activation_id: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            activation_id: activation_id.into(),
            state: BackgroundJobState::Queued,
        }
    }
}

pub struct ActivationCoordinator {
    active_profile: Option<ActiveProfile>,
    revision: u64,
}

impl ActivationCoordinator {
    pub fn with_active(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            active_profile: Some(ActiveProfile {
                id: id.into(),
                version: version.into(),
            }),
            revision: 0,
        }
    }

    pub fn active_profile(&self) -> Option<&ActiveProfile> {
        self.active_profile.as_ref()
    }

    pub fn begin(
        &mut self,
        to_profile_id: impl Into<String>,
        to_profile_version: impl Into<String>,
        now: i64,
    ) -> ProfileActivation {
        self.revision += 1;
        let from = self.active_profile.as_ref();
        ProfileActivation {
            activation_id: Uuid::new_v4().to_string(),
            revision: self.revision,
            from_profile_id: from.map(|profile| profile.id.clone()),
            from_profile_version: from.map(|profile| profile.version.clone()),
            to_profile_id: to_profile_id.into(),
            to_profile_version: to_profile_version.into(),
            target_spaces: Vec::new(),
            state: ProfileActivationState::PendingValidation,
            embedding_job_id: None,
            progress: 0,
            error_code: None,
            started_at: now,
            updated_at: now,
        }
    }

    pub fn validation_passed(&self, activation: &mut ProfileActivation) -> Result<(), String> {
        transition(
            activation,
            ProfileActivationState::PendingValidation,
            ProfileActivationState::SmokeTesting,
            10,
        )
    }

    pub fn smoke_test_passed(&self, activation: &mut ProfileActivation) -> Result<(), String> {
        transition(
            activation,
            ProfileActivationState::SmokeTesting,
            ProfileActivationState::Benchmarking,
            25,
        )
    }

    pub fn benchmark_completed(
        &self,
        activation: &mut ProfileActivation,
        requires_confirmation: bool,
    ) -> Result<(), String> {
        transition(
            activation,
            ProfileActivationState::Benchmarking,
            if requires_confirmation {
                ProfileActivationState::AwaitingUserConfirmation
            } else {
                ProfileActivationState::RebuildingEmbeddings
            },
            40,
        )
    }

    pub fn confirm(&self, activation: &mut ProfileActivation) -> Result<(), String> {
        transition(
            activation,
            ProfileActivationState::AwaitingUserConfirmation,
            ProfileActivationState::RebuildingEmbeddings,
            40,
        )
    }

    pub fn embeddings_rebuilt(&self, activation: &mut ProfileActivation) -> Result<(), String> {
        transition(
            activation,
            ProfileActivationState::RebuildingEmbeddings,
            ProfileActivationState::WarmingUp,
            85,
        )
    }

    pub fn warmup_passed(&self, activation: &mut ProfileActivation) -> Result<(), String> {
        transition(
            activation,
            ProfileActivationState::WarmingUp,
            ProfileActivationState::Switching,
            95,
        )
    }

    pub fn warmup_failed(
        &self,
        activation: &mut ProfileActivation,
        error_code: impl Into<String>,
    ) -> Result<(), String> {
        if activation.state != ProfileActivationState::WarmingUp {
            return Err("VISION_ACTIVATION_STATE_INVALID".to_string());
        }
        activation.state = ProfileActivationState::RolledBack;
        activation.error_code = Some(error_code.into());
        activation.updated_at += 1;
        Ok(())
    }

    pub fn commit_switch(&mut self, activation: &mut ProfileActivation) -> Result<(), String> {
        if activation.state != ProfileActivationState::Switching {
            return Err("VISION_ACTIVATION_STATE_INVALID".to_string());
        }
        self.active_profile = Some(ActiveProfile {
            id: activation.to_profile_id.clone(),
            version: activation.to_profile_version.clone(),
        });
        activation.state = ProfileActivationState::Active;
        activation.progress = 100;
        activation.updated_at += 1;
        Ok(())
    }

    pub fn cancel(
        &self,
        activation: &mut ProfileActivation,
        jobs: &mut [BackgroundJob],
    ) -> Result<(), String> {
        if matches!(
            activation.state,
            ProfileActivationState::Switching | ProfileActivationState::Active
        ) {
            return Err("VISION_ACTIVATION_COMMITTING".to_string());
        }
        if matches!(
            activation.state,
            ProfileActivationState::RolledBack
                | ProfileActivationState::Failed
                | ProfileActivationState::Cancelled
        ) {
            return Err("VISION_ACTIVATION_NOT_CANCELLABLE".to_string());
        }
        activation.state = ProfileActivationState::Cancelled;
        activation.updated_at += 1;
        for job in jobs
            .iter_mut()
            .filter(|job| job.activation_id == activation.activation_id)
        {
            if matches!(
                job.state,
                BackgroundJobState::Queued | BackgroundJobState::Running
            ) {
                job.state = BackgroundJobState::Cancelled;
            }
        }
        Ok(())
    }
}

fn transition(
    activation: &mut ProfileActivation,
    expected: ProfileActivationState,
    next: ProfileActivationState,
    progress: u8,
) -> Result<(), String> {
    if activation.state != expected {
        return Err("VISION_ACTIVATION_STATE_INVALID".to_string());
    }
    activation.state = next;
    activation.progress = progress;
    activation.updated_at += 1;
    Ok(())
}
