use crate::protocol::{CameraFaceAlertFrame, FacePersonPolicyFrame};
use crate::storage::FacePersonSampleRecord;
use crate::storage::{Storage, VisionEmbeddingWrite, VisionRemoteCommandReceipt};
use crate::vision::{
    runtime::VisionRuntimeState,
    types::{VisionLifecycleState, VisionModelProfileSummary, VisionSamplingState},
};

fn installed_profile(id: &str, version: &str) -> VisionModelProfileSummary {
    VisionModelProfileSummary {
        profile_id: id.to_string(),
        profile_version: version.to_string(),
        display_name: id.to_string(),
        tier: "low_resource".to_string(),
        inference_engine: Some("onnxruntime".to_string()),
        face_engine: Some("sface".to_string()),
        person_re_id_engine: Some("youtureid".to_string()),
        installed: true,
        active: false,
        compatible: true,
        compatibility_reason: None,
        downloadable: false,
        package_size_bytes: 0,
        restart_required: true,
        recommended_settings: None,
    }
}

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
fn feature_store_keeps_embeddings_in_separate_model_spaces() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
    storage
        .upsert_face_person(&FacePersonPolicyFrame {
            person_id: "person-spaces".to_string(),
            display_name: "多模型人员".to_string(),
            photo_url: None,
            photo_urls: vec![],
            photo_sha256: None,
            photo_sha256s: vec![],
            expires_at: None,
            enabled: true,
            version: 1,
            action: "upsert".to_string(),
            issued_by_device_id: "local".to_string(),
            issued_by_nickname: "本机".to_string(),
            issued_at: 100,
        })
        .expect("person saved");
    storage
        .ensure_vision_embedding_space(
            "face.sface.v1.aaaaaaaa",
            "baseline",
            "1.0.0",
            "face",
            "{\"adapter\":\"sface\"}",
        )
        .expect("face space");
    storage
        .ensure_vision_embedding_space(
            "body.osnet-x025.v1.bbbbbbbb",
            "office-osnet-x025",
            "1.0.0",
            "body",
            "{\"adapter\":\"osnet\"}",
        )
        .expect("body space");
    storage
        .replace_person_vision_embeddings(
            "person-spaces",
            "face.sface.v1.aaaaaaaa",
            "face",
            &[VisionEmbeddingWrite::new(vec![1.0, 0.0], 0.9)],
        )
        .expect("face embedding");
    storage
        .replace_person_vision_embeddings(
            "person-spaces",
            "body.osnet-x025.v1.bbbbbbbb",
            "body",
            &[VisionEmbeddingWrite::new(vec![0.0, 1.0, 0.0], 0.8)],
        )
        .expect("body embedding");

    assert_eq!(
        storage
            .list_person_vision_embeddings("person-spaces", "face.sface.v1.aaaaaaaa", "face")
            .expect("face embeddings")
            .len(),
        1
    );
    assert_eq!(
        storage
            .list_person_vision_embeddings("person-spaces", "body.osnet-x025.v1.bbbbbbbb", "body")
            .expect("body embeddings")
            .len(),
        1
    );
}

#[test]
fn failed_selected_profile_rolls_back_to_last_known_good() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
    let baseline_dir = temp.path().join("baseline");
    let candidate_dir = temp.path().join("candidate");
    storage
        .upsert_vision_model_profile(&installed_profile("baseline", "1.0.0"), "{}", &baseline_dir)
        .expect("baseline installed");
    storage
        .upsert_vision_model_profile(
            &installed_profile("candidate", "2.0.0"),
            "{}",
            &candidate_dir,
        )
        .expect("candidate installed");
    storage
        .activate_vision_model_profile("baseline", "1.0.0")
        .expect("baseline active");
    storage
        .mark_vision_model_profile_healthy("baseline", "1.0.0")
        .expect("baseline healthy");
    storage
        .activate_vision_model_profile("candidate", "2.0.0")
        .expect("candidate selected");

    let fallback = storage
        .rollback_failed_vision_model_profile("candidate", "2.0.0")
        .expect("rollback result")
        .expect("baseline fallback");
    assert_eq!(fallback.0, "baseline");
    assert_eq!(fallback.1, "1.0.0");
    assert_eq!(fallback.2, baseline_dir);
    let active = storage
        .active_vision_model_install_path()
        .expect("active model")
        .expect("fallback active");
    assert_eq!(active.0, "baseline");

    assert_eq!(
        storage
            .activate_vision_model_profile("missing", "1.0.0")
            .expect_err("missing profile rejected"),
        "VISION_MODEL_PROFILE_NOT_INSTALLED"
    );
}

#[test]
fn installed_inactive_profile_can_be_removed_but_active_profile_is_protected() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
    let active_dir = temp.path().join("active-profile");
    let removable_dir = temp.path().join("removable-profile");
    storage
        .upsert_vision_model_profile(&installed_profile("active", "1.0.0"), "{}", &active_dir)
        .expect("active profile installed");
    storage
        .upsert_vision_model_profile(
            &installed_profile("removable", "1.0.0"),
            "{}",
            &removable_dir,
        )
        .expect("removable profile installed");
    storage
        .activate_vision_model_profile("active", "1.0.0")
        .expect("active profile selected");

    assert_eq!(
        storage
            .remove_vision_model_profile("active", "1.0.0")
            .expect_err("active profile cannot be removed"),
        "VISION_MODEL_PROFILE_ACTIVE"
    );
    assert_eq!(
        storage
            .remove_vision_model_profile("removable", "1.0.0")
            .expect("inactive profile removed"),
        removable_dir
    );
    assert_eq!(
        storage
            .vision_model_install_path("removable", "1.0.0")
            .expect_err("removed profile is no longer installed"),
        "VISION_MODEL_PROFILE_NOT_INSTALLED"
    );
}

#[test]
fn removing_one_reference_photo_keeps_required_samples_and_updates_primary_photo() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");
    storage
        .upsert_face_person(&FacePersonPolicyFrame {
            person_id: "local-person".to_string(),
            display_name: "本机人员".to_string(),
            photo_url: Some("front.jpg".to_string()),
            photo_urls: vec![
                "front.jpg".to_string(),
                "left.jpg".to_string(),
                "right.jpg".to_string(),
                "back.jpg".to_string(),
            ],
            photo_sha256: None,
            photo_sha256s: vec![],
            expires_at: None,
            enabled: true,
            version: 1,
            action: "upsert".to_string(),
            issued_by_device_id: "local".to_string(),
            issued_by_nickname: "本机录入".to_string(),
            issued_at: 100,
        })
        .expect("person saved");

    let retained = storage
        .remove_face_person_sample("local-person", "front.jpg")
        .expect("reference photo removed");
    assert_eq!(retained.len(), 3);
    assert!(retained
        .iter()
        .all(|sample| sample.photo_url != "front.jpg"));
    let person = storage
        .list_face_people()
        .expect("people listed")
        .into_iter()
        .find(|person| person.person_id == "local-person")
        .expect("person retained");
    assert_eq!(person.photo_url.as_deref(), Some("left.jpg"));
    assert_eq!(person.photo_urls, vec!["left.jpg", "right.jpg", "back.jpg"]);
    assert_eq!(
        storage
            .remove_face_person_sample("local-person", "left.jpg")
            .expect_err("minimum reference count is protected"),
        "VISION_REFERENCE_IMAGE_MINIMUM_REQUIRED"
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
