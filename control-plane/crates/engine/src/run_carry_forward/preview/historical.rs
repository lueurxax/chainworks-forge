use super::*;

pub(super) fn backlog(bytes: &[u8]) -> anyhow::Result<()> {
    // A versioned reference reader, not a source contract or executable backlog.
    let value: Value = serde_yaml::from_slice(bytes)
        .context("artifact_provenance_invalid: legacy backlog YAML")?;
    ensure!(
        value["schema_version"] == "implementation_backlog_v1",
        "artifact_provenance_invalid: legacy backlog version"
    );
    strings(&value, &["proposal_revision_id", "status"])?;
    let items = value["backlog_items"]
        .as_array()
        .context("artifact_provenance_invalid: legacy backlog items")?;
    let mut ids = BTreeSet::new();
    for item in items {
        strings(
            item,
            &["id", "title", "owner_class", "description", "state"],
        )?;
        ensure!(
            ids.insert(item["id"].as_str().unwrap()),
            "artifact_provenance_invalid: duplicate legacy backlog item"
        );
    }
    Ok(())
}

fn strings(value: &Value, fields: &[&str]) -> anyhow::Result<()> {
    ensure!(
        fields
            .iter()
            .all(|field| value[*field].as_str().is_some_and(|s| !s.trim().is_empty())),
        "artifact_provenance_invalid: historical string fields"
    );
    Ok(())
}

pub(super) fn boundary(contract: &str, value: &Value) -> anyhow::Result<()> {
    match contract {
        "blocker_boundary_human_decision_v1" => strings(value, &["decision_label"]),
        "proposal_decomposition_plan_v1" => {
            strings(value, &["implementation_start_decision"])?;
            ensure!(
                value["requires_split"].is_boolean(),
                "artifact_provenance_invalid: decomposition boolean"
            );
            Ok(())
        }
        "followup_proposal_seed_v1" => strings(value, &["title", "problem", "proposed_scope"]),
        "quality_gate_blocker_assessment_v1" => {
            let evaluation =
                crate::quality_gate_boundary::evaluate_quality_gate_boundary_assessment(
                    "historical-reference",
                    value,
                )?;
            ensure!(
                evaluation.payload["projection_integrity"] == "valid",
                "artifact_provenance_invalid: historical blocker assessment"
            );
            Ok(())
        }
        "blocker_boundary_status_v1" => {
            strings(
                value,
                &[
                    "status",
                    "assessment_generation_id",
                    "projection_integrity",
                    "workflow_route_hint",
                ],
            )?;
            ensure!(
                [
                    "followup_proposal_required",
                    "has_release_blocking_external_blockers",
                    "has_no_release_blocking_external_blockers"
                ]
                .iter()
                .all(|field| value[*field].is_boolean()),
                "artifact_provenance_invalid: historical boundary booleans"
            );
            Ok(())
        }
        "blocker_boundary_approval_request_v1" => {
            strings(value, &["question"])?;
            let decisions = value["allowed_decisions"]
                .as_array()
                .context("artifact_provenance_invalid: historical decisions")?;
            let mapping = value["label_to_approval_state"]
                .as_object()
                .context("artifact_provenance_invalid: historical decision mapping")?;
            ensure!(
                !decisions.is_empty()
                    && decisions.iter().all(|decision| decision
                        .as_str()
                        .is_some_and(|label| !label.is_empty()
                            && mapping.get(label).is_some_and(Value::is_string))),
                "artifact_provenance_invalid: historical decision labels"
            );
            Ok(())
        }
        _ => Ok(()),
    }
}
