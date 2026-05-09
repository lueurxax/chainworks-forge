use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use db::repos::rollout_contract_checks::{
    self, ProjectionIntegrity, RolloutContractDecision, RolloutContractEnforcementMode,
    RolloutContractLifecycleState, RolloutContractStatus, StoredRolloutContractCheck,
    UpsertRolloutContractCheck,
};
use domain::{artifact::Artifact, run::Run};
use sha2::Digest;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::{sleep, timeout, Duration};
use uuid::Uuid;

const CHECKER_VERSION: &str = "p084-engine-preflight-v1";
const PREFLIGHT_TIMEOUT_SECONDS: i64 = 45;
const PREFLIGHT_INFRA_RETRY_BACKOFF_MS: &[u64] = &[25, 75, 150];
const MAX_ROLLOUT_CONTRACT_INPUT_BYTES: u64 = 1024 * 1024;
const DEFAULT_PROPOSAL_ID: &str = "unknown";
const DEFAULT_PROPOSAL_REVISION_ID: &str = "unknown";
const DEFAULT_PROPOSAL_CONTENT_HASH: &str = "sha256:unknown";
const DEFAULT_CONTRACT_OBJECT_HASH: &str = "sha256:unknown";
const DEFAULT_CONTENT_SNAPSHOT_ID: &str = "unknown";
const ROLLOUT_CONTRACT_SCHEMA_VERSION: &str = "rollout_contract_v1";
const ROLLOUT_PREFLIGHT_POLICY_KEY: &str = "rollout_contract_preflight";
const REQUIRED_WHEN_APPLICABLE: &[&str] = &[
    "gate_aliases",
    "commands",
    "migrations",
    "metrics",
    "readback_lanes",
    "readback_fields",
    "readback_fixture",
    "operator_report_fields",
    "hold_conditions",
    "rollback_disposition",
    "decision_vocabulary",
    "negative_fixtures",
];
const ROLLBACK_DISPOSITION_REQUIRED: &[&str] = &["mode", "data_loss_risk"];
const ALLOWED_TOP_LEVEL_FIELDS: &[&str] = &[
    "schema_version",
    "applicability",
    "gate_aliases",
    "commands",
    "migrations",
    "metrics",
    "readback_lanes",
    "readback_fields",
    "readback_fixture",
    "operator_report_fields",
    "hold_conditions",
    "hold_conditions_detail",
    "rollback_disposition",
    "decision_vocabulary",
    "negative_fixtures",
    "not_applicable_justification",
    "not_applicable_justifications",
    "cutover_policy",
    "operator_message",
];
const ALLOWED_COMMAND_FIELDS: &[&str] = &["allowlist", "commentary"];
const ALLOWED_MIGRATION_FIELDS: &[&str] = &["not_applicable", "justification", "description"];
const ALLOWED_METRIC_FIELDS: &[&str] = &["adoption_metric", "operational_metrics"];
const ALLOWED_ROLLBACK_FIELDS: &[&str] = &["mode", "data_loss_risk", "steps"];
const ALLOWED_CUTOVER_FIELDS: &[&str] = &[
    "revision",
    "enforcement_mode_at_cutover",
    "applicable_to",
    "grandfathered_rendering",
    "effective_timestamp_iso8601",
];
const ALLOWED_READBACK_LANES: &[&str] = &["run_report", "mcp", "release_receipt", "graphql"];
const ALLOWED_DATA_LOSS_RISK: &[&str] = &["none", "low", "medium", "high"];
const ALLOWED_ENFORCEMENT_MODES: &[&str] = &["enforce", "permissive", "disabled"];
const ALLOWED_DECISION_VOCABULARY: &[&str] = &[
    "pass",
    "fail",
    "waived",
    "not_applicable",
    "timeout",
    "cancelled",
    "missing_contract",
    "tamper_detected",
    "stale",
    "release",
    "hold",
    "waive",
];
const REQUIRED_OPERATOR_READBACK_FIELDS: &[&str] = &[
    "rollout_contract_status",
    "rollout_contract_decision",
    "rollout_contract_failure_reasons",
    "rollout_contract_waiver_state",
    "rollout_contract_waiver_expires_at",
    "rollout_contract_enforcement_mode",
    "rollout_contract_enforcement_mode_reason",
    "rollout_contract_hold_conditions",
    "rollout_contract_rollback_disposition",
    "rollout_contract_source_lane",
    "rollout_contract_enabled_state",
    "rollout_contract_disabled_reason_code",
    "rollout_contract_action_id",
    "rollout_contract_operator_message",
    "rollout_contract_projection_integrity",
    "rollout_contract_cutover_policy_revision",
    "rollout_contract_diagnostic_redaction",
    "rollout_contract_next_steps",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RolloutContractPreflightAction {
    Allow,
    Hold,
}

#[derive(Clone, Debug)]
pub struct RolloutContractPreflightEvaluation {
    pub action: RolloutContractPreflightAction,
    pub check: StoredRolloutContractCheck,
    pub would_block: bool,
}

pub async fn implementation_run_start_rollout_contract_preflight(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
) -> Result<RolloutContractPreflightEvaluation> {
    let singleflight_key = preflight_singleflight_key(run, approved_proposal)?;
    let singleflight_lock = preflight_singleflight_lock(&singleflight_key);
    let guard = singleflight_lock.lock().await;

    let result = match timeout(
        preflight_timeout_duration(),
        implementation_run_start_rollout_contract_preflight_with_retries(
            pool,
            run,
            approved_proposal,
        ),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            let mode = terminal_failure_enforcement_mode(pool, run).await;
            let check = upsert_timeout_contract_check(pool, run, approved_proposal, mode).await?;
            write_rollout_contract_check_projection(run, &check)?;
            Ok(evaluate_terminal_check(check))
        }
    };

    cleanup_preflight_singleflight_lock(&singleflight_key, &singleflight_lock);
    drop(guard);
    result
}

async fn implementation_run_start_rollout_contract_preflight_with_retries(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
) -> Result<RolloutContractPreflightEvaluation> {
    let mut retry_count = 0_i64;

    loop {
        match implementation_run_start_rollout_contract_preflight_inner(
            pool,
            run,
            approved_proposal,
            retry_count,
        )
        .await
        {
            Ok(evaluation) => return Ok(evaluation),
            Err(error) if should_retry_preflight_error(&error) => {
                let Some(backoff_ms) = PREFLIGHT_INFRA_RETRY_BACKOFF_MS.get(retry_count as usize)
                else {
                    let mode = terminal_failure_enforcement_mode(pool, run).await;
                    let check = upsert_retry_exhausted_contract_check(
                        pool,
                        run,
                        approved_proposal,
                        mode,
                        retry_count,
                        &error,
                    )
                    .await?;
                    write_rollout_contract_check_projection(run, &check)?;
                    return Ok(evaluate_terminal_check(check));
                };
                retry_count += 1;
                sleep(Duration::from_millis(*backoff_ms)).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn implementation_run_start_rollout_contract_preflight_inner(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    retry_count: i64,
) -> Result<RolloutContractPreflightEvaluation> {
    if run.cancellation_requested_at.is_some() {
        let mode = default_rollout_contract_enforcement_mode();
        let check =
            upsert_cancelled_contract_check(pool, run, approved_proposal, mode, retry_count)
                .await?;
        write_rollout_contract_check_projection(run, &check)?;
        return Ok(evaluate_terminal_check(check));
    }

    let current_identity = current_contract_identity(run, approved_proposal)?;
    if let Some(check) =
        rollout_contract_checks::find_terminal_rollout_contract_check_for_run(pool, run.id.inner())
            .await?
    {
        let check = if let Some(drift) = current_identity
            .as_ref()
            .and_then(|identity| identity.drift_from(&check))
        {
            upsert_hash_drift_contract_check(pool, &check, &drift, retry_count).await?
        } else if let Some(integrity) = classify_existing_projection(run, &check)? {
            upsert_projection_integrity_check(pool, &check, integrity, retry_count).await?
        } else {
            check
        };
        write_rollout_contract_check_projection(run, &check)?;
        return Ok(evaluate_terminal_check(check));
    }

    let policy = effective_preflight_policy(pool, run, Utc::now()).await?;
    let mode = policy.enforcement_mode.clone();
    if !policy.failures.is_empty() {
        let check = upsert_policy_failure_contract_check(
            pool,
            run,
            approved_proposal,
            mode,
            &policy,
            retry_count,
        )
        .await?;
        write_rollout_contract_check_projection(run, &check)?;
        return Ok(evaluate_terminal_check(check));
    }
    if let Some(waiver) = policy.active_waiver.clone() {
        let check =
            upsert_waived_contract_check(pool, run, approved_proposal, mode, waiver, retry_count)
                .await?;
        write_rollout_contract_check_projection(run, &check)?;
        return Ok(evaluate_terminal_check(check));
    }
    if mode == RolloutContractEnforcementMode::Disabled {
        let disabled =
            upsert_disabled_contract_check(pool, run, approved_proposal, &policy, retry_count)
                .await?;
        write_rollout_contract_check_projection(run, &disabled)?;
        return Ok(evaluate_terminal_check(disabled));
    }

    let check = if let Some(artifact) = approved_proposal {
        if let Some(check) =
            upsert_linted_contract_check(pool, run, artifact, &policy, retry_count).await?
        {
            check
        } else {
            upsert_missing_contract_check(pool, run, approved_proposal, mode, retry_count).await?
        }
    } else {
        upsert_missing_contract_check(pool, run, approved_proposal, mode, retry_count).await?
    };
    write_rollout_contract_check_projection(run, &check)?;
    Ok(evaluate_terminal_check(check))
}

#[derive(Clone, Debug)]
struct CurrentContractIdentity {
    proposal_id: String,
    proposal_revision_id: String,
    proposal_content_hash: String,
    contract_object_hash: String,
    content_snapshot_id: String,
}

#[derive(Clone, Debug)]
struct ContractIdentityDrift {
    identity: CurrentContractIdentity,
    mismatches: Vec<String>,
}

impl CurrentContractIdentity {
    fn drift_from(&self, check: &StoredRolloutContractCheck) -> Option<ContractIdentityDrift> {
        let mut mismatches = Vec::new();
        if self.proposal_revision_id != check.proposal_revision_id {
            mismatches.push("proposal_revision_id".to_string());
        }
        if self.proposal_content_hash != check.proposal_content_hash {
            mismatches.push("proposal_content_hash".to_string());
        }
        if self.contract_object_hash != check.contract_object_hash {
            mismatches.push("contract_object_hash".to_string());
        }
        if self.content_snapshot_id != check.content_snapshot_id {
            mismatches.push("content_snapshot_id".to_string());
        }
        if mismatches.is_empty() {
            None
        } else {
            Some(ContractIdentityDrift {
                identity: self.clone(),
                mismatches,
            })
        }
    }
}

fn should_retry_preflight_error(error: &anyhow::Error) -> bool {
    let messages = error
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" | ");
    [
        "upsert rollout_contract_checks",
        "find terminal rollout_contract_check",
        "find rollout_contract_check",
        "rollout contract check not found after upsert",
        "write rollout contract check projection",
        "read rollout contract projection",
        "create projection directory",
        "create temp projection",
        "write temp projection",
        "finish temp projection",
        "fsync temp projection",
        "rename temp projection",
        "fsync projection directory",
    ]
    .iter()
    .any(|marker| messages.contains(marker))
}

type PreflightSingleflightMap = Mutex<HashMap<String, Arc<AsyncMutex<()>>>>;

static PREFLIGHT_SINGLEFLIGHT: OnceLock<PreflightSingleflightMap> = OnceLock::new();

fn preflight_singleflight_key(run: &Run, approved_proposal: Option<&Artifact>) -> Result<String> {
    let revision_id = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .map(|metadata| metadata.proposal_revision_id)
        .unwrap_or_else(|| DEFAULT_PROPOSAL_REVISION_ID.to_string());
    Ok(format!("{}:{revision_id}", run.id.inner()))
}

fn preflight_singleflight_lock(key: &str) -> Arc<AsyncMutex<()>> {
    let map = PREFLIGHT_SINGLEFLIGHT.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = map
        .lock()
        .expect("rollout preflight singleflight map poisoned");
    locks
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

fn cleanup_preflight_singleflight_lock(key: &str, lock: &Arc<AsyncMutex<()>>) {
    let Some(map) = PREFLIGHT_SINGLEFLIGHT.get() else {
        return;
    };
    let mut locks = map
        .lock()
        .expect("rollout preflight singleflight map poisoned");
    if locks
        .get(key)
        .is_some_and(|stored| Arc::ptr_eq(stored, lock) && Arc::strong_count(stored) <= 2)
    {
        locks.remove(key);
    }
}

fn preflight_timeout_duration() -> Duration {
    Duration::from_secs(PREFLIGHT_TIMEOUT_SECONDS as u64)
}

fn default_rollout_contract_enforcement_mode() -> RolloutContractEnforcementMode {
    RolloutContractEnforcementMode::Enforce
}

async fn terminal_failure_enforcement_mode(
    pool: &SqlitePool,
    run: &Run,
) -> RolloutContractEnforcementMode {
    match effective_preflight_policy(pool, run, Utc::now()).await {
        Ok(policy) if policy.failures.is_empty() => policy.enforcement_mode,
        Ok(_) | Err(_) => RolloutContractEnforcementMode::Enforce,
    }
}

#[derive(Clone, Debug)]
struct EffectivePreflightPolicy {
    enforcement_mode: RolloutContractEnforcementMode,
    enforcement_mode_reason_code: Option<String>,
    enforcement_mode_supplied: bool,
    active_waiver: Option<serde_json::Value>,
    failures: Vec<String>,
    diagnostics: Vec<String>,
}

async fn effective_preflight_policy(
    pool: &SqlitePool,
    run: &Run,
    now: DateTime<Utc>,
) -> Result<EffectivePreflightPolicy> {
    let mut policy = EffectivePreflightPolicy {
        enforcement_mode: default_rollout_contract_enforcement_mode(),
        enforcement_mode_reason_code: None,
        enforcement_mode_supplied: false,
        active_waiver: None,
        failures: Vec::new(),
        diagnostics: Vec::new(),
    };

    let Some(preflight) = rollout_preflight_policy_value(run)? else {
        return Ok(policy);
    };
    let Some(object) = preflight.as_object() else {
        policy
            .failures
            .push("invalid_rollout_contract_preflight_policy: must be an object".to_string());
        return Ok(policy);
    };

    if let Some(mode_value) = object.get("enforcement_mode") {
        match validated_policy_record(pool, mode_value, "enforcement_mode", now).await? {
            PolicyRecordValidation::Valid(record) => {
                let Some(mode) = record.get("mode").and_then(|value| value.as_str()) else {
                    policy.failures.push(
                        "invalid_enforcement_mode: enforcement_mode.mode is required".to_string(),
                    );
                    return Ok(policy);
                };
                policy.enforcement_mode = match mode {
                    "enforce" => RolloutContractEnforcementMode::Enforce,
                    "permissive" => RolloutContractEnforcementMode::Permissive,
                    "disabled" => RolloutContractEnforcementMode::Disabled,
                    other => {
                        policy.failures.push(format!(
                            "invalid_enforcement_mode: {other:?} not in {ALLOWED_ENFORCEMENT_MODES:?}"
                        ));
                        return Ok(policy);
                    }
                };
                policy.enforcement_mode_supplied = true;
                policy.enforcement_mode_reason_code = record
                    .get("reason_code")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                policy.diagnostics.push(format!(
                    "rollout contract enforcement mode supplied by audited run-start policy: {}",
                    policy.enforcement_mode
                ));
            }
            PolicyRecordValidation::Invalid(failures) => policy.failures.extend(failures),
        }
    }

    if let Some(waiver_value) = object.get("waiver") {
        match validated_policy_record(pool, waiver_value, "waiver", now).await? {
            PolicyRecordValidation::Valid(record) => {
                if record.get("state").and_then(|value| value.as_str()) != Some("active") {
                    policy
                        .failures
                        .push("invalid_waiver.state: waiver.state must be 'active'".to_string());
                }
                if record.get("decision").and_then(|value| value.as_str()) != Some("waive") {
                    policy.failures.push(
                        "invalid_waiver.decision: waiver.decision must be 'waive'".to_string(),
                    );
                }
                if policy.failures.is_empty() {
                    policy.active_waiver = Some(serde_json::Value::Object(record.clone()));
                    policy
                        .diagnostics
                        .push("valid scheduling-time rollout contract waiver accepted".to_string());
                }
            }
            PolicyRecordValidation::Invalid(failures) => policy.failures.extend(failures),
        }
    }

    Ok(policy)
}

enum PolicyRecordValidation {
    Valid(serde_json::Map<String, serde_json::Value>),
    Invalid(Vec<String>),
}

fn rollout_preflight_policy_value(run: &Run) -> Result<Option<serde_json::Value>> {
    let Some(raw) = run.delivery_preflight_json.as_deref() else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value =
        serde_json::from_str(raw).context("parse delivery_preflight_json")?;
    Ok(value
        .get(ROLLOUT_PREFLIGHT_POLICY_KEY)
        .or_else(|| value.get("rollout_contract"))
        .cloned())
}

async fn validated_policy_record(
    pool: &SqlitePool,
    value: &serde_json::Value,
    context: &str,
    now: DateTime<Utc>,
) -> Result<PolicyRecordValidation> {
    let Some(object) = value.as_object() else {
        return Ok(PolicyRecordValidation::Invalid(vec![format!(
            "invalid_{context}: must be an object"
        )]));
    };
    let mut failures = Vec::new();
    failures.extend(check_secret_like_values(value, context));
    if object.get("authorized").and_then(|value| value.as_bool()) != Some(true) {
        failures.push(format!(
            "invalid_{context}.authorized: must be true for scheduling-time rollout policy"
        ));
    }
    for field in ["principal_id", "audit_event_id", "reason_code"] {
        if !object
            .get(field)
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.trim().is_empty())
        {
            failures.push(format!(
                "missing_{context}.{field}: must be a non-empty string"
            ));
        }
    }
    let enforce_mode_record = context == "enforcement_mode"
        && object.get("mode").and_then(|value| value.as_str()) == Some("enforce");
    if !enforce_mode_record {
        if let (Some(principal_id), Some(audit_event_id)) = (
            object.get("principal_id").and_then(|value| value.as_str()),
            object
                .get("audit_event_id")
                .and_then(|value| value.as_str()),
        ) {
            let row: Option<(String, Option<String>, Option<String>)> =
                sqlx::query_as(
                    "SELECT command_type, caller_principal_id, caller_principal_class FROM command_journal WHERE id = ?1",
                )
                    .bind(audit_event_id)
                    .fetch_optional(pool)
                    .await
                    .context("lookup rollout policy audit_event_id in command_journal")?;
            match row {
                Some((command_type, Some(caller_principal_id), Some(caller_principal_class))) => {
                    if !matches!(command_type.as_str(), "StartRun" | "command.StartRun") {
                        failures.push(format!(
                            "invalid_{context}.audit_event_id: command journal entry must be a StartRun command"
                        ));
                    }
                    if caller_principal_class != "operator" {
                        failures.push(format!(
                            "invalid_{context}.audit_event_id: command journal principal class must be operator"
                        ));
                    }
                    if caller_principal_id.trim().is_empty() || caller_principal_id != principal_id
                    {
                        failures.push(format!(
                            "invalid_{context}.audit_event_id: command journal principal does not match principal_id"
                        ));
                    }
                }
                Some(_) => failures.push(format!(
                    "invalid_{context}.audit_event_id: command journal entry must include operator principal identity"
                )),
                None => failures.push(format!(
                    "missing_{context}.audit_event_id: command journal entry not found"
                )),
            }
        }
    }
    match object.get("expires_at").and_then(|value| value.as_str()) {
        Some(expires_at) => match DateTime::parse_from_rfc3339(expires_at) {
            Ok(expires_at) if expires_at.with_timezone(&Utc) > now => {}
            Ok(_) => failures.push(format!(
                "expired_{context}: expires_at must be later than scheduling time"
            )),
            Err(_) => failures.push(format!(
                "invalid_{context}.expires_at: must be an ISO-8601 timestamp string"
            )),
        },
        None => failures.push(format!(
            "missing_{context}.expires_at: TTL-bound rollout policy requires expires_at"
        )),
    }

    if failures.is_empty() {
        Ok(PolicyRecordValidation::Valid(object.clone()))
    } else {
        Ok(PolicyRecordValidation::Invalid(failures))
    }
}

fn evaluate_terminal_check(
    check: StoredRolloutContractCheck,
) -> RolloutContractPreflightEvaluation {
    let green = check.lifecycle_state == RolloutContractLifecycleState::Terminal
        && matches!(
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
    let disabled = check.enforcement_mode == RolloutContractEnforcementMode::Disabled;
    let cancelled = check.status == RolloutContractStatus::Cancelled;
    let action = if cancelled {
        RolloutContractPreflightAction::Hold
    } else if green
        || disabled
        || check.enforcement_mode == RolloutContractEnforcementMode::Permissive
    {
        RolloutContractPreflightAction::Allow
    } else {
        RolloutContractPreflightAction::Hold
    };

    RolloutContractPreflightEvaluation {
        would_block: cancelled || (!green && !disabled),
        action,
        check,
    }
}

fn rollout_contract_operational_metric_diagnostics(
    proposal_id: &str,
    status: &RolloutContractStatus,
    enforcement_mode: &RolloutContractEnforcementMode,
    failure_reasons: &[String],
    declared_operational_metrics: &[String],
) -> Vec<String> {
    let status = status.to_string();
    let enforcement_mode = enforcement_mode.to_string();
    let failure_labels: Vec<&str> = if failure_reasons.is_empty() {
        vec!["none"]
    } else {
        failure_reasons.iter().map(String::as_str).collect()
    };
    let would_block = status != "pass" && status != "waived" && status != "not_applicable";
    let mut diagnostics = Vec::new();

    for failure_reason in failure_labels {
        diagnostics.push(format!(
            "metric:rollout_contract_lint_total{{proposal_id=\"{proposal_id}\",status=\"{status}\",failure_reason=\"{}\"}}=1",
            sanitize_metric_label(failure_reason)
        ));
    }
    diagnostics.push(format!(
        "metric:rollout_contract_enforcement_mode_total{{proposal_id=\"{proposal_id}\",mode=\"{enforcement_mode}\"}}=1"
    ));
    if enforcement_mode == "permissive" {
        diagnostics.push(format!(
            "metric:rollout_contract_permissive_dogfood_total{{proposal_id=\"{proposal_id}\",status=\"{status}\",would_block=\"{would_block}\"}}=1"
        ));
    }
    if enforcement_mode == "enforce" && would_block {
        diagnostics.push(format!(
            "metric:rollout_contract_run_start_block_total{{proposal_id=\"{proposal_id}\",reason=\"{}\",enforcement_mode=\"enforce\"}}=1",
            sanitize_metric_label(
                failure_reasons
                    .first()
                    .map(String::as_str)
                    .unwrap_or("rollout_contract_hold")
            )
        ));
    }
    diagnostics.extend(
        declared_operational_metrics
            .iter()
            .map(|metric| format!("declared_operational_metric:{metric}")),
    );
    diagnostics
}

fn append_rollout_contract_operational_metric_diagnostics(
    diagnostics: &mut Vec<String>,
    proposal_id: &str,
    status: &RolloutContractStatus,
    enforcement_mode: &RolloutContractEnforcementMode,
    failure_reasons: &[String],
    declared_operational_metrics: &[String],
) {
    diagnostics.extend(rollout_contract_operational_metric_diagnostics(
        proposal_id,
        status,
        enforcement_mode,
        failure_reasons,
        declared_operational_metrics,
    ));
    if status == &RolloutContractStatus::Waived {
        diagnostics.push(format!(
            "metric:rollout_contract_waiver_total{{proposal_id=\"{proposal_id}\",reason=\"{}\",waiver_state=\"active\"}}=1",
            sanitize_metric_label(
                failure_reasons
                    .first()
                    .map(String::as_str)
                    .unwrap_or("scheduling_time_waiver")
            )
        ));
    }
    if status == &RolloutContractStatus::Cancelled {
        diagnostics.push(format!(
            "metric:rollout_contract_preflight_cancelled_total{{proposal_id=\"{proposal_id}\",reason=\"{}\"}}=1",
            sanitize_metric_label(
                failure_reasons
                    .first()
                    .map(String::as_str)
                    .unwrap_or("rollout_contract_preflight_cancelled")
            )
        ));
    }
    if failure_reasons
        .iter()
        .any(|reason| reason == "rollout_contract_preflight_retry_exhausted")
    {
        diagnostics.push(format!(
            "metric:rollout_contract_retry_exhausted_total{{proposal_id=\"{proposal_id}\",failure_class=\"infrastructure\"}}=1"
        ));
    }
    if failure_reasons.iter().any(|reason| {
        reason == "stale_rollout_contract_projection"
            || reason == "tamper_detected_rollout_contract_projection"
    }) {
        diagnostics.push(format!(
            "metric:rollout_contract_tamper_or_stale_projection_total{{proposal_id=\"{proposal_id}\",projection_integrity=\"{}\"}}=1",
            if failure_reasons
                .iter()
                .any(|reason| reason == "tamper_detected_rollout_contract_projection")
            {
                "tamper_suspect"
            } else {
                "stale"
            }
        ));
    }
}

fn sanitize_metric_label(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | ':' => ch,
            _ => '_',
        })
        .collect()
}

fn bounded_diagnostic(message: String) -> String {
    const MAX_DIAGNOSTIC_CHARS: usize = 768;
    if message.chars().count() <= MAX_DIAGNOSTIC_CHARS {
        return message;
    }
    let mut bounded = message
        .chars()
        .take(MAX_DIAGNOSTIC_CHARS.saturating_sub(12))
        .collect::<String>();
    bounded.push_str("...<truncated>");
    bounded
}

fn scrub_diagnostic_for_workspace(message: String, workspace_root: &str) -> (String, bool) {
    let mut scrubbed = false;
    let mut output = message;
    let trimmed = workspace_root.trim_end_matches('/');
    if !trimmed.is_empty() && output.contains(trimmed) {
        output = output.replace(trimmed, "<workspace>");
        scrubbed = true;
    }
    (bounded_diagnostic(output), scrubbed)
}

fn scrub_diagnostics_for_run(run: &Run, diagnostics: Vec<String>) -> (Vec<String>, String) {
    let mut redacted = false;
    let diagnostics = diagnostics
        .into_iter()
        .map(|diagnostic| {
            let (diagnostic, scrubbed) =
                scrub_diagnostic_for_workspace(diagnostic, &run.workspace_root);
            redacted |= scrubbed;
            diagnostic
        })
        .collect();
    let redaction_state = if redacted { "partial" } else { "none" }.to_string();
    (diagnostics, redaction_state)
}

async fn upsert_linted_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: &Artifact,
    policy: &EffectivePreflightPolicy,
    retry_count: i64,
) -> Result<Option<StoredRolloutContractCheck>> {
    let now = Utc::now();
    let metadata = proposal_metadata_from_artifact(run, approved_proposal)?
        .unwrap_or_else(ProposalMetadata::unknown);
    let proposal_path = match safe_artifact_path(&approved_proposal.file_path, &run.workspace_root)
    {
        Ok(path) => path,
        Err(error) => {
            return upsert_invalid_approved_proposal_contract_check(
                pool,
                run,
                approved_proposal,
                policy.enforcement_mode.clone(),
                retry_count,
                vec!["unsafe_approved_proposal_artifact_path".to_string()],
                vec![bounded_diagnostic(format!(
                    "approved proposal artifact path rejected: {error}"
                ))],
            )
            .await
            .map(Some)
        }
    };
    let data = match read_bounded_rollout_contract_input(&proposal_path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return upsert_invalid_approved_proposal_contract_check(
                pool,
                run,
                approved_proposal,
                policy.enforcement_mode.clone(),
                retry_count,
                vec!["invalid_approved_proposal_artifact".to_string()],
                vec![bounded_diagnostic(format!(
                    "read approved proposal artifact {}: {error}",
                    proposal_path.display()
                ))],
            )
            .await
            .map(Some)
        }
    };
    let proposal_value: serde_json::Value = match serde_json::from_slice(&data) {
        Ok(value) => value,
        Err(error) => {
            return upsert_invalid_approved_proposal_contract_check(
                pool,
                run,
                approved_proposal,
                policy.enforcement_mode.clone(),
                retry_count,
                vec!["invalid_approved_proposal_artifact".to_string()],
                vec![bounded_diagnostic(format!(
                    "parse approved proposal artifact {}: {error}",
                    proposal_path.display()
                ))],
            )
            .await
            .map(Some)
        }
    };

    let Some(contract_source) =
        extract_rollout_contract_source(&proposal_value, &proposal_path, &run.workspace_root)?
    else {
        return Ok(None);
    };
    let mut lint = lint_rollout_contract(&contract_source.contract, &run.workspace_root);
    lint.failures.extend(contract_source.extraction_failures);
    let cutover = effective_cutover_decision(run, &lint, policy);
    let enforcement_mode = cutover.enforcement_mode;
    let (status, decision, failure_reasons, diagnostics) = if lint.failures.is_empty() {
        match (
            cutover.grandfathered_not_applicable,
            lint.applicability.as_deref(),
        ) {
            (true, _) => (
                RolloutContractStatus::NotApplicable,
                RolloutContractDecision::NotApplicable,
                Vec::new(),
                vec![format!(
                    "rollout_contract_v1 grandfathered before cutover source={}",
                    contract_source.source
                )],
            ),
            (_, Some("not_applicable")) => (
                RolloutContractStatus::NotApplicable,
                RolloutContractDecision::NotApplicable,
                Vec::new(),
                vec![format!(
                    "rollout_contract_v1 not_applicable source={}",
                    contract_source.source
                )],
            ),
            (_, _) => (
                RolloutContractStatus::Pass,
                RolloutContractDecision::Release,
                Vec::new(),
                vec![format!(
                    "rollout_contract_v1 passed source={}",
                    contract_source.source
                )],
            ),
        }
    } else {
        (
            RolloutContractStatus::Fail,
            RolloutContractDecision::Hold,
            lint.failures.clone(),
            vec![format!(
                "rollout_contract_v1 failed source={} failures={}",
                contract_source.source,
                lint.failures.len()
            )],
        )
    };

    let mut diagnostics = diagnostics;
    diagnostics.extend(cutover.diagnostics);
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &lint.operational_metrics,
    );

    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: proposal_content_hash(approved_proposal, &data),
        contract_object_hash: hash_json_value(&contract_source.contract),
        content_snapshot_id: approved_proposal.id.to_string(),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: lint.rollback_disposition,
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: lint.cutover_policy_revision,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now)
        .await
        .map(Some)
}

async fn upsert_disabled_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    policy: &EffectivePreflightPolicy,
    retry_count: i64,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::NotApplicable;
    let decision = RolloutContractDecision::NotApplicable;
    let failure_reasons = vec![];
    let mut diagnostics = disabled_policy_diagnostics(policy);
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &RolloutContractEnforcementMode::Disabled,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .and_then(|artifact| artifact.checksum_sha256.clone())
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal
            .map(|artifact| artifact.id.to_string())
            .unwrap_or_else(|| metadata.content_snapshot_id),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode: RolloutContractEnforcementMode::Disabled,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: None,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

fn disabled_policy_diagnostics(policy: &EffectivePreflightPolicy) -> Vec<String> {
    let mut diagnostics = if policy.diagnostics.is_empty() {
        vec!["rollout contract enforcement mode disabled".to_string()]
    } else {
        policy.diagnostics.clone()
    };
    if let Some(reason_code) = &policy.enforcement_mode_reason_code {
        diagnostics.push(format!(
            "rollout_contract_enforcement_mode_reason={reason_code}"
        ));
    }
    diagnostics
}

async fn upsert_waived_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    enforcement_mode: RolloutContractEnforcementMode,
    waiver: serde_json::Value,
    retry_count: i64,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::Waived;
    let decision = RolloutContractDecision::Waive;
    let failure_reasons = vec![];
    let mut diagnostics = vec!["scheduling-time rollout contract waiver accepted".to_string()];
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .and_then(|artifact| artifact.checksum_sha256.clone())
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal
            .map(|artifact| artifact.id.to_string())
            .unwrap_or_else(|| metadata.content_snapshot_id),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: Some(waiver),
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: None,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

async fn upsert_policy_failure_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    enforcement_mode: RolloutContractEnforcementMode,
    policy: &EffectivePreflightPolicy,
    retry_count: i64,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::Fail;
    let decision = RolloutContractDecision::Hold;
    let failure_reasons = policy.failures.clone();
    let mut diagnostics = if policy.diagnostics.is_empty() {
        vec!["invalid rollout contract run-start preflight policy".to_string()]
    } else {
        policy.diagnostics.clone()
    };
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .and_then(|artifact| artifact.checksum_sha256.clone())
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal
            .map(|artifact| artifact.id.to_string())
            .unwrap_or_else(|| metadata.content_snapshot_id),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: None,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

async fn upsert_missing_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    enforcement_mode: RolloutContractEnforcementMode,
    retry_count: i64,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::MissingContract;
    let decision = RolloutContractDecision::Hold;
    let failure_reasons = vec!["missing_rollout_contract_check".to_string()];
    let mut diagnostics = vec![
        "implementation_run_start_rollout_contract_preflight did not find a terminal rollout_contract_check_v1 record"
            .to_string(),
    ];
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .and_then(|artifact| artifact.checksum_sha256.clone())
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal
            .map(|artifact| artifact.id.to_string())
            .unwrap_or_else(|| metadata.content_snapshot_id),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Stale,
        cutover_policy_revision: None,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

async fn upsert_invalid_approved_proposal_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: &Artifact,
    enforcement_mode: RolloutContractEnforcementMode,
    retry_count: i64,
    failure_reasons: Vec<String>,
    diagnostics: Vec<String>,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = proposal_metadata_from_artifact(run, approved_proposal)?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::Fail;
    let decision = RolloutContractDecision::Hold;
    let mut diagnostics = diagnostics;
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .checksum_sha256
            .clone()
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal.id.to_string(),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: None,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

async fn upsert_timeout_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    enforcement_mode: RolloutContractEnforcementMode,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::Timeout;
    let decision = RolloutContractDecision::Hold;
    let failure_reasons = vec!["rollout_contract_preflight_timeout".to_string()];
    let mut diagnostics = vec![format!(
        "implementation_run_start_rollout_contract_preflight exceeded {PREFLIGHT_TIMEOUT_SECONDS}s"
    )];
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .and_then(|artifact| artifact.checksum_sha256.clone())
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal
            .map(|artifact| artifact.id.to_string())
            .unwrap_or_else(|| metadata.content_snapshot_id),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: None,
        redaction_state,
        retry_count: 0,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

async fn upsert_retry_exhausted_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    enforcement_mode: RolloutContractEnforcementMode,
    retry_count: i64,
    last_error: &anyhow::Error,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::Fail;
    let decision = RolloutContractDecision::Hold;
    let failure_reasons = vec!["rollout_contract_preflight_retry_exhausted".to_string()];
    let mut diagnostics = vec![format!(
        "implementation_run_start_rollout_contract_preflight exhausted {retry_count} infrastructure retries: {last_error}"
    )];
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .and_then(|artifact| artifact.checksum_sha256.clone())
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal
            .map(|artifact| artifact.id.to_string())
            .unwrap_or_else(|| metadata.content_snapshot_id),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: None,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

async fn upsert_cancelled_contract_check(
    pool: &SqlitePool,
    run: &Run,
    approved_proposal: Option<&Artifact>,
    enforcement_mode: RolloutContractEnforcementMode,
    retry_count: i64,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let metadata = approved_proposal
        .and_then(|artifact| proposal_metadata_from_artifact(run, artifact).transpose())
        .transpose()?
        .unwrap_or_else(ProposalMetadata::unknown);
    let status = RolloutContractStatus::Cancelled;
    let decision = RolloutContractDecision::Hold;
    let failure_reasons = vec!["rollout_contract_preflight_cancelled".to_string()];
    let mut diagnostics = vec![format!(
        "implementation_run_start_rollout_contract_preflight observed cancellation_requested_at={}",
        run.cancellation_requested_at
            .map(|timestamp| timestamp.to_rfc3339())
            .unwrap_or_else(|| "unknown".to_string())
    )];
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &metadata.proposal_id,
        &status,
        &enforcement_mode,
        &failure_reasons,
        &[],
    );
    let (diagnostics, redaction_state) = scrub_diagnostics_for_run(run, diagnostics);
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: run.id.inner(),
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: approved_proposal
            .and_then(|artifact| artifact.checksum_sha256.clone())
            .map(|hash| format!("sha256:{hash}"))
            .unwrap_or_else(|| DEFAULT_PROPOSAL_CONTENT_HASH.to_string()),
        contract_object_hash: metadata.contract_object_hash,
        content_snapshot_id: approved_proposal
            .map(|artifact| artifact.id.to_string())
            .unwrap_or_else(|| metadata.content_snapshot_id),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode,
        failure_reasons,
        diagnostics,
        waiver: None,
        rollback_disposition: default_rollback_disposition(),
        projection_integrity: ProjectionIntegrity::Valid,
        cutover_policy_revision: None,
        redaction_state,
        retry_count,
        preflight_timeout_seconds: PREFLIGHT_TIMEOUT_SECONDS,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

#[derive(Clone, Debug)]
struct ProjectionDrift {
    status: RolloutContractStatus,
    projection_integrity: ProjectionIntegrity,
    failure_reason: String,
    diagnostics: Vec<String>,
}

async fn upsert_projection_integrity_check(
    pool: &SqlitePool,
    previous: &StoredRolloutContractCheck,
    drift: ProjectionDrift,
    retry_count: i64,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let status = drift.status;
    let failure_reasons = vec![drift.failure_reason];
    let mut diagnostics = drift.diagnostics;
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &previous.proposal_id,
        &status,
        &previous.enforcement_mode,
        &failure_reasons,
        &[],
    );
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: previous.run_id,
        proposal_id: previous.proposal_id.clone(),
        proposal_revision_id: previous.proposal_revision_id.clone(),
        proposal_content_hash: previous.proposal_content_hash.clone(),
        contract_object_hash: previous.contract_object_hash.clone(),
        content_snapshot_id: previous.content_snapshot_id.clone(),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision: RolloutContractDecision::Hold,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode: previous.enforcement_mode.clone(),
        failure_reasons,
        diagnostics,
        waiver: previous.waiver.clone(),
        rollback_disposition: previous.rollback_disposition.clone(),
        projection_integrity: drift.projection_integrity,
        cutover_policy_revision: previous.cutover_policy_revision.clone(),
        redaction_state: previous.redaction_state.clone(),
        retry_count: previous.retry_count.max(retry_count),
        preflight_timeout_seconds: previous.preflight_timeout_seconds,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

async fn upsert_hash_drift_contract_check(
    pool: &SqlitePool,
    previous: &StoredRolloutContractCheck,
    drift: &ContractIdentityDrift,
    retry_count: i64,
) -> Result<StoredRolloutContractCheck> {
    let now = Utc::now();
    let status = RolloutContractStatus::Stale;
    let failure_reasons = vec!["stale_rollout_contract_check_hash_drift".to_string()];
    let mut diagnostics = vec![format!(
        "rollout_contract_check terminal record does not match current approved proposal artifact mismatches={}",
        drift.mismatches.join(",")
    )];
    append_rollout_contract_operational_metric_diagnostics(
        &mut diagnostics,
        &drift.identity.proposal_id,
        &status,
        &previous.enforcement_mode,
        &failure_reasons,
        &[],
    );
    let input = UpsertRolloutContractCheck {
        id: Uuid::new_v4(),
        run_id: previous.run_id,
        proposal_id: drift.identity.proposal_id.clone(),
        proposal_revision_id: drift.identity.proposal_revision_id.clone(),
        proposal_content_hash: drift.identity.proposal_content_hash.clone(),
        contract_object_hash: drift.identity.contract_object_hash.clone(),
        content_snapshot_id: drift.identity.content_snapshot_id.clone(),
        checker_version: CHECKER_VERSION.to_string(),
        status,
        decision: RolloutContractDecision::Hold,
        lifecycle_state: RolloutContractLifecycleState::Terminal,
        enforcement_mode: previous.enforcement_mode.clone(),
        failure_reasons,
        diagnostics,
        waiver: previous.waiver.clone(),
        rollback_disposition: previous.rollback_disposition.clone(),
        projection_integrity: ProjectionIntegrity::Stale,
        cutover_policy_revision: previous.cutover_policy_revision.clone(),
        redaction_state: previous.redaction_state.clone(),
        retry_count: previous.retry_count.max(retry_count),
        preflight_timeout_seconds: previous.preflight_timeout_seconds,
    };
    rollout_contract_checks::upsert_rollout_contract_check(pool, &input, now).await
}

#[derive(Clone, Debug)]
struct RolloutContractSource {
    contract: serde_json::Value,
    source: String,
    extraction_failures: Vec<String>,
}

pub(crate) fn approved_proposal_rollout_contract_lint_failures(
    data: &[u8],
    proposal_path: &std::path::Path,
    workspace_root: &str,
) -> Result<Vec<String>> {
    let proposal_value: serde_json::Value = match serde_json::from_slice(data) {
        Ok(value) => value,
        Err(_) => return Ok(vec!["invalid_approved_proposal_artifact".to_string()]),
    };
    let Some(contract_source) =
        extract_rollout_contract_source(&proposal_value, proposal_path, workspace_root)?
    else {
        return Ok(vec!["missing_rollout_contract_check".to_string()]);
    };
    let mut lint = lint_rollout_contract(&contract_source.contract, workspace_root);
    lint.failures.extend(contract_source.extraction_failures);
    Ok(lint.failures)
}

#[derive(Clone, Debug)]
struct RolloutContractLint {
    failures: Vec<String>,
    applicability: Option<String>,
    operational_metrics: Vec<String>,
    cutover_policy_revision: Option<String>,
    cutover_enforcement_mode: Option<RolloutContractEnforcementMode>,
    cutover_effective_at: Option<DateTime<Utc>>,
    cutover_applicable_to: Option<String>,
    cutover_grandfathered_rendering: Option<String>,
    rollback_disposition: serde_json::Value,
}

impl Default for RolloutContractLint {
    fn default() -> Self {
        Self {
            failures: Vec::new(),
            applicability: None,
            operational_metrics: Vec::new(),
            cutover_policy_revision: None,
            cutover_enforcement_mode: None,
            cutover_effective_at: None,
            cutover_applicable_to: None,
            cutover_grandfathered_rendering: None,
            rollback_disposition: default_rollback_disposition(),
        }
    }
}

#[derive(Clone, Debug)]
struct EffectiveCutoverDecision {
    enforcement_mode: RolloutContractEnforcementMode,
    grandfathered_not_applicable: bool,
    diagnostics: Vec<String>,
}

fn effective_cutover_decision(
    run: &Run,
    lint: &RolloutContractLint,
    policy: &EffectivePreflightPolicy,
) -> EffectiveCutoverDecision {
    if policy.enforcement_mode_supplied {
        return EffectiveCutoverDecision {
            enforcement_mode: policy.enforcement_mode.clone(),
            grandfathered_not_applicable: false,
            diagnostics: Vec::new(),
        };
    }

    let Some(effective_at) = lint.cutover_effective_at else {
        return EffectiveCutoverDecision {
            enforcement_mode: policy.enforcement_mode.clone(),
            grandfathered_not_applicable: false,
            diagnostics: vec!["rollout_contract_default_enforcement_mode=enforce".to_string()],
        };
    };

    let applies_to_post_cutover_starts =
        lint.cutover_applicable_to.as_deref() == Some("post_cutover_implementation_starts");
    if applies_to_post_cutover_starts && run.started_at < effective_at {
        return EffectiveCutoverDecision {
            enforcement_mode: policy.enforcement_mode.clone(),
            grandfathered_not_applicable: lint.cutover_grandfathered_rendering.as_deref()
                == Some("not_applicable"),
            diagnostics: vec![format!(
                "rollout_contract_cutover_grandfathered_until={}",
                effective_at.to_rfc3339()
            )],
        };
    }

    let enforcement_mode = lint
        .cutover_enforcement_mode
        .clone()
        .unwrap_or_else(|| policy.enforcement_mode.clone());
    EffectiveCutoverDecision {
        enforcement_mode: enforcement_mode.clone(),
        grandfathered_not_applicable: false,
        diagnostics: vec![format!(
            "rollout_contract_cutover_applied revision={} mode={}",
            lint.cutover_policy_revision.as_deref().unwrap_or("unknown"),
            enforcement_mode
        )],
    }
}

fn extract_rollout_contract_source(
    proposal: &serde_json::Value,
    proposal_path: &std::path::Path,
    workspace_root: &str,
) -> Result<Option<RolloutContractSource>> {
    let Some(object) = proposal.as_object() else {
        return Ok(None);
    };
    let has_inline = object.contains_key("rollout_contract_v1")
        || object.contains_key("p084_self_contract")
        || object
            .get("schema_version")
            .and_then(|value| value.as_str())
            == Some(ROLLOUT_CONTRACT_SCHEMA_VERSION);
    let has_sidecar = object.contains_key("rollout_contract_sidecar");
    if has_inline && has_sidecar {
        return Ok(Some(extraction_failure_contract(
            vec![
                "duplicate_source: inline rollout contract and rollout_contract_sidecar keys are both present"
                    .to_string(),
            ],
            "duplicate_source",
        )));
    }

    if object
        .get("schema_version")
        .and_then(|value| value.as_str())
        == Some(ROLLOUT_CONTRACT_SCHEMA_VERSION)
    {
        return Ok(Some(RolloutContractSource {
            contract: proposal.clone(),
            source: proposal_path.display().to_string(),
            extraction_failures: Vec::new(),
        }));
    }

    if let Some(inline) = object.get("rollout_contract_v1") {
        return Ok(Some(RolloutContractSource {
            contract: inline.clone(),
            source: format!("{}#rollout_contract_v1", proposal_path.display()),
            extraction_failures: Vec::new(),
        }));
    }

    if let Some(self_contract) = object.get("p084_self_contract") {
        return Ok(Some(RolloutContractSource {
            contract: normalize_p084_self_contract(object, self_contract),
            source: format!("{}#p084_self_contract", proposal_path.display()),
            extraction_failures: Vec::new(),
        }));
    }

    let Some(sidecar) = object.get("rollout_contract_sidecar") else {
        return Ok(None);
    };
    let Some(sidecar_path) = sidecar.as_str() else {
        return Ok(Some(extraction_failure_contract(
            vec![
                "invalid_rollout_contract_sidecar: rollout_contract_sidecar must be a string"
                    .to_string(),
            ],
            "invalid_sidecar",
        )));
    };
    let path_failures = check_path_safety_under_root(
        sidecar_path,
        "rollout_contract_sidecar",
        workspace_root,
        &["docs/proposals", "docs/evidence/rollout-contract"],
    );
    if !path_failures.is_empty() {
        return Ok(Some(extraction_failure_contract(
            path_failures,
            "unsafe_sidecar",
        )));
    }

    let sidecar_abs = match canonical_repo_path(
        sidecar_path,
        "rollout_contract_sidecar",
        workspace_root,
        &["docs/proposals", "docs/evidence/rollout-contract"],
    ) {
        Ok(path) => path,
        Err(failures) => {
            return Ok(Some(extraction_failure_contract(
                failures,
                "unsafe_sidecar",
            )))
        }
    };
    let data = match read_bounded_rollout_contract_input(&sidecar_abs) {
        Ok(data) => data,
        Err(error) => {
            return Ok(Some(extraction_failure_contract(
                vec![format!(
                    "sidecar_read_error: {}: {error}",
                    sidecar_abs.display()
                )],
                "sidecar_read_error",
            )))
        }
    };
    let contract: serde_json::Value = match serde_json::from_slice(&data) {
        Ok(contract) => contract,
        Err(error) => {
            return Ok(Some(extraction_failure_contract(
                vec![format!("sidecar_invalid_json: {error}")],
                "sidecar_invalid_json",
            )))
        }
    };
    Ok(Some(RolloutContractSource {
        contract,
        source: sidecar_path.to_string(),
        extraction_failures: Vec::new(),
    }))
}

fn extraction_failure_contract(failures: Vec<String>, source: &str) -> RolloutContractSource {
    RolloutContractSource {
        contract: serde_json::json!({
            "schema_version": ROLLOUT_CONTRACT_SCHEMA_VERSION,
            "applicability": "required"
        }),
        extraction_failures: failures,
        source: source.to_string(),
    }
}

fn normalize_p084_self_contract(
    proposal: &serde_json::Map<String, serde_json::Value>,
    self_contract: &serde_json::Value,
) -> serde_json::Value {
    let mut contract = serde_json::Map::new();
    contract.insert(
        "schema_version".to_string(),
        serde_json::json!(ROLLOUT_CONTRACT_SCHEMA_VERSION),
    );

    if let Some(applicability) = self_contract.get("applicability").cloned() {
        contract.insert("applicability".to_string(), applicability);
    }
    if let Some(gate_aliases) = self_contract.get("gate_aliases").cloned() {
        contract.insert("gate_aliases".to_string(), gate_aliases);
    }
    if let Some(readback_fixture) = self_contract.get("readback_fixture").cloned() {
        contract.insert("readback_fixture".to_string(), readback_fixture);
    }
    if let Some(readback_lanes) = self_contract.get("readback_lanes").cloned() {
        contract.insert("readback_lanes".to_string(), readback_lanes);
    }
    if let Some(required_report_fields) = self_contract.get("required_report_fields").cloned() {
        contract.insert(
            "readback_fields".to_string(),
            required_report_fields.clone(),
        );
        contract.insert("operator_report_fields".to_string(), required_report_fields);
    }
    if let Some(rollback_disposition) = self_contract.get("rollback_disposition").cloned() {
        contract.insert("rollback_disposition".to_string(), rollback_disposition);
    }

    contract.insert("hold_conditions".to_string(), serde_json::json!([]));
    contract.insert(
        "commands".to_string(),
        serde_json::json!({
            "allowlist": ["./scripts/test-gate.sh proposal-084"]
        }),
    );
    contract.insert(
        "migrations".to_string(),
        serde_json::json!({
            "not_applicable": true,
            "justification": "P084 self-contract normalization does not require an additional migration."
        }),
    );
    contract.insert(
        "decision_vocabulary".to_string(),
        serde_json::json!(ALLOWED_DECISION_VOCABULARY),
    );
    if let Some(metrics) = normalized_p084_metrics(proposal) {
        contract.insert("metrics".to_string(), metrics);
    }
    let negative_fixtures = normalized_p084_negative_fixtures(proposal);
    if negative_fixtures
        .as_object()
        .is_some_and(|fixtures| !fixtures.is_empty())
    {
        contract.insert("negative_fixtures".to_string(), negative_fixtures);
    }

    serde_json::Value::Object(contract)
}

fn normalized_p084_metrics(
    proposal: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let metrics = proposal.get("metrics")?.as_object()?;
    let mut normalized = serde_json::Map::new();
    if let Some(adoption_metric) = metrics.get("adoption_metric") {
        let adoption_name = adoption_metric
            .as_object()
            .and_then(|object| object.get("name"))
            .and_then(|value| value.as_str())
            .or_else(|| adoption_metric.as_str());
        if let Some(name) = adoption_name {
            normalized.insert("adoption_metric".to_string(), serde_json::json!(name));
        }
    }
    if let Some(operational_metrics) = metrics.get("operational_metrics") {
        normalized.insert(
            "operational_metrics".to_string(),
            operational_metrics.clone(),
        );
    }
    Some(serde_json::Value::Object(normalized))
}

fn normalized_p084_negative_fixtures(
    proposal: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let mut fixtures = serde_json::Map::new();
    if let Some(criteria) = proposal
        .get("acceptance_criteria")
        .and_then(|value| value.as_array())
    {
        for criterion in criteria {
            let Some(id) = criterion.get("id").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(path) = criterion
                .get("negative_fixture")
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            fixtures.insert(id.to_string(), serde_json::json!(path));
        }
    }
    serde_json::Value::Object(fixtures)
}

fn lint_rollout_contract(
    contract: &serde_json::Value,
    workspace_root: &str,
) -> RolloutContractLint {
    let Some(object) = contract.as_object() else {
        return RolloutContractLint {
            failures: vec!["invalid_json: top-level value must be a JSON object".to_string()],
            applicability: None,
            ..Default::default()
        };
    };
    let mut failures = Vec::new();
    let mut operational_metrics = Vec::new();
    let mut cutover_policy_revision = None;
    let mut cutover_enforcement_mode = None;
    let mut cutover_effective_at = None;
    let mut cutover_applicable_to = None;
    let mut cutover_grandfathered_rendering = None;
    let mut rollback_disposition = default_rollback_disposition();
    failures.extend(check_unknown_fields(
        object,
        ALLOWED_TOP_LEVEL_FIELDS,
        "rollout_contract_v1",
    ));
    failures.extend(check_secret_like_values(contract, "rollout_contract_v1"));

    match object
        .get("schema_version")
        .and_then(|value| value.as_str())
    {
        Some(ROLLOUT_CONTRACT_SCHEMA_VERSION) => {}
        Some(other) => failures.push(format!(
            "invalid_schema_version: expected {ROLLOUT_CONTRACT_SCHEMA_VERSION:?}, got {other:?}"
        )),
        None => {
            failures.push("missing_schema_version: schema_version field is required".to_string())
        }
    }

    let applicability = object
        .get("applicability")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    match applicability.as_deref() {
        Some("required") => {}
        Some("not_applicable") => {
            if object
                .get("not_applicable_justification")
                .or_else(|| object.get("not_applicable_justifications"))
                .is_none()
            {
                failures.push(
                    "missing_not_applicable_justification: applicability=not_applicable requires not_applicable_justification"
                        .to_string(),
                );
            }
            return RolloutContractLint {
                failures,
                applicability,
                ..Default::default()
            };
        }
        Some(other) => failures.push(format!(
            "invalid_applicability: {other:?} not in [\"not_applicable\", \"required\"]"
        )),
        None => failures.push("missing_applicability: applicability field is required".to_string()),
    }

    if applicability.as_deref() != Some("required") {
        return RolloutContractLint {
            failures,
            applicability,
            ..Default::default()
        };
    }

    for field in REQUIRED_WHEN_APPLICABLE {
        if !object.contains_key(*field) {
            failures.push(format!(
                "missing_{field}: {field} is required when applicability=required"
            ));
        }
    }

    for field in ["gate_aliases", "readback_lanes"] {
        if let Some(value) = object.get(field) {
            failures.extend(check_string_array(value, field, true));
        }
    }

    if let Some(lanes) = object
        .get("readback_lanes")
        .and_then(|value| value.as_array())
    {
        for (idx, lane) in lanes.iter().enumerate() {
            if let Some(lane) = lane.as_str() {
                if !ALLOWED_READBACK_LANES.contains(&lane) {
                    failures.push(format!(
                        "invalid_readback_lanes[{idx}]: {lane:?} not in {ALLOWED_READBACK_LANES:?}"
                    ));
                }
            }
        }
    }

    for field in ["readback_fields", "operator_report_fields"] {
        if let Some(value) = object.get(field) {
            failures.extend(check_string_array(value, field, true));
            if let Some(entries) = value.as_array() {
                for required in REQUIRED_OPERATOR_READBACK_FIELDS {
                    if !entries
                        .iter()
                        .any(|entry| entry.as_str() == Some(*required))
                    {
                        failures.push(format!(
                            "missing_{field}.{required}: {field} must include operator_readback_v1 field {required}"
                        ));
                    }
                }
            }
        }
    }

    if let Some(value) = object.get("hold_conditions") {
        failures.extend(check_string_array(value, "hold_conditions", false));
    }
    if let Some(value) = object.get("hold_conditions_detail") {
        failures.extend(check_string_array(value, "hold_conditions_detail", false));
    }

    match object.get("rollback_disposition") {
        Some(value) if value.is_object() => {
            rollback_disposition = value.clone();
            let rollback = value.as_object().expect("checked object");
            failures.extend(check_unknown_fields(
                rollback,
                ALLOWED_ROLLBACK_FIELDS,
                "rollback_disposition",
            ));
            for subfield in ROLLBACK_DISPOSITION_REQUIRED {
                if !rollback.contains_key(*subfield) {
                    failures.push(format!(
                        "missing_rollback_disposition.{subfield}: rollback_disposition.{subfield} is required"
                    ));
                }
            }
            if let Some(risk) = rollback
                .get("data_loss_risk")
                .and_then(|value| value.as_str())
            {
                if !ALLOWED_DATA_LOSS_RISK.contains(&risk) {
                    failures.push(format!(
                        "invalid_rollback_disposition.data_loss_risk: {risk:?} not in {ALLOWED_DATA_LOSS_RISK:?}"
                    ));
                }
            }
            if let Some(steps) = rollback.get("steps") {
                failures.extend(check_string_array(
                    steps,
                    "rollback_disposition.steps",
                    false,
                ));
            }
        }
        Some(_) => failures.push(
            "invalid_rollback_disposition: rollback_disposition must be an object".to_string(),
        ),
        None => {}
    }

    match object.get("metrics") {
        Some(value) if value.is_object() => {
            let metrics = value.as_object().expect("checked object");
            failures.extend(check_unknown_fields(
                metrics,
                ALLOWED_METRIC_FIELDS,
                "metrics",
            ));
            if !metrics.contains_key("adoption_metric")
                && !metrics.contains_key("operational_metrics")
            {
                failures.push(
                    "missing_metrics_content: metrics must define adoption_metric or operational_metrics"
                        .to_string(),
                );
            }
            if let Some(adoption) = metrics.get("adoption_metric") {
                if !adoption.is_string() {
                    failures.push("invalid_metrics.adoption_metric: must be a string".to_string());
                }
            }
            if let Some(operational) = metrics.get("operational_metrics") {
                failures.extend(check_string_array(
                    operational,
                    "metrics.operational_metrics",
                    true,
                ));
                if let Some(entries) = operational.as_array() {
                    operational_metrics.extend(
                        entries
                            .iter()
                            .filter_map(|entry| entry.as_str().map(ToString::to_string)),
                    );
                }
            }
        }
        Some(_) => failures.push("invalid_metrics: metrics must be an object".to_string()),
        None => {}
    }

    if let Some(commands) = object.get("commands") {
        match commands.as_object() {
            Some(commands) => {
                failures.extend(check_unknown_fields(
                    commands,
                    ALLOWED_COMMAND_FIELDS,
                    "commands",
                ));
                match commands.get("allowlist") {
                    Some(value) => match value.as_array() {
                        Some(allowlist) => {
                            for (idx, cmd) in allowlist.iter().enumerate() {
                                match cmd.as_str() {
                                    Some(cmd) => failures.extend(check_command_safety(
                                        cmd,
                                        &format!("commands.allowlist[{idx}]"),
                                    )),
                                    None => failures.push(format!(
                                        "invalid_commands.allowlist[{idx}]: entry must be a string"
                                    )),
                                }
                            }
                        }
                        None => failures
                            .push("invalid_commands.allowlist: must be an array".to_string()),
                    },
                    None => {}
                }
                if let Some(commentary) = commands.get("commentary") {
                    if !commentary.is_string() {
                        failures.push("invalid_commands.commentary: must be a string".to_string());
                    }
                }
            }
            None => failures.push("invalid_commands: commands must be an object".to_string()),
        }
    }

    if let Some(migrations) = object.get("migrations") {
        match migrations.as_object() {
            Some(migrations) => {
                failures.extend(check_unknown_fields(
                    migrations,
                    ALLOWED_MIGRATION_FIELDS,
                    "migrations",
                ));
                if let Some(not_applicable) = migrations.get("not_applicable") {
                    if !not_applicable.is_boolean() {
                        failures.push(
                            "invalid_migrations.not_applicable: must be a boolean".to_string(),
                        );
                    }
                    if not_applicable.as_bool() == Some(true)
                        && !migrations
                            .get("justification")
                            .and_then(|value| value.as_str())
                            .is_some_and(|value| !value.is_empty())
                    {
                        failures.push(
                            "missing_migrations.justification: migrations.not_applicable=true requires justification"
                                .to_string(),
                        );
                    }
                }
                for field in ["justification", "description"] {
                    if let Some(value) = migrations.get(field) {
                        if !value.is_string() {
                            failures.push(format!("invalid_migrations.{field}: must be a string"));
                        }
                    }
                }
            }
            None => failures.push("invalid_migrations: migrations must be an object".to_string()),
        }
    }

    if let Some(readback_fixture) = object
        .get("readback_fixture")
        .and_then(|value| value.as_str())
    {
        failures.extend(check_fixture_path_safety_under_root(
            readback_fixture,
            "readback_fixture",
            workspace_root,
            &["docs/evidence/rollout-contract"],
        ));
    }
    if let Some(negative_fixtures) = object
        .get("negative_fixtures")
        .and_then(|value| value.as_object())
    {
        for (key, value) in negative_fixtures {
            if let Some(path) = value.as_str() {
                failures.extend(check_fixture_path_safety_under_root(
                    path,
                    &format!("negative_fixtures.{key}"),
                    workspace_root,
                    &["docs/evidence/rollout-contract"],
                ));
            } else {
                failures.push(format!(
                    "invalid_negative_fixtures.{key}: value must be a string path"
                ));
            }
        }
    } else if object.contains_key("negative_fixtures") {
        failures.push("invalid_negative_fixtures: negative_fixtures must be an object".to_string());
    }

    if let Some(decision_vocabulary) = object.get("decision_vocabulary") {
        failures.extend(check_string_array(
            decision_vocabulary,
            "decision_vocabulary",
            true,
        ));
        if let Some(entries) = decision_vocabulary.as_array() {
            for (idx, entry) in entries.iter().enumerate() {
                if let Some(entry) = entry.as_str() {
                    if !ALLOWED_DECISION_VOCABULARY.contains(&entry) {
                        failures.push(format!(
                            "invalid_decision_vocabulary[{idx}]: {entry:?} not in {ALLOWED_DECISION_VOCABULARY:?}"
                        ));
                    }
                }
            }
        }
    }

    if let Some(cutover) = object.get("cutover_policy") {
        match cutover.as_object() {
            Some(cutover) => {
                failures.extend(check_unknown_fields(
                    cutover,
                    ALLOWED_CUTOVER_FIELDS,
                    "cutover_policy",
                ));
                for field in ["revision", "applicable_to"] {
                    if !cutover
                        .get(field)
                        .and_then(|value| value.as_str())
                        .is_some_and(|value| !value.is_empty())
                    {
                        failures.push(format!(
                            "missing_cutover_policy.{field}: must be a non-empty string"
                        ));
                    }
                }
                cutover_policy_revision = cutover
                    .get("revision")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
                cutover_applicable_to = cutover
                    .get("applicable_to")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string);
                if let Some(mode) = cutover
                    .get("enforcement_mode_at_cutover")
                    .and_then(|value| value.as_str())
                {
                    if !ALLOWED_ENFORCEMENT_MODES.contains(&mode) {
                        failures.push(format!(
                            "invalid_cutover_policy.enforcement_mode_at_cutover: {mode:?} not in {ALLOWED_ENFORCEMENT_MODES:?}"
                        ));
                    } else {
                        cutover_enforcement_mode = Some(match mode {
                            "enforce" => RolloutContractEnforcementMode::Enforce,
                            "permissive" => RolloutContractEnforcementMode::Permissive,
                            "disabled" => RolloutContractEnforcementMode::Disabled,
                            _ => unreachable!("validated enforcement mode"),
                        });
                    }
                }
                if let Some(grandfathered) = cutover
                    .get("grandfathered_rendering")
                    .and_then(|value| value.as_str())
                {
                    if grandfathered != "not_applicable" {
                        failures.push(
                            "invalid_cutover_policy.grandfathered_rendering: must be 'not_applicable'"
                                .to_string(),
                        );
                    } else {
                        cutover_grandfathered_rendering = Some(grandfathered.to_string());
                    }
                }
                if let Some(effective_at) = cutover.get("effective_timestamp_iso8601") {
                    match effective_at.as_str() {
                        Some(value) => match chrono::DateTime::parse_from_rfc3339(value) {
                            Ok(parsed) => cutover_effective_at = Some(parsed.with_timezone(&Utc)),
                            Err(_) => failures.push(
                                "invalid_cutover_policy.effective_timestamp_iso8601: must be an ISO-8601 timestamp string"
                                    .to_string(),
                            ),
                        },
                        _ => failures.push(
                            "invalid_cutover_policy.effective_timestamp_iso8601: must be an ISO-8601 timestamp string"
                                .to_string(),
                        ),
                    }
                }
            }
            None => failures
                .push("invalid_cutover_policy: cutover_policy must be an object".to_string()),
        }
    }

    for field in ["operator_message", "not_applicable_justification"] {
        if let Some(value) = object.get(field).and_then(|value| value.as_str()) {
            failures.extend(check_control_chars(value, field));
        }
    }

    RolloutContractLint {
        failures,
        applicability,
        operational_metrics,
        cutover_policy_revision,
        cutover_enforcement_mode,
        cutover_effective_at,
        cutover_applicable_to,
        cutover_grandfathered_rendering,
        rollback_disposition,
    }
}

fn check_path_safety(value: &str, context: &str) -> Vec<String> {
    if value.starts_with('/') {
        return vec![format!(
            "unsafe_path: absolute path in {context}: {value:?}"
        )];
    }
    if std::path::Path::new(value)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return vec![format!(
            "unsafe_path: parent traversal (..) in {context}: {value:?}"
        )];
    }
    Vec::new()
}

fn check_path_safety_under_root(
    value: &str,
    context: &str,
    workspace_root: &str,
    allowed_roots: &[&str],
) -> Vec<String> {
    let mut failures = check_path_safety(value, context);
    if !failures.is_empty() {
        return failures;
    }
    let path = std::path::Path::new(value);
    if !allowed_roots.iter().any(|root| path.starts_with(root)) {
        failures.push(format!(
            "unsafe_path: {context} must be under one of {allowed_roots:?}: {value:?}"
        ));
        return failures;
    }
    if let Err(path_failures) = canonical_repo_path(value, context, workspace_root, allowed_roots) {
        failures.extend(path_failures);
    }
    failures
}

fn check_fixture_path_safety_under_root(
    value: &str,
    context: &str,
    workspace_root: &str,
    allowed_roots: &[&str],
) -> Vec<String> {
    let mut failures = check_path_safety(value, context);
    if !failures.is_empty() {
        return failures;
    }
    let path = std::path::Path::new(value);
    if !allowed_roots.iter().any(|root| path.starts_with(root)) {
        failures.push(format!(
            "unsafe_path: {context} must be under one of {allowed_roots:?}: {value:?}"
        ));
        return failures;
    }
    let candidate = std::path::Path::new(workspace_root).join(path);
    if !candidate.exists() {
        failures.push(format!(
            "missing_fixture_path: {context} does not exist under workspace root: {value:?}"
        ));
        return failures;
    }
    if let Err(path_failures) = canonical_repo_path(value, context, workspace_root, allowed_roots) {
        failures.extend(path_failures);
    }
    failures
}

fn canonical_repo_path(
    value: &str,
    context: &str,
    workspace_root: &str,
    allowed_roots: &[&str],
) -> std::result::Result<std::path::PathBuf, Vec<String>> {
    let failures = check_path_safety(value, context);
    if !failures.is_empty() {
        return Err(failures);
    }
    let relative = std::path::Path::new(value);
    if !allowed_roots.iter().any(|root| relative.starts_with(root)) {
        return Err(vec![format!(
            "unsafe_path: {context} must be under one of {allowed_roots:?}: {value:?}"
        )]);
    }
    let workspace = std::path::Path::new(workspace_root);
    let candidate = workspace.join(relative);
    let canonical_candidate = candidate.canonicalize().map_err(|error| {
        vec![format!(
            "unsafe_path: {context} could not be canonicalized: {value:?}: {error}"
        )]
    })?;
    let mut allowed = Vec::new();
    for root in allowed_roots {
        match workspace.join(root).canonicalize() {
            Ok(canonical) => allowed.push(canonical),
            Err(error) => {
                return Err(vec![format!(
                    "unsafe_path: allowed root {root:?} could not be canonicalized for {context}: {error}"
                )])
            }
        }
    }
    if allowed
        .iter()
        .any(|allowed_root| canonical_candidate.starts_with(allowed_root))
    {
        Ok(canonical_candidate)
    } else {
        Err(vec![format!(
            "unsafe_path: symlink escape or disallowed canonical target in {context}: {value:?}"
        )])
    }
}

fn check_unknown_fields(
    object: &serde_json::Map<String, serde_json::Value>,
    allowed: &[&str],
    context: &str,
) -> Vec<String> {
    object
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .map(|key| format!("unknown_field: {context}.{key} is not allowed"))
        .collect()
}

fn check_string_array(value: &serde_json::Value, context: &str, non_empty: bool) -> Vec<String> {
    let Some(entries) = value.as_array() else {
        return vec![format!("invalid_{context}: must be an array")];
    };
    let mut failures = Vec::new();
    if non_empty && entries.is_empty() {
        failures.push(format!(
            "empty_{context}: {context} must have at least one entry when applicability=required"
        ));
    }
    for (idx, entry) in entries.iter().enumerate() {
        match entry.as_str() {
            Some(entry) => {
                failures.extend(check_control_chars(entry, &format!("{context}[{idx}]")))
            }
            None => failures.push(format!("invalid_{context}[{idx}]: entry must be a string")),
        }
    }
    failures
}

fn check_command_safety(cmd: &str, context: &str) -> Vec<String> {
    const SHELL_METACHARACTERS: &[char] = &['|', '&', ';', '$', '`', '>', '<', '(', ')', '{', '}'];
    let control_failures = check_control_chars(cmd, context);
    if !control_failures.is_empty() {
        return control_failures;
    }
    if let Some(ch) = SHELL_METACHARACTERS.iter().find(|ch| cmd.contains(**ch)) {
        return vec![format!(
            "unsafe_command: shell metacharacter {ch:?} in {context}: {cmd:?}"
        )];
    }
    if cmd.starts_with('/') {
        return vec![format!(
            "unsafe_command: absolute path in {context}: {cmd:?}"
        )];
    }
    if !cmd.starts_with("./scripts/") {
        return vec![format!(
            "unsafe_command: command must start with './scripts/' in {context}: {cmd:?}"
        )];
    }
    Vec::new()
}

fn check_control_chars(value: &str, context: &str) -> Vec<String> {
    for ch in value.chars() {
        let cp = ch as u32;
        if cp < 0x20 && !matches!(ch, '\t' | '\n' | '\r') {
            return vec![format!(
                "control_characters: control character U+{cp:04X} in {context}"
            )];
        }
    }
    Vec::new()
}

fn check_secret_like_values(value: &serde_json::Value, context: &str) -> Vec<String> {
    const MAX_JSON_DEPTH: usize = 64;
    const MAX_JSON_NODES: usize = 10_000;
    let mut failures = Vec::new();
    let mut stack = vec![(value, context.to_string(), 0usize)];
    let mut nodes = 0usize;
    while let Some((value, context, depth)) = stack.pop() {
        nodes += 1;
        if nodes > MAX_JSON_NODES {
            failures.push(format!(
                "json_too_large: {context} exceeds {MAX_JSON_NODES} JSON nodes"
            ));
            break;
        }
        if depth > MAX_JSON_DEPTH {
            failures.push(format!(
                "json_too_deep: {context} exceeds {MAX_JSON_DEPTH} levels"
            ));
            continue;
        }
        match value {
            serde_json::Value::Object(object) => {
                for (key, nested) in object {
                    let nested_context = format!("{context}.{key}");
                    stack.push((nested, nested_context, depth + 1));
                    let lowered = key.to_ascii_lowercase();
                    if ["password", "token", "api_key", "apikey", "secret"]
                        .iter()
                        .any(|marker| lowered.contains(marker))
                    {
                        failures.push(format!("secret_like_field: {context}.{key} is not allowed"));
                    }
                }
            }
            serde_json::Value::Array(entries) => {
                for (idx, nested) in entries.iter().enumerate() {
                    stack.push((nested, format!("{context}[{idx}]"), depth + 1));
                }
            }
            serde_json::Value::String(value) => {
                failures.extend(check_control_chars(value, &context));
                let lowered = value.to_ascii_lowercase();
                if ["password=", "token=", "api_key=", "apikey=", "secret="]
                    .iter()
                    .any(|marker| lowered.contains(marker))
                {
                    failures.push(format!(
                        "secret_like_value: {context} contains a secret-like assignment"
                    ));
                }
            }
            _ => {}
        }
    }
    failures
}

#[derive(Clone, Debug)]
struct ProposalMetadata {
    proposal_id: String,
    proposal_revision_id: String,
    contract_object_hash: String,
    content_snapshot_id: String,
}

impl ProposalMetadata {
    fn unknown() -> Self {
        Self {
            proposal_id: DEFAULT_PROPOSAL_ID.to_string(),
            proposal_revision_id: DEFAULT_PROPOSAL_REVISION_ID.to_string(),
            contract_object_hash: DEFAULT_CONTRACT_OBJECT_HASH.to_string(),
            content_snapshot_id: DEFAULT_CONTENT_SNAPSHOT_ID.to_string(),
        }
    }

    fn unknown_for_artifact(artifact: &Artifact) -> Self {
        Self {
            content_snapshot_id: artifact.id.to_string(),
            ..Self::unknown()
        }
    }
}

fn proposal_metadata_from_artifact(
    run: &Run,
    artifact: &Artifact,
) -> Result<Option<ProposalMetadata>> {
    let path = match safe_artifact_path(&artifact.file_path, &run.workspace_root) {
        Ok(path) => path,
        Err(_) => return Ok(Some(ProposalMetadata::unknown_for_artifact(artifact))),
    };
    if !path.exists() {
        return Ok(None);
    }
    let data = match read_bounded_rollout_contract_input(&path) {
        Ok(data) => data,
        Err(_) => return Ok(Some(ProposalMetadata::unknown_for_artifact(artifact))),
    };
    let value: serde_json::Value = serde_json::from_slice(&data).unwrap_or(serde_json::Value::Null);
    let proposal_revision_id = value
        .get("proposal_revision_id")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_PROPOSAL_REVISION_ID)
        .to_string();
    let proposal_id = value
        .get("source_proposal")
        .and_then(|v| v.as_str())
        .and_then(proposal_id_from_source_path)
        .unwrap_or_else(|| DEFAULT_PROPOSAL_ID.to_string());
    let contract_object_hash = value
        .get("p084_self_contract")
        .map(hash_json_value)
        .unwrap_or_else(|| DEFAULT_CONTRACT_OBJECT_HASH.to_string());

    Ok(Some(ProposalMetadata {
        proposal_id,
        proposal_revision_id,
        contract_object_hash,
        content_snapshot_id: artifact.id.to_string(),
    }))
}

fn current_contract_identity(
    run: &Run,
    artifact: Option<&Artifact>,
) -> Result<Option<CurrentContractIdentity>> {
    let Some(artifact) = artifact else {
        return Ok(None);
    };
    let path = match safe_artifact_path(&artifact.file_path, &run.workspace_root) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    if !path.exists() {
        return Ok(None);
    }
    let data = match read_bounded_rollout_contract_input(&path) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };
    let value: serde_json::Value = match serde_json::from_slice(&data) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let metadata = proposal_metadata_from_value(artifact, &value);
    let contract_object_hash =
        match extract_rollout_contract_source(&value, &path, &run.workspace_root)? {
            Some(source) => hash_json_value(&source.contract),
            None => DEFAULT_CONTRACT_OBJECT_HASH.to_string(),
        };
    Ok(Some(CurrentContractIdentity {
        proposal_id: metadata.proposal_id,
        proposal_revision_id: metadata.proposal_revision_id,
        proposal_content_hash: proposal_content_hash(artifact, &data),
        contract_object_hash,
        content_snapshot_id: artifact.id.to_string(),
    }))
}

fn proposal_metadata_from_value(
    artifact: &Artifact,
    value: &serde_json::Value,
) -> ProposalMetadata {
    let proposal_revision_id = value
        .get("proposal_revision_id")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_PROPOSAL_REVISION_ID)
        .to_string();
    let proposal_id = value
        .get("source_proposal")
        .and_then(|v| v.as_str())
        .and_then(proposal_id_from_source_path)
        .unwrap_or_else(|| DEFAULT_PROPOSAL_ID.to_string());
    let contract_object_hash = value
        .get("p084_self_contract")
        .map(hash_json_value)
        .unwrap_or_else(|| DEFAULT_CONTRACT_OBJECT_HASH.to_string());

    ProposalMetadata {
        proposal_id,
        proposal_revision_id,
        contract_object_hash,
        content_snapshot_id: artifact.id.to_string(),
    }
}

fn proposal_content_hash(_artifact: &Artifact, data: &[u8]) -> String {
    let digest = sha2::Sha256::digest(data);
    format!("sha256:{digest:x}")
}

fn safe_artifact_path(file_path: &str, workspace_root: &str) -> Result<std::path::PathBuf> {
    let relative = std::path::Path::new(file_path);
    if relative.is_absolute() {
        anyhow::bail!("artifact path must be relative to workspace root: {file_path}");
    }
    if relative
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        anyhow::bail!("artifact path must not contain parent traversal: {file_path}");
    }
    let workspace = std::path::Path::new(workspace_root);
    let candidate = workspace.join(relative);
    if !candidate.exists() {
        return Ok(candidate);
    }
    let workspace = workspace
        .canonicalize()
        .with_context(|| format!("canonicalize workspace root {workspace_root}"))?;
    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("canonicalize artifact path {}", candidate.display()))?;
    if !canonical.starts_with(&workspace) {
        anyhow::bail!(
            "artifact path {} resolves outside workspace root {}",
            canonical.display(),
            workspace.display()
        );
    }
    Ok(canonical)
}

fn read_bounded_rollout_contract_input(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?
    };
    #[cfg(not(unix))]
    let file = std::fs::OpenOptions::new().read(true).open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_ROLLOUT_CONTRACT_INPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "rollout contract input {} exceeds {} bytes",
                path.display(),
                MAX_ROLLOUT_CONTRACT_INPUT_BYTES
            ),
        ));
    }
    let mut bounded = file.take(MAX_ROLLOUT_CONTRACT_INPUT_BYTES + 1);
    let mut data = Vec::new();
    bounded.read_to_end(&mut data)?;
    if data.len() as u64 > MAX_ROLLOUT_CONTRACT_INPUT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "rollout contract input {} exceeds {} bytes",
                path.display(),
                MAX_ROLLOUT_CONTRACT_INPUT_BYTES
            ),
        ));
    }
    Ok(data)
}

fn proposal_id_from_source_path(path: &str) -> Option<String> {
    let file_name = std::path::Path::new(path).file_name()?.to_str()?;
    let number = file_name.split('-').next()?;
    if number.chars().all(|c| c.is_ascii_digit()) {
        Some(format!("proposal-{number}"))
    } else {
        None
    }
}

fn hash_json_value(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let digest = sha2::Sha256::digest(&bytes);
    format!("sha256:{digest:x}")
}

fn default_rollback_disposition() -> serde_json::Value {
    serde_json::json!({
        "mode": "not_applicable",
        "data_loss_risk": "none",
        "steps": []
    })
}

fn write_rollout_contract_check_projection(
    run: &Run,
    check: &StoredRolloutContractCheck,
) -> Result<()> {
    let projection_path = rollout_contract_check_projection_path(run);
    let projection = rollout_contract_check_projection_value(check)?;
    atomic_write_json(&projection_path, &projection).with_context(|| {
        format!(
            "write rollout contract check projection {}",
            projection_path.display()
        )
    })
}

fn classify_existing_projection(
    run: &Run,
    check: &StoredRolloutContractCheck,
) -> Result<Option<ProjectionDrift>> {
    let projection_path = rollout_contract_check_projection_path(run);
    if !projection_path.exists() {
        return Ok(None);
    }

    let data = std::fs::read(&projection_path).with_context(|| {
        format!(
            "read rollout contract projection {}",
            projection_path.display()
        )
    })?;
    let projection: serde_json::Value = match serde_json::from_slice(&data) {
        Ok(projection) => projection,
        Err(error) => {
            return Ok(Some(ProjectionDrift {
                status: RolloutContractStatus::TamperDetected,
                projection_integrity: ProjectionIntegrity::TamperDetected,
                failure_reason: "tamper_detected_projection_invalid_json".to_string(),
                diagnostics: vec![format!(
                    "rollout_contract_check projection invalid JSON at {}: {error}",
                    projection_path.display()
                )],
            }))
        }
    };
    let expected = rollout_contract_check_projection_value(check)?;
    let projected_record_id = projection
        .get("authoritative_record_id")
        .and_then(|value| value.as_str());
    let expected_record_id = expected
        .get("authoritative_record_id")
        .and_then(|value| value.as_str());

    let mut mismatches = projection_mismatches(&projection, &expected);
    if mismatches.is_empty() {
        return Ok(None);
    }
    mismatches.truncate(8);

    if projected_record_id != expected_record_id {
        return Ok(Some(ProjectionDrift {
            status: RolloutContractStatus::Stale,
            projection_integrity: ProjectionIntegrity::Stale,
            failure_reason: "stale_rollout_contract_projection".to_string(),
            diagnostics: vec![format!(
                "rollout_contract_check projection stale at {} mismatches={}",
                projection_path.display(),
                mismatches.join(",")
            )],
        }));
    }

    Ok(Some(ProjectionDrift {
        status: RolloutContractStatus::TamperDetected,
        projection_integrity: ProjectionIntegrity::TamperDetected,
        failure_reason: "tamper_detected_rollout_contract_projection".to_string(),
        diagnostics: vec![format!(
            "rollout_contract_check projection tamper-suspect at {} mismatches={}",
            projection_path.display(),
            mismatches.join(",")
        )],
    }))
}

fn projection_mismatches(
    projection: &serde_json::Value,
    expected: &serde_json::Value,
) -> Vec<String> {
    [
        "schema_version",
        "authoritative_record_id",
        "run_id",
        "proposal_id",
        "proposal_revision_id",
        "proposal_content_hash",
        "contract_object_hash",
        "content_snapshot_id",
        "checker_version",
        "status",
        "decision",
        "lifecycle_state",
        "enforcement_mode",
        "rollback_disposition",
        "projection_integrity",
        "cutover_policy_revision",
        "redaction_state",
        "retry_count",
        "timeouts",
    ]
    .iter()
    .filter(|field| projection.get(**field) != expected.get(**field))
    .map(|field| (*field).to_string())
    .collect()
}

fn rollout_contract_check_projection_path(run: &Run) -> std::path::PathBuf {
    std::path::Path::new(&run.artifact_root)
        .join("readiness")
        .join("rollout-contract-check.json")
}

fn rollout_contract_check_projection_value(
    check: &StoredRolloutContractCheck,
) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "schema_version": "rollout_contract_check_v1",
        "authoritative_record_id": check.id.to_string(),
        "run_id": check.run_id.to_string(),
        "proposal_id": check.proposal_id,
        "proposal_revision_id": check.proposal_revision_id,
        "proposal_content_hash": check.proposal_content_hash,
        "contract_object_hash": check.contract_object_hash,
        "content_snapshot_id": check.content_snapshot_id,
        "checker_version": check.checker_version,
        "status": check.status,
        "decision": check.decision,
        "lifecycle_state": check.lifecycle_state,
        "enforcement_mode": check.enforcement_mode,
        "failure_reasons": check.failure_reasons,
        "diagnostics": check.diagnostics,
        "waiver": check.waiver,
        "rollback_disposition": check.rollback_disposition,
        "projection_integrity": check.projection_integrity,
        "cutover_policy_revision": check.cutover_policy_revision,
        "redaction_state": check.redaction_state,
        "retry_count": check.retry_count,
        "timeouts": {
            "preflight_timeout_seconds": check.preflight_timeout_seconds,
            "infrastructure_retry_max": PREFLIGHT_INFRA_RETRY_BACKOFF_MS.len(),
        },
        "created_at": check.created_at.to_rfc3339(),
        "updated_at": check.updated_at.to_rfc3339(),
    }))
}

fn atomic_write_json(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create projection directory {}", parent.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                .with_context(|| format!("chmod projection directory {}", parent.display()))?;
        }
    }
    let parent = path
        .parent()
        .with_context(|| format!("projection path has no parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("projection path has invalid file name: {}", path.display()))?;
    let tmp_path = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let data = serde_json::to_vec_pretty(value).context("serialize rollout contract projection")?;

    {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp_path)
            .with_context(|| format!("create temp projection {}", tmp_path.display()))?;
        file.write_all(&data)
            .with_context(|| format!("write temp projection {}", tmp_path.display()))?;
        file.write_all(b"\n")
            .with_context(|| format!("finish temp projection {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("fsync temp projection {}", tmp_path.display()))?;
    }

    std::fs::rename(&tmp_path, path).with_context(|| {
        format!(
            "rename temp projection {} to {}",
            tmp_path.display(),
            path.display()
        )
    })?;
    let parent_dir = std::fs::File::open(parent)
        .with_context(|| format!("open projection directory {}", parent.display()))?;
    parent_dir
        .sync_all()
        .with_context(|| format!("fsync projection directory {}", parent.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::repos::rollout_contract_checks::{
        RolloutContractEnforcementMode, RolloutContractLifecycleState,
    };
    use domain::{
        artifact::ArtifactFormat,
        ids::{ArtifactId, IdeaId, RunId},
        run::RunStatus,
    };
    use tempfile::TempDir;

    fn stored_check(
        status: RolloutContractStatus,
        decision: RolloutContractDecision,
        enforcement_mode: RolloutContractEnforcementMode,
    ) -> StoredRolloutContractCheck {
        let now = Utc::now();
        StoredRolloutContractCheck {
            id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            proposal_id: "proposal-084".to_string(),
            proposal_revision_id: "p084-r5".to_string(),
            proposal_content_hash: "sha256:abc".to_string(),
            contract_object_hash: "sha256:def".to_string(),
            content_snapshot_id: "snapshot".to_string(),
            checker_version: CHECKER_VERSION.to_string(),
            status,
            decision,
            lifecycle_state: RolloutContractLifecycleState::Terminal,
            enforcement_mode,
            failure_reasons: vec![],
            diagnostics: vec![],
            waiver: None,
            rollback_disposition: default_rollback_disposition(),
            projection_integrity: ProjectionIntegrity::Valid,
            cutover_policy_revision: None,
            redaction_state: "none".to_string(),
            retry_count: 0,
            preflight_timeout_seconds: 45,
            created_at: now,
            updated_at: now,
        }
    }

    fn test_run() -> Run {
        Run {
            id: RunId::new(),
            idea_id: IdeaId::new(),
            status: RunStatus::Running,
            workflow_id: "proposal-implementation".to_string(),
            workflow_title: "Proposal implementation".to_string(),
            workspace_root: "/tmp/workspace".to_string(),
            artifact_root: "/tmp/artifacts".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_7_implementation_started".to_string()),
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
        }
    }

    fn test_artifact(run: &Run, file_path: String) -> Artifact {
        write_valid_rollout_contract_fixtures(std::path::Path::new(&run.workspace_root));
        let file_path = std::path::Path::new(&file_path)
            .strip_prefix(&run.workspace_root)
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or(file_path);
        Artifact {
            id: ArtifactId::new(),
            run_id: run.id,
            stage_id: "approved".to_string(),
            agent_id: "proposal".to_string(),
            name: "approved_proposal".to_string(),
            contract_id: "approved_proposal".to_string(),
            format: ArtifactFormat::Json,
            file_path,
            checksum_sha256: None,
            size_bytes: None,
            provider: "test".to_string(),
            model: None,
            created_at: Utc::now(),
            is_pinned: true,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        }
    }

    fn write_valid_rollout_contract_fixtures(workspace_root: &std::path::Path) {
        let readback = workspace_root.join(
            "docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json",
        );
        let negative = workspace_root
            .join("docs/evidence/rollout-contract/negative/unsafe-path-and-command.json");
        let self_contract_negative = workspace_root.join(
            "docs/evidence/rollout-contract/negative/p084-self-contract-missing-readback-field.json",
        );
        if let Some(parent) = readback.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        if let Some(parent) = negative.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        if let Some(parent) = self_contract_negative.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        if !readback.exists() {
            std::fs::write(&readback, br#"{"schema_version":"operator_readback_v1"}"#).unwrap();
        }
        if !negative.exists() {
            std::fs::write(&negative, br#"{"schema_version":"rollout_contract_v1"}"#).unwrap();
        }
        if !self_contract_negative.exists() {
            std::fs::write(
                &self_contract_negative,
                br#"{"schema_version":"rollout_contract_v1"}"#,
            )
            .unwrap();
        }
    }

    fn valid_rollout_contract() -> serde_json::Value {
        let required_readback_fields = REQUIRED_OPERATOR_READBACK_FIELDS.to_vec();
        serde_json::json!({
            "schema_version": ROLLOUT_CONTRACT_SCHEMA_VERSION,
            "applicability": "required",
            "gate_aliases": ["proposal-084", "p084"],
            "commands": {
                "allowlist": ["./scripts/test-gate.sh proposal-084"]
            },
            "migrations": {
                "not_applicable": true,
                "justification": "No migration required for this test contract."
            },
            "metrics": {
                "adoption_metric": "new_applicable_proposals_with_passing_rollout_contract_percent",
                "operational_metrics": [
                    "rollout_contract_lint_total{proposal_id,status,failure_reason}"
                ]
            },
            "readback_lanes": ["run_report", "mcp", "release_receipt", "graphql"],
            "readback_fields": required_readback_fields,
            "readback_fixture": "docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json",
            "operator_report_fields": REQUIRED_OPERATOR_READBACK_FIELDS.to_vec(),
            "hold_conditions": [],
            "rollback_disposition": {
                "mode": "feature_flag_disable_or_enforcement_mode_permissive",
                "data_loss_risk": "none"
            },
            "decision_vocabulary": ["pass", "fail", "waived", "not_applicable", "timeout"],
            "negative_fixtures": {
                "unsafe_path_and_command": "docs/evidence/rollout-contract/negative/unsafe-path-and-command.json"
            }
        })
    }

    fn audited_policy_record(mode: Option<&str>, expires_at: DateTime<Utc>) -> serde_json::Value {
        let mut record = serde_json::json!({
            "authorized": true,
            "principal_id": "operator:test",
            "audit_event_id": Uuid::new_v4().to_string(),
            "reason_code": "p084_test_policy",
            "expires_at": expires_at.to_rfc3339()
        });
        if let Some(mode) = mode {
            record["mode"] = serde_json::json!(mode);
        }
        record
    }

    fn test_effective_policy(
        enforcement_mode: RolloutContractEnforcementMode,
    ) -> EffectivePreflightPolicy {
        EffectivePreflightPolicy {
            enforcement_mode,
            enforcement_mode_reason_code: None,
            enforcement_mode_supplied: true,
            active_waiver: None,
            failures: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn apply_enforce_policy(run: &mut Run) {
        run.delivery_preflight_json = Some(
            serde_json::json!({
                ROLLOUT_PREFLIGHT_POLICY_KEY: {
                    "enforcement_mode": audited_policy_record(
                        Some("enforce"),
                        Utc::now() + chrono::Duration::hours(1)
                    )
                }
            })
            .to_string(),
        );
    }

    async fn insert_test_command_journal(pool: &SqlitePool, id: &str, principal_id: &str) {
        sqlx::query(
            r#"INSERT INTO command_journal
               (id, command_type, payload_json, result_status, run_id, created_at,
                caller_surface, caller_principal_id, caller_principal_class, caller_tool, request_id)
               VALUES (?1, 'StartRun', '{}', 'completed', NULL, ?2,
                       'test', ?3, 'operator', 'test', NULL)"#,
        )
        .bind(id)
        .bind(Utc::now().to_rfc3339())
        .bind(principal_id)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn terminal_failure_mode_uses_run_stamped_enforce_policy() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        apply_enforce_policy(&mut run);

        assert_eq!(
            terminal_failure_enforcement_mode(&pool, &run).await,
            RolloutContractEnforcementMode::Enforce
        );
    }

    #[tokio::test]
    async fn terminal_failure_mode_fails_closed_when_policy_parse_fails() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.delivery_preflight_json = Some("not json".to_string());

        assert_eq!(
            terminal_failure_enforcement_mode(&pool, &run).await,
            RolloutContractEnforcementMode::Enforce
        );
    }

    #[test]
    fn pass_release_allows_under_enforce() {
        let evaluation = evaluate_terminal_check(stored_check(
            RolloutContractStatus::Pass,
            RolloutContractDecision::Release,
            RolloutContractEnforcementMode::Enforce,
        ));
        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert!(!evaluation.would_block);
    }

    #[test]
    fn failing_check_holds_under_enforce() {
        let evaluation = evaluate_terminal_check(stored_check(
            RolloutContractStatus::Fail,
            RolloutContractDecision::Hold,
            RolloutContractEnforcementMode::Enforce,
        ));
        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
    }

    #[test]
    fn failing_check_allows_but_marks_would_block_under_permissive() {
        let evaluation = evaluate_terminal_check(stored_check(
            RolloutContractStatus::Fail,
            RolloutContractDecision::Hold,
            RolloutContractEnforcementMode::Permissive,
        ));
        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert!(evaluation.would_block);
    }

    #[test]
    fn cancelled_check_holds_even_under_permissive() {
        let evaluation = evaluate_terminal_check(stored_check(
            RolloutContractStatus::Cancelled,
            RolloutContractDecision::Hold,
            RolloutContractEnforcementMode::Permissive,
        ));
        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
    }

    #[tokio::test]
    async fn missing_contract_record_allows_in_permissive_with_would_block() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let run = test_run();

        let check = upsert_missing_contract_check(
            &pool,
            &run,
            None,
            RolloutContractEnforcementMode::Permissive,
            0,
        )
        .await
        .unwrap();
        let evaluation = evaluate_terminal_check(check);

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert!(evaluation.would_block);
        assert_eq!(
            evaluation.check.status,
            RolloutContractStatus::MissingContract
        );
        assert_eq!(evaluation.check.decision, RolloutContractDecision::Hold);
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["missing_rollout_contract_check"]
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| diagnostic
            == "metric:rollout_contract_permissive_dogfood_total{proposal_id=\"unknown\",status=\"missing_contract\",would_block=\"true\"}=1"));
    }

    #[tokio::test]
    async fn missing_contract_holds_under_stamped_enforce_policy() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        apply_enforce_policy(&mut run);

        let evaluation = implementation_run_start_rollout_contract_preflight(&pool, &run, None)
            .await
            .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
        assert_eq!(
            evaluation.check.status,
            RolloutContractStatus::MissingContract
        );
        assert_eq!(
            evaluation.check.enforcement_mode,
            RolloutContractEnforcementMode::Enforce
        );
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["missing_rollout_contract_check"]
        );
    }

    #[tokio::test]
    async fn timeout_record_holds_under_enforce() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let run = test_run();

        let check = upsert_timeout_contract_check(
            &pool,
            &run,
            None,
            RolloutContractEnforcementMode::Enforce,
        )
        .await
        .unwrap();
        let evaluation = evaluate_terminal_check(check);

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Timeout);
        assert_eq!(evaluation.check.decision, RolloutContractDecision::Hold);
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["rollout_contract_preflight_timeout"]
        );
        assert_eq!(
            evaluation.check.preflight_timeout_seconds,
            PREFLIGHT_TIMEOUT_SECONDS
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| diagnostic
            == "metric:rollout_contract_run_start_block_total{proposal_id=\"unknown\",reason=\"rollout_contract_preflight_timeout\",enforcement_mode=\"enforce\"}=1"));
    }

    #[test]
    fn preflight_retries_only_infrastructure_errors() {
        assert!(should_retry_preflight_error(&anyhow::anyhow!(
            "upsert rollout_contract_checks: database is locked"
        )));
        assert!(should_retry_preflight_error(&anyhow::anyhow!(
            "write rollout contract check projection /tmp/run/readiness/rollout-contract-check.json"
        )));
        assert!(!should_retry_preflight_error(&anyhow::anyhow!(
            "parse delivery_preflight_json: expected value at line 1 column 1"
        )));
        assert!(!should_retry_preflight_error(&anyhow::anyhow!(
            "invalid_rollout_contract_sidecar: rollout_contract_sidecar must be a string"
        )));
    }

    #[tokio::test]
    async fn retry_exhausted_record_holds_under_enforce_and_counts_attempts() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let run = test_run();
        let error = anyhow::anyhow!("upsert rollout_contract_checks: database is locked");

        let check = upsert_retry_exhausted_contract_check(
            &pool,
            &run,
            None,
            RolloutContractEnforcementMode::Enforce,
            PREFLIGHT_INFRA_RETRY_BACKOFF_MS.len() as i64,
            &error,
        )
        .await
        .unwrap();
        let evaluation = evaluate_terminal_check(check);

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Fail);
        assert_eq!(evaluation.check.decision, RolloutContractDecision::Hold);
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["rollout_contract_preflight_retry_exhausted"]
        );
        assert_eq!(
            evaluation.check.retry_count,
            PREFLIGHT_INFRA_RETRY_BACKOFF_MS.len() as i64
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| diagnostic
            == "metric:rollout_contract_retry_exhausted_total{proposal_id=\"unknown\",failure_class=\"infrastructure\"}=1"));
    }

    #[tokio::test]
    async fn scheduling_time_waiver_allows_missing_contract_under_enforce() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        apply_enforce_policy(&mut run);
        let mut waiver = audited_policy_record(None, Utc::now() + chrono::Duration::hours(1));
        waiver["state"] = serde_json::json!("active");
        waiver["decision"] = serde_json::json!("waive");
        let waiver_audit_event_id = waiver["audit_event_id"].as_str().unwrap().to_string();
        let waiver_principal_id = waiver["principal_id"].as_str().unwrap().to_string();
        insert_test_command_journal(&pool, &waiver_audit_event_id, &waiver_principal_id).await;
        run.delivery_preflight_json = Some(
            serde_json::json!({
                ROLLOUT_PREFLIGHT_POLICY_KEY: {
                    "enforcement_mode": audited_policy_record(Some("enforce"), Utc::now() + chrono::Duration::hours(1)),
                    "waiver": waiver
                }
            })
            .to_string(),
        );

        let evaluation = implementation_run_start_rollout_contract_preflight(&pool, &run, None)
            .await
            .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert!(!evaluation.would_block);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Waived);
        assert_eq!(evaluation.check.decision, RolloutContractDecision::Waive);
        assert_eq!(
            evaluation.check.enforcement_mode,
            RolloutContractEnforcementMode::Enforce
        );
        assert!(evaluation.check.waiver.is_some());
        let projection: serde_json::Value = serde_json::from_slice(
            &std::fs::read(rollout_contract_check_projection_path(&run)).unwrap(),
        )
        .unwrap();
        assert_eq!(projection["status"], serde_json::json!("waived"));
        assert_eq!(projection["decision"], serde_json::json!("waive"));
        assert!(projection["waiver"].is_object());
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| diagnostic
            == "metric:rollout_contract_waiver_total{proposal_id=\"unknown\",reason=\"scheduling_time_waiver\",waiver_state=\"active\"}=1"));
    }

    #[tokio::test]
    async fn expired_scheduling_time_waiver_holds_under_enforce() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        apply_enforce_policy(&mut run);
        let mut waiver = audited_policy_record(None, Utc::now() - chrono::Duration::hours(1));
        waiver["state"] = serde_json::json!("active");
        waiver["decision"] = serde_json::json!("waive");
        run.delivery_preflight_json = Some(
            serde_json::json!({
                ROLLOUT_PREFLIGHT_POLICY_KEY: {
                    "enforcement_mode": audited_policy_record(Some("enforce"), Utc::now() + chrono::Duration::hours(1)),
                    "waiver": waiver
                }
            })
            .to_string(),
        );

        let evaluation = implementation_run_start_rollout_contract_preflight(&pool, &run, None)
            .await
            .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Fail);
        assert_eq!(evaluation.check.decision, RolloutContractDecision::Hold);
        assert!(evaluation
            .check
            .failure_reasons
            .iter()
            .any(|reason| reason.starts_with("expired_waiver:")));
    }

    #[tokio::test]
    async fn cancellation_requested_holds_before_contract_linting() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        run.cancellation_requested_at = Some(Utc::now());

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Cancelled);
        assert_eq!(evaluation.check.decision, RolloutContractDecision::Hold);
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["rollout_contract_preflight_cancelled"]
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| diagnostic
            == "metric:rollout_contract_preflight_cancelled_total{proposal_id=\"proposal-084\",reason=\"rollout_contract_preflight_cancelled\"}=1"));
        let projection: serde_json::Value = serde_json::from_slice(
            &std::fs::read(rollout_contract_check_projection_path(&run)).unwrap(),
        )
        .unwrap();
        assert_eq!(projection["status"], serde_json::json!("cancelled"));
        assert_eq!(projection["decision"], serde_json::json!("hold"));
    }

    #[tokio::test]
    async fn p084_self_contract_normalizes_to_run_start_rollout_contract() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "metrics": {
                "adoption_metric": {
                    "name": "new_applicable_proposals_with_passing_rollout_contract_percent",
                    "source": "rollout_contract_check_v1 authoritative records summarized in run reports and release evidence",
                    "target": "100% after enforce cutover"
                },
                "operational_metrics": [
                    "rollout_contract_lint_total{proposal_id,status,failure_reason}",
                    "rollout_contract_permissive_dogfood_total{proposal_id,status,would_block}"
                ]
            },
            "acceptance_criteria": [{
                "id": "ac-006",
                "negative_fixture": "docs/evidence/rollout-contract/negative/p084-self-contract-missing-readback-field.json"
            }],
            "p084_self_contract": {
                "applicability": "required",
                "gate_aliases": ["proposal-084", "p084"],
                "readback_fixture": "docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json",
                "readback_lanes": ["run_report", "release_receipt", "mcp", "graphql"],
                "required_report_fields": REQUIRED_OPERATOR_READBACK_FIELDS,
                "rollback_disposition": {
                    "mode": "feature_flag_disable_or_enforcement_mode_permissive",
                    "data_loss_risk": "none",
                    "steps": ["Move enforcement mode through an audited mutation."]
                }
            }
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let check = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Enforce),
            0,
        )
        .await
        .unwrap()
        .expect("p084_self_contract should normalize to a terminal rollout_contract_v1 check");
        let evaluation = evaluate_terminal_check(check.clone());

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert_eq!(check.status, RolloutContractStatus::Pass);
        assert_eq!(check.decision, RolloutContractDecision::Release);
        assert_eq!(
            check.rollback_disposition["mode"],
            serde_json::json!("feature_flag_disable_or_enforcement_mode_permissive")
        );
        assert_eq!(
            check.rollback_disposition["steps"],
            serde_json::json!(["Move enforcement mode through an audited mutation."])
        );
        assert!(check.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("rollout_contract_v1 passed")
                && diagnostic.contains("#p084_self_contract")
        }));
        assert!(check.diagnostics.iter().any(|diagnostic| {
            diagnostic
                == "declared_operational_metric:rollout_contract_permissive_dogfood_total{proposal_id,status,would_block}"
        }));
    }

    #[tokio::test]
    async fn inline_contract_creates_pass_record_before_enqueue() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        let mut contract = valid_rollout_contract();
        contract["cutover_policy"] = serde_json::json!({
            "revision": "p084-cutover-v1",
            "enforcement_mode_at_cutover": "enforce",
            "applicable_to": "post_cutover_implementation_starts",
            "grandfathered_rendering": "not_applicable",
            "effective_timestamp_iso8601": "2026-05-02T00:00:00Z"
        });
        contract["metrics"]["operational_metrics"] = serde_json::json!([
            "rollout_contract_lint_total{proposal_id,status,failure_reason}",
            "rollout_contract_enforcement_mode_total{proposal_id,mode}",
            "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}",
            "rollout_contract_permissive_dogfood_total{proposal_id,status,would_block}"
        ]);
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": contract
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let check = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Enforce),
            0,
        )
        .await
        .unwrap()
        .expect("inline rollout_contract_v1 should produce a terminal check");
        let evaluation = evaluate_terminal_check(check.clone());

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert_eq!(check.status, RolloutContractStatus::Pass);
        assert_eq!(check.decision, RolloutContractDecision::Release);
        assert_eq!(check.proposal_id, "proposal-084");
        assert_eq!(check.proposal_revision_id, "p084-r5");
        assert!(check.failure_reasons.is_empty());
        assert_eq!(check.projection_integrity, ProjectionIntegrity::Valid);
        assert_eq!(
            check.cutover_policy_revision.as_deref(),
            Some("p084-cutover-v1")
        );
        assert!(check.diagnostics.iter().any(|diagnostic| diagnostic.contains(
            "metric:rollout_contract_lint_total{proposal_id=\"proposal-084\",status=\"pass\",failure_reason=\"none\"}=1"
        )));
        assert!(check.diagnostics.iter().any(|diagnostic| diagnostic.contains(
            "metric:rollout_contract_enforcement_mode_total{proposal_id=\"proposal-084\",mode=\"enforce\"}=1"
        )));
        assert!(check
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic == "declared_operational_metric:rollout_contract_permissive_dogfood_total{proposal_id,status,would_block}"));
    }

    #[tokio::test]
    async fn cutover_policy_grandfathers_pre_cutover_runs_as_not_applicable() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        run.started_at = chrono::DateTime::parse_from_rfc3339("2026-05-01T23:59:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let proposal_path = dir.path().join("approved-proposal.json");
        let mut contract = valid_rollout_contract();
        contract["cutover_policy"] = serde_json::json!({
            "revision": "p084-cutover-v1",
            "enforcement_mode_at_cutover": "enforce",
            "applicable_to": "post_cutover_implementation_starts",
            "grandfathered_rendering": "not_applicable",
            "effective_timestamp_iso8601": "2026-05-02T00:00:00Z"
        });
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": contract
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert!(!evaluation.would_block);
        assert_eq!(
            evaluation.check.status,
            RolloutContractStatus::NotApplicable
        );
        assert_eq!(
            evaluation.check.decision,
            RolloutContractDecision::NotApplicable
        );
        assert_eq!(
            evaluation.check.enforcement_mode,
            RolloutContractEnforcementMode::Enforce
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| {
            diagnostic == "rollout_contract_cutover_grandfathered_until=2026-05-02T00:00:00+00:00"
        }));
    }

    #[tokio::test]
    async fn invalid_inline_contract_creates_hold_record_under_enforce() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();

        let mut contract = valid_rollout_contract();
        contract["commands"]["allowlist"] =
            serde_json::json!(["./scripts/test-gate.sh proposal-084 && rm"]);
        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": contract
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let check = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Enforce),
            0,
        )
        .await
        .unwrap()
        .expect("invalid inline rollout_contract_v1 should produce a terminal check");
        let evaluation = evaluate_terminal_check(check.clone());

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
        assert_eq!(check.status, RolloutContractStatus::Fail);
        assert_eq!(check.decision, RolloutContractDecision::Hold);
        assert!(check
            .failure_reasons
            .iter()
            .any(|reason| reason.starts_with("unsafe_command:")));
        assert!(check
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains(
                "metric:rollout_contract_run_start_block_total{proposal_id=\"proposal-084\""
            )));
    }

    #[test]
    fn lint_rejects_control_characters_in_any_contract_string() {
        let dir = TempDir::new().unwrap();
        write_valid_rollout_contract_fixtures(dir.path());
        let mut contract = valid_rollout_contract();
        contract["commands"]["allowlist"] =
            serde_json::json!(["./scripts/test-gate.sh proposal-084\u{0007}"]);
        contract["metrics"]["adoption_metric"] =
            serde_json::json!("new_applicable_proposals\u{0008}_percent");

        let workspace_root = dir.path().to_string_lossy();
        let lint = lint_rollout_contract(&contract, &workspace_root);

        assert!(lint.failures.iter().any(|failure| failure
            .contains("control_characters: control character U+0007 in commands.allowlist[0]")));
        assert!(lint.failures.iter().any(|failure| failure
            .contains("control_characters: control character U+0008 in rollout_contract_v1.metrics.adoption_metric")));
    }

    #[test]
    fn lint_rejects_public_extraction_failure_field() {
        let dir = TempDir::new().unwrap();
        write_valid_rollout_contract_fixtures(dir.path());
        let mut contract = valid_rollout_contract();
        contract["p084_extraction_failures"] = serde_json::json!(["operator supplied"]);

        let workspace_root = dir.path().to_string_lossy();
        let lint = lint_rollout_contract(&contract, &workspace_root);

        assert!(lint.failures.iter().any(|failure| failure
            == "unknown_field: rollout_contract_v1.p084_extraction_failures is not allowed"));
    }

    #[test]
    fn lint_rejects_missing_readback_fixture_path() {
        let dir = TempDir::new().unwrap();
        write_valid_rollout_contract_fixtures(dir.path());
        let mut contract = valid_rollout_contract();
        contract["readback_fixture"] =
            serde_json::json!("docs/evidence/rollout-contract/operator-readback/missing.json");

        let workspace_root = dir.path().to_string_lossy();
        let lint = lint_rollout_contract(&contract, &workspace_root);

        assert!(lint
            .failures
            .iter()
            .any(|failure| failure.starts_with("missing_fixture_path: readback_fixture")));
    }

    #[test]
    fn lint_rejects_missing_negative_fixture_path() {
        let dir = TempDir::new().unwrap();
        write_valid_rollout_contract_fixtures(dir.path());
        let mut contract = valid_rollout_contract();
        contract["negative_fixtures"]["missing"] =
            serde_json::json!("docs/evidence/rollout-contract/negative/missing.json");

        let workspace_root = dir.path().to_string_lossy();
        let lint = lint_rollout_contract(&contract, &workspace_root);

        assert!(lint
            .failures
            .iter()
            .any(|failure| failure.starts_with("missing_fixture_path: negative_fixtures.missing")));
    }

    #[tokio::test]
    async fn policy_record_requires_operator_start_run_journal_entry() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let principal_id = "operator:test";
        let audit_event_id = Uuid::new_v4().to_string();
        sqlx::query(
            r#"INSERT INTO command_journal
               (id, command_type, payload_json, result_status, run_id, created_at,
                caller_surface, caller_principal_id, caller_principal_class, caller_tool, request_id)
               VALUES (?1, 'RetryStage', '{}', 'completed', NULL, ?2,
                       'test', ?3, 'agent', 'test', NULL)"#,
        )
        .bind(&audit_event_id)
        .bind(Utc::now().to_rfc3339())
        .bind(principal_id)
        .execute(&pool)
        .await
        .unwrap();
        let mut record =
            audited_policy_record(Some("permissive"), Utc::now() + chrono::Duration::hours(1));
        record["principal_id"] = serde_json::json!(principal_id);
        record["audit_event_id"] = serde_json::json!(audit_event_id);

        let validation = validated_policy_record(&pool, &record, "enforcement_mode", Utc::now())
            .await
            .unwrap();

        match validation {
            PolicyRecordValidation::Invalid(failures) => {
                assert!(failures
                    .iter()
                    .any(|failure| { failure.contains("must be a StartRun command") }));
                assert!(failures
                    .iter()
                    .any(|failure| { failure.contains("principal class must be operator") }));
            }
            PolicyRecordValidation::Valid(_) => panic!("non-operator non-StartRun policy passed"),
        }
    }

    #[tokio::test]
    async fn oversized_approved_proposal_creates_bounded_hold_record_under_enforce() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        apply_enforce_policy(&mut run);

        let proposal_path = dir.path().join("approved-proposal.json");
        std::fs::write(
            &proposal_path,
            vec![b' '; (MAX_ROLLOUT_CONTRACT_INPUT_BYTES + 1) as usize],
        )
        .unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Fail);
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["invalid_approved_proposal_artifact"]
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("read approved proposal artifact")
                && diagnostic.contains("exceeds")
                && diagnostic.len() < 1024
        }));
    }

    #[tokio::test]
    async fn preflight_writes_rollout_contract_check_projection() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        apply_enforce_policy(&mut run);

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        let projection_path = rollout_contract_check_projection_path(&run);
        let projection: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&projection_path).unwrap()).unwrap();
        assert_eq!(
            projection["schema_version"],
            serde_json::json!("rollout_contract_check_v1")
        );
        assert_eq!(
            projection["authoritative_record_id"],
            serde_json::json!(evaluation.check.id.to_string())
        );
        assert_eq!(projection["status"], serde_json::json!("pass"));
        assert_eq!(projection["decision"], serde_json::json!("release"));
        assert_eq!(
            projection["projection_integrity"],
            serde_json::json!("valid")
        );
        assert_eq!(
            projection["timeouts"]["preflight_timeout_seconds"],
            serde_json::json!(PREFLIGHT_TIMEOUT_SECONDS)
        );
        assert!(projection.get("preflight_timeout_seconds").is_none());
    }

    #[tokio::test]
    async fn concurrent_preflight_reuses_same_terminal_record() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let (first, second) = tokio::join!(
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact)),
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact)),
        );
        let first = first.unwrap();
        let second = second.unwrap();

        assert_eq!(first.action, RolloutContractPreflightAction::Allow);
        assert_eq!(second.action, RolloutContractPreflightAction::Allow);
        assert_eq!(first.check.id, second.check.id);
        let terminal_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM rollout_contract_checks WHERE run_id = ?1 AND lifecycle_state = 'terminal'",
        )
        .bind(run.id.inner().to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(terminal_count, 1);
    }

    #[tokio::test]
    async fn missing_existing_projection_self_heals_from_authoritative_record() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());
        let stored = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Enforce),
            0,
        )
        .await
        .unwrap()
        .unwrap();

        let projection_path = rollout_contract_check_projection_path(&run);
        assert!(!projection_path.exists());

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert_eq!(evaluation.check.id, stored.id);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Pass);
        assert_eq!(
            evaluation.check.projection_integrity,
            ProjectionIntegrity::Valid
        );
        let projection: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&projection_path).unwrap()).unwrap();
        assert_eq!(
            projection["authoritative_record_id"],
            serde_json::json!(stored.id.to_string())
        );
        assert_eq!(projection["status"], serde_json::json!("pass"));
        assert_eq!(
            projection["projection_integrity"],
            serde_json::json!("valid")
        );
    }

    #[tokio::test]
    async fn terminal_check_is_not_reused_after_approved_proposal_hash_drift() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        apply_enforce_policy(&mut run);

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let first =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();
        assert_eq!(first.action, RolloutContractPreflightAction::Allow);

        let mut changed_contract = valid_rollout_contract();
        changed_contract["commands"]["allowlist"] =
            serde_json::json!(["./scripts/test-gate.sh proposal-084 && rm"]);
        let changed_proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r6",
            "rollout_contract_v1": changed_contract
        });
        std::fs::write(
            &proposal_path,
            serde_json::to_vec(&changed_proposal).unwrap(),
        )
        .unwrap();

        let second =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_ne!(first.check.id, second.check.id);
        assert_eq!(second.action, RolloutContractPreflightAction::Hold);
        assert!(second.would_block);
        assert_eq!(second.check.status, RolloutContractStatus::Stale);
        assert_eq!(
            second.check.failure_reasons,
            vec!["stale_rollout_contract_check_hash_drift"]
        );
        assert!(second
            .check
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("proposal_content_hash")));
    }

    #[tokio::test]
    async fn proposal_content_hash_is_derived_from_current_artifact_bytes() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        let data = serde_json::to_vec(&proposal).unwrap();
        std::fs::write(&proposal_path, &data).unwrap();
        let mut artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());
        artifact.checksum_sha256 = Some("deadbeef".to_string());

        let check = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Enforce),
            0,
        )
        .await
        .unwrap()
        .unwrap();
        let digest = sha2::Sha256::digest(&data);

        assert_eq!(check.proposal_content_hash, format!("sha256:{digest:x}"));
    }

    #[tokio::test]
    async fn absolute_approved_proposal_artifact_path_holds_and_scrubs_diagnostic() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        std::fs::write(&proposal_path, b"{}").unwrap();
        let mut artifact = test_artifact(&run, "approved-proposal.json".to_string());
        artifact.file_path = proposal_path.to_string_lossy().to_string();

        let check = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Enforce),
            0,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(check.status, RolloutContractStatus::Fail);
        assert_eq!(
            check.failure_reasons,
            vec!["unsafe_approved_proposal_artifact_path"]
        );
        assert_eq!(check.redaction_state, "partial");
        assert!(check
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("<workspace>")));
    }

    #[test]
    fn default_enforcement_mode_fails_closed_without_operator_policy() {
        assert_eq!(
            default_rollout_contract_enforcement_mode(),
            RolloutContractEnforcementMode::Enforce
        );
    }

    #[test]
    fn bounded_rollout_contract_input_rejects_oversized_files() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("oversized.json");
        std::fs::write(
            &path,
            vec![b' '; (MAX_ROLLOUT_CONTRACT_INPUT_BYTES + 1) as usize],
        )
        .unwrap();

        let error = read_bounded_rollout_contract_input(&path).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sidecar_symlink_escape_creates_hold_record() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();
        apply_enforce_policy(&mut run);

        let fixture_dir = dir.path().join("docs/evidence/rollout-contract");
        std::fs::create_dir_all(&fixture_dir).unwrap();
        let outside = dir.path().join("outside-sidecar.json");
        std::fs::write(
            &outside,
            serde_json::to_vec(&valid_rollout_contract()).unwrap(),
        )
        .unwrap();
        let sidecar = fixture_dir.join("sidecar.json");
        std::os::unix::fs::symlink(&outside, &sidecar).unwrap();

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_sidecar": "docs/evidence/rollout-contract/sidecar.json"
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Fail);
        assert!(evaluation
            .check
            .failure_reasons
            .iter()
            .any(|reason| reason.contains("unsafe_path")));
    }

    #[tokio::test]
    async fn tampered_existing_projection_holds_under_enforce() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());
        let check = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Enforce),
            0,
        )
        .await
        .unwrap()
        .unwrap();
        write_rollout_contract_check_projection(&run, &check).unwrap();

        let projection_path = rollout_contract_check_projection_path(&run);
        let mut projection: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&projection_path).unwrap()).unwrap();
        projection["status"] = serde_json::json!("pass_but_modified");
        std::fs::write(&projection_path, serde_json::to_vec(&projection).unwrap()).unwrap();

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Hold);
        assert!(evaluation.would_block);
        assert_eq!(
            evaluation.check.status,
            RolloutContractStatus::TamperDetected
        );
        assert_eq!(
            evaluation.check.projection_integrity,
            ProjectionIntegrity::TamperDetected
        );
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["tamper_detected_rollout_contract_projection"]
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| diagnostic
            == "metric:rollout_contract_tamper_or_stale_projection_total{proposal_id=\"proposal-084\",projection_integrity=\"tamper_suspect\"}=1"));
        let rewritten: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&projection_path).unwrap()).unwrap();
        assert_eq!(rewritten["status"], serde_json::json!("tamper_detected"));
        assert_eq!(
            rewritten["projection_integrity"],
            serde_json::json!("tamper_detected")
        );
    }

    #[tokio::test]
    async fn stale_existing_projection_allows_but_marks_would_block_under_permissive() {
        let dir = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", dir.path().join("test.db").display());
        let pool = db::pool::create_pool(&url).await.unwrap();
        let mut run = test_run();
        run.workspace_root = dir.path().to_string_lossy().to_string();
        run.artifact_root = dir
            .path()
            .join("run-artifacts")
            .to_string_lossy()
            .to_string();

        let proposal_path = dir.path().join("approved-proposal.json");
        let proposal = serde_json::json!({
            "source_proposal": "docs/proposals/084-executable-rollout-gates-and-observability-contract.md",
            "proposal_revision_id": "p084-r5",
            "rollout_contract_v1": valid_rollout_contract()
        });
        std::fs::write(&proposal_path, serde_json::to_vec(&proposal).unwrap()).unwrap();
        let artifact = test_artifact(&run, proposal_path.to_string_lossy().to_string());
        let first_check = upsert_linted_contract_check(
            &pool,
            &run,
            &artifact,
            &test_effective_policy(RolloutContractEnforcementMode::Permissive),
            0,
        )
        .await
        .unwrap()
        .unwrap();
        write_rollout_contract_check_projection(&run, &first_check).unwrap();

        let replacement = UpsertRolloutContractCheck {
            id: Uuid::new_v4(),
            run_id: first_check.run_id,
            proposal_id: first_check.proposal_id.clone(),
            proposal_revision_id: first_check.proposal_revision_id.clone(),
            proposal_content_hash: first_check.proposal_content_hash.clone(),
            contract_object_hash: first_check.contract_object_hash.clone(),
            content_snapshot_id: first_check.content_snapshot_id.clone(),
            checker_version: first_check.checker_version.clone(),
            status: RolloutContractStatus::Pass,
            decision: RolloutContractDecision::Release,
            lifecycle_state: RolloutContractLifecycleState::Terminal,
            enforcement_mode: RolloutContractEnforcementMode::Permissive,
            failure_reasons: vec![],
            diagnostics: vec!["replacement authoritative record".to_string()],
            waiver: None,
            rollback_disposition: first_check.rollback_disposition.clone(),
            projection_integrity: ProjectionIntegrity::Valid,
            cutover_policy_revision: None,
            redaction_state: "none".to_string(),
            retry_count: 0,
            preflight_timeout_seconds: 45,
        };
        rollout_contract_checks::upsert_rollout_contract_check(
            &pool,
            &replacement,
            first_check.updated_at + chrono::Duration::seconds(1),
        )
        .await
        .unwrap();

        let evaluation =
            implementation_run_start_rollout_contract_preflight(&pool, &run, Some(&artifact))
                .await
                .unwrap();

        assert_eq!(evaluation.action, RolloutContractPreflightAction::Allow);
        assert!(evaluation.would_block);
        assert_eq!(evaluation.check.status, RolloutContractStatus::Stale);
        assert_eq!(
            evaluation.check.projection_integrity,
            ProjectionIntegrity::Stale
        );
        assert_eq!(
            evaluation.check.failure_reasons,
            vec!["stale_rollout_contract_projection"]
        );
        assert!(evaluation.check.diagnostics.iter().any(|diagnostic| diagnostic
            == "metric:rollout_contract_tamper_or_stale_projection_total{proposal_id=\"proposal-084\",projection_integrity=\"stale\"}=1"));
    }
}
