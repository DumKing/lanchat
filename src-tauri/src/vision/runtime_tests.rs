use super::runtime::VisionRuntimeState;
use super::types::{VisionPerformanceState, VisionSamplingState};

#[test]
fn user_pause_survives_runtime_restart() {
    let runtime = VisionRuntimeState::default();
    runtime.pause_by_user();
    let restored = VisionRuntimeState::restore(runtime.persisted_state());
    assert_eq!(restored.snapshot().sampling, VisionSamplingState::PausedByUser);
    assert_eq!(restored.snapshot().performance, VisionPerformanceState::Normal);
}
