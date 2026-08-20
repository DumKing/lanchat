//! 视觉运行时快照与 Profile 切换前的内存状态。

use super::types::{
    restore_sampling_state, VisionLifecycleState, VisionPerformanceState, VisionRuntimeSnapshot,
    VisionSamplingState,
};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct PersistedVisionRuntimeState {
    pub user_paused: bool,
    pub sampling: VisionSamplingState,
    pub revision: u64,
}

pub struct VisionRuntimeState {
    snapshot: Mutex<VisionRuntimeSnapshot>,
    user_paused: Mutex<bool>,
}

impl Default for VisionRuntimeState {
    fn default() -> Self {
        Self {
            snapshot: Mutex::new(VisionRuntimeSnapshot {
                lifecycle: VisionLifecycleState::Disabled,
                sampling: VisionSamplingState::Running,
                performance: VisionPerformanceState::Normal,
                active_profile_id: None,
                active_profile_version: None,
                revision: 0,
                reason_code: None,
            }),
            user_paused: Mutex::new(false),
        }
    }
}

impl VisionRuntimeState {
    pub fn restore(persisted: PersistedVisionRuntimeState) -> Self {
        let runtime = Self::default();
        {
            let mut snapshot = runtime.snapshot.lock().expect("vision runtime lock poisoned");
            snapshot.sampling = restore_sampling_state(persisted.user_paused, persisted.sampling);
            snapshot.revision = persisted.revision;
        }
        *runtime.user_paused.lock().expect("vision runtime lock poisoned") = persisted.user_paused;
        runtime
    }

    pub fn pause_by_user(&self) {
        *self.user_paused.lock().expect("vision runtime lock poisoned") = true;
        let mut snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        snapshot.sampling = VisionSamplingState::PausedByUser;
        snapshot.revision += 1;
    }

    pub fn persisted_state(&self) -> PersistedVisionRuntimeState {
        let snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        PersistedVisionRuntimeState {
            user_paused: *self.user_paused.lock().expect("vision runtime lock poisoned"),
            sampling: snapshot.sampling,
            revision: snapshot.revision,
        }
    }

    pub fn snapshot(&self) -> VisionRuntimeSnapshot {
        self.snapshot.lock().expect("vision runtime lock poisoned").clone()
    }
}
