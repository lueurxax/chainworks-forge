//! escalation_policy_v1 YAML parser and compile-time validation.
//!
//! Escalation policies declare an ordered retry/escalation chain for agent executions.
//! Policies are embedded in the agent catalog under `escalation_policies:`.
//!
//! Proposal requirements enforced here (Phase 0 compile validation):
//! - Unknown top-level fields fail compile (`deny_unknown_fields` on all structs).
//! - `schema_version` must equal `"escalation_policy_v1"`.
//! - Required fields: policy_id, schema_version, enabled_default, applies_to,
//!   max_chain_attempts, max_chain_wall_clock_seconds, triggers, tiers.
//! - `backend_profile` tier kind references must resolve in the catalog
//!   (`escalation_policy_unknown_backend_profile` pause reason on failure).
//! - `backend_profile` tier without `backend_profile_id` or `max_attempts` fails compile.
//! - `same_backend_retry` and `lead_mediation` tiers without `max_attempts` fail compile.
//! - Unknown tier kind strings fail compile.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::catalog::AgentCatalogFile;

pub const SCHEMA_VERSION: &str = "escalation_policy_v1";

/// Maximum payload_json size before the repository layer rejects the write (P058-SEC-L2).
/// This constant is the authoritative source; callers outside the db crate should import it.
pub const PAYLOAD_JSON_MAX_BYTES: usize = 64 * 1024; // 64 KiB

// ── Trigger vocabulary ─────────────────────────────────────────────────────────

/// Supported trigger vocabulary for escalation_policy_v1.
///
/// All variants are stored as raw snake_case strings on the wire for forward compatibility.
/// Unknown future trigger values pass through unchanged at runtime; the compile step
/// validates only what is declared in the YAML.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationTriggerYaml {
    RepeatedSameBlockerDigest,
    ContractOutputFailure,
    StaleNoOutput,
    ProviderQuotaExhausted,
    TransportFailure,
    LoopBudgetThreshold,
    /// Parsed but rejected at execution time — reserved per proposal non_goals.
    OperatorForcedReservedRejected,
}

impl EscalationTriggerYaml {
    pub fn as_raw_str(&self) -> &'static str {
        match self {
            Self::RepeatedSameBlockerDigest => "repeated_same_blocker_digest",
            Self::ContractOutputFailure => "contract_output_failure",
            Self::StaleNoOutput => "stale_no_output",
            Self::ProviderQuotaExhausted => "provider_quota_exhausted",
            Self::TransportFailure => "transport_failure",
            Self::LoopBudgetThreshold => "loop_budget_threshold",
            Self::OperatorForcedReservedRejected => "operator_forced_reserved_rejected",
        }
    }
}

// ── Binding selector ───────────────────────────────────────────────────────────

/// Binding selector — determines which agent executions this policy applies to.
///
/// At least one selector field must be set. Multiple selectors at equal specificity
/// on different policies targeting the same execution produce
/// `escalation_policy_ambiguous_at_compile`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AppliesToYaml {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage_id: Option<String>,
}

// ── Tier definition ────────────────────────────────────────────────────────────

/// A single tier in the ordered escalation chain.
///
/// `deny_unknown_fields` causes unknown YAML keys to fail compile rather than be silently
/// dropped — per proposal policy_schema.strictness: "unknown escalation fields fail compile".
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationTierYaml {
    pub tier_id: String,
    /// Kind: `same_backend_retry` | `backend_profile` | `lead_mediation` | `pause`
    pub kind: String,
    /// Required for `backend_profile` kind — the catalog backend_profile to escalate to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_profile_id: Option<String>,
    /// Max attempts for this tier. Required for all kinds except `pause`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_attempts: Option<u32>,
}

// ── Root policy document ───────────────────────────────────────────────────────

/// Root escalation_policy_v1 YAML document.
///
/// `deny_unknown_fields` enforces the proposal requirement: "unknown escalation fields fail compile".
/// All listed fields are required by the policy_schema.required_fields contract.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationPolicyYaml {
    pub policy_id: String,
    pub schema_version: String,
    pub enabled_default: bool,
    pub applies_to: AppliesToYaml,
    pub max_chain_attempts: u32,
    pub max_chain_wall_clock_seconds: u64,
    pub triggers: Vec<EscalationTriggerYaml>,
    pub tiers: Vec<EscalationTierYaml>,
}

// ── Compile diagnostics ────────────────────────────────────────────────────────

/// Compile-time diagnostic produced when a policy fails validation against the catalog.
///
/// The `pause_reason_code` is a stable string from the proposal's pause_reason_catalog and is
/// safe to embed in operator-visible error surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationCompileDiagnostic {
    pub policy_id: String,
    /// Stable pause reason code from proposal pause_reason_catalog.
    pub pause_reason_code: &'static str,
    pub detail: String,
}

// ── Parsing ────────────────────────────────────────────────────────────────────

/// Parse a single escalation_policy_v1 YAML string.
///
/// Returns `Err` on:
/// - Malformed YAML
/// - Unknown top-level or tier fields (`deny_unknown_fields`)
/// - Wrong `schema_version`
/// - Empty `tiers` or `triggers`
/// - Structural tier errors (missing `backend_profile_id`, `max_attempts`)
/// - Unknown tier `kind` values
pub fn parse_policy(yaml_str: &str) -> Result<EscalationPolicyYaml> {
    let policy: EscalationPolicyYaml = serde_yaml::from_str(yaml_str)
        .map_err(|e| anyhow::anyhow!("escalation_policy_v1 parse error: {e}"))?;
    validate_policy_structure(&policy)?;
    Ok(policy)
}

fn validate_policy_structure(policy: &EscalationPolicyYaml) -> Result<()> {
    if policy.schema_version != SCHEMA_VERSION {
        bail!(
            "escalation_policy_v1 '{}': schema_version must be '{}'; got '{}'",
            policy.policy_id,
            SCHEMA_VERSION,
            policy.schema_version,
        );
    }
    if policy.tiers.is_empty() {
        bail!(
            "escalation_policy_v1 '{}': tiers must not be empty",
            policy.policy_id
        );
    }
    if policy.triggers.is_empty() {
        bail!(
            "escalation_policy_v1 '{}': triggers must not be empty",
            policy.policy_id
        );
    }
    if policy.applies_to.agent_id.is_none()
        && policy.applies_to.backend_profile_id.is_none()
        && policy.applies_to.stage_id.is_none()
    {
        bail!(
            "escalation_policy_v1 '{}': applies_to must set at least one of agent_id, backend_profile_id, or stage_id",
            policy.policy_id
        );
    }
    for tier in &policy.tiers {
        validate_tier_structure(policy, tier)?;
    }
    Ok(())
}

fn validate_tier_structure(policy: &EscalationPolicyYaml, tier: &EscalationTierYaml) -> Result<()> {
    match tier.kind.as_str() {
        "backend_profile" => {
            if tier.backend_profile_id.is_none() {
                bail!(
                    "escalation_policy_v1 '{}' tier '{}': kind 'backend_profile' requires backend_profile_id",
                    policy.policy_id,
                    tier.tier_id
                );
            }
            if tier.max_attempts.is_none() {
                bail!(
                    "escalation_policy_v1 '{}' tier '{}': kind 'backend_profile' requires max_attempts",
                    policy.policy_id,
                    tier.tier_id
                );
            }
        }
        "same_backend_retry" | "lead_mediation" => {
            if tier.max_attempts.is_none() {
                bail!(
                    "escalation_policy_v1 '{}' tier '{}': kind '{}' requires max_attempts",
                    policy.policy_id,
                    tier.tier_id,
                    tier.kind
                );
            }
        }
        "pause" => {}
        other => bail!(
            "escalation_policy_v1 '{}' tier '{}': unknown kind '{}'; expected one of: same_backend_retry, backend_profile, lead_mediation, pause",
            policy.policy_id,
            tier.tier_id,
            other
        ),
    }
    Ok(())
}

// ── Catalog validation ─────────────────────────────────────────────────────────

/// Validate all escalation policies in a catalog against its backend_profiles.
///
/// Returns a list of compile diagnostics. An empty list means all policies are valid.
/// Any diagnostic should surface as a run preflight failure with the given `pause_reason_code`.
///
/// Currently checks:
/// - `backend_profile` tier references resolve in `catalog.backend_profiles`
///   → `escalation_policy_unknown_backend_profile`
pub fn validate_policies_against_catalog(
    catalog: &AgentCatalogFile,
) -> Vec<EscalationCompileDiagnostic> {
    let policies = match catalog.escalation_policies.as_deref() {
        Some(p) if !p.is_empty() => p,
        _ => return Vec::new(),
    };
    let backend_profiles = catalog.backend_profiles.as_ref();
    let mut diagnostics = Vec::new();

    for policy in policies {
        for tier in &policy.tiers {
            if tier.kind == "backend_profile" {
                if let Some(ref profile_id) = tier.backend_profile_id {
                    let exists = backend_profiles
                        .map(|bp| bp.contains_key(profile_id.as_str()))
                        .unwrap_or(false);
                    if !exists {
                        diagnostics.push(EscalationCompileDiagnostic {
                            policy_id: policy.policy_id.clone(),
                            pause_reason_code: "escalation_policy_unknown_backend_profile",
                            detail: format!(
                                "policy '{}' tier '{}' references unknown backend_profile_id '{}'",
                                policy.policy_id, tier.tier_id, profile_id
                            ),
                        });
                    }
                }
            }
        }
    }
    diagnostics
}

// ── Policy hashing ─────────────────────────────────────────────────────────────

/// Compute SHA-256 of the canonical JSON serialization of a policy.
/// Returns the hash prefixed with `"sha256:"` to match the project-wide hash format.
pub fn compute_policy_hash(policy: &EscalationPolicyYaml) -> Result<String> {
    use sha2::{Digest, Sha256};
    let canonical = serde_json::to_string(policy)
        .map_err(|e| anyhow::anyhow!("policy hash serialization failed: {e}"))?;
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(format!("sha256:{digest:x}"))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_yaml_roundtrips_via_raw_str() {
        let triggers = [
            EscalationTriggerYaml::RepeatedSameBlockerDigest,
            EscalationTriggerYaml::ContractOutputFailure,
            EscalationTriggerYaml::StaleNoOutput,
            EscalationTriggerYaml::ProviderQuotaExhausted,
            EscalationTriggerYaml::TransportFailure,
            EscalationTriggerYaml::LoopBudgetThreshold,
            EscalationTriggerYaml::OperatorForcedReservedRejected,
        ];
        for t in &triggers {
            let raw = t.as_raw_str();
            let json = serde_json::to_string(t).unwrap();
            // serde produces quoted json string matching the snake_case raw name
            assert_eq!(json, format!("\"{}\"", raw));
        }
    }

    #[test]
    fn parse_minimal_valid_policy() {
        let yaml = r#"
policy_id: test_policy
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: code_writer
max_chain_attempts: 3
max_chain_wall_clock_seconds: 3600
triggers:
  - contract_output_failure
tiers:
  - tier_id: primary_retry
    kind: same_backend_retry
    max_attempts: 2
  - tier_id: human_pause
    kind: pause
"#;
        let policy = parse_policy(yaml).unwrap();
        assert_eq!(policy.policy_id, "test_policy");
        assert_eq!(policy.schema_version, "escalation_policy_v1");
        assert!(!policy.enabled_default);
        assert_eq!(policy.max_chain_attempts, 3);
        assert_eq!(policy.tiers.len(), 2);
    }

    #[test]
    fn parse_full_proposal_example_policy() {
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
        let policy = parse_policy(yaml).unwrap();
        assert_eq!(policy.policy_id, "code_writer_default_escalation");
        assert_eq!(policy.triggers.len(), 6);
        assert_eq!(policy.tiers.len(), 5);
        assert_eq!(policy.tiers[1].backend_profile_id.as_deref(), Some("claude_builder_frontier"));
        assert_eq!(policy.tiers[4].kind, "pause");
    }

    #[test]
    fn parse_rejects_unknown_top_level_field() {
        let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: true
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
unknown_extra_field: this_should_fail
"#;
        let err = parse_policy(yaml).unwrap_err();
        assert!(
            err.to_string().contains("unknown") || err.to_string().contains("parse error"),
            "expected unknown field error; got: {err}"
        );
    }

    #[test]
    fn parse_rejects_unknown_tier_field() {
        let yaml = r#"
policy_id: test
schema_version: escalation_policy_v1
enabled_default: true
applies_to:
  agent_id: x
max_chain_attempts: 1
max_chain_wall_clock_seconds: 60
triggers:
  - contract_output_failure
tiers:
  - tier_id: t1
    kind: pause
    unknown_tier_key: bad
"#;
        let err = parse_policy(yaml).unwrap_err();
        assert!(
            err.to_string().contains("unknown") || err.to_string().contains("parse error"),
            "expected unknown tier field error; got: {err}"
        );
    }

    #[test]
    fn parse_rejects_wrong_schema_version() {
        let yaml = r#"
policy_id: test
schema_version: escalation_policy_v2_future
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
        assert!(err.to_string().contains("schema_version"), "got: {err}");
    }

    #[test]
    fn parse_rejects_empty_tiers() {
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
    fn parse_rejects_empty_triggers() {
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
    fn parse_rejects_backend_profile_tier_without_profile_id() {
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
    kind: backend_profile
    max_attempts: 1
"#;
        let err = parse_policy(yaml).unwrap_err();
        assert!(
            err.to_string().contains("backend_profile_id"),
            "got: {err}"
        );
    }

    #[test]
    fn parse_rejects_same_backend_retry_without_max_attempts() {
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
    kind: same_backend_retry
"#;
        let err = parse_policy(yaml).unwrap_err();
        assert!(err.to_string().contains("max_attempts"), "got: {err}");
    }

    #[test]
    fn parse_rejects_unknown_tier_kind() {
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
    kind: future_unknown_kind_v99
"#;
        let err = parse_policy(yaml).unwrap_err();
        assert!(
            err.to_string().contains("unknown kind"),
            "got: {err}"
        );
    }

    #[test]
    fn parse_rejects_empty_applies_to() {
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

    #[test]
    fn compute_policy_hash_is_deterministic() {
        let yaml = r#"
policy_id: test_hash
schema_version: escalation_policy_v1
enabled_default: false
applies_to:
  agent_id: code_writer
max_chain_attempts: 3
max_chain_wall_clock_seconds: 3600
triggers:
  - contract_output_failure
tiers:
  - tier_id: primary_retry
    kind: same_backend_retry
    max_attempts: 2
"#;
        let policy = parse_policy(yaml).unwrap();
        let hash1 = compute_policy_hash(&policy).unwrap();
        let hash2 = compute_policy_hash(&policy).unwrap();
        assert_eq!(hash1, hash2);
        assert!(hash1.starts_with("sha256:"), "hash must be prefixed; got: {hash1}");
        assert_eq!(hash1.len(), 7 + 64); // "sha256:" + 64 hex chars
    }

    #[test]
    fn validate_policies_against_catalog_reports_unknown_backend_profile() {
        use crate::catalog::AgentCatalogFile;
        use std::collections::HashMap;
        use crate::catalog::BackendProfile;

        let policy = EscalationPolicyYaml {
            policy_id: "test".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: false,
            applies_to: AppliesToYaml { agent_id: Some("x".into()), backend_profile_id: None, stage_id: None },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![
                EscalationTierYaml {
                    tier_id: "t1".into(),
                    kind: "backend_profile".into(),
                    backend_profile_id: Some("missing_profile".into()),
                    max_attempts: Some(1),
                },
            ],
        };

        let mut profiles = HashMap::new();
        profiles.insert("existing_profile".to_string(), BackendProfile {
            provider: "claude".into(),
            model: None,
            effort: None,
            temperature: None,
            max_turns: None,
            structured_output: None,
            mcp: None,
            runtime_profile: None,
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
            backend_profiles: Some(profiles),
            permission_profiles: None,
            agents: None,
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].pause_reason_code, "escalation_policy_unknown_backend_profile");
        assert!(diags[0].detail.contains("missing_profile"));
    }

    #[test]
    fn validate_policies_against_catalog_passes_when_profile_exists() {
        use crate::catalog::AgentCatalogFile;
        use std::collections::HashMap;
        use crate::catalog::BackendProfile;

        let policy = EscalationPolicyYaml {
            policy_id: "test".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: false,
            applies_to: AppliesToYaml { agent_id: Some("x".into()), backend_profile_id: None, stage_id: None },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![
                EscalationTierYaml {
                    tier_id: "t1".into(),
                    kind: "backend_profile".into(),
                    backend_profile_id: Some("claude_frontier".into()),
                    max_attempts: Some(1),
                },
            ],
        };

        let mut profiles = HashMap::new();
        profiles.insert("claude_frontier".to_string(), BackendProfile {
            provider: "claude".into(),
            model: None,
            effort: None,
            temperature: None,
            max_turns: None,
            structured_output: None,
            mcp: None,
            runtime_profile: None,
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
            backend_profiles: Some(profiles),
            permission_profiles: None,
            agents: None,
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert!(diags.is_empty(), "no diagnostics expected when profile exists; got: {diags:?}");
    }
}
