use anyhow::Result;
use sqlx::SqlitePool;

use domain::commands::{Command, OverrideArtifactContractCmd};
use domain::ids::RunId;
use engine::command_handler::{CommandHandler, CommandResult};

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "artifacts.override_contract".to_string(),
        description:
            "Create a typed, journaled operator override for a canonical artifact contract"
                .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["run_id", "contract_id", "override_type", "from_status", "to_status", "reason", "expires_at_stage", "idempotency_key"],
            "properties": {
                "run_id": { "type": "string" },
                "contract_id": { "type": "string" },
                "override_type": { "type": "string" },
                "from_status": { "type": "string" },
                "to_status": { "type": "string" },
                "reason": { "type": "string" },
                "source_artifacts": { "type": "array", "items": { "type": "string" } },
                "expires_at_stage": { "type": "string" },
                "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe retry." }
            }
        }),
        output_schema: None,
    }]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    _pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "artifacts.override_contract" => {
            if principal.class != auth::PrincipalClass::Operator {
                anyhow::bail!("forbidden: artifacts.override_contract requires operator principal");
            }
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let source_artifacts = params["source_artifacts"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let cmd = Command::OverrideArtifactContract(OverrideArtifactContractCmd {
                run_id,
                contract_id: required_string(&params, "contract_id")?,
                override_type: required_string(&params, "override_type")?,
                from_status: required_string(&params, "from_status")?,
                to_status: required_string(&params, "to_status")?,
                reason: required_string(&params, "reason")?,
                source_artifacts,
                expires_at_stage: required_string(&params, "expires_at_stage")?,
            });
            // R12 API-002 / §9.3: `mcp_caller` attaches the ambient
            // HTTP request id so the override appears in
            // `command_journal.request_id`. The previous code used
            // `CallerContext::mcp(...)` directly, which drops the id
            // even when the inbound MCP HTTP request had one set via
            // `X-Request-ID`.
            let caller = mcp_caller(&principal, tool_name);
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let override_id = match commanded.result {
                CommandResult::ArtifactContractOverrideCreated { override_id } => override_id,
                _ => anyhow::bail!("Unexpected command result for artifacts.override_contract"),
            };
            Ok(serde_json::json!({
                "override_id": override_id,
                "journal_id": commanded.journal_id,
            }))
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn required_string(params: &serde_json::Value, key: &str) -> Result<String> {
    params[key]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Missing '{key}'"))
}
