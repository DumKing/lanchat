use super::storage::{
    validate_new_reference_image_count, validate_reference_subject_count, MAX_REFERENCE_IMAGES,
    MIN_REFERENCE_IMAGES,
};

#[test]
fn rejects_reference_image_counts_outside_new_person_range() {
    assert!(validate_new_reference_image_count(MIN_REFERENCE_IMAGES - 1).is_err());
    assert!(validate_new_reference_image_count(MIN_REFERENCE_IMAGES).is_ok());
    assert!(validate_new_reference_image_count(MAX_REFERENCE_IMAGES).is_ok());
    assert!(validate_new_reference_image_count(MAX_REFERENCE_IMAGES + 1).is_err());
}

#[test]
fn rejects_multi_person_reference_image() {
    assert!(validate_reference_subject_count(1).is_ok());
    assert!(validate_reference_subject_count(2).is_err());
}
