use super::protocol::{legacy_face_monitor_patch, validate_policy_frame, VisionPolicyFrame};
use crate::protocol::{decode_frame, encode_frame, FaceMonitorPolicyFrame, WireFrame};

#[test]
fn duplicate_command_uses_one_stable_command_identity() {
    let frame = VisionPolicyFrame::new("issuer", "target", "policy_patch", 3, 100, 200);
    assert!(validate_policy_frame(&frame, 150).is_ok());
    assert_eq!(
        validate_policy_frame(&frame, 201).unwrap_err(),
        "VISION_POLICY_EXPIRED"
    );
}

#[test]
fn old_face_monitor_policy_only_updates_legacy_fields() {
    let legacy = FaceMonitorPolicyFrame {
        target_device_id: "target".to_string(),
        min_confidence: 81,
        body_min_confidence: 77,
        sample_fps: 3,
        consecutive_hits: 2,
        cooldown_seconds: 60,
        face_cooldown_seconds: 60,
        body_cooldown_seconds: 300,
        settings_locked: true,
        version: 4,
        issued_by_device_id: "issuer".to_string(),
        issued_by_nickname: "管理员".to_string(),
        issued_at: 100,
    };

    let patch = legacy_face_monitor_patch(&legacy);
    assert_eq!(patch.target_device_id, "target");
    assert_eq!(patch.revision, 4);
    assert!(patch.payload.get("profile").is_none());
    assert_eq!(patch.payload["faceMinScore"], 81);
}

#[test]
fn vision_policy_frame_round_trips_without_transporting_model_bytes() {
    let mut policy = VisionPolicyFrame::new("issuer", "target", "profile_install", 5, 100, 200);
    policy.payload = serde_json::json!({
        "profileId": "balanced-cpu",
        "profileVersion": "1.0.0",
        "packageUrl": "https://example.test/models/balanced.zip",
    });

    let encoded = encode_frame(&WireFrame::VisionPolicy(policy.clone())).expect("encodes");
    assert!(!encoded.contains("model_bytes"));
    assert_eq!(
        decode_frame(&encoded).expect("decodes"),
        WireFrame::VisionPolicy(policy)
    );
}
