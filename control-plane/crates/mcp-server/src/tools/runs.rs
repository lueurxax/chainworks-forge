use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

use db::repos::{
    artifact_contracts, closeout, code_writer_completion_receipts, escalation, ideas,
    legacy_discovery_overrides, projections, rollout_contract_checks, runs, side_effects,
};
use domain::commands::{
    CallerContext, CancelRunCmd, CatalogSnapshotRetrofitScope, Command, KnowledgeCapsuleIgnoreCmd,
    MainSyncMode, MainSyncRecordRecoveryDecisionCmd, MainSyncRecoveryDecision,
    MainSyncRepairStateCmd, MainSyncRequestCmd, MainSyncRetryCmd, MainSyncSetRunOverrideCmd,
    MainSyncTriggerReason, ProposalGateSettlementAction, RetrofitCatalogSnapshotCmd,
    SettleProposalGateCmd, StartRunCmd,
};
use domain::ids::{IdeaId, RunId};
use domain::risk_lineage::RiskAcceptanceLineage;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

fn mcp_caller_with_idempotency_request_id(
    principal: &auth::Principal,
    tool_name: &str,
    idempotency_key: &str,
) -> CallerContext {
    // boundary-no-op: preserves existing Operator-only tool boundary while stamping request identity.
    mcp_caller(principal, tool_name).with_request_id(idempotency_key)
}

/// SEC-HIGH-002: Remove sensitive fields from a serialized Run for non-Operator principals.
/// Strips absolute filesystem paths, delivery/preflight configs, workflow/catalog snapshots,
/// branch internals, and operator-only override fields to enforce least-privilege on Agent
/// and Observer callers.
pub fn redact_run_for_non_operator(obj: &mut serde_json::Map<String, serde_json::Value>) {
    for field in &[
        "workspace_root",
        "artifact_root",
        "workflow_yaml_path",
        "agent_catalog_yaml_path",
        "worktree_root",
        "base_branch",
        "base_revision",
        "target_branch",
        "delivery_configuration_json",
        "delivery_preflight_json",
        "workflow_snapshot_json",
        "catalog_snapshot_json",
        "workflow_snapshot_hash",
        "catalog_snapshot_hash",
        "drift_detected_at",
        "drift_details_json",
        "chainworks_meta_root",
        "review_routing_json",
        "cancellation_settlement_log",
    ] {
        obj.remove(*field);
    }
}

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
                    }
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
            description: "List all runs".to_string(),
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
                    "idempotency_key": { "type": "string", "description": "Caller-supplied idempotency key for cancel deduplication (P082)" }
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

            let idea = ideas::find_by_id(pool, idea_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("runs.start: idea not found"))?;
            let trusted_workspace_root = idea.workspace_root_path.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "runs.start: idea workspace_root_path is required as the trusted filesystem boundary"
                )
            })?;

            // SEC-001: validate and canonicalize caller-supplied paths before any filesystem
            // read. When an idea carries a workspace root, that durable root is the confinement
            // authority; callers cannot widen it by submitting a broader workspace_root.
            let (workspace_root, artifact_root, workflow_yaml_path, agent_catalog_yaml_path) =
                canonicalize_run_start_paths(
                    &workspace_root,
                    &artifact_root,
                    &workflow_yaml_path,
                    &agent_catalog_yaml_path,
                    trusted_workspace_root,
                )?;

            // Propagate idempotency_key as request_id so the command journal records it
            // for cross-surface correlation and replay detection.
            let caller = mcp_caller(principal, "runs.start").with_request_id(idempotency_key);
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
                    let mut value = serde_json::to_value(&run)?;
                    if let Some(obj) = value.as_object_mut() {
                        // SEC-HIGH-002: strip sensitive fields for non-Operator principals.
                        if principal.class != auth::PrincipalClass::Operator {
                            redact_run_for_non_operator(obj);
                        }
                        // active_artifact_index and run_state_projection include operator-only
                        // recovery diagnostics, local paths, and source IDs — Operator only.
                        if principal.class == auth::PrincipalClass::Operator {
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
                        }
                        // legacy_discovery_overrides contain internal routing — Operator only.
                        if principal.class == auth::PrincipalClass::Operator {
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
                            obj.insert(
                                "p082_recovery_matrix_readback".into(),
                                crate::tools::reports::p082_recovery_matrix_readback_json(
                                    pool,
                                    run_id,
                                    &principal.class,
                                )
                                .await?,
                            );
                            obj.insert(
                                "p082_recovery_matrix_readbacks".into(),
                                crate::tools::reports::p082_recovery_matrix_readbacks_json(
                                    pool,
                                    run_id,
                                    &principal.class,
                                    "mcp",
                                )
                                .await?,
                            );
                        }
                        let escalation_readback =
                            if principal.class == auth::PrincipalClass::Operator {
                                build_operator_escalation_readback_json(pool, run_id).await?
                            } else {
                                serde_json::Value::Object(
                                    build_escalation_readback_summary_json(pool, run_id).await?,
                                )
                            };
                        obj.insert("escalation_readback".into(), escalation_readback);
                        if principal.class == auth::PrincipalClass::Operator {
                            let p082_readbacks =
                                db::repos::p082_recovery_matrix::readbacks_for_run(pool, run_id)
                                    .await?;
                            let p082_singular =
                                db::repos::p082_recovery_matrix::latest_readback_from_readbacks(
                                    &p082_readbacks,
                                );
                            db::repos::p082_recovery_matrix::emit_readback_lane_metrics(
                                &p082_readbacks,
                                "mcp",
                            );
                            obj.insert("p082_recovery_matrix_readback".into(), p082_singular);
                            obj.insert(
                                "p082_recovery_matrix_readbacks".into(),
                                serde_json::Value::Array(p082_readbacks),
                            );
                        } else {
                            obj.insert(
                                "p082_recovery_matrix_readback".into(),
                                serde_json::Value::Null,
                            );
                            obj.insert(
                                "p082_recovery_matrix_readbacks".into(),
                                serde_json::Value::Array(vec![]),
                            );
                        }
                    }
                    let is_operator = principal.class == auth::PrincipalClass::Operator;
                    let value =
                        attach_implementation_self_assessment_summary(pool, value, is_operator)
                            .await?;
                    // P077 BLK-004: attach closeout_readiness_summary parity on runs.get.
                    attach_closeout_readiness_summary(pool, value).await
                }
                None => Ok(serde_json::Value::Null),
            }
        }

        "runs.list" => {
            let is_operator = principal.class == auth::PrincipalClass::Operator;
            let items = projections::list_active_projection(pool).await?;
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let mut value = serde_json::to_value(item)?;
                if !is_operator {
                    redact_non_operator_run_projection(&mut value);
                }
                values.push(value);
            }
            Ok(serde_json::Value::Array(values))
        }

        "runs.cancel" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let idempotency_key = params["idempotency_key"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idempotency_key'"))?
                .to_string();
            let caller = mcp_caller(principal, "runs.cancel").with_request_id(idempotency_key);
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
            let caller = mcp_caller(principal, "runs.retrofit_catalog_snapshot")
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
            let caller = mcp_caller_with_idempotency_request_id(
                principal,
                "runs.main_sync.request",
                &idempotency_key,
            );
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
            let caller = mcp_caller_with_idempotency_request_id(
                principal,
                "runs.main_sync.retry",
                &idempotency_key,
            );
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
            let caller = mcp_caller(principal, "runs.main_sync.set_override");
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
            let caller = mcp_caller(principal, "runs.main_sync.repair_state");
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
            let caller = mcp_caller(principal, "runs.main_sync.record_recovery_decision");
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
            let caller = mcp_caller(principal, "runs.knowledge_capsule.ignore");
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

            let caller = mcp_caller(principal, "runs.settle_proposal_gate");
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

/// SEC-001: Validate that a caller-supplied filesystem path does not contain path traversal
/// sequences, null bytes, or Windows-style separators that could escape the intended root.
fn validate_run_start_path(field: &str, path: &str) -> anyhow::Result<()> {
    if path.contains('\0') {
        anyhow::bail!("runs.start: field '{field}' contains a null byte");
    }
    if path.contains('\\') {
        anyhow::bail!("runs.start: field '{field}' contains a backslash separator");
    }
    for component in path.split('/') {
        if component == ".." {
            anyhow::bail!("runs.start: field '{field}' contains a path traversal component '..'");
        }
    }
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
    trusted_workspace_root: &str,
) -> anyhow::Result<(String, String, String, String)> {
    validate_run_start_path("workspace_root", workspace_root)?;
    validate_run_start_path("artifact_root", artifact_root)?;
    validate_run_start_path("workflow_yaml_path", workflow_yaml_path)?;
    validate_run_start_path("agent_catalog_yaml_path", agent_catalog_yaml_path)?;
    validate_run_start_path("idea.workspace_root_path", trusted_workspace_root)?;

    let workspace_path = Path::new(workspace_root);
    // MEDIUM-001: fail-closed when workspace_root does not exist.
    if !workspace_path.exists() {
        anyhow::bail!(
            "runs.start: workspace_root '{}' does not exist; create the directory before starting a run",
            workspace_root
        );
    }
    let canonical_workspace = std::fs::canonicalize(workspace_path)
        .with_context(|| format!("runs.start: canonicalize workspace_root '{workspace_root}'"))?;
    let trusted_path = Path::new(trusted_workspace_root);
    if !trusted_path.exists() {
        anyhow::bail!(
            "runs.start: idea workspace_root_path '{}' does not exist; update the idea before starting a run",
            trusted_workspace_root
        );
    }
    let canonical_trusted_workspace = std::fs::canonicalize(trusted_path).with_context(|| {
        format!("runs.start: canonicalize idea workspace_root_path '{trusted_workspace_root}'")
    })?;
    let canonical_policy_root = canonical_trusted_workspace.as_path();
    reject_broad_run_start_workspace_root(canonical_policy_root)?;
    if canonical_workspace != canonical_trusted_workspace {
        anyhow::bail!(
            "runs.start: workspace_root must match the idea workspace_root_path policy boundary"
        );
    }
    let artifact = canonicalize_run_start_child_path(
        "artifact_root",
        artifact_root,
        workspace_path,
        canonical_policy_root,
        true,
    )?;
    let workflow = canonicalize_run_start_child_path(
        "workflow_yaml_path",
        workflow_yaml_path,
        workspace_path,
        canonical_policy_root,
        false,
    )?;
    let catalog = canonicalize_run_start_child_path(
        "agent_catalog_yaml_path",
        agent_catalog_yaml_path,
        workspace_path,
        canonical_policy_root,
        false,
    )?;
    Ok((
        canonical_workspace.to_string_lossy().to_string(),
        artifact.to_string_lossy().to_string(),
        workflow.to_string_lossy().to_string(),
        catalog.to_string_lossy().to_string(),
    ))
}

fn reject_broad_run_start_workspace_root(canonical_workspace: &Path) -> anyhow::Result<()> {
    let path = canonical_workspace;
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|home| std::fs::canonicalize(home).ok());
    let broad_literals = [
        Path::new("/"),
        Path::new("/Applications"),
        Path::new("/Library"),
        Path::new("/System"),
        Path::new("/Volumes"),
        Path::new("/etc"),
        Path::new("/private"),
        Path::new("/private/etc"),
        Path::new("/tmp"),
        Path::new("/private/tmp"),
        Path::new("/var"),
        Path::new("/private/var"),
        Path::new("/Users"),
        Path::new("/home"),
    ];
    if broad_literals.iter().any(|broad| path == *broad)
        || path
            .parent()
            .is_some_and(|parent| parent == Path::new("/Volumes"))
        || home.as_deref().is_some_and(|home| path == home)
    {
        anyhow::bail!(
            "runs.start: workspace_root is too broad to use as a trusted filesystem boundary"
        );
    }
    Ok(())
}

fn canonicalize_run_start_child_path(
    field: &str,
    raw: &str,
    raw_workspace: &Path,
    canonical_workspace: &Path,
    allow_missing_leaf: bool,
) -> anyhow::Result<PathBuf> {
    let path = Path::new(raw);
    if path.exists() {
        reject_run_start_symlink_components_under(field, path, raw_workspace)?;
    }
    let canonical = if path.exists() {
        std::fs::canonicalize(path)
            .with_context(|| format!("runs.start: canonicalize field '{field}'"))?
    } else if allow_missing_leaf {
        let created_path =
            create_dir_all_no_symlink_under(field, path, raw_workspace, canonical_workspace)?;
        reject_run_start_symlink_components_under(field, path, raw_workspace)?;
        std::fs::canonicalize(created_path)
            .with_context(|| format!("runs.start: canonicalize field '{field}' after create"))?
    } else {
        anyhow::bail!("runs.start: field '{field}' does not exist");
    };

    if !canonical.starts_with(canonical_workspace) {
        anyhow::bail!("runs.start: field '{field}' escapes canonical workspace_root");
    }
    Ok(canonical)
}

fn reject_run_start_symlink_components_under(
    field: &str,
    path: &Path,
    raw_workspace: &Path,
) -> anyhow::Result<()> {
    let relative = path.strip_prefix(raw_workspace).with_context(|| {
        format!("runs.start: field '{field}' is not under the submitted workspace_root")
    })?;
    let mut current = raw_workspace.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        if let Ok(metadata) = std::fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                anyhow::bail!("runs.start: field '{field}' contains a symlink component");
            }
        }
    }
    Ok(())
}

fn create_dir_all_no_symlink_under(
    field: &str,
    path: &Path,
    raw_workspace: &Path,
    canonical_workspace: &Path,
) -> anyhow::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("runs.start: field '{field}' has no parent"))?;
    let leaf = path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("runs.start: field '{field}' has no leaf"))?;
    let target_path =
        canonicalize_missing_directory_target(field, path, parent, leaf, raw_workspace)?;

    let relative = target_path.strip_prefix(canonical_workspace).map_err(|_| {
        anyhow::anyhow!("runs.start: field '{field}' escapes canonical workspace_root")
    })?;
    let mut current = canonical_workspace.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    anyhow::bail!("runs.start: field '{field}' contains a symlink component");
                }
                if !metadata.is_dir() {
                    anyhow::bail!("runs.start: field '{field}' path component is not a directory");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).with_context(|| {
                    format!("runs.start: create directory {}", current.display())
                })?;
                let metadata = std::fs::symlink_metadata(&current).with_context(|| {
                    format!("runs.start: verify created directory {}", current.display())
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    anyhow::bail!(
                        "runs.start: field '{field}' created path was replaced before verification"
                    );
                }
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("runs.start: inspect directory {}", current.display())
                });
            }
        }
    }
    Ok(target_path)
}

fn canonicalize_missing_directory_target(
    field: &str,
    path: &Path,
    parent: &Path,
    leaf: &std::ffi::OsStr,
    raw_workspace: &Path,
) -> anyhow::Result<PathBuf> {
    if parent.exists() {
        reject_run_start_symlink_components_under(field, parent, raw_workspace)?;
        let canonical_parent = std::fs::canonicalize(parent)
            .with_context(|| format!("runs.start: canonicalize parent for field '{field}'"))?;
        return Ok(canonical_parent.join(leaf));
    }

    let mut missing = Vec::new();
    let mut cursor = path;
    while !cursor.exists() {
        let name = cursor.file_name().ok_or_else(|| {
            anyhow::anyhow!("runs.start: field '{field}' has no existing ancestor")
        })?;
        missing.push(name.to_os_string());
        cursor = cursor.parent().ok_or_else(|| {
            anyhow::anyhow!("runs.start: field '{field}' has no existing ancestor")
        })?;
    }
    reject_run_start_symlink_components_under(field, cursor, raw_workspace)?;
    let mut canonical = std::fs::canonicalize(cursor)
        .with_context(|| format!("runs.start: canonicalize ancestor for field '{field}'"))?;
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
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
    let code_writer_completion_receipts = if is_operator {
        match run_id {
            Some(run_id) => {
                let receipts = code_writer_completion_receipts::list_by_run(pool, run_id).await?;
                let canonical_receipts =
                    code_writer_completion_receipts::list_canonical_by_run(pool, run_id).await?;
                Some((
                    serde_json::to_value(&receipts)?,
                    serde_json::to_value(
                        domain::code_writer_completion::project_implementation_completion(
                            &canonical_receipts,
                        ),
                    )?,
                ))
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

/// SEC-004: Non-Operator escalation summary — exposes paused_chain_count and has_active_escalation
/// without leaking dominant_pause_reason_raw or other operator-only chain internals.
pub async fn build_escalation_readback_summary_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let paused_count = escalation::count_paused_ledgers_by_run(pool, run_id).await?;
    let triggered_count = escalation::count_triggered_ledgers_by_run(pool, run_id).await?;
    let has_active = triggered_count > 0;
    let mut map = serde_json::Map::new();
    map.insert(
        "paused_chain_count".into(),
        serde_json::Value::Number(paused_count.into()),
    );
    map.insert(
        "has_active_escalation".into(),
        serde_json::Value::Bool(has_active),
    );
    map.insert("chains_redacted".into(), serde_json::Value::Bool(true));
    Ok(map)
}

async fn build_operator_escalation_readback_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let ledgers = escalation::find_ledgers_by_run(pool, run_id).await?;
    let chains_truncated = ledgers.len() > 50;
    let mut chains = Vec::with_capacity(ledgers.len().min(50));
    for ledger in ledgers.into_iter().take(50) {
        let events = escalation::find_events_by_ledger(pool, &ledger.id).await?;
        let events_total = escalation::count_events_by_ledger(pool, &ledger.id).await?;
        let events_truncated = events.len() > 200;
        let execution_metas =
            escalation::find_execution_metadata_by_ledger(pool, &ledger.id).await?;
        let execution_metas_total = escalation::count_metas_by_ledger(pool, &ledger.id).await?;
        let execution_metas_truncated = execution_metas.len() > 100;
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
            "events": events.into_iter().take(200).map(|event| serde_json::json!({
                "id": event.id,
                "escalation_ledger_id": event.escalation_ledger_id,
                "event_kind_raw": event.event_kind_raw,
                "tier_id": event.tier_id,
                "tier_kind_raw": event.tier_kind_raw,
                "trigger_raw": event.trigger_raw,
                "pause_reason_raw": event.pause_reason_raw,
                "payload_json": event.payload_json,
                "redaction_version": event.redaction_version,
                "created_at": event.created_at.to_rfc3339()
            })).collect::<Vec<_>>(),
            "events_total": events_total,
            "events_truncated": events_truncated,
            "execution_metas": execution_metas.into_iter().take(100).map(|meta| serde_json::json!({
                "agent_execution_id": meta.agent_execution_id.to_string(),
                "escalation_ledger_id": meta.escalation_ledger_id,
                "tier_id": meta.tier_id,
                "tier_kind_raw": meta.tier_kind_raw,
                "tier_attempt_index": meta.tier_attempt_index,
                "trigger_raw": meta.trigger_raw,
                "digest_version": meta.digest_version,
                "capacity_probe_counter": meta.capacity_probe_counter,
                "created_at": meta.created_at.to_rfc3339(),
                "updated_at": meta.updated_at.to_rfc3339(),
                "would_select_tier_id": meta.would_select_tier_id,
                "would_select_trigger_raw": meta.would_select_trigger_raw,
                "would_select_decision_json": meta.would_select_decision_json
            })).collect::<Vec<_>>(),
            "execution_metas_total": execution_metas_total,
            "execution_metas_truncated": execution_metas_truncated
        }));
    }
    Ok(serde_json::json!({
        "schema_version": "p058_escalation_readback_v1",
        "run_id": run_id.to_string(),
        "chains": chains,
        "chains_total": escalation::count_ledgers_by_run(pool, run_id).await?,
        "chains_truncated": chains_truncated
    }))
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

pub(crate) fn redact_non_operator_run_projection(value: &mut serde_json::Value) {
    if let Some(obj) = value.as_object_mut() {
        for field in &[
            "workspace_root",
            "artifact_root",
            "chainworks_meta_root",
            "implementationCompletion",
            "closeout_readiness_summary",
            "implementation_closeout_readiness_summary",
        ] {
            obj.remove(*field);
        }
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
        db::writer::register_shared_writer(
            &pool,
            std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .expect("register shared writer");
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

    #[test]
    fn p082_workspace_root_guard_rejects_broad_system_roots() {
        for root in [
            "/",
            "/tmp",
            "/var",
            "/private",
            "/private/tmp",
            "/private/var",
            "/private/etc",
            "/etc",
            "/Users",
            "/home",
            "/Library",
            "/System",
            "/Volumes",
            "/Applications",
        ] {
            let err = reject_broad_run_start_workspace_root(std::path::Path::new(root))
                .expect_err("broad workspace root must be rejected");
            assert!(
                err.to_string().contains("too broad"),
                "unexpected error for {root}: {err}"
            );
        }
    }

    #[test]
    fn p082_workspace_root_guard_allows_project_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        let canonical = std::fs::canonicalize(&project).unwrap();

        reject_broad_run_start_workspace_root(&canonical)
            .expect("project directory should be allowed");
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

    #[test]
    fn p083_main_sync_mcp_callers_stamp_idempotency_key_as_request_id() {
        let principal = test_principal();

        for tool_name in ["runs.main_sync.request", "runs.main_sync.retry"] {
            let caller = mcp_caller_with_idempotency_request_id(
                &principal,
                tool_name,
                "0197f0d1-1dd2-7b7a-a2b7-2dd6d0052d57",
            );

            assert_eq!(caller.caller_tool, tool_name);
            assert_eq!(
                caller.request_id.as_deref(),
                Some("0197f0d1-1dd2-7b7a-a2b7-2dd6d0052d57")
            );
        }
    }

    #[test]
    fn runs_start_rejects_broad_workspace_root_boundary() {
        let err = canonicalize_run_start_paths(
            "/",
            "/tmp/chainworks-artifact-root",
            "/tmp/workflow.yaml",
            "/tmp/agents.yaml",
            "/",
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("workspace_root is too broad to use as a trusted filesystem boundary"));
    }

    #[test]
    fn sec_med_001_runs_start_rejects_macos_system_workspace_roots() {
        for root in [
            "/private",
            "/private/etc",
            "/Library",
            "/System",
            "/Volumes",
            "/Volumes/External",
            "/Applications",
        ] {
            let err = reject_broad_run_start_workspace_root(Path::new(root)).unwrap_err();
            assert!(
                err.to_string().contains("too broad"),
                "runs.start must reject broad root {root}; got: {err}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn p082_runs_start_rejects_workspace_symlink_to_broad_system_root() {
        let temp = tempfile::tempdir().expect("temp root");
        let workspace_link = temp.path().join("workspace-link");
        std::os::unix::fs::symlink("/private", &workspace_link).expect("workspace symlink");

        let err = canonicalize_run_start_paths(
            workspace_link.to_string_lossy().as_ref(),
            workspace_link
                .join(".chainworks")
                .to_string_lossy()
                .as_ref(),
            workspace_link
                .join("workflow.yaml")
                .to_string_lossy()
                .as_ref(),
            workspace_link
                .join("agents.yaml")
                .to_string_lossy()
                .as_ref(),
            workspace_link.to_string_lossy().as_ref(),
        )
        .expect_err("workspace symlink to broad root must be rejected");

        assert!(
            err.to_string().contains("too broad"),
            "canonicalized workspace symlink must fail broad-root guard; got: {err}"
        );
    }

    #[test]
    fn runs_start_rejects_workspace_root_that_widens_idea_boundary() {
        let trusted = tempfile::tempdir().expect("trusted root");
        let caller_root = tempfile::tempdir().expect("caller root");
        let workflow = caller_root.path().join("workflow.yaml");
        let catalog = caller_root.path().join("agents.yaml");
        std::fs::write(&workflow, "states: {}\n").expect("workflow fixture");
        std::fs::write(&catalog, "agents: []\n").expect("catalog fixture");

        let err = canonicalize_run_start_paths(
            caller_root.path().to_string_lossy().as_ref(),
            caller_root
                .path()
                .join(".chainworks")
                .to_string_lossy()
                .as_ref(),
            workflow.to_string_lossy().as_ref(),
            catalog.to_string_lossy().as_ref(),
            trusted.path().to_string_lossy().as_ref(),
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("workspace_root must match the idea workspace_root_path policy boundary"));
    }

    #[test]
    fn p082_runs_start_rejects_symlinked_artifact_root_component_even_inside_workspace() {
        let workspace = tempfile::tempdir().expect("workspace root");
        let real_artifacts = workspace.path().join("real-artifacts");
        std::fs::create_dir(&real_artifacts).expect("real artifacts dir");
        let workflow = workspace.path().join("workflow.yaml");
        let catalog = workspace.path().join("agents.yaml");
        std::fs::write(&workflow, "states: {}\n").expect("workflow fixture");
        std::fs::write(&catalog, "agents: []\n").expect("catalog fixture");

        let artifact_symlink = workspace.path().join(".chainworks");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real_artifacts, &artifact_symlink)
            .expect("artifact symlink fixture");
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&real_artifacts, &artifact_symlink)
            .expect("artifact symlink fixture");

        let err = canonicalize_run_start_paths(
            workspace.path().to_string_lossy().as_ref(),
            artifact_symlink.to_string_lossy().as_ref(),
            workflow.to_string_lossy().as_ref(),
            catalog.to_string_lossy().as_ref(),
            workspace.path().to_string_lossy().as_ref(),
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("symlink"),
            "artifact_root symlink components must fail closed; got: {err}"
        );
    }

    #[test]
    fn p082_runs_start_rejects_symlinked_artifact_root_child_component() {
        let workspace = tempfile::tempdir().expect("workspace root");
        let outside = tempfile::tempdir().expect("outside target");
        let workflow = workspace.path().join("workflow.yaml");
        let catalog = workspace.path().join("agents.yaml");
        std::fs::write(&workflow, "states: {}\n").expect("workflow fixture");
        std::fs::write(&catalog, "agents: []\n").expect("catalog fixture");

        let chainworks_link = workspace.path().join(".chainworks");
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), &chainworks_link)
            .expect(".chainworks symlink fixture");
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(outside.path(), &chainworks_link)
            .expect(".chainworks symlink fixture");

        let artifact_root = chainworks_link.join("artifacts");
        std::fs::create_dir_all(outside.path().join("artifacts")).expect("target child");

        let err = canonicalize_run_start_paths(
            workspace.path().to_string_lossy().as_ref(),
            artifact_root.to_string_lossy().as_ref(),
            workflow.to_string_lossy().as_ref(),
            catalog.to_string_lossy().as_ref(),
            workspace.path().to_string_lossy().as_ref(),
        )
        .unwrap_err();

        assert!(
            err.to_string().contains("symlink"),
            "artifact_root child below symlinked .chainworks must fail closed; got: {err}"
        );
    }

    #[test]
    fn p082_runs_start_creates_and_canonicalizes_artifact_root_before_persistence() {
        let workspace = tempfile::tempdir().expect("workspace root");
        let workflow = workspace.path().join("workflow.yaml");
        let catalog = workspace.path().join("agents.yaml");
        std::fs::write(&workflow, "states: {}\n").expect("workflow fixture");
        std::fs::write(&catalog, "agents: []\n").expect("catalog fixture");
        let artifact_root = workspace.path().join(".chainworks").join("artifacts");

        let (_, artifact_root_out, _, _) = canonicalize_run_start_paths(
            workspace.path().to_string_lossy().as_ref(),
            artifact_root.to_string_lossy().as_ref(),
            workflow.to_string_lossy().as_ref(),
            catalog.to_string_lossy().as_ref(),
            workspace.path().to_string_lossy().as_ref(),
        )
        .expect("missing artifact_root leaf should be created safely");

        assert!(
            artifact_root.is_dir(),
            "artifact_root must exist before StartRun"
        );
        assert_eq!(
            std::fs::canonicalize(&artifact_root).unwrap(),
            std::path::PathBuf::from(artifact_root_out)
        );
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
        let mut idea = make_idea(idea_id);
        idea.workspace_root_path = Some(workspace_root.to_string_lossy().into_owned());
        ideas::insert(&pool, &idea).await.unwrap();
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
    async fn p058_runs_get_attaches_operator_escalation_readback_and_non_operator_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let now = Utc::now();
        escalation::insert_ledger(
            &pool,
            &domain::escalation::EscalationLedger {
                id: "ledger-runs-get-p058".into(),
                run_id,
                stage_id: "state_3".into(),
                agent_id: "code_writer".into(),
                policy_id: "policy-runs-get".into(),
                policy_hash: "sha256:runs-get".into(),
                status_raw: "paused".into(),
                current_tier_id: None,
                current_tier_kind_raw: None,
                chain_attempt_index: 1,
                trigger_raw: Some("contract_output_failure".into()),
                pause_reason_raw: Some("escalation_chain_exhausted".into()),
                operator_action_hint: None,
                runbook_anchor: None,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let operator = execute(
            "runs.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &auth::Principal::new("operator-p058", auth::PrincipalClass::Operator),
        )
        .await
        .unwrap();
        assert!(
            operator["escalation_readback"]["chains"].is_array(),
            "Operator runs.get must include full escalation_readback: {operator:?}"
        );

        let agent = execute(
            "runs.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &auth::Principal::new("agent-p058", auth::PrincipalClass::Agent),
        )
        .await
        .unwrap();
        assert_eq!(agent["escalation_readback"]["chains_redacted"], true);
        assert_eq!(agent["escalation_readback"]["paused_chain_count"], 1);
        assert!(
            agent["escalation_readback"]
                .get("dominant_pause_reason_raw")
                .is_none(),
            "non-Operator escalation summary must not leak raw pause reason: {agent:?}"
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
    async fn runs_get_includes_implementation_self_assessment_summary() {
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
            "runs.get",
            serde_json::json!({"run_id": run.id.to_string()}),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        assert_eq!(
            result["implementation_self_assessment_summary"]["status"],
            serde_json::json!("blocked")
        );
    }

    #[tokio::test]
    async fn proposal_087_runs_list_is_projection_only_without_detail_attachments() {
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
        for forbidden in [
            "implementation_self_assessment_summary",
            "code_writer_completion_receipts",
            "rollout_contract_readback",
            "side_effect_readback",
        ] {
            assert!(
                item.get(forbidden).is_none(),
                "runs.list must stay projection-only and omit {forbidden}: {item}"
            );
        }
        assert!(
            item.get("implementationCompletion").is_some(),
            "runs.list keeps P088 compatibility via projection-backed compact summary"
        );
        assert_eq!(item["total_stages"], serde_json::json!(0));
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
}
