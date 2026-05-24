use anyhow::{anyhow, Result};
use auth::boundary::{BoundaryPolicy, PolicyMode};
use chrono::Utc;
use serde_json::json;
use sqlx::SqlitePool;

use crate::protocol::McpTool;

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

pub async fn boundary_runtime_readback(
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    let audit_health = db::repos::audit_log::health_snapshot(pool).await?;
    let integrity_state = db::repos::audit_log::verify_latest_checkpoint(pool).await;
    let safe_mode_active = boundary_policy
        .map(|policy| matches!(policy.mode(), PolicyMode::ReadOnlySafeMode))
        .unwrap_or(false);

    Ok(json!({
        "schemaVersion": "boundary_runtime.v1",
        "matrixId": boundary_policy.map(|_| "p081-boundary-matrix-v1"),
        "policyInjected": boundary_policy.is_some(),
        "policyMode": boundary_policy.map(|policy| policy.mode().as_str()),
        "safeModeActive": safe_mode_active,
        "fixtureDigest": boundary_policy.map(|policy| policy.fixture_digest()),
        "auditLogHealth": {
            "schemaVersion": "audit_log_health.v1",
            "rowCount": audit_health.row_count,
            "latestRowId": audit_health.latest_row_id,
            "latestCheckpointSeq": audit_health.latest_checkpoint_seq,
            "latestCheckpointHash": audit_health.latest_checkpoint_hash,
            "integrityState": integrity_state.as_str(),
            "writable": audit_health.writable,
            "retentionMinDays": audit_health.retention_min_days,
            "cleanupState": audit_health.cleanup_state,
            "cleanupEligibleRowCount": audit_health.cleanup_eligible_row_count,
            "cleanupProtectedRowCount": audit_health.cleanup_protected_row_count,
            "payloadBudgetBytes": audit_health.payload_budget_bytes,
            "payloadUsedBytes": audit_health.payload_used_bytes,
            "shadowCoverageReportRef": audit_health.shadow_coverage_report_ref,
        }
    }))
}

pub async fn operator_alerts_readback(
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    let boundary_runtime = boundary_runtime_readback(pool, boundary_policy).await?;
    let now_ms = Utc::now().timestamp_millis();
    let mut alerts = Vec::new();

    if boundary_runtime["safeModeActive"]
        .as_bool()
        .unwrap_or(false)
    {
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
        }
    }))
}

pub async fn execute_with_name(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    boundary_policy: Option<&BoundaryPolicy>,
) -> Result<serde_json::Value> {
    match tool_name {
        "runtime.health" => execute(params, pool, boundary_policy).await,
        "operator.alerts.list" => operator_alerts_readback(pool, boundary_policy).await,
        _ => Err(anyhow!("Unknown runtime tool: {tool_name}")),
    }
}
