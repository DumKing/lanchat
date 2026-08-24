//! 视觉运行时快照与 Profile 切换前的内存状态。

use super::types::{
    restore_sampling_state, VisionLifecycleState, VisionPerformanceState, VisionRuntimeDiagnostics,
    VisionRuntimeSnapshot, VisionSamplingState,
};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PersistedVisionRuntimeState {
    pub user_paused: bool,
    pub snapshot: VisionRuntimeSnapshot,
}

pub struct VisionRuntimeState {
    snapshot: Mutex<VisionRuntimeSnapshot>,
    user_paused: Mutex<bool>,
    diagnostics: Mutex<RuntimeDiagnosticsState>,
}

#[derive(Default)]
struct RuntimeDiagnosticsState {
    accepted_frames: u64,
    dropped_frames: u64,
    processed_frames: u64,
    worker_queue_depth: u8,
    estimated_memory_bytes: u64,
    stream_resets: u64,
    recent_processing_ms: Vec<u64>,
    consecutive_failures: u8,
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
            diagnostics: Mutex::new(RuntimeDiagnosticsState::default()),
        }
    }
}

impl VisionRuntimeState {
    pub fn restore(persisted: PersistedVisionRuntimeState) -> Self {
        let runtime = Self {
            snapshot: Mutex::new(persisted.snapshot),
            user_paused: Mutex::new(persisted.user_paused),
            diagnostics: Mutex::new(RuntimeDiagnosticsState::default()),
        };
        {
            let mut snapshot = runtime
                .snapshot
                .lock()
                .expect("vision runtime lock poisoned");
            snapshot.sampling = restore_sampling_state(persisted.user_paused, snapshot.sampling);
            // 资源争用、推理重建等临时状态不能跨重启延续；模型 Profile 本身保留。
            if !matches!(snapshot.lifecycle, VisionLifecycleState::Disabled) {
                snapshot.lifecycle = VisionLifecycleState::Initializing;
            }
            snapshot.performance = VisionPerformanceState::Normal;
            snapshot.reason_code = None;
        }
        runtime
    }

    pub fn pause_by_user(&self) {
        *self
            .user_paused
            .lock()
            .expect("vision runtime lock poisoned") = true;
        let mut snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        snapshot.sampling = VisionSamplingState::PausedByUser;
        snapshot.revision += 1;
    }

    pub fn resume_by_user(&self) {
        *self
            .user_paused
            .lock()
            .expect("vision runtime lock poisoned") = false;
        let mut snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        snapshot.sampling = VisionSamplingState::Running;
        snapshot.revision += 1;
    }

    pub fn mark_model_availability(&self, model_ready: bool) {
        let mut snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        snapshot.lifecycle = if model_ready {
            VisionLifecycleState::Ready
        } else {
            VisionLifecycleState::Disabled
        };
        if !model_ready {
            snapshot.reason_code = Some("VISION_MODEL_UNAVAILABLE".to_string());
        } else if snapshot.reason_code.as_deref() == Some("VISION_MODEL_UNAVAILABLE") {
            snapshot.reason_code = None;
        }
    }

    pub fn set_active_profile(&self, id: impl Into<String>, version: impl Into<String>) {
        let mut snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        let id = id.into();
        let version = version.into();
        if snapshot.active_profile_id.as_deref() != Some(id.as_str())
            || snapshot.active_profile_version.as_deref() != Some(version.as_str())
        {
            snapshot.active_profile_id = Some(id);
            snapshot.active_profile_version = Some(version);
            snapshot.revision += 1;
        }
    }

    pub fn persisted_state(&self) -> PersistedVisionRuntimeState {
        let snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        PersistedVisionRuntimeState {
            user_paused: *self
                .user_paused
                .lock()
                .expect("vision runtime lock poisoned"),
            snapshot: snapshot.clone(),
        }
    }

    pub fn snapshot(&self) -> VisionRuntimeSnapshot {
        self.snapshot
            .lock()
            .expect("vision runtime lock poisoned")
            .clone()
    }

    pub fn record_frame_accepted(
        &self,
        dropped_frames: u64,
        queue_depth: u8,
        memory_bytes: u64,
        stream_resets: u64,
    ) {
        let mut diagnostics = self
            .diagnostics
            .lock()
            .expect("vision diagnostics lock poisoned");
        diagnostics.accepted_frames += 1;
        diagnostics.dropped_frames = dropped_frames;
        diagnostics.worker_queue_depth = queue_depth;
        diagnostics.estimated_memory_bytes = memory_bytes;
        diagnostics.stream_resets = stream_resets;
    }

    pub fn record_processing_duration(&self, elapsed: Duration) {
        let millis = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let mut diagnostics = self
            .diagnostics
            .lock()
            .expect("vision diagnostics lock poisoned");
        diagnostics.processed_frames += 1;
        diagnostics.consecutive_failures = 0;
        diagnostics.recent_processing_ms.push(millis);
        if diagnostics.recent_processing_ms.len() > 20 {
            diagnostics.recent_processing_ms.remove(0);
        }
        let p95 = percentile(&diagnostics.recent_processing_ms, 0.95);
        drop(diagnostics);

        let mut snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        if snapshot.lifecycle != VisionLifecycleState::Disabled {
            snapshot.lifecycle = VisionLifecycleState::Ready;
        }
        if snapshot.sampling != VisionSamplingState::PausedByUser {
            snapshot.sampling = VisionSamplingState::Running;
        }
        if p95 > 300 {
            snapshot.performance = VisionPerformanceState::Degraded;
            snapshot.reason_code = Some("VISION_LATENCY_OVER_BUDGET".to_string());
        } else if snapshot.performance == VisionPerformanceState::Degraded
            && snapshot.reason_code.as_deref() == Some("VISION_LATENCY_OVER_BUDGET")
        {
            snapshot.performance = VisionPerformanceState::Normal;
            snapshot.reason_code = None;
        }
    }

    pub fn record_processing_failure(&self, reason_code: &str) {
        let mut diagnostics = self
            .diagnostics
            .lock()
            .expect("vision diagnostics lock poisoned");
        diagnostics.consecutive_failures = diagnostics.consecutive_failures.saturating_add(1);
        if diagnostics.consecutive_failures < 3 {
            return;
        }
        drop(diagnostics);
        let mut snapshot = self.snapshot.lock().expect("vision runtime lock poisoned");
        snapshot.lifecycle = VisionLifecycleState::RebuildingSession;
        snapshot.performance = VisionPerformanceState::Recovering;
        snapshot.reason_code = Some(reason_code.to_string());
        snapshot.revision += 1;
    }

    pub fn diagnostics(&self) -> VisionRuntimeDiagnostics {
        let diagnostics = self
            .diagnostics
            .lock()
            .expect("vision diagnostics lock poisoned");
        VisionRuntimeDiagnostics {
            accepted_frames: diagnostics.accepted_frames,
            dropped_frames: diagnostics.dropped_frames,
            processed_frames: diagnostics.processed_frames,
            p50_processing_ms: percentile(&diagnostics.recent_processing_ms, 0.50),
            p95_processing_ms: percentile(&diagnostics.recent_processing_ms, 0.95),
            estimated_memory_bytes: diagnostics.estimated_memory_bytes,
            worker_queue_depth: diagnostics.worker_queue_depth,
            stream_resets: diagnostics.stream_resets,
        }
    }
}

fn percentile(values: &[u64], percentile: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) as f64 * percentile).ceil() as usize;
    sorted[index]
}
