/// P066 Phase 0 workflow crate tests.
///
/// Covers:
/// - Unknown keys in toolchain_cache_policy fail YAML compilation (deny_unknown_fields)
/// - Unknown version values fail validation
/// - Version 1 + enabled=false parses and validates correctly
/// - Absent toolchain_cache_policy decodes as None (policy_absent)
/// - catalog_snapshot_format_version validation logic
/// - Scope defaults: xcode→run, go→session when enabled and absent
use workflow::catalog::{
    validate_catalog_snapshot_format_version, validate_toolchain_cache_policies, AgentCatalogFile,
    AgentEntry, ToolchainCachePolicyYaml, ToolchainCacheScope,
};
use workflow::plan::{ToolchainCachePolicySnapshot, ToolchainCacheScopeSnapshot};

fn minimal_agent_entry(id: &str) -> AgentEntry {
    AgentEntry {
        id: id.to_string(),
        title: None,
        mode: None,
        system_role: None,
        backend_profile: "test_backend".to_string(),
        permission_profile: None,
        skill_ref: None,
        skill_role: None,
        session_reuse_scope: None,
        session_family_id: None,
        inputs: None,
        outputs: None,
        output_contract: None,
        lead_resolution_contract: None,
        requires_human_approval: None,
        prompt: None,
        notes: None,
        worktree_policy: None,
        required_tools: None,
        xcode_broker_required: None,
        xcode_shim_injection_signal: None,
        requires_xcode_host_execution: None,
        routing: None,
        toolchain_cache_policy: None,
    }
}

// ── YAML serde tests ──────────────────────────────────────────────────────────

#[test]
fn p066_unknown_key_in_toolchain_cache_policy_fails_deserialization() {
    let yaml = r#"
version: 1
enabled: false
unknown_key: should_fail
"#;
    let result: Result<ToolchainCachePolicyYaml, _> = serde_yaml::from_str(yaml);
    assert!(
        result.is_err(),
        "unknown key should fail with deny_unknown_fields"
    );
}

#[test]
fn p066_valid_disabled_policy_parses_correctly() {
    let yaml = r#"
version: 1
enabled: false
"#;
    let policy: ToolchainCachePolicyYaml = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(policy.version, 1);
    assert!(!policy.enabled);
    assert!(policy.xcode_scope.is_none());
    assert!(policy.go_scope.is_none());
    policy.validate().expect("version 1 should be valid");
}

#[test]
fn p066_valid_enabled_policy_with_scopes_parses_correctly() {
    let yaml = r#"
version: 1
enabled: true
xcode_scope: run
go_scope: session
"#;
    let policy: ToolchainCachePolicyYaml = serde_yaml::from_str(yaml).unwrap();
    assert!(policy.enabled);
    assert_eq!(policy.xcode_scope, Some(ToolchainCacheScope::Run));
    assert_eq!(policy.go_scope, Some(ToolchainCacheScope::Session));
    policy.validate().expect("version 1 should be valid");
}

#[test]
fn p066_unsupported_version_fails_validate() {
    let yaml = r#"
version: 99
enabled: false
"#;
    let policy: ToolchainCachePolicyYaml = serde_yaml::from_str(yaml).unwrap();
    let err = policy.validate().unwrap_err();
    assert!(
        err.to_string().contains("99"),
        "error should mention unsupported version"
    );
}

#[test]
fn p066_unknown_xcode_scope_enum_fails_deserialization() {
    let yaml = r#"
version: 1
enabled: true
xcode_scope: global
"#;
    let result: Result<ToolchainCachePolicyYaml, _> = serde_yaml::from_str(yaml);
    assert!(
        result.is_err(),
        "unknown enum value for xcode_scope should fail"
    );
}

// ── Snapshot compatibility gate tests ─────────────────────────────────────────

#[test]
fn p066_catalog_without_version_or_policy_is_legacy_v0() {
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
        agents: Some(vec![minimal_agent_entry("a1")]),
        escalation_policies: None,
    };
    let result = validate_catalog_snapshot_format_version(&catalog).unwrap();
    assert!(!result, "no version + no policy = legacy_v0");
}

#[test]
fn p066_catalog_with_policy_but_no_version_fails() {
    let mut entry = minimal_agent_entry("a1");
    entry.toolchain_cache_policy = Some(ToolchainCachePolicyYaml {
        version: 1,
        enabled: false,
        xcode_scope: None,
        go_scope: None,
    });
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
        agents: Some(vec![entry]),
        escalation_policies: None,
    };
    let result = validate_catalog_snapshot_format_version(&catalog);
    assert!(
        result.is_err(),
        "policy present but no format version = frozen_snapshot_contract_incompatible"
    );
}

#[test]
fn p066_catalog_with_version_1_is_valid() {
    let catalog = AgentCatalogFile {
        schema_version: None,
        catalog_snapshot_format_version: Some(1),
        app: None,
        paths: None,
        artifacts: None,
        skills: None,
        contracts: None,
        runtime_profiles: None,
        backend_profiles: None,
        permission_profiles: None,
        agents: Some(vec![minimal_agent_entry("a1")]),
        escalation_policies: None,
    };
    let result = validate_catalog_snapshot_format_version(&catalog).unwrap();
    assert!(result, "version 1 = P066-aware snapshot");
}

#[test]
fn p066_catalog_with_unsupported_version_fails() {
    let catalog = AgentCatalogFile {
        schema_version: None,
        catalog_snapshot_format_version: Some(99),
        app: None,
        paths: None,
        artifacts: None,
        skills: None,
        contracts: None,
        runtime_profiles: None,
        backend_profiles: None,
        permission_profiles: None,
        agents: Some(vec![minimal_agent_entry("a1")]),
        escalation_policies: None,
    };
    let result = validate_catalog_snapshot_format_version(&catalog);
    assert!(
        result.is_err(),
        "unsupported version should fail as frozen_snapshot_contract_incompatible"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("frozen_snapshot_contract_incompatible"),
        "error should identify as frozen_snapshot_contract_incompatible"
    );
}

#[test]
fn p066_catalog_with_zero_version_fails() {
    let catalog = AgentCatalogFile {
        schema_version: None,
        catalog_snapshot_format_version: Some(0),
        app: None,
        paths: None,
        artifacts: None,
        skills: None,
        contracts: None,
        runtime_profiles: None,
        backend_profiles: None,
        permission_profiles: None,
        agents: Some(vec![minimal_agent_entry("a1")]),
        escalation_policies: None,
    };
    let result = validate_catalog_snapshot_format_version(&catalog);
    assert!(
        result.is_err(),
        "version 0 should fail as frozen_snapshot_contract_incompatible"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("frozen_snapshot_contract_incompatible"),
        "error should identify as frozen_snapshot_contract_incompatible"
    );
}

// ── Plan snapshot types ────────────────────────────────────────────────────────

#[test]
fn p066_toolchain_cache_policy_snapshot_scope_defaults() {
    let policy_enabled = ToolchainCachePolicySnapshot {
        version: 1,
        enabled: true,
        xcode_scope: None,
        go_scope: None,
    };
    assert_eq!(
        policy_enabled.effective_xcode_scope(),
        Some(ToolchainCacheScopeSnapshot::Run),
        "xcode defaults to run when enabled and absent"
    );
    assert_eq!(
        policy_enabled.effective_go_scope(),
        Some(ToolchainCacheScopeSnapshot::Session),
        "go defaults to session when enabled and absent"
    );

    let policy_disabled = ToolchainCachePolicySnapshot {
        version: 1,
        enabled: false,
        xcode_scope: None,
        go_scope: None,
    };
    assert!(
        policy_disabled.effective_xcode_scope().is_none(),
        "disabled policy returns no scope"
    );
    assert!(
        policy_disabled.effective_go_scope().is_none(),
        "disabled policy returns no scope"
    );
}

// ── validate_toolchain_cache_policies ─────────────────────────────────────────

#[test]
fn p066_validate_policies_rejects_unsupported_version_in_catalog() {
    let mut entry = minimal_agent_entry("a1");
    entry.toolchain_cache_policy = Some(ToolchainCachePolicyYaml {
        version: 99,
        enabled: false,
        xcode_scope: None,
        go_scope: None,
    });
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
        agents: Some(vec![entry]),
        escalation_policies: None,
    };
    let result = validate_toolchain_cache_policies(&catalog);
    assert!(
        result.is_err(),
        "unsupported version in catalog should fail"
    );
}

#[test]
fn p066_validate_policies_accepts_absent_policy() {
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
        agents: Some(vec![minimal_agent_entry("a1")]),
        escalation_policies: None,
    };
    validate_toolchain_cache_policies(&catalog).expect("absent policy should pass validation");
}
