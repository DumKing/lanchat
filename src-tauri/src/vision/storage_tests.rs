use crate::storage::Storage;

#[test]
fn opening_legacy_database_creates_vision_v5_without_losing_existing_face_alerts() {
    let temp = tempfile::tempdir().expect("tempdir");
    let storage = Storage::open(temp.path().join("lanchat.sqlite3")).expect("storage opens");

    assert_eq!(storage.vision_schema_version().expect("schema version"), 5);
    assert_eq!(storage.legacy_face_alert_count().expect("legacy alert count"), 0);
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
