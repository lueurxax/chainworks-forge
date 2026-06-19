use anyhow::{anyhow, Result};
use auth::boundary::{BoundaryPolicy, PolicyMode};
use chrono::Utc;
use domain::tool_policy::{
    DEFAULT_CUMULATIVE_TOOL_OUTPUT_MAX_BYTES, DEFAULT_TOOL_OUTPUT_MAX_BYTES,
    DEFAULT_TOOL_OUTPUT_MAX_LINES, GENERATED_ROOT_DENYLIST, TOOL_GUARD_VERSION,
    TOOL_POLICY_VERSION,
};
use serde_json::json;
use sqlx::SqlitePool;
use std::sync::{Mutex, OnceLock};

use crate::protocol::McpTool;

static P081_MCP_SAFE_MODE_ALERT_OPENED_AT_MS: OnceLock<Mutex<Option<i64>>> = OnceLock::new();

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "runtime.health".to_string(),
            description: "Read lightweight MCP/runtime liveness status".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
        boundary_runtime_tool_spec(),
        McpTool {
            name: "operator.alerts.list".to_string(),
            description: "Read bounded operator alerts derived from runtime policy health"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

pub fn boundary_runtime_tool_spec() -> McpTool {
    McpTool {
        name: "boundary.runtime.get".to_string(),
        description: "Read P081 boundary runtime diagnostics using the MCP snake_case contract"
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub async fn boundary_runtime_readback(
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    let audit_health = db::repos::audit_log::health_snapshot(pool).await?;
    let integrity_state = db::repos::audit_log::verify_latest_checkpoint(pool).await;
    let safe_mode_active = boundary_policy
        .map(|policy| matches!(policy.mode(), PolicyMode::ReadOnlySafeMode))
        .unwrap_or(false)
        || audit_health.payload_budget_state == "read_only_safe_mode";

    Ok(json!({
        "schemaVersion": "boundary_runtime.v1",
        "matrixId": boundary_policy.map(|_| "p081-boundary-matrix-v1"),
        "policyInjected": boundary_policy.is_some(),
        "policyMode": boundary_policy.map(|policy| policy.mode().as_str()),
        "safeModeActive": safe_mode_active,
        "safeModeReason": if audit_health.payload_budget_state == "read_only_safe_mode" {
            Some("AUDIT_BUDGET_EXHAUSTED")
        } else if safe_mode_active {
            Some("BOUNDARY_POLICY_SAFE_MODE")
        } else {
            None
        },
        "fixtureDigest": boundary_policy.map(|policy| policy.fixture_digest()),
        "subscriptionReplay": {
            "schemaVersion": "subscription_replay_runtime_v1",
            "sequenceCursor": "seq-0",
            "projectionGeneration": 0,
            "gapDetected": false,
            "requestedCursor": null,
            "oldestRetainedCursor": "seq-0",
            "retentionMinutes": 15,
            "retentionEventCount": 10000,
            "requiresFullRefetch": false
        },
        "auditLogHealth": {
            "schemaVersion": "audit_log_health.v1",
            "rowCount": audit_health.row_count,
            "latestRowId": audit_health.latest_row_id,
            "latestCheckpointSeq": audit_health.latest_checkpoint_seq,
            "latestCheckpointHash": audit_health.latest_checkpoint_hash,
            "integrityState": integrity_state.as_str(),
            "writable": audit_health.writable,
            "lastWriteOkAtMs": audit_health.last_write_ok_at_ms,
            "consecutiveFailures": audit_health.consecutive_failures,
            "cumulativeFailures": audit_health.cumulative_failures,
            "retentionMinDays": audit_health.retention_min_days,
            "cleanupState": audit_health.cleanup_state,
            "cleanupEligibleRowCount": audit_health.cleanup_eligible_row_count,
            "cleanupProtectedRowCount": audit_health.cleanup_protected_row_count,
            "budgetBytes": audit_health.budget_bytes,
            "usedBytes": audit_health.used_bytes,
            "payloadBudgetBytes": audit_health.payload_budget_bytes,
            "payloadUsedBytes": audit_health.payload_used_bytes,
            "payloadBudgetState": audit_health.payload_budget_state,
            "payloadBudgetUsedPercent": audit_health.payload_budget_used_percent,
            "halfOpenProbeSuccessCount": audit_health.half_open_probe_success_count,
            "shadowCoverageReportRef": audit_health.shadow_coverage_report_ref,
        }
    }))
}

pub async fn boundary_runtime_readback_snake_case(
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    Ok(to_snake_case_value(
        boundary_runtime_readback(pool, boundary_policy).await?,
    ))
}

fn to_snake_case_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (camel_to_snake(&key), to_snake_case_value(value)))
                .collect::<serde_json::Map<_, _>>(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(to_snake_case_value).collect())
        }
        other => other,
    }
}

fn camel_to_snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (idx, ch) in key.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn record_safe_mode_alert_lifecycle(active: bool, now_ms: i64) {
    let slot = P081_MCP_SAFE_MODE_ALERT_OPENED_AT_MS.get_or_init(|| Mutex::new(None));
    let mut opened = slot.lock().expect("p081 mcp alert lifecycle poisoned");
    match (active, *opened) {
        (true, None) => *opened = Some(now_ms),
        (false, Some(opened_at)) => {
            *opened = None;
            let elapsed_ms = now_ms.saturating_sub(opened_at).max(0);
            db::metrics::record_p081_operator_alert_clear_latency(
                "p081-safe-mode-active",
                "critical",
                std::time::Duration::from_millis(elapsed_ms as u64),
            );
        }
        _ => {}
    }
}

pub async fn operator_alerts_readback(
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    let boundary_runtime = boundary_runtime_readback(pool, boundary_policy).await?;
    let now_ms = Utc::now().timestamp_millis();
    let mut alerts = Vec::new();

    let safe_mode_active = boundary_runtime["safeModeActive"]
        .as_bool()
        .unwrap_or(false);
    record_safe_mode_alert_lifecycle(safe_mode_active, now_ms);

    if safe_mode_active {
        alerts.push(json!({
            "schemaVersion": "operator_alert_v1",
            "id": "p081-safe-mode-active",
            "dedupeKey": "p081.boundary.safe_mode_active",
            "severity": "critical",
            "title": "Boundary policy is in safe mode",
            "message": "State-changing GraphQL and MCP operations are denied until boundary policy health is restored.",
            "source": "boundaryRuntime",
            "active": true,
            "silenceable": false,
            "acknowledgedAtMs": null,
            "silencedUntilMs": null,
            "nativeDelivery": {
                "schemaVersion": "operator_alert_native_delivery_v1",
                "deliveryKey": "p081.boundary.safe_mode_active",
                "dockBadgeContribution": 1,
                "requestUserAttention": "critical",
                "notificationCategory": "BOUNDARY_POLICY_CRITICAL",
                "dedupePolicy": "dedupe_key_until_clear"
            },
            "lifecycle": {
                "state": "active_unacknowledged",
                "dedupeKey": "p081.boundary.safe_mode_active",
                "ackRequired": true,
                "clearCondition": "boundaryRuntime.safeModeActive=false"
            },
            "createdAtMs": now_ms,
            "clearCondition": "boundaryRuntime.safeModeActive=false",
            "boundaryRuntime": boundary_runtime,
        }));
    }

    Ok(json!({
        "schemaVersion": "operator_alerts_readback_v1",
        "alerts": alerts,
    }))
}

pub async fn execute(
    _params: serde_json::Value,
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    let hot_read_circuit_rows =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hot_read_circuit_states")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let runtime_health_projection =
        db::repos::storage_health::runtime_health_projection(pool).await?;
    let boundary_runtime = boundary_runtime_readback(pool, boundary_policy).await?;

    Ok(json!({
        "schemaVersion": "runtime_health.v1",
        "status": "available",
        "updatedAt": Utc::now().to_rfc3339(),
        "runtimeHealthProjection": runtime_health_projection,
        "boundaryRuntime": boundary_runtime,
        "mcpRequestLoop": {
            "status": "available",
            "singleRequestSerialized": false
        },
        "hotReadGuard": {
            "status": "available",
            "trackedSurfaces": hot_read_circuit_rows
        },
        "toolOutputGuard": {
            "status": "available",
            "policyReadback": {
                "status": "available",
                "source": "domain::tool_policy"
            },
            "enforcement": {
                "status": "configured",
                "scope": "codex_provider_runtime",
                "wrapperTools": ["rg", "find"],
                "pathStrategy": "runtime_home_bin_prepend",
                "activeProbeStatus": "not_run_by_runtime_health",
                "activeProbeEvidence": "./scripts/test-gate.sh proposal-096"
            },
            "policyVersion": TOOL_POLICY_VERSION,
            "guardVersion": TOOL_GUARD_VERSION,
            "generatedRootDenylist": GENERATED_ROOT_DENYLIST,
            "maxOutputBytes": DEFAULT_TOOL_OUTPUT_MAX_BYTES,
            "maxOutputLines": DEFAULT_TOOL_OUTPUT_MAX_LINES,
            "maxCumulativeOutputBytes": DEFAULT_CUMULATIVE_TOOL_OUTPUT_MAX_BYTES
        },
        "qualityGateBoundary": p094_quality_gate_boundary_readback()
    }))
}

pub fn p094_quality_gate_boundary_readback() -> serde_json::Value {
    json!({
        "schemaVersion": "quality_gate_boundary_runtime_v1",
        "proposalId": "P094",
        "status": "available",
        "mode": "approval_dry_run",
        "source": "workflow_owned_quality_gate_boundary",
        "allowedModes": ["disabled", "readback_only", "approval_dry_run", "enforcing", "held"],
        "activeGateEvidence": "./scripts/test-gate.sh proposal-094",
        "contracts": [
            "proposal_decomposition_plan_v1",
            "quality_gate_blocker_assessment_v1",
            "blocker_boundary_status_v1",
            "blocker_boundary_approval_request_v1",
            "blocker_boundary_human_decision_v1",
            "followup_proposal_seed_v1"
        ],
        "operatorAction": "use workflow approval gates; accept/reject only",
        "rolloutDecision": {
            "schemaVersion": "p094_rollout_decision_readback_v1",
            "mode": "approval_dry_run",
            "decisionState": "held_before_enforcement",
            "modeSource": "retained_gate_default",
            "sampleWindow": {
                "source": "retained_gate",
                "commands": ["./scripts/test-gate.sh proposal-094", "./scripts/test-gate.sh p094"]
            },
            "goCriteria": {
                "accepted_boundary_later_rejected_percent_max": 5,
                "post_boundary_reopen_local_code_tail_missed_max": 1,
                "projection_integrity_required": "valid",
                "release_blocking_external_evidence_not_satisfied_by_acceptance": true
            },
            "metricNames": [
                "quality_gate_blocker_assessments_total",
                "quality_gate_blocker_validation_rejections_total",
                "quality_gate_blocker_freshness_total",
                "implementation_refine_loops_avoided_total",
                "followup_proposal_seeds_created_total",
                "external_blockers_accepted_total",
                "invalid_blocker_claims_total",
                "review_refresh_required_total",
                "output_settlement_required_before_boundary_total",
                "human_boundary_approval_latency_seconds",
                "post_boundary_reopen_total",
                "false_external_blocker_rate",
                "repeated_blocker_no_progress_total",
                "accepted_boundary_later_rejected_percent",
                "blocker_boundary_approvals_total",
                "quality_gate_blocker_boundary_route_total"
            ],
            "metricValues": db::metrics::p094_rollout_metric_values_json(),
            "ownerDecision": {
                "state": "hold",
                "reason": "P094 remains in approval dry-run until an operator-owned rollout-contract entry or command-journal-backed mode change promotes enforcement.",
                "commandJournalEntryId": null,
                "rolloutContractEntryId": null
            },
            "promotionReadiness": {
                "enforcingAllowed": false,
                "blockedReason": "missing_audited_owner_rollout_decision",
                "requiresBeforeEnforcing": "non_null_command_journal_or_rollout_contract_entry"
            },
            "auditability": {
                "status": "available",
                "enforcementModeChangesRequire": "command_journal_or_rollout_contract_entry"
            }
        }
    })
}

pub async fn execute_with_name(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    match tool_name {
        "runtime.health" => execute(params, pool, boundary_policy).await,
        "boundary.runtime.get" => boundary_runtime_readback_snake_case(pool, boundary_policy).await,
        "operator.alerts.list" => operator_alerts_readback(pool, boundary_policy).await,
        _ => Err(anyhow!("Unknown runtime tool: {tool_name}")),
    }
}
