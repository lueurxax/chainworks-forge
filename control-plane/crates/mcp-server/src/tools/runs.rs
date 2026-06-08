use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

use db::repos::{
    artifact_contracts, closeout, code_writer_completion_receipts, escalation as escalation_repo,
    legacy_discovery_overrides, projections, rollout_contract_checks, runs, side_effects,
};
use domain::commands::{
    CancelRunCmd, CatalogSnapshotRetrofitScope, Command, KnowledgeCapsuleIgnoreCmd, MainSyncMode,
    MainSyncRecordRecoveryDecisionCmd, MainSyncRecoveryDecision, MainSyncRepairStateCmd,
    MainSyncRequestCmd, MainSyncRetryCmd, MainSyncSetRunOverrideCmd, MainSyncTriggerReason,
    ProposalGateSettlementAction, RetrofitCatalogSnapshotCmd, SettleProposalGateCmd, StartRunCmd,
};
use domain::ids::{IdeaId, RunId};
use domain::risk_lineage::RiskAcceptanceLineage;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "runs.start".to_string(),
            description: "Start a new run for an idea".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["idea_id", "workflow_id", "workflow_title", "workspace_root", "artifact_root", "workflow_yaml_path", "agent_catalog_yaml_path", "idempotency_key"],
                "properties": {
                    "idea_id": { "type": "string", "description": "ID of the idea" },
                    "workflow_id": { "type": "string" },
                    "workflow_title": { "type": "string" },
                    "workspace_root": { "type": "string" },
                    "artifact_root": { "type": "string" },
                    "workflow_yaml_path": { "type": "string", "description": "Path to workflow YAML file (enables state-machine execution)" },
                    "agent_catalog_yaml_path": { "type": "string", "description": "Path to agent catalog YAML file" },
                    "idempotency_key": { "type": "string", "description": "Caller-supplied idempotency key for replay safety. Stored in the command journal as request_id." },
                    "delivery_configuration_json": { "type": "string", "description": "Frozen delivery configuration JSON for repo-backed runs" },
                    "review_routing_json": { "type": "string", "description": "Review routing options JSON for P060 dynamic reviewer selection" },
                    "rollout_contract_preflight_policy_json": {
                        "type": "string",
                        "description": "P084 rollout-contract run-start policy request JSON, capped at 64 KiB by the engine. Accepts waiver and/or enforcement_mode objects; server stamps authorization, principal, and audit event."
                    },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe retry." }
                }
            }),
        },
        McpTool {
            name: "runs.get".to_string(),
            description: "Get a run by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id"],
                "properties": {
                    "run_id": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.list".to_string(),
            description: "List active runs".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpTool {
            name: "runs.cancel".to_string(),
            description: "Cancel a run".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "idempotency_key"],
                "properties": {
                    "run_id": { "type": "string" },
                    "idempotency_key": { "type": "string", "description": "Caller-supplied idempotency key for replay safety. Stored in the command journal as request_id." }
                }
            }),
        },
        McpTool {
            name: "runs.retrofit_catalog_snapshot".to_string(),
            description: "Emergency operator repair: replace a blocked run's frozen catalog snapshot from the current catalog YAML with audit/hash guardrails".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "expected_catalog_snapshot_hash", "reason", "idempotency_key"],
                "properties": {
                    "run_id": { "type": "string" },
                    "expected_catalog_snapshot_hash": {
                        "type": "string",
                        "description": "The current frozen catalog snapshot hash expected by the operator; mismatch fails closed."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["escalation_policy_only"],
                        "description": "Emergency retrofit scope. Only escalation_policy_only is currently supported."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Operator audit reason for retrofitting the frozen catalog snapshot."
                    },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 for safe repair." }
                }
            }),
        },
        McpTool {
            name: "runs.main_sync.request".to_string(),
            description: "Queue or dedupe a main-sync request for a run".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "trigger_reason", "idempotency_key"],
                "properties": {
                    "run_id": { "type": "string" },
                    "trigger_reason": {
                        "type": "string",
                        "enum": [
                            "before_initial_implementation",
                            "before_retry",
                            "before_review",
                            "operator_request",
                            "before_final_approval",
                            "startup_repair"
                        ]
                    },
                    "idempotency_key": { "type": "string" },
                    "requested_by_stage_id": { "type": "string" },
                    "requested_by_work_item_id": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.main_sync.retry".to_string(),
            description: "Retry a previously failed or blocked main-sync request".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "idempotency_key"],
                "properties": {
                    "run_id": { "type": "string" },
                    "idempotency_key": { "type": "string" },
                    "failed_attempt_id": { "type": "string" },
                    "reason": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.main_sync.set_override".to_string(),
            description: "Set the per-run main-sync mode override".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "mode", "reason"],
                "properties": {
                    "run_id": { "type": "string" },
                    "mode": {
                        "type": "string",
                        "enum": ["off", "dry_run", "manual_only", "automatic"]
                    },
                    "reason": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.main_sync.repair_state".to_string(),
            description: "Reconcile a run stuck in main-sync recovery".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id"],
                "properties": {
                    "run_id": { "type": "string" },
                    "attempt_id": { "type": "string" },
                    "recovery_note": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.main_sync.record_recovery_decision".to_string(),
            description: "Record an operator recovery decision for main-sync".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "decision", "summary"],
                "properties": {
                    "run_id": { "type": "string" },
                    "decision": {
                        "type": "string",
                        "enum": ["retry_sync", "mark_recovered", "escalate"]
                    },
                    "summary": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.knowledge_capsule.ignore".to_string(),
            description: "Ignore a matched knowledge capsule for the current run".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "capsule_id", "reason"],
                "properties": {
                    "run_id": { "type": "string" },
                    "capsule_id": { "type": "string" },
                    "reason": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.settle_proposal_gate".to_string(),
            description: "P077: Execute, import, or waive a proposal gate settlement result. \
                Requires operator principal. The principal field is bound from the authenticated \
                caller context at the engine boundary."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": [
                    "run_id", "proposal_id", "stage_id", "capability", "journal_id",
                    "authority", "reason", "source_artifacts", "workflow_digest",
                    "worktree_head", "dirty_or_changed_file_digest",
                    "source_generation_ids", "current_fingerprint"
                ],
                "properties": {
                    "run_id": { "type": "string" },
                    "proposal_id": { "type": "string", "maxLength": 64 },
                    "stage_id": { "type": "string", "maxLength": 128 },
                    "action": {
                        "type": "string",
                        "enum": ["execute", "import_receipt", "waive"],
                        "description": "Defaults to import_receipt when receipt_json is supplied; required otherwise. Use import_receipt with a governed gate receipt, waive to waive with lineage, or execute to run the bounded managed ProposalGateExecutor."
                    },
                    "capability": { "type": "string", "maxLength": 1024 },
                    "journal_id": { "type": "string", "maxLength": 1024 },
                    "authority": { "type": "string", "maxLength": 1024 },
                    "reason": { "type": "string", "maxLength": 4096 },
                    "source_artifacts": {
                        "type": "array",
                        "items": { "type": "string", "maxLength": 1024 },
                        "maxItems": 64
                    },
                    "workflow_digest": { "type": "string", "maxLength": 1024 },
                    "worktree_head": { "type": "string", "maxLength": 1024 },
                    "dirty_or_changed_file_digest": { "type": "string", "maxLength": 1024 },
                    "source_generation_ids": {
                        "type": "array",
                        "items": { "type": "string", "maxLength": 1024 },
                        "maxItems": 64
                    },
                    "current_fingerprint": { "type": "string", "maxLength": 1024 },
                    "timeout_ms": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 600000,
                        "description": "Optional bounded timeout for action=execute. The engine applies a default when omitted."
                    },
                    "accepted_risks": {
                        "type": "array",
                        "description": "Optional typed RiskAcceptanceLineage rows for governed risk settlement.",
                        "items": { "type": "object" },
                        "maxItems": 64
                    },
                    "receipt_json": { "type": "string", "description": "Raw JSON receipt from the gate executor (max 256KiB)" }
                }
            }),
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "runs.start" => {
            let idea_id: IdeaId = params["idea_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idea_id'"))?
                .parse()?;
            let workflow_id = params["workflow_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_id'"))?
                .to_string();
            let workflow_title = params["workflow_title"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_title'"))?
                .to_string();
            let workspace_root = params["workspace_root"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workspace_root'"))?
                .to_string();
            let artifact_root = params["artifact_root"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'artifact_root'"))?
                .to_string();

            let workflow_yaml_path = params["workflow_yaml_path"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_yaml_path'"))?
                .to_string();
            let agent_catalog_yaml_path = params["agent_catalog_yaml_path"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'agent_catalog_yaml_path'"))?
                .to_string();
            // HIGH-002: idempotency_key is required so callers can perform safe replay.
            // Stored in the command journal as request_id for cross-surface correlation.
            let idempotency_key = params["idempotency_key"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idempotency_key'"))?
                .to_string();
            let delivery_configuration_json = params["delivery_configuration_json"]
                .as_str()
                .map(String::from);
            let review_routing_json = params["review_routing_json"].as_str().map(String::from);
            let rollout_contract_preflight_policy_json = params
                ["rollout_contract_preflight_policy_json"]
                .as_str()
                .map(String::from);

            // SEC-001: validate and, when possible, canonicalize caller-supplied paths before
            // any filesystem read. Existing workspaces are root-confined so symlinks cannot
            // smuggle workflow/catalog/artifact paths outside the selected workspace.
            let (workspace_root, artifact_root, workflow_yaml_path, agent_catalog_yaml_path) =
                canonicalize_run_start_paths(
                    &workspace_root,
                    &artifact_root,
                    &workflow_yaml_path,
                    &agent_catalog_yaml_path,
                )?;

            // Propagate idempotency_key as request_id so the command journal records it
            // for cross-surface correlation and replay detection.
            let caller = mcp_caller(&principal, "runs.start").with_request_id(idempotency_key);
            let cmd = Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id,
                workflow_title,
                workspace_root,
                artifact_root,
                delivery_configuration_json,
                workflow_yaml_path,
                agent_catalog_yaml_path,
                review_routing_json,
                rollout_contract_preflight_policy_json,
                closeout_readiness_mode: None,
            });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let run_id = match &commanded.result {
                engine::command_handler::CommandResult::RunStarted { run_id } => *run_id,
                engine::command_handler::CommandResult::StartRunBlockedByDeliveryPreflight(
                    blocked,
                ) => {
                    return Ok(serde_json::json!({
                        "blocked": true,
                        "reason": "delivery_preflight_failed",
                        "delivery_preflight": blocked.delivery_preflight,
                        "journal_id": commanded.journal_id,
                    }));
                }
                _ => return Err(anyhow::anyhow!("Unexpected result")),
            };
            let run = runs::find_by_id(pool, run_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Run not found"))?;
            let mut value = serde_json::to_value(&run)?;
            if let Some(obj) = value.as_object_mut() {
                obj.insert(
                    "journal_id".to_string(),
                    serde_json::Value::String(commanded.journal_id),
                );
            }
            attach_implementation_self_assessment_summary(pool, value, true).await
        }

        "runs.get" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let run = runs::find_by_id(pool, run_id).await?;
            match run {
                Some(run) => {
                    let is_operator = matches!(principal.class, auth::PrincipalClass::Operator);
                    let mut value = serde_json::to_value(&run)?;
                    // SEC HIGH-001: strip operator-only snapshot fields before any caller-visible return.
                    redact_run_snapshot_fields(&mut value, is_operator);
                    // HIGH-002: projection, overrides, and retry readbacks are Operator-only.
                    if is_operator {
                        if let Some(obj) = value.as_object_mut() {
                            if let Some(projection) =
                                db::repos::artifact_contracts::find_run_state_projection(
                                    pool, run_id,
                                )
                                .await?
                            {
                                obj.insert(
                                    "active_artifact_index".into(),
                                    projection.active_index_json,
                                );
                                obj.insert(
                                    "run_state_projection".into(),
                                    projection.run_state_json,
                                );
                                obj.insert(
                                    "operator_overrides".into(),
                                    serde_json::to_value(
                                        db::repos::artifact_contracts::list_overrides(pool, run_id)
                                            .await?,
                                    )?,
                                );
                            }
                            obj.insert(
                                "legacy_discovery_overrides".into(),
                                serde_json::to_value(
                                    legacy_discovery_overrides::list_by_run(pool, run_id).await?,
                                )?,
                            );
                            obj.insert(
                                "retry_authority".into(),
                                crate::tools::reports::retry_authority_current_json(pool, run_id)
                                    .await?,
                            );
                            obj.insert(
                                "retry_authority_history".into(),
                                crate::tools::reports::retry_authority_history_json(pool, run_id)
                                    .await?,
                            );
                            obj.insert(
                                "p091_orphan_repair_readback".into(),
                                crate::tools::reports::p091_orphan_repair_readback_json(
                                    pool, run_id,
                                )
                                .await?,
                            );
                        }
                    }
                    let value =
                        attach_implementation_self_assessment_summary(pool, value, is_operator)
                            .await?;
                    // P077 BLK-004: attach closeout_readiness_summary parity on runs.get.
                    let value = attach_closeout_readiness_summary(pool, value).await?;
                    // P058 Phase 1: attach escalation_readback parity on runs.get.
                    // Full chain detail only for Operator; summary-only for Agent/Observer.
                    attach_escalation_readback(pool, value, principal).await
                }
                None => Ok(serde_json::Value::Null),
            }
        }

        "runs.list" => {
            let started = std::time::Instant::now();
            let is_operator = principal.class == auth::PrincipalClass::Operator;
            let items = projections::list_active_projection(pool).await?;
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let mut value = serde_json::to_value(item)?;
                redact_run_projection_paths(&mut value, is_operator);
                let value =
                    attach_implementation_self_assessment_summary(pool, value, is_operator).await?;
                values.push(value);
            }
            db::metrics::record_hot_read_latency("runs.list", started.elapsed());
            Ok(serde_json::Value::Array(values))
        }

        "runs.cancel" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            // HIGH-002: idempotency_key is required so callers can perform safe replay.
            let idempotency_key = params["idempotency_key"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idempotency_key'"))?
                .to_string();
            let caller = mcp_caller(&principal, "runs.cancel").with_request_id(idempotency_key);
            let cmd = Command::CancelRun(CancelRunCmd { run_id });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            Ok(serde_json::json!({
                "cancelled": true,
                "journal_id": commanded.journal_id,
            }))
        }

        "runs.retrofit_catalog_snapshot" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let expected_catalog_snapshot_hash = params["expected_catalog_snapshot_hash"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'expected_catalog_snapshot_hash'"))?
                .to_string();
            let reason = params["reason"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'reason'"))?
                .to_string();
            let scope = match params["scope"].as_str().unwrap_or("escalation_policy_only") {
                "escalation_policy_only" => CatalogSnapshotRetrofitScope::EscalationPolicyOnly,
                other => anyhow::bail!("unsupported catalog snapshot retrofit scope: {other}"),
            };
            let idempotency_key = params["idempotency_key"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idempotency_key'"))?
                .to_string();
            let caller = mcp_caller(&principal, "runs.retrofit_catalog_snapshot")
                .with_request_id(idempotency_key);
            let commanded = cmd_handler
                .handle(
                    Command::RetrofitCatalogSnapshot(RetrofitCatalogSnapshotCmd {
                        run_id,
                        expected_catalog_snapshot_hash,
                        reason,
                        scope,
                    }),
                    caller,
                )
                .await?;
            let (previous_catalog_snapshot_hash, new_catalog_snapshot_hash, applied_policy_ids) =
                match &commanded.result {
                    engine::command_handler::CommandResult::CatalogSnapshotRetrofitted {
                        previous_catalog_snapshot_hash,
                        new_catalog_snapshot_hash,
                        applied_policy_ids,
                        ..
                    } => (
                        previous_catalog_snapshot_hash.clone(),
                        new_catalog_snapshot_hash.clone(),
                        applied_policy_ids.clone(),
                    ),
                    _ => anyhow::bail!("Unexpected command result"),
                };
            Ok(serde_json::json!({
                "retrofitted": true,
                "run_id": run_id.to_string(),
                "previous_catalog_snapshot_hash": previous_catalog_snapshot_hash,
                "new_catalog_snapshot_hash": new_catalog_snapshot_hash,
                "applied_policy_ids": applied_policy_ids,
                "journal_id": commanded.journal_id,
            }))
        }

        "runs.main_sync.request" => {
            let run_id = parse_run_id(&params)?;
            let trigger_reason = parse_main_sync_trigger_reason(
                params["trigger_reason"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'trigger_reason'"))?,
            )?;
            let idempotency_key = params["idempotency_key"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idempotency_key'"))?
                .to_string();
            let caller = mcp_caller(&principal, "runs.main_sync.request");
            cmd_handler
                .handle(
                    Command::MainSyncRequest(MainSyncRequestCmd {
                        run_id,
                        trigger_reason,
                        idempotency_key,
                        requested_by_stage_id: params["requested_by_stage_id"]
                            .as_str()
                            .map(String::from),
                        requested_by_work_item_id: params["requested_by_work_item_id"]
                            .as_str()
                            .map(String::from),
                    }),
                    caller,
                )
                .await?;
            unreachable!("MainSyncRequest is contract-only and should not return success yet");
        }

        "runs.main_sync.retry" => {
            let run_id = parse_run_id(&params)?;
            let idempotency_key = params["idempotency_key"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idempotency_key'"))?
                .to_string();
            let caller = mcp_caller(&principal, "runs.main_sync.retry");
            cmd_handler
                .handle(
                    Command::MainSyncRetry(MainSyncRetryCmd {
                        run_id,
                        idempotency_key,
                        failed_attempt_id: params["failed_attempt_id"].as_str().map(String::from),
                        reason: params["reason"].as_str().map(String::from),
                    }),
                    caller,
                )
                .await?;
            unreachable!("MainSyncRetry is contract-only and should not return success yet");
        }

        "runs.main_sync.set_override" => {
            let run_id = parse_run_id(&params)?;
            let mode = parse_main_sync_mode(
                params["mode"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'mode'"))?,
            )?;
            let reason = params["reason"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'reason'"))?
                .to_string();
            let caller = mcp_caller(&principal, "runs.main_sync.set_override");
            cmd_handler
                .handle(
                    Command::MainSyncSetRunOverride(MainSyncSetRunOverrideCmd {
                        run_id,
                        mode,
                        reason,
                    }),
                    caller,
                )
                .await?;
            unreachable!(
                "MainSyncSetRunOverride is contract-only and should not return success yet"
            );
        }

        "runs.main_sync.repair_state" => {
            let run_id = parse_run_id(&params)?;
            let caller = mcp_caller(&principal, "runs.main_sync.repair_state");
            cmd_handler
                .handle(
                    Command::MainSyncRepairState(MainSyncRepairStateCmd {
                        run_id,
                        attempt_id: params["attempt_id"].as_str().map(String::from),
                        recovery_note: params["recovery_note"].as_str().map(String::from),
                    }),
                    caller,
                )
                .await?;
            unreachable!("MainSyncRepairState is contract-only and should not return success yet");
        }

        "runs.main_sync.record_recovery_decision" => {
            let run_id = parse_run_id(&params)?;
            let decision = parse_main_sync_recovery_decision(
                params["decision"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'decision'"))?,
            )?;
            let summary = params["summary"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'summary'"))?
                .to_string();
            let caller = mcp_caller(&principal, "runs.main_sync.record_recovery_decision");
            cmd_handler
                .handle(
                    Command::MainSyncRecordRecoveryDecision(MainSyncRecordRecoveryDecisionCmd {
                        run_id,
                        decision,
                        summary,
                    }),
                    caller,
                )
                .await?;
            unreachable!(
                "MainSyncRecordRecoveryDecision is contract-only and should not return success yet"
            );
        }

        "runs.knowledge_capsule.ignore" => {
            let run_id = parse_run_id(&params)?;
            let capsule_id = params["capsule_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'capsule_id'"))?
                .to_string();
            let reason = params["reason"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'reason'"))?
                .to_string();
            let caller = mcp_caller(&principal, "runs.knowledge_capsule.ignore");
            cmd_handler
                .handle(
                    Command::KnowledgeCapsuleIgnore(KnowledgeCapsuleIgnoreCmd {
                        run_id,
                        capsule_id,
                        reason,
                    }),
                    caller,
                )
                .await?;
            unreachable!(
                "KnowledgeCapsuleIgnore is contract-only and should not return success yet"
            );
        }

        "runs.settle_proposal_gate" => {
            let run_id = parse_run_id(&params)?;
            let proposal_id = params["proposal_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'proposal_id'"))?
                .to_string();
            let stage_id = params["stage_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'stage_id'"))?
                .to_string();
            let capability = params["capability"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'capability'"))?
                .to_string();
            let journal_id = params["journal_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'journal_id'"))?
                .to_string();
            let authority = params["authority"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'authority'"))?
                .to_string();
            // sec-002: cap reason at 4 KiB
            let reason = params["reason"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'reason'"))?
                .to_string();
            if reason.len() > 4096 {
                anyhow::bail!("'reason' exceeds maximum length of 4096 bytes");
            }
            // sec-002: cap source_artifacts at 64 entries, each string ≤ 1 KiB
            let source_artifacts: Vec<String> = params["source_artifacts"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Missing 'source_artifacts'"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or_else(|| anyhow::anyhow!("source_artifacts entry is not a string"))
                        .map(str::to_string)
                })
                .collect::<Result<Vec<_>>>()?;
            if source_artifacts.len() > 64 {
                anyhow::bail!("'source_artifacts' exceeds maximum of 64 entries");
            }
            for s in &source_artifacts {
                if s.len() > 1024 {
                    anyhow::bail!("source_artifacts entry exceeds maximum length of 1024 bytes");
                }
            }
            let workflow_digest = params["workflow_digest"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_digest'"))?
                .to_string();
            let worktree_head = params["worktree_head"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'worktree_head'"))?
                .to_string();
            let dirty_or_changed_file_digest = params["dirty_or_changed_file_digest"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'dirty_or_changed_file_digest'"))?
                .to_string();
            // sec-002: cap source_generation_ids at 64 entries, each string ≤ 1 KiB
            let source_generation_ids: Vec<String> = params["source_generation_ids"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("Missing 'source_generation_ids'"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or_else(|| {
                            anyhow::anyhow!("source_generation_ids entry is not a string")
                        })
                        .map(str::to_string)
                })
                .collect::<Result<Vec<_>>>()?;
            if source_generation_ids.len() > 64 {
                anyhow::bail!("'source_generation_ids' exceeds maximum of 64 entries");
            }
            for s in &source_generation_ids {
                if s.len() > 1024 {
                    anyhow::bail!(
                        "source_generation_ids entry exceeds maximum length of 1024 bytes"
                    );
                }
            }
            let current_fingerprint = params["current_fingerprint"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'current_fingerprint'"))?
                .to_string();
            let timeout_ms = match params.get("timeout_ms") {
                None | Some(serde_json::Value::Null) => None,
                Some(value) => {
                    let timeout = value
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("'timeout_ms' must be an integer"))?;
                    if !(1..=600_000).contains(&timeout) {
                        anyhow::bail!("'timeout_ms' must be between 1 and 600000");
                    }
                    Some(timeout)
                }
            };
            // sec-002: cap receipt_json at 256 KiB
            let receipt_json = params["receipt_json"].as_str().map(str::to_string);
            if let Some(ref r) = receipt_json {
                if r.len() > 262_144 {
                    anyhow::bail!("'receipt_json' exceeds maximum length of 256 KiB");
                }
            }
            let accepted_risks = match params.get("accepted_risks") {
                None | Some(serde_json::Value::Null) => Vec::new(),
                Some(value) => {
                    let raw = serde_json::to_string(value)?;
                    if raw.len() > 262_144 {
                        anyhow::bail!("'accepted_risks' exceeds maximum length of 256 KiB");
                    }
                    let risks: Vec<RiskAcceptanceLineage> =
                        serde_json::from_value(value.clone())
                            .map_err(|error| anyhow::anyhow!("invalid accepted_risks: {error}"))?;
                    if risks.len() > 64 {
                        anyhow::bail!("'accepted_risks' exceeds maximum of 64 entries");
                    }
                    risks
                }
            };
            let has_receipt = receipt_json
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
            let action_str = params["action"].as_str();
            // When receipt is present, default to import_receipt.
            // When receipt is absent, action must be specified explicitly.
            let resolved_action = match (action_str, has_receipt) {
                (Some(a), _) => a,
                (None, true) => "import_receipt",
                (None, false) => anyhow::bail!(
                    "action is required when receipt_json is absent; \
                     use action=waive with lineage or provide a governed gate receipt and action=import_receipt"
                ),
            };
            let action = match resolved_action {
                "record_settlement" => anyhow::bail!(
                    "record_settlement is no longer supported; \
                     use import_receipt with a governed gate receipt from ./scripts/test-gate.sh proposal-077"
                ),
                "execute" => ProposalGateSettlementAction::Execute,
                "import_receipt" => ProposalGateSettlementAction::ImportReceipt,
                "waive" => ProposalGateSettlementAction::Waive,
                other => anyhow::bail!("invalid proposal gate settlement action '{other}'"),
            };

            let caller = mcp_caller(&principal, "runs.settle_proposal_gate");
            let commanded = cmd_handler
                .handle(
                    Command::SettleProposalGate(SettleProposalGateCmd {
                        run_id,
                        proposal_id,
                        stage_id,
                        action,
                        // Overridden at engine boundary from CallerContext (BLK-008)
                        principal: String::new(),
                        capability,
                        journal_id,
                        authority,
                        reason,
                        source_artifacts,
                        workflow_digest,
                        worktree_head,
                        dirty_or_changed_file_digest,
                        source_generation_ids,
                        current_fingerprint,
                        timeout_ms,
                        receipt_json,
                        accepted_risks,
                    }),
                    caller,
                )
                .await?;
            match commanded.result {
                engine::command_handler::CommandResult::ProposalGateSettled {
                    run_id,
                    gate_id,
                    journal_id: settled_journal_id,
                    gate_generation_id,
                    readiness_generation_id,
                } => Ok(serde_json::json!({
                    "settled": true,
                    "run_id": run_id.to_string(),
                    "gate_id": gate_id,
                    "journal_id": settled_journal_id,
                    "gate_generation_id": gate_generation_id,
                    "readiness_generation_id": readiness_generation_id,
                })),
                _ => Err(anyhow::anyhow!("Unexpected result from SettleProposalGate")),
            }
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn parse_run_id(params: &serde_json::Value) -> Result<RunId> {
    params["run_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
        .parse()
        .map_err(Into::into)
}

fn parse_main_sync_trigger_reason(value: &str) -> Result<MainSyncTriggerReason> {
    match value {
        "before_initial_implementation" => Ok(MainSyncTriggerReason::BeforeInitialImplementation),
        "before_retry" => Ok(MainSyncTriggerReason::BeforeRetry),
        "before_review" => Ok(MainSyncTriggerReason::BeforeReview),
        "operator_request" => Ok(MainSyncTriggerReason::OperatorRequest),
        "before_final_approval" => Ok(MainSyncTriggerReason::BeforeFinalApproval),
        "startup_repair" => Ok(MainSyncTriggerReason::StartupRepair),
        _ => anyhow::bail!("unknown trigger_reason: {value}"),
    }
}

fn parse_main_sync_mode(value: &str) -> Result<MainSyncMode> {
    match value {
        "off" => Ok(MainSyncMode::Off),
        "dry_run" => Ok(MainSyncMode::DryRun),
        "manual_only" => Ok(MainSyncMode::ManualOnly),
        "automatic" => Ok(MainSyncMode::Automatic),
        _ => anyhow::bail!("unknown main_sync mode: {value}"),
    }
}

fn parse_main_sync_recovery_decision(value: &str) -> Result<MainSyncRecoveryDecision> {
    match value {
        "retry_sync" => Ok(MainSyncRecoveryDecision::RetrySync),
        "mark_recovered" => Ok(MainSyncRecoveryDecision::MarkRecovered),
        "escalate" => Ok(MainSyncRecoveryDecision::Escalate),
        _ => anyhow::bail!("unknown recovery decision: {value}"),
    }
}

async fn attach_implementation_self_assessment_summary(
    pool: &SqlitePool,
    mut value: serde_json::Value,
    is_operator: bool,
) -> Result<serde_json::Value> {
    let run_id = value
        .get("id")
        .and_then(|id| id.as_str())
        .and_then(|id| id.parse::<RunId>().ok());
    let summary = match run_id {
        Some(run_id) => {
            artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
                .await?
                .map(|stored| {
                    let mut summary = stored.summary;
                    summary.artifact_path =
                        super::reports::public_artifact_path(&summary.artifact_path);
                    serde_json::to_value(summary)
                })
                .transpose()?
        }
        None => None,
    };

    // HIGH-002: rollout_contract_readback, code_writer_completion_receipts, and
    // side_effect_readback carry operator-only evidence paths and workflow material.
    // Only fetch and attach these for Operator principals.
    let rollout_contract_readback = if is_operator {
        match run_id {
            Some(run_id) => rollout_contract_checks::find_terminal_rollout_contract_check_for_run(
                pool,
                run_id.inner(),
            )
            .await?
            .map(|check| check.operator_readback_json_for_lane("mcp")),
            None => None,
        }
    } else {
        None
    };
    // P087/P088: If the projection already includes implementationCompletion (baked by
    // refresh_run_list_readbacks via rebuild_all_for_run), use it directly and skip all
    // receipt DB queries — the full-fidelity receipts list is available on the single-run
    // detail path. Fall back to live receipt queries only for legacy runs without baked
    // projections so the list path stays to one query per run.
    let projected_implementation_completion = value
        .get("implementationCompletion")
        .filter(|v| !v.is_null())
        .cloned();
    let code_writer_completion_receipts = if is_operator {
        match run_id {
            Some(run_id) => {
                if let Some(projected) = projected_implementation_completion {
                    Some((serde_json::Value::Null, projected))
                } else {
                    let canonical_receipts =
                        code_writer_completion_receipts::list_canonical_by_run(pool, run_id)
                            .await?;
                    let implementation_completion = serde_json::to_value(
                        domain::code_writer_completion::project_implementation_completion(
                            &canonical_receipts,
                        ),
                    )?;
                    Some((
                        serde_json::to_value(&canonical_receipts)?,
                        implementation_completion,
                    ))
                }
            }
            None => None,
        }
    } else {
        None
    };
    let side_effect_readback = if is_operator {
        match run_id {
            Some(run_id) => Some(build_side_effect_readback(pool, run_id).await?),
            None => None,
        }
    } else {
        None
    };

    if let Some(object) = value.as_object_mut() {
        object.insert(
            "implementation_self_assessment_summary".to_string(),
            summary.unwrap_or(serde_json::Value::Null),
        );
        if is_operator {
            object.insert(
                "code_writer_completion_receipts".to_string(),
                code_writer_completion_receipts
                    .as_ref()
                    .map(|(receipts, _)| receipts.clone())
                    .unwrap_or(serde_json::Value::Null),
            );
            object.insert(
                "implementationCompletion".to_string(),
                code_writer_completion_receipts
                    .map(|(_, implementation_completion)| implementation_completion)
                    .unwrap_or_else(|| {
                        serde_json::to_value(
                            domain::code_writer_completion::project_implementation_completion(&[]),
                        )
                        .unwrap_or(serde_json::Value::Null)
                    }),
            );
            object.insert(
                "rollout_contract_readback".to_string(),
                rollout_contract_readback.unwrap_or(serde_json::Value::Null),
            );
            object.insert(
                "side_effect_readback".to_string(),
                side_effect_readback.unwrap_or(serde_json::Value::Null),
            );
        }
    }

    Ok(value)
}

async fn build_side_effect_readback(pool: &SqlitePool, run_id: RunId) -> Result<serde_json::Value> {
    let unresolved = side_effects::list_unresolved_for_run(pool, &run_id.to_string()).await?;
    let items: Vec<serde_json::Value> = unresolved
        .iter()
        .map(|effect| {
            let observed = effect
                .observed_evidence_summary_json
                .as_deref()
                .map(parse_json_or_string);
            let expected = effect
                .expected_evidence_json
                .as_deref()
                .map(parse_json_or_string);
            let report_path = observed
                .as_ref()
                .and_then(|value| value.get("manifest_path"))
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned);
            serde_json::json!({
                "id": effect.id.to_string(),
                "run_id": effect.run_id.to_string(),
                "stage_execution_id": effect.stage_execution_id.to_string(),
                "agent_execution_id": effect.agent_execution_id.as_ref().map(|id| id.to_string()),
                "effect_kind": effect.effect_kind.to_string(),
                "status": effect.status.to_string(),
                "target_key": effect.target_key,
                "external_write_attempted": effect.external_write_attempted,
                "expected_evidence": expected,
                "observed_evidence_summary": observed,
                "evidence_root": effect.evidence_root.clone(),
                "readback_source": "side_effects_ledger",
                "report_path": report_path,
                "blocked_reason": side_effect_blocked_reason(&effect.status),
                "operator_next_action": side_effect_operator_next_action(&effect.status),
                "recommended_mcp_tool": side_effect_operator_next_action(&effect.status),
                "retry_forbidden": true,
                "last_error_kind": effect.last_error_kind.clone(),
                "updated_at": effect.updated_at.to_rfc3339()
            })
        })
        .collect();

    Ok(serde_json::json!({
        "schema_version": "p078_side_effect_readback_v1",
        "run_id": run_id.to_string(),
        "unresolved_count": items.len(),
        "blocked": !items.is_empty(),
        "readback_source": "side_effects_ledger",
        "effects": items
    }))
}

fn parse_json_or_string(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

fn side_effect_blocked_reason(status: &domain::side_effect::SideEffectStatus) -> &'static str {
    match status {
        domain::side_effect::SideEffectStatus::Prepared => "prepared_effect_not_executed",
        domain::side_effect::SideEffectStatus::Executing => "executing_effect_not_settled",
        domain::side_effect::SideEffectStatus::ExternallyObserved => {
            "external_write_observed_pending_settlement"
        }
        domain::side_effect::SideEffectStatus::NeedsReconciliation => "effect_needs_reconciliation",
        domain::side_effect::SideEffectStatus::Conflict => "effect_conflict_requires_disposition",
        domain::side_effect::SideEffectStatus::Unrecoverable => {
            "effect_unrecoverable_requires_manual_clear"
        }
        _ => "not_blocking",
    }
}

fn side_effect_operator_next_action(
    status: &domain::side_effect::SideEffectStatus,
) -> &'static str {
    match status {
        domain::side_effect::SideEffectStatus::NeedsReconciliation
        | domain::side_effect::SideEffectStatus::ExternallyObserved => "effects.reconcile",
        domain::side_effect::SideEffectStatus::Conflict => {
            "effects.mark_unrecoverable or effects.clear_after_manual_verification"
        }
        domain::side_effect::SideEffectStatus::Unrecoverable => {
            "effects.clear_after_manual_verification"
        }
        _ => "effects.inspect",
    }
}

/// P077 BLK-004: Attach closeout_readiness_summary to a run JSON value.
///
/// Routes through CloseoutReadinessSummaryAccessor (R14 §architecture.single_accessor)
/// so GraphQL, MCP runs.get/list, and exported projections all share one typed shape.
async fn attach_closeout_readiness_summary(
    pool: &SqlitePool,
    mut value: serde_json::Value,
) -> Result<serde_json::Value> {
    let run_id = value
        .get("id")
        .and_then(|id| id.as_str())
        .and_then(|id| id.parse::<RunId>().ok());

    let summary_value = match run_id {
        Some(run_id) => {
            let run_id_str = run_id.to_string();
            match closeout::load_closeout_readiness_summary(pool, &run_id_str).await? {
                Some(summary) => serde_json::to_value(&summary)?,
                None => serde_json::Value::Null,
            }
        }
        None => serde_json::Value::Null,
    };

    if let Some(object) = value.as_object_mut() {
        object.insert(
            "closeout_readiness_summary".to_string(),
            summary_value.clone(),
        );
        object.insert(
            "implementation_closeout_readiness_summary".to_string(),
            summary_value,
        );
    }

    Ok(value)
}

/// P058 SEC-003: Per-array row caps to prevent unbounded MCP readback expansion.
const ESCALATION_MAX_LEDGERS: usize = 50;
const ESCALATION_MAX_EVENTS_PER_LEDGER: usize = 200;
const ESCALATION_MAX_EXEC_METAS_PER_LEDGER: usize = 100;

/// P058 Phase 1: Build escalation_readback JSON for a run.
/// Returns the same frozen escalation fields as the GraphQL runEscalationReadback query.
/// Parity rules (must match graphql-server/src/types/escalation.rs::run_escalation_readback):
///   - has_active_escalation: true only when trigger_raw IS NOT NULL or status != 'active' (BLOCK-2)
///   - paused_chain_count / dominant_pause_reason_raw: include both "paused" and "exhausted" status
/// Row caps: max 50 ledgers, 200 events/ledger, 100 exec metas/ledger; truncated arrays include
/// a `*_truncated: true` marker and `*_total` count.
/// Authorization: call only for Operator principals; use build_escalation_readback_summary_json
/// for Agent/Observer principals.
pub async fn build_escalation_readback_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    // Fetch the capped page of ledgers plus aggregate totals from separate COUNT queries
    // so that chains_total / paused_chain_count are accurate even when >ESCALATION_MAX_LEDGERS
    // rows exist (SEC-P058-004, SEC-P058-005).
    let all_ledgers = escalation_repo::find_ledgers_by_run(pool, run_id).await?;
    let chains_total = escalation_repo::count_ledgers_by_run(pool, run_id).await?;
    let paused_count = escalation_repo::count_paused_ledgers_by_run(pool, run_id).await?;
    // BLOCK-2: use triggered count (trigger_raw IS NOT NULL OR status != 'active') rather than
    // chains_total to avoid false has_active_escalation for claim-start ledgers.
    let triggered_count = escalation_repo::count_triggered_ledgers_by_run(pool, run_id).await?;
    let dominant_pause_reason =
        escalation_repo::dominant_pause_reason_for_run(pool, run_id).await?;

    let ledger_page_len = all_ledgers.len();
    let ledgers_truncated =
        ledger_page_len >= ESCALATION_MAX_LEDGERS && chains_total > ESCALATION_MAX_LEDGERS as i64;
    let ledgers = &all_ledgers[..ledger_page_len.min(ESCALATION_MAX_LEDGERS)];
    let has_active = triggered_count > 0;

    let mut chains: Vec<serde_json::Value> = Vec::with_capacity(ledgers.len());
    for ledger in ledgers {
        let all_events = escalation_repo::find_events_by_ledger(pool, &ledger.id).await?;
        let all_exec_metas =
            escalation_repo::find_execution_metadata_by_ledger(pool, &ledger.id).await?;

        // Use separate COUNT queries so totals are accurate even when fetch is capped.
        let event_total = escalation_repo::count_events_by_ledger(pool, &ledger.id).await?;
        let meta_total = escalation_repo::count_metas_by_ledger(pool, &ledger.id).await?;
        let events_truncated = all_events.len() >= ESCALATION_MAX_EVENTS_PER_LEDGER
            && event_total > ESCALATION_MAX_EVENTS_PER_LEDGER as i64;
        let metas_truncated = all_exec_metas.len() >= ESCALATION_MAX_EXEC_METAS_PER_LEDGER
            && meta_total > ESCALATION_MAX_EXEC_METAS_PER_LEDGER as i64;
        let runtime_readback =
            escalation_repo::runtime_readback_from_events(ledger, &all_events, events_truncated);

        let event_values: Vec<serde_json::Value> = all_events
            .iter()
            .take(ESCALATION_MAX_EVENTS_PER_LEDGER)
            .map(|ev| {
                serde_json::json!({
                    "id": ev.id,
                    "escalation_ledger_id": ev.escalation_ledger_id,
                    "event_kind_raw": ev.event_kind_raw,
                    "tier_id": ev.tier_id,
                    "tier_kind_raw": ev.tier_kind_raw,
                    "trigger_raw": ev.trigger_raw,
                    "pause_reason_raw": ev.pause_reason_raw,
                    "payload_json": ev.payload_json,
                    "redaction_version": ev.redaction_version,
                    "created_at": ev.created_at.to_rfc3339(),
                })
            })
            .collect();
        let meta_values: Vec<serde_json::Value> = all_exec_metas
            .iter()
            .take(ESCALATION_MAX_EXEC_METAS_PER_LEDGER)
            .map(|em| {
                serde_json::json!({
                    "agent_execution_id": em.agent_execution_id.to_string(),
                    "escalation_ledger_id": em.escalation_ledger_id,
                    "tier_id": em.tier_id,
                    "tier_kind_raw": em.tier_kind_raw,
                    "tier_attempt_index": em.tier_attempt_index,
                    "trigger_raw": em.trigger_raw,
                    "digest_version": em.digest_version,
                    "capacity_probe_counter": em.capacity_probe_counter,
                    "created_at": em.created_at.to_rfc3339(),
                    "updated_at": em.updated_at.to_rfc3339(),
                    // Phase 1b+: shadow columns read from agent_execution_runtime_facts via LEFT JOIN.
                    "would_select_tier_id": em.would_select_tier_id,
                    "would_select_trigger_raw": em.would_select_trigger_raw,
                    "would_select_decision_json": em.would_select_decision_json,
                    "digest_inputs": escalation_repo::digest_inputs_for_meta_from_events(
                        &all_events,
                        &em.tier_id,
                        em.trigger_raw.as_deref(),
                    ),
                })
            })
            .collect();
        chains.push(serde_json::json!({
            "id": ledger.id,
            "run_id": ledger.run_id.to_string(),
            "stage_id": ledger.stage_id,
            "agent_id": ledger.agent_id,
            "policy_id": ledger.policy_id,
            "policy_hash": ledger.policy_hash,
            "status_raw": ledger.status_raw,
            "current_tier_id": ledger.current_tier_id,
            "current_tier_kind_raw": ledger.current_tier_kind_raw,
            "chain_attempt_index": ledger.chain_attempt_index,
            "trigger_raw": ledger.trigger_raw,
            "pause_reason_raw": ledger.pause_reason_raw,
            "operator_action_hint": ledger.operator_action_hint,
            "runbook_anchor": ledger.runbook_anchor,
            "created_at": ledger.created_at.to_rfc3339(),
            "updated_at": ledger.updated_at.to_rfc3339(),
            "waiting_retry_after_until": runtime_readback.waiting_retry_after_until,
            "trace_unavailable_reason_raw": runtime_readback.trace_unavailable_reason_raw,
            "escalation_trace_json_redacted": runtime_readback.escalation_trace_json_redacted,
            "policy_drift_state": runtime_readback.policy_drift_state,
            "external_acknowledgement_ref": runtime_readback.external_acknowledgement_ref,
            "feature_flag_state": runtime_readback.feature_flag_state,
            "events": event_values,
            "events_truncated": events_truncated,
            "events_total": event_total,
            "execution_metas": meta_values,
            "execution_metas_truncated": metas_truncated,
            "execution_metas_total": meta_total,
        }));
    }
    Ok(serde_json::json!({
        "run_id": run_id.to_string(),
        "chains": chains,
        "chains_truncated": ledgers_truncated,
        "chains_total": chains_total,
        "paused_chain_count": paused_count,
        "has_active_escalation": has_active,
        "dominant_pause_reason_raw": dominant_pause_reason,
    }))
}

/// P058 Phase 1: Build escalation_readback summary JSON for non-Operator principals.
/// Returns aggregate fields only — no chain detail, events, execution metadata, or operator hints.
/// Agent/Observer principals see paused_chain_count and has_active_escalation only.
/// SEC-004: dominant_pause_reason_raw is intentionally excluded — it leaks summary operator
/// hint intent to non-Operator principals contrary to the authz contract.
pub async fn build_escalation_readback_summary_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    // Use unbounded aggregate COUNT queries so paused_chain_count is accurate
    // even when the run has more than ESCALATION_MAX_LEDGERS chains.
    // BLOCK-2: use triggered count (not chains_total) to avoid false has_active_escalation.
    let triggered_count = escalation_repo::count_triggered_ledgers_by_run(pool, run_id).await?;
    let paused_count = escalation_repo::count_paused_ledgers_by_run(pool, run_id).await?;
    let has_active = triggered_count > 0;
    Ok(serde_json::json!({
        "run_id": run_id.to_string(),
        "paused_chain_count": paused_count,
        "has_active_escalation": has_active,
        "chains_redacted": true,
    }))
}

/// SEC-001: Validate that a caller-supplied filesystem path does not contain path traversal
/// sequences, null bytes, or Windows-style separators that could escape the intended root.
/// This is a defense-in-depth check; canonicalization and server-side authorization are the
/// primary controls, but we reject suspicious inputs before any `std::fs` call.
fn validate_run_start_path(field: &str, path: &str) -> anyhow::Result<()> {
    if path.contains('\0') {
        anyhow::bail!("runs.start: field '{field}' contains a null byte");
    }
    // Reject Windows-style separators (the daemon runs on macOS/Linux only).
    if path.contains('\\') {
        anyhow::bail!("runs.start: field '{field}' contains a backslash separator");
    }
    // Reject traversal components regardless of surrounding context.
    for component in path.split('/') {
        if component == ".." {
            anyhow::bail!("runs.start: field '{field}' contains a path traversal component '..'");
        }
    }
    // Reject scheme-like prefixes that could be used to confuse path resolution.
    if path.contains("://") {
        anyhow::bail!("runs.start: field '{field}' contains a URI scheme separator");
    }
    Ok(())
}

fn canonicalize_run_start_paths(
    workspace_root: &str,
    artifact_root: &str,
    workflow_yaml_path: &str,
    agent_catalog_yaml_path: &str,
) -> anyhow::Result<(String, String, String, String)> {
    validate_run_start_path("workspace_root", workspace_root)?;
    validate_run_start_path("artifact_root", artifact_root)?;
    validate_run_start_path("workflow_yaml_path", workflow_yaml_path)?;
    validate_run_start_path("agent_catalog_yaml_path", agent_catalog_yaml_path)?;

    let workspace_path = Path::new(workspace_root);
    // MEDIUM-001: fail-closed when workspace_root does not exist. Returning uncanonicalized
    // paths would skip the descendant containment check and allow arbitrary daemon-readable
    // YAML paths to pass through to the compiler under an Operator token.
    if !workspace_path.exists() {
        anyhow::bail!(
            "runs.start: workspace_root '{}' does not exist; create the directory before starting a run",
            workspace_root
        );
    }
    let canonical_workspace = std::fs::canonicalize(workspace_path)
        .with_context(|| format!("runs.start: canonicalize workspace_root '{workspace_root}'"))?;
    let artifact = canonicalize_run_start_child_path(
        "artifact_root",
        artifact_root,
        &canonical_workspace,
        true,
    )?;
    let workflow = canonicalize_run_start_child_path(
        "workflow_yaml_path",
        workflow_yaml_path,
        &canonical_workspace,
        false,
    )?;
    let catalog = canonicalize_run_start_child_path(
        "agent_catalog_yaml_path",
        agent_catalog_yaml_path,
        &canonical_workspace,
        false,
    )?;
    Ok((
        canonical_workspace.to_string_lossy().to_string(),
        artifact.to_string_lossy().to_string(),
        workflow.to_string_lossy().to_string(),
        catalog.to_string_lossy().to_string(),
    ))
}

fn canonicalize_run_start_child_path(
    field: &str,
    raw: &str,
    canonical_workspace: &Path,
    allow_missing_leaf: bool,
) -> anyhow::Result<PathBuf> {
    let path = Path::new(raw);
    let canonical = if path.exists() {
        std::fs::canonicalize(path)
            .with_context(|| format!("runs.start: canonicalize field '{field}'"))?
    } else if allow_missing_leaf {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("runs.start: field '{field}' has no parent"))?;
        let leaf = path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("runs.start: field '{field}' has no leaf"))?;
        if !parent.exists() {
            anyhow::bail!("runs.start: field '{field}' parent does not exist");
        }
        std::fs::canonicalize(parent)
            .with_context(|| format!("runs.start: canonicalize parent for field '{field}'"))?
            .join(leaf)
    } else {
        anyhow::bail!("runs.start: field '{field}' does not exist");
    };

    if !canonical.starts_with(canonical_workspace) {
        anyhow::bail!("runs.start: field '{field}' escapes canonical workspace_root");
    }
    Ok(canonical)
}

/// SEC HIGH-001: Remove operator-only run snapshot fields for non-Operator MCP principals.
/// These fields carry frozen escalation policies, workflow snapshots, and local filesystem paths
/// that Agent/Observer principals must not be able to recover. Call immediately after serializing
/// a full domain Run — before any caller-facing return — in both runs.get and run:// paths.
pub fn redact_run_snapshot_fields(value: &mut serde_json::Value, is_operator: bool) {
    if is_operator {
        return;
    }
    const OPERATOR_ONLY_FIELDS: &[&str] = &[
        "catalog_snapshot_json",
        "workflow_snapshot_json",
        "delivery_configuration_json",
        "delivery_preflight_json",
        "drift_details_json",
        "chainworks_meta_root",
        "workflow_yaml_path",
        "agent_catalog_yaml_path",
        "worktree_root",
        // HIGH-001: local filesystem paths must not be exposed to Agent/Observer principals.
        "workspace_root",
        "artifact_root",
    ];
    if let Some(obj) = value.as_object_mut() {
        for field in OPERATOR_ONLY_FIELDS {
            obj.remove(*field);
        }
    }
}

/// SEC-003 / HIGH-001: Remove local filesystem path fields and operator-grade completion
/// projection fields from a serialized RunProjectionRow for non-Operator MCP principals.
/// Called on every row returned by runs.list and chainworks://runs.
///
/// The completion/closeout fields carry runtime tool paths, failure envelopes,
/// prompt/artifact paths, and evidence paths that must not be visible to Agent or
/// Observer principals.
pub fn redact_run_projection_paths(value: &mut serde_json::Value, is_operator: bool) {
    if is_operator {
        return;
    }
    const REDACTED_FIELDS: &[&str] = &[
        // Filesystem paths
        "workspace_root",
        "artifact_root",
        "chainworks_meta_root",
        // HIGH-001: operator-grade completion/closeout projection nested objects contain
        // runtime tool paths, failure envelopes, artifact paths, and evidence paths.
        "implementationCompletion",
        "closeout_readiness_summary",
        "implementation_closeout_readiness_summary",
    ];
    if let Some(obj) = value.as_object_mut() {
        for field in REDACTED_FIELDS {
            obj.remove(*field);
        }
    }
}

/// Attach escalation_readback to a run JSON value.
/// Extracts run_id from the value's "id" field; returns empty readback when absent or unparseable.
/// Operator principals receive full chain detail; Agent/Observer receive aggregate summary only.
async fn attach_escalation_readback(
    pool: &SqlitePool,
    mut value: serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let is_operator = matches!(principal.class, auth::PrincipalClass::Operator);
    let run_id = value
        .get("id")
        .and_then(|id| id.as_str())
        .and_then(|id| id.parse::<RunId>().ok());

    let readback = match (run_id, is_operator) {
        (Some(run_id), true) => build_escalation_readback_json(pool, run_id).await?,
        (Some(run_id), false) => build_escalation_readback_summary_json(pool, run_id).await?,
        // SEC-L-001: route by principal class so Agent/Observer always receive the summary
        // shape (chains_redacted=true) even when run_id is absent or unparseable.
        (None, true) => serde_json::json!({
            "chains": [],
            "paused_chain_count": 0,
            "has_active_escalation": false,
        }),
        (None, false) => serde_json::json!({
            "paused_chain_count": 0,
            "has_active_escalation": false,
            "chains_redacted": true,
        }),
    };

    if let Some(obj) = value.as_object_mut() {
        obj.insert("escalation_readback".to_string(), readback);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{artifact_contracts, artifacts, ideas, rollout_contract_checks, runs};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::run::{Run, RunStatus};
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "Test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed");
        let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .expect("register shared DbWriter for test pool");
        pool
    }

    fn make_run(id: RunId, idea_id: IdeaId) -> Run {
        Run {
            id,
            idea_id,
            status: RunStatus::Ready,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: None,
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: Some(
                "{\"repo_identifier\":\"repo-3\",\"repo_root\":\"/repo-3\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .into(),
            ),
            delivery_preflight_json: Some(r#"{"passed":true,"checks":[{"id":"repo_root_exists","passed":true}]}"#.into()),
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
        }
    }

    fn test_workflow_yaml_path() -> String {
        // Canonicalize to resolve `..` components so validate_run_start_path accepts it.
        let raw = format!(
            "{}/../../../examples/workflows/workflow.yaml",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::canonicalize(&raw)
            .unwrap_or_else(|_| std::path::PathBuf::from(&raw))
            .to_string_lossy()
            .to_string()
    }

    fn test_agent_catalog_yaml_path() -> String {
        let raw = format!(
            "{}/../../../examples/agents/agents.yaml",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::canonicalize(&raw)
            .unwrap_or_else(|_| std::path::PathBuf::from(&raw))
            .to_string_lossy()
            .to_string()
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        CommandHandler::new(pool, events, work_queue)
    }

    fn test_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    async fn persist_blocked_implementation_summary(pool: &SqlitePool, run_id: RunId) {
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_8_implementation_continued".into(),
            agent_id: "code_writer".into(),
            name: "implementation_self_assessment".into(),
            contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/implementation/self-assessment.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "test".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(pool, &artifact).await.unwrap();
        let raw = serde_json::json!({
            "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
            "implementation_complete": true,
            "verification_green": false,
            "remaining_code_tasks": [],
            "handoff_tasks": [],
            "known_risks": ["verification blocked by environment"],
            "tests_run": ["cargo test: blocked"],
            "docs_impacted": []
        });
        let summary = parse_implementation_self_assessment_v2(
            &raw,
            ContractParseContext {
                run_id: run_id.to_string(),
                run_age: None,
                declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
                canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
                raw_artifact_path: Some(artifact.file_path.clone()),
                source_generation_id: None,
                artifact_created_at: Some(artifact.created_at),
                v2_generation_seen_for_run: true,
                legacy_v1_generation_available: false,
            },
        );
        artifact_contracts::persist_implementation_self_assessment_summary(
            pool,
            run_id,
            artifact.id,
            &artifact.contract_id,
            &summary,
            artifact.created_at,
        )
        .await
        .unwrap();
    }

    async fn persist_rollout_contract_readback(pool: &SqlitePool, run_id: RunId) {
        use rollout_contract_checks::{
            ProjectionIntegrity, RolloutContractDecision, RolloutContractEnforcementMode,
            RolloutContractLifecycleState, RolloutContractStatus, UpsertRolloutContractCheck,
        };

        let now = Utc::now();
        rollout_contract_checks::upsert_rollout_contract_check(
            pool,
            &UpsertRolloutContractCheck {
                id: uuid::Uuid::new_v4(),
                run_id: run_id.inner(),
                proposal_id: "proposal-084".into(),
                proposal_revision_id: "p084-r5".into(),
                proposal_content_hash: "sha256:proposal".into(),
                contract_object_hash: "sha256:contract".into(),
                content_snapshot_id: "snapshot-1".into(),
                checker_version: "p084-lint-1".into(),
                status: RolloutContractStatus::Pass,
                decision: RolloutContractDecision::Release,
                lifecycle_state: RolloutContractLifecycleState::Terminal,
                enforcement_mode: RolloutContractEnforcementMode::Enforce,
                failure_reasons: vec![],
                diagnostics: vec![],
                waiver: None,
                rollback_disposition: serde_json::json!({
                    "mode": "feature_flag_disable_or_enforcement_mode_permissive",
                    "data_loss_risk": "none",
                    "steps": ["Move enforcement mode through an audited mutation."]
                }),
                projection_integrity: ProjectionIntegrity::Valid,
                cutover_policy_revision: Some("p084-cutover-v1".into()),
                redaction_state: "partial".into(),
                retry_count: 0,
                preflight_timeout_seconds: 45,
            },
            now,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn runs_start_persists_delivery_configuration_json() {
        let start_schema = tool_specs()
            .into_iter()
            .find(|tool| tool.name == "runs.start")
            .expect("runs.start tool spec")
            .input_schema;
        assert_eq!(
            start_schema["properties"]["review_routing_json"]["type"],
            "string"
        );
        // HIGH-002: idempotency_key must be required in the runs.start spec.
        assert_eq!(
            start_schema["required"]
                .as_array()
                .map(|arr| arr.iter().any(|v| v == "idempotency_key")),
            Some(true),
            "runs.start must require idempotency_key"
        );

        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .expect("git init should run");
        let worktrees = tempfile::tempdir().unwrap();
        let delivery_json = format!(
            r#"{{"repo_identifier":"repo-1","repo_root":"{}","base_branch":"main","worktree_base_path":"{}","target_branch":"cw/release","release_target_id":"app-store"}}"#,
            repo.path().display(),
            worktrees.path().display()
        );
        let review_routing_json =
            r#"{"mode":"legacy_fixed","force_include":[],"force_exclude":[]}"#.to_string();
        // MEDIUM-001: workspace_root must be an existing directory; yaml paths must be
        // canonical descendants. Use the control-plane root where examples/ already lives.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("control-plane workspace root");
        // artifact_root parent (workspace_root) must exist; leaf need not.
        let artifact_root = workspace_root.join(".test_chainworks_artifacts_tmp");
        let params = serde_json::json!({
            "idea_id": idea_id.to_string(),
            "workflow_id": "wf-start",
            "workflow_title": "Start Run",
            "workspace_root": workspace_root.to_string_lossy(),
            "artifact_root": artifact_root.to_string_lossy(),
            "workflow_yaml_path": test_workflow_yaml_path(),
            "agent_catalog_yaml_path": test_agent_catalog_yaml_path(),
            "idempotency_key": uuid::Uuid::new_v4().to_string(),
            "delivery_configuration_json": delivery_json,
            "review_routing_json": review_routing_json
        });

        let result = execute("runs.start", params, &pool, &handler, &test_principal())
            .await
            .unwrap();
        let run_id = result["id"].as_str().expect("run id");
        let run = runs::find_by_id(&pool, run_id.parse().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(run.delivery_configuration_json, Some(delivery_json));
        assert_eq!(
            run.review_routing_json
                .as_deref()
                .and_then(
                    |json| serde_json::from_str::<domain::routing::ReviewRoutingOptions>(json).ok()
                )
                .map(|opts| opts.mode),
            Some(domain::routing::ReviewRoutingMode::LegacyFixed)
        );
    }

    #[tokio::test]
    async fn runs_start_rejects_workflow_yaml_symlink_escape_from_existing_workspace() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let artifact_root = workspace.path().join(".chainworks");
        std::fs::create_dir_all(&artifact_root).unwrap();
        let catalog_path = workspace.path().join("agents.yaml");
        std::fs::write(&catalog_path, "agents: []\n").unwrap();
        let outside_workflow = outside.path().join("workflow.yaml");
        std::fs::write(&outside_workflow, "states: {}\n").unwrap();
        let escaped_workflow = workspace.path().join("workflow-link.yaml");
        std::os::unix::fs::symlink(&outside_workflow, &escaped_workflow).unwrap();

        let params = serde_json::json!({
            "idea_id": idea_id.to_string(),
            "workflow_id": "wf-start",
            "workflow_title": "Start Run",
            "workspace_root": workspace.path().to_string_lossy(),
            "artifact_root": artifact_root.to_string_lossy(),
            "workflow_yaml_path": escaped_workflow.to_string_lossy(),
            "agent_catalog_yaml_path": catalog_path.to_string_lossy(),
            "idempotency_key": uuid::Uuid::new_v4().to_string()
        });

        let error = execute("runs.start", params, &pool, &handler, &test_principal())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("escapes canonical workspace_root"));
    }

    #[tokio::test]
    async fn runs_get_returns_cancellation_settlement_log() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let run = domain::run::Run {
            cancellation_settlement_log: Some(
                serde_json::json!([
                    {
                        "agent_execution_id": "ae-1",
                        "agent_id": "writer",
                        "prior_status": "running",
                        "terminal_status": "cancelled",
                        "session_close_attempted": true,
                        "session_close_succeeded": true,
                        "settled_at": "2026-04-15T10:00:00Z"
                    }
                ])
                .to_string(),
            ),
            ..make_run(RunId::new(), idea_id)
        };
        runs::insert(&pool, &run).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(result["cancellation_settlement_log"].as_str().unwrap()).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!([
                {
                    "agent_execution_id": "ae-1",
                    "agent_id": "writer",
                    "prior_status": "running",
                    "terminal_status": "cancelled",
                    "session_close_attempted": true,
                    "session_close_succeeded": true,
                    "settled_at": "2026-04-15T10:00:00Z"
                }
            ])
        );
    }

    #[tokio::test]
    async fn runs_get_returns_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();
        persist_blocked_implementation_summary(&pool, run.id).await;
        persist_rollout_contract_readback(&pool, run.id).await;

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let summary = &result["implementation_self_assessment_summary"];
        assert_eq!(summary["status"], serde_json::json!("blocked"));
        assert_eq!(summary["implementation_complete"], serde_json::json!(true));
        assert_eq!(summary["verification_green"], serde_json::json!(false));
        assert_eq!(
            result["rollout_contract_readback"]["schema_version"],
            serde_json::json!("operator_readback_v1")
        );
        assert_eq!(
            result["rollout_contract_readback"]["backend_decision"],
            serde_json::json!("release")
        );
    }

    #[tokio::test]
    async fn delivery_preflight_mcp_readback_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        assert!(result["delivery_preflight_json"]
            .as_str()
            .unwrap()
            .contains("repo_root_exists"));
    }

    #[tokio::test]
    async fn runs_list_returns_projection_summary_not_full_log() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let run = domain::run::Run {
            status: domain::run::RunStatus::Cancelling,
            cancellation_settlement_log: Some(
                serde_json::json!([
                    {
                        "agent_execution_id": "ae-1",
                        "agent_id": "writer",
                        "prior_status": "running",
                        "terminal_status": "cancelled",
                        "session_close_attempted": true,
                        "session_close_succeeded": true,
                        "settled_at": "2026-04-15T10:00:00Z"
                    },
                    {
                        "agent_execution_id": "ae-2",
                        "agent_id": "reviewer",
                        "prior_status": "running",
                        "terminal_status": "cancelled",
                        "session_close_attempted": true,
                        "session_close_succeeded": false,
                        "settled_at": "2026-04-15T10:00:02Z"
                    }
                ])
                .to_string(),
            ),
            ..make_run(RunId::new(), idea_id)
        };
        runs::insert(&pool, &run).await.unwrap();
        db::repos::projections::rebuild_all_for_run(&pool, run.id)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.list",
            serde_json::json!({}),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let item = result.as_array().unwrap().first().unwrap();
        assert_eq!(
            item["cancellation_settlement_summary"],
            serde_json::json!("2/2 agents settled, 1 sessions closed")
        );
        assert!(item.get("cancellation_settlement_log").is_none());
    }

    #[tokio::test]
    async fn runs_list_includes_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();
        persist_blocked_implementation_summary(&pool, run.id).await;
        db::repos::projections::rebuild_all_for_run(&pool, run.id)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.list",
            serde_json::json!({}),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let item = result.as_array().unwrap().first().unwrap();
        assert_eq!(
            item["implementation_self_assessment_summary"]["status"],
            serde_json::json!("blocked")
        );
    }

    #[tokio::test]
    async fn runs_list_records_production_read_latency_metric() {
        db::repos::storage_health::reset_read_path_metrics_for_tests();
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();
        db::repos::projections::rebuild_all_for_run(&pool, run.id)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        execute(
            "runs.list",
            serde_json::json!({}),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let health = db::repos::storage_health::storage_health(&pool)
            .await
            .unwrap();
        assert!(
            health["readPath"]["runsList"]["sampleCount"]
                .as_u64()
                .is_some_and(|n| n >= 1),
            "expected at least 1 runs.list sample after call"
        );
        assert!(health["readPath"]["runsList"]["p95Ms"].as_u64().is_some());
    }

    #[tokio::test]
    async fn proposal_087_runs_list_p95_stays_under_budget_from_projection() {
        db::repos::storage_health::reset_read_path_metrics_for_tests();
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let now = Utc::now().to_rfc3339();
        for index in 0..120 {
            let run = make_run(RunId::new(), idea_id);
            runs::insert(&pool, &run).await.unwrap();
            sqlx::query(
                r#"INSERT INTO run_summaries
                   (run_id, idea_id, workflow_title, status, total_stages, completed_stages,
                    failed_stages, pending_approvals, started_at, updated_at)
                   VALUES (?1, ?2, ?3, 'ready', ?4, 0, 0, 0, ?5, ?5)"#,
            )
            .bind(run.id.to_string())
            .bind(idea_id.to_string())
            .bind(format!("Workflow {index}"))
            .bind((index % 4) as i64)
            .bind(&now)
            .execute(&pool)
            .await
            .unwrap();
        }

        let handler = make_command_handler(pool.clone());
        let mut durations_ms = Vec::new();
        for _ in 0..8 {
            let started = std::time::Instant::now();
            let result = execute(
                "runs.list",
                serde_json::json!({}),
                &pool,
                &handler,
                &test_principal(),
            )
            .await
            .unwrap();
            assert_eq!(result.as_array().unwrap().len(), 120);
            durations_ms.push(started.elapsed().as_millis() as u64);
        }
        durations_ms.sort_unstable();
        let p95 = durations_ms[durations_ms.len() - 1];
        assert!(
            p95 <= 500,
            "projection-backed runs.list p95 must stay under 500 ms, got {p95} ms from samples {durations_ms:?}"
        );

        let health = db::repos::storage_health::storage_health(&pool)
            .await
            .unwrap();
        assert_eq!(health["readPath"]["runsList"]["sampleCount"], 8);
        assert!(
            health["readPath"]["runsList"]["p95Ms"]
                .as_u64()
                .unwrap_or(u64::MAX)
                <= 500
        );
    }

    #[test]
    fn proposal_064_parsers_accept_frozen_contract_values() {
        assert_eq!(
            parse_main_sync_trigger_reason("before_review").unwrap(),
            MainSyncTriggerReason::BeforeReview
        );
        assert_eq!(
            parse_main_sync_mode("manual_only").unwrap(),
            MainSyncMode::ManualOnly
        );
        assert_eq!(
            parse_main_sync_recovery_decision("retry_sync").unwrap(),
            MainSyncRecoveryDecision::RetrySync
        );
    }

    // P058 Phase 1: MCP escalation readback parity ────────────────────────────

    #[tokio::test]
    async fn runs_get_returns_escalation_readback_key() {
        // runs.get must include escalation_readback even when no ledger rows exist.
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let readback = &result["escalation_readback"];
        assert!(
            !readback.is_null(),
            "escalation_readback must be present on runs.get"
        );
        assert_eq!(readback["chains"].as_array().unwrap().len(), 0);
        assert_eq!(readback["paused_chain_count"], serde_json::json!(0));
        assert_eq!(readback["has_active_escalation"], serde_json::json!(false));
    }

    // BLOCK-2 regression: a claim-start ledger (trigger_raw NULL, status = 'active') must NOT
    // flip has_active_escalation to true — it represents the first attempt with a configured
    // policy, not an active escalation event. has_active_escalation is false until a trigger fires.
    #[tokio::test]
    async fn runs_get_claim_start_ledger_does_not_set_has_active_escalation() {
        use chrono::Utc;
        use domain::escalation::EscalationLedger;

        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        // Insert a claim-start ledger: policy is configured but no trigger has fired yet.
        let ledger = EscalationLedger {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run.id,
            stage_id: "state_10_implementation".to_string(),
            agent_id: "code_writer".to_string(),
            policy_id: "code_writer_default_escalation".to_string(),
            policy_hash: "sha256:abc123".to_string(),
            status_raw: "active".to_string(),
            current_tier_id: Some("primary_retry".to_string()),
            current_tier_kind_raw: Some("same_backend_retry".to_string()),
            chain_attempt_index: 0,
            trigger_raw: None, // no trigger yet — first attempt underway
            pause_reason_raw: None,
            operator_action_hint: None,
            runbook_anchor: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::repos::escalation::insert_ledger(&pool, &ledger)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let readback = &result["escalation_readback"];
        // BLOCK-2: claim-start ledger (trigger_raw=NULL, status='active') must NOT indicate active escalation.
        assert_eq!(
            readback["has_active_escalation"],
            serde_json::json!(false),
            "BLOCK-2: claim-start ledger with trigger_raw=NULL must not set has_active_escalation=true; \
             has_active_escalation should only be true after a failure trigger fires"
        );
        // chains_total still reflects the ledger exists.
        assert_eq!(readback["chains_total"], serde_json::json!(1));
        // paused_chain_count is 0 (status is 'active', not 'paused' or 'exhausted').
        assert_eq!(readback["paused_chain_count"], serde_json::json!(0));
    }

    #[tokio::test]
    async fn runs_get_returns_escalation_readback_with_ledger() {
        use chrono::Utc;
        use domain::escalation::EscalationLedger;

        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        // Insert a paused escalation ledger.
        let ledger = EscalationLedger {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run.id,
            stage_id: "state_10_implementation".to_string(),
            agent_id: "code_writer".to_string(),
            policy_id: "code_writer_default_escalation".to_string(),
            policy_hash: "sha256:abc123".to_string(),
            status_raw: "paused".to_string(),
            current_tier_id: Some("human_pause".to_string()),
            current_tier_kind_raw: Some("pause".to_string()),
            chain_attempt_index: 3,
            trigger_raw: Some("contract_output_failure".to_string()),
            pause_reason_raw: Some("escalation_chain_exhausted".to_string()),
            operator_action_hint: Some("Extend the chain or accept terminal pause.".to_string()),
            runbook_anchor: Some("escalation/chain-exhausted".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        db::repos::escalation::insert_ledger(&pool, &ledger)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let readback = &result["escalation_readback"];
        assert_eq!(readback["paused_chain_count"], serde_json::json!(1));
        // has_active_escalation is true: ledger has trigger_raw set and status_raw = 'paused'
        assert_eq!(readback["has_active_escalation"], serde_json::json!(true));
        assert_eq!(
            readback["dominant_pause_reason_raw"],
            serde_json::json!("escalation_chain_exhausted")
        );
        let chains = readback["chains"].as_array().unwrap();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0]["status_raw"], serde_json::json!("paused"));
        assert_eq!(
            chains[0]["policy_id"],
            serde_json::json!("code_writer_default_escalation")
        );
        assert_eq!(chains[0]["chain_attempt_index"], serde_json::json!(3));
        assert!(chains[0]["events"].as_array().is_some());
        assert!(chains[0]["execution_metas"].as_array().is_some());
    }

    // BLOCK-3: payload_json round-trips through MCP readback
    // BLOCK-2: all GraphQL-parity aggregate and chain fields are present and correct
    #[tokio::test]
    async fn runs_get_escalation_readback_event_payload_json_roundtrip() {
        use domain::escalation::{EscalationEvent, EscalationLedger};

        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let ledger = EscalationLedger {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run.id,
            stage_id: "state_10_implementation".to_string(),
            agent_id: "code_writer".to_string(),
            policy_id: "policy_escalation_v1".to_string(),
            policy_hash: "sha256:deadbeef".to_string(),
            status_raw: "active".to_string(),
            current_tier_id: Some("primary_retry".to_string()),
            current_tier_kind_raw: Some("same_backend_retry".to_string()),
            chain_attempt_index: 1,
            trigger_raw: Some("contract_output_failure".to_string()),
            pause_reason_raw: None,
            operator_action_hint: None,
            runbook_anchor: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        db::repos::escalation::insert_ledger(&pool, &ledger)
            .await
            .unwrap();

        // Insert an event WITH payload_json — verifies the field round-trips (BLOCK-3).
        let event_payload = r#"{"digest_inputs":{"failure_kind":"contract_output_failure"},"redacted_evidence_ref":"sha256:abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789","waiting_retry_after_until":"2026-05-25T12:00:00Z","external_acknowledgement_ref":"ack_p058_001"}"#;
        let event = EscalationEvent {
            id: uuid::Uuid::new_v4().to_string(),
            escalation_ledger_id: ledger.id.clone(),
            event_kind_raw: "escalation.tier_selected".to_string(),
            tier_id: Some("primary_retry".to_string()),
            tier_kind_raw: Some("same_backend_retry".to_string()),
            trigger_raw: Some("contract_output_failure".to_string()),
            pause_reason_raw: None,
            payload_json: Some(event_payload.to_string()),
            redaction_version: Some("redaction_v1".to_string()),
            created_at: chrono::Utc::now(),
        };
        db::repos::escalation::insert_event(&pool, &event)
            .await
            .unwrap();

        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &make_command_handler(pool.clone()),
            &test_principal(),
        )
        .await
        .unwrap();

        let readback = &result["escalation_readback"];

        // BLOCK-2: aggregate parity with GraphQL runEscalationReadback shape
        assert_eq!(readback["run_id"].as_str().unwrap(), run.id.to_string());
        assert_eq!(readback["has_active_escalation"], serde_json::json!(true));
        assert_eq!(readback["paused_chain_count"], serde_json::json!(0)); // active, not paused
        assert!(readback["dominant_pause_reason_raw"].is_null());

        let chains = readback["chains"].as_array().unwrap();
        assert_eq!(chains.len(), 1);
        let chain = &chains[0];

        // All chain fields present — mirrors GqlEscalationChainState field set
        assert_eq!(
            chain["stage_id"].as_str().unwrap(),
            "state_10_implementation"
        );
        assert_eq!(chain["agent_id"].as_str().unwrap(), "code_writer");
        assert_eq!(chain["policy_id"].as_str().unwrap(), "policy_escalation_v1");
        assert_eq!(chain["policy_hash"].as_str().unwrap(), "sha256:deadbeef");
        assert_eq!(chain["status_raw"].as_str().unwrap(), "active");
        assert_eq!(chain["current_tier_id"].as_str().unwrap(), "primary_retry");
        assert_eq!(
            chain["current_tier_kind_raw"].as_str().unwrap(),
            "same_backend_retry"
        );
        assert_eq!(chain["chain_attempt_index"], serde_json::json!(1));
        assert_eq!(
            chain["trigger_raw"].as_str().unwrap(),
            "contract_output_failure"
        );
        assert!(chain["pause_reason_raw"].is_null());

        // BLOCK-3: event fields including payload_json round-trip correctly
        let events = chain["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        let ev = &events[0];
        assert_eq!(
            ev["event_kind_raw"].as_str().unwrap(),
            "escalation.tier_selected"
        );
        assert_eq!(ev["tier_id"].as_str().unwrap(), "primary_retry");
        assert_eq!(ev["tier_kind_raw"].as_str().unwrap(), "same_backend_retry");
        assert_eq!(
            ev["trigger_raw"].as_str().unwrap(),
            "contract_output_failure"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(ev["payload_json"].as_str().unwrap())
                .unwrap(),
            serde_json::from_str::<serde_json::Value>(event_payload).unwrap()
        );
        assert_eq!(ev["redaction_version"].as_str().unwrap(), "redaction_v1");

        assert_eq!(
            chain["waiting_retry_after_until"].as_str().unwrap(),
            "2026-05-25T12:00:00Z"
        );
        assert_eq!(
            chain["external_acknowledgement_ref"].as_str().unwrap(),
            "ack_p058_001"
        );
        assert_eq!(
            chain["feature_flag_state"].as_str().unwrap(),
            "in_flight_continue"
        );
        let trace = serde_json::from_str::<serde_json::Value>(
            chain["escalation_trace_json_redacted"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            trace["schema_version"],
            serde_json::json!("p058_escalation_trace_redacted_v1")
        );
        assert_eq!(trace["events"].as_array().unwrap().len(), 1);
    }

    // BLOCK-1 (SEC-001): non-Operator principals receive summary-only escalation_readback ─────────

    #[tokio::test]
    async fn runs_get_agent_principal_receives_summary_only_readback() {
        // Agent/Observer principals must not receive chain detail, events, execution metas,
        // operator_action_hint, or runbook_anchor — only aggregate counts and status flags.
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let ledger = domain::escalation::EscalationLedger {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run.id,
            stage_id: "state_10".to_string(),
            agent_id: "code_writer".to_string(),
            policy_id: "p1".to_string(),
            policy_hash: "sha256:abc".to_string(),
            status_raw: "paused".to_string(),
            current_tier_id: Some("human_pause".to_string()),
            current_tier_kind_raw: Some("pause".to_string()),
            chain_attempt_index: 1,
            trigger_raw: Some("contract_output_failure".to_string()),
            pause_reason_raw: Some("escalation_chain_exhausted".to_string()),
            operator_action_hint: Some("Extend the chain.".to_string()),
            runbook_anchor: Some("escalation/chain-exhausted".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        db::repos::escalation::insert_ledger(&pool, &ledger)
            .await
            .unwrap();

        // Agent principal — must NOT see chain detail
        let agent_principal = auth::Principal::new("agent-x", auth::PrincipalClass::Agent);
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &make_command_handler(pool.clone()),
            &agent_principal,
        )
        .await
        .unwrap();

        let readback = &result["escalation_readback"];
        // Aggregate fields are present
        assert_eq!(readback["has_active_escalation"], serde_json::json!(true));
        assert_eq!(readback["paused_chain_count"], serde_json::json!(1));
        // SEC-004: dominant_pause_reason_raw must NOT be present for Agent/Observer principals.
        assert!(
            readback.get("dominant_pause_reason_raw").is_none(),
            "Agent principal must not receive dominant_pause_reason_raw (SEC-004); got: {readback:?}"
        );
        // Summary marker is set
        assert_eq!(readback["chains_redacted"], serde_json::json!(true));
        // Chain detail must be absent
        assert!(
            readback.get("chains").is_none(),
            "Agent principal must not receive chain array"
        );

        // Observer principal — same expectation
        let observer_principal = auth::Principal::new("obs-y", auth::PrincipalClass::Observer);
        let result2 = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &make_command_handler(pool.clone()),
            &observer_principal,
        )
        .await
        .unwrap();
        let readback2 = &result2["escalation_readback"];
        assert_eq!(readback2["chains_redacted"], serde_json::json!(true));
        assert!(readback2.get("chains").is_none());
    }

    // BLOCK-3 (SEC-003): event row cap prevents unbounded expansion ─────────────────────────────

    #[tokio::test]
    async fn build_escalation_readback_truncates_events_beyond_cap() {
        use domain::escalation::{EscalationEvent, EscalationLedger};

        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let ledger = EscalationLedger {
            id: uuid::Uuid::new_v4().to_string(),
            run_id: run.id,
            stage_id: "state_10".to_string(),
            agent_id: "code_writer".to_string(),
            policy_id: "p1".to_string(),
            policy_hash: "sha256:abc".to_string(),
            status_raw: "active".to_string(),
            current_tier_id: None,
            current_tier_kind_raw: None,
            chain_attempt_index: 0,
            trigger_raw: None,
            pause_reason_raw: None,
            operator_action_hint: None,
            runbook_anchor: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        db::repos::escalation::insert_ledger(&pool, &ledger)
            .await
            .unwrap();

        // Insert ESCALATION_MAX_EVENTS_PER_LEDGER + 5 events to force truncation.
        let event_count = ESCALATION_MAX_EVENTS_PER_LEDGER + 5;
        for _ in 0..event_count {
            let event = EscalationEvent {
                id: uuid::Uuid::new_v4().to_string(),
                escalation_ledger_id: ledger.id.clone(),
                event_kind_raw: "escalation.tier_selected".to_string(),
                tier_id: None,
                tier_kind_raw: None,
                trigger_raw: None,
                pause_reason_raw: None,
                payload_json: None,
                redaction_version: Some("redaction_v1".to_string()),
                created_at: chrono::Utc::now(),
            };
            db::repos::escalation::insert_event(&pool, &event)
                .await
                .unwrap();
        }

        let readback = build_escalation_readback_json(&pool, run.id).await.unwrap();
        let chains = readback["chains"].as_array().unwrap();
        assert_eq!(chains.len(), 1);
        let chain = &chains[0];

        // events array is capped to ESCALATION_MAX_EVENTS_PER_LEDGER
        let events = chain["events"].as_array().unwrap();
        assert_eq!(events.len(), ESCALATION_MAX_EVENTS_PER_LEDGER);
        // truncation marker is present
        assert_eq!(chain["events_truncated"], serde_json::json!(true));
        // events_total is the exact COUNT(*) result — greater than the display cap.
        let events_total = chain["events_total"].as_u64().unwrap() as usize;
        assert!(
            events_total >= ESCALATION_MAX_EVENTS_PER_LEDGER,
            "events_total must be at least the cap when truncated; got {events_total}"
        );
    }

    // BLOCK-4 (SEC HIGH-001): operator-only run snapshot fields must not leak ──────────────────────

    fn make_run_with_snapshots(id: RunId, idea_id: IdeaId) -> domain::run::Run {
        domain::run::Run {
            id,
            idea_id,
            status: domain::run::RunStatus::Running,
            workflow_id: "wf-snap".into(),
            workflow_title: "Snapshot Run".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_1".into()),
            workflow_yaml_path: Some("/workspace/workflows/main.yaml".into()),
            agent_catalog_yaml_path: Some("/workspace/agents/agents.yaml".into()),
            worktree_root: Some("/tmp/worktrees/cw-abc123".into()),
            base_branch: Some("main".into()),
            base_revision: None,
            target_branch: Some("cw/feature".into()),
            delivery_configuration_json: Some(r#"{"repo_identifier":"repo-3"}"#.into()),
            delivery_preflight_json: Some(r#"{"passed":true}"#.into()),
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: Some("sha256:wf-hash".into()),
            catalog_snapshot_hash: Some("sha256:cat-hash".into()),
            workflow_snapshot_json: Some(r#"{"states":{"state_1":{"owner":"code_writer"}}}"#.into()),
            catalog_snapshot_json: Some(r#"{"escalation_policies":[{"policy_id":"p1","secret_tier_data":"must-not-leak"}]}"#.into()),
            drift_detected_at: None,
            drift_details_json: Some(r#"{"policy_hash_mismatch":true}"#.into()),
            chainworks_meta_root: Some("/Users/user/Documents/Chainworks Forge/.chainworks".into()),
            review_routing_json: None,
            closeout_readiness_mode: None,
        }
    }

    #[tokio::test]
    async fn p058_sec001_runs_get_operator_receives_snapshot_fields() {
        // Operator principals must receive the full run including snapshot fields for their
        // operator-facing tooling (drift review, policy audit, rollout inspection).
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run_with_snapshots(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let operator = auth::Principal::new("op", auth::PrincipalClass::Operator);
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &make_command_handler(pool.clone()),
            &operator,
        )
        .await
        .unwrap();

        assert!(
            result.get("catalog_snapshot_json").is_some(),
            "Operator must receive catalog_snapshot_json; got: {result:?}"
        );
        assert!(
            result.get("workflow_snapshot_json").is_some(),
            "Operator must receive workflow_snapshot_json"
        );
        assert!(
            result.get("delivery_configuration_json").is_some(),
            "Operator must receive delivery_configuration_json"
        );
        assert!(
            result.get("chainworks_meta_root").is_some(),
            "Operator must receive chainworks_meta_root"
        );
        assert!(
            result.get("workflow_yaml_path").is_some(),
            "Operator must receive workflow_yaml_path"
        );
    }

    #[tokio::test]
    async fn p058_sec001_runs_get_non_operator_snapshot_fields_redacted() {
        // Agent and Observer principals must NOT see catalog_snapshot_json, workflow_snapshot_json,
        // or any other operator-only snapshot/path field — these carry frozen escalation policies,
        // delivery config, and local filesystem paths outside the agent's authz boundary.
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run_with_snapshots(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let operator_only_fields = [
            "catalog_snapshot_json",
            "workflow_snapshot_json",
            "delivery_configuration_json",
            "delivery_preflight_json",
            "drift_details_json",
            "chainworks_meta_root",
            "workflow_yaml_path",
            "agent_catalog_yaml_path",
            "worktree_root",
        ];

        for (class_name, principal) in [
            (
                "Agent",
                auth::Principal::new("agent-a", auth::PrincipalClass::Agent),
            ),
            (
                "Observer",
                auth::Principal::new("obs-b", auth::PrincipalClass::Observer),
            ),
        ] {
            let result = execute(
                "runs.get",
                serde_json::json!({ "run_id": run.id.to_string() }),
                &pool,
                &make_command_handler(pool.clone()),
                &principal,
            )
            .await
            .unwrap();

            for field in &operator_only_fields {
                assert!(
                    result.get(*field).is_none(),
                    "{class_name} principal must not receive {field} in runs.get; got: {result:?}"
                );
            }
            // Aggregate escalation readback must still be present.
            assert!(
                result.get("escalation_readback").is_some(),
                "{class_name} must still receive escalation_readback summary"
            );
        }
    }
}
