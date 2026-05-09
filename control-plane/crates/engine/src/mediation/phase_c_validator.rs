//! P017 Phase C: Static validator and runtime preflight for lead validation.
//!
//! Static validator: Verifies exactly-one lead resolution per workflow+catalog pair.
//! Runtime preflight: Verifies provider availability and profile configuration
//! before a mediation-eligible run starts.

use serde::{Deserialize, Serialize};

use super::lead_resolver::{LeadResolution, PhaseBLeadResolver};

/// Outcome label for the phase_c_validation_outcome_total metric.
/// Bounded cardinality per proposal requirements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseCValidationOutcome {
    Pass,
    StaticFail,
    PreflightFail,
    LegacyCatalogWarning,
}

impl std::fmt::Display for PhaseCValidationOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PhaseCValidationOutcome::Pass => "pass",
            PhaseCValidationOutcome::StaticFail => "static_fail",
            PhaseCValidationOutcome::PreflightFail => "preflight_fail",
            PhaseCValidationOutcome::LegacyCatalogWarning => "legacy_catalog_warning",
        })
    }
}

/// Result of Phase C static validation for a workflow+catalog pair.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StaticValidationResult {
    pub outcome: PhaseCValidationOutcome,
    pub workflow_source_path: String,
    pub catalog_source_path: String,
    pub lead_agent_id: Option<String>,
    pub detail: Option<String>,
}

/// Validate that exactly one lead resolves for the given workflow+catalog pair.
/// Returns StaticFail if zero or multiple leads match, Pass if exactly one matches.
pub fn validate_exactly_one_lead(
    resolver: &PhaseBLeadResolver,
    workflow_source_path: &str,
    catalog_source_path: &str,
) -> StaticValidationResult {
    match resolver.resolve(workflow_source_path, catalog_source_path) {
        LeadResolution::Resolved { lead_agent_id, .. } => StaticValidationResult {
            outcome: PhaseCValidationOutcome::Pass,
            workflow_source_path: workflow_source_path.to_string(),
            catalog_source_path: catalog_source_path.to_string(),
            lead_agent_id: Some(lead_agent_id),
            detail: None,
        },
        LeadResolution::FailedClosed { reason } => StaticValidationResult {
            outcome: PhaseCValidationOutcome::StaticFail,
            workflow_source_path: workflow_source_path.to_string(),
            catalog_source_path: catalog_source_path.to_string(),
            lead_agent_id: None,
            detail: Some(reason),
        },
    }
}

/// Result of Phase C runtime preflight checks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimePreflightResult {
    pub outcome: PhaseCValidationOutcome,
    pub checks: Vec<PreflightCheck>,
}

/// A single preflight check.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreflightCheck {
    pub id: String,
    pub passed: bool,
    pub detail: Option<String>,
}

/// Run Phase C runtime preflight checks for a mediation-eligible run.
/// Verifies provider availability and agent profile configuration.
pub fn run_runtime_preflight(
    lead_agent_id: &str,
    provider: &str,
    model: Option<&str>,
    catalog_agents: &[workflow::catalog::AgentEntry],
    catalog: &workflow::catalog::AgentCatalogFile,
) -> RuntimePreflightResult {
    let mut checks = Vec::new();

    // Check 1: Lead agent exists in catalog
    let agent_exists = catalog_agents.iter().any(|a| a.id == lead_agent_id);
    checks.push(PreflightCheck {
        id: "lead_agent_in_catalog".to_string(),
        passed: agent_exists,
        detail: if agent_exists {
            None
        } else {
            Some(format!(
                "Lead agent '{}' not found in agent catalog",
                lead_agent_id
            ))
        },
    });

    // Check 2: Provider is a recognized first-party ACP provider
    let provider_valid = matches!(provider, "claude" | "codex" | "gemini" | "auggie" | "junie");
    checks.push(PreflightCheck {
        id: "provider_available".to_string(),
        passed: provider_valid,
        detail: if provider_valid {
            None
        } else {
            Some(format!(
                "Provider '{}' is not a recognized first-party ACP provider",
                provider
            ))
        },
    });

    // Check 3: Backend profile exists for the agent
    let agent = catalog_agents.iter().find(|a| a.id == lead_agent_id);
    let profile_exists = agent
        .map(|a| {
            catalog
                .backend_profiles
                .as_ref()
                .map(|profiles| profiles.contains_key(&a.backend_profile))
                .unwrap_or(false)
        })
        .unwrap_or(false);
    checks.push(PreflightCheck {
        id: "backend_profile_configured".to_string(),
        passed: profile_exists,
        detail: if profile_exists {
            None
        } else {
            Some("Backend profile not found for lead agent".to_string())
        },
    });

    // Check 4: Model is specified (not empty/None)
    let model_specified = model.map(|m| !m.is_empty()).unwrap_or(false);
    checks.push(PreflightCheck {
        id: "model_specified".to_string(),
        passed: model_specified,
        detail: if model_specified {
            None
        } else {
            Some("No model specified for lead agent".to_string())
        },
    });

    let all_passed = checks.iter().all(|c| c.passed);
    RuntimePreflightResult {
        outcome: if all_passed {
            PhaseCValidationOutcome::Pass
        } else {
            PhaseCValidationOutcome::PreflightFail
        },
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::lead_resolver::PhaseBLeadResolverMap;

    fn sample_resolver() -> PhaseBLeadResolver {
        PhaseBLeadResolver::from_map(PhaseBLeadResolverMap {
            schema_version: "phase_b_lead_resolver_v1".to_string(),
            mapping_version: "1".to_string(),
            entries: vec![super::super::lead_resolver::LeadResolverEntry {
                workflow_source_path: "examples/workflows/full-mvp-live.yaml".to_string(),
                catalog_source_path: "examples/agents/agents.yaml".to_string(),
                lead_agent_id: "lead_agent".to_string(),
                lead_resolution_contract_ref: "phase_0_lead_contract".to_string(),
                mapping_owner: "p017_implementation_lead".to_string(),
                entry_attested_by: "p017_implementation_lead".to_string(),
                reviewed_at: "2026-04-24".to_string(),
                phase_c_removal_condition: "Phase C static validation authoritative".to_string(),
            }],
            mapping_owner: "p017_implementation_lead".to_string(),
            entry_attestation_rule: "Implementation lead reviews each entry".to_string(),
            staleness_review_trigger: "30 days without review".to_string(),
            fail_closed_behavior: "Block mediation when no exact match".to_string(),
            upgrade_and_removal_criteria: "Remove when Phase C static validation is authoritative"
                .to_string(),
        })
    }

    #[test]
    fn static_validation_pass() {
        let resolver = sample_resolver();
        let result = validate_exactly_one_lead(
            &resolver,
            "examples/workflows/full-mvp-live.yaml",
            "examples/agents/agents.yaml",
        );
        assert_eq!(result.outcome, PhaseCValidationOutcome::Pass);
        assert_eq!(result.lead_agent_id, Some("lead_agent".to_string()));
    }

    #[test]
    fn static_validation_fail_no_match() {
        let resolver = sample_resolver();
        let result = validate_exactly_one_lead(&resolver, "nonexistent.yaml", "nonexistent.yaml");
        assert_eq!(result.outcome, PhaseCValidationOutcome::StaticFail);
        assert!(result.detail.is_some());
    }

    fn make_test_catalog(
        agent_id: &str,
        backend_profile_name: &str,
        profiles: Option<std::collections::HashMap<String, workflow::catalog::BackendProfile>>,
    ) -> (
        Vec<workflow::catalog::AgentEntry>,
        workflow::catalog::AgentCatalogFile,
    ) {
        let agents = vec![workflow::catalog::AgentEntry {
            id: agent_id.to_string(),
            title: None,
            mode: None,
            system_role: None,
            backend_profile: backend_profile_name.to_string(),
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
        }];
        let catalog = workflow::catalog::AgentCatalogFile {
            schema_version: None,
            catalog_snapshot_format_version: None,
            app: None,
            paths: None,
            artifacts: None,
            skills: None,
            contracts: None,
            runtime_profiles: None,
            backend_profiles: profiles,
            permission_profiles: None,
            agents: None,
            escalation_policies: None,
        };
        (agents, catalog)
    }

    #[test]
    fn runtime_preflight_pass() {
        let mut profiles = std::collections::HashMap::new();
        profiles.insert(
            "claude_opus".to_string(),
            workflow::catalog::BackendProfile {
                provider: "claude".to_string(),
                model: Some("claude-opus-4-6".to_string()),
                effort: None,
                temperature: None,
                max_turns: None,
                structured_output: None,
                mcp: None,
                runtime_profile: None,
            },
        );
        let (agents, catalog) = make_test_catalog("lead_agent", "claude_opus", Some(profiles));
        let result = run_runtime_preflight(
            "lead_agent",
            "claude",
            Some("claude-opus-4-6"),
            &agents,
            &catalog,
        );
        assert_eq!(result.outcome, PhaseCValidationOutcome::Pass);
        assert!(result.checks.iter().all(|c| c.passed));
    }

    #[test]
    fn runtime_preflight_fail_unknown_provider() {
        let (agents, catalog) = make_test_catalog("lead_agent", "custom", None);
        let result =
            run_runtime_preflight("lead_agent", "unknown_provider", None, &agents, &catalog);
        assert_eq!(result.outcome, PhaseCValidationOutcome::PreflightFail);
    }
}
