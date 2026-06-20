use anyhow::Result;
use sqlx::SqlitePool;

use domain::commands::{
    Command, ConsumeProviderQuotaHoldCmd, ExtendWorkflowLoopBudgetCmd,
    OverrideLegacyDiscoveryPolicyCmd, ResolveWorkflowConflictTransitionCmd, RetryStageCmd,
    WorkflowLoopBudgetExtensionCmd,
};
use domain::discovery::LegacyBroadDiscoveryPolicy;
use domain::ids::{RunId, StageExecutionId};
use engine::command_handler::{validate_caller_request_id, CommandHandler};

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "stages.retry".to_string(),
            description: "Retry a failed or blocked stage. Requires a CallerRequestId \
                (lowercase UUIDv4) for command_idempotency_contract_v1 replay safety (TTL=120s)."
                .to_string(),
            input_schema: serde_json::json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "required": ["run_id", "stage_id", "request_id"],
                "additionalProperties": false,
                "properties": {
                    "run_id": { "type": "string" },
                    "stage_id": { "type": "string" },
                    "request_id": {
                        "type": "string",
                        "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
                        "description": "CallerRequestId: lowercase UUIDv4 for command_idempotency_contract_v1"
                    },
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
            output_schema: Some(serde_json::json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "additionalProperties": false,
                "required": ["retried"],
                "properties": {
                    "retried": { "type": "boolean", "description": "true when the stage retry was durably committed; false on denial" },
                    "run_id": { "type": "string" },
                    "stage_id": { "type": "string" },
                    "journal_id": { "type": "string" },
                    "request_id": { "type": "string", "description": "Replayed CallerRequestId for idempotency confirmation" },
                    "denied": { "type": "boolean", "description": "true when the command was denied before execution" },
                    "denial_code": { "type": "string", "enum": ["malformed_request_id","request_intent_mismatch","idempotency_in_flight","idempotency_replayed","idempotency_expired_reacquired","idempotency_replay_corrupt","idempotency_terminal_failure","operator_required","p083_operator_required","provider_session_not_found","run_not_found","stage_not_retryable","approval_not_actionable","side_effect_not_reconcilable","enforcement_mode_transition_denied","identity_ambiguous","internal"], "description": "Bounded denial code from P083LifecycleDenialCode" },
                    "message": { "type": "string", "description": "Human-readable denial or error message" }
                }
            })),
        },
        McpTool {
            name: "stages.consume_provider_quota_hold".to_string(),
            description:
                "Consume an active provider quota hold for an existing pending InvokeAgent"
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "stage_id", "reason", "idempotency_key"],
                "properties": {
                    "run_id": { "type": "string" },
                    "stage_id": { "type": "string" },
                    "reason": { "type": "string", "description": "Operator audit reason for overriding the active quota hold." },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe repair." }
                }
            }),
            output_schema: None,
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
                    "legacy_discovery_override_reason",
                    "idempotency_key"
                ],
                "properties": {
                    "run_id": { "type": "string" },
                    "stage_id": { "type": "string" },
                    "target_stage_execution_id": { "type": "string" },
                    "target_attempt_number": { "type": "integer", "minimum": 1 },
                    "legacy_discovery_override_policy": { "type": "string", "enum": ["workflow_opt_in"] },
                    "legacy_discovery_override_reason": { "type": "string" },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe retry." }
                }
            }),
            output_schema: None,
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
                    "resolution_reason",
                    "idempotency_key"
                ],
                "properties": {
                    "run_id": { "type": "string" },
                    "conflict_id": { "type": "string" },
                    "selected_transition_id": { "type": "string" },
                    "resolution_reason": { "type": "string" },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe retry." },
                    "operator_instruction": { "type": "string", "description": "Optional one-shot operator instruction for the retry-created invocation scope (1-2000 chars, operator-only)." },
                    "loop_budget_extension": {
                        "type": "object",
                        "description": "Optional atomic run-local frozen workflow loop budget extension applied before selecting the transition.",
                        "required": ["counter", "additional_cycles", "reason"],
                        "properties": {
                            "counter": { "type": "string" },
                            "additional_cycles": { "type": "integer", "minimum": 1, "maximum": 100 },
                            "reason": { "type": "string" },
                            "target_conflict_id": { "type": "string" }
                        }
                    }
                }
            }),
            output_schema: None,
        },
        McpTool {
            name: "workflow_loop_budget.extend".to_string(),
            description: "Extend a run-local frozen workflow loop budget and re-run orchestration"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "counter", "additional_cycles", "reason", "idempotency_key"],
                "properties": {
                    "run_id": { "type": "string" },
                    "counter": { "type": "string" },
                    "additional_cycles": { "type": "integer", "minimum": 1, "maximum": 100 },
                    "reason": { "type": "string" },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe retry." },
                    "target_conflict_id": { "type": "string" }
                }
            }),
            output_schema: None,
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
            // P083 command_idempotency_contract_v1: request_id must be a lowercase UUIDv4
            // (CallerRequestId). Required per schema; validate format before acquiring lease.
            let request_id = params["request_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'request_id'"))?
                .to_string();
            validate_caller_request_id(&request_id)?;

            // SEC-P083-MED-003: stamp the validated CallerRequestId into CallerContext so
            // command_journal.request_id carries the lifecycle request id, not the HTTP request id.
            let caller = mcp_caller(&principal, "stages.retry")
                .with_request_id(request_id.clone());
            let cmd = Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id,
                consume_quota_budget_now,
                agent_execution_id,
                legacy_discovery_override_policy,
                legacy_discovery_override_reason,
                operator_instruction,
                request_id: Some(request_id.clone()),
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
                "retried": true,
                "request_id": request_id,
                "journal_id": commanded.journal_id,
                "legacy_discovery_override_id": legacy_discovery_override_id,
                "retry_instruction_binding_id": retry_instruction_binding_id,
            }))
        }

        "stages.consume_provider_quota_hold" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let stage_id = params["stage_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'stage_id'"))?
                .to_string();
            let reason = params["reason"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'reason'"))?
                .to_string();

            let caller = mcp_caller(&principal, "stages.consume_provider_quota_hold");
            let cmd = Command::ConsumeProviderQuotaHold(ConsumeProviderQuotaHoldCmd {
                run_id,
                stage_id,
                reason,
            });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let (result_run_id, result_stage_id, consumed_ledger_count, released_work_item_count) =
                match &commanded.result {
                    engine::command_handler::CommandResult::ProviderQuotaHoldConsumed {
                        run_id,
                        stage_id,
                        consumed_ledger_count,
                        released_work_item_count,
                    } => (
                        run_id.to_string(),
                        stage_id.clone(),
                        *consumed_ledger_count,
                        *released_work_item_count,
                    ),
                    _ => anyhow::bail!("Unexpected command result"),
                };
            Ok(serde_json::json!({
                "consumed": true,
                "run_id": result_run_id,
                "stage_id": result_stage_id,
                "consumed_ledger_count": consumed_ledger_count,
                "released_work_item_count": released_work_item_count,
                "journal_id": commanded.journal_id,
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

            let caller = mcp_caller(&principal, "legacy_discovery_override_create");
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
            let operator_instruction = params["operator_instruction"].as_str().map(String::from);
            let loop_budget_extension =
                parse_loop_budget_extension(params.get("loop_budget_extension"))?;

            let caller = mcp_caller(&principal, "workflow_conflicts.resolve");
            let cmd =
                Command::ResolveWorkflowConflictTransition(ResolveWorkflowConflictTransitionCmd {
                    run_id,
                    conflict_id,
                    selected_transition_id,
                    resolution_reason,
                    operator_instruction,
                    loop_budget_extension,
                });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let (selected_transition_id, selected_next_state_id, retry_instruction_binding_id) =
                match &commanded.result {
                engine::command_handler::CommandResult::WorkflowConflictTransitionSelected {
                    selected_transition_id,
                    selected_next_state_id,
                    retry_instruction_binding_id,
                    ..
                } => (
                    selected_transition_id.clone(),
                    selected_next_state_id.clone(),
                    retry_instruction_binding_id.clone(),
                ),
                _ => anyhow::bail!("Unexpected command result"),
            };
            Ok(serde_json::json!({
                "resolved": true,
                "selected_transition_id": selected_transition_id,
                "selected_next_state_id": selected_next_state_id,
                "retry_instruction_binding_id": retry_instruction_binding_id,
                "journal_id": commanded.journal_id,
            }))
        }

        "workflow_loop_budget.extend" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let extension = parse_loop_budget_extension(Some(&params))?
                .ok_or_else(|| anyhow::anyhow!("Missing loop budget extension payload"))?;
            let caller = mcp_caller(principal, "workflow_loop_budget.extend");
            let commanded = cmd_handler
                .handle(
                    Command::ExtendWorkflowLoopBudget(ExtendWorkflowLoopBudgetCmd {
                        run_id,
                        extension,
                    }),
                    caller,
                )
                .await?;
            let (counter, previous_max, new_max) = match &commanded.result {
                engine::command_handler::CommandResult::WorkflowLoopBudgetExtended {
                    counter,
                    previous_max,
                    new_max,
                    ..
                } => (counter.clone(), *previous_max, *new_max),
                _ => anyhow::bail!("Unexpected command result"),
            };
            Ok(serde_json::json!({
                "extended": true,
                "counter": counter,
                "previous_max": previous_max,
                "new_max": new_max,
                "journal_id": commanded.journal_id,
            }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn parse_loop_budget_extension(
    value: Option<&serde_json::Value>,
) -> Result<Option<WorkflowLoopBudgetExtensionCmd>> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let counter = value["counter"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'counter'"))?
        .to_string();
    let additional_cycles = value["additional_cycles"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("Missing 'additional_cycles'"))?;
    let additional_cycles = u32::try_from(additional_cycles)
        .map_err(|_| anyhow::anyhow!("additional_cycles is too large"))?;
    let reason = value["reason"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'reason'"))?
        .to_string();
    let target_conflict_id = value["target_conflict_id"].as_str().map(String::from);
    Ok(Some(WorkflowLoopBudgetExtensionCmd {
        counter,
        additional_cycles,
        reason,
        target_conflict_id,
    }))
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
    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{ideas, runs, stages, workflow_conflicts};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{IdeaId, StageExecutionId};
    use domain::run::{Run, RunStatus};
    use domain::stage::{StageExecution, StageStatus};
    use domain::workflow_conflict::{
        candidate_transition_hash, workflow_conflict_fingerprint, CandidateTransitionEvaluation,
        CandidateTransitionResult, WorkflowConflictReason, WorkflowConflictRecord,
        WorkflowConflictStatus,
    };
    use engine::event_bus;
    use engine::work_queue::WorkQueue;

    #[test]
    fn parse_target_attempt_number_rejects_non_positive_values() {
        assert!(parse_target_attempt_number(Some(&serde_json::json!(0))).is_err());
        assert!(parse_target_attempt_number(Some(&serde_json::json!(-1))).is_err());
        assert_eq!(
            parse_target_attempt_number(Some(&serde_json::json!(1))).unwrap(),
            1
        );
    }

    #[test]
    fn workflow_conflicts_resolve_schema_accepts_operator_instruction() {
        let spec = tool_specs()
            .into_iter()
            .find(|tool| tool.name == "workflow_conflicts.resolve")
            .expect("workflow_conflicts.resolve tool spec exists");
        assert_eq!(
            spec.input_schema["properties"]["operator_instruction"]["type"],
            serde_json::json!("string")
        );
        assert!(
            !spec.input_schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "operator_instruction"),
            "operator_instruction must remain optional"
        );
        assert_eq!(
            spec.input_schema["properties"]["loop_budget_extension"]["properties"]["counter"]
                ["type"],
            serde_json::json!("string")
        );
        assert!(
            !spec.input_schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "loop_budget_extension"),
            "loop_budget_extension must remain optional"
        );
    }

    #[test]
    fn workflow_loop_budget_extend_schema_is_registered() {
        let spec = tool_specs()
            .into_iter()
            .find(|tool| tool.name == "workflow_loop_budget.extend")
            .expect("workflow_loop_budget.extend tool spec exists");
        assert_eq!(
            spec.input_schema["properties"]["additional_cycles"]["maximum"],
            serde_json::json!(100)
        );
        assert!(
            spec.input_schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "reason"),
            "standalone loop budget extension must require an audited reason"
        );
    }

    #[tokio::test]
    async fn workflow_conflicts_resolve_response_includes_retry_instruction_binding_id() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .unwrap();

        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let state_id = "state_8_implementation_continued";
        let transition_id =
            "state_8_implementation_continued__to__state_8_implementation_continued__0";

        ideas::insert(
            &pool,
            &Idea {
                id: idea_id,
                title: "Test idea".into(),
                body: "body".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: Utc::now(),
                archived_at: None,
            },
        )
        .await
        .unwrap();
        let run = Run {
            id: run_id,
            idea_id,
            status: RunStatus::Blocked,
            workflow_id: "wf-test".into(),
            workflow_title: "Test Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some(state_id.into()),
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: None,
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: None,
            catalog_snapshot_hash: None,
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        };
        runs::insert(&pool, &run).await.unwrap();

        let completed_stage_id = StageExecutionId::new();
        stages::insert(
            &pool,
            &StageExecution {
                id: completed_stage_id,
                run_id,
                stage_id: state_id.into(),
                label: "Implementation Continued".into(),
                status: StageStatus::Completed,
                iteration: 50,
                attempt_number: 1,
                settlement_kind: None,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                owner_agent: None,
                provider: None,
                model: None,
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();

        let candidates = vec![CandidateTransitionEvaluation {
            transition_id: transition_id.into(),
            from_state_id: state_id.into(),
            to_state_id: state_id.into(),
            condition_expression_id: Some("implementation_self_assessment_needs_code_fixes".into()),
            result: CandidateTransitionResult::NotMatched,
            required_artifacts: vec!["implementation_self_assessment_v2".into()],
            missing_artifacts: Vec::new(),
            missing_fields: Vec::new(),
            source_artifact_ids: vec!["implementation_self_assessment_v2".into()],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some(
                "Loop budget exhausted for implementation_progress_count: 50/50 iterations".into(),
            ),
        }];
        let reason = WorkflowConflictReason::NoDeclarativeTransitionMatched;
        let candidate_hash = candidate_transition_hash(&candidates);
        let conflict = WorkflowConflictRecord {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            conflict_fingerprint: workflow_conflict_fingerprint(
                &run_id.to_string(),
                state_id,
                &reason,
                &candidate_hash,
                &[],
            ),
            run_id: run_id.to_string(),
            stage_execution_id: Some(completed_stage_id.to_string()),
            lineage_id: None,
            current_state_id: state_id.into(),
            reason,
            operator_label: "No declarative workflow transition matched".into(),
            status: WorkflowConflictStatus::OperatorConfirmationRequired,
            candidate_transitions: candidates,
            candidate_transition_hash: candidate_hash,
            advisory_evidence_refs: Vec::new(),
            lead_agent_id: None,
            mediation_record_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            resolved_at: None,
            superseded_by_conflict_id: None,
            resolution_record_json: None,
            terminal_failure_reason: None,
            diagnostic_redaction_tier: "operator_safe".into(),
        };
        let conflict_id = conflict.conflict_id.clone();
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let handler = CommandHandler::new(
            pool.clone(),
            event_bus::new_bus(16),
            WorkQueue::new(pool.clone()),
        );
        let response = execute(
            "workflow_conflicts.resolve",
            serde_json::json!({
                "run_id": run_id.to_string(),
                "conflict_id": conflict_id,
                "selected_transition_id": transition_id,
                "resolution_reason": "operator selected one more refine",
                "operator_instruction": "Refine only the requested blockers."
            }),
            &pool,
            &handler,
            &auth::Principal::new("operator-test", auth::PrincipalClass::Operator),
        )
        .await
        .unwrap();

        assert_eq!(response["resolved"], serde_json::json!(true));
        assert!(
            response["retry_instruction_binding_id"].as_str().is_some(),
            "MCP response should surface retry instruction binding id"
        );
    }
}
