use crate::protocol::{CameraFaceAlertFrame, FacePersonPolicyFrame};
use crate::storage::FacePersonSampleRecord;
use crate::storage::{Storage, VisionRemoteCommandReceipt};
use crate::vision::{
    runtime::VisionRuntimeState,
    types::{VisionLifecycleState, VisionSamplingState},
};

#[test]
fn opening_legacy_database_creates_vision_v5_without_losing_existing_face_alerts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");

    assert_eq!(storage.vision_schema_version().expect("schema version"), 5);
    assert_eq!(
        storage
            .legacy_face_alert_count()
            .expect("legacy alert count"),
        0
    );
}

#[test]
fn duplicate_remote_command_returns_the_first_persisted_result() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");

    let first = storage
        .record_vision_remote_command("issuer", "target", "command-1", "nonce-1", 100, "accepted")
        .expect("first command");
    let duplicate = storage
        .record_vision_remote_command("issuer", "target", "command-1", "nonce-2", 101, "other")
        .expect("duplicate command");

    assert_eq!(first, "accepted");
    assert_eq!(duplicate, "accepted");
}

#[test]
fn vision_policy_receipt_is_idempotent_and_rejects_stale_revisions() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");

    let accepted = storage
        .accept_vision_remote_policy_command(
            "issuer",
            "target",
            "command-2",
            "nonce-2",
            2,
            1_000,
            100,
            "{\"operation\":\"policy_patch\"}",
        )
        .expect("accepts latest revision");
    assert_eq!(
        accepted,
        VisionRemoteCommandReceipt::Accepted {
            result_json: "{\"operation\":\"policy_patch\"}".to_string()
        }
    );

    let duplicate = storage
        .accept_vision_remote_policy_command(
            "issuer",
            "target",
            "command-2",
            "other-nonce",
            2,
            1_000,
            101,
            "{\"operation\":\"profile_install\"}",
        )
        .expect("returns stored receipt");
    assert_eq!(
        duplicate,
        VisionRemoteCommandReceipt::Duplicate {
            result_json: "{\"operation\":\"policy_patch\"}".to_string()
        }
    );

    let stale = storage
        .accept_vision_remote_policy_command(
            "issuer",
            "target",
            "command-3",
            "nonce-3",
            1,
            1_000,
            102,
            "{\"operation\":\"policy_patch\"}",
        )
        .expect("stale revision is persisted without scheduling");
    assert_eq!(stale, VisionRemoteCommandReceipt::Stale);
}

#[test]
fn legacy_person_samples_are_copied_idempotently_into_v5_reference_images() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
    storage
        .upsert_face_person(&FacePersonPolicyFrame {
            person_id: "person-1".to_string(),
            display_name: "测试人员".to_string(),
            photo_url: Some("photo://one".to_string()),
            photo_urls: vec!["photo://one".to_string(), "photo://two".to_string()],
            photo_sha256: Some("hash-one".to_string()),
            photo_sha256s: vec!["hash-one".to_string(), "hash-two".to_string()],
            expires_at: None,
            enabled: true,
            version: 1,
            action: "upsert".to_string(),
            issued_by_device_id: "local".to_string(),
            issued_by_nickname: "本机".to_string(),
            issued_at: 100,
        })
        .expect("legacy person saved");

    storage.migrate_legacy_vision_data().expect("migration");
    storage
        .migrate_legacy_vision_data()
        .expect("migration retry");

    assert_eq!(
        storage
            .vision_reference_image_count("person-1")
            .expect("reference images"),
        2
    );
}

#[test]
fn legacy_embeddings_and_alert_history_are_copied_without_mutating_legacy_tables() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
    storage
        .upsert_face_person(&FacePersonPolicyFrame {
            person_id: "person-legacy".to_string(),
            display_name: "历史人员".to_string(),
            photo_url: Some("photo://legacy".to_string()),
            photo_urls: vec!["photo://legacy".to_string()],
            photo_sha256: Some("hash-legacy".to_string()),
            photo_sha256s: vec!["hash-legacy".to_string()],
            expires_at: None,
            enabled: true,
            version: 1,
            action: "upsert".to_string(),
            issued_by_device_id: "local".to_string(),
            issued_by_nickname: "本机".to_string(),
            issued_at: 100,
        })
        .expect("legacy person saved");
    storage
        .replace_face_person_samples(
            "person-legacy",
            &[FacePersonSampleRecord {
                sample_id: "sample-legacy".to_string(),
                person_id: "person-legacy".to_string(),
                photo_url: "photo://legacy".to_string(),
                photo_sha256: Some("hash-legacy".to_string()),
                embedding: Some(vec![1, 2, 3]),
                embedding_model_version: Some("face-v1".to_string()),
                body_embedding: Some(vec![4, 5, 6]),
                body_embedding_model_version: Some("body-v1".to_string()),
            }],
        )
        .expect("legacy embeddings saved");
    storage
        .upsert_camera_face_alert(&CameraFaceAlertFrame {
            alert_id: "legacy-alert".to_string(),
            source_kind: "camera_face".to_string(),
            source_device_id: "local".to_string(),
            source_nickname: "本机".to_string(),
            source_address: Some("192.168.1.9".to_string()),
            person_id: "person-legacy".to_string(),
            person_name: "历史人员".to_string(),
            confidence: 90,
            recognition_level: "confirmed".to_string(),
            face_confidence: Some(90),
            body_confidence: Some(70),
            consecutive_hits: 2,
            policy_version: 1,
            created_at: 100,
        })
        .expect("legacy alert saved");

    storage.migrate_legacy_vision_data().expect("migration");
    storage
        .migrate_legacy_vision_data()
        .expect("migration retry");

    assert_eq!(storage.legacy_face_alert_count().expect("legacy alerts"), 1);
    assert_eq!(storage.vision_embedding_count().expect("v5 embeddings"), 2);
    assert_eq!(storage.vision_alert_event_count().expect("v5 alerts"), 1);
    storage
        .verify_vision_database_integrity()
        .expect("vision database integrity");
}

#[test]
fn persists_only_user_pause_across_runtime_restart() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
    let runtime = VisionRuntimeState::restore(
        storage
            .load_vision_runtime_state()
            .expect("default runtime state"),
    );
    runtime.mark_model_availability(true);
    runtime.set_active_profile("baseline", "1.0.0");
    runtime.pause_by_user();
    storage
        .save_vision_runtime_state(&runtime.persisted_state())
        .expect("runtime state saved");

    let restored = VisionRuntimeState::restore(
        storage
            .load_vision_runtime_state()
            .expect("persisted runtime state"),
    );
    let snapshot = restored.snapshot();
    assert_eq!(snapshot.sampling, VisionSamplingState::PausedByUser);
    assert_eq!(snapshot.lifecycle, VisionLifecycleState::Initializing);
    assert_eq!(snapshot.active_profile_id.as_deref(), Some("baseline"));
}
