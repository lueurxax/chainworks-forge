use anyhow::Result;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;

use db::repos::{
    agent_execution_discovery_diagnostics, agent_execution_runtime_facts,
    agent_execution_runtime_receipts, agent_executions, artifact_contracts, artifacts, closeout,
    code_writer_completion_receipts, lead_conflict_mediations, legacy_discovery_overrides,
    retry_payload_recovery_events, retry_stage_execution_authorities, rollout_contract_checks,
    runs, sessions, validation, workflow_conflicts,
};
use db::write_class::WriteLane;
use db::writer::class_a_operation;
use domain::agent::{AgentExecution, AgentExecutionRuntimeFacts};
use domain::artifact::Artifact;
use domain::ids::RunId;
use domain::xcode_runtime::XcodeRuntimeObservation;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;

fn fresh_provider_process_for_disposition(disposition: Option<&str>) -> Option<bool> {
    match disposition {
        Some("reused") => Some(false),
        Some("fresh")
        | Some("reused_after_resume")
        | Some("fresh_after_reset")
        | Some("fresh_after_invalidation")
        | Some("fresh_after_budget")
        | Some("fresh_after_compaction")
        | Some("fresh_after_transport_error")
        | Some("fresh_after_timeout")
        | Some("fresh_session_required")
        | Some("unverifiable_session_history") => Some(true),
        Some(_) | None => None,
    }
}

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "reports.get".to_string(),
        description: "Get report and release artifact payloads for a run".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["run_id"],
            "properties": {
                "run_id": { "type": "string", "description": "The run ID to retrieve report artifacts for" }
            }
        }),
    }]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "reports.get" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;

            let all_artifacts = artifacts::list_by_run(pool, run_id).await?;
            let rollout_contract_readback = rollout_contract_readback_json(pool, run_id).await?;
            let mut reports = Vec::new();
            for artifact in all_artifacts.into_iter() {
                if artifact.report_kind.is_some() || is_release_report_artifact(&artifact.name) {
                    reports.push(
                        artifact_report_json(pool, &artifact, Some(&rollout_contract_readback))
                            .await?,
                    );
                }
            }
            let closeout_readiness_summary = closeout_readiness_summary_json(pool, run_id).await?;
            let code_writer_completion_receipts =
                code_writer_completion_receipts_json(pool, run_id).await?;
            let implementation_completion = implementation_completion_json(pool, run_id).await?;
            reports.push(serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "run_id": run_id.to_string(),
                "stage_id": "__run__",
                "agent_id": "system",
                "name": "mcp_execution_truth",
                "contract_id": "mcp_execution_truth",
                "format": "json",
                "artifact_metadata_pointer": serde_json::Value::Null,
                "checksum_sha256": serde_json::Value::Null,
                "size_bytes": serde_json::Value::Null,
                "provider": "system",
                "model": serde_json::Value::Null,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "is_pinned": false,
                "report_kind": "mcp_execution_truth",
                "report_version": 1,
                "agent_executions": execution_mcp_truth_json(
                    pool,
                    run_id,
                    principal.class == auth::PrincipalClass::Operator,
                )
                .await?,
                "code_writer_completion_receipts": code_writer_completion_receipts,
                "implementationCompletion": implementation_completion,
                "workflow_conflict": workflow_conflict_json(pool, cmd_handler, run_id).await?,
                "retryAuthority": retry_authority_current_json(pool, run_id).await?,
                "retryAuthorityHistory": retry_authority_history_json(pool, run_id).await?,
                "p091OrphanRepairReadback": p091_orphan_repair_readback_json(pool, run_id).await?,
                "implementation_handoff_status": implementation_handoff_status_json(pool, run_id).await?,
                "implementation_self_assessment_summary": implementation_self_assessment_summary_json(pool, run_id).await?,
                "rollout_contract_readback": rollout_contract_readback,
                // P080: top-level reconciliation section per proposal §8.1 placement.
                "p080_reconciliation": db::repos::p080::p080_run_report_section_for_report(pool, &run_id.to_string()).await,
                "implementation_closeout_readiness_summary": closeout_readiness_summary.clone(),
                "closeout_readiness_summary": closeout_readiness_summary,
            }));
            if let Some(projection) =
                db::repos::artifact_contracts::find_run_state_projection(pool, run_id).await?
            {
                let overrides = db::repos::artifact_contracts::list_overrides(pool, run_id).await?;
                reports.push(serde_json::json!({
                    "id": uuid::Uuid::new_v4().to_string(),
                    "run_id": run_id.to_string(),
                    "stage_id": "__run__",
                    "agent_id": "system",
                    "name": "canonical_artifact_contracts",
                    "contract_id": "canonical_artifact_contracts",
                    "format": "json",
                    "artifact_metadata_pointer": {
                        "schemaVersion": "artifact_metadata_pointer.v1",
                        "artifactId": "canonical_artifact_contracts",
                        "checksumSha256": serde_json::Value::Null,
                        "sizeBytes": serde_json::Value::Null,
                        "authorizedPayloadRoute": serde_json::Value::Null,
                        "payloadPathRedacted": true,
                        "forbiddenFields": ["absolutePath", "filesystemPath", "rawPayload"]
                    },
                    "checksum_sha256": serde_json::Value::Null,
                    "size_bytes": serde_json::Value::Null,
                    "provider": "system",
                    "model": serde_json::Value::Null,
                    "created_at": projection.updated_at.to_rfc3339(),
                    "is_pinned": true,
                    "report_kind": "canonical_artifact_contracts",
                    "report_version": 1,
                    "active_index": projection.active_index_json,
                    "run_state_projection": projection.run_state_json,
                    "operator_overrides": overrides,
                    "legacy_discovery_overrides": legacy_discovery_overrides::list_by_run(pool, run_id).await?,
                }));
            }

            Ok(serde_json::Value::Array(reports))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

pub(crate) async fn retry_authority_history_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let events = retry_payload_recovery_events::latest_by_authority_for_run(pool, run_id).await?;
    let mut values = retry_stage_execution_authorities::list_by_run(pool, run_id)
        .await?
        .into_iter()
        .map(|authority| {
            let mut value = serde_json::to_value(&authority)?;
            if let Some(event) = events.get(&authority.id) {
                let readback = event.readback_json();
                value["retryPayloadRecovery"] = readback.clone();
                value["retry_payload_recovery"] = readback;
            }
            Ok::<_, anyhow::Error>(value)
        })
        .collect::<Result<Vec<_>>>()?;
    for event in retry_payload_recovery_events::list_by_run(pool, run_id).await? {
        if event.retry_authority_id.is_none() {
            let readback = event.readback_json();
            values.push(serde_json::json!({
                "schema_version": "retry_payload_recovery_history_v1",
                "authority_state": "missing_authority",
                "run_id": event.run_id.to_string(),
                "source_invoke_work_item_id": event.invoke_work_item_id,
                "retryPayloadRecovery": readback.clone(),
                "retry_payload_recovery": readback,
            }));
        }
    }
    Ok(serde_json::Value::Array(values))
}

pub(crate) async fn retry_authority_current_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let events = retry_payload_recovery_events::latest_by_authority_for_run(pool, run_id).await?;
    let history = retry_stage_execution_authorities::list_by_run(pool, run_id).await?;
    let Some(authority) = history
        .into_iter()
        .find(|authority| authority.authority_state.to_string() == "active")
    else {
        return Ok(serde_json::Value::Null);
    };
    let mut value = serde_json::to_value(&authority)?;
    if let Some(event) = events.get(&authority.id) {
        let readback = event.readback_json();
        value["retryPayloadRecovery"] = readback.clone();
        value["retry_payload_recovery"] = readback;
    }
    Ok(value)
}

pub(crate) async fn p091_orphan_repair_readback_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let row = sqlx::query(
        r#"SELECT mode, disabled, candidates_total, excluded_total,
                  would_repair_total, repaired_total, disabled_total,
                  bounded_samples_json, created_at
           FROM p091_orphan_repair_passes
           WHERE run_id IS NULL OR run_id = ?1
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await?;
    let operator_disabled = std::env::var("CHAINWORKS_P091_DISABLE_STARTUP_ORPHAN_REPAIR")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let configured_mode = std::env::var("CHAINWORKS_P091_STARTUP_ORPHAN_REPAIR_MODE")
        .unwrap_or_else(|_| "diagnostic".to_string());
    if let Some(row) = row {
        let samples_raw: Option<String> = row.get("bounded_samples_json");
        let samples = samples_raw
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .unwrap_or_else(|| serde_json::json!([]));
        Ok(serde_json::json!({
            "configured_mode": configured_mode,
            "operator_disabled": operator_disabled,
            "latest_pass": {
                "mode": row.get::<String, _>("mode"),
                "disabled": row.get::<i64, _>("disabled") != 0,
                "candidates_total": row.get::<i64, _>("candidates_total"),
                "excluded_total": row.get::<i64, _>("excluded_total"),
                "would_repair_total": row.get::<i64, _>("would_repair_total"),
                "repaired_total": row.get::<i64, _>("repaired_total"),
                "disabled_total": row.get::<i64, _>("disabled_total"),
                "bounded_samples": samples,
                "created_at": row.get::<String, _>("created_at"),
            }
        }))
    } else {
        Ok(serde_json::json!({
            "configured_mode": configured_mode,
            "operator_disabled": operator_disabled,
            "latest_pass": null,
        }))
    }
}

pub(crate) async fn workflow_conflict_json(
    pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    run_id: RunId,
) -> Result<serde_json::Value> {
    match workflow_conflicts::get_current_blocking_conflict(pool, run_id).await? {
        Some(conflict) => {
            let conflict_id = conflict.conflict_id.clone();
            let mediation_id = conflict.mediation_record_id.clone();
            let suggested_operator_action =
                domain::workflow_conflict::workflow_conflict_suggested_operator_action(&conflict)
                    .map(str::to_string);
            let mut value = serde_json::to_value(conflict)?;
            if let Some(object) = value.as_object_mut() {
                if let Some(action) = suggested_operator_action {
                    object.insert(
                        "suggested_operator_action".into(),
                        serde_json::Value::String(action),
                    );
                }
                let lead_mediation = match mediation_id {
                    Some(id) => lead_mediation_readback_json(pool, &id)
                        .await?
                        .unwrap_or(serde_json::Value::Null),
                    None => serde_json::Value::Null,
                };
                object.insert("lead_mediation".into(), lead_mediation);
            }
            // OPS-002 (P017 R4): emit report_readback_completeness with the
            // ratio of expected→present fields so dashboards can flag
            // partial readbacks without parsing the full payload.
            //
            // Expected fields are the proposal's "current conflict, history,
            // advisory rejections, lead owner, valid action class, and
            // terminal failure reason" set, as listed in the proposal's
            // operational metrics block.
            let expected: &[&str] = &[
                "conflict_id",
                "current_state_id",
                "reason",
                "status",
                "candidate_transitions",
                "operator_label",
                "lead_agent_id",
                "mediation_record_id",
                "terminal_failure_reason",
                "diagnostic_redaction_tier",
            ];
            let present: Vec<&str> = expected
                .iter()
                .copied()
                .filter(|key| value.get(*key).map(|v| !v.is_null()).unwrap_or(false))
                .collect();
            let now = chrono::Utc::now();
            let db_writer = cmd_handler.db_writer();
            let mut tx = db_writer
                .begin_immediate_transaction(
                    class_a_operation(
                        "mcp.reports.record_workflow_conflict_readback_completeness",
                        WriteLane::CriticalBarrier,
                        format!(
                            "mcp.reports.record_workflow_conflict_readback_completeness:{}",
                            conflict_id
                        ),
                    ),
                    "mcp.reports.record_workflow_conflict_readback_completeness",
                )
                .await?;
            let _ = workflow_conflicts::record_report_readback_completeness_tx(
                &mut tx,
                &run_id.to_string(),
                Some(&conflict_id),
                expected,
                &present,
                "mcp.workflow_conflict_json",
                now,
            )
            .await;
            let _ = tx.commit().await;
            Ok(value)
        }
        None => Ok(serde_json::Value::Null),
    }
}

async fn lead_mediation_readback_json(
    pool: &SqlitePool,
    mediation_id: &str,
) -> Result<Option<serde_json::Value>> {
    let Some(record) = lead_conflict_mediations::find_by_id(pool, mediation_id).await? else {
        return Ok(None);
    };
    let resolution_mode = domain::mediation::derive_resolution_mode(&record);
    let validation_errors = record
        .validation_errors_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
    let cost_summary = record
        .cost_summary_json
        .as_deref()
        .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());

    // API-001 (P017 R2 audit): expose conflict-scoped execution attempts so
    // operators and agents can inspect the mediation-owned `AgentExecution`
    // through the workflow conflict surface that P017 designates as
    // authoritative. Without this, runtime facts, transcript refs,
    // watchdog outcome, and cost are not grouped with the conflict.
    let execution_attempts = mediation_execution_attempts_json(pool, mediation_id, &record).await?;
    let attempt_count = execution_attempts.len();

    // The synthetic single status_updates entry below is preserved for
    // backward compatibility with existing readback consumers, but
    // `attempt_number` now reflects the durable count of mediation-owned
    // execution rows (was hard-coded to `1`).
    let attempt_number = if attempt_count == 0 {
        1
    } else {
        attempt_count as i64
    };

    Ok(Some(serde_json::json!({
        "id": record.id,
        "conflict_id": record.conflict_id,
        "lead_agent_id": record.lead_agent_id,
        "status": record.status.to_string(),
        "resolution_mode": resolution_mode,
        "chosen_action": record.chosen_action,
        "chosen_next_state_id": record.chosen_next_state_id,
        "chosen_next_state_label": record.chosen_next_state_label,
        "sanitized_progress": record.sanitized_progress.clone(),
        "status_updates": [{
            "status": record.status.to_string(),
            "sanitized_progress": record.sanitized_progress.clone(),
            "updated_at": record.updated_at.to_rfc3339(),
            "attempt_number": attempt_number,
        }],
        "validation_errors": validation_errors,
        "confirmation_subject_id": record.confirmation_subject_id,
        "superseded_by_event_ref": record.superseded_by_event_ref,
        "cost_summary": cost_summary,
        "execution_attempts": execution_attempts,
    })))
}

/// Build the `execution_attempts` array for a mediation: one entry per
/// mediation-owned `agent_executions` row, sorted by `started_at`.
///
/// Each entry preserves owner identity, the nullable `stage_execution_id`,
/// timing, status, runtime facts summary, watchdog outcome, transcript
/// refs, artifact refs, and cost — the fields P017 commits to.
///
/// Notes on field availability (post-R4 / API-002 closure):
/// - `cost` is now populated from
///   `agent_executions.{total_cost_cents, input_tokens, output_tokens, cached_input_tokens}`,
///   stamped by the executor's mediation-completion path via
///   `agent_executions::update_attempt_attribution`. Null only when the
///   provider returned no usage data for the attempt.
/// - `transcript_ref` is now populated from
///   `agent_executions.transcript_artifact_id`, which the executor sets
///   inline on the mediation completion path before this readback is
///   composed.
/// - `artifacts` first lists the direct transcript artifact, then any
///   artifacts linked through `artifact_source_generation_claims` to
///   this mediation-owned `AgentExecution` (P058 owner-aware claims),
///   and finally falls back to the run-level filter by `agent_id` for
///   pre-P017-R4 attempts that have no direct linkage yet.
///
/// `operator_rationale` and other non-sanitized fields are deliberately
/// omitted; artifacts are referenced by file path only, never inlined,
/// so the readback stays sanitized.
async fn mediation_execution_attempts_json(
    pool: &SqlitePool,
    mediation_id: &str,
    record: &domain::mediation::LeadConflictMediationRecord,
) -> Result<Vec<serde_json::Value>> {
    let executions = agent_executions::list_by_mediation_id(pool, mediation_id).await?;

    // Best-effort artifact correlation by agent_id. Parsing run_id from the
    // mediation record (string form) keeps this query scoped.
    let run_artifacts = match record.run_id.parse::<RunId>() {
        Ok(run_id) => artifacts::list_by_run(pool, run_id)
            .await
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    let mut attempts = Vec::with_capacity(executions.len());
    for (idx, execution) in executions.iter().enumerate() {
        let attempt_number = (idx + 1) as i64;
        let runtime_facts =
            agent_execution_runtime_facts::find_by_execution_id(pool, execution.id).await?;
        let runtime_receipt =
            agent_execution_runtime_receipts::find_by_execution_id(pool, execution.id).await?;

        let runtime_facts_summary = runtime_facts
            .as_ref()
            .map(|f| {
                serde_json::json!({
                    "valid_required_outputs": f.valid_required_outputs,
                    "failure_kind": f.failure_kind.as_ref().map(|k| k.to_string()),
                    "failure_message_redacted": f.failure_message_redacted,
                    "output_settlement": format!("{:?}", f.output_settlement).to_lowercase(),
                    "late_output_count": f.late_output_count,
                    "ignored_late_output_count": f.ignored_late_output_count,
                    "operator_action_hint": f.operator_action_hint.as_ref().map(|h| format!("{:?}", h).to_lowercase()),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let runtime_receipt_summary = runtime_receipt
            .as_ref()
            .map(|receipt| {
                serde_json::from_str::<serde_json::Value>(&receipt.receipt_json).unwrap_or_else(
                    |error| {
                        serde_json::json!({
                            "parse_error": error.to_string(),
                            "raw_receipt_available": true,
                        })
                    },
                )
            })
            .unwrap_or(serde_json::Value::Null);

        // Watchdog summary derived from the supervision classification + provider exit
        // status + transport error code. The runtime facts row doesn't have a
        // dedicated watchdog struct yet; this groups the relevant signals.
        let watchdog = runtime_facts
            .as_ref()
            .map(|f| {
                serde_json::json!({
                    "supervision_classification": f.supervision_classification,
                    "provider_exit_status": f.provider_exit_status,
                    "transport_error_code": f.transport_error_code,
                    "retry_after": f.retry_after.map(|t| t.to_rfc3339()),
                })
            })
            .unwrap_or(serde_json::Value::Null);

        // P017 R5 / API-003: attempt artifacts in three tiers.
        //  Tier 1: direct transcript artifact via FK on agent_executions.
        //  Tier 2: direct execution-attempt FK on artifacts (R5 canonical
        //          path) — retries by the same lead agent surface only
        //          their own artifacts (cross-retry isolation).
        //  Tier 3: legacy `agent_id` correlation, used only as a fallback
        //          for attempts that have no direct linkage at all
        //          (pre-R5 rows). Skipping tier 3 when any direct
        //          linkage exists is the cross-retry isolation guarantee.
        // ID dedup across tiers keeps the array clean.
        let mut seen_artifact_ids: std::collections::HashSet<String> = Default::default();
        let mut attempt_artifacts: Vec<serde_json::Value> = Vec::new();

        // Tier 1: transcript artifact (direct FK link).
        let transcript_artifact = if let Some(ref tid) = execution.transcript_artifact_id {
            if let Ok(parsed_id) = tid.parse::<domain::ids::ArtifactId>() {
                let found = artifacts::find_by_id(pool, parsed_id).await.ok().flatten();
                if let Some(ref a) = found {
                    seen_artifact_ids.insert(a.id.to_string());
                    attempt_artifacts.push(public_artifact_ref(a, "transcript_direct"));
                }
                found
            } else {
                None
            }
        } else {
            None
        };

        // Tier 2: direct execution-attempt FK linkage.
        let direct_artifacts = artifacts::list_by_agent_execution(pool, &execution.id.to_string())
            .await
            .unwrap_or_default();
        let attempt_has_direct_link = !direct_artifacts.is_empty();
        for a in direct_artifacts.iter() {
            if !seen_artifact_ids.insert(a.id.to_string()) {
                continue;
            }
            attempt_artifacts.push(public_artifact_ref(a, "execution_id_direct"));
        }

        // Tier 3: legacy `agent_id` correlation. Only reachable when
        // neither transcript nor direct linkage exists for this attempt.
        if !attempt_has_direct_link && transcript_artifact.is_none() {
            for a in run_artifacts.iter() {
                if !seen_artifact_ids.insert(a.id.to_string()) {
                    continue;
                }
                if a.agent_id != execution.agent_id {
                    continue;
                }
                attempt_artifacts.push(public_artifact_ref(a, "agent_id_correlation"));
            }
        }

        attempts.push(serde_json::json!({
            "agent_execution_id": execution.id.to_string(),
            "owner_kind": execution.owner_kind.clone()
                .unwrap_or_else(|| "lead_conflict_mediation".to_string()),
            "owner_id": execution.owner_id.clone()
                .unwrap_or_else(|| record.id.clone()),
            "mediation_record_id": record.id.clone(),
            "stage_execution_id": execution.stage_execution_id.map(|id| id.to_string()),
            "agent_id": execution.agent_id,
            "provider": execution.provider,
            "model": execution.model,
            "status": execution.status.to_string(),
            "started_at": execution.started_at.to_rfc3339(),
            "completed_at": execution.completed_at.map(|t| t.to_rfc3339()),
            "attempt_number": attempt_number,
            "runtime_facts": runtime_facts_summary,
            "runtime_receipt": runtime_receipt_summary,
            "watchdog": watchdog,
            // P017 R4 / API-002: cost + transcript_ref are now populated
            // from per-execution columns when the provider reported
            // usage data and the executor persisted a transcript.
            "cost": match (
                execution.total_cost_cents,
                execution.input_tokens,
                execution.output_tokens,
                execution.cached_input_tokens,
            ) {
                (None, None, None, None) => serde_json::Value::Null,
                (cents, input, output, cached) => serde_json::json!({
                    "total_cost_cents": cents,
                    "input_tokens": input,
                    "output_tokens": output,
                    "cached_input_tokens": cached,
                }),
            },
            "transcript_ref": match transcript_artifact.as_ref() {
                Some(a) => serde_json::json!({
                    "artifact_id": a.id.to_string(),
                    "artifact_metadata_pointer": artifact_metadata_pointer(a),
                    "format": format!("{:?}", a.format).to_lowercase(),
                }),
                None => serde_json::Value::Null,
            },
            "artifacts": attempt_artifacts,
        }));
    }
    Ok(attempts)
}

pub(crate) async fn implementation_handoff_status_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    match workflow_conflicts::get_implementation_handoff_status(pool, run_id).await? {
        Some(status) => Ok(serde_json::to_value(status)?),
        None => Ok(serde_json::Value::Null),
    }
}

pub(crate) async fn execution_mcp_truth_json(
    pool: &SqlitePool,
    run_id: RunId,
    include_operator_debug: bool,
) -> Result<serde_json::Value> {
    let executions = agent_executions::list_by_run(pool, run_id).await?;
    let runtime_facts = agent_execution_runtime_facts::list_by_run(pool, run_id).await?;
    let runtime_facts_by_execution_id: HashMap<_, _> = runtime_facts
        .into_iter()
        .map(|facts| {
            let execution_id = facts.agent_execution_id.to_string();
            (execution_id, facts)
        })
        .collect::<HashMap<_, _>>();
    let runtime_receipts = agent_execution_runtime_receipts::list_by_run(pool, run_id).await?;
    let runtime_receipts_by_execution_id: HashMap<_, _> = runtime_receipts
        .into_iter()
        .map(|receipt| (receipt.agent_execution_id.to_string(), receipt))
        .collect::<HashMap<_, _>>();
    let discovery_diagnostics =
        agent_execution_discovery_diagnostics::list_readback_by_run(pool, run_id).await?;
    let discovery_diagnostics_by_execution_id: HashMap<_, _> = discovery_diagnostics
        .into_iter()
        .map(|readback| (readback.diagnostics.agent_execution_id.clone(), readback))
        .collect::<HashMap<_, _>>();
    let completion_receipts = code_writer_completion_receipts::list_by_run(pool, run_id).await?;
    let completion_receipts_by_execution_id: HashMap<_, _> = completion_receipts
        .iter()
        .map(|readback| (readback.receipt.agent_execution_id.to_string(), readback))
        .collect::<HashMap<_, _>>();
    let mut items = Vec::with_capacity(executions.len());
    for execution in executions.into_iter() {
        let execution_id = execution.id.to_string();
        let discovery_diagnostics_readback =
            discovery_diagnostics_by_execution_id.get(&execution_id);
        let reconciliation_pending =
            discovery_diagnostics_readback.is_some_and(|readback| readback.reconciliation_pending);
        let runtime_facts = match runtime_facts_by_execution_id.get(&execution_id) {
            Some(facts) => {
                let mut facts = facts.clone();
                if reconciliation_pending {
                    facts.valid_required_outputs = false;
                }
                runtime_facts_json(
                    pool,
                    &execution,
                    &facts,
                    runtime_receipts_by_execution_id.get(&execution_id),
                    include_operator_debug,
                )
                .await?
            }
            None => {
                let mut facts =
                    AgentExecutionRuntimeFacts::defaults_for(execution.id, chrono::Utc::now());
                if reconciliation_pending {
                    facts.valid_required_outputs = false;
                }
                runtime_facts_json(
                    pool,
                    &execution,
                    &facts,
                    runtime_receipts_by_execution_id.get(&execution_id),
                    include_operator_debug,
                )
                .await?
            }
        };
        let discovery_diagnostics = match discovery_diagnostics_readback {
            Some(readback) => {
                let diagnostics = &readback.diagnostics;
                serde_json::json!({
                "agent_execution_id": diagnostics.agent_execution_id.clone(),
                "discovery_schema_version": diagnostics.discovery_schema_version.clone(),
                "legacy_broad_discovery_used": diagnostics.legacy_broad_discovery_used,
                "missing_required_output_count": diagnostics.missing_required_output_count,
                "rejected_output_count": diagnostics.rejected_output_count,
                "stale_output_count": diagnostics.stale_output_count,
                "meta_discovery_truncated": diagnostics.meta_discovery_truncated,
                "git_manifest_status": diagnostics.git_manifest_status.clone(),
                "resume_warning_count": diagnostics.resume_warning_count,
                "reconciliation_pending": readback.reconciliation_pending,
                "reconciliation_warnings": readback.reconciliation_warnings.clone(),
                "runtime_facts_present": readback.runtime_facts_present,
                "matching_active_artifact_generation_count": readback.matching_active_artifact_generation_count,
                "payload": readback.projected_payload(),
                "created_at": diagnostics.created_at.to_rfc3339(),
                "updated_at": diagnostics.updated_at.to_rfc3339(),
                })
            }
            None => serde_json::Value::Null,
        };
        items.push(serde_json::json!({
            "agent_execution_id": execution_id,
            "stage_execution_id": execution.stage_execution_id.map(|id| id.to_string()),
            "agent_id": execution.agent_id,
            "provider": execution.provider,
            "model": execution.model,
            "status": execution.status.to_string(),
            "backend_profile_id": execution.backend_profile_id,
            "requested_mcp_extensions_json": execution.requested_mcp_extensions_json,
            "predicted_mcp_extensions_json": execution.predicted_mcp_extensions_json,
            "predicted_mcp_runtime_ids_json": execution.predicted_mcp_runtime_ids_json,
            "actual_mcp_extensions_json": execution.actual_mcp_extensions_json,
            "actual_mcp_runtime_ids_json": execution.actual_mcp_runtime_ids_json,
            "denied_mcp_extensions_json": execution.denied_mcp_extensions_json,
            "mcp_blocking_issues_json": execution.mcp_blocking_issues_json,
            "actual_mcp_observation_json": execution.actual_mcp_observation_json,
            "xcode_runtime_observation": xcode_runtime_observation_json(
                execution.actual_xcode_runtime_observation_json.as_deref()
            ),
            "mcp_session_startup_latency_ms": execution.mcp_session_startup_latency_ms,
            "runtime_facts": runtime_facts,
            "discovery_diagnostics": discovery_diagnostics,
            "code_writer_completion_receipt": completion_receipts_by_execution_id
                .get(&execution_id)
                .map(|readback| {
                    serde_json::to_value(domain::code_writer_completion::project_implementation_completion(
                        std::slice::from_ref(*readback),
                    ))
                })
                .transpose()?
                .unwrap_or(serde_json::Value::Null),
            // P066: toolchain mapping diagnostics — always non-null, legacy rows synthesized.
            "actual_toolchain_mapping_diagnostics": toolchain_mapping_diagnostics_mcp(
                execution.actual_toolchain_mapping_diagnostics_json.as_deref()
            ),
        }));
    }
    Ok(serde_json::Value::Array(items))
}

pub(crate) async fn code_writer_completion_receipts_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let receipts = code_writer_completion_receipts::list_by_run(pool, run_id).await?;
    Ok(serde_json::Value::Array(
        receipts
            .into_iter()
            .map(serde_json::to_value)
            .collect::<serde_json::Result<Vec<_>>>()?,
    ))
}

pub(crate) async fn implementation_completion_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let receipts = code_writer_completion_receipts::list_canonical_by_run(pool, run_id).await?;
    Ok(serde_json::to_value(
        domain::code_writer_completion::project_implementation_completion(&receipts),
    )?)
}

/// P066: Build the MCP-surface toolchain mapping diagnostics value.
/// Synthesizes a legacy_row_unavailable sentinel when the column is NULL.
/// Absolute paths are never exposed.
fn toolchain_mapping_diagnostics_mcp(raw: Option<&str>) -> serde_json::Value {
    let Some(raw) = raw else {
        return serde_json::json!({
            "mapping_state": "legacy_row_unavailable",
            "mapping_enabled": false,
            "inactive_reason": "legacy_row",
            "policy_source": "synthesized_legacy",
            "version": 1,
        });
    };
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(val) => {
            // Return a filtered subset — no absolute paths.
            serde_json::json!({
                "mapping_state": val.get("mapping_state"),
                "mapping_enabled": val.get("mapping_enabled"),
                "inactive_reason": val.get("inactive_reason"),
                "policy_source": val.get("policy_source"),
                "policy_version": val.get("policy_version"),
                "provider_family": val.get("provider_family"),
                "version": val.get("version"),
            })
        }
        Err(_) => serde_json::json!({
            "mapping_state": "legacy_row_unavailable",
            "mapping_enabled": false,
            "inactive_reason": "legacy_row",
            "policy_source": "synthesized_legacy",
            "version": 1,
        }),
    }
}

fn xcode_runtime_observation_json(raw: Option<&str>) -> serde_json::Value {
    let Some(raw) = raw else {
        return serde_json::Value::Null;
    };
    match serde_json::from_str::<XcodeRuntimeObservation>(raw) {
        Ok(observation) => serde_json::to_value(observation.redacted_for_surface())
            .unwrap_or(serde_json::Value::Null),
        Err(error) => serde_json::json!({
            "parse_error": error.to_string(),
            "raw_observation_available": true,
        }),
    }
}

async fn runtime_facts_json(
    pool: &SqlitePool,
    execution: &AgentExecution,
    facts: &AgentExecutionRuntimeFacts,
    runtime_receipt: Option<&domain::agent::AgentExecutionRuntimeReceiptRecord>,
    include_operator_debug: bool,
) -> Result<serde_json::Value> {
    let lineage = match execution.session_lineage_id.as_deref() {
        Some(lineage_id) => sessions::find_lineage_by_id(pool, lineage_id).await?,
        None => None,
    };
    let generation = match execution.session_generation_id.as_deref() {
        Some(generation_id) => sessions::find_generation_by_id(pool, generation_id).await?,
        None => None,
    };
    let provider_session_id = generation
        .as_ref()
        .and_then(|generation| generation.provider_session_id.clone());
    let generation_status = generation.as_ref().map(|generation| {
        sessions::session_generation_status_to_str(&generation.status).to_string()
    });
    let active_session_generation_id = lineage
        .as_ref()
        .and_then(|lineage| lineage.active_generation_id.clone());
    let active_generation_matches_execution =
        match (lineage.as_ref(), execution.session_generation_id.as_deref()) {
            (Some(lineage), Some(execution_generation_id)) => {
                Some(lineage.active_generation_id.as_deref() == Some(execution_generation_id))
            }
            _ => None,
        };
    let runtime_receipt = runtime_receipt
        .map(|receipt| {
            serde_json::from_str::<serde_json::Value>(&receipt.receipt_json).unwrap_or_else(
                |error| {
                    serde_json::json!({
                        "parse_error": error.to_string(),
                        "raw_receipt_available": true,
                    })
                },
            )
        })
        .unwrap_or(serde_json::Value::Null);
    Ok(serde_json::json!({
        "agent_execution_id": facts.agent_execution_id.to_string(),
        "failure_kind": facts.failure_kind.as_ref().map(ToString::to_string),
        "failure_kind_raw_debug": include_operator_debug.then(|| facts.failure_kind_raw_debug.clone()).flatten(),
        "failure_kind_version": facts.failure_kind_version,
        "failure_message_redacted": facts.failure_message_redacted.clone(),
        "failure_message_redaction_version": facts.failure_message_redaction_version,
        "retry_after": facts.retry_after.as_ref().map(|dt| dt.to_rfc3339()),
        "operator_action_hint": facts.operator_action_hint.as_ref().map(ToString::to_string),
        "provider_exit_status": facts.provider_exit_status,
        "transport_error_code": facts.transport_error_code.clone(),
        "supervision_classification": facts.supervision_classification.clone(),
        "output_settlement": facts.output_settlement.to_string(),
        "valid_required_outputs": facts.valid_required_outputs,
        "late_output_count": facts.late_output_count,
        "ignored_late_output_count": facts.ignored_late_output_count,
        "session_lineage_id": execution.session_lineage_id.clone(),
        "session_generation_id": execution.session_generation_id.clone(),
        "invocation_owner_key": execution.invocation_owner_key.clone(),
        "session_reuse_scope": execution.session_reuse_scope.clone(),
        "session_family_id": execution.session_family_id.clone(),
        "session_reuse_disposition": execution.session_reuse_disposition.clone(),
        "session_reuse_reason": facts.session_reuse_reason.clone(),
        "session_reset_reason": execution.session_reset_reason.clone(),
        "provider_session_id": provider_session_id,
        "active_session_generation_id": active_session_generation_id,
        "active_generation_matches_execution": active_generation_matches_execution,
        "generation_status": generation_status,
        "fresh_provider_process": fresh_provider_process_for_disposition(execution.session_reuse_disposition.as_deref()),
        "rehydrated_from_checkpoint_artifact_id": execution.rehydrated_from_checkpoint_artifact_id.clone(),
        "quota_ledger_id": facts.quota_ledger_id.clone(),
        "runtime_receipt": runtime_receipt,
        "created_at": facts.created_at.to_rfc3339(),
        "updated_at": facts.updated_at.to_rfc3339(),
    }))
}

fn is_release_report_artifact(name: &str) -> bool {
    matches!(
        name,
        "release_manifest"
            | "git_push_receipt"
            | "release_bundle_manifest"
            | "connect_upload_receipt"
            | "delivery_receipt"
    )
}

pub(crate) async fn implementation_self_assessment_summary_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
        .await?
        .map(|stored| {
            let mut summary = stored.summary;
            summary.artifact_path = public_artifact_path(&summary.artifact_path);
            serde_json::to_value(summary)
        })
        .transpose()
        .map(|summary| summary.unwrap_or(serde_json::Value::Null))
        .map_err(Into::into)
}

pub(crate) async fn rollout_contract_readback_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let base =
        rollout_contract_checks::find_terminal_rollout_contract_check_for_run(pool, run_id.inner())
            .await?
            .map(|check| check.operator_readback_json_for_lane("run_report"))
            .unwrap_or(serde_json::Value::Null);

    if base.is_null() {
        return Ok(base);
    }
    // Merge live P087 fields into the run_report readback lane.
    // P080 reconciliation is exposed as a separate top-level field (proposal §8.1 placement).
    let p087 = db::repos::storage_health::p087_rollout_readback_fields(pool).await;
    if let Some(base_obj) = base.as_object() {
        let mut merged = base_obj.clone();
        if let Some(p087_obj) = p087.as_object() {
            for (k, v) in p087_obj {
                merged.insert(k.clone(), v.clone());
            }
        }
        Ok(serde_json::Value::Object(merged))
    } else {
        Ok(base)
    }
}

/// P077: Serialize the active closeout readiness generation for MCP readback.
/// Routes through CloseoutReadinessSummaryAccessor (R14 §architecture.single_accessor).
/// Returns null when no active generation exists (run not yet at state_9 or gate not settled).
pub(crate) async fn closeout_readiness_summary_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let run_id_str = run_id.to_string();
    match closeout::load_closeout_readiness_summary(pool, &run_id_str).await? {
        Some(summary) => Ok(serde_json::to_value(&summary)?),
        None => Ok(serde_json::Value::Null),
    }
}

pub(crate) fn public_artifact_path(path: &str) -> String {
    if path.ends_with("implementation/self-assessment.json") {
        "implementation/self-assessment.json".to_string()
    } else {
        path.to_string()
    }
}

fn artifact_metadata_pointer_value(
    artifact_id: &str,
    checksum_sha256: Option<&str>,
    size_bytes: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": "artifact_metadata_pointer.v1",
        "artifactId": artifact_id,
        "checksumSha256": checksum_sha256,
        "sizeBytes": size_bytes,
        "authorizedPayloadRoute": format!("/artifacts/{artifact_id}/payload"),
        "payloadPathRedacted": true,
        "forbiddenFields": ["absolutePath", "filesystemPath", "rawPayload"]
    })
}

fn artifact_metadata_pointer(artifact: &Artifact) -> serde_json::Value {
    artifact_metadata_pointer_value(
        &artifact.id.to_string(),
        artifact.checksum_sha256.as_deref(),
        artifact.size_bytes,
    )
}

fn public_artifact_ref(artifact: &Artifact, linkage: &str) -> serde_json::Value {
    serde_json::json!({
        "id": artifact.id.to_string(),
        "name": artifact.name,
        "format": format!("{:?}", artifact.format).to_lowercase(),
        "artifact_metadata_pointer": artifact_metadata_pointer(artifact),
        "report_kind": artifact.report_kind,
        "is_pinned": artifact.is_pinned,
        "linkage": linkage,
    })
}

pub(crate) fn public_artifact_index_row(
    row: &db::repos::projections::ArtifactIndexRow,
) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "run_id": row.run_id,
        "stage_id": row.stage_id,
        "agent_id": row.agent_id,
        "name": row.name,
        "contract_id": row.contract_id,
        "format": row.format,
        "artifact_metadata_pointer": artifact_metadata_pointer_value(
            &row.id,
            row.checksum_sha256.as_deref(),
            row.size_bytes,
        ),
        "checksum_sha256": row.checksum_sha256,
        "size_bytes": row.size_bytes,
        "provider": row.provider,
        "model": row.model,
        "created_at": row.created_at,
        "is_pinned": row.is_pinned,
        "report_kind": row.report_kind,
        "report_version": row.report_version,
        "artifact_generation_id": row.artifact_generation_id,
        "source_agent_execution_id": row.source_agent_execution_id,
        "source_stage_execution_id": row.source_stage_execution_id,
        "source_session_generation_id": row.source_session_generation_id,
        "source_work_item_id": row.source_work_item_id,
        "supersedes_artifact_generation_id": row.supersedes_artifact_generation_id,
        "output_settlement": row.output_settlement,
        "source_generation_verified": row.source_generation_verified,
    })
}

pub(crate) async fn artifact_report_json(
    pool: &SqlitePool,
    artifact: &Artifact,
    rollout_contract_readback: Option<&serde_json::Value>,
) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(artifact)?;
    if let serde_json::Value::Object(ref mut map) = value {
        map.remove("file_path");
        map.insert(
            "artifact_metadata_pointer".to_string(),
            artifact_metadata_pointer(artifact),
        );
    }
    let include_rollout_readback =
        artifact.report_kind.is_some() || is_release_report_artifact(&artifact.name);

    if include_rollout_readback {
        if let serde_json::Value::Object(ref mut map) = value {
            let readback = match rollout_contract_readback {
                Some(readback) => readback.clone(),
                None => rollout_contract_readback_json(pool, artifact.run_id).await?,
            };
            map.insert("rollout_contract_readback".to_string(), readback);
        }
    }

    if artifact.report_kind.as_deref() == Some("validation_failure") {
        if let Some(record) = validation::find_by_artifact_id(pool, artifact.id).await? {
            let agent_execution_id = record.agent_execution_id;
            let mut payload = ValidationFailureRecordPayload::from(record);
            if let Some(execution) =
                db::repos::agent_executions::find_by_id(pool, agent_execution_id).await?
            {
                payload.session_reuse_disposition = execution.session_reuse_disposition;
                payload.session_reset_reason = execution.session_reset_reason;
            }

            if let serde_json::Value::Object(ref mut map) = value {
                map.insert(
                    "validation_failure_record".to_string(),
                    serde_json::to_value(payload)?,
                );
            }
        } else if let serde_json::Value::Object(ref mut map) = value {
            map.insert(
                "validation_failure_record".to_string(),
                serde_json::Value::Null,
            );
        }
    }
    if artifact.report_kind.as_deref() == Some("failed_stage_evidence") {
        // SEC-P081: Enforce path containment under the run's artifact_root before
        // reading. artifact.file_path is daemon-written, but corrupted metadata or
        // a crafted DB row could redirect reads to arbitrary daemon-readable files.
        // We resolve the run's artifact_root from DB, canonicalize both paths, and
        // require the evidence file to reside strictly inside that root.
        const MAX_EVIDENCE_BYTES: u64 = 1_048_576; // 1 MiB
        let payload: Option<serde_json::Value> = async {
            let run = runs::find_by_id(pool, artifact.run_id).await?;
            let root_str = run
                .as_ref()
                .map(|r| r.artifact_root.trim())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!("artifact_root missing or empty"))?;
            let canonical_root = std::fs::canonicalize(root_str)
                .map_err(|e| anyhow::anyhow!("cannot canonicalize artifact_root: {e}"))?;
            let canonical_path = std::fs::canonicalize(&artifact.file_path)
                .map_err(|e| anyhow::anyhow!("cannot canonicalize file_path: {e}"))?;
            if !canonical_path.starts_with(&canonical_root) {
                anyhow::bail!("file_path escapes artifact_root containment");
            }
            let meta = std::fs::metadata(&canonical_path)?;
            if meta.len() > MAX_EVIDENCE_BYTES {
                anyhow::bail!("evidence file exceeds {} byte limit", MAX_EVIDENCE_BYTES);
            }
            let content = std::fs::read_to_string(&canonical_path)?;
            let v: serde_json::Value = serde_json::from_str(&content)?;
            Ok::<_, anyhow::Error>(v)
        }
        .await
        .ok();
        if let serde_json::Value::Object(ref mut map) = value {
            map.insert(
                "failed_stage_evidence".to_string(),
                payload.unwrap_or(serde_json::Value::Null),
            );
        }
    }

    Ok(value)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidationFailureRecordPayload {
    id: String,
    timestamp: String,
    #[serde(rename = "agentID")]
    agent_id: String,
    #[serde(rename = "stageID")]
    stage_id: String,
    #[serde(rename = "runID")]
    run_id: String,
    output_results: Vec<OutputValidationResultPayload>,
    failure_summary: String,
    failure_class: String,
    contract_metadata: Vec<ContractValidationMetadataPayload>,
    raw_output_exists: bool,
    receipt_exists: bool,
    transcript_exists: bool,
    recovery_recommendation: RecoveryRecommendationPayload,
    session_reuse_disposition: Option<String>,
    session_reset_reason: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputValidationResultPayload {
    output_name: String,
    #[serde(rename = "contractID")]
    contract_id: Option<String>,
    status: String,
    missing_fields: Vec<String>,
    validation_error: Option<String>,
    raw_payload_size: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractValidationMetadataPayload {
    output_name: String,
    #[serde(rename = "contractID")]
    contract_id: String,
    machine_format: String,
    validation_mode: String,
    required_field_count: i64,
    raw_artifact_name: Option<String>,
    normalized_artifact_name: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRecommendationPayload {
    action: String,
    explanation: String,
    #[serde(default)]
    source: Option<String>,
}

impl From<domain::validation::ValidationFailureRecord> for ValidationFailureRecordPayload {
    fn from(record: domain::validation::ValidationFailureRecord) -> Self {
        ValidationFailureRecordPayload {
            id: record.id,
            timestamp: record.timestamp.to_rfc3339(),
            agent_id: record.agent_id,
            stage_id: record.stage_id,
            run_id: record.run_id.to_string(),
            output_results: record
                .output_results
                .into_iter()
                .map(|output| OutputValidationResultPayload {
                    output_name: output.output_name,
                    contract_id: output.contract_id,
                    status: match output.status {
                        domain::validation::ValidationStatus::Passed => "passed".to_string(),
                        domain::validation::ValidationStatus::Failed => "failed".to_string(),
                        domain::validation::ValidationStatus::NoContractDeclared => {
                            "no_contract_declared".to_string()
                        }
                    },
                    missing_fields: output.missing_fields,
                    validation_error: output.validation_error,
                    raw_payload_size: output.raw_payload_size as i64,
                })
                .collect(),
            failure_summary: record.failure_summary,
            failure_class: match record.failure_class {
                domain::validation::ValidationFailureClass::OutputContractMismatch => {
                    "output_contract_mismatch".to_string()
                }
                domain::validation::ValidationFailureClass::NoOutputProduced => {
                    "no_output_produced".to_string()
                }
                domain::validation::ValidationFailureClass::EmptyOutput => {
                    "empty_output".to_string()
                }
                domain::validation::ValidationFailureClass::PersistenceFailure => {
                    "persistence_failure".to_string()
                }
            },
            contract_metadata: record
                .contract_metadata
                .into_iter()
                .map(|meta| ContractValidationMetadataPayload {
                    output_name: meta.output_name,
                    contract_id: meta.contract_id,
                    machine_format: meta.machine_format,
                    validation_mode: meta.validation_mode,
                    required_field_count: meta.required_field_count as i64,
                    raw_artifact_name: meta.raw_artifact_name,
                    normalized_artifact_name: meta.normalized_artifact_name,
                })
                .collect(),
            raw_output_exists: record.raw_output_exists,
            receipt_exists: record.receipt_exists,
            transcript_exists: record.transcript_exists,
            recovery_recommendation: RecoveryRecommendationPayload {
                action: record.recovery_recommendation.action,
                explanation: record.recovery_recommendation.explanation,
                source: None,
            },
            session_reuse_disposition: None,
            session_reset_reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{
        artifact_contracts, artifacts, ideas, rollout_contract_checks, runs, validation,
        workflow_conflicts,
    };
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::mediation::{LeadConflictMediationRecord, LeadMediationStatus};
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
    use domain::workflow_conflict::{
        candidate_transition_hash, workflow_conflict_fingerprint, CandidateTransitionEvaluation,
        CandidateTransitionResult, WorkflowConflictReason, WorkflowConflictRecord,
        WorkflowConflictStatus,
    };
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

    fn validation_failure_payload(run_id: RunId) -> serde_json::Value {
        serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "timestamp": "2026-04-15T09:30:00Z",
            "agentID": "validation_agent",
            "stageID": "stage_1",
            "runID": run_id.to_string(),
            "outputResults": [{
                "outputName": "report",
                "contractID": "report_v1",
                "status": "failed",
                "missingFields": ["summary"],
                "validationError": "Missing required fields: summary",
                "rawPayloadSize": 17
            }],
            "failureSummary": "report: Missing required fields: summary",
            "failureClass": "output_contract_mismatch",
            "contractMetadata": [{
                "outputName": "report",
                "contractID": "report_v1",
                "machineFormat": "json",
                "validationMode": "strict_structured",
                "requiredFieldCount": 1,
                "rawArtifactName": "report_raw",
                "normalizedArtifactName": "report"
            }],
            "rawOutputExists": true,
            "receiptExists": false,
            "transcriptExists": true,
            "recoveryRecommendation": {
                "action": "retry_failed_agent",
                "explanation": "Retry the agent with the same inputs.",
                "source": "runtime_policy"
            }
        })
    }

    fn validation_failure_record(
        artifact_id: ArtifactId,
        run_id: RunId,
        stage_execution_id: domain::ids::StageExecutionId,
        agent_execution_id: domain::ids::AgentExecutionId,
    ) -> ValidationFailureRecord {
        ValidationFailureRecord {
            id: "11111111-1111-1111-1111-111111111111".to_string(),
            artifact_id,
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-15T09:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            agent_id: "validation_agent".to_string(),
            stage_id: "stage_1".to_string(),
            stage_execution_id,
            agent_execution_id,
            run_id,
            output_results: vec![OutputValidationResult {
                output_name: "report".to_string(),
                contract_id: Some("report_v1".to_string()),
                status: ValidationStatus::Failed,
                missing_fields: vec!["summary"].into_iter().map(String::from).collect(),
                validation_error: Some("Missing required fields: summary".to_string()),
                raw_payload_size: 17,
            }],
            failure_summary: "report: Missing required fields: summary".to_string(),
            failure_class: ValidationFailureClass::OutputContractMismatch,
            contract_metadata: vec![ContractValidationMetadata {
                output_name: "report".to_string(),
                contract_id: "report_v1".to_string(),
                machine_format: "json".to_string(),
                validation_mode: "strict_structured".to_string(),
                required_field_count: 1,
                raw_artifact_name: Some("report_raw".to_string()),
                normalized_artifact_name: Some("report".to_string()),
            }],
            raw_output_exists: true,
            receipt_exists: false,
            transcript_exists: true,
            recovery_recommendation: RecoveryRecommendation {
                action: "retry_failed_agent".to_string(),
                explanation: "Retry the agent with the same inputs.".to_string(),
            },
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

    async fn seed_validation_attempt(
        pool: &sqlx::SqlitePool,
        run_id: RunId,
    ) -> (domain::ids::StageExecutionId, domain::ids::AgentExecutionId) {
        let stage_execution_id = domain::ids::StageExecutionId::new();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        db::repos::stages::insert(
            pool,
            &domain::stage::StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "stage_1".to_string(),
                label: "Stage 1".to_string(),
                status: domain::stage::StageStatus::Failed,
                iteration: 1,
                attempt_number: 1,
                settlement_kind: Some(domain::stage::StageSettlementKind::Failed),
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                owner_agent: Some("validation_agent".to_string()),
                provider: Some("system".to_string()),
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
        db::repos::agent_executions::insert(
            pool,
            &domain::agent::AgentExecution {
                id: agent_execution_id,
                stage_execution_id: Some(stage_execution_id),
                agent_id: "validation_agent".to_string(),
                provider: "system".to_string(),
                model: None,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                status: domain::agent::AgentStatus::Failed,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: Some("reused".into()),
                session_reset_reason: Some("operator_reset".into()),
                backend_profile_id: Some("codex_with_mcp".into()),
                requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                actual_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                actual_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                denied_mcp_extensions_json: Some("[]".into()),
                mcp_blocking_issues_json: Some("[]".into()),
                actual_mcp_observation_json: Some(
                    r#"{"source":"provider_session_new_response"}"#.into(),
                ),
                actual_xcode_runtime_observation_json: Some(
                    serde_json::json!({
                        "version": 1,
                        "mcp_broker_observations": [{
                            "source": "xcode_mcp_broker",
                            "backend_start_disposition": "lease_reserved",
                            "pool_id": "pool-1",
                            "lease_id": "lease-1",
                            "xcode_pid": "1234",
                            "backend_process_id": 5678,
                            "http_endpoint": "127.0.0.1:<redacted>",
                            "xcode_home_disposition": "host_user_home",
                            "xcode_tmpdir_disposition": "host_user_tmpdir",
                            "simulator_selection": {
                                "mode": "explicit_uuid",
                                "simulator_id": "SIM-1"
                            },
                            "sibling_leases_at_spawn": 1,
                            "backend_initialize_wait_ms": 42,
                            "backend_startup_latency_ms": 73,
                            "http_session_startup_latency_ms": 17,
                            "backend_failure_class": null,
                            "originating_execution_id": agent_execution_id.to_string(),
                            "prompt_cycle_index": 0,
                            "status_update": null
                        }],
                        "xcode_shim_events": [],
                        "xcode_host_executor_events": [],
                        "storage": {
                            "max_events": 1000,
                            "max_bytes": 1048576,
                            "truncated": false,
                            "total_events_dropped": 0,
                            "mcp_broker_observations_dropped": 0,
                            "xcode_shim_events_dropped": 0,
                            "xcode_host_executor_events_dropped": 0,
                            "corrupt_json_recovery_count": 0,
                            "corrupt_json_quarantined_bytes": 0
                        }
                    })
                    .to_string(),
                ),
                mcp_session_startup_latency_ms: Some(17),
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
                escalation_policy_id: None,
                escalation_policy_hash: None,
                escalation_tier_id: None,
                escalation_tier_kind_raw: None,
                escalation_trigger_raw: None,
                escalation_digest_version: None,
                escalation_ledger_id: None,
            },
        )
        .await
        .unwrap();
        (stage_execution_id, agent_execution_id)
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
    fn xcode_runtime_observation_readback_redacts_raw_stored_tokens() {
        let raw = serde_json::json!({
            "version": 1,
            "mcp_broker_observations": [{
                "source": "xcode_mcp_broker",
                "backend_start_disposition": "lease_reserved",
                "pool_id": "pool-1",
                "lease_id": "lease-1",
                "xcode_pid": "1234",
                "backend_process_id": 5678,
                "http_endpoint": "http://127.0.0.1:4000/xcode-mcp/lease-1?token=raw-report-token",
                "xcode_home_disposition": "host_user_home",
                "xcode_tmpdir_disposition": "host_user_tmpdir",
                "simulator_selection": null,
                "sibling_leases_at_spawn": 1,
                "backend_initialize_wait_ms": 42,
                "backend_startup_latency_ms": 73,
                "http_session_startup_latency_ms": 17,
                "backend_failure_class": null,
                "originating_execution_id": "execution-1",
                "prompt_cycle_index": 0,
                "status_update": "forwarded Bearer raw-report-bearer"
            }],
            "xcode_shim_events": [{
                "kind": "warning",
                "ts": "2026-04-21T12:00:00Z",
                "policy_reason": "residual_absolute_path",
                "source_field": "session_update",
                "matched_substring": "bearer_token=raw-report-warning-token",
                "excerpt": "provider mentioned xcode-lease-raw-report-shim-token"
            }],
            "xcode_host_executor_events": [{
                "ts": "2026-04-21T12:00:01Z",
                "tool": "simctl",
                "argv": ["simctl", "token=raw-report-host-token"],
                "cwd": "/workspace?access_token=raw-report-cwd-token",
                "host_env_disposition": "allowlist_applied",
                "env_allowlist_applied": ["SCHEME"],
                "env_dropped_from_provider": ["TOKEN"],
                "selected_simulator_id": "SIM-123",
                "exit_status": 0,
                "duration_ms": 120
            }],
            "storage": {
                "max_events": 1000,
                "max_bytes": 1048576,
                "truncated": false,
                "total_events_dropped": 0,
                "mcp_broker_observations_dropped": 0,
                "xcode_shim_events_dropped": 0,
                "xcode_host_executor_events_dropped": 0,
                "corrupt_json_recovery_count": 0,
                "corrupt_json_quarantined_bytes": 0
            }
        })
        .to_string();

        let readback = xcode_runtime_observation_json(Some(&raw));
        let serialized = readback.to_string();

        assert!(!serialized.contains("raw-report-token"));
        assert!(!serialized.contains("raw-report-bearer"));
        assert!(!serialized.contains("raw-report-warning-token"));
        assert!(!serialized.contains("raw-report-shim-token"));
        assert!(!serialized.contains("raw-report-host-token"));
        assert!(!serialized.contains("raw-report-cwd-token"));
        assert!(serialized.contains("token=<redacted>"));
        assert!(serialized.contains("Bearer <redacted>"));
        assert!(serialized.contains("xcode-lease-<redacted>"));
    }

    fn make_run(id: RunId, idea_id: IdeaId) -> domain::run::Run {
        domain::run::Run {
            id,
            idea_id,
            status: domain::run::RunStatus::Ready,
            workflow_id: "wf-release".into(),
            workflow_title: "Release".into(),
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
                "{\"repo_identifier\":\"repo-1\",\"repo_root\":\"/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .into(),
            ),
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
        }
    }

    fn make_artifact(run_id: RunId, name: &str, report_kind: Option<&str>) -> Artifact {
        Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_12_release".into(),
            agent_id: "release_agent".into(),
            name: name.into(),
            contract_id: name.into(),
            format: ArtifactFormat::Json,
            file_path: format!("/tmp/art/{name}.json"),
            checksum_sha256: None,
            size_bytes: None,
            provider: "custom".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: report_kind.map(|s| s.to_string()),
            report_version: None,
            agent_execution_id: None,
        }
    }

    async fn persist_rollout_contract_readback(pool: &sqlx::SqlitePool, run_id: RunId) {
        use rollout_contract_checks::{
            ProjectionIntegrity, RolloutContractDecision, RolloutContractEnforcementMode,
            RolloutContractLifecycleState, RolloutContractStatus, UpsertRolloutContractCheck,
        };

        rollout_contract_checks::upsert_rollout_contract_check(
            pool,
            &UpsertRolloutContractCheck {
                id: uuid::Uuid::new_v4(),
                run_id: run_id.inner(),
                proposal_id: "proposal-084".to_string(),
                proposal_revision_id: "p084-r5".to_string(),
                proposal_content_hash: "sha256:proposal".to_string(),
                contract_object_hash: "sha256:contract".to_string(),
                content_snapshot_id: "artifact-1".to_string(),
                checker_version: "test-checker".to_string(),
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
                cutover_policy_revision: Some("cutover-p084-test".to_string()),
                redaction_state: "partial".to_string(),
                retry_count: 0,
                preflight_timeout_seconds: 45,
            },
            Utc::now(),
        )
        .await
        .unwrap();
    }

    fn make_workflow_conflict(run_id: RunId) -> WorkflowConflictRecord {
        let candidates = vec![CandidateTransitionEvaluation {
            transition_id: "review_to_complete".into(),
            from_state_id: "review".into(),
            to_state_id: "complete".into(),
            condition_expression_id: Some("proposal_review_summary.pass == true".into()),
            result: CandidateTransitionResult::MissingInput,
            required_artifacts: vec!["proposal_review_summary".into()],
            missing_artifacts: vec!["proposal_review_summary".into()],
            missing_fields: vec![],
            source_artifact_ids: vec![],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some("proposal_review_summary is required".into()),
        }];
        let reason = WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition;
        let candidate_hash = candidate_transition_hash(&candidates);
        WorkflowConflictRecord {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            conflict_fingerprint: workflow_conflict_fingerprint(
                &run_id.to_string(),
                "review",
                &reason,
                &candidate_hash,
                &[],
            ),
            run_id: run_id.to_string(),
            stage_execution_id: None,
            lineage_id: Some("lineage-p017".into()),
            current_state_id: "review".into(),
            reason,
            operator_label: "Required transition input is missing".into(),
            status: WorkflowConflictStatus::Unresolved,
            candidate_transitions: candidates,
            candidate_transition_hash: candidate_hash,
            advisory_evidence_refs: vec![],
            lead_agent_id: None,
            mediation_record_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            resolved_at: None,
            superseded_by_conflict_id: None,
            resolution_record_json: None,
            terminal_failure_reason: None,
            diagnostic_redaction_tier: "operator_safe".into(),
        }
    }

    fn make_lead_mediation_record(
        run_id: RunId,
        conflict: &WorkflowConflictRecord,
        mediation_id: &str,
    ) -> LeadConflictMediationRecord {
        LeadConflictMediationRecord {
            id: mediation_id.to_string(),
            run_id: run_id.to_string(),
            conflict_id: conflict.conflict_id.clone(),
            conflict_fingerprint: conflict.conflict_fingerprint.clone(),
            lead_agent_id: "lead-agent-1".into(),
            status: LeadMediationStatus::OperatorConfirmationRequired,
            settlement_result: Some("operator_confirmed".into()),
            recovery_action: None,
            chosen_action: Some("advance".into()),
            chosen_next_state_id: Some("release".into()),
            chosen_next_state_label: Some("Release".into()),
            operator_rationale: Some("PRIVATE rationale must not leave storage".into()),
            sanitized_progress: Some("Lead mediation selected a release transition.".into()),
            validation_errors_json: Some(
                serde_json::json!([{"field": "summary", "message": "safe validation note"}])
                    .to_string(),
            ),
            cost_summary_json: Some(
                serde_json::json!({
                    "total_cost_cents": 42,
                    "input_tokens": 100,
                    "output_tokens": 25
                })
                .to_string(),
            ),
            metric_event_id: Some("metric-1".into()),
            superseded_by_event_ref: Some("event-2".into()),
            agent_execution_id: Some("agent-exec-1".into()),
            confirmation_subject_id: Some("confirmation-1".into()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settled_at: None,
        }
    }

    async fn persist_handoff_required_summary(pool: &sqlx::SqlitePool, run_id: RunId) {
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
            "verification_green": true,
            "remaining_code_tasks": [],
            "handoff_tasks": [{
                "summary": "Capture release note",
                "owner_class": "release",
                "target_stage": "state_11_manual_release",
                "blocking_review": true,
                "evidence": "release owner must attach the note before go/no-go"
            }],
            "known_risks": [],
            "tests_run": ["proposal-054: green"],
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

    #[tokio::test]
    async fn reports_get_includes_release_artifacts_and_report_kind_entries() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        artifacts::insert(&pool, &make_artifact(run_id, "release_manifest", None))
            .await
            .unwrap();
        artifacts::insert(&pool, &make_artifact(run_id, "delivery_receipt", None))
            .await
            .unwrap();
        artifacts::insert(
            &pool,
            &make_artifact(run_id, "execution_report", Some("execution_report")),
        )
        .await
        .unwrap();
        artifacts::insert(&pool, &make_artifact(run_id, "other_blob", None))
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        // reports.get returns enriched serde_json::Value objects with file_path stripped,
        // so we extract names from the JSON array rather than deserializing as Vec<Artifact>.
        let reports: Vec<serde_json::Value> = serde_json::from_value(result).unwrap();
        let names: Vec<String> = reports
            .into_iter()
            .filter_map(|v| v["name"].as_str().map(String::from))
            .collect();

        assert!(names.contains(&"release_manifest".to_string()));
        assert!(names.contains(&"delivery_receipt".to_string()));
        assert!(names.contains(&"execution_report".to_string()));
        assert!(!names.contains(&"other_blob".to_string()));
    }

    #[tokio::test]
    async fn reports_get_exposes_rollout_readback_on_run_and_release_reports() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_rollout_contract_readback(&pool, run_id).await;

        artifacts::insert(&pool, &make_artifact(run_id, "delivery_receipt", None))
            .await
            .unwrap();
        artifacts::insert(
            &pool,
            &make_artifact(run_id, "execution_report", Some("execution_report")),
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let delivery_receipt = reports
            .iter()
            .find(|report| report["name"] == serde_json::json!("delivery_receipt"))
            .expect("delivery receipt report");
        let execution_report = reports
            .iter()
            .find(|report| report["name"] == serde_json::json!("execution_report"))
            .expect("execution report");

        for readback in [
            &mcp_truth["rollout_contract_readback"],
            &delivery_receipt["rollout_contract_readback"],
            &execution_report["rollout_contract_readback"],
        ] {
            assert_eq!(
                readback["schema_version"],
                serde_json::json!("operator_readback_v1")
            );
            assert_eq!(readback["backend_decision"], serde_json::json!("release"));
            assert_eq!(
                readback["cutover_policy_revision"],
                serde_json::json!("cutover-p084-test")
            );
        }
    }

    #[tokio::test]
    async fn reports_get_includes_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_handoff_required_summary(&pool, run_id).await;

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let summary = &mcp_truth["implementation_self_assessment_summary"];

        assert_eq!(summary["status"], serde_json::json!("handoff_required"));
        assert_eq!(
            summary["handoff_tasks"][0]["summary"],
            serde_json::json!("Capture release note")
        );
        assert_eq!(
            summary["owner_class_counts"]["release"],
            serde_json::json!(1)
        );
    }

    #[tokio::test]
    async fn proposal_017_reports_get_includes_current_workflow_conflict() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let conflict = make_workflow_conflict(run_id);
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let workflow_conflict = &mcp_truth["workflow_conflict"];

        assert_eq!(
            workflow_conflict["reason"],
            serde_json::json!("required_artifact_or_field_missing_for_transition")
        );
        assert_eq!(workflow_conflict["status"], serde_json::json!("unresolved"));
        assert_eq!(
            workflow_conflict["candidate_transitions"][0]["result"],
            serde_json::json!("missing_input")
        );
        assert_eq!(
            workflow_conflict["current_state_id"],
            serde_json::json!("review")
        );
    }

    #[tokio::test]
    async fn proposal_017_reports_get_exposes_refine_instruction_action_hint() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let mut conflict = make_workflow_conflict(run_id);
        conflict.reason = WorkflowConflictReason::NoDeclarativeTransitionMatched;
        conflict.operator_label = "No declarative workflow transition matched".into();
        conflict.candidate_transitions = vec![CandidateTransitionEvaluation {
            transition_id: "review_to_refine".into(),
            from_state_id: "review".into(),
            to_state_id: "review".into(),
            condition_expression_id: Some("proposal_needs_refine".into()),
            result: CandidateTransitionResult::NotMatched,
            required_artifacts: vec!["proposal_review_summary".into()],
            missing_artifacts: vec![],
            missing_fields: vec![],
            source_artifact_ids: vec!["proposal_review_summary".into()],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some(
                "Loop budget exhausted for proposal_review_count: 3/3 iterations".into(),
            ),
        }];
        conflict.candidate_transition_hash =
            candidate_transition_hash(&conflict.candidate_transitions);
        conflict.conflict_fingerprint = workflow_conflict_fingerprint(
            &run_id.to_string(),
            "review",
            &conflict.reason,
            &conflict.candidate_transition_hash,
            &[],
        );
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        assert_eq!(
            mcp_truth["workflow_conflict"]["suggested_operator_action"],
            serde_json::json!("choose_transition_or_provide_refine_instruction")
        );
    }

    #[tokio::test]
    async fn proposal_017_reports_get_includes_sanitized_lead_mediation_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let mediation_id = "mediation-p017-readback";
        let mut conflict = make_workflow_conflict(run_id);
        conflict.status = WorkflowConflictStatus::OperatorConfirmationRequired;
        conflict.mediation_record_id = Some(mediation_id.into());
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();
        db::repos::lead_conflict_mediations::insert(
            &pool,
            &make_lead_mediation_record(run_id, &conflict, mediation_id),
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let mediation = &mcp_truth["workflow_conflict"]["lead_mediation"];

        assert_eq!(mediation["id"], serde_json::json!(mediation_id));
        assert_eq!(
            mediation["conflict_id"],
            serde_json::json!(conflict.conflict_id)
        );
        assert_eq!(
            mediation["lead_agent_id"],
            serde_json::json!("lead-agent-1")
        );
        assert_eq!(
            mediation["status"],
            serde_json::json!("operator_confirmation_required")
        );
        assert_eq!(
            mediation["resolution_mode"],
            serde_json::json!("operator_confirmation")
        );
        assert_eq!(mediation["chosen_action"], serde_json::json!("advance"));
        assert_eq!(
            mediation["chosen_next_state_id"],
            serde_json::json!("release")
        );
        assert_eq!(
            mediation["chosen_next_state_label"],
            serde_json::json!("Release")
        );
        assert_eq!(
            mediation["sanitized_progress"],
            serde_json::json!("Lead mediation selected a release transition.")
        );
        assert_eq!(
            mediation["status_updates"][0]["status"],
            serde_json::json!("operator_confirmation_required")
        );
        assert_eq!(
            mediation["status_updates"][0]["sanitized_progress"],
            serde_json::json!("Lead mediation selected a release transition.")
        );
        assert_eq!(
            mediation["status_updates"][0]["attempt_number"],
            serde_json::json!(1)
        );
        assert!(mediation["status_updates"][0]["updated_at"].is_string());
        assert_eq!(
            mediation["confirmation_subject_id"],
            serde_json::json!("confirmation-1")
        );
        assert_eq!(
            mediation["superseded_by_event_ref"],
            serde_json::json!("event-2")
        );
        assert_eq!(
            mediation["validation_errors"][0]["field"],
            serde_json::json!("summary")
        );
        assert_eq!(
            mediation["cost_summary"]["total_cost_cents"],
            serde_json::json!(42)
        );

        let serialized = serde_json::to_string(&mcp_truth).unwrap();
        assert!(!serialized.contains("operator_rationale"));
        assert!(!serialized.contains("operatorRationale"));
        assert!(!serialized.contains("PRIVATE rationale"));
    }

    /// P017 R2 / API-001: every mediation-owned `agent_executions` row must
    /// surface under `workflow_conflict.lead_mediation.execution_attempts`
    /// in MCP, with owner identity, nullable stage execution ID, runtime
    /// facts, watchdog, and per-attempt timing.
    #[tokio::test]
    async fn proposal_017_workflow_conflict_lead_mediation_execution_attempts() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let mediation_id = "mediation-p017-attempts";
        let mut conflict = make_workflow_conflict(run_id);
        conflict.status = WorkflowConflictStatus::OperatorConfirmationRequired;
        conflict.mediation_record_id = Some(mediation_id.into());
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();
        db::repos::lead_conflict_mediations::insert(
            &pool,
            &make_lead_mediation_record(run_id, &conflict, mediation_id),
        )
        .await
        .unwrap();

        // Insert two mediation-owned agent_executions (no stage_execution_id).
        let exec_one = AgentExecution {
            id: domain::ids::AgentExecutionId::new(),
            stage_execution_id: None,
            agent_id: "lead-agent-1".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            status: domain::agent::AgentStatus::Failed,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: None,
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
            owner_kind: Some("lead_conflict_mediation".into()),
            owner_id: Some(mediation_id.into()),
            lead_mediation_record_id: Some(mediation_id.into()),
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        };
        let exec_one_id = exec_one.id;
        db::repos::agent_executions::insert(&pool, &exec_one)
            .await
            .unwrap();

        let exec_two = AgentExecution {
            id: domain::ids::AgentExecutionId::new(),
            started_at: exec_one.started_at + chrono::Duration::seconds(1),
            status: domain::agent::AgentStatus::Completed,
            ..exec_one.clone()
        };
        let exec_two_id = exec_two.id;
        db::repos::agent_executions::insert(&pool, &exec_two)
            .await
            .unwrap();

        // P017 R4 / API-002: stamp per-attempt cost + transcript on the
        // second execution so the readback proves non-null cost/transcript_ref.
        let transcript_artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_test".into(),
            agent_id: "lead-agent-1".into(),
            name: "session_transcript".into(),
            contract_id: "session_transcript".into(),
            format: ArtifactFormat::Markdown,
            file_path: "/tmp/session_transcript.md".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("session_transcript".into()),
            report_version: Some(1),
            agent_execution_id: None,
        };
        let transcript_artifact_id = transcript_artifact.id.to_string();
        artifacts::insert(&pool, &transcript_artifact)
            .await
            .unwrap();
        db::repos::agent_executions::update_attempt_attribution(
            &pool,
            exec_two_id,
            Some(123), // total_cost_cents
            Some(500), // input_tokens
            Some(75),  // output_tokens
            Some(40),  // cached_input_tokens
            Some(&transcript_artifact_id),
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|r| r["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let mediation = &mcp_truth["workflow_conflict"]["lead_mediation"];

        let attempts = mediation["execution_attempts"]
            .as_array()
            .expect("execution_attempts array");
        assert_eq!(attempts.len(), 2, "two attempts expected");

        // Both attempts carry mediation-owned identity and a NULL stage execution id.
        for attempt in attempts {
            assert_eq!(
                attempt["owner_kind"],
                serde_json::json!("lead_conflict_mediation")
            );
            assert_eq!(attempt["owner_id"], serde_json::json!(mediation_id));
            assert_eq!(
                attempt["mediation_record_id"],
                serde_json::json!(mediation_id)
            );
            assert!(
                attempt["stage_execution_id"].is_null(),
                "mediation-owned attempt has no stage execution id"
            );
            assert_eq!(attempt["agent_id"], serde_json::json!("lead-agent-1"));
            assert_eq!(attempt["provider"], serde_json::json!("claude"));
            assert!(attempt["started_at"].is_string());
        }

        // Attempts are sorted by started_at ASC; attempt_number is durable
        // (1..N), not hard-coded.
        assert_eq!(
            attempts[0]["agent_execution_id"],
            serde_json::json!(exec_one_id.to_string())
        );
        assert_eq!(attempts[0]["attempt_number"], serde_json::json!(1));
        assert_eq!(attempts[0]["status"], serde_json::json!("failed"));
        assert_eq!(
            attempts[1]["agent_execution_id"],
            serde_json::json!(exec_two_id.to_string())
        );
        assert_eq!(attempts[1]["attempt_number"], serde_json::json!(2));
        assert_eq!(attempts[1]["status"], serde_json::json!("completed"));

        // The synthesized status_updates entry now reflects the durable
        // attempt count instead of hard-coded 1.
        assert_eq!(
            mediation["status_updates"][0]["attempt_number"],
            serde_json::json!(2)
        );

        // P017 R4 / API-002: attempt 1 left cost/transcript as None;
        // attempt 2 has both populated.
        assert!(
            attempts[0]["cost"].is_null(),
            "attempt 1 cost should be null when no usage data was recorded"
        );
        assert!(
            attempts[0]["transcript_ref"].is_null(),
            "attempt 1 transcript_ref should be null when no transcript was persisted"
        );

        let attempt2_cost = &attempts[1]["cost"];
        assert!(
            !attempt2_cost.is_null(),
            "attempt 2 cost must be non-null after update_attempt_attribution"
        );
        assert_eq!(attempt2_cost["total_cost_cents"], serde_json::json!(123));
        assert_eq!(attempt2_cost["input_tokens"], serde_json::json!(500));
        assert_eq!(attempt2_cost["output_tokens"], serde_json::json!(75));
        assert_eq!(attempt2_cost["cached_input_tokens"], serde_json::json!(40));

        let attempt2_transcript = &attempts[1]["transcript_ref"];
        assert!(
            !attempt2_transcript.is_null(),
            "attempt 2 transcript_ref must be non-null when artifact is linked"
        );
        assert_eq!(
            attempt2_transcript["artifact_id"],
            serde_json::json!(transcript_artifact_id)
        );
        assert_eq!(attempt2_transcript["format"], serde_json::json!("markdown"));

        // Attempt 2 artifacts include the direct transcript via tier-1 linkage.
        let attempt2_artifacts = attempts[1]["artifacts"]
            .as_array()
            .expect("attempt 2 artifacts array");
        assert!(
            attempt2_artifacts.iter().any(|a| {
                a["id"] == serde_json::json!(transcript_artifact_id)
                    && a["linkage"] == serde_json::json!("transcript_direct")
            }),
            "attempt 2 must surface the direct transcript artifact"
        );

        // No operator_rationale anywhere in the readback.
        let serialized = serde_json::to_string(&mcp_truth).unwrap();
        assert!(!serialized.contains("operator_rationale"));
        assert!(!serialized.contains("operatorRationale"));
        assert!(!serialized.contains("PRIVATE rationale"));
    }

    #[tokio::test]
    async fn reports_get_decodes_validation_failure_payload() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let payload = validation_failure_payload(run_id);
        let payload_path = std::env::temp_dir().join(format!("validation-failure-{}.json", run_id));
        std::fs::write(&payload_path, serde_json::to_vec(&payload).unwrap()).unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;

        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "stage_1".into(),
            agent_id: "validation_agent".into(),
            name: "validation_failure_validation_agent".into(),
            contract_id: "validation_failure_record".into(),
            format: ArtifactFormat::Json,
            file_path: payload_path.to_string_lossy().to_string(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".into()),
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        validation::insert(
            &pool,
            &validation_failure_record(artifact.id, run_id, stage_execution_id, agent_execution_id),
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("array");
        let validation_failure = reports
            .iter()
            .find(|artifact| artifact["report_kind"] == serde_json::json!("validation_failure"))
            .expect("validation failure artifact");

        assert_eq!(
            validation_failure["validation_failure_record"]["failureSummary"],
            serde_json::json!("report: Missing required fields: summary")
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["outputResults"][0]["missingFields"],
            serde_json::json!(["summary"])
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["sessionReuseDisposition"],
            serde_json::json!("reused")
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["sessionResetReason"],
            serde_json::json!("operator_reset")
        );
    }

    #[tokio::test]
    async fn reports_mcp_resolution_truth_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        seed_validation_attempt(&pool, run_id).await;

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let execution = &mcp_truth["agent_executions"][0];

        assert_eq!(
            execution["backend_profile_id"],
            serde_json::json!("codex_with_mcp")
        );
        assert_eq!(
            execution["requested_mcp_extensions_json"],
            serde_json::json!(r#"["filesystem"]"#)
        );
        assert_eq!(
            execution["actual_mcp_runtime_ids_json"],
            serde_json::json!(r#"["fs-runtime"]"#)
        );
        assert_eq!(
            execution["xcode_runtime_observation"]["mcp_broker_observations"][0]["lease_id"],
            serde_json::json!("lease-1")
        );
        assert_eq!(
            execution["xcode_runtime_observation"]["mcp_broker_observations"][0]["http_endpoint"],
            serde_json::json!("127.0.0.1:<redacted>")
        );
        assert!(
            !execution["xcode_runtime_observation"]
                .to_string()
                .contains("Bearer "),
            "reports.get must expose only the persisted redacted observation payload"
        );
    }

    #[tokio::test]
    async fn proposal_041_reports_get_readback_parity_surface() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        seed_validation_attempt(&pool, run_id).await;
        artifacts::insert(
            &pool,
            &make_artifact(run_id, "p041_operator_report", Some("operator_summary")),
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();
        let reports = result.as_array().expect("reports array");
        assert!(reports.iter().any(|report| {
            report["name"] == serde_json::json!("p041_operator_report")
                && report["report_kind"] == serde_json::json!("operator_summary")
        }));

        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let execution = &mcp_truth["agent_executions"][0];
        assert_eq!(
            execution["actual_mcp_runtime_ids_json"],
            serde_json::json!(r#"["fs-runtime"]"#)
        );
    }

    fn make_run_with_root(id: RunId, idea_id: IdeaId, artifact_root: &str) -> domain::run::Run {
        let mut run = make_run(id, idea_id);
        run.artifact_root = artifact_root.to_string();
        run
    }

    #[tokio::test]
    async fn reports_failed_stage_evidence_contract_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        // SEC-P081: evidence file must reside inside the run's artifact_root.
        let artifact_dir = tempfile::TempDir::new().unwrap();
        let artifact_root = artifact_dir.path().to_string_lossy().to_string();
        runs::insert(&pool, &make_run_with_root(run_id, idea_id, &artifact_root))
            .await
            .unwrap();

        let payload_path = artifact_dir
            .path()
            .join(format!("failed-stage-evidence-{run_id}.json"));
        std::fs::write(
            &payload_path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "report_kind": "failed_stage_evidence",
                "run_id": run_id.to_string(),
                "stage_id": "stage_1",
                "failure_summary": "failed",
                "recovery_snapshot": { "status": "available" }
            }))
            .unwrap(),
        )
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "stage_1".into(),
                agent_id: "agent_1".into(),
                name: "failed_stage_evidence_stage_1".into(),
                contract_id: "failed_stage_evidence".into(),
                format: ArtifactFormat::Json,
                file_path: payload_path.to_string_lossy().to_string(),
                checksum_sha256: None,
                size_bytes: None,
                provider: "system".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("failed_stage_evidence".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();
        let reports = result.as_array().expect("reports array");
        let evidence = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("failed_stage_evidence"))
            .expect("failed-stage evidence report");

        assert_eq!(
            evidence["failed_stage_evidence"]["report_kind"],
            serde_json::json!("failed_stage_evidence")
        );
        assert_eq!(
            evidence["failed_stage_evidence"]["recovery_snapshot"]["status"],
            serde_json::json!("available")
        );
    }

    // SEC-P081: evidence files outside the run's artifact_root must be silently
    // omitted (null) rather than read and exposed through reports.get.
    #[tokio::test]
    async fn failed_stage_evidence_outside_artifact_root_is_rejected() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        // artifact_root points to one temp dir; the evidence file lives outside it.
        let artifact_dir = tempfile::TempDir::new().unwrap();
        let outside_dir = tempfile::TempDir::new().unwrap();
        let artifact_root = artifact_dir.path().to_string_lossy().to_string();
        runs::insert(&pool, &make_run_with_root(run_id, idea_id, &artifact_root))
            .await
            .unwrap();

        // Write the "sensitive" file outside the artifact_root.
        let outside_path = outside_dir
            .path()
            .join(format!("outside-evidence-{run_id}.json"));
        std::fs::write(&outside_path, br#"{"secret": "should_not_be_returned"}"#).unwrap();

        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "stage_1".into(),
                agent_id: "agent_1".into(),
                name: "failed_stage_evidence_stage_1".into(),
                contract_id: "failed_stage_evidence".into(),
                format: ArtifactFormat::Json,
                // Crafted to point outside artifact_root — containment must block this.
                file_path: outside_path.to_string_lossy().to_string(),
                checksum_sha256: None,
                size_bytes: None,
                provider: "system".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("failed_stage_evidence".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();
        let reports = result.as_array().expect("reports array");
        let evidence = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("failed_stage_evidence"))
            .expect("failed-stage evidence report present");

        // The payload must be null — the containment check must reject the outside path.
        assert_eq!(
            evidence["failed_stage_evidence"],
            serde_json::Value::Null,
            "evidence file outside artifact_root must be rejected (null), not served"
        );
        // The secret content must not appear anywhere in the response.
        let serialized = serde_json::to_string(&evidence).unwrap();
        assert!(
            !serialized.contains("should_not_be_returned"),
            "secret content must not leak through path-containment boundary"
        );
    }
}
