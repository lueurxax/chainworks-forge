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

/// Maximum YAML input size for a single escalation policy (P058-SEC-M1).
const POLICY_YAML_MAX_BYTES: usize = 64 * 1024; // 64 KiB
/// Maximum number of tiers per policy (P058-SEC-M1).
const POLICY_MAX_TIERS: usize = 32;
/// Maximum number of triggers per policy (P058-SEC-M1).
const POLICY_MAX_TRIGGERS: usize = 16;
/// Maximum identifier length for policy/tier/agent/profile IDs (P058-SEC-L1).
const POLICY_ID_MAX_LEN: usize = 64;

/// Validate that `s` is a safe identifier: 1-64 characters, ASCII alphanumeric plus `_.:-`.
/// P058-SEC-L1: compile diagnostics embed these identifiers verbatim in error strings.
fn is_safe_policy_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= POLICY_ID_MAX_LEN
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-'))
}

/// Parse a single escalation_policy_v1 YAML string.
///
/// Returns `Err` on:
/// - Input exceeding 64 KiB (P058-SEC-M1)
/// - Malformed YAML
/// - Unknown top-level or tier fields (`deny_unknown_fields`)
/// - Wrong `schema_version`
/// - Empty `tiers` or `triggers`
/// - Structural tier errors (missing `backend_profile_id`, `max_attempts`)
/// - Unknown tier `kind` values
pub fn parse_policy(yaml_str: &str) -> Result<EscalationPolicyYaml> {
    if yaml_str.len() > POLICY_YAML_MAX_BYTES {
        bail!(
            "escalation_policy_v1 input exceeds maximum size of {} bytes (got {}); \
             use a shorter policy document (P058-SEC-M1)",
            POLICY_YAML_MAX_BYTES,
            yaml_str.len()
        );
    }
    let policy: EscalationPolicyYaml = serde_yaml::from_str(yaml_str)
        .map_err(|e| anyhow::anyhow!("escalation_policy_v1 parse error: {e}"))?;
    validate_policy_structure(&policy)?;
    Ok(policy)
}

/// Validate the structural invariants of a single escalation policy.
/// Called by `parse_policy` (YAML path) and by `compile_escalation_policies`
/// (catalog path where policies are deserialized directly without parse_policy).
/// SEC-002: must be invoked on the catalog path before hash/freeze to prevent
/// structurally invalid policies (wrong schema_version, empty tiers/triggers, etc.)
/// from being persisted as frozen snapshots.
pub fn validate_policy_structure(policy: &EscalationPolicyYaml) -> Result<()> {
    // P058-SEC-L1: validate policy_id before embedding in any error message.
    if !is_safe_policy_identifier(&policy.policy_id) {
        bail!(
            "escalation_policy_v1: policy_id must be 1-{} ASCII alphanumeric/_.:-characters; \
             got value that fails identifier validation (P058-SEC-L1)",
            POLICY_ID_MAX_LEN
        );
    }
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
    // P058-SEC-M1: cap tier count to prevent CPU/memory exhaustion during hash computation.
    if policy.tiers.len() > POLICY_MAX_TIERS {
        bail!(
            "escalation_policy_v1 '{}': tiers must not exceed {} entries; got {} (P058-SEC-M1)",
            policy.policy_id,
            POLICY_MAX_TIERS,
            policy.tiers.len()
        );
    }
    if policy.triggers.is_empty() {
        bail!(
            "escalation_policy_v1 '{}': triggers must not be empty",
            policy.policy_id
        );
    }
    // P058-SEC-M1: cap trigger count to prevent CPU/memory exhaustion.
    if policy.triggers.len() > POLICY_MAX_TRIGGERS {
        bail!(
            "escalation_policy_v1 '{}': triggers must not exceed {} entries; got {} (P058-SEC-M1)",
            policy.policy_id,
            POLICY_MAX_TRIGGERS,
            policy.triggers.len()
        );
    }
    // P058-SEC-L1: validate applies_to selectors before embedding in error messages.
    if let Some(ref agent_id) = policy.applies_to.agent_id {
        if !is_safe_policy_identifier(agent_id) {
            bail!(
                "escalation_policy_v1 '{}': applies_to.agent_id must be 1-{} \
                 ASCII alphanumeric/_.:-characters (P058-SEC-L1)",
                policy.policy_id,
                POLICY_ID_MAX_LEN
            );
        }
    }
    if let Some(ref bp_id) = policy.applies_to.backend_profile_id {
        if !is_safe_policy_identifier(bp_id) {
            bail!(
                "escalation_policy_v1 '{}': applies_to.backend_profile_id must be 1-{} \
                 ASCII alphanumeric/_.:-characters (P058-SEC-L1)",
                policy.policy_id,
                POLICY_ID_MAX_LEN
            );
        }
    }
    if let Some(ref stage_id) = policy.applies_to.stage_id {
        if !is_safe_policy_identifier(stage_id) {
            bail!(
                "escalation_policy_v1 '{}': applies_to.stage_id must be 1-{} \
                 ASCII alphanumeric/_.:-characters (P058-SEC-L1)",
                policy.policy_id,
                POLICY_ID_MAX_LEN
            );
        }
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
    // SEC-P058-002: reject applies_to combinations that resolve_policy_for_agent will never match.
    // The runtime resolver recognises exactly five binding patterns (scored 1-5). Any selector
    // that combines agent_id with backend_profile_id (with or without stage_id) falls through
    // to the `_ => continue` arm and is silently ignored at runtime — a fail-open that could
    // make an apparently stricter policy have no effect.
    if policy.applies_to.agent_id.is_some() && policy.applies_to.backend_profile_id.is_some() {
        bail!(
            "escalation_policy_v1 '{}': applies_to combining agent_id and backend_profile_id \
             is not a recognised binding pattern and will never be matched at runtime; \
             use a single specificity axis or a recognised two-field combination \
             (stage_id+agent_id or stage_id+backend_profile_id) \
             (escalation_policy_ambiguous_at_compile)",
            policy.policy_id
        );
    }
    for tier in &policy.tiers {
        validate_tier_structure(policy, tier)?;
    }
    Ok(())
}

fn validate_tier_structure(policy: &EscalationPolicyYaml, tier: &EscalationTierYaml) -> Result<()> {
    // P058-SEC-L1: validate tier_id before embedding in error messages.
    if !is_safe_policy_identifier(&tier.tier_id) {
        bail!(
            "escalation_policy_v1 '{}': tier_id must be 1-{} ASCII alphanumeric/_.:-characters; \
             got value that fails identifier validation (P058-SEC-L1)",
            policy.policy_id,
            POLICY_ID_MAX_LEN
        );
    }
    match tier.kind.as_str() {
        "backend_profile" => {
            if tier.backend_profile_id.is_none() {
                bail!(
                    "escalation_policy_v1 '{}' tier '{}': kind 'backend_profile' requires backend_profile_id",
                    policy.policy_id,
                    tier.tier_id
                );
            }
            // P058-SEC-L1: validate backend_profile_id before embedding in error messages.
            if let Some(ref bp_id) = tier.backend_profile_id {
                if !is_safe_policy_identifier(bp_id) {
                    bail!(
                        "escalation_policy_v1 '{}' tier '{}': backend_profile_id must be 1-{} \
                         ASCII alphanumeric/_.:-characters (P058-SEC-L1)",
                        policy.policy_id, tier.tier_id, POLICY_ID_MAX_LEN
                    );
                }
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

/// Validate all escalation policies in a catalog against its backend_profiles and agents.
///
/// Returns a list of compile diagnostics. An empty list means all policies are valid.
/// Any diagnostic should surface as a run preflight failure with the given `pause_reason_code`.
///
/// Checks:
/// - `applies_to.backend_profile_id` must resolve in `catalog.backend_profiles`
///   → `escalation_policy_unknown_backend_profile` (HIGH-001: fail closed)
/// - `applies_to.agent_id` must resolve in `catalog.agents`
///   → `escalation_policy_ambiguous_at_compile` (HIGH-002: fail closed)
/// - `backend_profile` tier references must resolve in `catalog.backend_profiles`
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
        // HIGH-001: validate applies_to.backend_profile_id against catalog backend_profiles.
        // An unknown selector compiles and freezes but silently never matches at runtime,
        // so an operator could believe a stricter escalation policy guards an agent when it
        // is actually inert. Fail closed with escalation_policy_unknown_backend_profile.
        if let Some(ref applies_to_bp_id) = policy.applies_to.backend_profile_id {
            let exists = backend_profiles
                .map(|bp| bp.contains_key(applies_to_bp_id.as_str()))
                .unwrap_or(false);
            if !exists {
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_unknown_backend_profile",
                    detail: format!(
                        "policy '{}' applies_to.backend_profile_id '{}' does not exist in catalog; \
                         the policy would compile but never match any agent execution at runtime",
                        policy.policy_id, applies_to_bp_id
                    ),
                });
            }
        }

        // HIGH-002: validate applies_to.agent_id against catalog agents.
        // An unknown agent_id compiles and freezes but silently never matches at runtime —
        // a typo such as `code_wrter` leaves the policy inert while appearing active.
        // Fail closed with escalation_policy_ambiguous_at_compile.
        if let Some(ref agent_id) = policy.applies_to.agent_id {
            let exists = catalog
                .agents
                .as_deref()
                .map(|agents| agents.iter().any(|a| a.id.as_str() == agent_id.as_str()))
                .unwrap_or(false);
            if !exists {
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_ambiguous_at_compile",
                    detail: format!(
                        "policy '{}' applies_to.agent_id '{}' does not exist in the agent catalog; \
                         the policy would compile but never match any agent execution at runtime",
                        policy.policy_id, agent_id
                    ),
                });
            }
        }

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

/// Check for ambiguous equal-specificity policy bindings.
///
/// Two policies with identical `(applies_to.agent_id, applies_to.backend_profile_id,
/// applies_to.stage_id)` are ambiguous — the runtime cannot deterministically select one.
///
/// Cross-axis ambiguity: a policy bound by `agent_id: X` (one selector) and a policy bound by
/// `backend_profile_id: Y` (one selector) have equal specificity. If agent X uses profile Y,
/// both policies would match the same execution at runtime and the selector would be ambiguous.
/// This is detected using `agent_to_profile` (agent_id → backend_profile_id from the catalog).
///
/// Returns diagnostics with `escalation_policy_ambiguous_at_compile` for all policy IDs
/// that share a binding tuple or produce a cross-axis conflict. Empty if no ambiguity is found.
pub fn validate_policies_for_ambiguous_bindings(
    policies: &[EscalationPolicyYaml],
    agent_to_profile: &std::collections::HashMap<&str, &str>,
) -> Vec<EscalationCompileDiagnostic> {
    use std::collections::HashMap;
    type BindingKey<'a> = (Option<&'a str>, Option<&'a str>, Option<&'a str>);

    // --- identical-tuple check ---
    let mut binding_map: HashMap<BindingKey<'_>, Vec<&str>> = HashMap::new();
    for policy in policies {
        let key: BindingKey<'_> = (
            policy.applies_to.agent_id.as_deref(),
            policy.applies_to.backend_profile_id.as_deref(),
            policy.applies_to.stage_id.as_deref(),
        );
        binding_map.entry(key).or_default().push(&policy.policy_id);
    }

    let mut diagnostics = Vec::new();
    for ((agent_id, bp_id, stage_id), policy_ids) in &binding_map {
        if policy_ids.len() > 1 {
            for &policy_id in policy_ids.iter() {
                let conflicting: Vec<&str> = policy_ids
                    .iter()
                    .copied()
                    .filter(|&p| p != policy_id)
                    .collect();
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: policy_id.to_string(),
                    pause_reason_code: "escalation_policy_ambiguous_at_compile",
                    detail: format!(
                        "policy '{policy_id}' has the same applies_to binding \
                         (agent_id={agent_id:?}, backend_profile_id={bp_id:?}, \
                         stage_id={stage_id:?}) as: {}",
                        conflicting.join(", ")
                    ),
                });
            }
        }
    }

    // --- cross-axis check: agent_id-only vs backend_profile_id-only (equal specificity) ---
    // A policy with exactly one selector (agent_id XOR backend_profile_id, no stage_id) has equal
    // specificity to a policy with the opposite single selector. If the agent uses that profile,
    // both policies match the same execution and the binding is ambiguous.
    let agent_only: Vec<&EscalationPolicyYaml> = policies
        .iter()
        .filter(|p| {
            p.applies_to.agent_id.is_some()
                && p.applies_to.backend_profile_id.is_none()
                && p.applies_to.stage_id.is_none()
        })
        .collect();
    let profile_only: Vec<&EscalationPolicyYaml> = policies
        .iter()
        .filter(|p| {
            p.applies_to.agent_id.is_none()
                && p.applies_to.backend_profile_id.is_some()
                && p.applies_to.stage_id.is_none()
        })
        .collect();

    // --- stage-scoped cross-axis check: {stage_id, agent_id} vs {stage_id, backend_profile_id} ---
    // A policy with {stage_id, agent_id} and a policy with {stage_id, backend_profile_id} have
    // equal specificity within the same stage scope. If the agent uses that profile, both would
    // match and the binding is ambiguous (SEC-M2).
    {
        let stage_agent_only: Vec<&EscalationPolicyYaml> = policies
            .iter()
            .filter(|p| {
                p.applies_to.stage_id.is_some()
                    && p.applies_to.agent_id.is_some()
                    && p.applies_to.backend_profile_id.is_none()
            })
            .collect();
        let stage_profile_only: Vec<&EscalationPolicyYaml> = policies
            .iter()
            .filter(|p| {
                p.applies_to.stage_id.is_some()
                    && p.applies_to.backend_profile_id.is_some()
                    && p.applies_to.agent_id.is_none()
            })
            .collect();
        for sa_policy in &stage_agent_only {
            let stage_id = sa_policy.applies_to.stage_id.as_deref().unwrap();
            let agent_id = sa_policy.applies_to.agent_id.as_deref().unwrap();
            let Some(&agent_profile) = agent_to_profile.get(agent_id) else {
                continue;
            };
            for sp_policy in &stage_profile_only {
                let other_stage_id = sp_policy.applies_to.stage_id.as_deref().unwrap();
                let bp_id = sp_policy.applies_to.backend_profile_id.as_deref().unwrap();
                if stage_id == other_stage_id && agent_profile == bp_id {
                    diagnostics.push(EscalationCompileDiagnostic {
                        policy_id: sa_policy.policy_id.clone(),
                        pause_reason_code: "escalation_policy_ambiguous_at_compile",
                        detail: format!(
                            "policy '{}' (stage_id={stage_id:?}, agent_id={agent_id:?}) and \
                             policy '{}' (stage_id={other_stage_id:?}, backend_profile_id={bp_id:?}) \
                             have equal specificity and both match agent '{agent_id}' which uses \
                             profile '{agent_profile}' in stage '{stage_id}'; declare deterministic \
                             precedence or use a single binding axis",
                            sa_policy.policy_id, sp_policy.policy_id,
                        ),
                    });
                    diagnostics.push(EscalationCompileDiagnostic {
                        policy_id: sp_policy.policy_id.clone(),
                        pause_reason_code: "escalation_policy_ambiguous_at_compile",
                        detail: format!(
                            "policy '{}' (stage_id={other_stage_id:?}, backend_profile_id={bp_id:?}) \
                             and policy '{}' (stage_id={stage_id:?}, agent_id={agent_id:?}) have equal \
                             specificity and both match agent '{agent_id}' which uses profile \
                             '{agent_profile}' in stage '{stage_id}'; declare deterministic \
                             precedence or use a single binding axis",
                            sp_policy.policy_id, sa_policy.policy_id,
                        ),
                    });
                }
            }
        }
    }

    for agent_policy in &agent_only {
        let agent_id = agent_policy.applies_to.agent_id.as_deref().unwrap();
        let Some(&agent_profile) = agent_to_profile.get(agent_id) else {
            continue;
        };
        for profile_policy in &profile_only {
            let bp_id = profile_policy
                .applies_to
                .backend_profile_id
                .as_deref()
                .unwrap();
            if agent_profile == bp_id {
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: agent_policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_ambiguous_at_compile",
                    detail: format!(
                        "policy '{}' (agent_id={agent_id:?}) and policy '{}' \
                         (backend_profile_id={bp_id:?}) have equal specificity and both match \
                         agent '{agent_id}' which uses profile '{agent_profile}'; \
                         declare deterministic precedence or use a single binding axis",
                        agent_policy.policy_id, profile_policy.policy_id,
                    ),
                });
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: profile_policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_ambiguous_at_compile",
                    detail: format!(
                        "policy '{}' (backend_profile_id={bp_id:?}) and policy '{}' \
                         (agent_id={agent_id:?}) have equal specificity and both match \
                         agent '{agent_id}' which uses profile '{agent_profile}'; \
                         declare deterministic precedence or use a single binding axis",
                        profile_policy.policy_id, agent_policy.policy_id,
                    ),
                });
            }
        }
    }

    diagnostics
}

/// Check for policies that target unsafe side-effect/release stages.
///
/// Three binding axes are checked fail-closed (SEC-001):
/// - `applies_to.stage_id` matched directly against `unsafe_stage_ids`
/// - `applies_to.agent_id` matched against `unsafe_agent_ids` (agents that own or run tasks
///   in unsafe stages)
/// - `applies_to.backend_profile_id` matched against `unsafe_backend_profile_ids` (profiles
///   used by agents that appear in unsafe stages)
///
/// Any match produces `escalation_policy_unsafe_for_side_effect_stage`. The caller is
/// responsible for building all three sets from the workflow and catalog definitions.
pub fn validate_policies_for_unsafe_stage_bindings(
    policies: &[EscalationPolicyYaml],
    unsafe_stage_ids: &std::collections::HashSet<String>,
    unsafe_agent_ids: &std::collections::HashSet<String>,
    unsafe_backend_profile_ids: &std::collections::HashSet<String>,
) -> Vec<EscalationCompileDiagnostic> {
    let mut diagnostics = Vec::new();
    for policy in policies {
        // Explicit stage_id binding.
        if let Some(ref stage_id) = policy.applies_to.stage_id {
            if unsafe_stage_ids.contains(stage_id.as_str()) {
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_unsafe_for_side_effect_stage",
                    detail: format!(
                        "policy '{}' targets side-effect/release stage '{}'; \
                         auto-escalation on manual-gate and release stages is not permitted",
                        policy.policy_id, stage_id
                    ),
                });
            }
        }
        // agent_id binding — fail closed if the agent runs in any unsafe stage.
        if let Some(ref agent_id) = policy.applies_to.agent_id {
            if unsafe_agent_ids.contains(agent_id.as_str()) {
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_unsafe_for_side_effect_stage",
                    detail: format!(
                        "policy '{}' targets agent '{}' which runs in a side-effect/release stage; \
                         auto-escalation on manual-gate and release stages is not permitted",
                        policy.policy_id, agent_id
                    ),
                });
            }
        }
        // backend_profile_id binding — fail closed if the profile is used by any agent in an
        // unsafe stage.
        if let Some(ref bp_id) = policy.applies_to.backend_profile_id {
            if unsafe_backend_profile_ids.contains(bp_id.as_str()) {
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_unsafe_for_side_effect_stage",
                    detail: format!(
                        "policy '{}' targets backend_profile '{}' which is used by an agent \
                         running in a side-effect/release stage; \
                         auto-escalation on manual-gate and release stages is not permitted",
                        policy.policy_id, bp_id
                    ),
                });
            }
        }
    }
    diagnostics
}

/// Validate that `applies_to.stage_id` selectors exist in the set of known workflow state IDs.
///
/// Returns diagnostics with `escalation_policy_ambiguous_at_compile` for each policy whose
/// `applies_to.stage_id` does not appear in `all_stage_ids`. An unknown stage_id compiles and
/// freezes but silently never matches at runtime — fail closed per the proposal's binding posture.
///
/// Call this from the compiler after collecting all state IDs from the workflow definition.
pub fn validate_applies_to_stage_selectors(
    policies: &[EscalationPolicyYaml],
    all_stage_ids: &std::collections::HashSet<String>,
) -> Vec<EscalationCompileDiagnostic> {
    let mut diagnostics = Vec::new();
    for policy in policies {
        if let Some(ref stage_id) = policy.applies_to.stage_id {
            if !all_stage_ids.contains(stage_id.as_str()) {
                diagnostics.push(EscalationCompileDiagnostic {
                    policy_id: policy.policy_id.clone(),
                    pause_reason_code: "escalation_policy_ambiguous_at_compile",
                    detail: format!(
                        "policy '{}' applies_to.stage_id '{}' does not exist in the workflow states; \
                         the policy would compile but never match any agent execution at runtime",
                        policy.policy_id, stage_id
                    ),
                });
            }
        }
    }
    diagnostics
}

// ── Policy hashing ─────────────────────────────────────────────────────────────

/// Serialize a JSON value with object keys sorted alphabetically at every level.
/// Using sorted keys makes the hash deterministic regardless of YAML field order.
/// A re-exported YAML with different field order must produce the same hash.
fn sorted_key_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            use std::collections::BTreeMap;
            let sorted: BTreeMap<&str, &serde_json::Value> =
                map.iter().map(|(k, v)| (k.as_str(), v)).collect();
            let inner: Vec<String> = sorted
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(*k).unwrap_or_else(|_| format!("\"{k}\"")),
                        sorted_key_json(v)
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(arr) => {
            let inner: Vec<String> = arr.iter().map(sorted_key_json).collect();
            format!("[{}]", inner.join(","))
        }
        _ => serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()),
    }
}

/// Compute SHA-256 of the canonical sorted-key JSON serialization of a policy.
/// Keys are sorted alphabetically at every level so that field order in the source
/// YAML cannot cause spurious hash differences on re-export.
/// Returns the hash prefixed with `"sha256:"` to match the project-wide hash format.
pub fn compute_policy_hash(policy: &EscalationPolicyYaml) -> Result<String> {
    use sha2::{Digest, Sha256};
    let value = serde_json::to_value(policy)
        .map_err(|e| anyhow::anyhow!("policy hash serialization failed: {e}"))?;
    let canonical = sorted_key_json(&value);
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(format!("sha256:{digest:x}"))
}

// ── Runtime policy resolution ──────────────────────────────────────────────────

/// Resolution result from `resolve_policy_for_agent`.
#[derive(Debug, Clone)]
pub struct ResolvedEscalationPolicy {
    pub policy_id: String,
    pub policy_hash: String,
    pub first_tier_id: String,
    pub first_tier_kind_raw: String,
}

/// Resolve the best-matching escalation policy snapshot for a given agent execution.
///
/// Specificity ordering (most specific wins):
///   1. stage_id + agent_id (most specific)
///   2. stage_id + backend_profile_id
///   3. stage_id only  ← explicit stage binding outranks generic agent/profile bindings
///   4. agent_id only
///   5. backend_profile_id only
///
/// The approved binding contract (P058 architecture.binding_precedence) requires that an
/// explicit workflow-stage policy binding wins over a generic agent or profile binding.
/// stage_id-only (score 3) therefore outranks agent_id-only (score 2) and
/// backend_profile_id-only (score 1). SEC-HIGH-001 fix.
///
/// Returns `None` if no policy matches or the matched policy has `enabled_default = false`.
/// Callers may override with runtime_policy_overrides (Phase 2+); for Phase 1 only catalog
/// `enabled_default` is evaluated.
pub fn resolve_policy_for_agent(
    policies: &[crate::plan::EscalationPolicySnapshot],
    stage_id: &str,
    agent_id: &str,
    backend_profile_id: Option<&str>,
) -> Option<ResolvedEscalationPolicy> {
    // Ranked candidates: score higher = more specific.
    let mut best: Option<(u8, &crate::plan::EscalationPolicySnapshot)> = None;

    for policy in policies {
        if !policy.enabled_default {
            continue;
        }

        let score: u8 = match (
            policy.applies_to_stage_id.as_deref(),
            policy.applies_to_agent_id.as_deref(),
            policy.applies_to_backend_profile_id.as_deref(),
        ) {
            (Some(s), Some(a), None) if s == stage_id && a == agent_id => 5,
            (Some(s), None, Some(bp))
                if s == stage_id && backend_profile_id.map_or(false, |id| id == bp) =>
            {
                4
            }
            // stage_id-only outranks agent/profile-only per approved binding contract
            (Some(s), None, None) if s == stage_id => 3,
            (None, Some(a), None) if a == agent_id => 2,
            (None, None, Some(bp)) if backend_profile_id.map_or(false, |id| id == bp) => 1,
            _ => continue,
        };

        if best.map_or(true, |(prev_score, _)| score > prev_score) {
            best = Some((score, policy));
        }
    }

    best.and_then(|(_, policy)| {
        let first_tier = policy.tiers.first()?;
        Some(ResolvedEscalationPolicy {
            policy_id: policy.policy_id.clone(),
            policy_hash: policy.policy_hash.clone(),
            first_tier_id: first_tier.tier_id.clone(),
            first_tier_kind_raw: first_tier.kind.clone(),
        })
    })
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
        assert_eq!(
            policy.tiers[1].backend_profile_id.as_deref(),
            Some("claude_builder_frontier")
        );
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
        assert!(err.to_string().contains("backend_profile_id"), "got: {err}");
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
        assert!(err.to_string().contains("unknown kind"), "got: {err}");
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
        assert!(
            hash1.starts_with("sha256:"),
            "hash must be prefixed; got: {hash1}"
        );
        assert_eq!(hash1.len(), 7 + 64); // "sha256:" + 64 hex chars
    }

    #[test]
    fn validate_policies_against_catalog_reports_unknown_backend_profile() {
        use crate::catalog::AgentCatalogFile;
        use crate::catalog::BackendProfile;
        use std::collections::HashMap;

        // Use backend_profile_id in applies_to (known) so we isolate the tier-level
        // backend_profile validation from the applies_to selector validation.
        let policy = EscalationPolicyYaml {
            policy_id: "test".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: false,
            applies_to: AppliesToYaml {
                agent_id: None,
                backend_profile_id: Some("existing_profile".into()),
                stage_id: None,
            },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "backend_profile".into(),
                backend_profile_id: Some("missing_profile".into()),
                max_attempts: Some(1),
            }],
        };

        let mut profiles = HashMap::new();
        profiles.insert(
            "existing_profile".to_string(),
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
            backend_profiles: Some(profiles),
            permission_profiles: None,
            agents: None,
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].pause_reason_code,
            "escalation_policy_unknown_backend_profile"
        );
        assert!(diags[0].detail.contains("missing_profile"));
    }

    #[test]
    fn validate_policies_against_catalog_passes_when_profile_exists() {
        use crate::catalog::AgentCatalogFile;
        use crate::catalog::BackendProfile;
        use std::collections::HashMap;

        // Use backend_profile_id in applies_to (known) so both the selector and tier resolve cleanly.
        let policy = EscalationPolicyYaml {
            policy_id: "test".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: false,
            applies_to: AppliesToYaml {
                agent_id: None,
                backend_profile_id: Some("claude_frontier".into()),
                stage_id: None,
            },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "backend_profile".into(),
                backend_profile_id: Some("claude_frontier".into()),
                max_attempts: Some(1),
            }],
        };

        let mut profiles = HashMap::new();
        profiles.insert(
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
            backend_profiles: Some(profiles),
            permission_profiles: None,
            agents: None,
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert!(
            diags.is_empty(),
            "no diagnostics expected when profile exists; got: {diags:?}"
        );
    }

    // HIGH-001 regression: applies_to.backend_profile_id must be validated against catalog.
    // A misspelled or removed selector used to compile and freeze silently, meaning the policy
    // would never match any agent execution at runtime, giving operators a false sense of safety.
    #[test]
    fn validate_policies_against_catalog_rejects_unknown_applies_to_backend_profile() {
        use crate::catalog::AgentCatalogFile;
        use crate::catalog::BackendProfile;
        use std::collections::HashMap;

        let policy = EscalationPolicyYaml {
            policy_id: "test_applies_to_bp".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: true,
            applies_to: AppliesToYaml {
                agent_id: None,
                backend_profile_id: Some("nonexistent_applies_to_profile".into()),
                stage_id: None,
            },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
        };

        let mut profiles = HashMap::new();
        profiles.insert(
            "some_other_profile".to_string(),
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
            backend_profiles: Some(profiles),
            permission_profiles: None,
            agents: None,
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert_eq!(
            diags.len(),
            1,
            "expected one diagnostic for unknown applies_to.backend_profile_id; got: {diags:?}"
        );
        assert_eq!(
            diags[0].pause_reason_code,
            "escalation_policy_unknown_backend_profile"
        );
        assert!(
            diags[0].detail.contains("applies_to.backend_profile_id"),
            "detail should mention applies_to.backend_profile_id; got: {}",
            diags[0].detail
        );
        assert!(diags[0].detail.contains("nonexistent_applies_to_profile"));
    }

    #[test]
    fn validate_policies_against_catalog_passes_when_applies_to_backend_profile_exists() {
        use crate::catalog::AgentCatalogFile;
        use crate::catalog::BackendProfile;
        use std::collections::HashMap;

        let policy = EscalationPolicyYaml {
            policy_id: "test_applies_to_bp_ok".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: true,
            applies_to: AppliesToYaml {
                agent_id: None,
                backend_profile_id: Some("known_profile".into()),
                stage_id: None,
            },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
        };

        let mut profiles = HashMap::new();
        profiles.insert(
            "known_profile".to_string(),
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
            backend_profiles: Some(profiles),
            permission_profiles: None,
            agents: None,
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert!(
            diags.is_empty(),
            "no diagnostics expected for known applies_to.backend_profile_id; got: {diags:?}"
        );
    }

    // HIGH-002 regression: applies_to.agent_id must be validated against catalog agents.
    // A misspelled agent_id (e.g. `code_wrter`) used to compile silently, meaning the policy
    // would never match, giving operators a false sense of safety.
    #[test]
    fn validate_policies_against_catalog_rejects_unknown_applies_to_agent_id() {
        use crate::catalog::{AgentCatalogFile, AgentEntry};

        let policy = EscalationPolicyYaml {
            policy_id: "test_unknown_agent".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: true,
            applies_to: AppliesToYaml {
                agent_id: Some("code_wrter_typo".into()),
                backend_profile_id: None,
                stage_id: None,
            },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
        };

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
            agents: Some(vec![AgentEntry {
                id: "code_writer".into(), // correct name; policy has a typo
                title: None,
                mode: None,
                system_role: None,
                backend_profile: "claude_sonnet".into(),
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
                continuation_capability: None,
                required_tools: None,
                xcode_broker_required: None,
                xcode_shim_injection_signal: None,
                requires_xcode_host_execution: None,
                routing: None,
                toolchain_cache_policy: None,
            }]),
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert_eq!(
            diags.len(),
            1,
            "expected one diagnostic for unknown agent_id; got: {diags:?}"
        );
        assert_eq!(
            diags[0].pause_reason_code, "escalation_policy_ambiguous_at_compile",
            "wrong pause_reason_code; got: {}",
            diags[0].pause_reason_code
        );
        assert!(
            diags[0].detail.contains("code_wrter_typo"),
            "detail must name the missing agent_id; got: {}",
            diags[0].detail
        );
        assert!(
            diags[0].detail.contains("applies_to.agent_id"),
            "detail must reference applies_to.agent_id; got: {}",
            diags[0].detail
        );
    }

    #[test]
    fn validate_policies_against_catalog_passes_when_applies_to_agent_id_exists() {
        use crate::catalog::{AgentCatalogFile, AgentEntry};

        let policy = EscalationPolicyYaml {
            policy_id: "test_known_agent".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: false,
            applies_to: AppliesToYaml {
                agent_id: Some("code_writer".into()),
                backend_profile_id: None,
                stage_id: None,
            },
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
        };

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
            agents: Some(vec![AgentEntry {
                id: "code_writer".into(),
                title: None,
                mode: None,
                system_role: None,
                backend_profile: "claude_sonnet".into(),
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
                continuation_capability: None,
                required_tools: None,
                xcode_broker_required: None,
                xcode_shim_injection_signal: None,
                requires_xcode_host_execution: None,
                routing: None,
                toolchain_cache_policy: None,
            }]),
            escalation_policies: Some(vec![policy]),
        };

        let diags = validate_policies_against_catalog(&catalog);
        assert!(
            diags.is_empty(),
            "no diagnostics expected for known applies_to.agent_id; got: {diags:?}"
        );
    }

    // validate_applies_to_stage_selectors tests
    #[test]
    fn validate_applies_to_stage_selectors_rejects_unknown_stage() {
        use std::collections::HashSet;

        let policy = EscalationPolicyYaml {
            policy_id: "test_unknown_stage".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: true,
            applies_to: AppliesToYaml {
                agent_id: None,
                backend_profile_id: None,
                stage_id: Some("state_999_nonexistent".into()),
            },
            max_chain_attempts: 2,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
        };

        let known_stages: HashSet<String> = ["state_implementation", "state_review"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let diags = validate_applies_to_stage_selectors(&[policy], &known_stages);
        assert_eq!(
            diags.len(),
            1,
            "expected one diagnostic for unknown stage_id; got: {diags:?}"
        );
        assert_eq!(
            diags[0].pause_reason_code,
            "escalation_policy_ambiguous_at_compile"
        );
        assert!(
            diags[0].detail.contains("state_999_nonexistent"),
            "got: {}",
            diags[0].detail
        );
    }

    #[test]
    fn validate_applies_to_stage_selectors_passes_for_known_stage() {
        use std::collections::HashSet;

        let policy = EscalationPolicyYaml {
            policy_id: "test_known_stage".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: false,
            applies_to: AppliesToYaml {
                agent_id: None,
                backend_profile_id: None,
                stage_id: Some("state_implementation".into()),
            },
            max_chain_attempts: 2,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec![EscalationTriggerYaml::ContractOutputFailure],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
        };

        let known_stages: HashSet<String> = ["state_implementation", "state_review"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let diags = validate_applies_to_stage_selectors(&[policy], &known_stages);
        assert!(
            diags.is_empty(),
            "no diagnostics for known stage_id; got: {diags:?}"
        );
    }

    #[test]
    fn validate_applies_to_stage_selectors_skips_policies_without_stage_id() {
        use std::collections::HashSet;

        let policy = EscalationPolicyYaml {
            policy_id: "no_stage_selector".into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: false,
            applies_to: AppliesToYaml {
                agent_id: None,
                backend_profile_id: Some("some_profile".into()),
                stage_id: None, // no stage_id — must not produce any diagnostic
            },
            max_chain_attempts: 1,
            max_chain_wall_clock_seconds: 60,
            triggers: vec![EscalationTriggerYaml::StaleNoOutput],
            tiers: vec![EscalationTierYaml {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
        };

        let known_stages: HashSet<String> = HashSet::new(); // empty — would reject any stage_id

        let diags = validate_applies_to_stage_selectors(&[policy], &known_stages);
        assert!(
            diags.is_empty(),
            "policies without applies_to.stage_id must not produce diagnostics; got: {diags:?}"
        );
    }

    // ── SEC-HIGH-001 regression tests ─────────────────────────────────────────
    // Stage-scoped policy binding must outrank generic agent/profile bindings per
    // the approved P058 architecture.binding_precedence contract.

    fn make_snapshot(
        policy_id: &str,
        stage_id: Option<&str>,
        agent_id: Option<&str>,
        bp_id: Option<&str>,
        enabled: bool,
    ) -> crate::plan::EscalationPolicySnapshot {
        crate::plan::EscalationPolicySnapshot {
            policy_id: policy_id.into(),
            schema_version: SCHEMA_VERSION.into(),
            enabled_default: enabled,
            applies_to_stage_id: stage_id.map(String::from),
            applies_to_agent_id: agent_id.map(String::from),
            applies_to_backend_profile_id: bp_id.map(String::from),
            max_chain_attempts: 3,
            max_chain_wall_clock_seconds: 3600,
            triggers: vec!["contract_output_failure".into()],
            tiers: vec![crate::plan::EscalationTierSnapshot {
                tier_id: "t1".into(),
                kind: "pause".into(),
                backend_profile_id: None,
                max_attempts: None,
            }],
            policy_hash: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .into(),
            digest_version: None,
            rollout_override_state: None,
        }
    }

    #[test]
    fn resolve_stage_only_wins_over_agent_only() {
        // SEC-HIGH-001: stage_id-only binding (score 3) must outrank agent_id-only (score 2).
        let stage_policy = make_snapshot(
            "stage_policy",
            Some("state_implementation"),
            None,
            None,
            true,
        );
        let agent_policy = make_snapshot("agent_policy", None, Some("code_writer"), None, true);
        let policies = vec![agent_policy, stage_policy]; // agent first to catch ordering bugs

        let result = resolve_policy_for_agent(
            &policies,
            "state_implementation",
            "code_writer",
            Some("claude_sonnet"),
        );
        assert!(result.is_some(), "expected a matching policy");
        assert_eq!(
            result.unwrap().policy_id,
            "stage_policy",
            "stage_id-only binding must win over agent_id-only binding"
        );
    }

    #[test]
    fn resolve_stage_only_wins_over_backend_profile_only() {
        // SEC-HIGH-001: stage_id-only binding (score 3) must outrank backend_profile_id-only (score 1).
        let stage_policy = make_snapshot(
            "stage_policy",
            Some("state_implementation"),
            None,
            None,
            true,
        );
        let profile_policy =
            make_snapshot("profile_policy", None, None, Some("claude_sonnet"), true);
        let policies = vec![profile_policy, stage_policy];

        let result = resolve_policy_for_agent(
            &policies,
            "state_implementation",
            "code_writer",
            Some("claude_sonnet"),
        );
        assert!(result.is_some(), "expected a matching policy");
        assert_eq!(
            result.unwrap().policy_id,
            "stage_policy",
            "stage_id-only binding must win over backend_profile_id-only binding"
        );
    }

    #[test]
    fn resolve_stage_plus_agent_wins_over_stage_only() {
        // Finer specificity: stage_id+agent_id (score 5) beats stage_id-only (score 3).
        let stage_agent_policy = make_snapshot(
            "stage_agent_policy",
            Some("state_implementation"),
            Some("code_writer"),
            None,
            true,
        );
        let stage_only_policy = make_snapshot(
            "stage_only_policy",
            Some("state_implementation"),
            None,
            None,
            true,
        );
        let policies = vec![stage_only_policy, stage_agent_policy];

        let result =
            resolve_policy_for_agent(&policies, "state_implementation", "code_writer", None);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().policy_id,
            "stage_agent_policy",
            "stage_id+agent_id binding must win over stage_id-only binding"
        );
    }

    #[test]
    fn resolve_agent_only_wins_over_profile_only() {
        // agent_id-only (score 2) beats backend_profile_id-only (score 1) for an agent whose
        // profile matches the profile-only policy.
        let agent_policy = make_snapshot("agent_policy", None, Some("code_writer"), None, true);
        let profile_policy =
            make_snapshot("profile_policy", None, None, Some("claude_sonnet"), true);
        let policies = vec![profile_policy, agent_policy];

        let result = resolve_policy_for_agent(
            &policies,
            "state_impl",
            "code_writer",
            Some("claude_sonnet"),
        );
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().policy_id,
            "agent_policy",
            "agent_id-only binding must win over backend_profile_id-only binding"
        );
    }

    #[test]
    fn resolve_returns_none_when_stage_does_not_match() {
        // Stage-only policy for a DIFFERENT stage must not match.
        let stage_policy = make_snapshot("stage_policy", Some("state_review"), None, None, true);
        let policies = vec![stage_policy];

        let result =
            resolve_policy_for_agent(&policies, "state_implementation", "code_writer", None);
        assert!(
            result.is_none(),
            "stage_id-only policy for 'state_review' must not match 'state_implementation'"
        );
    }

    #[test]
    fn resolve_returns_none_when_disabled() {
        // enabled_default=false must not select, even for a perfect match.
        let policy = make_snapshot(
            "disabled_policy",
            Some("state_implementation"),
            Some("code_writer"),
            None,
            false,
        );
        let policies = vec![policy];

        let result =
            resolve_policy_for_agent(&policies, "state_implementation", "code_writer", None);
        assert!(
            result.is_none(),
            "disabled policy (enabled_default=false) must not be selected"
        );
    }
}
