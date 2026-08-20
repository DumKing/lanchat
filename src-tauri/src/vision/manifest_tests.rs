use super::manifest::{embedding_space_id, validate_manifest, VisionManifestV3};

fn manifest_json(profile_version: &str, color_order: &str) -> String {
    format!(
        r#"{{
          "schemaVersion": 3,
          "package": {{ "id": "official.baseline", "version": "1.0.0" }},
          "profile": {{ "id": "baseline", "version": "{profile_version}", "tier": "low_resource" }},
          "components": [{{
            "id": "face-recognizer", "category": "face_recognizer", "file": "face.onnx",
            "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "adapterId": "builtin.face.v1",
            "input": {{ "colorOrder": "{color_order}", "resizeMode": "letterbox", "normalization": "arcface" }},
            "output": {{ "embeddingDimension": 128, "distanceMetric": "cosine" }}
          }}]
        }}"#
    )
}

#[test]
fn rejects_profile_when_profile_version_differs_from_package_version() {
    let manifest: VisionManifestV3 = serde_json::from_str(&manifest_json("1.0.1", "RGB")).unwrap();
    assert!(validate_manifest(&manifest).is_err());
}

#[test]
fn embedding_space_changes_when_preprocessing_changes() {
    let rgb: VisionManifestV3 = serde_json::from_str(&manifest_json("1.0.0", "RGB")).unwrap();
    let bgr: VisionManifestV3 = serde_json::from_str(&manifest_json("1.0.0", "BGR")).unwrap();
    assert_ne!(embedding_space_id(&rgb, "face-recognizer").unwrap(), embedding_space_id(&bgr, "face-recognizer").unwrap());
}

#[test]
fn bundled_v3_manifest_is_valid() {
    let manifest: VisionManifestV3 =
        serde_json::from_str(include_str!("../../resources/object-models/manifest.v3.json")).unwrap();
    validate_manifest(&manifest).unwrap();
}
