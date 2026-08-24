use super::runtime::VisionRuntimeState;
use super::types::{VisionPerformanceState, VisionSamplingState};
use std::time::Duration;

#[test]
fn user_pause_survives_runtime_restart() {
    let runtime = VisionRuntimeState::default();
    runtime.pause_by_user();
    let restored = VisionRuntimeState::restore(runtime.persisted_state());
    assert_eq!(
        restored.snapshot().sampling,
        VisionSamplingState::PausedByUser
    );
    assert_eq!(
        restored.snapshot().performance,
        VisionPerformanceState::Normal
    );
}

#[test]
fn overload_adapts_sampling_before_switching_model() {
    let runtime = VisionRuntimeState::default();
    runtime.record_processing_duration(Duration::from_millis(320));

    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.performance, VisionPerformanceState::Degraded);
    assert_eq!(
        snapshot.reason_code.as_deref(),
        Some("VISION_LATENCY_OVER_BUDGET")
    );
    assert!(snapshot.active_profile_id.is_none());
}

#[test]
fn three_worker_failures_request_session_recovery() {
    let runtime = VisionRuntimeState::default();
    runtime.record_processing_failure("VISION_INFERENCE_FAILED");
    runtime.record_processing_failure("VISION_INFERENCE_FAILED");
    assert_ne!(
        runtime.snapshot().performance,
        VisionPerformanceState::Recovering
    );

    runtime.record_processing_failure("VISION_INFERENCE_FAILED");
    assert_eq!(
        runtime.snapshot().performance,
        VisionPerformanceState::Recovering
    );
}
