use domain::run_carry_forward_api::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const ID: &str = "00000000-0000-4000-8000-000000000039";
const REQUEST: &str = "00000000-0000-4000-8000-000000000040";
const HASH: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn preview() -> Value {
    json!({
        "run_id": ID,
        "target": {
            "workflow_yaml_path": "examples/workflows/full-mvp-live.yaml",
            "agent_catalog_yaml_path": "examples/agents/agents.yaml",
            "delivery_configuration_json": "{\"repo_identifier\":\"repo\",\"repo_root\":\"/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/worktrees\",\"target_branch\":\"topic\"}"
        },
        "profile": "implementation_restart_v1",
        "selection": {
            "proposal_input": {"kind": "artifact", "id": ID},
            "reference_inputs": [],
            "include_dirty_work": true,
            "excluded_workspace_paths": []
        }
    })
}

fn prepare() -> Value {
    let mut value = preview();
    value["target"]["expected_workflow_snapshot_hash"] = json!(HASH);
    value["target"]["expected_catalog_snapshot_hash"] = json!(HASH);
    value["expected_plan_sha256"] = json!(HASH);
    value["caller_request_id"] = json!(REQUEST);
    value["reason"] = json!("Fresh review under current policy");
    value
}

fn activate() -> Value {
    json!({"operation_id": ID, "expected_version": 3,
        "expected_manifest_sha256": HASH, "caller_request_id": REQUEST})
}

fn action() -> Value {
    let mut value = activate();
    value
        .as_object_mut()
        .unwrap()
        .remove("expected_manifest_sha256");
    value["reason"] = json!("Inspect interrupted preparation");
    value
}

fn round_trip<T: DeserializeOwned + Serialize>(value: Value) {
    let parsed: T = serde_json::from_value(value).unwrap();
    let serialized = serde_json::to_value(parsed).unwrap();
    let again: T = serde_json::from_value(serialized.clone()).unwrap();
    assert_eq!(serde_json::to_value(again).unwrap(), serialized);
}

fn reject<T: DeserializeOwned>(value: Value) {
    assert!(
        serde_json::from_value::<T>(value.clone()).is_err(),
        "accepted {value}"
    );
}

#[test]
fn six_requests_round_trip_with_closed_profiles_and_required_prepare_hashes() {
    round_trip::<ContinuationPreviewRequest>(preview());
    round_trip::<ContinueBlockedRequest>(prepare());
    round_trip::<ContinuationGetRequest>(json!({"operation_id": ID}));
    round_trip::<ContinuationActivateRequest>(activate());
    round_trip::<ContinuationReconcileRequest>(action());
    round_trip::<ContinuationAbortRequest>(action());
    let mut value = prepare();
    value["target"]
        .as_object_mut()
        .unwrap()
        .remove("expected_catalog_snapshot_hash");
    reject::<ContinueBlockedRequest>(value);
    let mut value = preview();
    value["profile"] = json!("arbitrary_state_v2");
    reject::<ContinuationPreviewRequest>(value);
}

fn strict<T: DeserializeOwned>(value: Value) {
    for field in [
        "authority",
        "source_root",
        "destination_id",
        "schema_version",
        "approval_decision",
        "provider_token",
        "delete_source",
    ] {
        let mut invalid = value.clone();
        invalid[field] = json!("caller supplied");
        reject::<T>(invalid);
    }
    // Raw JSON is essential: Value has already lost duplicate keys.
    for (field, entry) in value.as_object().unwrap() {
        let raw = serde_json::to_string(&value).unwrap();
        let duplicate = format!("{{{}:{},{}", json!(field), entry, &raw[1..]);
        assert!(
            serde_json::from_str::<T>(&duplicate).is_err(),
            "duplicate {field}"
        );
    }
}

#[test]
fn every_request_rejects_unknown_authority_and_raw_duplicate_known_fields() {
    strict::<ContinuationPreviewRequest>(preview());
    strict::<ContinueBlockedRequest>(prepare());
    strict::<ContinuationGetRequest>(json!({"operation_id": ID, "cursor": null, "limit": 100}));
    strict::<ContinuationActivateRequest>(activate());
    strict::<ContinuationReconcileRequest>(action());
    strict::<ContinuationAbortRequest>(action());
}

#[test]
fn canonical_uuid_and_v4_caller_id_are_not_silently_normalized() {
    for id in [
        "00000000-0000-4000-8000-0000000000AA",
        "00000000000040008000000000000039",
        "urn:uuid:00000000-0000-4000-8000-000000000039",
        "not-a-uuid",
    ] {
        let mut value = activate();
        value["operation_id"] = json!(id);
        reject::<ContinuationActivateRequest>(value);
    }
    for id in [
        "00000000-0000-7000-8000-000000000040",
        "00000000-0000-4000-0000-000000000040",
        "00000000-0000-0000-0000-000000000000",
    ] {
        let mut value = activate();
        value["caller_request_id"] = json!(id);
        reject::<ContinuationActivateRequest>(value);
    }
    let mut value = preview();
    value["selection"]["proposal_input"]["id"] = json!("00000000-0000-4000-8000-0000000000AA");
    reject::<ContinuationPreviewRequest>(value);
}

#[test]
fn pagination_versions_digests_and_reasons_are_bounded() {
    let value: ContinuationGetRequest =
        serde_json::from_value(json!({"operation_id": ID})).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap()["limit"], 100);
    for limit in [json!(0), json!(501), json!(-1), json!(1.5), Value::Null] {
        reject::<ContinuationGetRequest>(json!({"operation_id": ID, "limit": limit}));
    }
    round_trip::<ContinuationGetRequest>(
        json!({"operation_id": ID, "limit": 500, "cursor": "x".repeat(4096)}),
    );
    reject::<ContinuationGetRequest>(json!({"operation_id": ID, "cursor": "x".repeat(4097)}));
    for version in [json!(0), json!(-1), json!(1.5)] {
        let mut value = activate();
        value["expected_version"] = version;
        reject::<ContinuationActivateRequest>(value);
    }
    for hash in [
        "a".repeat(64),
        format!("sha256:{}", "A".repeat(64)),
        "sha256:abc".into(),
    ] {
        let mut value = activate();
        value["expected_manifest_sha256"] = json!(hash);
        reject::<ContinuationActivateRequest>(value);
    }
    for reason in ["".to_owned(), " \n ".to_owned(), "x".repeat(4097)] {
        let mut value = action();
        value["reason"] = json!(reason);
        reject::<ContinuationAbortRequest>(value);
    }
}

#[test]
fn selection_cannot_discard_work_or_smuggle_duplicate_references_and_paths() {
    let mut value = preview();
    value["selection"]["include_dirty_work"] = json!(false);
    reject::<ContinuationPreviewRequest>(value);
    let reference = json!({"kind": "carried_input", "id": ID});
    let mut value = preview();
    value["selection"]["reference_inputs"] = json!([reference, reference]);
    reject::<ContinuationPreviewRequest>(value);
    let refs: Vec<_> = (0..128)
        .map(|i| json!({"kind": "artifact", "id": format!("00000000-0000-4000-8000-{i:012x}")}))
        .collect();
    let mut value = preview();
    value["selection"]["reference_inputs"] = json!(refs);
    round_trip::<ContinuationPreviewRequest>(value.clone());
    value["selection"]["reference_inputs"]
        .as_array_mut()
        .unwrap()
        .push(reference);
    reject::<ContinuationPreviewRequest>(value);
    for path in [
        "../target",
        "/target",
        "target/../src",
        "target//file",
        "target/./file",
        "target\\file",
    ] {
        let mut value = preview();
        value["selection"]["excluded_workspace_paths"] =
            json!([{"path": path, "reason": "generated"}]);
        reject::<ContinuationPreviewRequest>(value);
    }
    let mut value = preview();
    value["selection"]["excluded_workspace_paths"] =
        json!([{"path": "target/", "reason": "generated"}]);
    round_trip::<ContinuationPreviewRequest>(value.clone());
    value["selection"]["excluded_workspace_paths"] = json!((0..128)
        .map(|i| json!({"path": format!("target/generated-{i}"), "reason": "generated"}))
        .collect::<Vec<_>>());
    round_trip::<ContinuationPreviewRequest>(value.clone());
    value["selection"]["excluded_workspace_paths"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path": "target/generated-128", "reason": "generated"}));
    reject::<ContinuationPreviewRequest>(value);
}

#[test]
fn nested_authority_unknown_fields_and_duplicate_keys_fail_closed() {
    let mut value = preview();
    value["target"]["source_status"] = json!("blocked");
    reject::<ContinuationPreviewRequest>(value);
    let mut value = preview();
    value["selection"]["proposal_input"]["authority"] = json!("approved");
    reject::<ContinuationPreviewRequest>(value);
    let raw = serde_json::to_string(&preview()).unwrap().replace(
        "\"include_dirty_work\":true",
        "\"include_dirty_work\":true,\"include_dirty_work\":true",
    );
    assert!(serde_json::from_str::<ContinuationPreviewRequest>(&raw).is_err());
    let raw = serde_json::to_string(&preview()).unwrap().replace(
        "\"kind\":\"artifact\"",
        "\"kind\":\"artifact\",\"kind\":\"artifact\"",
    );
    assert!(serde_json::from_str::<ContinuationPreviewRequest>(&raw).is_err());
    let mut value = preview();
    let original = value["target"]["delivery_configuration_json"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut delivery: Value = serde_json::from_str(&original).unwrap();
    delivery["source_status"] = json!("blocked");
    value["target"]["delivery_configuration_json"] =
        json!(serde_json::to_string(&delivery).unwrap());
    reject::<ContinuationPreviewRequest>(value);
    let mut value = preview();
    value["target"]["delivery_configuration_json"] =
        json!(format!("{{\"repo_root\":\"/repo\",{}", &original[1..]));
    reject::<ContinuationPreviewRequest>(value);
}

#[test]
fn value_conversion_cannot_recover_protocol_duplicate_keys() {
    let raw = serde_json::to_string(&activate()).unwrap();
    let duplicate = format!("{{\"operation_id\":\"{ID}\",{}", &raw[1..]);
    assert!(serde_json::from_str::<ContinuationActivateRequest>(&duplicate).is_err());
    let collapsed: Value = serde_json::from_str(&duplicate).unwrap();
    assert!(serde_json::from_value::<ContinuationActivateRequest>(collapsed).is_ok());
}

fn accepted() -> Value {
    json!({"schema_version": "run_continuation_command_result_v1", "status": "accepted",
        "caller_request_id": REQUEST, "journal_id": REQUEST,
        "operation": {"operation_id": ID, "phase": "preparing", "version": 1, "successor_run_id": null},
        "denial": null, "next_action": "await_worker", "replay_of": null})
}

#[test]
fn command_envelope_is_exact_required_nullable_and_consistent() {
    round_trip::<RunContinuationCommandResultV1>(accepted());
    strict::<RunContinuationCommandResultV1>(accepted());
    for field in accepted().as_object().unwrap().keys() {
        let mut value = accepted();
        value.as_object_mut().unwrap().remove(field);
        reject::<RunContinuationCommandResultV1>(value);
    }
    for (field, wrong) in [
        ("journal_id", Value::Null),
        ("operation", Value::Null),
        ("denial", json!({"code": "source_busy", "message": "busy"})),
        ("replay_of", json!("accepted")),
        (
            "schema_version",
            json!("run_continuation_command_result_v2"),
        ),
    ] {
        let mut value = accepted();
        value[field] = wrong;
        reject::<RunContinuationCommandResultV1>(value);
    }
    let mut denied = accepted();
    denied["status"] = json!("denied");
    denied["journal_id"] = Value::Null;
    denied["operation"] = Value::Null;
    denied["denial"] = json!({"code": "source_busy", "message": "Source is busy"});
    denied["next_action"] = json!("resolve_hold");
    round_trip::<RunContinuationCommandResultV1>(denied.clone());
    denied["denial"]["code"] = json!("arbitrary_failure");
    reject::<RunContinuationCommandResultV1>(denied);
}

#[test]
fn replay_preserves_payload_and_original_journal_without_claiming_current_state() {
    let result: RunContinuationCommandResultV1 = serde_json::from_value(accepted()).unwrap();
    let replay = result.replayed().unwrap();
    let mut expected = accepted();
    expected["status"] = json!("replayed");
    expected["replay_of"] = json!("accepted");
    assert_eq!(serde_json::to_value(&replay).unwrap(), expected);
    round_trip::<RunContinuationCommandResultV1>(expected.clone());
    expected["replay_of"] = json!("replayed");
    reject::<RunContinuationCommandResultV1>(expected);
}

fn readback() -> Value {
    json!({"schema_version": "run_continuation_readback_v1", "operation_id": ID,
        "source_run_id": ID, "successor_run_id": null, "phase": "prepared", "version": 2,
        "plan_sha256": HASH, "manifest_sha256": HASH,
        "execution_disposition": "source_reserved", "verification_status": "verified",
        "holds": [], "next_actions": ["inspect"], "journal_id": REQUEST,
        "updated_at": "2026-09-20T00:00:00Z", "projection_freshness": "fresh",
        "manifest_page": [], "next_cursor": null,
        "admission": {"schema_version": "run_continuation_admission_v1", "enabled": true,
            "reason": null, "fences_enforced": true, "reconcile_available": true}})
}

#[test]
fn readback_retains_unknown_phase_but_never_exposes_actionability() {
    let value: RunContinuationReadbackV1 = serde_json::from_value(readback()).unwrap();
    assert!(!value.actionable_next_actions().is_empty());
    round_trip::<RunContinuationReadbackV1>(readback());
    strict::<RunContinuationReadbackV1>(readback());
    for field in readback().as_object().unwrap().keys() {
        let mut value = readback();
        value.as_object_mut().unwrap().remove(field);
        reject::<RunContinuationReadbackV1>(value);
    }
    let mut unknown = readback();
    unknown["phase"] = json!("future_phase_v9");
    let parsed: RunContinuationReadbackV1 = serde_json::from_value(unknown).unwrap();
    assert!(parsed.actionable_next_actions().is_empty());
    assert_eq!(
        serde_json::to_value(parsed).unwrap()["phase"],
        "future_phase_v9"
    );
    let mut stale = readback();
    stale["projection_freshness"] = json!("stale");
    let parsed: RunContinuationReadbackV1 = serde_json::from_value(stale).unwrap();
    assert!(parsed.actionable_next_actions().is_empty());
    let mut raw = readback();
    raw["schema_version"] = json!("run_continuation_readback_v2");
    reject::<RunContinuationReadbackV1>(raw);
}

#[test]
fn versioned_stale_preview_transport_error_and_links_have_closed_shapes() {
    let stale = json!({"schema_version": "run_carry_forward_preview_stale_v1", "code": "preview_stale", "next_action": "refresh_preview"});
    round_trip::<RunCarryForwardPreviewStaleV1>(stale.clone());
    strict::<RunCarryForwardPreviewStaleV1>(stale);
    let error = json!({"schema_version": "p039_transport_error_v1", "code": "command_outcome_unknown", "next_action": "replay_same_request"});
    round_trip::<P039TransportErrorV1>(error.clone());
    strict::<P039TransportErrorV1>(error);
    round_trip::<RunCarryForwardLinksV1>(
        json!({"schema_version": "run_carry_forward_links_v1", "incoming": null, "outgoing": null}),
    );
    let mut compact = readback();
    compact.as_object_mut().unwrap().remove("manifest_page");
    compact.as_object_mut().unwrap().remove("next_cursor");
    let mut incoming = compact.clone();
    incoming["phase"] = json!("activated");
    incoming["successor_run_id"] = json!(REQUEST);
    let links = json!({"schema_version": "run_carry_forward_links_v1", "incoming": incoming, "outgoing": compact});
    round_trip::<RunCarryForwardLinksV1>(links.clone());
    let mut invalid = links;
    invalid["outgoing"]["manifest_page"] = json!([]);
    reject::<RunCarryForwardLinksV1>(invalid);
}

#[test]
fn raw_request_parser_applies_byte_ceiling_before_deserializing() {
    let raw = serde_json::to_vec(&preview()).unwrap();
    assert!(parse_request::<ContinuationPreviewRequest>(&raw).is_ok());
    let mut oversized = raw;
    oversized.resize(1024 * 1024 + 1, b' ');
    assert!(parse_request::<ContinuationPreviewRequest>(&oversized).is_err());
}

fn entry() -> Value {
    json!({"role": "execution_seed", "entry_id": "proposal-1", "source_namespace": "artifacts",
        "source_artifact_id": ID, "logical_name": "approved_proposal", "source_relative_path": "proposal.md",
        "sha256": HASH, "bytes": 10, "mode": 420, "source_contract": "proposal",
        "source_schema_version": "proposal_v1", "target_logical_name": "proposal_current",
        "reason": "Fresh review input", "historical_approval_relation": "bound"})
}

#[test]
fn inventory_roles_require_hashes_except_excluded_and_reject_unknown_fields() {
    for role in ["execution_seed", "reference_only", "preserve_only"] {
        let mut value = entry();
        value["role"] = json!(role);
        round_trip::<CarryForwardEntryV1>(value.clone());
        strict::<CarryForwardEntryV1>(value.clone());
        value.as_object_mut().unwrap().remove("sha256");
        reject::<CarryForwardEntryV1>(value);
    }
    let excluded = json!({"role": "excluded", "entry_id": "generated-1", "source_namespace": "workspace",
        "source_artifact_id": null, "logical_name": "generated", "source_relative_path": "target/",
        "reason": "Generated output, not read"});
    round_trip::<CarryForwardEntryV1>(excluded.clone());
    let mut invalid = excluded;
    invalid["sha256"] = json!(HASH);
    reject::<CarryForwardEntryV1>(invalid);
}

fn summary() -> Value {
    json!({"source_run_id": ID, "source_witness": {
        "workflow_snapshot_hash": HASH, "catalog_snapshot_hash": HASH,
        "stage_execution_id": ID, "cursor": "implementation", "journal_ids": [], "approval_ids": [],
        "artifact_generation_ids": [], "idea_sha256": HASH, "head": "a".repeat(40), "tree": "b".repeat(40),
        "index_sha256": HASH, "content_sha256": HASH, "directory_identities": [], "settled_ownership_effect_sha256": HASH},
        "target": prepare()["target"], "profile": "implementation_restart_v1", "selection": preview()["selection"],
        "workspace_snapshot": {"base_branch": "main", "base_revision": "a".repeat(40), "head": "a".repeat(40),
            "staged_patch_sha256": HASH, "unstaged_patch_sha256": HASH, "inventory_sha256": HASH,
            "untracked_count": 0, "deleted_count": 0, "link_count": 0, "target_content_sha256": HASH},
        "capability_delta": {"added": ["xcode_headless"], "removed": [], "sha256": HASH}, "holds": [],
        "limits": {"max_entries": 50000, "max_preservation_bytes": 2147483648_u64,
            "max_file_bytes": 268435456, "max_reference_artifacts": 128, "max_request_bytes": 1048576,
            "max_summary_bytes": 65536, "default_page_size": 100, "max_page_size": 500,
            "streaming_buffer_bytes": 1048576}})
}

fn preview_page() -> Value {
    json!({"schema_version": "run_carry_forward_preview_page_v1", "plan_summary": summary(),
        "entries": [entry()], "entry_count": 1, "plan_sha256": HASH, "next_cursor": null})
}

#[test]
fn preview_page_is_bounded_and_carries_full_plan_identity_not_page_digest() {
    round_trip::<RunCarryForwardPreviewPageV1>(preview_page());
    strict::<RunCarryForwardPreviewPageV1>(preview_page());
    let mut value = preview_page();
    value["entries"] = json!([]);
    let page: RunCarryForwardPreviewPageV1 = serde_json::from_value(value).unwrap();
    assert_eq!(page.plan_sha256.as_str(), HASH);
    let mut value = preview_page();
    value["entries"] = json!(vec![entry(); 501]);
    reject::<RunCarryForwardPreviewPageV1>(value);
    let mut value = preview_page();
    value["plan_summary"]["limits"]["max_entries"] = json!(50001);
    reject::<RunCarryForwardPreviewPageV1>(value);
    let mut value = preview_page();
    value["plan_summary"]["holds"] = json!(vec![
        json!({"code": "source_busy", "message": "x".repeat(4096)});
        20
    ]);
    reject::<RunCarryForwardPreviewPageV1>(value);
    for field in ["next_cursor", "entry_count", "plan_sha256"] {
        let mut value = preview_page();
        value.as_object_mut().unwrap().remove(field);
        reject::<RunCarryForwardPreviewPageV1>(value);
    }
}
