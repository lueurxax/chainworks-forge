pub mod approvals;
pub mod ideas;
pub mod reports;
pub mod runs;
pub mod stages;
pub mod steward;

use crate::protocol::McpTool;
use domain::CapabilityToolId;

pub fn all_capability_tool_ids() -> [CapabilityToolId; 13] {
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
        "steward.run_analysis" => Some(CapabilityToolId::StewardRunAnalysis),
        "steward.list_analyses" => Some(CapabilityToolId::StewardListAnalyses),
        "steward.get_analysis" => Some(CapabilityToolId::StewardGetAnalysis),
        _ => None,
    }
}

pub fn mcp_tool_for(id: CapabilityToolId) -> McpTool {
    all_tool_specs()
        .into_iter()
        .find(|tool| capability_id_for(&tool.name) == Some(id))
        .expect("every CapabilityToolId must have an MCP tool spec")
}

pub fn all_tool_specs() -> Vec<McpTool> {
    let mut specs = Vec::new();
    specs.extend(ideas::tool_specs());
    specs.extend(runs::tool_specs());
    specs.extend(approvals::tool_specs());
    specs.extend(stages::tool_specs());
    specs.extend(reports::tool_specs());
    specs.extend(steward::tool_specs());
    specs
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
