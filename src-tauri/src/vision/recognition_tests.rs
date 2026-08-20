use super::alert::{fuse_track_evidence, AlertDispatch};
use super::matching::{match_identity, ReferenceEmbedding};
use super::tracking::{BoundingBox, Detection, TrackStore};
use super::types::{IdentityDecision, VisionModality};

#[test]
fn same_face_and_body_track_emit_one_upgraded_alert() {
    let mut tracks = TrackStore::new(2_500);
    let face = tracks.observe(
        Detection::new(
            VisionModality::Face,
            BoundingBox::new(0.20, 0.20, 0.20, 0.20),
        ),
        100,
    );
    let body = tracks.observe(
        Detection::new(
            VisionModality::Body,
            BoundingBox::new(0.16, 0.12, 0.34, 0.60),
        ),
        200,
    );

    assert_eq!(face, body);
    let fused = fuse_track_evidence(Some(("alice", 92)), Some(("alice", 86)));
    assert_eq!(fused.decision, IdentityDecision::ConfirmedFusion);
    assert_eq!(fused.dispatch, AlertDispatch::LanAndLocal);
}

#[test]
fn prototype_and_top_k_agree_before_identity_is_confirmed() {
    let references = vec![
        ReferenceEmbedding::new("alice", vec![1.0, 0.0], 0.9),
        ReferenceEmbedding::new("alice", vec![0.98, 0.04], 0.8),
        ReferenceEmbedding::new("alice", vec![0.97, -0.02], 0.8),
        ReferenceEmbedding::new("bob", vec![0.50, 0.50], 1.0),
    ];
    let matched = match_identity(&[1.0, 0.0], &references, 2, 70.0, 8.0).expect("confirmed");

    assert_eq!(matched.person_id, "alice");
    assert!(matched.raw_similarity > 0.9);
    assert!(matched.normalized_match_score >= 70.0);
}

#[test]
fn low_body_score_stays_local_only() {
    let fused = fuse_track_evidence(None, Some(("alice", 64)));
    assert_eq!(fused.decision, IdentityDecision::ProbableBody);
    assert_eq!(fused.dispatch, AlertDispatch::LocalOnly);
}

#[test]
fn expired_tracks_are_not_reused_after_stream_gap() {
    let mut tracks = TrackStore::new(2_500);
    let first = tracks.observe(
        Detection::new(
            VisionModality::Face,
            BoundingBox::new(0.20, 0.20, 0.20, 0.20),
        ),
        100,
    );
    let next = tracks.observe(
        Detection::new(
            VisionModality::Face,
            BoundingBox::new(0.20, 0.20, 0.20, 0.20),
        ),
        2_700,
    );
    assert_ne!(first, next);
}
