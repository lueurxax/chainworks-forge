use domain::{
    ids::{ApprovalId, RunId, StageExecutionId},
    run_carry_forward::{
        canonical_digest, ApprovalBindingV1, ApprovalEvidenceV1, ContentDigest, EntryRole,
    },
};
use serde_json::json;

fn evidence() -> ApprovalEvidenceV1 {
    ApprovalEvidenceV1 {
        source_run_id: RunId::new(),
        successor_run_id: RunId::new(),
        manifest_sha256: ContentDigest::of(b"manifest"),
        proposal_sha256: ContentDigest::of(b"proposal"),
        code_sha256: ContentDigest::of(b"code"),
        workflow_snapshot_hash: ContentDigest::of(b"workflow"),
        catalog_snapshot_hash: ContentDigest::of(b"current catalog"),
        capability_delta_sha256: ContentDigest::of(b"new headless grant"),
        unresolved_findings_sha256: ContentDigest::of(b"findings"),
    }
}

#[test]
fn content_hash_is_strict_and_canonical_object_order_is_stable() {
    let digest = ContentDigest::of(b"abc");
    assert_eq!(
        serde_json::from_str::<ContentDigest>(&serde_json::to_string(&digest).unwrap()).unwrap(),
        digest
    );
    assert_eq!(
        digest.as_str(),
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        canonical_digest(&json!({"b":2,"a":1})).unwrap(),
        canonical_digest(&json!({"a":1,"b":2})).unwrap()
    );
    for value in [
        "",
        "abc",
        "sha256:xyz",
        "SHA256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    ] {
        assert!(serde_json::from_value::<ContentDigest>(json!(value)).is_err());
    }
}

#[test]
fn every_approval_tuple_change_invalidates_an_old_grant() {
    let evidence = evidence();
    let stage = StageExecutionId::new();
    let mut binding =
        ApprovalBindingV1::pending(ApprovalId::new(), stage, evidence.clone()).unwrap();
    assert!(!binding.authorizes(&evidence, stage));
    binding.grant(&evidence, stage).unwrap();
    assert!(binding.authorizes(&evidence, stage));
    assert!(!binding.authorizes(&evidence, StageExecutionId::new()));
    let original = serde_json::to_value(&evidence).unwrap();
    for field in original.as_object().unwrap().keys() {
        let mut changed = original.clone();
        changed[field] = if field.ends_with("run_id") {
            json!(RunId::new())
        } else {
            json!(ContentDigest::of(b"changed"))
        };
        let changed = serde_json::from_value(changed).unwrap();
        assert!(!binding.authorizes(&changed, stage), "{field}");
    }
}

#[test]
fn old_grant_cannot_be_reused_after_consumption_or_supersession() {
    let evidence = evidence();
    let stage = StageExecutionId::new();
    let mut binding =
        ApprovalBindingV1::pending(ApprovalId::new(), stage, evidence.clone()).unwrap();
    assert!(binding.consume(&evidence, stage).is_err());
    binding.grant(&evidence, stage).unwrap();
    binding.consume(&evidence, stage).unwrap();
    assert!(!binding.authorizes(&evidence, stage));
    assert!(binding.consume(&evidence, stage).is_err());
    assert!(binding.grant(&evidence, stage).is_err());
    let mut next = ApprovalBindingV1::pending(ApprovalId::new(), stage, evidence.clone()).unwrap();
    next.grant(&evidence, stage).unwrap();
    next.supersede();
    assert!(!next.authorizes(&evidence, stage));
}

#[test]
fn source_cannot_be_its_own_successor_and_wire_cannot_invent_authority() {
    let mut evidence = evidence();
    evidence.successor_run_id = evidence.source_run_id;
    assert!(
        ApprovalBindingV1::pending(ApprovalId::new(), StageExecutionId::new(), evidence).is_err()
    );
    assert!(serde_json::from_value::<EntryRole>(json!("successful_output")).is_err());
    assert!(!EntryRole::ReferenceOnly.can_seed_input());
    assert!(!EntryRole::PreserveOnly.can_seed_input());
    assert!(!EntryRole::Excluded.can_seed_input());
    assert!(EntryRole::ExecutionSeed.can_seed_input());
}
