use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedSnapshotPairV1<'a> {
    pub workflow_json: &'a str,
    pub catalog_json: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotIntegrityInvalidReasonV1 {
    IncompleteQuartet,
    MalformedHash,
    DigestMismatch,
}

impl SnapshotIntegrityInvalidReasonV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IncompleteQuartet => "incomplete_quartet",
            Self::MalformedHash => "malformed_hash",
            Self::DigestMismatch => "digest_mismatch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotIntegrityV1<'a> {
    Absent,
    Verified(VerifiedSnapshotPairV1<'a>),
    Invalid(SnapshotIntegrityInvalidReasonV1),
}

pub fn verify_complete_pair_v1<'a>(
    workflow_json: Option<&'a str>,
    catalog_json: Option<&'a str>,
    workflow_hash: Option<&str>,
    catalog_hash: Option<&str>,
) -> SnapshotIntegrityV1<'a> {
    match (workflow_json, catalog_json, workflow_hash, catalog_hash) {
        (None, None, None, None) => SnapshotIntegrityV1::Absent,
        (Some(workflow_json), Some(catalog_json), Some(workflow_hash), Some(catalog_hash)) => {
            if [workflow_json, catalog_json, workflow_hash, catalog_hash]
                .iter()
                .any(|value| value.trim().is_empty())
            {
                return SnapshotIntegrityV1::Invalid(
                    SnapshotIntegrityInvalidReasonV1::IncompleteQuartet,
                );
            }
            if !is_canonical_sha256(workflow_hash) || !is_canonical_sha256(catalog_hash) {
                return SnapshotIntegrityV1::Invalid(
                    SnapshotIntegrityInvalidReasonV1::MalformedHash,
                );
            }
            let actual_workflow_hash = format!("{:x}", Sha256::digest(workflow_json.as_bytes()));
            let actual_catalog_hash = format!("{:x}", Sha256::digest(catalog_json.as_bytes()));
            if actual_workflow_hash != workflow_hash || actual_catalog_hash != catalog_hash {
                return SnapshotIntegrityV1::Invalid(
                    SnapshotIntegrityInvalidReasonV1::DigestMismatch,
                );
            }
            SnapshotIntegrityV1::Verified(VerifiedSnapshotPairV1 {
                workflow_json,
                catalog_json,
            })
        }
        _ => SnapshotIntegrityV1::Invalid(SnapshotIntegrityInvalidReasonV1::IncompleteQuartet),
    }
}

fn is_canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}
