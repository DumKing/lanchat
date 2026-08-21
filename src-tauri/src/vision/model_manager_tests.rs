use super::{
    model_manager::parse_signed_catalog_with_key_ring,
    registry::{CatalogSignature, SignedCatalog, TrustedKeyRing},
};
use ed25519_dalek::Signer;

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
