pub mod approvals;
pub mod artifacts;
pub mod ideas;
pub mod reports;
pub mod runs;
pub mod stages;
pub mod steward;

use crate::protocol::McpTool;
use domain::CapabilityToolId;

pub fn all_capability_tool_ids() -> [CapabilityToolId; 22] {
    [
        CapabilityToolId::IdeasCreate,
        CapabilityToolId::IdeasList,
        CapabilityToolId::RunsStart,
        CapabilityToolId::RunsList,
        CapabilityToolId::RunsGet,
        CapabilityToolId::RunsMainSyncRequest,
        CapabilityToolId::RunsMainSyncRetry,
        CapabilityToolId::RunsMainSyncSetOverride,
        CapabilityToolId::RunsMainSyncRepairState,
        CapabilityToolId::RunsMainSyncRecordRecoveryDecision,
        CapabilityToolId::RunsKnowledgeCapsuleIgnore,
        CapabilityToolId::RunsCancel,
        CapabilityToolId::ApprovalsList,
        CapabilityToolId::ApprovalsResolve,
        CapabilityToolId::StagesRetry,
        CapabilityToolId::WorkflowConflictsResolve,
        CapabilityToolId::LegacyDiscoveryOverrideCreate,
        CapabilityToolId::ReportsGet,
        CapabilityToolId::ArtifactsOverrideContract,
        CapabilityToolId::StewardRunAnalysis,
        CapabilityToolId::StewardListAnalyses,
        CapabilityToolId::StewardGetAnalysis,
    ]
}

pub fn p064_operator_tool_enabled(tool_name: &str) -> bool {
    let tool_name = canonical_tool_name(tool_name);
    !matches!(
        tool_name,
        "runs.main_sync.request"
            | "runs.main_sync.retry"
            | "runs.main_sync.set_override"
            | "runs.main_sync.repair_state"
            | "runs.main_sync.record_recovery_decision"
            | "runs.knowledge_capsule.ignore"
    )
}

pub fn capability_id_for(tool_name: &str) -> Option<CapabilityToolId> {
    let tool_name = canonical_tool_name(tool_name);
    match tool_name {
        "ideas.create" => Some(CapabilityToolId::IdeasCreate),
        "ideas.list" => Some(CapabilityToolId::IdeasList),
        "runs.start" => Some(CapabilityToolId::RunsStart),
        "runs.list" => Some(CapabilityToolId::RunsList),
        "runs.get" => Some(CapabilityToolId::RunsGet),
        "runs.main_sync.request" => Some(CapabilityToolId::RunsMainSyncRequest),
        "runs.main_sync.retry" => Some(CapabilityToolId::RunsMainSyncRetry),
        "runs.main_sync.set_override" => Some(CapabilityToolId::RunsMainSyncSetOverride),
        "runs.main_sync.repair_state" => Some(CapabilityToolId::RunsMainSyncRepairState),
        "runs.main_sync.record_recovery_decision" => {
            Some(CapabilityToolId::RunsMainSyncRecordRecoveryDecision)
        }
        "runs.knowledge_capsule.ignore" => Some(CapabilityToolId::RunsKnowledgeCapsuleIgnore),
        "runs.cancel" => Some(CapabilityToolId::RunsCancel),
        "approvals.list" => Some(CapabilityToolId::ApprovalsList),
        "approvals.resolve" => Some(CapabilityToolId::ApprovalsResolve),
        "stages.retry" => Some(CapabilityToolId::StagesRetry),
        "workflow_conflicts.resolve" => Some(CapabilityToolId::WorkflowConflictsResolve),
        "legacy_discovery_override_create" => Some(CapabilityToolId::LegacyDiscoveryOverrideCreate),
        "reports.get" => Some(CapabilityToolId::ReportsGet),
        "artifacts.override_contract" => Some(CapabilityToolId::ArtifactsOverrideContract),
        "steward.run_analysis" => Some(CapabilityToolId::StewardRunAnalysis),
        "steward.list_analyses" => Some(CapabilityToolId::StewardListAnalyses),
        "steward.get_analysis" => Some(CapabilityToolId::StewardGetAnalysis),
        _ => None,
    }
}

pub fn canonical_tool_name(tool_name: &str) -> &str {
    match tool_name {
        "ideas_create" => "ideas.create",
        "ideas_list" => "ideas.list",
        "runs_start" => "runs.start",
        "runs_list" => "runs.list",
        "runs_get" => "runs.get",
        "runs_main_sync_request" => "runs.main_sync.request",
        "runs_main_sync_retry" => "runs.main_sync.retry",
        "runs_main_sync_set_override" => "runs.main_sync.set_override",
        "runs_main_sync_repair_state" => "runs.main_sync.repair_state",
        "runs_main_sync_record_recovery_decision" => "runs.main_sync.record_recovery_decision",
        "runs_knowledge_capsule_ignore" => "runs.knowledge_capsule.ignore",
        "runs_cancel" => "runs.cancel",
        "approvals_list" => "approvals.list",
        "approvals_resolve" => "approvals.resolve",
        "stages_retry" => "stages.retry",
        "workflow_conflicts_resolve" => "workflow_conflicts.resolve",
        "reports_get" => "reports.get",
        "artifacts_override_contract" => "artifacts.override_contract",
        "steward_run_analysis" => "steward.run_analysis",
        "steward_list_analyses" => "steward.list_analyses",
        "steward_get_analysis" => "steward.get_analysis",
        _ => tool_name,
    }
}

pub fn codex_compatible_tool(mut tool: McpTool) -> McpTool {
    tool.name = tool.name.replace('.', "_");
    tool
}

pub fn mcp_tool_for(id: CapabilityToolId) -> McpTool {
    match id {
        CapabilityToolId::IdeasCreate => tool_spec_by_name(ideas::tool_specs(), "ideas.create"),
        CapabilityToolId::IdeasList => tool_spec_by_name(ideas::tool_specs(), "ideas.list"),
        CapabilityToolId::RunsStart => tool_spec_by_name(runs::tool_specs(), "runs.start"),
        CapabilityToolId::RunsList => tool_spec_by_name(runs::tool_specs(), "runs.list"),
        CapabilityToolId::RunsGet => tool_spec_by_name(runs::tool_specs(), "runs.get"),
        CapabilityToolId::RunsMainSyncRequest => {
            tool_spec_by_name(runs::tool_specs(), "runs.main_sync.request")
        }
        CapabilityToolId::RunsMainSyncRetry => {
            tool_spec_by_name(runs::tool_specs(), "runs.main_sync.retry")
        }
        CapabilityToolId::RunsMainSyncSetOverride => {
            tool_spec_by_name(runs::tool_specs(), "runs.main_sync.set_override")
        }
        CapabilityToolId::RunsMainSyncRepairState => {
            tool_spec_by_name(runs::tool_specs(), "runs.main_sync.repair_state")
        }
        CapabilityToolId::RunsMainSyncRecordRecoveryDecision => tool_spec_by_name(
            runs::tool_specs(),
            "runs.main_sync.record_recovery_decision",
        ),
        CapabilityToolId::RunsKnowledgeCapsuleIgnore => {
            tool_spec_by_name(runs::tool_specs(), "runs.knowledge_capsule.ignore")
        }
        CapabilityToolId::RunsCancel => tool_spec_by_name(runs::tool_specs(), "runs.cancel"),
        CapabilityToolId::ApprovalsList => {
            tool_spec_by_name(approvals::tool_specs(), "approvals.list")
        }
        CapabilityToolId::ApprovalsResolve => {
            tool_spec_by_name(approvals::tool_specs(), "approvals.resolve")
        }
        CapabilityToolId::StagesRetry => tool_spec_by_name(stages::tool_specs(), "stages.retry"),
        CapabilityToolId::WorkflowConflictsResolve => {
            tool_spec_by_name(stages::tool_specs(), "workflow_conflicts.resolve")
        }
        CapabilityToolId::LegacyDiscoveryOverrideCreate => {
            tool_spec_by_name(stages::tool_specs(), "legacy_discovery_override_create")
        }
        CapabilityToolId::ReportsGet => tool_spec_by_name(reports::tool_specs(), "reports.get"),
        CapabilityToolId::ArtifactsOverrideContract => {
            tool_spec_by_name(artifacts::tool_specs(), "artifacts.override_contract")
        }
        CapabilityToolId::StewardRunAnalysis => {
            tool_spec_by_name(steward::tool_specs(), "steward.run_analysis")
        }
        CapabilityToolId::StewardListAnalyses => {
            tool_spec_by_name(steward::tool_specs(), "steward.list_analyses")
        }
        CapabilityToolId::StewardGetAnalysis => {
            tool_spec_by_name(steward::tool_specs(), "steward.get_analysis")
        }
        CapabilityToolId::ProposalGateSettle => {
            tool_spec_by_name(runs::tool_specs(), "runs.settle_proposal_gate")
        }
    }
}

pub fn all_tool_specs() -> Vec<McpTool> {
    let mut specs = Vec::new();
    specs.extend(ideas::tool_specs());
    specs.extend(runs::tool_specs());
    specs.extend(approvals::tool_specs());
    specs.extend(stages::tool_specs());
    specs.extend(reports::tool_specs());
    specs.extend(artifacts::tool_specs());
    specs.extend(steward::tool_specs());
    specs
}

fn tool_spec_by_name(specs: Vec<McpTool>, name: &str) -> McpTool {
    specs
        .into_iter()
        .find(|tool| tool.name == name)
        .expect("explicit CapabilityToolId mapping must reference a registered MCP tool")
}

#[cfg(test)]
mod tests {
    use domain::CapabilityToolId;

    #[test]
    fn mcp_tool_converter_covers_registered_tools() {
        assert_eq!(
            super::capability_id_for("runs.start"),
            Some(CapabilityToolId::RunsStart)
        );
        assert_eq!(
            super::capability_id_for("steward.run_analysis"),
            Some(CapabilityToolId::StewardRunAnalysis)
        );
        assert_eq!(
            super::capability_id_for("runs.main_sync.request"),
            Some(CapabilityToolId::RunsMainSyncRequest)
        );
        assert_eq!(
            super::mcp_tool_for(CapabilityToolId::RunsKnowledgeCapsuleIgnore).name,
            "runs.knowledge_capsule.ignore"
        );
        assert_eq!(
            super::capability_id_for("legacy_discovery_override_create"),
            Some(CapabilityToolId::LegacyDiscoveryOverrideCreate)
        );
        assert_eq!(
            super::capability_id_for("workflow_conflicts.resolve"),
            Some(CapabilityToolId::WorkflowConflictsResolve)
        );
        assert_eq!(
            super::capability_id_for("workflow_conflicts_resolve"),
            Some(CapabilityToolId::WorkflowConflictsResolve)
        );
        assert_eq!(
            super::capability_id_for("runs_list"),
            Some(CapabilityToolId::RunsList)
        );
        assert_eq!(
            super::mcp_tool_for(CapabilityToolId::StewardGetAnalysis).name,
            "steward.get_analysis"
        );
        assert_eq!(
            super::mcp_tool_for(CapabilityToolId::LegacyDiscoveryOverrideCreate).name,
            "legacy_discovery_override_create"
        );
        assert_eq!(
            super::mcp_tool_for(CapabilityToolId::WorkflowConflictsResolve).name,
            "workflow_conflicts.resolve"
        );
        assert_eq!(super::capability_id_for("missing.tool"), None);
    }

    #[test]
    fn p064_operator_tools_are_registered_but_hidden_until_modes_enable_runtime() {
        assert_eq!(
            super::capability_id_for("runs.main_sync.request"),
            Some(CapabilityToolId::RunsMainSyncRequest)
        );
        assert_eq!(
            super::capability_id_for("runs.knowledge_capsule.ignore"),
            Some(CapabilityToolId::RunsKnowledgeCapsuleIgnore)
        );
        assert!(!super::p064_operator_tool_enabled("runs.main_sync.request"));
        assert!(!super::p064_operator_tool_enabled(
            "runs.knowledge_capsule.ignore"
        ));
        assert!(super::p064_operator_tool_enabled("runs.get"));
    }
}
