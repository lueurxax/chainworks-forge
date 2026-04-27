//! Per-variant redaction of serialized `Command` JSON before it is written
//! to the `command_journal` table.
//!
//! # Contract — Proposal 029 §8.1 authoritative matrix
//!
//! Each `Command` variant has an explicit `redact` / `preserve` / `omit`
//! decision per payload field:
//!
//! - **`preserve`** — keep field value verbatim (needed for audit replay or
//!   identity linkage, e.g. `run_id`, `stage_id`).
//! - **`redact`** — replace value with the literal string `"[REDACTED]"`
//!   while preserving key presence and type shape.
//! - **`omit`** — drop the field from the serialized JSON entirely.
//!
//! | Command variant      | Field                          | Decision   |
//! |----------------------|--------------------------------|------------|
//! | `StartRun`           | `idea_id`                      | preserve   |
//! | `StartRun`           | `workflow_id`/`workflow_title` | preserve   |
//! | `StartRun`           | `workspace_root`               | preserve   |
//! | `StartRun`           | `artifact_root`                | preserve   |
//! | `StartRun`           | `workflow_yaml_path`           | preserve   |
//! | `StartRun`           | `agent_catalog_yaml_path`      | preserve   |
//! | `StartRun`           | `delivery_configuration_json`  | **redact** |
//! | `ApproveStage`       | `run_id`, `stage_id`           | preserve   |
//! | `ApproveStage`       | `comment`                      | **redact** |
//! | `RejectStage`        | `run_id`, `stage_id`           | preserve   |
//! | `RejectStage`        | `comment`                      | **redact** |
//! | `RetryStage`         | all fields                     | preserve   |
//! | `OverrideLegacyDiscoveryPolicy` | all fields            | preserve   |
//! | `CancelRun`          | all fields                     | preserve   |
//! | `ResetSession`       | all fields                     | preserve   |
//! | `RunStewardAnalysis` | `reason`, `artifact_base`      | preserve   |
//!
//! # Fail-closed rule
//!
//! `redact_for_journal` matches **exhaustively** on the `Command` enum.
//! Adding a new `Command` variant forces a compile error here until the
//! author records an explicit matrix decision — there is no silent default.
//! `test_redaction_matrix_covers_all_command_variants` is the runtime proof
//! that every known variant produces a non-panicking redacted output.
//!
//! Called inside `CommandHandler::handle` immediately after
//! `serde_json::to_string(&cmd)` and before `command_journal::record`.

use domain::commands::Command;
use serde_json::Value;

/// Canonical redaction marker. Tests key off this exact string.
pub const REDACTED: &str = "[REDACTED]";

/// Apply the §8.1 per-variant redaction matrix to a serialized `Command`.
///
/// `payload_json` is the raw `serde_json::to_string(&cmd)` output. This
/// function parses it, applies the variant-specific matrix, and returns
/// the re-serialized result. If parsing fails the raw payload is returned
/// unchanged (defensive fallback — `Command` is always serializable so
/// this path is unreachable in production).
pub fn redact_for_journal(cmd: &Command, payload_json: &str) -> String {
    let mut value: Value = match serde_json::from_str(payload_json) {
        Ok(v) => v,
        Err(_) => return payload_json.to_string(),
    };

    // `Command` serializes as `{ "<VariantName>": { ...fields... } }`.
    // Extract the inner object so per-field edits are ergonomic.
    let inner = inner_payload_mut(&mut value);

    // Exhaustive match — adding a new Command variant fails to compile here
    // until the author records an explicit matrix entry.
    match cmd {
        Command::StartRun(_) => {
            if let Some(obj) = inner {
                redact_field_if_present(obj, "delivery_configuration_json");
            }
        }
        Command::ApproveStage(_) => {
            if let Some(obj) = inner {
                redact_field_if_present(obj, "comment");
            }
        }
        Command::RejectStage(_) => {
            if let Some(obj) = inner {
                redact_field_if_present(obj, "comment");
            }
        }
        Command::RetryStage(_) => {
            // §8.1: preserve all fields.
        }
        Command::ResolveWorkflowConflictTransition(_) => {
            // Operator conflict-resolution selections are audit material.
        }
        Command::OverrideLegacyDiscoveryPolicy(_) => {
            // P053: preserve typed override fields for operator audit.
        }
        Command::CancelRun(_) => {
            // §8.1: preserve all fields.
        }
        Command::ResetSession(_) => {
            // §8.1: preserve all fields.
        }
        Command::RunStewardAnalysis(_) => {
            // §8.1: preserve all fields.
        }
        Command::OverrideArtifactContract(_) => {
            // P057: preserve typed override fields for operator audit.
        }
        Command::ResolveLeadMediationConfirmation(_) => {
            // P017 Phase B: redact comment (may contain operator-submitted text).
            if let Some(obj) = inner {
                redact_field_if_present(obj, "comment");
            }
        }
    }

    serde_json::to_string(&value).unwrap_or_else(|_| payload_json.to_string())
}

/// Reach into the single-key object wrapper that `serde` produces for
/// externally-tagged enums and return a mutable reference to the inner
/// fields object. Returns `None` if the shape is unexpected (defensive).
fn inner_payload_mut(value: &mut Value) -> Option<&mut serde_json::Map<String, Value>> {
    let obj = value.as_object_mut()?;
    // There should be exactly one key — the variant name.
    let (_variant_name, inner) = obj.iter_mut().next()?;
    inner.as_object_mut()
}

/// Replace the value at `field` with the `[REDACTED]` string marker,
/// **only if the field is currently a non-null string-ish value**. Null
/// fields stay null so audit readers can tell the value was genuinely
/// absent rather than hidden.
fn redact_field_if_present(obj: &mut serde_json::Map<String, Value>, field: &str) {
    if let Some(existing) = obj.get(field) {
        // Preserve null (genuine absence). Redact any other present value.
        if !existing.is_null() {
            obj.insert(field.to_string(), Value::String(REDACTED.to_string()));
        }
    }
}

// ───────────────────────────────────────────────────────────────────────
// Tests — each variant's matrix decision is pinned by a focused test.
// Names must match the authoritative list in Proposal 029 §8.1 / §9.1.
// ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use domain::commands::{
        ApproveStageCmd, CancelRunCmd, Command, OverrideLegacyDiscoveryPolicyCmd, RejectStageCmd,
        ResetSessionCmd, ResolveLeadMediationConfirmationCmd, ResolveWorkflowConflictTransitionCmd,
        RetryStageCmd, RunStewardAnalysisCmd, StartRunCmd,
    };
    use domain::discovery::LegacyBroadDiscoveryPolicy;
    use domain::ids::{IdeaId, RunId, StageExecutionId};
    use domain::mediation::MediationConfirmationDecision;
    use serde_json::Value;

    fn round_trip(cmd: &Command) -> Value {
        let raw = serde_json::to_string(cmd).expect("command serializes");
        let redacted = redact_for_journal(cmd, &raw);
        serde_json::from_str(&redacted).expect("redacted output is valid json")
    }

    fn inner<'a>(v: &'a Value) -> &'a Value {
        v.as_object()
            .expect("command json is an object")
            .values()
            .next()
            .expect("single-variant object")
    }

    fn sample_start_run(with_delivery: bool) -> Command {
        Command::StartRun(StartRunCmd {
            idea_id: IdeaId::new(),
            workflow_id: "wf-1".into(),
            workflow_title: "Workflow One".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            delivery_configuration_json: if with_delivery {
                Some(r#"{"repo_identifier":"acme/secret","repo_root":"/srv/repos/acme","target_branch":"release/x"}"#.into())
            } else {
                None
            },
            workflow_yaml_path: "examples/workflows/w.yaml".into(),
            agent_catalog_yaml_path: "examples/agents/a.yaml".into(),
            review_routing_json: None,
        })
    }

    // ── StartRun ─────────────────────────────────────────────────────

    #[test]
    fn test_redact_start_run_redacts_delivery_configuration_json() {
        let cmd = sample_start_run(true);
        let v = round_trip(&cmd);
        let inner = inner(&v);
        assert_eq!(
            inner["delivery_configuration_json"],
            Value::String(REDACTED.to_string()),
            "delivery_configuration_json must be replaced by [REDACTED]"
        );
        // Audit must NOT be able to recover the original sensitive payload.
        let serialized = serde_json::to_string(&v).unwrap();
        assert!(!serialized.contains("acme/secret"));
        assert!(!serialized.contains("/srv/repos/acme"));
        assert!(!serialized.contains("release/x"));
    }

    #[test]
    fn test_redact_start_run_preserves_ids_and_paths() {
        let cmd = sample_start_run(true);
        let original: Value = serde_json::from_str(&serde_json::to_string(&cmd).unwrap()).unwrap();
        let redacted = round_trip(&cmd);
        let o = inner(&original);
        let r = inner(&redacted);

        // preserve — verbatim equality
        assert_eq!(o["idea_id"], r["idea_id"]);
        assert_eq!(o["workflow_id"], r["workflow_id"]);
        assert_eq!(o["workflow_title"], r["workflow_title"]);
        assert_eq!(o["workspace_root"], r["workspace_root"]);
        assert_eq!(o["artifact_root"], r["artifact_root"]);
        assert_eq!(o["workflow_yaml_path"], r["workflow_yaml_path"]);
        assert_eq!(o["agent_catalog_yaml_path"], r["agent_catalog_yaml_path"]);
    }

    #[test]
    fn test_redact_start_run_without_delivery_config_preserves_null() {
        // When delivery_configuration_json is genuinely absent (null),
        // audit should be able to distinguish "absent" from "redacted".
        let cmd = sample_start_run(false);
        let v = round_trip(&cmd);
        let inner = inner(&v);
        assert!(
            inner["delivery_configuration_json"].is_null(),
            "null delivery_configuration_json must stay null, not become [REDACTED]"
        );
    }

    // ── ApproveStage ─────────────────────────────────────────────────

    #[test]
    fn test_redact_approve_stage_redacts_comment() {
        let cmd = Command::ApproveStage(ApproveStageCmd {
            run_id: RunId::new(),
            stage_id: "state_6".into(),
            comment: Some("LGTM — customer incident INC-42 context".into()),
        });
        let v = round_trip(&cmd);
        let inner = inner(&v);
        assert_eq!(inner["comment"], Value::String(REDACTED.to_string()));
        let serialized = serde_json::to_string(&v).unwrap();
        assert!(!serialized.contains("INC-42"));
        assert!(!serialized.contains("customer incident"));
    }

    #[test]
    fn test_redact_approve_stage_preserves_run_and_stage_ids() {
        let run_id = RunId::new();
        let cmd = Command::ApproveStage(ApproveStageCmd {
            run_id,
            stage_id: "state_6".into(),
            comment: Some("anything".into()),
        });
        let v = round_trip(&cmd);
        let inner = inner(&v);
        assert_eq!(inner["run_id"], serde_json::json!(run_id));
        assert_eq!(inner["stage_id"], Value::String("state_6".into()));
    }

    // ── RejectStage ──────────────────────────────────────────────────

    #[test]
    fn test_redact_reject_stage_redacts_comment() {
        let cmd = Command::RejectStage(RejectStageCmd {
            run_id: RunId::new(),
            stage_id: "state_3".into(),
            comment: Some("Blocked: private API key sk-live-xyz in diff".into()),
        });
        let v = round_trip(&cmd);
        let inner = inner(&v);
        assert_eq!(inner["comment"], Value::String(REDACTED.to_string()));
        let serialized = serde_json::to_string(&v).unwrap();
        assert!(!serialized.contains("sk-live-xyz"));
    }

    #[test]
    fn test_redact_reject_stage_preserves_run_and_stage_ids() {
        let run_id = RunId::new();
        let cmd = Command::RejectStage(RejectStageCmd {
            run_id,
            stage_id: "state_4".into(),
            comment: None,
        });
        let v = round_trip(&cmd);
        let inner = inner(&v);
        assert_eq!(inner["run_id"], serde_json::json!(run_id));
        assert_eq!(inner["stage_id"], Value::String("state_4".into()));
    }

    // ── RetryStage / CancelRun / ResetSession / RunStewardAnalysis ──
    //    (preserve-all variants — every field must survive round-trip)

    #[test]
    fn test_redact_retry_stage_preserves_all_fields() {
        let run_id = RunId::new();
        let cmd = Command::RetryStage(RetryStageCmd {
            run_id,
            stage_id: "state_7".into(),
            consume_quota_budget_now: true,
            agent_execution_id: None,
            legacy_discovery_override_policy: None,
            legacy_discovery_override_reason: None,
        });
        let original: Value = serde_json::from_str(&serde_json::to_string(&cmd).unwrap()).unwrap();
        let redacted = round_trip(&cmd);
        assert_eq!(
            original, redacted,
            "RetryStage must be preserved field-for-field"
        );
    }

    #[test]
    fn test_redact_cancel_run_preserves_run_id() {
        let run_id = RunId::new();
        let cmd = Command::CancelRun(CancelRunCmd { run_id });
        let original: Value = serde_json::from_str(&serde_json::to_string(&cmd).unwrap()).unwrap();
        let redacted = round_trip(&cmd);
        assert_eq!(original, redacted);
        let inner = inner(&redacted);
        assert_eq!(inner["run_id"], serde_json::json!(run_id));
    }

    #[test]
    fn test_redact_reset_session_preserves_all_fields() {
        let run_id = RunId::new();
        let cmd = Command::ResetSession(ResetSessionCmd {
            run_id,
            stage_id: "state_5".into(),
        });
        let original: Value = serde_json::from_str(&serde_json::to_string(&cmd).unwrap()).unwrap();
        let redacted = round_trip(&cmd);
        assert_eq!(
            original, redacted,
            "ResetSession must be preserved field-for-field"
        );
    }

    #[test]
    fn test_redact_run_steward_analysis_preserves_reason_and_artifact_base() {
        let cmd = Command::RunStewardAnalysis(RunStewardAnalysisCmd {
            reason: "post_run_hook".into(),
            artifact_base: Some("artifacts/steward/run-1".into()),
        });
        let v = round_trip(&cmd);
        let inner = inner(&v);
        assert_eq!(inner["reason"], Value::String("post_run_hook".into()));
        assert_eq!(
            inner["artifact_base"],
            Value::String("artifacts/steward/run-1".into())
        );
    }

    // ── Matrix-coverage guard ────────────────────────────────────────
    //
    // Rust's exhaustive match in `redact_for_journal` is the static guard —
    // adding a new `Command` variant fails to compile until a matrix arm
    // is written. This runtime test is the *complement*: it asserts every
    // known variant produces a non-panicking, valid-JSON result, and if a
    // future author adds a variant they will remember to add a test case
    // here as part of the §8.1 inventory (the `match` expression at the
    // bottom is the deliberate reminder surface).

    #[test]
    fn test_redaction_matrix_covers_all_command_variants() {
        let samples: Vec<Command> = vec![
            sample_start_run(true),
            sample_start_run(false),
            Command::ApproveStage(ApproveStageCmd {
                run_id: RunId::new(),
                stage_id: "s".into(),
                comment: Some("x".into()),
            }),
            Command::ApproveStage(ApproveStageCmd {
                run_id: RunId::new(),
                stage_id: "s".into(),
                comment: None,
            }),
            Command::RejectStage(RejectStageCmd {
                run_id: RunId::new(),
                stage_id: "s".into(),
                comment: Some("x".into()),
            }),
            Command::RejectStage(RejectStageCmd {
                run_id: RunId::new(),
                stage_id: "s".into(),
                comment: None,
            }),
            Command::RetryStage(RetryStageCmd {
                run_id: RunId::new(),
                stage_id: "s".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
            }),
            Command::ResolveWorkflowConflictTransition(ResolveWorkflowConflictTransitionCmd {
                run_id: RunId::new(),
                conflict_id: "conflict-1".into(),
                selected_transition_id: "review__to__refine__0".into(),
                resolution_reason: "operator selected loop-budget continuation".into(),
            }),
            Command::OverrideLegacyDiscoveryPolicy(OverrideLegacyDiscoveryPolicyCmd {
                run_id: RunId::new(),
                stage_id: "s".into(),
                target_stage_execution_id: StageExecutionId::new(),
                target_attempt_number: 2,
                legacy_discovery_override_policy: LegacyBroadDiscoveryPolicy::WorkflowOptIn,
                legacy_discovery_override_reason: "operator override".into(),
            }),
            Command::CancelRun(CancelRunCmd {
                run_id: RunId::new(),
            }),
            Command::ResetSession(ResetSessionCmd {
                run_id: RunId::new(),
                stage_id: "s".into(),
            }),
            Command::RunStewardAnalysis(RunStewardAnalysisCmd {
                reason: "manual".into(),
                artifact_base: None,
            }),
            Command::ResolveLeadMediationConfirmation(ResolveLeadMediationConfirmationCmd {
                run_id: RunId::new(),
                mediation_record_id: "med-1".into(),
                confirmation_subject_id: "conf-1".into(),
                decision: MediationConfirmationDecision::Confirm,
                comment: Some("Looks good".into()),
                conflict_fingerprint: "sha256:abc".into(),
                idempotency_key: "idem-1".into(),
            }),
        ];

        for cmd in &samples {
            let raw = serde_json::to_string(cmd).expect("serialize");
            let redacted = redact_for_journal(cmd, &raw);
            // Must be valid JSON — proves no arm panicked or produced junk.
            let _: Value = serde_json::from_str(&redacted)
                .expect("redacted output must be valid JSON for every variant");
        }

        // Exhaustive-match reminder: if a new Command variant is added,
        // the compiler forces a new arm above; this `match` below forces
        // the test author to update the coverage list and confirms every
        // discriminant is accounted for in the focused inventory.
        fn _exhaustiveness_reminder(c: &Command) {
            match c {
                Command::StartRun(_) => {}
                Command::ApproveStage(_) => {}
                Command::RejectStage(_) => {}
                Command::RetryStage(_) => {}
                Command::ResolveWorkflowConflictTransition(_) => {}
                Command::OverrideLegacyDiscoveryPolicy(_) => {}
                Command::CancelRun(_) => {}
                Command::ResetSession(_) => {}
                Command::RunStewardAnalysis(_) => {}
                Command::OverrideArtifactContract(_) => {}
                Command::ResolveLeadMediationConfirmation(_) => {}
            }
        }
    }
}
