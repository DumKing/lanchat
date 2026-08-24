use super::activation::{ActivationCoordinator, BackgroundJob, BackgroundJobState};
use super::types::ProfileActivationState;

#[test]
fn cancel_before_switch_cancels_only_its_child_jobs() {
    let mut coordinator = ActivationCoordinator::with_active("baseline", "1.0.0");
    let mut activation = coordinator.begin("balanced", "2.0.0", 100);
    let other_activation = "other-activation".to_string();
    let mut jobs = vec![
        BackgroundJob::new("rebuild", &activation.activation_id),
        BackgroundJob::new("benchmark", &activation.activation_id),
        BackgroundJob::new("other", &other_activation),
    ];

    coordinator
        .cancel(&mut activation, &mut jobs)
        .expect("cancelled");

    assert_eq!(activation.state, ProfileActivationState::Cancelled);
    assert_eq!(jobs[0].state, BackgroundJobState::Cancelled);
    assert_eq!(jobs[1].state, BackgroundJobState::Cancelled);
    assert_eq!(jobs[2].state, BackgroundJobState::Queued);
}

#[test]
fn failed_warmup_keeps_previous_profile_active() {
    let mut coordinator = ActivationCoordinator::with_active("baseline", "1.0.0");
    let mut activation = coordinator.begin("balanced", "2.0.0", 100);
    coordinator.validation_passed(&mut activation).unwrap();
    coordinator.smoke_test_passed(&mut activation).unwrap();
    coordinator
        .benchmark_completed(&mut activation, false)
        .unwrap();
    coordinator.embeddings_rebuilt(&mut activation).unwrap();
    coordinator
        .warmup_failed(&mut activation, "VISION_WARMUP_FAILED")
        .unwrap();

    assert_eq!(activation.state, ProfileActivationState::RolledBack);
    assert_eq!(
        activation.error_code.as_deref(),
        Some("VISION_WARMUP_FAILED")
    );
    assert_eq!(coordinator.active_profile().unwrap().id, "baseline");
}

#[test]
fn switching_is_the_non_cancellable_commit_point() {
    let mut coordinator = ActivationCoordinator::with_active("baseline", "1.0.0");
    let mut activation = coordinator.begin("balanced", "2.0.0", 100);
    coordinator.validation_passed(&mut activation).unwrap();
    coordinator.smoke_test_passed(&mut activation).unwrap();
    coordinator
        .benchmark_completed(&mut activation, false)
        .unwrap();
    coordinator.embeddings_rebuilt(&mut activation).unwrap();
    coordinator.warmup_passed(&mut activation).unwrap();

    assert_eq!(activation.state, ProfileActivationState::Switching);
    assert_eq!(
        coordinator.cancel(&mut activation, &mut []).unwrap_err(),
        "VISION_ACTIVATION_COMMITTING"
    );
    coordinator.commit_switch(&mut activation).unwrap();
    assert_eq!(coordinator.active_profile().unwrap().id, "balanced");
}
