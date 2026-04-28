use anyhow::Result;
use sqlx::SqlitePool;

use domain::commands::{
    Command, OverrideLegacyDiscoveryPolicyCmd, ResolveWorkflowConflictTransitionCmd, RetryStageCmd,
};
use domain::discovery::LegacyBroadDiscoveryPolicy;
use domain::ids::{RunId, StageExecutionId};
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "stages.retry".to_string(),
            description: "Retry a failed or blocked stage".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "stage_id"],
                "properties": {
                    "run_id": { "type": "string" },
                    "stage_id": { "type": "string" },
                    "agent_execution_id": {
                        "type": "string",
                        "description": "Optional. Retry only this InvokeAgent execution instead of the full stage fanout."
                    },
                    "consume_quota_budget_now": { "type": "boolean", "description": "Allow an early retry before a persisted quota retry_after has elapsed." },
                    "legacy_discovery_override_policy": { "type": "string", "enum": ["workflow_opt_in"], "description": "Optional audited one-shot legacy discovery policy for this retry attempt." },
                    "legacy_discovery_override_reason": { "type": "string", "description": "Required reason when legacy_discovery_override_policy is set." },
                    "operator_instruction": { "type": "string", "description": "Optional one-shot operator instruction for the retry-created invocation scope (1-2000 chars, operator-only)." }
                }
            }),
        },
        McpTool {
            name: "legacy_discovery_override_create".to_string(),
            description:
                "Attach an audited one-shot legacy discovery override to a pending retry attempt"
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": [
                    "run_id",
                    "stage_id",
                    "target_stage_execution_id",
                    "target_attempt_number",
                    "legacy_discovery_override_policy",
                    "legacy_discovery_override_reason"
                ],
                "properties": {
                    "run_id": { "type": "string" },
                    "stage_id": { "type": "string" },
                    "target_stage_execution_id": { "type": "string" },
                    "target_attempt_number": { "type": "integer", "minimum": 1 },
                    "legacy_discovery_override_policy": { "type": "string", "enum": ["workflow_opt_in"] },
                    "legacy_discovery_override_reason": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "workflow_conflicts.resolve".to_string(),
            description:
                "Resolve a blocking workflow conflict by selecting an existing candidate transition"
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": [
                    "run_id",
                    "conflict_id",
                    "selected_transition_id",
                    "resolution_reason"
                ],
                "properties": {
                    "run_id": { "type": "string" },
                    "conflict_id": { "type": "string" },
                    "selected_transition_id": { "type": "string" },
                    "resolution_reason": { "type": "string" }
                }
            }),
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    _pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "stages.retry" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let stage_id = params["stage_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'stage_id'"))?
                .to_string();
            let consume_quota_budget_now = params["consume_quota_budget_now"]
                .as_bool()
                .unwrap_or(false);
            let agent_execution_id = params["agent_execution_id"]
                .as_str()
                .map(|value| value.parse())
                .transpose()?;
            let legacy_discovery_override_policy = params["legacy_discovery_override_policy"]
                .as_str()
                .map(parse_legacy_broad_discovery_policy)
                .transpose()?;
            let legacy_discovery_override_reason = params["legacy_discovery_override_reason"]
                .as_str()
                .map(String::from);
            let operator_instruction = params["operator_instruction"].as_str().map(String::from);

            let caller = mcp_caller(&principal.id, &principal.class, "stages.retry");
            let cmd = Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id,
                consume_quota_budget_now,
                agent_execution_id,
                legacy_discovery_override_policy,
                legacy_discovery_override_reason,
                operator_instruction,
            });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let (legacy_discovery_override_id, retry_instruction_binding_id) =
                match &commanded.result {
                    engine::command_handler::CommandResult::StageRetryScheduled {
                        legacy_discovery_override_id,
                        retry_instruction_binding_id,
                        ..
                    } => (
                        legacy_discovery_override_id.clone(),
                        retry_instruction_binding_id.clone(),
                    ),
                    _ => (None, None),
                };
            Ok(serde_json::json!({
                "scheduled": true,
                "journal_id": commanded.journal_id,
                "legacy_discovery_override_id": legacy_discovery_override_id,
                "retry_instruction_binding_id": retry_instruction_binding_id,
            }))
        }

        "legacy_discovery_override_create" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let stage_id = params["stage_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'stage_id'"))?
                .to_string();
            let target_stage_execution_id: StageExecutionId = params["target_stage_execution_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'target_stage_execution_id'"))?
                .parse()?;
            let target_attempt_number =
                parse_target_attempt_number(params.get("target_attempt_number"))?;
            let legacy_discovery_override_policy = params["legacy_discovery_override_policy"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'legacy_discovery_override_policy'"))
                .and_then(parse_legacy_broad_discovery_policy)?;
            let legacy_discovery_override_reason = params["legacy_discovery_override_reason"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'legacy_discovery_override_reason'"))?
                .to_string();

            let caller = mcp_caller(
                &principal.id,
                &principal.class,
                "legacy_discovery_override_create",
            );
            let cmd = Command::OverrideLegacyDiscoveryPolicy(OverrideLegacyDiscoveryPolicyCmd {
                run_id,
                stage_id,
                target_stage_execution_id,
                target_attempt_number,
                legacy_discovery_override_policy,
                legacy_discovery_override_reason,
            });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let override_id = match &commanded.result {
                engine::command_handler::CommandResult::LegacyDiscoveryOverrideCreated {
                    override_id,
                } => override_id.clone(),
                _ => anyhow::bail!("Unexpected command result"),
            };
            Ok(serde_json::json!({
                "override_id": override_id,
                "journal_id": commanded.journal_id,
            }))
        }

        "workflow_conflicts.resolve" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let conflict_id = params["conflict_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'conflict_id'"))?
                .to_string();
            let selected_transition_id = params["selected_transition_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'selected_transition_id'"))?
                .to_string();
            let resolution_reason = params["resolution_reason"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'resolution_reason'"))?
                .to_string();

            let caller = mcp_caller(
                &principal.id,
                &principal.class,
                "workflow_conflicts.resolve",
            );
            let cmd =
                Command::ResolveWorkflowConflictTransition(ResolveWorkflowConflictTransitionCmd {
                    run_id,
                    conflict_id,
                    selected_transition_id,
                    resolution_reason,
                });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let (selected_transition_id, selected_next_state_id) = match &commanded.result {
                engine::command_handler::CommandResult::WorkflowConflictTransitionSelected {
                    selected_transition_id,
                    selected_next_state_id,
                    ..
                } => (
                    selected_transition_id.clone(),
                    selected_next_state_id.clone(),
                ),
                _ => anyhow::bail!("Unexpected command result"),
            };
            Ok(serde_json::json!({
                "resolved": true,
                "selected_transition_id": selected_transition_id,
                "selected_next_state_id": selected_next_state_id,
                "journal_id": commanded.journal_id,
            }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn parse_legacy_broad_discovery_policy(value: &str) -> Result<LegacyBroadDiscoveryPolicy> {
    match value {
        "workflow_opt_in" => Ok(LegacyBroadDiscoveryPolicy::WorkflowOptIn),
        "disabled" => Ok(LegacyBroadDiscoveryPolicy::Disabled),
        _ => anyhow::bail!("unknown legacy_discovery_override_policy: {value}"),
    }
}

fn parse_target_attempt_number(value: Option<&serde_json::Value>) -> Result<i64> {
    let attempt_number = value
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("Missing 'target_attempt_number'"))?;
    if attempt_number <= 0 {
        anyhow::bail!("target_attempt_number must be greater than 0");
    }
    Ok(attempt_number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_attempt_number_rejects_non_positive_values() {
        assert!(parse_target_attempt_number(Some(&serde_json::json!(0))).is_err());
        assert!(parse_target_attempt_number(Some(&serde_json::json!(-1))).is_err());
        assert_eq!(
            parse_target_attempt_number(Some(&serde_json::json!(1))).unwrap(),
            1
        );
    }
}
