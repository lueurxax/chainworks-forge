//! Synthetic in-memory validator cases, never installed-host release evidence.
use domain::xcode_contract::{canonical_digest, manifest_digest, trust_policy_digest};
use domain::xcode_release::{
    validate_release_receipt, HoldReason, ReleaseCandidate, ReleaseError, ReleaseVerdict,
    MAX_EVIDENCE_BYTES, MAX_EVIDENCE_FILES, MAX_RECEIPT_BYTES, MAX_TOTAL_EVIDENCE_BYTES,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const NOW: &str = "2026-09-20T12:00:00Z";
const CHECKS: &[(&str, &str, &[&str])] = &[
    (
        "ide_absent",
        "installed_host",
        &["ide_absent_before_and_after"],
    ),
    (
        "packaged_launch_identity",
        "installed_host",
        &["packaged_signing_and_launch_chain"],
    ),
    (
        "consent_scope",
        "installed_host",
        &[
            "exact_principal_and_folder",
            "global_permission_disabled",
            "no_automatic_grants",
        ],
    ),
    (
        "cold_start",
        "installed_host",
        &["selected_service_acquired", "foreign_clients_untouched"],
    ),
    (
        "warm_attach",
        "installed_host",
        &["no_ide_or_service_restart"],
    ),
    (
        "project_binding",
        "installed_host",
        &[
            "initial_structured_id_and_exact_path",
            "same_service_generation",
            "fresh_exact_path_presence",
            "explicit_id_read_via_broker_lease",
        ],
    ),
    (
        "foreign_project_rejection",
        "fixture",
        &["foreign_selectors_handles_paths_denied_via_lease"],
    ),
    (
        "trusted_project_admission",
        "fixture",
        &[
            "operator_trust_pins_root_project_uid_installation_generation_mapping",
            "evaluated_permissions_and_tool_allowlist",
            "absent_revoked_mismatched_authority_denied",
        ],
    ),
    (
        "scope_controls",
        "fixture",
        &[
            "closed_tested_adapters",
            "explicit_bound_ids_and_path_handle_rules",
            "fresh_drift_checks",
            "unknown_default_prose_and_close_reopen_denied",
            "blind_interval_and_external_references_acknowledged",
        ],
    ),
    (
        "reuse_revalidation",
        "fixture",
        &["stale_project_service_policy_denied_before_prompt"],
    ),
    (
        "broker_shim_exclusion",
        "fixture",
        &["shared_coordinator", "concurrent_effect_dispatch_excluded"],
    ),
    (
        "restart_recovery",
        "fixture",
        &[
            "stale_leases_invalidated",
            "uncertainty_restored",
            "zero_automatic_replay",
        ],
    ),
    (
        "effect_dedupe",
        "fixture",
        &[
            "repeat_nonce_and_new_key_zero_second_dispatch",
            "fault_boundaries_and_journal_transitions",
        ],
    ),
    (
        "public_redaction",
        "fixture",
        &["health_and_errors_no_private_details"],
    ),
    (
        "mixed_versions",
        "fixture",
        &["old_new_clients_and_future_values"],
    ),
    (
        "canonical_gates",
        "repository_gate",
        &["required_gates_passed", "remote_only_ui_policy"],
    ),
    (
        "required_capabilities",
        "fixture",
        &["frozen_workflow_inventory_and_admitted_routes"],
    ),
];

fn digest(ch: char) -> String {
    ch.to_string().repeat(64)
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn seal(receipt: &Value) -> Vec<u8> {
    let mut receipt = receipt.clone();
    receipt.as_object_mut().unwrap().remove("receipt_digest");
    receipt["receipt_digest"] = json!(canonical_digest("cw.xcode.receipt.v1", &receipt).unwrap());
    serde_json::to_vec(&receipt).unwrap()
}

fn fixture() -> (Value, ReleaseCandidate, BTreeMap<String, Vec<u8>>) {
    let candidate = ReleaseCandidate {
        identity: json!({
            "release_id":"test-candidate-001", "source_commit":"a".repeat(40),
            "source_content_digest":digest('b'),
            "app":{"version":"1.0", "build":"123", "bundle_id":"test.chainworks.app", "signing_team":"TESTTEAM", "designated_requirement_digest":digest('c'), "artifact_digest":digest('d')},
            "daemon":{"version":"0.1.0", "build_sha":"a".repeat(40), "artifact_digest":digest('e'), "signing_requirement_digest":digest('f'), "db_schema_version":"100"},
            "xcode":{"version":"26.3", "build":"17C100", "toolchain_identity_digest":digest('a'), "service_bundle_id":"test.xcode.service", "service_build":"25317"},
            "launch_identity":{"mode":"packaged", "uid_pseudonym":"host-user-1", "daemon_executable_digest":digest('e'), "bridge_executable_digest":digest('b'), "daemon_signing_requirement_digest":digest('f'), "bridge_signing_requirement_digest":digest('c')},
            "service_generation":{"boot_identity":"boot-1", "start_identity":"start-1", "pid":"1234", "bundle_identity_digest":digest('d'), "executable_identity_digest":digest('e')},
            "contract":{"manifest_digest":manifest_digest().unwrap(), "schema_bundle_digest":digest('f'), "effective_policy_digest":digest('a'), "binding_digest":digest('b'), "trust_policy_id":"trusted_local_project_v1", "trust_policy_digest":trust_policy_digest().unwrap()},
            "consent_identity":{"principal_digest":digest('c'), "folder_scope_digest":digest('d')}
        }),
        required_capabilities: json!([
            {"capability":"XcodeRead", "required_by":"workflow-1", "admitted_routes":["headless_mcp"]}
        ]),
        unresolved_attempt_count: 0,
    };
    let source = candidate.source_identity_digest().unwrap();
    let mut receipt = candidate.identity.clone();
    let consent = receipt
        .as_object_mut()
        .unwrap()
        .remove("consent_identity")
        .unwrap();
    receipt["schema_version"] = json!(1);
    receipt["generated_at"] = json!(NOW);
    receipt["consent"] = json!({"status":"approved", "principal_digest":consent["principal_digest"], "folder_scope_digest":consent["folder_scope_digest"], "evidence_ref":"consent_scope.json"});
    receipt["checks"] = json!({});
    receipt["evidence_manifest"] = json!({});
    let mut evidence = BTreeMap::new();
    for (name, kind, assertions) in CHECKS {
        let path = format!("{name}.json");
        let observation = json!({"schema_version":1, "contract_revision":4,
            "source_identity_digest":source, "observed_at":NOW, "kind":kind,
            "checks":{*name:{"status":"pass", "assertions":assertions}},
            "capabilities": if *name == "project_binding" { json!([{"capability":"XcodeRead", "required_by":"workflow-1", "route":"headless_mcp", "status":"pass"}]) } else {json!([])}
        });
        let bytes = serde_json::to_vec(&observation).unwrap();
        receipt["evidence_manifest"][&path] = json!(sha(&bytes));
        receipt["checks"][*name] = json!({"status":"pass", "observed_at":NOW,
            "evidence_refs":[path], "details":"Redacted synthetic observation.", "source_identity_digest":source});
        evidence.insert(path, bytes);
    }
    receipt["capabilities"] = json!([{"capability":"XcodeRead", "required_by":"workflow-1", "route":"headless_mcp", "status":"pass", "evidence_refs":["project_binding.json"]}]);
    receipt["unresolved_attempt_count"] = json!(0);
    receipt["verdict"] = json!("pass");
    (receipt, candidate, evidence)
}

fn rewrite_evidence(
    receipt: &mut Value,
    evidence: &mut BTreeMap<String, Vec<u8>>,
    path: &str,
    edit: impl FnOnce(&mut Value),
) {
    let mut value: Value = serde_json::from_slice(&evidence[path]).unwrap();
    edit(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap();
    receipt["evidence_manifest"][path] = json!(sha(&bytes));
    evidence.insert(path.to_owned(), bytes);
}

#[test]
fn complete_identity_bound_synthetic_evidence_passes_validation_only() {
    let (receipt, candidate, evidence) = fixture();
    let validated = validate_release_receipt(&seal(&receipt), &candidate, &evidence).unwrap();
    assert_eq!(validated.verdict, ReleaseVerdict::Pass);
    assert!(validated.holds.is_empty());
}

#[test]
fn missing_or_unknown_checks_and_fields_are_rejected() {
    let (receipt, candidate, evidence) = fixture();
    for field in receipt.as_object().unwrap().keys() {
        let mut changed = receipt.clone();
        changed.as_object_mut().unwrap().remove(field);
        assert!(
            validate_release_receipt(&seal(&changed), &candidate, &evidence).is_err(),
            "removed {field}"
        );
    }
    for (name, _, _) in CHECKS {
        let mut changed = receipt.clone();
        changed["checks"].as_object_mut().unwrap().remove(*name);
        assert!(validate_release_receipt(&seal(&changed), &candidate, &evidence).is_err());
    }
    for pointer in [
        "",
        "/app",
        "/daemon",
        "/xcode",
        "/launch_identity",
        "/service_generation",
        "/contract",
        "/consent",
        "/checks",
        "/checks/ide_absent",
        "/capabilities/0",
    ] {
        let mut changed = receipt.clone();
        changed
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!(true));
        assert!(
            validate_release_receipt(&seal(&changed), &candidate, &evidence).is_err(),
            "extra field {pointer}"
        );
    }
}

#[test]
fn incomplete_checks_hold_and_cannot_be_promoted_by_manual_pass() {
    let (receipt, candidate, evidence) = fixture();
    for status in ["unknown", "not_run", "fail"] {
        let mut changed = receipt.clone();
        changed["checks"]["cold_start"]["status"] = json!(status);
        changed["verdict"] = json!("hold");
        let result = validate_release_receipt(&seal(&changed), &candidate, &evidence).unwrap();
        assert_eq!(result.verdict, ReleaseVerdict::Hold);
        assert!(result.holds.contains(&HoldReason::IncompleteChecks));
        changed["verdict"] = json!("pass");
        assert_eq!(
            validate_release_receipt(&seal(&changed), &candidate, &evidence),
            Err(ReleaseError::VerdictMismatch)
        );
    }
}

#[test]
fn missing_modified_or_unreferenced_evidence_never_supplies_a_pass() {
    for mode in [
        "missing",
        "modified",
        "unreferenced",
        "empty_refs",
        "unknown_ref",
    ] {
        let (mut receipt, candidate, mut evidence) = fixture();
        match mode {
            "missing" => {
                evidence.remove("cold_start.json");
            }
            "modified" => {
                evidence.get_mut("cold_start.json").unwrap().push(b' ');
            }
            "unreferenced" => {
                receipt["evidence_manifest"]
                    .as_object_mut()
                    .unwrap()
                    .remove("cold_start.json");
            }
            "empty_refs" => receipt["checks"]["cold_start"]["evidence_refs"] = json!([]),
            _ => receipt["checks"]["cold_start"]["evidence_refs"] = json!(["missing.json"]),
        }
        assert!(
            validate_release_receipt(&seal(&receipt), &candidate, &evidence).is_err(),
            "{mode}"
        );
        receipt["verdict"] = json!("hold");
        assert_eq!(
            validate_release_receipt(&seal(&receipt), &candidate, &evidence)
                .unwrap()
                .verdict,
            ReleaseVerdict::Hold,
            "{mode}"
        );
    }
}

#[test]
fn receipt_digest_duplicate_keys_future_versions_and_invalid_numbers_are_rejected() {
    let (receipt, candidate, evidence) = fixture();
    let mut sealed: Value = serde_json::from_slice(&seal(&receipt)).unwrap();
    sealed["app"]["build"] = json!("999");
    assert_eq!(
        validate_release_receipt(&serde_json::to_vec(&sealed).unwrap(), &candidate, &evidence),
        Err(ReleaseError::ReceiptDigestMismatch)
    );
    for (pointer, value) in [
        ("/schema_version", json!(2)),
        ("/schema_version", json!(1.0)),
        ("/unresolved_attempt_count", json!(-1)),
        ("/unresolved_attempt_count", json!(9007199254740992u64)),
        ("/checks/cold_start/status", json!("skipped")),
        ("/app/artifact_digest", json!("A".repeat(64))),
    ] {
        let mut changed = receipt.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        let bytes = if pointer == "/unresolved_attempt_count" {
            serde_json::to_vec(&changed).unwrap()
        } else {
            seal(&changed)
        };
        assert!(validate_release_receipt(&bytes, &candidate, &evidence).is_err());
    }
    let bytes = String::from_utf8(seal(&receipt)).unwrap();
    let duplicate = bytes.replacen(
        "\"status\":\"pass\"",
        "\"status\":\"pass\",\"status\":\"pass\"",
        1,
    );
    assert!(validate_release_receipt(duplicate.as_bytes(), &candidate, &evidence).is_err());
    assert!(validate_release_receipt(b"{} {}", &candidate, &evidence).is_err());
}

#[test]
fn candidate_source_artifact_contract_and_consent_identity_mismatches_hold() {
    let (receipt, candidate, evidence) = fixture();
    for pointer in [
        "/release_id",
        "/source_commit",
        "/source_content_digest",
        "/app/artifact_digest",
        "/daemon/artifact_digest",
        "/contract/manifest_digest",
        "/contract/schema_bundle_digest",
        "/contract/effective_policy_digest",
        "/contract/binding_digest",
        "/consent/principal_digest",
        "/consent/folder_scope_digest",
        "/service_generation/executable_identity_digest",
    ] {
        let mut changed = receipt.clone();
        *changed.pointer_mut(pointer).unwrap() = json!(if pointer == "/release_id" {
            "another-candidate".into()
        } else if pointer == "/source_commit" {
            "0".repeat(40)
        } else {
            digest('0')
        });
        changed["verdict"] = json!("hold");
        let result = validate_release_receipt(&seal(&changed), &candidate, &evidence).unwrap();
        assert!(
            result.holds.contains(&HoldReason::IdentityMismatch),
            "{pointer}"
        );
        changed["verdict"] = json!("pass");
        assert!(validate_release_receipt(&seal(&changed), &candidate, &evidence).is_err());
    }
}

#[test]
fn wrong_evidence_source_kind_revision_or_observation_time_never_passes() {
    for (pointer, value) in [
        ("/source_identity_digest", json!(digest('0'))),
        ("/kind", json!("fixture")),
        ("/contract_revision", json!(3)),
        ("/observed_at", json!("2026-09-20T12:00:01Z")),
        ("/checks/project_binding/status", json!("fail")),
    ] {
        let (mut receipt, candidate, mut evidence) = fixture();
        rewrite_evidence(&mut receipt, &mut evidence, "project_binding.json", |v| {
            *v.pointer_mut(pointer).unwrap() = value
        });
        assert!(
            validate_release_receipt(&seal(&receipt), &candidate, &evidence).is_err(),
            "{pointer}"
        );
    }
    let (mut receipt, candidate, evidence) = fixture();
    receipt["checks"]["cold_start"]["source_identity_digest"] = json!(digest('0'));
    receipt["verdict"] = json!("hold");
    assert_eq!(
        validate_release_receipt(&seal(&receipt), &candidate, &evidence)
            .unwrap()
            .verdict,
        ReleaseVerdict::Hold
    );
}

#[test]
fn dev_launch_denied_unknown_consent_and_unresolved_journal_count_hold() {
    for mode in [
        "development",
        "denied",
        "unknown",
        "receipt_attempt",
        "journal_attempt",
    ] {
        let (mut receipt, mut candidate, evidence) = fixture();
        match mode {
            "development" => receipt["launch_identity"]["mode"] = json!("development"),
            "denied" | "unknown" => receipt["consent"]["status"] = json!(mode),
            "receipt_attempt" => receipt["unresolved_attempt_count"] = json!(1),
            _ => candidate.unresolved_attempt_count = 1,
        }
        receipt["verdict"] = json!("hold");
        assert_eq!(
            validate_release_receipt(&seal(&receipt), &candidate, &evidence)
                .unwrap()
                .verdict,
            ReleaseVerdict::Hold,
            "{mode}"
        );
        receipt["verdict"] = json!("pass");
        assert!(validate_release_receipt(&seal(&receipt), &candidate, &evidence).is_err());
    }
}

#[test]
fn trust_policy_and_required_admission_binding_scope_proofs_cannot_be_substituted() {
    let (receipt, candidate, evidence) = fixture();
    for pointer in ["/contract/trust_policy_id", "/contract/trust_policy_digest"] {
        let mut changed = receipt.clone();
        *changed.pointer_mut(pointer).unwrap() = json!(if pointer.ends_with("_id") {
            "other_policy".into()
        } else {
            digest('0')
        });
        assert!(validate_release_receipt(&seal(&changed), &candidate, &evidence).is_err());
    }
    for name in [
        "trusted_project_admission",
        "project_binding",
        "scope_controls",
    ] {
        let (mut receipt, candidate, mut evidence) = fixture();
        rewrite_evidence(&mut receipt, &mut evidence, &format!("{name}.json"), |v| {
            v["checks"][name]["assertions"]
                .as_array_mut()
                .unwrap()
                .remove(0);
        });
        receipt["verdict"] = json!("hold");
        assert_eq!(
            validate_release_receipt(&seal(&receipt), &candidate, &evidence)
                .unwrap()
                .verdict,
            ReleaseVerdict::Hold
        );
        receipt["verdict"] = json!("pass");
        assert!(validate_release_receipt(&seal(&receipt), &candidate, &evidence).is_err());
    }
}

#[test]
fn capabilities_are_taken_from_candidate_not_the_receipts_enabled_tools() {
    for mode in [
        "missing",
        "extra",
        "foreign_workflow",
        "wrong_route",
        "not_run",
        "no_evidence",
        "no_admitted_route",
    ] {
        let (mut receipt, mut candidate, evidence) = fixture();
        match mode {
            "missing" => receipt["capabilities"] = json!([]),
            "extra" => receipt["capabilities"].as_array_mut().unwrap().push(json!({"capability":"XcodeBuild", "required_by":"workflow-1", "route":"headless_mcp", "status":"pass", "evidence_refs":["project_binding.json"]})),
            "foreign_workflow" => receipt["capabilities"][0]["required_by"] = json!("workflow-2"),
            "wrong_route" => receipt["capabilities"][0]["route"] = json!("existing_filesystem"),
            "not_run" => receipt["capabilities"][0]["status"] = json!("not_run"),
            "no_evidence" => receipt["capabilities"][0]["evidence_refs"] = json!([]),
            _ => candidate.required_capabilities[0]["admitted_routes"] = json!([]),
        }
        receipt["verdict"] = json!("hold");
        let result = validate_release_receipt(&seal(&receipt), &candidate, &evidence).unwrap();
        assert!(
            result.holds.contains(&HoldReason::CapabilityCoverage),
            "{mode}"
        );
        receipt["verdict"] = json!("pass");
        assert!(validate_release_receipt(&seal(&receipt), &candidate, &evidence).is_err());
    }
}

#[test]
fn raw_paths_tokens_payloads_and_escaping_evidence_paths_are_rejected_without_echo() {
    let (receipt, candidate, evidence) = fixture();
    for forbidden in [
        "/Users/private/project",
        "C:\\private\\project",
        "~/private",
        "https://mutable.example/evidence",
        "Bearer confidential-token",
        "token=confidential",
        "-----BEGIN PRIVATE KEY-----",
        "{\"openWorkspaces\":[]}",
        "run `sudo command`",
        "safe\nunsafe",
    ] {
        let mut changed = receipt.clone();
        changed["checks"]["cold_start"]["details"] = json!(forbidden);
        let error = validate_release_receipt(&seal(&changed), &candidate, &evidence).unwrap_err();
        assert!(!error.to_string().contains(forbidden));
    }
    for path in [
        "../outside.json",
        "/tmp/evidence.json",
        "a/../evidence.json",
        "a//evidence.json",
        "./evidence.json",
        "a\\evidence.json",
        "https://evidence",
        "a/%2e%2e/evidence.json",
    ] {
        let mut changed = receipt.clone();
        changed["evidence_manifest"][path] = json!(digest('0'));
        assert!(
            validate_release_receipt(&seal(&changed), &candidate, &evidence).is_err(),
            "{path}"
        );
    }
}

#[test]
fn bounds_apply_before_receipt_and_evidence_decoding() {
    let (receipt, candidate, evidence) = fixture();
    assert_eq!(
        validate_release_receipt(&vec![b' '; MAX_RECEIPT_BYTES + 1], &candidate, &evidence),
        Err(ReleaseError::InputTooLarge)
    );
    let mut oversized = evidence.clone();
    oversized.insert("large.json".into(), vec![b' '; MAX_EVIDENCE_BYTES + 1]);
    assert_eq!(
        validate_release_receipt(&seal(&receipt), &candidate, &oversized),
        Err(ReleaseError::InputTooLarge)
    );
    let mut too_many = BTreeMap::new();
    for n in 0..=MAX_EVIDENCE_FILES {
        too_many.insert(format!("{n}.json"), vec![]);
    }
    assert_eq!(
        validate_release_receipt(&seal(&receipt), &candidate, &too_many),
        Err(ReleaseError::InputTooLarge)
    );
    let mut total = BTreeMap::new();
    for n in 0..=(MAX_TOTAL_EVIDENCE_BYTES / MAX_EVIDENCE_BYTES) {
        total.insert(format!("{n}.json"), vec![b' '; MAX_EVIDENCE_BYTES]);
    }
    assert_eq!(
        validate_release_receipt(&seal(&receipt), &candidate, &total),
        Err(ReleaseError::InputTooLarge)
    );
}
