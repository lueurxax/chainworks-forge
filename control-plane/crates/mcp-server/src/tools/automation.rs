use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::protocol::McpTool;

const READBACK_VERSION: &str = "auto_retry_readback.v1";

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "automation.auto_retry.latest".to_string(),
        description: "Read the latest P076 observe-only auto-retry ledger and policy readback"
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "run_id": {
                    "type": "string",
                    "description": "Optional run id to include a null-history row when the ledger has no record for the run"
                },
                "blocker_signature_id": {
                    "type": "string",
                    "description": "Optional blocker signature filter"
                },
                "client_supported_versions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Supported readback versions; currently auto_retry_readback.v1"
                }
            }
        }),
        output_schema: None,
    }]
}

/// SEC-P083-MED-003: Principal is required so that Observer principals receive
/// redacted output: absolute filesystem paths and free-form ledger text
/// (failure_summary, next_systemic_action) are omitted for non-Operator callers.
pub async fn execute(tool_name: &str, params: Value, principal: &auth::Principal) -> Result<Value> {
    match tool_name {
        "automation.auto_retry.latest" => {
            let is_operator = matches!(principal.class, auth::PrincipalClass::Operator);
            Ok(auto_retry_latest(params, is_operator))
        }
        _ => Err(anyhow::anyhow!("Unknown automation tool: {tool_name}")),
    }
}

fn auto_retry_latest(params: Value, is_operator: bool) -> Value {
    let paths = P076Paths::resolve();
    auto_retry_latest_with_paths(params, paths, is_operator)
}

fn auto_retry_latest_with_paths(params: Value, paths: P076Paths, is_operator: bool) -> Value {
    let run_filter = params.get("run_id").and_then(Value::as_str);
    let signature_filter = params.get("blocker_signature_id").and_then(Value::as_str);

    let mut diagnostics = Vec::new();
    let mut observations = Vec::new();

    // SEC-P083-MED-003: Only Operator principals see absolute filesystem paths in diagnostics.
    let path_for_diagnostic = if is_operator {
        Some(paths.ledger_path.to_string_lossy().into_owned())
    } else {
        None
    };

    match std::fs::read_to_string(&paths.ledger_path) {
        Ok(text) => {
            let mut lines: Vec<&str> = text.lines().collect();
            if !text.is_empty() && !text.as_bytes().ends_with(b"\n") {
                diagnostics.push(diagnostic(
                    "partial_trailing_record",
                    "warning",
                    format!(
                        "Ignored partial trailing auto-retry observation at line {}: ledger does not end with newline",
                        lines.len()
                    ),
                    path_for_diagnostic.as_deref(),
                ));
                lines.pop();
            }
            for (idx, raw_line) in lines.iter().enumerate() {
                let line = raw_line.trim();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(line) {
                    Ok(record) => {
                        observations.extend(observation_summaries(
                            &record,
                            &paths.ledger_path,
                            run_filter,
                            signature_filter,
                            is_operator,
                        ));
                    }
                    Err(error) => {
                        return degraded_readback(
                            paths.clone(),
                            diagnostic(
                                "artifact_read_degraded",
                                "error",
                                format!("ledger parse failed at line {}: {error}", idx + 1),
                                path_for_diagnostic.as_deref(),
                            ),
                            is_operator,
                        );
                    }
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            diagnostics.push(diagnostic(
                "no_observation_history",
                "info",
                "No auto-retry observation ledger exists yet",
                path_for_diagnostic.as_deref(),
            ));
        }
        Err(error) => {
            return degraded_readback(
                paths.clone(),
                diagnostic(
                    "artifact_read_degraded",
                    "error",
                    format!("ledger read failed: {error}"),
                    path_for_diagnostic.as_deref(),
                ),
                is_operator,
            );
        }
    }

    let latest_by_run = latest_by_run(&observations, run_filter);
    readback_envelope(paths, diagnostics, observations, latest_by_run, is_operator)
}

fn observation_summaries(
    record: &Value,
    ledger_path: &PathBuf,
    run_filter: Option<&str>,
    signature_filter: Option<&str>,
    is_operator: bool,
) -> Vec<Value> {
    let observation_id = record
        .get("observation_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let observed_at = record
        .get("observed_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    record
        .get("blocked_runs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|blocked| {
            run_filter
                .map(|run_id| blocked.get("run_id").and_then(Value::as_str) == Some(run_id))
                .unwrap_or(true)
        })
        .filter(|blocked| {
            signature_filter
                .map(|signature| {
                    blocked
                        .get("blocker_signature_id")
                        .and_then(Value::as_str)
                        == Some(signature)
                })
                .unwrap_or(true)
        })
        .map(|blocked| {
            // SEC-P083-MED-003: absolute paths and free-form ledger text are Operator-only.
            let observation_path: Value = if is_operator {
                Value::String(ledger_path.to_string_lossy().into_owned())
            } else {
                Value::Null
            };
            let failure_summary: Value = if is_operator {
                blocked.get("failure_summary").cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            let next_systemic_action: Value = if is_operator {
                blocked.get("next_systemic_action").cloned().unwrap_or(Value::Null)
            } else {
                Value::Null
            };
            json!({
                "observation_id": observation_id,
                "observed_at": observed_at,
                "run_id": blocked.get("run_id").and_then(Value::as_str).unwrap_or(""),
                "stage_id": blocked.get("stage_id").and_then(Value::as_str).unwrap_or(""),
                "blocker_signature_id": blocked.get("blocker_signature_id").and_then(Value::as_str).unwrap_or(""),
                "blocker_class": blocked.get("blocker_class").and_then(Value::as_str).unwrap_or("unknown"),
                "policy_decision": blocked.get("policy_decision").and_then(Value::as_str).unwrap_or("observe_only"),
                "retry_action": blocked.get("retry_action").and_then(Value::as_str).unwrap_or("none"),
                "retry_result": blocked.get("retry_result").and_then(Value::as_str).unwrap_or("not_attempted"),
                "known_issue_status": "observed",
                "observation_path": observation_path,
                "stage_execution_id": blocked.get("stage_execution_id").cloned().unwrap_or(Value::Null),
                "failure_summary": failure_summary,
                "next_systemic_action": next_systemic_action,
                "evidence_report_id": blocked.get("evidence_report_id").cloned().unwrap_or(Value::Null)
            })
        })
        .collect()
}

fn latest_by_run(observations: &[Value], run_filter: Option<&str>) -> Vec<Value> {
    let mut latest = BTreeMap::<String, Value>::new();
    for obs in observations {
        let Some(run_id) = obs.get("run_id").and_then(Value::as_str) else {
            continue;
        };
        latest.insert(run_id.to_string(), run_summary_from_observation(obs));
    }
    if let Some(run_id) = run_filter {
        latest.entry(run_id.to_string()).or_insert_with(|| {
            json!({
                "run_id": run_id,
                "auto_retry_policy_status": "no_observation_history",
                "auto_retry_policy_decision": Value::Null,
                "auto_retry_observation_record_id": Value::Null,
                "auto_retry_observation_path": Value::Null,
                "auto_retry_blocker_signature_id": Value::Null,
                "auto_retry_blocker_class": Value::Null,
                "auto_retry_retry_budget_state": Value::Null,
                "auto_retry_last_retry_result": Value::Null,
                "auto_retry_known_issue_status": Value::Null,
                "auto_retry_next_systemic_action": Value::Null,
                "auto_retry_rollup_report_path": Value::Null,
                "auto_retry_human_gate_retry_attempt_total": 0,
                "auto_retry_budget_unavailable_reason": Value::Null,
                "auto_retry_backpressure_skip_count": 0,
                "auto_retry_readback_version": READBACK_VERSION,
                "oldest_planned_attempt_at": Value::Null,
                "planned_attempt_age_seconds": Value::Null,
                "unknown_attempt_count": Value::Null,
                "required_operator_settlement": Value::Null
            })
        });
    }
    latest.into_values().collect()
}

fn run_summary_from_observation(obs: &Value) -> Value {
    // SEC-P083-MED-003: observation_path and next_systemic_action are already redacted
    // in observation_summaries for non-Operator principals; carry through as-is here.
    json!({
        "run_id": obs.get("run_id").cloned().unwrap_or(Value::Null),
        "auto_retry_policy_status": "observed",
        "auto_retry_policy_decision": obs.get("policy_decision").cloned().unwrap_or(Value::Null),
        "auto_retry_observation_record_id": obs.get("observation_id").cloned().unwrap_or(Value::Null),
        "auto_retry_observation_path": obs.get("observation_path").cloned().unwrap_or(Value::Null),
        "auto_retry_blocker_signature_id": obs.get("blocker_signature_id").cloned().unwrap_or(Value::Null),
        "auto_retry_blocker_class": obs.get("blocker_class").cloned().unwrap_or(Value::Null),
        "auto_retry_retry_budget_state": "available",
        "auto_retry_last_retry_result": obs.get("retry_result").cloned().unwrap_or(Value::Null),
        "auto_retry_known_issue_status": obs.get("known_issue_status").cloned().unwrap_or(Value::Null),
        "auto_retry_next_systemic_action": obs.get("next_systemic_action").cloned().unwrap_or(Value::Null),
        "auto_retry_rollup_report_path": Value::Null,
        "auto_retry_human_gate_retry_attempt_total": 0,
        "auto_retry_budget_unavailable_reason": Value::Null,
        "auto_retry_backpressure_skip_count": 0,
        "auto_retry_readback_version": READBACK_VERSION,
        "oldest_planned_attempt_at": Value::Null,
        "planned_attempt_age_seconds": Value::Null,
        "unknown_attempt_count": Value::Null,
        "required_operator_settlement": Value::Null
    })
}

fn degraded_readback(paths: P076Paths, diagnostic: Value, is_operator: bool) -> Value {
    readback_envelope(paths, vec![diagnostic], Vec::new(), Vec::new(), is_operator)
}

fn readback_envelope(
    paths: P076Paths,
    diagnostics: Vec<Value>,
    observations: Vec<Value>,
    latest_by_run: Vec<Value>,
    is_operator: bool,
) -> Value {
    // SEC-P083-MED-003: absolute filesystem paths in top-level envelope are Operator-only.
    let (
        ledger_path,
        budget_state_path,
        known_issue_catalog_path,
        generated_markdown_catalog_path,
        lock_path,
        rollup_report_path,
    ) = if is_operator {
        (
            Value::String(paths.ledger_path.to_string_lossy().into_owned()),
            Value::String(paths.budget_state_path.to_string_lossy().into_owned()),
            Value::String(
                paths
                    .known_issue_catalog_path
                    .to_string_lossy()
                    .into_owned(),
            ),
            Value::String(
                paths
                    .generated_markdown_catalog_path
                    .to_string_lossy()
                    .into_owned(),
            ),
            Value::String(paths.lock_path.to_string_lossy().into_owned()),
            Value::String(paths.rollup_report_path.to_string_lossy().into_owned()),
        )
    } else {
        (
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
        )
    };
    json!({
        "schema_version": READBACK_VERSION,
        "generated_at": Utc::now().to_rfc3339(),
        "version_negotiation": {
            "selected_version": READBACK_VERSION,
            "supported_versions": [READBACK_VERSION],
            "unsupported_versions": []
        },
        "ledger_path": ledger_path,
        "budget_state_path": budget_state_path,
        "known_issue_catalog_path": known_issue_catalog_path,
        "generated_markdown_catalog_path": generated_markdown_catalog_path,
        "lock_path": lock_path,
        "rollup_report_path": rollup_report_path,
        "diagnostics": diagnostics,
        "observations": observations,
        "latest_by_run": latest_by_run
    })
}

fn diagnostic(code: &str, severity: &str, message: impl Into<String>, path: Option<&str>) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "message": message.into(),
        "path": path,
        "run_id": Value::Null,
        "blocker_signature_id": Value::Null,
        "observation_id": Value::Null
    })
}

#[derive(Clone)]
struct P076Paths {
    ledger_path: PathBuf,
    budget_state_path: PathBuf,
    known_issue_catalog_path: PathBuf,
    generated_markdown_catalog_path: PathBuf,
    lock_path: PathBuf,
    rollup_report_path: PathBuf,
}

impl P076Paths {
    fn resolve() -> Self {
        let meta_root = resolve_meta_root();
        let automation = meta_root.join("automation");
        Self {
            ledger_path: automation.join("auto-retry-observations.jsonl"),
            budget_state_path: automation.join("auto-retry-budget.json"),
            known_issue_catalog_path: automation.join("auto-retry-known-issues.json"),
            generated_markdown_catalog_path: automation.join("auto-retry-known-issues.md"),
            lock_path: automation.join("auto-retry.lock"),
            rollup_report_path: automation.join("auto-retry-rollup.json"),
        }
    }
}

fn resolve_meta_root() -> PathBuf {
    if let Ok(root) = std::env::var("CHAINWORKS_META_ROOT") {
        if !root.is_empty() {
            return PathBuf::from(root);
        }
    }
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        let path_str = db_url.strip_prefix("sqlite://").unwrap_or(&db_url);
        let path_str = path_str.split('?').next().unwrap_or(path_str);
        if !path_str.is_empty() && path_str != ":memory:" && !path_str.starts_with(':') {
            let db_path = PathBuf::from(path_str);
            if let Some(parent) = db_path.parent() {
                return parent.to_path_buf();
            }
        }
    }
    PathBuf::from(".chainworks")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p076_auto_retry_latest_reads_ledger_and_reports_latest_by_run() {
        let temp = tempfile::tempdir().unwrap();
        let meta = temp.path().join(".chainworks");
        let automation = meta.join("automation");
        std::fs::create_dir_all(&automation).unwrap();
        std::fs::write(
            automation.join("auto-retry-observations.jsonl"),
            concat!(
                r#"{"schema_version":"auto-retry-observation.v1","observation_id":"ar_obs_20260523T100000Z_a1b2c3d4e5f6","observed_at":"2026-05-23T10:00:00Z","blocked_runs":[{"run_id":"run-1","stage_id":"state_9","blocker_signature_id":"sig-1","blocker_class":"substantive_output_contract","policy_decision":"observe_only","retry_action":"none","retry_result":"not_attempted","stage_execution_id":null,"failure_summary":"missing output","next_systemic_action":"fix contract","evidence_report_id":null}]}"#,
                "\n"
            ),
        )
        .unwrap();

        let payload = auto_retry_latest_with_paths(
            json!({"run_id": "run-1"}),
            test_paths(automation.clone()),
            true,
        );

        assert_eq!(payload["schema_version"], READBACK_VERSION);
        assert_eq!(payload["observations"][0]["run_id"], "run-1");
        assert_eq!(
            payload["latest_by_run"][0]["auto_retry_policy_decision"],
            "observe_only"
        );
        assert_eq!(
            payload["ledger_path"].as_str().unwrap(),
            automation
                .join("auto-retry-observations.jsonl")
                .to_string_lossy()
                .as_ref()
        );
    }

    #[test]
    fn p076_auto_retry_latest_observer_paths_redacted() {
        let temp = tempfile::tempdir().unwrap();
        let meta = temp.path().join(".chainworks");
        let automation = meta.join("automation");
        std::fs::create_dir_all(&automation).unwrap();
        std::fs::write(
            automation.join("auto-retry-observations.jsonl"),
            concat!(
                r#"{"schema_version":"auto-retry-observation.v1","observation_id":"ar_obs_20260523T100000Z_a1b2c3d4e5f6","observed_at":"2026-05-23T10:00:00Z","blocked_runs":[{"run_id":"run-1","stage_id":"state_9","blocker_signature_id":"sig-1","blocker_class":"substantive_output_contract","policy_decision":"observe_only","retry_action":"none","retry_result":"not_attempted","stage_execution_id":null,"failure_summary":"secret failure","next_systemic_action":"secret action","evidence_report_id":null}]}"#,
                "\n"
            ),
        )
        .unwrap();

        let payload = auto_retry_latest_with_paths(
            json!({"run_id": "run-1"}),
            test_paths(automation.clone()),
            false, // Observer
        );

        // Absolute paths are null for Observer principals.
        assert_eq!(payload["ledger_path"], Value::Null);
        assert_eq!(payload["budget_state_path"], Value::Null);
        assert_eq!(payload["lock_path"], Value::Null);
        // Sensitive free-form text is null for Observer principals.
        assert_eq!(payload["observations"][0]["failure_summary"], Value::Null);
        assert_eq!(
            payload["observations"][0]["next_systemic_action"],
            Value::Null
        );
        assert_eq!(payload["observations"][0]["observation_path"], Value::Null);
        // Non-sensitive aggregate fields are still present.
        assert_eq!(payload["observations"][0]["run_id"], "run-1");
        assert_eq!(
            payload["observations"][0]["policy_decision"],
            "observe_only"
        );
    }

    #[test]
    fn p076_auto_retry_latest_ignores_only_unterminated_trailing_fragment() {
        let temp = tempfile::tempdir().unwrap();
        let meta = temp.path().join(".chainworks");
        let automation = meta.join("automation");
        std::fs::create_dir_all(&automation).unwrap();
        std::fs::write(
            automation.join("auto-retry-observations.jsonl"),
            concat!(
                r#"{"schema_version":"auto-retry-observation.v1","observation_id":"ar_obs_20260523T100000Z_a1b2c3d4e5f6","observed_at":"2026-05-23T10:00:00Z","blocked_runs":[{"run_id":"run-1","stage_id":"state_9","blocker_signature_id":"sig-1","blocker_class":"substantive_output_contract","policy_decision":"observe_only","retry_action":"none","retry_result":"not_attempted","stage_execution_id":null,"failure_summary":"missing output","next_systemic_action":"fix contract","evidence_report_id":null}]}"#,
                "\n",
                r#"{"schema_version":"#
            ),
        )
        .unwrap();

        let payload =
            auto_retry_latest_with_paths(json!({"run_id": "run-1"}), test_paths(automation), true);

        assert_eq!(payload["observations"][0]["run_id"], "run-1");
        assert_eq!(payload["diagnostics"][0]["code"], "partial_trailing_record");
        assert_eq!(payload["diagnostics"][0]["severity"], "warning");
    }

    #[test]
    fn p076_auto_retry_latest_degrades_on_malformed_complete_final_line() {
        let temp = tempfile::tempdir().unwrap();
        let meta = temp.path().join(".chainworks");
        let automation = meta.join("automation");
        std::fs::create_dir_all(&automation).unwrap();
        std::fs::write(
            automation.join("auto-retry-observations.jsonl"),
            concat!(
                r#"{"schema_version":"auto-retry-observation.v1","observation_id":"ar_obs_20260523T100000Z_a1b2c3d4e5f6","observed_at":"2026-05-23T10:00:00Z","blocked_runs":[{"run_id":"run-1","stage_id":"state_9","blocker_signature_id":"sig-1","blocker_class":"substantive_output_contract","policy_decision":"observe_only","retry_action":"none","retry_result":"not_attempted","stage_execution_id":null,"failure_summary":"missing output","next_systemic_action":"fix contract","evidence_report_id":null}]}"#,
                "\n",
                r#"{"schema_version":"#,
                "\n"
            ),
        )
        .unwrap();

        let payload =
            auto_retry_latest_with_paths(json!({"run_id": "run-1"}), test_paths(automation), true);

        assert_eq!(payload["diagnostics"][0]["code"], "artifact_read_degraded");
        assert_eq!(payload["diagnostics"][0]["severity"], "error");
        assert_eq!(payload["observations"].as_array().unwrap().len(), 0);
        assert_eq!(payload["latest_by_run"].as_array().unwrap().len(), 0);
    }

    fn test_paths(automation: PathBuf) -> P076Paths {
        P076Paths {
            ledger_path: automation.join("auto-retry-observations.jsonl"),
            budget_state_path: automation.join("auto-retry-budget.json"),
            known_issue_catalog_path: automation.join("auto-retry-known-issues.json"),
            generated_markdown_catalog_path: automation.join("auto-retry-known-issues.md"),
            lock_path: automation.join("auto-retry.lock"),
            rollup_report_path: automation.join("auto-retry-rollup.json"),
        }
    }
}
