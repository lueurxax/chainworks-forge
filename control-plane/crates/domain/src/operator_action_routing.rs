//! P072: Typed operator-action routing registry.
//!
//! This module defines the canonical runtime registry that enumerates
//! operator actions, allowed northbound surfaces, principal profiles,
//! and mutation/tool capability bindings. The registry is the single
//! source of truth once Phase 1 lands; earlier phases used the
//! proposal-owned fixture at
//! `docs/proposals/072-artifacts/operator-action-routing-policy.v1.json`.

use serde::{Deserialize, Serialize};

/// Which northbound surfaces can invoke an operator action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NorthboundSurface {
    Graphql,
    Mcp,
}

/// GraphQL binding for an operator action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphqlBinding {
    pub mutation_name: String,
}

/// MCP binding for an operator action.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpBinding {
    pub tool_name: String,
    pub subject_kind: Option<String>,
}

/// A single entry in the operator-action routing registry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperatorActionEntry {
    pub action_id: String,
    pub description: String,
    pub graphql: Option<GraphqlBinding>,
    pub mcp: Option<McpBinding>,
    /// Which GraphQL mutations ui_operator is allowed to invoke for this action.
    pub ui_operator_graphql_allowed: bool,
}

/// The typed runtime registry. Constructed once at startup from the
/// compiled constant list; drives auth checks, gate exports, and
/// dependent-proposal drift verification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperatorActionRoutingRegistry {
    pub schema_version: u32,
    pub entries: Vec<OperatorActionEntry>,
    /// Exact GraphQL mutations allowed for ui_operator.
    pub ui_operator_allowed_mutations: Vec<String>,
    /// GraphQL mutations forbidden for ui_operator.
    pub ui_operator_forbidden_mutations: Vec<String>,
}

impl OperatorActionRoutingRegistry {
    /// Build the canonical P072 registry from the compiled constant list.
    pub fn build() -> Self {
        let entries = vec![
            OperatorActionEntry {
                action_id: "approve_stage_approval".into(),
                description: "Approve a pending stage approval gate".into(),
                graphql: Some(GraphqlBinding {
                    mutation_name: "approveApproval".into(),
                }),
                mcp: Some(McpBinding {
                    tool_name: "approvals.resolve".into(),
                    subject_kind: Some("stage_approval".into()),
                }),
                ui_operator_graphql_allowed: true,
            },
            OperatorActionEntry {
                action_id: "reject_stage_approval".into(),
                description: "Reject a pending stage approval gate with reason".into(),
                graphql: Some(GraphqlBinding {
                    mutation_name: "rejectApproval".into(),
                }),
                mcp: Some(McpBinding {
                    tool_name: "approvals.resolve".into(),
                    subject_kind: Some("stage_approval".into()),
                }),
                ui_operator_graphql_allowed: true,
            },
            OperatorActionEntry {
                action_id: "resolve_lead_mediation_confirmation".into(),
                description: "Resolve a lead mediation confirmation".into(),
                graphql: None,
                mcp: Some(McpBinding {
                    tool_name: "approvals.resolve".into(),
                    subject_kind: Some("lead_mediation_confirmation".into()),
                }),
                ui_operator_graphql_allowed: false,
            },
            OperatorActionEntry {
                action_id: "start_run".into(),
                description: "Start a new workflow run".into(),
                graphql: Some(GraphqlBinding {
                    mutation_name: "startRun".into(),
                }),
                mcp: Some(McpBinding {
                    tool_name: "runs.start".into(),
                    subject_kind: None,
                }),
                ui_operator_graphql_allowed: false,
            },
            OperatorActionEntry {
                action_id: "cancel_run".into(),
                description: "Cancel an active run".into(),
                graphql: Some(GraphqlBinding {
                    mutation_name: "cancelRun".into(),
                }),
                mcp: Some(McpBinding {
                    tool_name: "runs.cancel".into(),
                    subject_kind: None,
                }),
                ui_operator_graphql_allowed: false,
            },
            OperatorActionEntry {
                action_id: "retry_stage".into(),
                description: "Retry a failed or rejected stage".into(),
                graphql: Some(GraphqlBinding {
                    mutation_name: "retryStage".into(),
                }),
                mcp: Some(McpBinding {
                    tool_name: "stages.retry".into(),
                    subject_kind: None,
                }),
                ui_operator_graphql_allowed: false,
            },
            OperatorActionEntry {
                action_id: "resolve_workflow_conflict".into(),
                description: "Resolve a workflow conflict by selecting a transition".into(),
                graphql: None,
                mcp: Some(McpBinding {
                    tool_name: "workflow_conflicts.resolve".into(),
                    subject_kind: None,
                }),
                ui_operator_graphql_allowed: false,
            },
            OperatorActionEntry {
                action_id: "override_legacy_discovery_policy".into(),
                description: "Override legacy discovery policy for a stage".into(),
                graphql: Some(GraphqlBinding {
                    mutation_name: "overrideLegacyDiscoveryPolicy".into(),
                }),
                mcp: Some(McpBinding {
                    tool_name: "legacy_discovery_override_create".into(),
                    subject_kind: None,
                }),
                ui_operator_graphql_allowed: false,
            },
        ];

        let ui_operator_allowed_mutations: Vec<String> = entries
            .iter()
            .filter(|e| e.ui_operator_graphql_allowed)
            .filter_map(|e| e.graphql.as_ref().map(|g| g.mutation_name.clone()))
            .collect();

        let ui_operator_forbidden_mutations: Vec<String> = entries
            .iter()
            .filter(|e| !e.ui_operator_graphql_allowed)
            .filter_map(|e| e.graphql.as_ref().map(|g| g.mutation_name.clone()))
            .collect();

        Self {
            schema_version: 1,
            entries,
            ui_operator_allowed_mutations,
            ui_operator_forbidden_mutations,
        }
    }

    /// Check if a GraphQL mutation name is in the ui_operator allowlist.
    pub fn is_ui_operator_mutation_allowed(&self, mutation_name: &str) -> bool {
        self.ui_operator_allowed_mutations
            .iter()
            .any(|m| m == mutation_name)
    }

    /// Serialize the registry to a JSON export suitable for gate tests
    /// and CI comparison against the proposal fixture.
    pub fn to_generated_export(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("registry serializes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_builds_with_expected_entries() {
        let registry = OperatorActionRoutingRegistry::build();
        assert!(registry.entries.len() >= 8);
        assert_eq!(registry.schema_version, 1);
    }

    #[test]
    fn ui_operator_allowed_mutations_are_exactly_two() {
        let registry = OperatorActionRoutingRegistry::build();
        assert_eq!(
            registry.ui_operator_allowed_mutations,
            vec!["approveApproval", "rejectApproval"]
        );
    }

    #[test]
    fn ui_operator_forbidden_mutations_exclude_approval() {
        let registry = OperatorActionRoutingRegistry::build();
        assert!(registry
            .ui_operator_forbidden_mutations
            .contains(&"startRun".to_string()));
        assert!(registry
            .ui_operator_forbidden_mutations
            .contains(&"cancelRun".to_string()));
        assert!(!registry
            .ui_operator_forbidden_mutations
            .contains(&"approveApproval".to_string()));
    }

    #[test]
    fn is_ui_operator_mutation_allowed_checks() {
        let registry = OperatorActionRoutingRegistry::build();
        assert!(registry.is_ui_operator_mutation_allowed("approveApproval"));
        assert!(registry.is_ui_operator_mutation_allowed("rejectApproval"));
        assert!(!registry.is_ui_operator_mutation_allowed("startRun"));
        assert!(!registry.is_ui_operator_mutation_allowed("cancelRun"));
        assert!(!registry.is_ui_operator_mutation_allowed("retryStage"));
    }

    #[test]
    fn registry_export_roundtrips() {
        let registry = OperatorActionRoutingRegistry::build();
        let export = registry.to_generated_export();
        let round: OperatorActionRoutingRegistry =
            serde_json::from_value(export).expect("deserializes");
        assert_eq!(round.schema_version, 1);
        assert_eq!(
            round.ui_operator_allowed_mutations,
            registry.ui_operator_allowed_mutations
        );
    }
}
