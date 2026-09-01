use sha2::{Digest, Sha256};
use workflow::snapshot_integrity::{
    verify_complete_pair_v1, SnapshotIntegrityInvalidReasonV1, SnapshotIntegrityV1,
};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[test]
fn complete_pair_verifier_accepts_only_exact_authenticated_quartet() {
    let workflow = r#"{"workflow":"v1"}"#;
    let catalog = r#"{"catalog":"v1"}"#;
    let workflow_hash = digest(workflow);
    let catalog_hash = digest(catalog);

    let verified = verify_complete_pair_v1(
        Some(workflow),
        Some(catalog),
        Some(&workflow_hash),
        Some(&catalog_hash),
    );
    let SnapshotIntegrityV1::Verified(pair) = verified else {
        panic!("complete valid quartet must verify")
    };
    assert_eq!(pair.workflow_json, workflow);
    assert_eq!(pair.catalog_json, catalog);

    assert_eq!(
        verify_complete_pair_v1(None, None, None, None),
        SnapshotIntegrityV1::Absent
    );
}

#[test]
fn complete_pair_verifier_rejects_every_partial_or_blank_presence_state() {
    let workflow = r#"{"workflow":"v1"}"#;
    let catalog = r#"{"catalog":"v1"}"#;
    let workflow_hash = digest(workflow);
    let catalog_hash = digest(catalog);
    let values = [workflow, catalog, &workflow_hash, &catalog_hash];

    for mask in 1u8..15 {
        let result = verify_complete_pair_v1(
            (mask & 1 != 0).then_some(values[0]),
            (mask & 2 != 0).then_some(values[1]),
            (mask & 4 != 0).then_some(values[2]),
            (mask & 8 != 0).then_some(values[3]),
        );
        assert_eq!(
            result,
            SnapshotIntegrityV1::Invalid(SnapshotIntegrityInvalidReasonV1::IncompleteQuartet),
            "mask {mask:04b}"
        );
    }

    for blank_index in 0..4 {
        let mut fields = [workflow, catalog, &workflow_hash, &catalog_hash];
        fields[blank_index] = " \n";
        assert_eq!(
            verify_complete_pair_v1(
                Some(fields[0]),
                Some(fields[1]),
                Some(fields[2]),
                Some(fields[3]),
            ),
            SnapshotIntegrityV1::Invalid(SnapshotIntegrityInvalidReasonV1::IncompleteQuartet)
        );
    }
}

#[test]
fn complete_pair_verifier_rejects_malformed_hash_and_digest_mismatch() {
    let workflow = r#"{"workflow":"v1"}"#;
    let catalog = r#"{"catalog":"v1"}"#;
    let workflow_hash = digest(workflow);
    let catalog_hash = digest(catalog);

    for malformed in [
        "a".repeat(63),
        "A".repeat(64),
        "z".repeat(64),
        format!(" {}", "a".repeat(64)),
    ] {
        assert_eq!(
            verify_complete_pair_v1(
                Some(workflow),
                Some(catalog),
                Some(&malformed),
                Some(&catalog_hash),
            ),
            SnapshotIntegrityV1::Invalid(SnapshotIntegrityInvalidReasonV1::MalformedHash)
        );
    }

    assert_eq!(
        verify_complete_pair_v1(
            Some(r#"{"workflow":"tampered"}"#),
            Some(catalog),
            Some(&workflow_hash),
            Some(&catalog_hash),
        ),
        SnapshotIntegrityV1::Invalid(SnapshotIntegrityInvalidReasonV1::DigestMismatch)
    );
}
