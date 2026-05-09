/// P058 Phase 0 — escalation_policy_v1 YAML schema compile validation.
///
/// Validates that:
/// - The proposal's example policy parses successfully.
/// - Unknown top-level and tier fields are rejected at parse time (deny_unknown_fields).
/// - All required field validations fire correctly.
/// - Policy hash is deterministic and prefixed "sha256:".
/// - `validate_policies_against_catalog` rejects unknown backend_profile_id references.
/// - A catalog with escalation_policies round-trips through `catalog::load`-style parsing.
/// - The compiler integrates the policies into RunPlan without breaking existing tests.
use workflow::escalation_policy::{
    compute_policy_hash, parse_policy, validate_policies_against_catalog, AppliesToYaml,
    EscalationPolicyYaml, EscalationTierYaml, EscalationTriggerYaml, SCHEMA_VERSION,
};

// ── parse_policy tests ────────────────────────────────────────────────────────

#[test]
fn p058_policy_parse_full_proposal_example() {
    let yaml = r#"
policy_id: code_writer_default_escalation
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: code_writer
max_chain_attempts: 6
max_chain_wall_clock_seconds: 7200
triggers:
  - repeated_same_blocker_digest
  - contract_output_failure
  - stale_no_output
  - provider_quota_exhausted
  - transport_failure
  - loop_budget_threshold
tiers:
  - tier_id: primary_retry
    kind: same_backend_retry
    max_attempts: 2
  - tier_id: frontier_profile
    kind: backend_profile
    backend_profile_id: claude_builder_frontier
    max_attempts: 1
  - tier_id: codex_profile
    kind: backend_profile
    backend_profile_id: codex_implementer_high
    max_attempts: 1
  - tier_id: lead_review
    kind: lead_mediation
    max_attempts: 1
  - tier_id: human_pause
    kind: pause
"#;
    let policy = parse_policy(yaml).expect("proposal example policy must parse");
    assert_eq!(policy.policy_id, "code_writer_default_escalation");
    assert_eq!(policy.schema_version, SCHEMA_VERSION);
    assert!(!policy.enabled_default);
    assert_eq!(policy.applies_to.agent_id.as_deref(), Some("code_writer"));
    assert_eq!(policy.max_chain_attempts, 6);
    assert_eq!(policy.max_chain_wall_clock_seconds, 7200);
    assert_eq!(policy.triggers.len(), 6);
    assert_eq!(policy.tiers.len(), 5);
    assert_eq!(policy.tiers[0].kind, "same_backend_retry");
    assert_eq!(policy.tiers[0].max_attempts, Some(2));
    assert_eq!(policy.tiers[1].kind, "backend_profile");
    assert_eq!(
        policy.tiers[1].backend_profile_id.as_deref(),
        Some("claude_builder_frontier")
    );
    assert_eq!(policy.tiers[4].kind, "pause");
    assert!(policy.tiers[4].max_attempts.is_none());
}

#[test]
fn p058_policy_parse_rejects_unknown_top_level_field() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
unknown_key_that_should_fail: yes
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown") || err.to_string().contains("parse error"),
        "expected parse error for unknown field; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_unknown_tier_field() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
    extra_tier_field: not_allowed
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown") || err.to_string().contains("parse error"),
        "expected parse error for unknown tier field; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_unknown_applies_to_field() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
  unknown_selector: not_allowed
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown") || err.to_string().contains("parse error"),
        "expected parse error for unknown applies_to field; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_wrong_schema_version() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v99_future
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("schema_version"),
        "error must mention schema_version; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_empty_tiers() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers: []
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(err.to_string().contains("tiers"), "got: {err}");
}

#[test]
fn p058_policy_parse_rejects_empty_triggers() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers: []
tiers:
  - tier_id: t1
    kind: pause
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(err.to_string().contains("triggers"), "got: {err}");
}

#[test]
fn p058_policy_parse_rejects_backend_profile_tier_without_profile_id() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 2
max_chain_wall_clock_seconds: 120
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: backend_profile
    max_attempts: 1
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("backend_profile_id"),
        "must mention backend_profile_id; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_same_backend_retry_without_max_attempts() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 2
max_chain_wall_clock_seconds: 120
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: same_backend_retry
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("max_attempts"),
        "must mention max_attempts; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_lead_mediation_without_max_attempts() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 2
max_chain_wall_clock_seconds: 120
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: lead_mediation
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("max_attempts"),
        "must mention max_attempts; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_unknown_tier_kind() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: future_tier_kind_v99
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(
        err.to_string().contains("unknown kind"),
        "must mention unknown kind; got: {err}"
    );
}

#[test]
fn p058_policy_parse_rejects_empty_applies_to() {
    let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: false
applies_to: {}
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
"#;
    let err = parse_policy(yaml).unwrap_err();
    assert!(err.to_string().contains("applies_to"), "got: {err}");
}

// ── compute_policy_hash tests ─────────────────────────────────────────────────

#[test]
fn p058_policy_hash_is_deterministic_and_prefixed() {
    let yaml = r#"
policy_id: hash_test
schema_version: escalation_policy_v1
enabled_default: true
applies_to:
  agent_id: code_writer
max_chain_attempts: 3
max_chain_wall_clock_seconds: 1800
triggers:
  - stale_no_output
tiers:
  - tier_id: retry
    kind: same_backend_retry
    max_attempts: 1
  - tier_id: stop
    kind: pause
"#;
    let policy = parse_policy(yaml).unwrap();
    let h1 = compute_policy_hash(&policy).unwrap();
    let h2 = compute_policy_hash(&policy).unwrap();
    assert_eq!(h1, h2, "hash must be deterministic");
    assert!(h1.starts_with("sha256:"), "hash must be prefixed; got: {h1}");
    assert_eq!(h1.len(), 7 + 64, "sha256: prefix + 64 hex chars");
}

#[test]
fn p058_policy_hash_differs_for_different_policies() {
    let make = |policy_id: &str| -> String {
        let yaml = format!(
            r#"
policy_id: {policy_id}
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: agent_x
max_chain_attempts: 2
max_chain_wall_clock_seconds: 600
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
"#
        );
        let p = parse_policy(&yaml).unwrap();
        compute_policy_hash(&p).unwrap()
    };
    assert_ne!(make("policy_a"), make("policy_b"), "different policy_ids must produce different hashes");
}

// ── validate_policies_against_catalog tests ───────────────────────────────────

#[test]
fn p058_catalog_validation_flags_unknown_backend_profile() {
    use std::collections::HashMap;
    use workflow::catalog::{AgentCatalogFile, BackendProfile};

    let policy = EscalationPolicyYaml {
        policy_id: "p1".into(),
        schema_version: SCHEMA_VERSION.into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("x".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 2,
        max_chain_wall_clock_seconds: 600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "t1".into(),
            kind: "backend_profile".into(),
            backend_profile_id: Some("nonexistent_profile".into()),
            max_attempts: Some(1),
        }],
    };

    let mut backend_profiles = HashMap::new();
    backend_profiles.insert(
        "other_profile".to_string(),
        BackendProfile {
            provider: "claude".into(),
            model: None,
            effort: None,
            temperature: None,
            max_turns: None,
            structured_output: None,
            mcp: None,
            runtime_profile: None,
        },
    );

    let catalog = AgentCatalogFile {
        schema_version: None,
        catalog_snapshot_format_version: None,
        app: None,
        paths: None,
        artifacts: None,
        skills: None,
        contracts: None,
        runtime_profiles: None,
        backend_profiles: Some(backend_profiles),
        permission_profiles: None,
        agents: None,
        escalation_policies: Some(vec![policy]),
    };

    let diags = validate_policies_against_catalog(&catalog);
    assert_eq!(diags.len(), 1, "one unknown profile must produce one diagnostic");
    assert_eq!(
        diags[0].pause_reason_code,
        "escalation_policy_unknown_backend_profile"
    );
    assert!(
        diags[0].detail.contains("nonexistent_profile"),
        "detail must name the missing profile; got: {}",
        diags[0].detail
    );
}

#[test]
fn p058_catalog_validation_passes_for_known_profiles() {
    use std::collections::HashMap;
    use workflow::catalog::{AgentCatalogFile, BackendProfile};

    let policy = EscalationPolicyYaml {
        policy_id: "p1".into(),
        schema_version: SCHEMA_VERSION.into(),
        enabled_default: false,
        applies_to: AppliesToYaml {
            agent_id: Some("x".into()),
            backend_profile_id: None,
            stage_id: None,
        },
        max_chain_attempts: 2,
        max_chain_wall_clock_seconds: 600,
        triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
        tiers: vec![EscalationTierYaml {
            tier_id: "t1".into(),
            kind: "backend_profile".into(),
            backend_profile_id: Some("claude_frontier".into()),
            max_attempts: Some(1),
        }],
    };

    let mut backend_profiles = HashMap::new();
    backend_profiles.insert(
        "claude_frontier".to_string(),
        BackendProfile {
            provider: "claude".into(),
            model: None,
            effort: None,
            temperature: None,
            max_turns: None,
            structured_output: None,
            mcp: None,
            runtime_profile: None,
        },
    );

    let catalog = AgentCatalogFile {
        schema_version: None,
        catalog_snapshot_format_version: None,
        app: None,
        paths: None,
        artifacts: None,
        skills: None,
        contracts: None,
        runtime_profiles: None,
        backend_profiles: Some(backend_profiles),
        permission_profiles: None,
        agents: None,
        escalation_policies: Some(vec![policy]),
    };

    let diags = validate_policies_against_catalog(&catalog);
    assert!(
        diags.is_empty(),
        "no diagnostics expected for known profile; got: {diags:?}"
    );
}

#[test]
fn p058_catalog_validation_passes_for_empty_policies() {
    use workflow::catalog::AgentCatalogFile;

    let catalog = AgentCatalogFile {
        schema_version: None,
        catalog_snapshot_format_version: None,
        app: None,
        paths: None,
        artifacts: None,
        skills: None,
        contracts: None,
        runtime_profiles: None,
        backend_profiles: None,
        permission_profiles: None,
        agents: None,
        escalation_policies: None,
    };

    let diags = validate_policies_against_catalog(&catalog);
    assert!(diags.is_empty(), "no policies = no diagnostics");
}

// ── AgentCatalogFile escalation_policies field tests ─────────────────────────

#[test]
fn p058_catalog_file_deserializes_escalation_policies_section() {
    let yaml = r#"
backend_profiles:
  claude_sonnet:
    provider: claude
    model: claude-sonnet-4-6

escalation_policies:
  - policy_id: code_writer_default_escalation
    schema_version: escalation_policy_v1
    enabled_default: false
    applies_to:
      agent_id: code_writer
    max_chain_attempts: 6
    max_chain_wall_clock_seconds: 7200
    triggers:
      - contract_output_failure
    tiers:
      - tier_id: primary_retry
        kind: same_backend_retry
        max_attempts: 2
      - tier_id: human_pause
        kind: pause
"#;
    let catalog: workflow::catalog::AgentCatalogFile =
        serde_yaml::from_str(yaml).expect("catalog with escalation_policies must parse");
    let policies = catalog.escalation_policies.as_ref().expect("must have policies");
    assert_eq!(policies.len(), 1);
    assert_eq!(policies[0].policy_id, "code_writer_default_escalation");
    assert_eq!(policies[0].tiers.len(), 2);
}

#[test]
fn p058_catalog_file_without_escalation_policies_is_backward_compatible() {
    let yaml = r#"
backend_profiles:
  claude_sonnet:
    provider: claude
"#;
    let catalog: workflow::catalog::AgentCatalogFile =
        serde_yaml::from_str(yaml).expect("catalog without escalation_policies must parse");
    assert!(
        catalog.escalation_policies.is_none(),
        "absent escalation_policies must parse as None"
    );
}

// ── EscalationPolicySnapshot in RunPlan tests ─────────────────────────────────

#[test]
fn p058_escalation_policy_snapshot_round_trips_through_json() {
    use workflow::plan::{EscalationPolicySnapshot, EscalationTierSnapshot};

    let snapshot = EscalationPolicySnapshot {
        policy_id: "test_policy".into(),
        schema_version: "escalation_policy_v1".into(),
        enabled_default: false,
        applies_to_agent_id: Some("code_writer".into()),
        applies_to_backend_profile_id: None,
        applies_to_stage_id: None,
        max_chain_attempts: 6,
        max_chain_wall_clock_seconds: 7200,
        triggers: vec![
            "contract_output_failure".into(),
            "stale_no_output".into(),
        ],
        tiers: vec![
            EscalationTierSnapshot {
                tier_id: "primary_retry".into(),
                kind: "same_backend_retry".into(),
                backend_profile_id: None,
                max_attempts: Some(2),
            },
            EscalationTierSnapshot {
                tier_id: "human_pause".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            },
        ],
        policy_hash: "sha256:abc123def456".into(),
    };

    let json = serde_json::to_string(&snapshot).unwrap();
    let decoded: EscalationPolicySnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.policy_id, "test_policy");
    assert_eq!(decoded.tiers.len(), 2);
    assert_eq!(decoded.triggers.len(), 2);
    assert_eq!(decoded.policy_hash, "sha256:abc123def456");
}

// ── Trigger raw string vocabulary tests ──────────────────────────────────────

#[test]
fn p058_trigger_yaml_raw_str_covers_all_6_proposal_triggers() {
    let triggers = [
        (
            EscalationTriggerYaml::RepeatedSameBlockerDigest,
            "repeated_same_blocker_digest",
        ),
        (
            EscalationTriggerYaml::ContractOutputFailure,
            "contract_output_failure",
        ),
        (EscalationTriggerYaml::StaleNoOutput, "stale_no_output"),
        (
            EscalationTriggerYaml::ProviderQuotaExhausted,
            "provider_quota_exhausted",
        ),
        (EscalationTriggerYaml::TransportFailure, "transport_failure"),
        (
            EscalationTriggerYaml::LoopBudgetThreshold,
            "loop_budget_threshold",
        ),
    ];
    for (trigger, expected_raw) in &triggers {
        assert_eq!(
            trigger.as_raw_str(),
            *expected_raw,
            "trigger raw string mismatch"
        );
    }
}

#[test]
fn p058_operator_forced_trigger_is_reserved_vocabulary() {
    let t = EscalationTriggerYaml::OperatorForcedReservedRejected;
    assert_eq!(t.as_raw_str(), "operator_forced_reserved_rejected");
}
