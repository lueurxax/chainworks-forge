pub mod approvals;
pub mod artifacts;
pub mod ideas;
pub mod reports;
pub mod runs;
pub mod stages;
pub mod steward;

use crate::protocol::McpTool;
use domain::CapabilityToolId;

pub fn all_capability_tool_ids() -> [CapabilityToolId; 14] {
    [
        CapabilityToolId::IdeasCreate,
        CapabilityToolId::IdeasList,
        CapabilityToolId::RunsStart,
        CapabilityToolId::RunsList,
        CapabilityToolId::RunsGet,
        CapabilityToolId::RunsCancel,
        CapabilityToolId::ApprovalsList,
        CapabilityToolId::ApprovalsResolve,
        CapabilityToolId::StagesRetry,
        CapabilityToolId::ReportsGet,
        CapabilityToolId::ArtifactsOverrideContract,
        CapabilityToolId::StewardRunAnalysis,
        CapabilityToolId::StewardListAnalyses,
        CapabilityToolId::StewardGetAnalysis,
    ]
}

pub fn capability_id_for(tool_name: &str) -> Option<CapabilityToolId> {
    match tool_name {
        "ideas.create" => Some(CapabilityToolId::IdeasCreate),
        "ideas.list" => Some(CapabilityToolId::IdeasList),
        "runs.start" => Some(CapabilityToolId::RunsStart),
        "runs.list" => Some(CapabilityToolId::RunsList),
        "runs.get" => Some(CapabilityToolId::RunsGet),
        "runs.cancel" => Some(CapabilityToolId::RunsCancel),
        "approvals.list" => Some(CapabilityToolId::ApprovalsList),
        "approvals.resolve" => Some(CapabilityToolId::ApprovalsResolve),
        "stages.retry" => Some(CapabilityToolId::StagesRetry),
        "reports.get" => Some(CapabilityToolId::ReportsGet),
        "artifacts.override_contract" => Some(CapabilityToolId::ArtifactsOverrideContract),
        "steward.run_analysis" => Some(CapabilityToolId::StewardRunAnalysis),
        "steward.list_analyses" => Some(CapabilityToolId::StewardListAnalyses),
        "steward.get_analysis" => Some(CapabilityToolId::StewardGetAnalysis),
        _ => None,
    }
}

pub fn mcp_tool_for(id: CapabilityToolId) -> McpTool {
    match id {
        CapabilityToolId::IdeasCreate => tool_spec_by_name(ideas::tool_specs(), "ideas.create"),
        CapabilityToolId::IdeasList => tool_spec_by_name(ideas::tool_specs(), "ideas.list"),
        CapabilityToolId::RunsStart => tool_spec_by_name(runs::tool_specs(), "runs.start"),
        CapabilityToolId::RunsList => tool_spec_by_name(runs::tool_specs(), "runs.list"),
        CapabilityToolId::RunsGet => tool_spec_by_name(runs::tool_specs(), "runs.get"),
        CapabilityToolId::RunsCancel => tool_spec_by_name(runs::tool_specs(), "runs.cancel"),
        CapabilityToolId::ApprovalsList => {
            tool_spec_by_name(approvals::tool_specs(), "approvals.list")
        }
        CapabilityToolId::ApprovalsResolve => {
            tool_spec_by_name(approvals::tool_specs(), "approvals.resolve")
        }
        CapabilityToolId::StagesRetry => tool_spec_by_name(stages::tool_specs(), "stages.retry"),
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
            super::mcp_tool_for(CapabilityToolId::StewardGetAnalysis).name,
            "steward.get_analysis"
        );
        assert_eq!(super::capability_id_for("missing.tool"), None);
    }
}
