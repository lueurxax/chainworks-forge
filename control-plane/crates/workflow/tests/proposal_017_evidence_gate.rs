use std::collections::HashSet;
use std::path::PathBuf;

use serde_json::Value;
use workflow::transition_lint::{
    scan_workflow_file_for_simultaneous_transitions, SimultaneousTransitionFinding,
};

const KNOWN_ISSUES_REQUIRED_FIELDS: &[&str] = &[
    "record_id",
    "workflow_path",
    "workflow_id",
    "from_state_id",
    "transition_ids",
    "expected_conflict_reason",
    "operator_label",
    "why_not_reauthored_before_merge",
    "mitigation_or_followup_issue",
    "approver",
    "approved_at",
    "expires_at_or_release",
];

#[test]
fn p017_phase_b_dogfood_exit_record_has_operator_approved_flag_gated_evidence() {
    let record = read_json("docs/reference/workflow-conflict-evidence/phase-b-dogfood-exit-record.json");

    assert_eq!(string_at(&record, "proposal_id"), "P017");
    assert_eq!(string_at(&record, "phase"), "Phase B");
    assert_eq!(
        string_at(&record, "runtime_flag_decision"),
        "remain_flag_gated"
    );
    assert_eq!(string_at(&record, "operator_approval.decision"), "approved");
    assert_non_empty_string(&record, "operator_approval.approver");
    assert_non_empty_string(&record, "operator_approval.approved_at");

    let runs = array_at(&record, "dogfood_runs");
    assert!(
        runs.len() >= 10,
        "Phase B dogfood exit record must include at least 10 mediation-exercising runs"
    );

    let organic_count = runs
        .iter()
        .filter(|run| string_at(run, "scenario_source") == "organic")
        .count();
    assert!(
        organic_count >= 5,
        "Phase B dogfood exit record must include at least five organic operator workflow runs"
    );

    let workflows: HashSet<_> = runs
        .iter()
        .map(|run| string_at(run, "workflow_id").to_string())
        .collect();
    assert!(
        workflows.len() >= 2,
        "Phase B dogfood exit record must cover at least two workflows"
    );

    let reasons: HashSet<_> = runs
        .iter()
        .map(|run| string_at(run, "conflict_reason").to_string())
        .collect();
    assert!(
        reasons.contains("no_declarative_transition_matched")
            || reasons.contains("required_artifact_or_field_missing_for_transition"),
        "Phase B dogfood exit record must cover no-match or missing-input conflicts"
    );
    assert!(
        reasons.contains("same_run_continue"),
        "Phase B dogfood exit record must cover same-run continuation"
    );
    assert!(
        reasons.contains("terminal_unverifiable")
            || reasons.contains("operator_confirmation_required"),
        "Phase B dogfood exit record must cover terminal-unverifiable or operator-confirmation paths"
    );

    let completion_rate = number_at(&record, "gate_results.completion_rate_observed");
    assert!(
        completion_rate >= 0.9,
        "Phase B dogfood completion rate must be at least 90%"
    );
    assert_eq!(
        number_at(&record, "gate_results.duplicate_mediation_sessions"),
        0.0
    );
    assert_eq!(
        number_at(&record, "gate_results.stage_scoped_readback_leaks"),
        0.0
    );
    assert!(
        number_at(&record, "gate_results.operator_guidance_sufficient_rate") >= 0.8,
        "Phase B operator guidance sufficiency must be at least 80%"
    );
    assert_non_empty_string(&record, "gate_results.time_to_resolution_comparison");
}

#[test]
fn p017_phase_c_external_catalog_inventory_has_operator_attestation_and_warning_decision() {
    let inventory = read_json(
        "docs/reference/workflow-conflict-evidence/phase-c-external-catalog-enforcement-inventory.json",
    );

    assert_eq!(string_at(&inventory, "proposal_id"), "P017");
    assert_eq!(string_at(&inventory, "phase"), "Phase C");
    assert_eq!(
        string_at(&inventory, "phase_c_enforcement_status"),
        "flag_gated"
    );
    assert_eq!(
        string_at(&inventory, "operator_approval.decision"),
        "approved"
    );

    assert!(
        number_at(
            &inventory,
            "bundled_catalog_scan.simultaneous_transition_findings_count"
        ) == 0.0,
        "bundled catalog scan must currently record zero simultaneous-transition findings"
    );
    assert!(
        !array_at(&inventory, "bundled_catalog_scan.scanned_workflow_paths").is_empty(),
        "bundled catalog scan must name scanned workflow paths"
    );

    let attestation = value_at(
        &inventory,
        "external_catalog_discovery.operator_attestation",
    );
    for field in [
        "attestor",
        "attested_at",
        "scanned_paths",
        "catalog_count",
        "active_external_catalog_count",
        "last_used_evidence",
        "unknown_coverage_risks",
        "warning_window_decision",
        "approval_ref",
    ] {
        assert!(
            has_path(attestation, field),
            "operator attestation is missing required field {field}"
        );
    }

    if number_at(attestation, "active_external_catalog_count") == 0.0 {
        assert_eq!(
            string_at(attestation, "warning_window_decision"),
            "waive_warning_window_no_active_external_catalogs"
        );
        assert_eq!(
            bool_at(attestation, "unknown_coverage_risks_accepted_by_operator"),
            true
        );
    }

    let warnings = array_at(&inventory, "typed_migration_warnings");
    assert!(
        !warnings.is_empty(),
        "Phase C inventory must include typed migration warning evidence"
    );
    for warning in warnings {
        for field in ["warning_code", "severity", "target", "operator_message"] {
            assert!(
                has_path(warning, field),
                "typed migration warning is missing required field {field}"
            );
        }
    }
}

#[test]
fn p017_zero_bundled_simultaneous_findings_pass_without_known_issues_records() {
    let known_issues =
        read_json("docs/reference/workflow-conflict-evidence/phase-a-known-issues-migration-records.json");

    let required_fields = array_at(&known_issues, "schema.required_fields");
    let actual_required_fields: HashSet<_> = required_fields
        .iter()
        .map(|field| {
            field
                .as_str()
                .expect("required field names must be strings")
        })
        .collect();
    for required_field in KNOWN_ISSUES_REQUIRED_FIELDS {
        assert!(
            actual_required_fields.contains(required_field),
            "known-issues schema is missing required field {required_field}"
        );
    }

    let records = array_at(&known_issues, "records");
    assert!(
        records.is_empty(),
        "current bundled scan has no simultaneous-transition findings, so no known-issues records should be needed"
    );
    assert_eq!(
        string_at(&known_issues, "operator_approval.decision"),
        "approved"
    );

    let findings = bundled_simultaneous_transition_findings();
    assert!(
        findings.is_empty(),
        "test fixture expects current bundled workflows to have zero simultaneous-transition findings: {findings:#?}"
    );
    assert!(
        known_issues_gate_allows(&findings, records),
        "zero simultaneous-transition findings must pass without known-issues records"
    );
}

fn bundled_simultaneous_transition_findings() -> Vec<SimultaneousTransitionFinding> {
    let root = repo_root();
    let workflow_dir = root.join("examples").join("workflows");
    let workflow_paths = [
        workflow_dir.join("workflow.yaml"),
        workflow_dir.join("full-mvp-live.yaml"),
        workflow_dir.join("proposal-loop-live.yaml"),
        workflow_dir.join("proposal-to-release.yaml"),
    ];

    let mut findings = Vec::new();
    for workflow_path in workflow_paths {
        findings.extend(
            scan_workflow_file_for_simultaneous_transitions(&workflow_path)
                .unwrap_or_else(|err| panic!("scan {}: {err:#}", workflow_path.display())),
        );
    }
    findings
}

fn known_issues_gate_allows(findings: &[SimultaneousTransitionFinding], records: &[Value]) -> bool {
    if findings.is_empty() {
        return records.is_empty();
    }

    findings.iter().all(|finding| {
        records.iter().any(|record| {
            string_at(record, "workflow_path") == finding.workflow_path
                && string_at(record, "from_state_id") == finding.state_id
                && string_at(record, "expected_conflict_reason")
                    == "multiple_declarative_transitions_matched_without_tie_break"
                && string_at(record, "approval_status") == "approved"
                && KNOWN_ISSUES_REQUIRED_FIELDS
                    .iter()
                    .all(|field| has_path(record, field))
        })
    })
}

fn read_json(relative_path: &str) -> Value {
    let path = repo_root().join(relative_path);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err:#}", path.display()));
    serde_json::from_str(&raw).unwrap_or_else(|err| panic!("parse {}: {err:#}", path.display()))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn value_at<'a>(value: &'a Value, path: &str) -> &'a Value {
    let mut current = value;
    for segment in path.split('.') {
        current = current
            .get(segment)
            .unwrap_or_else(|| panic!("missing JSON path {path}"));
    }
    current
}

fn string_at<'a>(value: &'a Value, path: &str) -> &'a str {
    value_at(value, path)
        .as_str()
        .unwrap_or_else(|| panic!("JSON path {path} must be a string"))
}

fn number_at(value: &Value, path: &str) -> f64 {
    value_at(value, path)
        .as_f64()
        .unwrap_or_else(|| panic!("JSON path {path} must be a number"))
}

fn bool_at(value: &Value, path: &str) -> bool {
    value_at(value, path)
        .as_bool()
        .unwrap_or_else(|| panic!("JSON path {path} must be a boolean"))
}

fn array_at<'a>(value: &'a Value, path: &str) -> &'a [Value] {
    value_at(value, path)
        .as_array()
        .unwrap_or_else(|| panic!("JSON path {path} must be an array"))
}

fn assert_non_empty_string(value: &Value, path: &str) {
    assert!(
        !string_at(value, path).trim().is_empty(),
        "JSON path {path} must be a non-empty string"
    );
}

fn has_path(value: &Value, path: &str) -> bool {
    let mut current = value;
    for segment in path.split('.') {
        let Some(next) = current.get(segment) else {
            return false;
        };
        current = next;
    }
    true
}
