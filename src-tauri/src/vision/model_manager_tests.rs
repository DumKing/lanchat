use super::{
    model_manager::{
        parse_package_manifest, parse_signed_catalog_with_key_ring, validate_catalog_model_stack,
        validate_installed_package, VisionModelStack,
    },
    registry::{CatalogSignature, SignedCatalog, TrustedKeyRing},
};
use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn signed_catalog(catalog: serde_json::Value) -> (Vec<u8>, TrustedKeyRing) {
    let signing = ed25519_dalek::SigningKey::from_bytes(&[19; 32]);
    let signature = signing.sign(&serde_json::to_vec(&catalog).unwrap());
    let signed = SignedCatalog {
        catalog,
        signature: CatalogSignature {
            key_id: "test-root".to_string(),
            signature_hex: hex::encode(signature.to_bytes()),
        },
    };
    (
        serde_json::to_vec(&signed).unwrap(),
        TrustedKeyRing::new("test-root", hex::encode(signing.verifying_key().to_bytes())),
    )
}

#[test]
fn catalog_accepts_a_profile_with_recommended_runtime_settings() {
    let (bytes, key_ring) = signed_catalog(serde_json::json!({
        "schemaVersion": 1,
        "profiles": [{
            "profileId": "balanced-office",
            "profileVersion": "1.0.0",
            "displayName": "均衡识别",
            "tier": "balanced",
            "modelStack": {
                "inferenceEngine": "onnxruntime",
                "personDetector": "yolox",
                "faceEngine": "sface",
                "personReIdEngine": "osnet-x025",
                "provider": "torchreid",
                "license": "Apache-2.0"
            },
            "downloadUrl": "https://github.com/DumKing/lanchat/releases/download/v0.5.1/balanced-office.zip",
            "packageSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "packageSizeBytes": 1024,
            "recommendedSettings": {
                "sampleFps": 2,
                "faceMinConfidence": 60,
                "bodyMinConfidence": 68,
                "consecutiveHits": 1
            }
        }]
    }));

    let parsed = parse_signed_catalog_with_key_ring(&bytes, &key_ring).unwrap();
    assert_eq!(parsed.profiles[0].profile_id, "balanced-office");
    assert_eq!(
        parsed.profiles[0]
            .recommended_settings
            .as_ref()
            .unwrap()
            .sample_fps,
        2
    );
    assert_eq!(
        parsed.profiles[0]
            .model_stack
            .as_ref()
            .expect("catalog v2 profile exposes its model stack")
            .person_re_id_engine,
        "osnet-x025"
    );
}

#[test]
fn catalog_rejects_unknown_model_stack_adapter() {
    let (bytes, key_ring) = signed_catalog(serde_json::json!({
        "schemaVersion": 2,
        "profiles": [{
            "profileId": "bad-stack",
            "profileVersion": "1.0.0",
            "displayName": "坏模型",
            "tier": "balanced",
            "modelStack": {
                "inferenceEngine": "onnxruntime",
                "personDetector": "yolox",
                "faceEngine": "sface",
                "personReIdEngine": "not-real",
                "provider": "example",
                "license": "Apache-2.0"
            },
            "downloadUrl": "https://github.com/DumKing/lanchat/releases/download/v0.6.0/bad.zip",
            "packageSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }]
    }));

    assert_eq!(
        parse_signed_catalog_with_key_ring(&bytes, &key_ring).unwrap_err(),
        "VISION_CATALOG_MODEL_STACK_INVALID"
    );
}

#[test]
fn catalog_stack_must_match_the_package_v4_pipeline() {
    let manifest = r#"{
      "schemaVersion": 4,
      "package": { "id": "official.test", "version": "1.0.0" },
      "profile": {
        "id": "office-osnet-x025", "version": "1.0.0", "tier": "balanced",
        "displayName": "SFace + OSNet", "provider": "test", "engine": "onnxruntime",
        "supportedBackends": ["cpu"]
      },
      "pipeline": {
        "personDetector": "person-detector", "faceEngine": "face-recognizer",
        "personReIdEngine": "person-reid", "fusionPolicy": "quality-temporal-v1"
      },
      "components": [
        {"id":"face-detector","category":"face_detector","family":"yunet","file":"face-detector.onnx","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","adapterId":"builtin.face-detector.yunet.v1","engine":"onnxruntime"},
        {"id":"face-recognizer","category":"face_recognizer","family":"sface","file":"face.onnx","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","adapterId":"builtin.face-recognizer.sface.v1","engine":"onnxruntime","input":{"colorOrder":"BGR","resizeMode":"aligned_112","normalization":"sface_127_5"},"output":{"embeddingDimension":128,"distanceMetric":"cosine"}},
        {"id":"person-detector","category":"person_detector","family":"yolox","file":"person.onnx","sha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","adapterId":"builtin.person-detector.yolox.v1","engine":"onnxruntime"},
        {"id":"person-reid","category":"person_reid","family":"osnet-x025","file":"osnet.onnx","sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd","adapterId":"builtin.person-reid.osnet.v1","engine":"onnxruntime","input":{"colorOrder":"RGB","resizeMode":"256x128","normalization":"imagenet"},"output":{"embeddingDimension":512,"distanceMetric":"cosine"}}
      ]
    }"#;
    let stack = VisionModelStack {
        inference_engine: "onnxruntime".to_string(),
        person_detector: "yolox".to_string(),
        face_engine: "sface".to_string(),
        person_re_id_engine: "youtureid".to_string(),
        provider: "test".to_string(),
        license: "Apache-2.0".to_string(),
    };

    assert_eq!(
        validate_catalog_model_stack(manifest, &stack).unwrap_err(),
        "VISION_PACKAGE_MODEL_STACK_MISMATCH"
    );
}

#[test]
fn catalog_rejects_an_out_of_range_recommended_setting() {
    let (bytes, key_ring) = signed_catalog(serde_json::json!({
        "schemaVersion": 1,
        "profiles": [{
            "profileId": "invalid-profile",
            "profileVersion": "1.0.0",
            "displayName": "无效档位",
            "tier": "low_resource",
            "downloadUrl": "https://github.com/DumKing/lanchat/releases/download/v0.5.1/invalid.zip",
            "packageSha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "recommendedSettings": {
                "sampleFps": 0,
                "faceMinConfidence": 60,
                "bodyMinConfidence": 68,
                "consecutiveHits": 1
            }
        }]
    }));

    assert_eq!(
        parse_signed_catalog_with_key_ring(&bytes, &key_ring).unwrap_err(),
        "VISION_CATALOG_RECOMMENDED_SETTINGS_INVALID"
    );
}

#[test]
fn package_parser_accepts_v4_profile_with_distinct_osnet_component() {
    let parsed = parse_package_manifest(
        r#"{
          "schemaVersion": 4,
          "package": { "id": "official.osnet", "version": "1.0.0" },
          "profile": {
            "id": "office-osnet-x025", "version": "1.0.0", "tier": "balanced",
            "displayName": "SFace + OSNet x0.25", "provider": "torchreid", "engine": "onnxruntime",
            "supportedBackends": ["cpu"]
          },
          "pipeline": {
            "personDetector": "person-detector", "faceEngine": "face-recognizer",
            "personReIdEngine": "person-reid", "fusionPolicy": "quality-temporal-v1"
          },
          "components": [
            {
              "id": "person-detector", "category": "person_detector", "family": "yolox",
              "file": "person-detector.onnx", "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              "adapterId": "builtin.person-detector.yolox.v1", "engine": "onnxruntime"
            },
            {
              "id": "face-recognizer", "category": "face_recognizer", "family": "sface",
              "file": "sface.onnx", "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
              "adapterId": "builtin.face-recognizer.sface.v1", "engine": "onnxruntime",
              "input": { "colorOrder": "RGB", "resizeMode": "aligned_112", "normalization": "sface_127_5" },
              "output": { "embeddingDimension": 128, "distanceMetric": "cosine" }
            },
            {
              "id": "person-reid", "category": "person_reid", "family": "osnet-x025",
              "file": "osnet.onnx", "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
              "adapterId": "builtin.person-reid.osnet.v1", "engine": "onnxruntime",
              "input": { "colorOrder": "RGB", "resizeMode": "256x128", "normalization": "imagenet" },
              "output": { "embeddingDimension": 512, "distanceMetric": "cosine" }
            }
          ]
        }"#,
    )
    .unwrap();

    assert_eq!(parsed.profile_id, "office-osnet-x025");
    assert_eq!(parsed.profile_version, "1.0.0");
    assert_eq!(
        parsed.components[2].adapter_id,
        "builtin.person-reid.osnet.v1"
    );
}

#[test]
fn v4_package_rejects_a_tampered_auxiliary_model_file() {
    let root = tempdir().expect("package root");
    let models = root.path().join("object-models");
    std::fs::create_dir_all(&models).expect("model directory");
    let digest = |value: &[u8]| hex::encode(Sha256::digest(value));
    let detector = b"detector";
    let face = b"face";
    let reid = b"reid";
    std::fs::write(models.join("detector.onnx"), detector).expect("detector");
    std::fs::write(models.join("face.onnx"), face).expect("face");
    std::fs::write(models.join("reid.onnx"), reid).expect("reid");
    // 该文件存在，但内容已被替换；安装预检必须验证它的哈希。
    std::fs::write(models.join("reid.calibration"), b"tampered").expect("auxiliary");
    std::fs::write(
        models.join("manifest.v4.json"),
        format!(
            r#"{{
              "schemaVersion": 4,
              "package": {{ "id": "official.test", "version": "1.0.0" }},
              "profile": {{
                "id": "test-profile", "version": "1.0.0", "tier": "balanced",
                "displayName": "Test", "provider": "test", "engine": "onnxruntime",
                "supportedBackends": ["cpu"]
              }},
              "pipeline": {{
                "personDetector": "person-detector", "faceEngine": "face-recognizer",
                "personReIdEngine": "person-reid", "fusionPolicy": "quality-temporal-v1"
              }},
              "components": [
                {{
                  "id": "person-detector", "category": "person_detector", "family": "yolox",
                  "file": "detector.onnx", "sha256": "{}",
                  "adapterId": "builtin.person-detector.yolox.v1", "engine": "onnxruntime"
                }},
                {{
                  "id": "face-recognizer", "category": "face_recognizer", "family": "sface",
                  "file": "face.onnx", "sha256": "{}",
                  "adapterId": "builtin.face-recognizer.sface.v1", "engine": "onnxruntime",
                  "input": {{ "colorOrder": "RGB", "resizeMode": "aligned_112", "normalization": "sface_127_5" }},
                  "output": {{ "embeddingDimension": 128, "distanceMetric": "cosine" }}
                }},
                {{
                  "id": "person-reid", "category": "person_reid", "family": "osnet-x025",
                  "file": "reid.onnx", "sha256": "{}",
                  "adapterId": "builtin.person-reid.osnet.v1", "engine": "onnxruntime",
                  "input": {{ "colorOrder": "RGB", "resizeMode": "256x128", "normalization": "imagenet" }},
                  "output": {{ "embeddingDimension": 512, "distanceMetric": "cosine" }},
                  "auxiliaryFiles": [{{ "file": "reid.calibration", "sha256": "{}" }}]
                }}
              ]
            }}"#,
            digest(detector),
            digest(face),
            digest(reid),
            digest(b"expected")
        ),
    )
    .expect("manifest");

    assert_eq!(
        validate_installed_package(root.path()).unwrap_err(),
        "VISION_PACKAGE_COMPONENT_HASH_MISMATCH"
    );
}
