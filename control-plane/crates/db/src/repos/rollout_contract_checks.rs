use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

/// Status of a rollout contract check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutContractStatus {
    Pass,
    Fail,
    Waived,
    NotApplicable,
    Timeout,
    Cancelled,
    MissingContract,
    TamperDetected,
    Stale,
}

impl std::fmt::Display for RolloutContractStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Waived => "waived",
            Self::NotApplicable => "not_applicable",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::MissingContract => "missing_contract",
            Self::TamperDetected => "tamper_detected",
            Self::Stale => "stale",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for RolloutContractStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            "waived" => Ok(Self::Waived),
            "not_applicable" => Ok(Self::NotApplicable),
            "timeout" => Ok(Self::Timeout),
            "cancelled" => Ok(Self::Cancelled),
            "missing_contract" => Ok(Self::MissingContract),
            "tamper_detected" => Ok(Self::TamperDetected),
            "stale" => Ok(Self::Stale),
            other => Err(format!("unknown rollout contract status: {other:?}")),
        }
    }
}

/// Scheduling decision derived from a rollout contract check.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutContractDecision {
    Release,
    Hold,
    Waive,
    NotApplicable,
}

impl std::fmt::Display for RolloutContractDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Release => "release",
            Self::Hold => "hold",
            Self::Waive => "waive",
            Self::NotApplicable => "not_applicable",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for RolloutContractDecision {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "release" => Ok(Self::Release),
            "hold" => Ok(Self::Hold),
            "waive" => Ok(Self::Waive),
            "not_applicable" => Ok(Self::NotApplicable),
            other => Err(format!("unknown rollout contract decision: {other:?}")),
        }
    }
}

/// Lifecycle state of a rollout contract check record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutContractLifecycleState {
    Running,
    Terminal,
    Partial,
}

impl std::fmt::Display for RolloutContractLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Running => "running",
            Self::Terminal => "terminal",
            Self::Partial => "partial",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for RolloutContractLifecycleState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(Self::Running),
            "terminal" => Ok(Self::Terminal),
            "partial" => Ok(Self::Partial),
            other => Err(format!(
                "unknown rollout contract lifecycle state: {other:?}"
            )),
        }
    }
}

/// Enforcement mode for the rollout contract preflight.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutContractEnforcementMode {
    Enforce,
    Permissive,
    Disabled,
}

impl std::fmt::Display for RolloutContractEnforcementMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Enforce => "enforce",
            Self::Permissive => "permissive",
            Self::Disabled => "disabled",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for RolloutContractEnforcementMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enforce" => Ok(Self::Enforce),
            "permissive" => Ok(Self::Permissive),
            "disabled" => Ok(Self::Disabled),
            other => Err(format!(
                "unknown rollout contract enforcement mode: {other:?}"
            )),
        }
    }
}

/// Projection integrity classification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionIntegrity {
    Valid,
    TamperDetected,
    Stale,
}

impl std::fmt::Display for ProjectionIntegrity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Valid => "valid",
            Self::TamperDetected => "tamper_detected",
            Self::Stale => "stale",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for ProjectionIntegrity {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "valid" => Ok(Self::Valid),
            "tamper_detected" => Ok(Self::TamperDetected),
            "stale" => Ok(Self::Stale),
            other => Err(format!("unknown projection integrity: {other:?}")),
        }
    }
}

fn validate_redaction_state(redaction_state: &str) -> Result<&str> {
    match redaction_state {
        "none" | "partial" | "full" => Ok(redaction_state),
        other => anyhow::bail!(
            "invalid rollout_contract_checks.redaction_state {other:?}; expected one of none, partial, full"
        ),
    }
}

/// A stored rollout contract check record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredRolloutContractCheck {
    pub id: Uuid,
    pub run_id: Uuid,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub proposal_content_hash: String,
    pub contract_object_hash: String,
    pub content_snapshot_id: String,
    pub checker_version: String,
    pub status: RolloutContractStatus,
    pub decision: RolloutContractDecision,
    pub lifecycle_state: RolloutContractLifecycleState,
    pub enforcement_mode: RolloutContractEnforcementMode,
    pub failure_reasons: Vec<String>,
    pub diagnostics: Vec<String>,
    pub waiver: Option<serde_json::Value>,
    pub rollback_disposition: serde_json::Value,
    pub projection_integrity: ProjectionIntegrity,
    pub cutover_policy_revision: Option<String>,
    pub redaction_state: String,
    pub retry_count: i64,
    pub preflight_timeout_seconds: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RolloutContractMetricEvent {
    pub event_id: String,
    pub run_id: String,
    pub rollout_contract_check_id: String,
    pub metric_name: String,
    pub labels_json: serde_json::Value,
    pub value: f64,
    pub unit: String,
    pub occurred_at: DateTime<Utc>,
}

impl StoredRolloutContractCheck {
    pub fn operator_readback_json(&self) -> serde_json::Value {
        self.operator_readback_json_for_lane("run_report")
    }

    pub fn operator_readback_json_for_lane(&self, source_lane: &str) -> serde_json::Value {
        let source_lane = match source_lane {
            "run_report" | "mcp" | "release_receipt" | "graphql" => source_lane,
            _ => "run_report",
        };
        let waiver_state = self
            .waiver
            .as_ref()
            .and_then(|waiver| waiver.get("state"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or(if self.waiver.is_some() {
                "active"
            } else {
                "none"
            });
        let waiver_expires_at = self
            .waiver
            .as_ref()
            .and_then(|waiver| waiver.get("expires_at"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let waiver_reason_code = self
            .waiver
            .as_ref()
            .and_then(|waiver| waiver.get("reason_code"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let enforcement_mode_reason =
            waiver_reason_code.as_str().map(str::to_string).or_else(|| {
                self.diagnostics.iter().find_map(|diagnostic| {
                    diagnostic
                        .strip_prefix("rollout_contract_enforcement_mode_reason=")
                        .map(str::to_string)
                })
            });
        let disabled_reason_code =
            if self.enforcement_mode == RolloutContractEnforcementMode::Disabled {
                enforcement_mode_reason
                    .as_ref()
                    .map(|reason| serde_json::Value::String(reason.clone()))
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            };
        let hold_conditions = if self.decision == RolloutContractDecision::Hold {
            self.failure_reasons.clone()
        } else {
            Vec::new()
        };
        let next_steps = match self.decision {
            RolloutContractDecision::Release => {
                vec!["continue_implementation_scheduling".to_string()]
            }
            RolloutContractDecision::Waive => {
                vec!["continue_under_audited_scheduling_time_waiver".to_string()]
            }
            RolloutContractDecision::Hold => {
                vec!["repair_rollout_contract_or_apply_privileged_waiver".to_string()]
            }
            RolloutContractDecision::NotApplicable => {
                vec!["no_rollout_contract_action".to_string()]
            }
        };
        let operator_message = match self.decision {
            RolloutContractDecision::Release => {
                "Rollout contract preflight passed; implementation scheduling may continue."
            }
            RolloutContractDecision::Waive => {
                "Rollout contract preflight was waived by an audited scheduling-time policy."
            }
            RolloutContractDecision::Hold => {
                "Rollout contract preflight held implementation scheduling."
            }
            RolloutContractDecision::NotApplicable => {
                "Rollout contract preflight is not applicable for this run."
            }
        };

        let readback = serde_json::json!({
            "schema_version": "operator_readback_v1",
            "authoritative_record_id": self.id.to_string(),
            "run_id": self.run_id.to_string(),
            "proposal_id": self.proposal_id,
            "proposal_revision_id": self.proposal_revision_id,
            "status": self.status,
            "backend_decision": self.decision,
            "failure_reasons": self.failure_reasons,
            "waiver_state": waiver_state,
            "waiver_expires_at": waiver_expires_at,
            "waiver": self.waiver,
            "enforcement_mode": self.enforcement_mode,
            "enforcement_mode_reason": enforcement_mode_reason,
            "hold_conditions": hold_conditions,
            "rollback_disposition": self.rollback_disposition,
            "enabled_state": if self.enforcement_mode == RolloutContractEnforcementMode::Disabled {
                "disabled"
            } else {
                "enabled"
            },
            "disabled_reason_code": disabled_reason_code,
            "action_id": format!("rollout_contract_check:{}", self.id),
            "operator_message": operator_message,
            "source_lane": source_lane,
            "projection_integrity": self.projection_integrity,
            "cutover_policy_revision": self.cutover_policy_revision,
            "diagnostic_redaction": self.redaction_state,
            "next_steps": next_steps,
            "adoption_metric": self.adoption_metric_summary_json(),
            "updated_at": self.updated_at.to_rfc3339(),
        });
        if source_lane == "graphql" {
            camel_case_operator_readback_json(&readback)
        } else {
            readback
        }
    }

    fn adoption_metric_summary_json(&self) -> serde_json::Value {
        let passed = matches!(
            (&self.status, &self.decision),
            (
                RolloutContractStatus::Pass,
                RolloutContractDecision::Release
            ) | (
                RolloutContractStatus::Waived,
                RolloutContractDecision::Waive
            )
        );
        serde_json::json!({
            "name": "new_applicable_proposals_with_passing_rollout_contract_percent",
            "applicable_proposals": 1,
            "passing_rollout_contracts": if passed { 1 } else { 0 },
            "percent": if passed { 100.0 } else { 0.0 },
            "status": if passed { "pass" } else { "not_green" }
        })
    }
}

fn camel_case_operator_readback_json(value: &serde_json::Value) -> serde_json::Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut out = serde_json::Map::new();
    for (key, value) in object {
        let mapped = match key.as_str() {
            "schema_version" => "schemaVersion",
            "authoritative_record_id" => "authoritativeRecordId",
            "run_id" => "runId",
            "proposal_id" => "proposalId",
            "proposal_revision_id" => "proposalRevisionId",
            "backend_decision" => "backendDecision",
            "failure_reasons" => "failureReasons",
            "waiver_state" => "waiverState",
            "waiver_expires_at" => "waiverExpiresAt",
            "enforcement_mode" => "enforcementMode",
            "enforcement_mode_reason" => "enforcementModeReason",
            "hold_conditions" => "holdConditions",
            "rollback_disposition" => "rollbackDisposition",
            "enabled_state" => "enabledState",
            "disabled_reason_code" => "disabledReasonCode",
            "action_id" => "actionId",
            "operator_message" => "operatorMessage",
            "source_lane" => "sourceLane",
            "projection_integrity" => "projectionIntegrity",
            "cutover_policy_revision" => "cutoverPolicyRevision",
            "diagnostic_redaction" => "diagnosticRedaction",
            "next_steps" => "nextSteps",
            "adoption_metric" => "adoptionMetric",
            "updated_at" => "updatedAt",
            other => other,
        };
        let value = if key == "rollback_disposition" {
            camel_case_rollback_disposition_json(value)
        } else if key == "adoption_metric" {
            camel_case_adoption_metric_json(value)
        } else {
            value.clone()
        };
        out.insert(mapped.to_string(), value);
    }
    serde_json::Value::Object(out)
}

fn camel_case_rollback_disposition_json(value: &serde_json::Value) -> serde_json::Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut out = serde_json::Map::new();
    for (key, value) in object {
        out.insert(
            match key.as_str() {
                "data_loss_risk" => "dataLossRisk".to_string(),
                other => other.to_string(),
            },
            value.clone(),
        );
    }
    serde_json::Value::Object(out)
}

fn camel_case_adoption_metric_json(value: &serde_json::Value) -> serde_json::Value {
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    let mut out = serde_json::Map::new();
    for (key, value) in object {
        out.insert(
            match key.as_str() {
                "applicable_proposals" => "applicableProposals".to_string(),
                "passing_rollout_contracts" => "passingRolloutContracts".to_string(),
                other => other.to_string(),
            },
            value.clone(),
        );
    }
    serde_json::Value::Object(out)
}

/// Input for creating or upserting a rollout contract check.
#[derive(Clone, Debug)]
pub struct UpsertRolloutContractCheck {
    pub id: Uuid,
    pub run_id: Uuid,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub proposal_content_hash: String,
    pub contract_object_hash: String,
    pub content_snapshot_id: String,
    pub checker_version: String,
    pub status: RolloutContractStatus,
    pub decision: RolloutContractDecision,
    pub lifecycle_state: RolloutContractLifecycleState,
    pub enforcement_mode: RolloutContractEnforcementMode,
    pub failure_reasons: Vec<String>,
    pub diagnostics: Vec<String>,
    pub waiver: Option<serde_json::Value>,
    pub rollback_disposition: serde_json::Value,
    pub projection_integrity: ProjectionIntegrity,
    pub cutover_policy_revision: Option<String>,
    pub redaction_state: String,
    pub retry_count: i64,
    pub preflight_timeout_seconds: i64,
}

pub async fn upsert_rollout_contract_check(
    pool: &SqlitePool,
    input: &UpsertRolloutContractCheck,
    now: DateTime<Utc>,
) -> Result<StoredRolloutContractCheck> {
    let id_str = input.id.to_string();
    let run_id_str = input.run_id.to_string();
    let status_str = input.status.to_string();
    let decision_str = input.decision.to_string();
    let lifecycle_str = input.lifecycle_state.to_string();
    let enforcement_str = input.enforcement_mode.to_string();
    let projection_str = input.projection_integrity.to_string();
    let redaction_state = validate_redaction_state(&input.redaction_state)?;
    let failure_reasons_json =
        serde_json::to_string(&input.failure_reasons).context("serialize failure_reasons")?;
    let diagnostics_json =
        serde_json::to_string(&input.diagnostics).context("serialize diagnostics")?;
    let waiver_json = input
        .waiver
        .as_ref()
        .map(|w| serde_json::to_string(w).context("serialize waiver"))
        .transpose()?;
    let rollback_disposition_json = serde_json::to_string(&input.rollback_disposition)
        .context("serialize rollback_disposition")?;
    let now_str = now.to_rfc3339();

    crate::execute_repository_write!(
        pool,
        "rollout_contract_checks.upsert_rollout_contract_check",
        sqlx::query(
            r#"
        INSERT INTO rollout_contract_checks (
          id, run_id, proposal_id, proposal_revision_id,
          proposal_content_hash, contract_object_hash, content_snapshot_id,
          checker_version, status, decision, lifecycle_state, enforcement_mode,
          failure_reasons_json, diagnostics_json, waiver_json, rollback_disposition_json,
          projection_integrity, cutover_policy_revision, redaction_state,
          retry_count, preflight_timeout_seconds, created_at, updated_at
        ) VALUES (
          ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
          ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?22
        )
        ON CONFLICT(id) DO UPDATE SET
          status                  = excluded.status,
          decision                = excluded.decision,
          lifecycle_state         = excluded.lifecycle_state,
          enforcement_mode        = excluded.enforcement_mode,
          failure_reasons_json    = excluded.failure_reasons_json,
          diagnostics_json        = excluded.diagnostics_json,
          waiver_json             = excluded.waiver_json,
          rollback_disposition_json = excluded.rollback_disposition_json,
          projection_integrity    = excluded.projection_integrity,
          cutover_policy_revision = excluded.cutover_policy_revision,
          redaction_state         = excluded.redaction_state,
          retry_count             = excluded.retry_count,
          updated_at              = excluded.updated_at
        "#,
        )
        .bind(&id_str)
        .bind(&run_id_str)
        .bind(&input.proposal_id)
        .bind(&input.proposal_revision_id)
        .bind(&input.proposal_content_hash)
        .bind(&input.contract_object_hash)
        .bind(&input.content_snapshot_id)
        .bind(&input.checker_version)
        .bind(&status_str)
        .bind(&decision_str)
        .bind(&lifecycle_str)
        .bind(&enforcement_str)
        .bind(&failure_reasons_json)
        .bind(&diagnostics_json)
        .bind(waiver_json.as_deref())
        .bind(&rollback_disposition_json)
        .bind(&projection_str)
        .bind(input.cutover_policy_revision.as_deref())
        .bind(redaction_state)
        .bind(input.retry_count)
        .bind(input.preflight_timeout_seconds)
        .bind(&now_str)
    )
    .context("upsert rollout_contract_checks")?;

    let stored = find_rollout_contract_check(pool, input.id)
        .await?
        .context("rollout contract check not found after upsert")?;
    record_rollout_contract_metric_events(pool, &stored, now).await?;
    Ok(stored)
}

pub async fn find_rollout_contract_check(
    pool: &SqlitePool,
    id: Uuid,
) -> Result<Option<StoredRolloutContractCheck>> {
    let id_str = id.to_string();
    let row = sqlx::query("SELECT * FROM rollout_contract_checks WHERE id = ?1")
        .bind(&id_str)
        .fetch_optional(pool)
        .await
        .context("find rollout_contract_check")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn find_terminal_rollout_contract_check_for_run(
    pool: &SqlitePool,
    run_id: Uuid,
) -> Result<Option<StoredRolloutContractCheck>> {
    let run_id_str = run_id.to_string();
    let row = sqlx::query(
        r#"SELECT * FROM rollout_contract_checks
           WHERE run_id = ?1 AND lifecycle_state = 'terminal'
           ORDER BY updated_at DESC LIMIT 1"#,
    )
    .bind(&run_id_str)
    .fetch_optional(pool)
    .await
    .context("find terminal rollout_contract_check for run")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn list_rollout_contract_metric_events_for_run(
    pool: &SqlitePool,
    run_id: Uuid,
) -> Result<Vec<RolloutContractMetricEvent>> {
    let run_id_str = run_id.to_string();
    let rows = sqlx::query(
        r#"SELECT * FROM rollout_contract_metric_events
           WHERE run_id = ?1
           ORDER BY occurred_at ASC, event_id ASC"#,
    )
    .bind(&run_id_str)
    .fetch_all(pool)
    .await
    .context("list rollout contract metric events for run")?;

    rows.iter().map(parse_metric_event_row).collect()
}

async fn record_rollout_contract_metric_events(
    pool: &SqlitePool,
    check: &StoredRolloutContractCheck,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    if check.lifecycle_state != RolloutContractLifecycleState::Terminal {
        return Ok(());
    }

    let events = metric_events_for_check(check, occurred_at);
    let check_id = check.id.to_string();
    crate::execute_repository_write!(
        pool,
        "rollout_contract_checks.record_rollout_contract_metric_events",
        sqlx::query(
            "DELETE FROM rollout_contract_metric_events WHERE rollout_contract_check_id = ?1"
        )
        .bind(&check_id)
    )
    .context("delete stale rollout contract metric events")?;

    for event in events {
        crate::execute_repository_write!(
            pool,
            "rollout_contract_checks.record_rollout_contract_metric_events",
            sqlx::query(
                r#"INSERT INTO rollout_contract_metric_events
               (event_id, run_id, rollout_contract_check_id, metric_name, labels_json,
                value, unit, occurred_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            )
            .bind(&event.event_id)
            .bind(&event.run_id)
            .bind(&event.rollout_contract_check_id)
            .bind(&event.metric_name)
            .bind(serde_json::to_string(&event.labels_json)?)
            .bind(event.value)
            .bind(&event.unit)
            .bind(event.occurred_at.to_rfc3339())
        )
        .context("insert rollout contract metric event")?;
    }

    Ok(())
}

fn metric_events_for_check(
    check: &StoredRolloutContractCheck,
    occurred_at: DateTime<Utc>,
) -> Vec<RolloutContractMetricEvent> {
    let mut events = Vec::new();
    let status = check.status.to_string();
    let enforcement_mode = check.enforcement_mode.to_string();
    let would_block = rollout_contract_check_would_block(check);
    let failure_reasons = if check.failure_reasons.is_empty() {
        vec!["none".to_string()]
    } else {
        check.failure_reasons.clone()
    };

    for reason in &failure_reasons {
        push_metric(
            &mut events,
            check,
            "rollout_contract_lint_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "status": &status,
                "failure_reason": bounded_metric_label(reason),
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    push_metric(
        &mut events,
        check,
        "rollout_contract_enforcement_mode_total",
        serde_json::json!({
            "proposal_id": &check.proposal_id,
            "mode": &enforcement_mode,
        }),
        1.0,
        "count",
        occurred_at,
    );

    if check.enforcement_mode == RolloutContractEnforcementMode::Permissive {
        push_metric(
            &mut events,
            check,
            "rollout_contract_permissive_dogfood_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "status": &status,
                "would_block": would_block,
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    if check.enforcement_mode == RolloutContractEnforcementMode::Enforce && would_block {
        push_metric(
            &mut events,
            check,
            "rollout_contract_run_start_block_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "reason": bounded_metric_label(primary_failure_reason(check)),
                "enforcement_mode": "enforce",
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    if check.status == RolloutContractStatus::Waived {
        push_metric(
            &mut events,
            check,
            "rollout_contract_waiver_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "reason": bounded_metric_label(waiver_reason(check)),
                "waiver_state": waiver_state(check),
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    if check.status == RolloutContractStatus::Cancelled {
        push_metric(
            &mut events,
            check,
            "rollout_contract_preflight_cancelled_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "reason": bounded_metric_label(primary_failure_reason(check)),
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    if check
        .failure_reasons
        .iter()
        .any(|reason| reason == "rollout_contract_preflight_retry_exhausted")
    {
        push_metric(
            &mut events,
            check,
            "rollout_contract_retry_exhausted_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "failure_class": "infrastructure",
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    if check.projection_integrity != ProjectionIntegrity::Valid {
        let projection_integrity = check.projection_integrity.to_string();
        push_metric(
            &mut events,
            check,
            "rollout_contract_tamper_or_stale_projection_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "projection_integrity": &projection_integrity,
            }),
            1.0,
            "count",
            occurred_at,
        );
        push_metric(
            &mut events,
            check,
            "rollout_contract_hash_drift_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "drift_class": &projection_integrity,
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    if check
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("partial") && diagnostic.contains("recovered"))
    {
        push_metric(
            &mut events,
            check,
            "rollout_contract_partial_write_recovered_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "recovery": "projection_rewrite",
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    for followup in late_followup_reasons(check) {
        push_metric(
            &mut events,
            check,
            "late_rollout_evidence_followup_total",
            serde_json::json!({
                "proposal_id": &check.proposal_id,
                "followup_type": bounded_metric_label(followup),
            }),
            1.0,
            "count",
            occurred_at,
        );
    }

    events
}

fn push_metric(
    events: &mut Vec<RolloutContractMetricEvent>,
    check: &StoredRolloutContractCheck,
    metric_name: &str,
    labels_json: serde_json::Value,
    value: f64,
    unit: &str,
    occurred_at: DateTime<Utc>,
) {
    events.push(RolloutContractMetricEvent {
        event_id: format!("rollout-contract:{}:{:03}", check.id, events.len()),
        run_id: check.run_id.to_string(),
        rollout_contract_check_id: check.id.to_string(),
        metric_name: metric_name.to_string(),
        labels_json,
        value,
        unit: unit.to_string(),
        occurred_at,
    });
}

fn rollout_contract_check_would_block(check: &StoredRolloutContractCheck) -> bool {
    let green = matches!(
        (&check.status, &check.decision),
        (
            RolloutContractStatus::Pass,
            RolloutContractDecision::Release
        ) | (
            RolloutContractStatus::Waived,
            RolloutContractDecision::Waive
        ) | (
            RolloutContractStatus::NotApplicable,
            RolloutContractDecision::NotApplicable
        )
    );
    !green && check.enforcement_mode != RolloutContractEnforcementMode::Disabled
}

fn primary_failure_reason(check: &StoredRolloutContractCheck) -> &str {
    check
        .failure_reasons
        .first()
        .map(String::as_str)
        .unwrap_or_else(|| match check.status {
            RolloutContractStatus::Pass => "none",
            RolloutContractStatus::Fail => "rollout_contract_fail",
            RolloutContractStatus::Waived => "scheduling_time_waiver",
            RolloutContractStatus::NotApplicable => "not_applicable",
            RolloutContractStatus::Timeout => "rollout_contract_preflight_timeout",
            RolloutContractStatus::Cancelled => "rollout_contract_preflight_cancelled",
            RolloutContractStatus::MissingContract => "missing_rollout_contract_check",
            RolloutContractStatus::TamperDetected => "tamper_detected_rollout_contract_projection",
            RolloutContractStatus::Stale => "stale_rollout_contract_projection",
        })
}

fn waiver_state(check: &StoredRolloutContractCheck) -> &str {
    check
        .waiver
        .as_ref()
        .and_then(|waiver| waiver.get("state"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("active")
}

fn waiver_reason(check: &StoredRolloutContractCheck) -> &str {
    check
        .waiver
        .as_ref()
        .and_then(|waiver| waiver.get("reason_code"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("scheduling_time_waiver")
}

fn late_followup_reasons(check: &StoredRolloutContractCheck) -> Vec<&str> {
    check
        .failure_reasons
        .iter()
        .filter_map(|reason| match reason.as_str() {
            reason if reason.contains("missing_metrics") => Some("missing_metrics"),
            reason if reason.contains("missing_readback") => Some("missing_readback"),
            reason if reason.contains("missing_gate") => Some("missing_gate_alias"),
            reason if reason.contains("missing_operator_report_fields") => {
                Some("missing_operator_report_fields")
            }
            _ => None,
        })
        .collect()
}

fn bounded_metric_label(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | ':' => ch,
            _ => '_',
        })
        .take(96)
        .collect()
}

fn parse_row(row: &sqlx::sqlite::SqliteRow) -> Result<StoredRolloutContractCheck> {
    let id_str: String = row.get("id");
    let run_id_str: String = row.get("run_id");
    let status_str: String = row.get("status");
    let decision_str: String = row.get("decision");
    let lifecycle_str: String = row.get("lifecycle_state");
    let enforcement_str: String = row.get("enforcement_mode");
    let projection_str: String = row.get("projection_integrity");
    let failure_reasons_json: String = row.get("failure_reasons_json");
    let diagnostics_json: String = row.get("diagnostics_json");
    let waiver_json: Option<String> = row.get("waiver_json");
    let rollback_disposition_json: String = row
        .try_get("rollback_disposition_json")
        .unwrap_or_else(|_| default_rollback_disposition_json());
    let created_at_str: String = row.get("created_at");
    let updated_at_str: String = row.get("updated_at");

    Ok(StoredRolloutContractCheck {
        id: id_str.parse().context("parse rollout_contract_check id")?,
        run_id: run_id_str
            .parse()
            .context("parse rollout_contract_check run_id")?,
        proposal_id: row.get("proposal_id"),
        proposal_revision_id: row.get("proposal_revision_id"),
        proposal_content_hash: row.get("proposal_content_hash"),
        contract_object_hash: row.get("contract_object_hash"),
        content_snapshot_id: row.get("content_snapshot_id"),
        checker_version: row.get("checker_version"),
        status: status_str.parse().map_err(|e| anyhow::anyhow!("{e}"))?,
        decision: decision_str.parse().map_err(|e| anyhow::anyhow!("{e}"))?,
        lifecycle_state: lifecycle_str.parse().map_err(|e| anyhow::anyhow!("{e}"))?,
        enforcement_mode: enforcement_str
            .parse()
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        failure_reasons: serde_json::from_str(&failure_reasons_json)
            .context("parse failure_reasons_json")?,
        diagnostics: serde_json::from_str(&diagnostics_json).context("parse diagnostics_json")?,
        waiver: waiver_json
            .map(|j| serde_json::from_str::<serde_json::Value>(&j).context("parse waiver_json"))
            .transpose()?,
        rollback_disposition: serde_json::from_str(&rollback_disposition_json)
            .context("parse rollback_disposition_json")?,
        projection_integrity: projection_str.parse().map_err(|e| anyhow::anyhow!("{e}"))?,
        cutover_policy_revision: row.get("cutover_policy_revision"),
        redaction_state: row.get("redaction_state"),
        retry_count: row.get("retry_count"),
        preflight_timeout_seconds: row.get("preflight_timeout_seconds"),
        created_at: DateTime::parse_from_rfc3339(&created_at_str)
            .context("parse created_at")?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
            .context("parse updated_at")?
            .with_timezone(&Utc),
    })
}

fn default_rollback_disposition_json() -> String {
    serde_json::json!({
        "mode": "not_applicable",
        "data_loss_risk": "none",
        "steps": []
    })
    .to_string()
}

fn parse_metric_event_row(row: &sqlx::sqlite::SqliteRow) -> Result<RolloutContractMetricEvent> {
    let labels_raw: String = row.get("labels_json");
    let occurred_raw: String = row.get("occurred_at");
    Ok(RolloutContractMetricEvent {
        event_id: row.get("event_id"),
        run_id: row.get("run_id"),
        rollout_contract_check_id: row.get("rollout_contract_check_id"),
        metric_name: row.get("metric_name"),
        labels_json: serde_json::from_str(&labels_raw)
            .context("parse rollout contract metric labels_json")?,
        value: row.get("value"),
        unit: row.get("unit"),
        occurred_at: DateTime::parse_from_rfc3339(&occurred_raw)
            .context("parse rollout contract metric occurred_at")?
            .with_timezone(&Utc),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn test_pool() -> (TempDir, SqlitePool) {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = crate::pool::create_pool(&url).await.unwrap();
        let writer = Arc::new(crate::writer::DbWriter::new(pool.clone()));
        crate::writer::register_shared_writer(&pool, writer)
            .await
            .unwrap();
        (dir, pool)
    }

    fn make_input(run_id: Uuid) -> UpsertRolloutContractCheck {
        UpsertRolloutContractCheck {
            id: Uuid::new_v4(),
            run_id,
            proposal_id: "proposal-084".to_string(),
            proposal_revision_id: "p084-r5".to_string(),
            proposal_content_hash: "sha256:abc".to_string(),
            contract_object_hash: "sha256:def".to_string(),
            content_snapshot_id: "snap-test".to_string(),
            checker_version: "1.0.0".to_string(),
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
            cutover_policy_revision: Some("p084-cutover-v1".to_string()),
            redaction_state: "none".to_string(),
            retry_count: 0,
            preflight_timeout_seconds: 45,
        }
    }

    #[tokio::test]
    async fn upsert_and_find_roundtrip() {
        let (_dir, pool) = test_pool().await;
        let run_id = Uuid::new_v4();
        let input = make_input(run_id);
        let check_id = input.id;
        let now = Utc::now();

        let stored = upsert_rollout_contract_check(&pool, &input, now)
            .await
            .unwrap();
        assert_eq!(stored.id, check_id);
        assert_eq!(stored.run_id, run_id);
        assert_eq!(stored.status, RolloutContractStatus::Pass);
        assert_eq!(stored.decision, RolloutContractDecision::Release);
        assert_eq!(
            stored.lifecycle_state,
            RolloutContractLifecycleState::Terminal
        );
        assert_eq!(
            stored.enforcement_mode,
            RolloutContractEnforcementMode::Enforce
        );
        assert!(stored.failure_reasons.is_empty());
        assert_eq!(
            stored.cutover_policy_revision.as_deref(),
            Some("p084-cutover-v1")
        );

        let found = find_rollout_contract_check(&pool, check_id)
            .await
            .unwrap()
            .expect("should find by id");
        assert_eq!(found.id, check_id);
        assert_eq!(found.proposal_id, "proposal-084");
    }

    #[tokio::test]
    async fn find_terminal_for_run() {
        let (_dir, pool) = test_pool().await;
        let run_id = Uuid::new_v4();
        let now = Utc::now();

        let mut input = make_input(run_id);
        input.lifecycle_state = RolloutContractLifecycleState::Running;
        upsert_rollout_contract_check(&pool, &input, now)
            .await
            .unwrap();

        let not_terminal = find_terminal_rollout_contract_check_for_run(&pool, run_id)
            .await
            .unwrap();
        assert!(not_terminal.is_none());

        let mut terminal_input = make_input(run_id);
        terminal_input.lifecycle_state = RolloutContractLifecycleState::Terminal;
        upsert_rollout_contract_check(&pool, &terminal_input, now)
            .await
            .unwrap();

        let found = find_terminal_rollout_contract_check_for_run(&pool, run_id)
            .await
            .unwrap()
            .expect("should find terminal record");
        assert_eq!(
            found.lifecycle_state,
            RolloutContractLifecycleState::Terminal
        );
    }

    #[tokio::test]
    async fn upsert_updates_status_on_conflict() {
        let (_dir, pool) = test_pool().await;
        let run_id = Uuid::new_v4();
        let now = Utc::now();
        let input = make_input(run_id);
        let check_id = input.id;

        upsert_rollout_contract_check(&pool, &input, now)
            .await
            .unwrap();

        let updated_input = UpsertRolloutContractCheck {
            id: check_id,
            run_id,
            status: RolloutContractStatus::Fail,
            decision: RolloutContractDecision::Hold,
            failure_reasons: vec!["missing_metrics".to_string()],
            ..make_input(run_id)
        };
        let updated = upsert_rollout_contract_check(&pool, &updated_input, now)
            .await
            .unwrap();
        assert_eq!(updated.status, RolloutContractStatus::Fail);
        assert_eq!(updated.decision, RolloutContractDecision::Hold);
        assert_eq!(updated.failure_reasons, vec!["missing_metrics"]);
    }

    #[tokio::test]
    async fn terminal_upsert_records_bounded_rollout_metric_events() {
        let (_dir, pool) = test_pool().await;
        let run_id = Uuid::new_v4();
        let now = Utc::now();
        let input = UpsertRolloutContractCheck {
            status: RolloutContractStatus::Fail,
            decision: RolloutContractDecision::Hold,
            failure_reasons: vec!["missing_metrics".to_string()],
            ..make_input(run_id)
        };
        let check_id = input.id.to_string();

        upsert_rollout_contract_check(&pool, &input, now)
            .await
            .unwrap();

        let events = list_rollout_contract_metric_events_for_run(&pool, run_id)
            .await
            .unwrap();
        assert!(events.iter().any(|event| {
            event.metric_name == "rollout_contract_lint_total"
                && event.labels_json["failure_reason"] == "missing_metrics"
        }));
        assert!(events
            .iter()
            .any(|event| event.metric_name == "rollout_contract_enforcement_mode_total"));
        assert!(events.iter().any(|event| {
            event.metric_name == "rollout_contract_run_start_block_total"
                && event.labels_json["reason"] == "missing_metrics"
        }));
        assert!(events
            .iter()
            .any(|event| event.metric_name == "late_rollout_evidence_followup_total"));

        let updated = UpsertRolloutContractCheck {
            id: input.id,
            run_id,
            status: RolloutContractStatus::Pass,
            decision: RolloutContractDecision::Release,
            failure_reasons: vec![],
            ..make_input(run_id)
        };
        upsert_rollout_contract_check(&pool, &updated, now)
            .await
            .unwrap();

        let updated_events = list_rollout_contract_metric_events_for_run(&pool, run_id)
            .await
            .unwrap();
        assert!(updated_events
            .iter()
            .all(|event| event.rollout_contract_check_id == check_id));
        assert!(!updated_events
            .iter()
            .any(|event| event.metric_name == "rollout_contract_run_start_block_total"));
    }

    #[tokio::test]
    async fn upsert_rejects_non_canonical_redaction_state() {
        let (_dir, pool) = test_pool().await;
        let mut input = make_input(Uuid::new_v4());
        input.redaction_state = "bounded".to_string();

        let error = upsert_rollout_contract_check(&pool, &input, Utc::now())
            .await
            .expect_err("non-canonical diagnostic redaction state must not persist");

        assert!(
            error.to_string().contains("redaction_state"),
            "error should name the invalid redaction_state: {error:#}"
        );
    }

    #[test]
    fn operator_readback_json_exposes_required_decision_surface() {
        let run_id = Uuid::new_v4();
        let mut check = make_input(run_id);
        check.status = RolloutContractStatus::Fail;
        check.decision = RolloutContractDecision::Hold;
        check.failure_reasons = vec!["missing_rollout_contract_check".to_string()];
        check.waiver = Some(serde_json::json!({
            "state": "active",
            "expires_at": "2026-05-03T00:00:00Z",
            "reason_code": "emergency_override"
        }));
        check.redaction_state = "partial".to_string();

        let stored = StoredRolloutContractCheck {
            id: check.id,
            run_id: check.run_id,
            proposal_id: check.proposal_id,
            proposal_revision_id: check.proposal_revision_id,
            proposal_content_hash: check.proposal_content_hash,
            contract_object_hash: check.contract_object_hash,
            content_snapshot_id: check.content_snapshot_id,
            checker_version: check.checker_version,
            status: check.status,
            decision: check.decision,
            lifecycle_state: check.lifecycle_state,
            enforcement_mode: check.enforcement_mode,
            failure_reasons: check.failure_reasons,
            diagnostics: check.diagnostics,
            waiver: check.waiver,
            rollback_disposition: check.rollback_disposition,
            projection_integrity: check.projection_integrity,
            cutover_policy_revision: check.cutover_policy_revision,
            redaction_state: check.redaction_state,
            retry_count: check.retry_count,
            preflight_timeout_seconds: check.preflight_timeout_seconds,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let readback = stored.operator_readback_json();
        assert_eq!(readback["schema_version"], "operator_readback_v1");
        assert_eq!(readback["status"], "fail");
        assert_eq!(readback["backend_decision"], "hold");
        assert_eq!(readback["waiver_state"], "active");
        assert_eq!(readback["enforcement_mode_reason"], "emergency_override");
        assert_eq!(
            readback["hold_conditions"],
            serde_json::json!(["missing_rollout_contract_check"])
        );
        assert_eq!(readback["projection_integrity"], "valid");
        assert_eq!(
            readback["rollback_disposition"]["mode"],
            "feature_flag_disable_or_enforcement_mode_permissive"
        );
        assert_eq!(readback["source_lane"], "run_report");
        assert_eq!(
            stored.operator_readback_json_for_lane("release_receipt")["source_lane"],
            "release_receipt"
        );
        let graphql = stored.operator_readback_json_for_lane("graphql");
        assert_eq!(graphql["schemaVersion"], "operator_readback_v1");
        assert_eq!(graphql["backendDecision"], "hold");
        assert_eq!(graphql["sourceLane"], "graphql");
        assert_eq!(
            graphql["rollbackDisposition"]["dataLossRisk"],
            serde_json::json!("none")
        );
        assert_eq!(
            graphql["adoptionMetric"]["name"],
            "new_applicable_proposals_with_passing_rollout_contract_percent"
        );
        assert_eq!(readback["adoption_metric"]["applicable_proposals"], 1);
        assert_eq!(readback["diagnostic_redaction"], "partial");
        assert!(readback["action_id"]
            .as_str()
            .unwrap()
            .starts_with("rollout_contract_check:"));
    }
}
