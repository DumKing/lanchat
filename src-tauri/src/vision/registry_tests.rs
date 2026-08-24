use super::registry::{
    eviction_candidates, validate_package_entries, verify_catalog, verify_package_source,
    CatalogSignature, ModelCacheEntry, ModelPackageSource, SignedCatalog, TrustedKeyRing,
};
use ed25519_dalek::Signer;

#[test]
fn rejects_unsigned_remote_package_but_labels_local_import_unsigned() {
    assert_eq!(
        verify_package_source(ModelPackageSource::OfficialCatalog, None).unwrap_err(),
        "VISION_PACKAGE_SIGNATURE_REQUIRED"
    );
    assert_eq!(
        verify_package_source(ModelPackageSource::LocalImport, None).unwrap(),
        "unsigned_local"
    );
}

#[test]
fn eviction_never_removes_baseline_active_or_last_known_good() {
    let entries = vec![
        ModelCacheEntry::new("baseline", 400, 1).baseline(),
        ModelCacheEntry::new("active", 400, 2).active(),
        ModelCacheEntry::new("lkg", 400, 3).last_known_good(),
        ModelCacheEntry::new("old", 400, 4),
        ModelCacheEntry::new("older", 400, 0),
    ];

    assert_eq!(eviction_candidates(&entries, 1_000), vec!["older", "old"]);
}

#[test]
fn catalog_rejects_a_revoked_signing_key() {
    let signing = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let catalog = serde_json::json!({"schemaVersion": 1, "packages": []});
    let payload = serde_json::to_vec(&catalog).unwrap();
    let signature = signing.sign(&payload);
    let signed = SignedCatalog {
        catalog,
        signature: CatalogSignature {
            key_id: "root-1".to_string(),
            signature_hex: hex::encode(signature.to_bytes()),
        },
    };
    let mut ring = TrustedKeyRing::new("root-1", hex::encode(signing.verifying_key().to_bytes()));
    ring.revoke("root-1");

    assert_eq!(
        verify_catalog(&signed, &ring).unwrap_err(),
        "VISION_CATALOG_KEY_REVOKED"
    );
}

#[test]
fn data_only_model_package_rejects_executables_and_path_traversal() {
    assert_eq!(
        validate_package_entries(&["models/face.onnx", "bin/helper.exe"]).unwrap_err(),
        "VISION_PACKAGE_ENTRY_FORBIDDEN"
    );
    assert_eq!(
        validate_package_entries(&["../models/face.onnx"]).unwrap_err(),
        "VISION_PACKAGE_ENTRY_INVALID"
    );
}
