use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryEvaluation {
    pub status: String,
    pub workflow_route_hint: String,
    pub followup_proposal_required: bool,
    pub has_release_blocking_external_blockers: bool,
    pub has_no_release_blocking_external_blockers: bool,
    pub invalid_claim_count: u64,
    pub primary_owner_class: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoundaryEvaluationContext {
    pub run_id: String,
    pub stage_execution_id: String,
    pub assessment_generation_id: String,
    pub updated_at: String,
    pub server_verified_no_progress_signatures: Vec<String>,
}

#[derive(Clone, Debug)]
struct BoundaryBlocker {
    id: String,
    summary: String,
    blocker_signature_id: String,
    evidence_fingerprint: String,
    evidence_freshness: String,
    source_artifact_generation_id: Option<String>,
    observed_after_stage_execution_id: String,
    observed_after_agent_execution_id: String,
    owner_class: String,
    blocker_class: String,
    is_code_writer_blocking: bool,
    severity: String,
    release_blocking: bool,
    server_verified_no_progress: bool,
    no_progress_repeat_count: Option<u64>,
    budget_source: Option<String>,
    budget_remaining: Option<u64>,
    last_progress_fingerprint: Option<String>,
    allowed_workflow_routes: Vec<String>,
    forbidden_routes: Vec<String>,
    gate_command: Option<String>,
    evidence_refs: Vec<String>,
    validation_errors: Vec<String>,
}

pub fn evaluate_quality_gate_boundary_assessment(
    assessment_generation_id: &str,
    assessment: &serde_json::Value,
) -> Result<BoundaryEvaluation> {
    evaluate_quality_gate_boundary_assessment_with_context(
        BoundaryEvaluationContext {
            run_id: "unknown".to_string(),
            stage_execution_id: "unknown".to_string(),
            assessment_generation_id: assessment_generation_id.to_string(),
            updated_at: "unknown".to_string(),
            server_verified_no_progress_signatures: Vec::new(),
        },
        assessment,
    )
}

pub fn evaluate_quality_gate_boundary_assessment_with_context(
    context: BoundaryEvaluationContext,
    assessment: &serde_json::Value,
) -> Result<BoundaryEvaluation> {
    let (blockers, mut validation_errors) = parse_blockers(assessment)?;
    let server_verified_no_progress_signatures = context
        .server_verified_no_progress_signatures
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut warnings: Vec<String> = Vec::new();

    if assessment
        .get("schema_version")
        .and_then(|value| value.as_str())
        != Some("quality_gate_blocker_assessment_v1")
    {
        validation_errors.push(
            "assessment schema_version must be quality_gate_blocker_assessment_v1".to_string(),
        );
    }
    for blocker in &blockers {
        validation_errors.extend(blocker.validation_errors.iter().cloned());
    }

    let invalid_claims: Vec<_> = blockers
        .iter()
        .filter(|blocker| {
            matches!(
                blocker.owner_class.as_str(),
                "invalid_claim" | "invalid_blocker_claim"
            ) || matches!(
                blocker.blocker_class.as_str(),
                "invalid_claim" | "invalid_blocker_claim"
            )
        })
        .collect();
    let lower_layer_status = blockers.iter().find_map(lower_layer_status_for_blocker);
    let stale_review = blockers.iter().any(|blocker| {
        matches!(
            blocker.evidence_freshness.as_str(),
            "stale" | "unknown" | "superseded"
        ) || matches!(
            blocker.owner_class.as_str(),
            "review_refresh" | "stale_review"
        )
    });
    let local_code_tail = blockers.iter().any(|blocker| {
        blocker.is_code_writer_blocking
            && blocker.evidence_freshness == "fresh"
            && matches!(
                blocker.owner_class.as_str(),
                "code_writer" | "local_code" | "docs_guardian" | "test_owner" | "implementation"
            )
    });
    let blocked_no_progress_blocker = blockers.iter().find(|blocker| {
        server_verified_no_progress_signatures.contains(&blocker.blocker_signature_id)
            && (matches!(
                blocker.owner_class.as_str(),
                "blocked_no_progress" | "no_progress"
            ) || matches!(
                blocker.blocker_class.as_str(),
                "blocked_no_progress" | "no_progress"
            ))
    });
    let review_owner_refresh_required = blockers.iter().any(|blocker| {
        matches!(
            blocker.owner_class.as_str(),
            "security_reviewer" | "prepush_reviewer" | "implementation_auditor" | "unknown"
        )
    });
    let blocked_no_progress = blocked_no_progress_blocker.is_some();
    let followup_proposal_required = blockers
        .iter()
        .any(|blocker| blocker.owner_class == "followup_proposal");
    let has_release_blocking_external_blockers = blockers.iter().any(|blocker| {
        blocker.release_blocking
            && matches!(
                blocker.owner_class.as_str(),
                "external_blocker"
                    | "external_evidence"
                    | "external_environment"
                    | "release_evidence"
                    | "operator"
                    | "remote_host"
            )
    });
    let release_blocking_external_blocker_count = blockers
        .iter()
        .filter(|blocker| {
            blocker.release_blocking
                && matches!(
                    blocker.owner_class.as_str(),
                    "external_blocker"
                        | "external_evidence"
                        | "external_environment"
                        | "release_evidence"
                        | "operator"
                        | "remote_host"
                )
        })
        .count() as u64;
    let external_blocker_count = blockers
        .iter()
        .filter(|blocker| {
            matches!(
                blocker.owner_class.as_str(),
                "external_blocker"
                    | "external_evidence"
                    | "external_environment"
                    | "release_evidence"
                    | "operator"
                    | "remote_host"
            )
        })
        .count() as u64;
    let followup_code_tail_count = blockers
        .iter()
        .filter(|blocker| blocker.owner_class == "followup_proposal")
        .count() as u64;
    let review_refresh_required_count = blockers
        .iter()
        .filter(|blocker| {
            matches!(
                blocker.evidence_freshness.as_str(),
                "stale" | "unknown" | "superseded"
            )
        })
        .count() as u64;

    let status = if !validation_errors.is_empty() {
        "invalid_claim".to_string()
    } else if let Some(status) = lower_layer_status {
        status.to_string()
    } else if stale_review {
        "review_refresh_required".to_string()
    } else if local_code_tail {
        "local_code_tail_present".to_string()
    } else if review_owner_refresh_required {
        "review_refresh_required".to_string()
    } else if !invalid_claims.is_empty() {
        "invalid_claim".to_string()
    } else if blocked_no_progress {
        "blocked_no_progress".to_string()
    } else if followup_proposal_required || has_release_blocking_external_blockers {
        "awaiting_human_boundary_approval".to_string()
    } else if blockers.is_empty() {
        "pass".to_string()
    } else {
        "awaiting_human_boundary_approval".to_string()
    };

    if status == "invalid_claim" && invalid_claims.is_empty() {
        validation_errors.push(
            "assessment contains blockers without required server-owned evidence fields"
                .to_string(),
        );
    } else if status == "invalid_claim" {
        validation_errors
            .push("assessment contains invalid or locally solvable blocker claims".to_string());
    }
    if status == "local_code_tail_present" {
        warnings.push(
            "local code-owned work remains; boundary acceptance cannot route to release"
                .to_string(),
        );
    }

    let workflow_route_hint = route_hint_for_status(&status).to_string();
    let primary_owner_class = primary_owner_class(&blockers, &status);
    let hard_blockers: Vec<_> = blockers
        .iter()
        .filter(|blocker| blocker.release_blocking || blocker.owner_class != "invalid_claim")
        .map(blocker_json)
        .collect();
    let blocker_payloads: Vec<_> = blockers.iter().map(blocker_json).collect();
    let local_work_complete =
        !local_code_tail && lower_layer_status.is_none() && validation_errors.is_empty();
    let projection_integrity = if validation_errors.is_empty() {
        "valid"
    } else {
        "tamper_detected"
    };

    let payload = serde_json::json!({
        "schema_version": "blocker_boundary_status_v1",
        "run_id": context.run_id,
        "stage_execution_id": context.stage_execution_id,
        "status": status,
        "assessment_generation_id": context.assessment_generation_id,
        "local_work_complete": local_work_complete,
        "local_code_tail_present": local_code_tail,
        "followup_proposal_required": followup_proposal_required,
        "has_release_blocking_external_blockers": has_release_blocking_external_blockers,
        "has_no_release_blocking_external_blockers": !has_release_blocking_external_blockers,
        "external_blocker_count": external_blocker_count,
        "release_blocking_external_blocker_count": release_blocking_external_blocker_count,
        "followup_code_tail_count": followup_code_tail_count,
        "invalid_claim_count": invalid_claims.len() as u64,
        "review_refresh_required_count": review_refresh_required_count,
        "primary_owner_class": primary_owner_class,
        "blockers": blocker_payloads,
        "hard_blockers": hard_blockers,
        "workflow_route_hint": workflow_route_hint,
        "projection_integrity": projection_integrity,
        "updated_at": context.updated_at,
        "allowed_workflow_routes": allowed_routes_for_status(&status),
        "no_progress_repeat_count": blocked_no_progress_blocker.and_then(|blocker| blocker.no_progress_repeat_count),
        "budget_source": blocked_no_progress_blocker.and_then(|blocker| blocker.budget_source.clone()),
        "budget_remaining": blocked_no_progress_blocker.and_then(|blocker| blocker.budget_remaining),
        "last_progress_fingerprint": blocked_no_progress_blocker.and_then(|blocker| blocker.last_progress_fingerprint.clone()),
        "validation_errors": validation_errors,
        "warnings": warnings,
    });

    Ok(BoundaryEvaluation {
        status,
        workflow_route_hint,
        followup_proposal_required,
        has_release_blocking_external_blockers,
        has_no_release_blocking_external_blockers: !has_release_blocking_external_blockers,
        invalid_claim_count: invalid_claims.len() as u64,
        primary_owner_class,
        payload,
    })
}

fn parse_blockers(assessment: &serde_json::Value) -> Result<(Vec<BoundaryBlocker>, Vec<String>)> {
    let mut raw = Vec::new();
    let mut validation_errors = Vec::new();
    for field_name in [
        "blockers",
        "candidate_blockers",
        "external_blockers",
        "followup_code_tail",
        "local_code_tail",
    ] {
        match assessment.get(field_name) {
            Some(serde_json::Value::Array(items)) => raw.extend(items.iter().cloned()),
            Some(_) => validation_errors.push(format!("assessment.{field_name} must be an array")),
            None if field_name == "blockers" => {
                validation_errors.push("assessment.blockers is required".to_string());
            }
            None => {}
        }
    }
    let blockers = raw
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            let object = value
                .as_object()
                .with_context(|| format!("blocker at index {idx} must be an object"))?;
            let mut validation_errors = Vec::new();
            let owner_class = string_field(object, "owner_class")
                .or_else(|| string_field(object, "owner"))
                .unwrap_or_else(|| {
                    validation_errors.push(format!("blocker[{idx}].owner_class is required"));
                    "unknown".to_string()
                });
            if !valid_owner_class(&owner_class) {
                validation_errors.push(format!(
                    "blocker[{idx}].owner_class has unknown value {owner_class}"
                ));
            }
            let blocker_class = string_field(object, "blocker_class")
                .or_else(|| string_field(object, "class"))
                .unwrap_or_else(|| {
                    validation_errors.push(format!("blocker[{idx}].blocker_class is required"));
                    owner_class.clone()
                });
            if !valid_blocker_class(&blocker_class) {
                validation_errors.push(format!(
                    "blocker[{idx}].blocker_class has unknown value {blocker_class}"
                ));
            }
            let release_blocking = object
                .get("release_blocking")
                .or_else(|| object.get("is_release_blocking"))
                .and_then(|value| value.as_bool())
                .unwrap_or_else(|| {
                    validation_errors.push(format!("blocker[{idx}].release_blocking is required"));
                    false
                });
            let allowed_workflow_routes = string_array_field(object, "allowed_workflow_routes");
            let forbidden_routes = string_array_field(object, "forbidden_routes");
            if allowed_workflow_routes.is_empty() {
                validation_errors.push(format!(
                    "blocker[{idx}].allowed_workflow_routes must be non-empty"
                ));
            }
            if !has_array_field(object, "forbidden_routes") {
                validation_errors.push(format!("blocker[{idx}].forbidden_routes is required"));
            }
            let evidence_refs = string_array_field(object, "evidence_refs");
            if evidence_refs.is_empty() {
                validation_errors.push(format!("blocker[{idx}].evidence_refs must be non-empty"));
            }
            let evidence_freshness = string_field(object, "evidence_freshness")
                .or_else(|| string_field(object, "freshness"))
                .unwrap_or_else(|| {
                    validation_errors
                        .push(format!("blocker[{idx}].evidence_freshness is required"));
                    "unknown".to_string()
                });
            if !matches!(
                evidence_freshness.as_str(),
                "fresh" | "stale" | "unknown" | "superseded"
            ) {
                validation_errors.push(format!(
                    "blocker[{idx}].evidence_freshness has unknown value {evidence_freshness}"
                ));
            }
            let is_code_writer_blocking = object
                .get("is_code_writer_blocking")
                .and_then(|value| value.as_bool())
                .unwrap_or_else(|| {
                    matches!(
                        owner_class.as_str(),
                        "code_writer"
                            | "local_code"
                            | "docs_guardian"
                            | "test_owner"
                            | "implementation"
                    )
                });
            let server_verified_no_progress = object
                .get("server_verified_no_progress")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let no_progress_repeat_count = object
                .get("no_progress_repeat_count")
                .and_then(|value| value.as_u64());
            let budget_source = string_field(object, "budget_source");
            let budget_remaining = object.get("budget_remaining").and_then(|value| value.as_u64());
            let last_progress_fingerprint = string_field(object, "last_progress_fingerprint");
            if server_verified_no_progress {
                if no_progress_repeat_count.is_none() {
                    validation_errors.push(format!(
                        "blocker[{idx}].no_progress_repeat_count is required for server_verified_no_progress"
                    ));
                }
                if budget_source.is_none() {
                    validation_errors.push(format!(
                        "blocker[{idx}].budget_source is required for server_verified_no_progress"
                    ));
                }
                if budget_remaining.is_none() {
                    validation_errors.push(format!(
                        "blocker[{idx}].budget_remaining is required for server_verified_no_progress"
                    ));
                }
                if last_progress_fingerprint.is_none() {
                    validation_errors.push(format!(
                        "blocker[{idx}].last_progress_fingerprint is required for server_verified_no_progress"
                    ));
                }
            }

            Ok(BoundaryBlocker {
                id: string_field(object, "id").unwrap_or_else(|| format!("blocker-{idx}")),
                summary: string_field(object, "summary")
                    .or_else(|| string_field(object, "description"))
                    .unwrap_or_else(|| {
                        validation_errors.push(format!("blocker[{idx}].summary is required"));
                        "unspecified blocker".to_string()
                    }),
                blocker_signature_id: required_string_field(
                    object,
                    idx,
                    "blocker_signature_id",
                    &mut validation_errors,
                )
                .unwrap_or_else(|| format!("missing-blocker-signature-{idx}")),
                evidence_fingerprint: required_string_field(
                    object,
                    idx,
                    "evidence_fingerprint",
                    &mut validation_errors,
                )
                .unwrap_or_else(|| format!("missing-evidence-fingerprint-{idx}")),
                evidence_freshness,
                source_artifact_generation_id: required_string_field(
                    object,
                    idx,
                    "source_artifact_generation_id",
                    &mut validation_errors,
                ),
                observed_after_stage_execution_id: string_field(
                    object,
                    "observed_after_stage_execution_id",
                )
                .unwrap_or_else(|| {
                    validation_errors.push(format!(
                        "blocker[{idx}].observed_after_stage_execution_id is required"
                    ));
                    "unknown".to_string()
                }),
                observed_after_agent_execution_id: string_field(
                    object,
                    "observed_after_agent_execution_id",
                )
                .unwrap_or_else(|| {
                    validation_errors.push(format!(
                        "blocker[{idx}].observed_after_agent_execution_id is required"
                    ));
                    "unknown".to_string()
                }),
                owner_class,
                blocker_class,
                is_code_writer_blocking,
                severity: required_string_field(object, idx, "severity", &mut validation_errors)
                    .map(|severity| {
                        if !matches!(severity.as_str(), "hard" | "soft" | "advisory") {
                            validation_errors.push(format!(
                                "blocker[{idx}].severity has unknown value {severity}"
                            ));
                        }
                        severity
                    })
                    .unwrap_or_else(|| "hard".to_string()),
                release_blocking,
                server_verified_no_progress,
                no_progress_repeat_count,
                budget_source,
                budget_remaining,
                last_progress_fingerprint,
                allowed_workflow_routes,
                forbidden_routes,
                gate_command: string_field(object, "gate_command").or_else(|| {
                    validation_errors.push(format!("blocker[{idx}].gate_command is required"));
                    None
                }),
                evidence_refs,
                validation_errors,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((blockers, validation_errors))
}

fn valid_owner_class(owner_class: &str) -> bool {
    matches!(
        owner_class,
        "output_settlement"
            | "missing_required_outputs"
            | "side_effect"
            | "side_effect_reconciliation"
            | "runtime_recovery"
            | "provider_recovery"
            | "review_refresh"
            | "stale_review"
            | "security_reviewer"
            | "prepush_reviewer"
            | "implementation_auditor"
            | "unknown"
            | "code_writer"
            | "local_code"
            | "docs_guardian"
            | "test_owner"
            | "implementation"
            | "invalid_claim"
            | "invalid_blocker_claim"
            | "blocked_no_progress"
            | "no_progress"
            | "followup_proposal"
            | "external_blocker"
            | "external_evidence"
            | "external_environment"
            | "release_evidence"
            | "operator"
            | "remote_host"
    )
}

fn valid_blocker_class(blocker_class: &str) -> bool {
    matches!(
        blocker_class,
        "output_settlement"
            | "missing_required_outputs"
            | "side_effect"
            | "side_effect_reconciliation"
            | "runtime_recovery"
            | "provider_recovery"
            | "review_refresh"
            | "stale_review"
            | "local_code_tail"
            | "local_code"
            | "code_issue"
            | "docs_guardian"
            | "test_failure"
            | "implementation"
            | "invalid_claim"
            | "invalid_blocker_claim"
            | "blocked_no_progress"
            | "no_progress"
            | "followup_proposal"
            | "external_blocker"
            | "external_evidence"
            | "external_environment"
            | "remote_environment_required"
            | "release_evidence"
            | "operator_decision_required"
    )
}

fn string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Option<String> {
    object
        .get(field)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn required_string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    idx: usize,
    field: &str,
    validation_errors: &mut Vec<String>,
) -> Option<String> {
    string_field(object, field).or_else(|| {
        validation_errors.push(format!("blocker[{idx}].{field} is required"));
        None
    })
}

fn string_array_field(
    object: &serde_json::Map<String, serde_json::Value>,
    field: &str,
) -> Vec<String> {
    object
        .get(field)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn has_array_field(object: &serde_json::Map<String, serde_json::Value>, field: &str) -> bool {
    object
        .get(field)
        .is_some_and(|value| value.as_array().is_some())
}

fn lower_layer_status_for_blocker(blocker: &BoundaryBlocker) -> Option<&'static str> {
    let owner = blocker.owner_class.as_str();
    let class = blocker.blocker_class.as_str();
    if matches!(owner, "output_settlement" | "missing_required_outputs")
        || matches!(class, "output_settlement" | "missing_required_outputs")
    {
        Some("output_settlement_required")
    } else if matches!(owner, "side_effect" | "side_effect_reconciliation")
        || matches!(class, "side_effect" | "side_effect_reconciliation")
    {
        Some("side_effect_reconciliation_required")
    } else if matches!(owner, "runtime_recovery" | "provider_recovery")
        || matches!(class, "runtime_recovery" | "provider_recovery")
    {
        Some("runtime_recovery_required")
    } else {
        None
    }
}

fn route_hint_for_status(status: &str) -> &'static str {
    match status {
        "output_settlement_required" => "output_settlement_recovery",
        "side_effect_reconciliation_required" => "side_effect_reconciliation",
        "runtime_recovery_required" => "runtime_recovery",
        "review_refresh_required" => "implementation_review_refresh",
        "local_code_tail_present" => "implementation_refine",
        "invalid_claim" => "implementation_review_refresh",
        "blocked_no_progress" | "awaiting_human_boundary_approval" => "human_boundary_approval",
        "pass" => "next_release_or_closeout_state",
        _ => "implementation_review_refresh",
    }
}

fn allowed_routes_for_status(status: &str) -> Vec<&'static str> {
    match status {
        "output_settlement_required" => vec!["output_settlement_recovery"],
        "side_effect_reconciliation_required" => vec!["side_effect_reconciliation"],
        "runtime_recovery_required" => vec!["runtime_recovery"],
        "review_refresh_required" => vec!["implementation_review_refresh"],
        "local_code_tail_present" => vec!["implementation_refine"],
        "invalid_claim" => vec!["implementation_review_refresh"],
        "blocked_no_progress" | "awaiting_human_boundary_approval" => {
            vec!["human_boundary_approval"]
        }
        "pass" => vec!["next_release_or_closeout_state"],
        _ => vec!["implementation_review_refresh"],
    }
}

fn primary_owner_class(blockers: &[BoundaryBlocker], status: &str) -> String {
    blockers
        .first()
        .map(|blocker| blocker.owner_class.clone())
        .unwrap_or_else(|| {
            if status == "pass" {
                "none".to_string()
            } else {
                "unknown".to_string()
            }
        })
}

fn blocker_json(blocker: &BoundaryBlocker) -> serde_json::Value {
    serde_json::json!({
        "id": blocker.id,
        "summary": blocker.summary,
        "blocker_signature_id": blocker.blocker_signature_id,
        "evidence_fingerprint": blocker.evidence_fingerprint,
        "evidence_freshness": blocker.evidence_freshness,
        "source_artifact_generation_id": blocker.source_artifact_generation_id,
        "observed_after_stage_execution_id": blocker.observed_after_stage_execution_id,
        "observed_after_agent_execution_id": blocker.observed_after_agent_execution_id,
        "owner_class": blocker.owner_class,
        "is_code_writer_blocking": blocker.is_code_writer_blocking,
        "class": blocker.blocker_class,
        "severity": blocker.severity,
        "release_blocking": blocker.release_blocking,
        "server_verified_no_progress": blocker.server_verified_no_progress,
        "no_progress_repeat_count": blocker.no_progress_repeat_count,
        "budget_source": blocker.budget_source.clone(),
        "budget_remaining": blocker.budget_remaining,
        "last_progress_fingerprint": blocker.last_progress_fingerprint.clone(),
        "gate_command": blocker.gate_command,
        "evidence_refs": blocker.evidence_refs,
        "allowed_workflow_routes": if blocker.allowed_workflow_routes.is_empty() {
            allowed_routes_for_status(route_status_for_owner(&blocker.owner_class))
        } else {
            blocker.allowed_workflow_routes.iter().map(String::as_str).collect()
        },
        "forbidden_routes": blocker.forbidden_routes,
    })
}

fn route_status_for_owner(owner_class: &str) -> &'static str {
    match owner_class {
        "output_settlement" | "missing_required_outputs" => "output_settlement_required",
        "side_effect" | "side_effect_reconciliation" => "side_effect_reconciliation_required",
        "runtime_recovery" | "provider_recovery" => "runtime_recovery_required",
        "review_refresh"
        | "stale_review"
        | "security_reviewer"
        | "prepush_reviewer"
        | "implementation_auditor"
        | "unknown" => "review_refresh_required",
        "code_writer" | "local_code" | "docs_guardian" | "test_owner" | "implementation" => {
            "local_code_tail_present"
        }
        "invalid_claim" | "invalid_blocker_claim" => "invalid_claim",
        "blocked_no_progress" | "no_progress" => "blocked_no_progress",
        _ => "awaiting_human_boundary_approval",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocker(
        owner_class: &str,
        blocker_class: &str,
        release_blocking: bool,
    ) -> serde_json::Value {
        serde_json::json!({
            "id": format!("{owner_class}-{blocker_class}"),
            "summary": "test blocker",
            "blocker_signature_id": format!("sig-{owner_class}-{blocker_class}"),
            "evidence_fingerprint": format!("fingerprint-{owner_class}-{blocker_class}"),
            "source_artifact_generation_id": format!("generation-{owner_class}-{blocker_class}"),
            "observed_after_stage_execution_id": "stage-exec-1",
            "observed_after_agent_execution_id": "agent-exec-1",
            "owner_class": owner_class,
            "blocker_class": blocker_class,
            "evidence_freshness": "fresh",
            "severity": "hard",
            "release_blocking": release_blocking,
            "allowed_workflow_routes": ["human_boundary_approval"],
            "forbidden_routes": [],
            "gate_command": "quality_gate_blocker_boundary",
            "evidence_refs": ["artifact_contracts:test"]
        })
    }

    #[test]
    fn p094_external_release_blocker_requires_human_boundary_approval() {
        let assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [blocker("external_evidence", "remote_environment_required", true)]
        });
        let evaluated =
            evaluate_quality_gate_boundary_assessment("assessment-1", &assessment).unwrap();
        assert_eq!(evaluated.status, "awaiting_human_boundary_approval");
        assert!(evaluated.has_release_blocking_external_blockers);
        assert_eq!(evaluated.workflow_route_hint, "human_boundary_approval");
    }

    #[test]
    fn p094_lower_layer_output_settlement_wins_before_boundary() {
        let assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [blocker("output_settlement", "missing_required_outputs", true)]
        });
        let evaluated =
            evaluate_quality_gate_boundary_assessment("assessment-2", &assessment).unwrap();
        assert_eq!(evaluated.status, "output_settlement_required");
        assert_eq!(evaluated.workflow_route_hint, "output_settlement_recovery");
    }

    #[test]
    fn p094_local_code_tail_does_not_route_to_boundary_approval() {
        let assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [blocker("code_writer", "local_code_tail", false)]
        });
        let evaluated =
            evaluate_quality_gate_boundary_assessment("assessment-3", &assessment).unwrap();
        assert_eq!(evaluated.status, "local_code_tail_present");
        assert_eq!(evaluated.workflow_route_hint, "implementation_refine");
    }

    #[test]
    fn p094_unknown_freshness_requires_review_refresh_before_code_tail() {
        let mut stale = blocker("code_writer", "local_code_tail", false);
        stale["evidence_freshness"] = serde_json::json!("unknown");
        let assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [stale]
        });
        let evaluated =
            evaluate_quality_gate_boundary_assessment("assessment-4", &assessment).unwrap();
        assert_eq!(evaluated.status, "review_refresh_required");
        assert_eq!(
            evaluated.workflow_route_hint,
            "implementation_review_refresh"
        );
    }

    #[test]
    fn p094_incomplete_blocker_claim_fails_closed_before_boundary_approval() {
        let assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [{
                "owner_class": "external_evidence",
                "blocker_class": "remote_environment_required",
                "release_blocking": true
            }]
        });
        let evaluated =
            evaluate_quality_gate_boundary_assessment("assessment-5", &assessment).unwrap();
        assert_eq!(evaluated.status, "invalid_claim");
        assert_eq!(evaluated.payload["projection_integrity"], "tamper_detected");
        assert!(evaluated.payload["validation_errors"]
            .as_array()
            .expect("validation errors should be present")
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("blocker_signature_id"))));
    }

    #[test]
    fn p094_external_environment_counts_as_release_blocking_external() {
        let assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [blocker("external_environment", "remote_environment_required", true)]
        });
        let evaluated =
            evaluate_quality_gate_boundary_assessment("assessment-6", &assessment).unwrap();
        assert_eq!(evaluated.status, "awaiting_human_boundary_approval");
        assert!(evaluated.has_release_blocking_external_blockers);
    }

    #[test]
    fn p094_review_owner_classes_route_to_review_refresh_not_code_writer() {
        for owner_class in [
            "security_reviewer",
            "prepush_reviewer",
            "implementation_auditor",
        ] {
            let mut review_blocker = blocker(owner_class, "review_refresh", false);
            review_blocker["allowed_workflow_routes"] =
                serde_json::json!(["implementation_review_refresh"]);
            let evaluated = evaluate_quality_gate_boundary_assessment(
                &format!("assessment-review-owner-{owner_class}"),
                &serde_json::json!({
                    "schema_version": "quality_gate_blocker_assessment_v1",
                    "blockers": [review_blocker]
                }),
            )
            .unwrap();
            assert_eq!(evaluated.status, "review_refresh_required");
            assert_eq!(
                evaluated.workflow_route_hint,
                "implementation_review_refresh"
            );
            assert_eq!(evaluated.primary_owner_class, owner_class);
            assert_eq!(evaluated.payload["local_code_tail_present"], false);
            assert_eq!(
                evaluated.payload["allowed_workflow_routes"],
                serde_json::json!(["implementation_review_refresh"])
            );
            assert!(
                !evaluated.payload["validation_errors"]
                    .as_array()
                    .expect("validation errors array")
                    .iter()
                    .any(|error| error
                        .as_str()
                        .is_some_and(|error| error.contains("owner_class has unknown value"))),
                "{owner_class} must be a known P094 owner_class"
            );
        }

        let mut unknown_owner = blocker("unknown", "review_refresh", false);
        unknown_owner["allowed_workflow_routes"] =
            serde_json::json!(["implementation_review_refresh"]);
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-known-unknown-owner",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [unknown_owner]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "review_refresh_required");
        assert_eq!(
            evaluated.workflow_route_hint,
            "implementation_review_refresh"
        );
        assert_eq!(evaluated.payload["local_code_tail_present"], false);
        assert!(
            !evaluated.payload["validation_errors"]
                .as_array()
                .expect("validation errors array")
                .iter()
                .any(|error| error
                    .as_str()
                    .is_some_and(|error| error.contains("owner_class has unknown value"))),
            "proposal-defined owner_class=unknown should fail closed by routing, not by future-enum rejection"
        );
    }

    #[test]
    fn p094_blocked_no_progress_requires_server_verified_marker() {
        let assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [blocker("blocked_no_progress", "no_progress", true)]
        });
        let evaluated =
            evaluate_quality_gate_boundary_assessment("assessment-7", &assessment).unwrap();
        assert_eq!(evaluated.status, "awaiting_human_boundary_approval");

        let mut verified = blocker("blocked_no_progress", "no_progress", true);
        verified["server_verified_no_progress"] = serde_json::json!(true);
        let incomplete_verified_assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [verified]
        });
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-8",
            &incomplete_verified_assessment,
        )
        .unwrap();
        assert_eq!(evaluated.status, "invalid_claim");

        let mut complete_verified = blocker("blocked_no_progress", "no_progress", true);
        complete_verified["server_verified_no_progress"] = serde_json::json!(true);
        complete_verified["no_progress_repeat_count"] = serde_json::json!(2);
        complete_verified["budget_source"] =
            serde_json::json!("workflow.vars.max_implementation_revision_cycles");
        complete_verified["budget_remaining"] = serde_json::json!(0);
        complete_verified["last_progress_fingerprint"] = serde_json::json!("sha256:prior");
        let complete_verified_assessment = serde_json::json!({
            "schema_version": "quality_gate_blocker_assessment_v1",
            "blockers": [complete_verified]
        });
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-9",
            &complete_verified_assessment,
        )
        .unwrap();
        assert_eq!(
            evaluated.status, "awaiting_human_boundary_approval",
            "agent-authored server_verified_no_progress must not be enough"
        );

        let evaluated = evaluate_quality_gate_boundary_assessment_with_context(
            BoundaryEvaluationContext {
                run_id: "run-1".into(),
                stage_execution_id: "stage-1".into(),
                assessment_generation_id: "assessment-9".into(),
                updated_at: "2026-05-26T00:00:00Z".into(),
                server_verified_no_progress_signatures: vec![
                    "sig-blocked_no_progress-no_progress".into()
                ],
            },
            &complete_verified_assessment,
        )
        .unwrap();
        assert_eq!(evaluated.status, "blocked_no_progress");
        assert_eq!(evaluated.payload["no_progress_repeat_count"], 2);
        assert_eq!(
            evaluated.payload["budget_source"],
            "workflow.vars.max_implementation_revision_cycles"
        );
        assert_eq!(evaluated.payload["budget_remaining"], 0);
        assert_eq!(
            evaluated.payload["last_progress_fingerprint"],
            "sha256:prior"
        );
        assert_eq!(
            evaluated.payload["blockers"][0]["no_progress_repeat_count"],
            2
        );
    }

    #[test]
    fn p094_unknown_enum_or_missing_required_route_fields_fail_closed() {
        let mut invalid_owner = blocker("external_evidence", "remote_environment_required", true);
        invalid_owner["owner_class"] = serde_json::json!("future_unknown_owner");
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-10",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [invalid_owner]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "invalid_claim");

        let mut invalid_blocker_class =
            blocker("external_evidence", "remote_environment_required", true);
        invalid_blocker_class["blocker_class"] = serde_json::json!("future_unknown_blocker_class");
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-10b",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [invalid_blocker_class]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "invalid_claim");
        assert!(evaluated.payload["validation_errors"]
            .as_array()
            .expect("validation errors should be present")
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error.contains("blocker_class has unknown value"))));

        let mut missing_routes = blocker("external_evidence", "remote_environment_required", true);
        missing_routes
            .as_object_mut()
            .expect("object blocker")
            .remove("allowed_workflow_routes");
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-11",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [missing_routes]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "invalid_claim");

        let mut missing_gate_command =
            blocker("external_evidence", "remote_environment_required", true);
        missing_gate_command
            .as_object_mut()
            .expect("object blocker")
            .remove("gate_command");
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-12",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [missing_gate_command]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "invalid_claim");

        let mut advisory = blocker("external_evidence", "remote_environment_required", true);
        advisory["severity"] = serde_json::json!("advisory");
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-13",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [advisory]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "awaiting_human_boundary_approval");

        let mut medium = blocker("external_evidence", "remote_environment_required", true);
        medium["severity"] = serde_json::json!("medium");
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-14",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [medium]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "invalid_claim");

        let mut unknown_freshness =
            blocker("external_evidence", "remote_environment_required", true);
        unknown_freshness["evidence_freshness"] = serde_json::json!("future_freshness");
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-15",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": [unknown_freshness]
            }),
        )
        .unwrap();
        assert_eq!(evaluated.status, "invalid_claim");
    }

    #[test]
    fn p094_non_array_blockers_from_live_run_fail_closed() {
        let evaluated = evaluate_quality_gate_boundary_assessment(
            "assessment-live-regression",
            &serde_json::json!({
                "schema_version": "quality_gate_blocker_assessment_v1",
                "blockers": "DISPOSITION: BLOCKED; return to delegated implementation/code-fix work"
            }),
        )
        .unwrap();

        assert_eq!(evaluated.status, "invalid_claim");
        assert_eq!(
            evaluated.workflow_route_hint,
            "implementation_review_refresh"
        );
        assert_eq!(evaluated.payload["projection_integrity"], "tamper_detected");
        assert_eq!(evaluated.payload["local_work_complete"], false);
        assert!(evaluated.payload["validation_errors"]
            .as_array()
            .expect("validation errors should be present")
            .iter()
            .any(|error| error
                .as_str()
                .is_some_and(|error| error == "assessment.blockers must be an array")));
    }
}
