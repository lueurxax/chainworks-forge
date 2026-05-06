use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::{
    artifact_contracts, closeout, legacy_discovery_overrides, projections, rollout_contract_checks,
    runs,
};
use domain::commands::{
    CancelRunCmd, Command, KnowledgeCapsuleIgnoreCmd, MainSyncMode,
    MainSyncRecordRecoveryDecisionCmd, MainSyncRecoveryDecision, MainSyncRepairStateCmd,
    MainSyncRequestCmd, MainSyncRetryCmd, MainSyncSetRunOverrideCmd, MainSyncTriggerReason,
    ProposalGateSettlementAction, SettleProposalGateCmd, StartRunCmd,
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
                "required": ["idea_id", "workflow_id", "workflow_title", "workspace_root", "artifact_root", "workflow_yaml_path", "agent_catalog_yaml_path"],
                "properties": {
                    "idea_id": { "type": "string", "description": "ID of the idea" },
                    "workflow_id": { "type": "string" },
                    "workflow_title": { "type": "string" },
                    "workspace_root": { "type": "string" },
                    "artifact_root": { "type": "string" },
                    "workflow_yaml_path": { "type": "string", "description": "Path to workflow YAML file (enables state-machine execution)" },
                    "agent_catalog_yaml_path": { "type": "string", "description": "Path to agent catalog YAML file" },
                    "delivery_configuration_json": { "type": "string", "description": "Frozen delivery configuration JSON for repo-backed runs" },
                    "review_routing_json": { "type": "string", "description": "Review routing options JSON for P060 dynamic reviewer selection" },
                    "rollout_contract_preflight_policy_json": {
                        "type": "string",
                        "description": "P084 rollout-contract run-start policy request JSON. Accepts waiver and/or enforcement_mode objects; server stamps authorization, principal, and audit event."
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
                "required": ["run_id"],
                "properties": {
                    "run_id": { "type": "string" }
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
            let delivery_configuration_json = params["delivery_configuration_json"]
                .as_str()
                .map(String::from);
            let review_routing_json = params["review_routing_json"].as_str().map(String::from);
            let rollout_contract_preflight_policy_json = params
                ["rollout_contract_preflight_policy_json"]
                .as_str()
                .map(String::from);

            let caller = mcp_caller(&principal.id, &principal.class, "runs.start");
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
            attach_implementation_self_assessment_summary(pool, value).await
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
                        if let Some(projection) =
                            db::repos::artifact_contracts::find_run_state_projection(pool, run_id)
                                .await?
                        {
                            obj.insert(
                                "active_artifact_index".into(),
                                projection.active_index_json,
                            );
                            obj.insert("run_state_projection".into(), projection.run_state_json);
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
                    }
                    let value = attach_implementation_self_assessment_summary(pool, value).await?;
                    // P077 BLK-004: attach closeout_readiness_summary parity on runs.get.
                    attach_closeout_readiness_summary(pool, value).await
                }
                None => Ok(serde_json::Value::Null),
            }
        }

        "runs.list" => {
            let items = projections::list_active_projection(pool).await?;
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let value = attach_implementation_self_assessment_summary(
                    pool,
                    serde_json::to_value(&item)?,
                )
                .await?;
                // P077 BLK-004: attach closeout_readiness_summary parity on runs.list.
                values.push(attach_closeout_readiness_summary(pool, value).await?);
            }
            Ok(serde_json::Value::Array(values))
        }

        "runs.cancel" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let caller = mcp_caller(&principal.id, &principal.class, "runs.cancel");
            let cmd = Command::CancelRun(CancelRunCmd { run_id });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            Ok(serde_json::json!({
                "cancelled": true,
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
            let caller = mcp_caller(&principal.id, &principal.class, "runs.main_sync.request");
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
            let caller = mcp_caller(&principal.id, &principal.class, "runs.main_sync.retry");
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
            let caller = mcp_caller(
                &principal.id,
                &principal.class,
                "runs.main_sync.set_override",
            );
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
            let caller = mcp_caller(
                &principal.id,
                &principal.class,
                "runs.main_sync.repair_state",
            );
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
            let caller = mcp_caller(
                &principal.id,
                &principal.class,
                "runs.main_sync.record_recovery_decision",
            );
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
            let caller = mcp_caller(
                &principal.id,
                &principal.class,
                "runs.knowledge_capsule.ignore",
            );
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

            let caller = mcp_caller(&principal.id, &principal.class, "runs.settle_proposal_gate");
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
    let rollout_contract_readback = match run_id {
        Some(run_id) => rollout_contract_checks::find_terminal_rollout_contract_check_for_run(
            pool,
            run_id.inner(),
        )
        .await?
        .map(|check| check.operator_readback_json()),
        None => None,
    };

    if let Some(object) = value.as_object_mut() {
        object.insert(
            "implementation_self_assessment_summary".to_string(),
            summary.unwrap_or(serde_json::Value::Null),
        );
        object.insert(
            "rollout_contract_readback".to_string(),
            rollout_contract_readback.unwrap_or(serde_json::Value::Null),
        );
    }

    Ok(value)
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
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
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
        format!(
            "{}/../../../examples/workflows/workflow.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn test_agent_catalog_yaml_path() -> String {
        format!(
            "{}/../../../examples/agents/agents.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
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
                projection_integrity: ProjectionIntegrity::Valid,
                cutover_policy_revision: Some("p084-cutover-v1".into()),
                redaction_state: "bounded".into(),
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
        let params = serde_json::json!({
            "idea_id": idea_id.to_string(),
            "workflow_id": "wf-start",
            "workflow_title": "Start Run",
            "workspace_root": "/tmp/ws",
            "artifact_root": "/tmp/art",
            "workflow_yaml_path": test_workflow_yaml_path(),
            "agent_catalog_yaml_path": test_agent_catalog_yaml_path(),
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
