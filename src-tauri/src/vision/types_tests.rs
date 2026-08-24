use super::types::{restore_sampling_state, VisionSamplingState};

#[test]
fn user_pause_survives_restart_but_resource_pause_does_not() {
    assert_eq!(
        restore_sampling_state(true, VisionSamplingState::Running),
        VisionSamplingState::PausedByUser
    );
    assert_eq!(
        restore_sampling_state(false, VisionSamplingState::Starved),
        VisionSamplingState::Running
    );
}
