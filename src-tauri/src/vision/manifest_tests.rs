use super::backend::{adapter_descriptor, RuntimeBackend};
use super::manifest::{embedding_space_id, validate_manifest, VisionManifestV3};
use super::profile::manifest_v4::{validate_manifest_v4, VisionManifestV4};
use super::profile::pipeline::resolve_pipeline;

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
    assert_ne!(
        embedding_space_id(&rgb, "face-recognizer").unwrap(),
        embedding_space_id(&bgr, "face-recognizer").unwrap()
    );
}

#[test]
fn bundled_v3_manifest_is_valid() {
    let manifest: VisionManifestV3 = serde_json::from_str(include_str!(
        "../../resources/object-models/manifest.v3.json"
    ))
    .unwrap();
    validate_manifest(&manifest).unwrap();
}

#[test]
fn rejects_v4_manifest_with_unknown_pipeline_adapter() {
    let manifest: VisionManifestV4 = serde_json::from_str(
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
              "adapterId": "unregistered.person-reid.v1", "engine": "onnxruntime",
              "input": { "colorOrder": "RGB", "resizeMode": "256x128", "normalization": "imagenet" },
              "output": { "embeddingDimension": 512, "distanceMetric": "cosine" }
            }
          ]
        }"#,
    )
    .unwrap();

    assert_eq!(
        validate_manifest_v4(&manifest).unwrap_err(),
        "VISION_MANIFEST_ADAPTER_UNSUPPORTED"
    );
}

#[test]
fn omz_and_osnet_adapters_keep_their_runtime_backends_separate() {
    let osnet = adapter_descriptor("builtin.person-reid.osnet.v1").expect("OSNet adapter");
    let omz = adapter_descriptor("builtin.person-reid.omz.v1").expect("OMZ adapter");

    assert_eq!(osnet.backend, RuntimeBackend::OnnxRuntime);
    assert_eq!(omz.backend, RuntimeBackend::OpenVino);
    assert_ne!(
        osnet.embedding_space_namespace,
        omz.embedding_space_namespace
    );
}

#[test]
fn omz_retail_variants_have_independent_adapter_namespaces() {
    let retail_0288 = adapter_descriptor("builtin.person-reid.omz.v1").expect("0288 adapter");
    let retail_0286 = adapter_descriptor("builtin.person-reid.omz.0286.v1").expect("0286 adapter");

    assert_eq!(retail_0286.backend, RuntimeBackend::OpenVino);
    assert_ne!(
        retail_0288.embedding_space_namespace,
        retail_0286.embedding_space_namespace
    );
}

#[test]
fn v4_pipeline_assigns_separate_embedding_spaces_to_face_and_body() {
    let manifest: VisionManifestV4 = serde_json::from_str(include_str!(
        "../../resources/object-models/manifest.v4.json"
    ))
    .expect("bundled v4 manifest");
    let pipeline = resolve_pipeline(&manifest).expect("pipeline");

    assert_ne!(
        pipeline.face_recognizer.embedding_space_id,
        pipeline.person_reid.embedding_space_id
    );
    assert_eq!(
        pipeline.person_detector.adapter_id,
        "builtin.person-detector.yolox.v1"
    );
}
