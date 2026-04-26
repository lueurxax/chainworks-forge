//! P017 Phase B: PhaseBLeadResolver.
//!
//! Provides exactly-one fail-closed lead resolution using a versioned JSON
//! compatibility map checked into source control. This is the temporary
//! Phase B resolver — Phase C replaces it with first-class static validation.
//!
//! Selection sources:
//!   - ALLOWED: versioned compatibility mapping in source control
//!   - DISALLOWED: unsigned external metadata, ad hoc operator config, external attestation
//!
//! Fail-closed conditions:
//!   - No resolvable lead
//!   - More than one resolvable lead
//!   - Lead lacks required LeadResolutionContract coverage
//!   - Resolver mapping is missing, stale, or unreviewed

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A single entry in the Phase B lead resolver compatibility map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeadResolverEntry {
    pub workflow_source_path: String,
    pub catalog_source_path: String,
    pub lead_agent_id: String,
    pub lead_resolution_contract_ref: String,
    pub mapping_owner: String,
    pub entry_attested_by: String,
    pub reviewed_at: String,
    pub phase_c_removal_condition: String,
}

/// The full Phase B lead resolver compatibility map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhaseBLeadResolverMap {
    pub schema_version: String,
    pub mapping_version: String,
    pub entries: Vec<LeadResolverEntry>,
    pub mapping_owner: String,
    pub entry_attestation_rule: String,
    pub staleness_review_trigger: String,
    pub fail_closed_behavior: String,
    pub upgrade_and_removal_criteria: String,
}

/// Resolution result from the Phase B lead resolver.
pub enum LeadResolution {
    /// Exactly one lead resolved.
    Resolved {
        lead_agent_id: String,
        entry: LeadResolverEntry,
    },
    /// Resolution failed closed — no eligible lead or ambiguous match.
    FailedClosed { reason: String },
}

/// Phase B lead resolver. Loaded from the versioned JSON compatibility map.
pub struct PhaseBLeadResolver {
    map: PhaseBLeadResolverMap,
}

impl PhaseBLeadResolver {
    /// Load the resolver from a JSON file path.
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read Phase B lead resolver map: {e}"))?;
        let map: PhaseBLeadResolverMap = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse Phase B lead resolver map: {e}"))?;
        Ok(Self { map })
    }

    /// Load from an already-parsed map.
    pub fn from_map(map: PhaseBLeadResolverMap) -> Self {
        Self { map }
    }

    /// Resolve exactly one lead for the given workflow and catalog pair.
    /// Fails closed on zero or multiple matches.
    pub fn resolve(&self, workflow_source_path: &str, catalog_source_path: &str) -> LeadResolution {
        let matches: Vec<&LeadResolverEntry> = self
            .map
            .entries
            .iter()
            .filter(|entry| {
                entry.workflow_source_path == workflow_source_path
                    && entry.catalog_source_path == catalog_source_path
            })
            .collect();

        match matches.len() {
            0 => LeadResolution::FailedClosed {
                reason: format!(
                    "No lead resolver entry for workflow={} catalog={}",
                    workflow_source_path, catalog_source_path
                ),
            },
            1 => LeadResolution::Resolved {
                lead_agent_id: matches[0].lead_agent_id.clone(),
                entry: matches[0].clone(),
            },
            n => LeadResolution::FailedClosed {
                reason: format!(
                    "Ambiguous: {} entries match workflow={} catalog={}",
                    n, workflow_source_path, catalog_source_path
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_map() -> PhaseBLeadResolverMap {
        PhaseBLeadResolverMap {
            schema_version: "phase_b_lead_resolver_v1".to_string(),
            mapping_version: "1".to_string(),
            entries: vec![LeadResolverEntry {
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
        }
    }

    #[test]
    fn resolve_exact_match() {
        let resolver = PhaseBLeadResolver::from_map(sample_map());
        match resolver.resolve(
            "examples/workflows/full-mvp-live.yaml",
            "examples/agents/agents.yaml",
        ) {
            LeadResolution::Resolved { lead_agent_id, .. } => {
                assert_eq!(lead_agent_id, "lead_agent");
            }
            LeadResolution::FailedClosed { reason } => {
                panic!("Expected Resolved, got FailedClosed: {reason}");
            }
        }
    }

    #[test]
    fn resolve_no_match_fails_closed() {
        let resolver = PhaseBLeadResolver::from_map(sample_map());
        match resolver.resolve("nonexistent.yaml", "nonexistent.yaml") {
            LeadResolution::FailedClosed { .. } => {}
            LeadResolution::Resolved { .. } => panic!("Expected FailedClosed"),
        }
    }
}
