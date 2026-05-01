use domain::run::Run;
use domain::steward::CohortQuality;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CohortKey {
    pub workflow_family: String,
    pub risk_class: String,
}

pub fn is_p049_eligible(run: &Run) -> bool {
    run.workflow_family
        .as_deref()
        .is_some_and(|v| !v.trim().is_empty())
        && run
            .risk_class
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
        && run
            .workflow_snapshot_hash
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
        && run
            .catalog_snapshot_hash
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
        && run
            .workflow_snapshot_json
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
        && run
            .catalog_snapshot_json
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty())
}

pub fn primary_cohort_key(runs: &[Run]) -> Option<CohortKey> {
    let mut cohorts: BTreeMap<(String, String), (usize, chrono::DateTime<chrono::Utc>)> =
        BTreeMap::new();
    for run in runs.iter().filter(|run| is_p049_eligible(run)) {
        let key = (
            run.workflow_family
                .clone()
                .expect("eligible workflow family"),
            run.risk_class.clone().expect("eligible risk class"),
        );
        let recency = run.completed_at.unwrap_or(run.started_at);
        let entry = cohorts.entry(key).or_insert((0, recency));
        entry.0 += 1;
        entry.1 = entry.1.max(recency);
    }

    cohorts
        .into_iter()
        .max_by(|(left_key, left), (right_key, right)| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| right_key.cmp(left_key))
        })
        .map(|((workflow_family, risk_class), _)| CohortKey {
            workflow_family,
            risk_class,
        })
}

pub fn runs_for_cohort(runs: &[Run], key: &CohortKey) -> Vec<Run> {
    runs.iter()
        .filter(|run| {
            run.workflow_family.as_deref() == Some(key.workflow_family.as_str())
                && run.risk_class.as_deref() == Some(key.risk_class.as_str())
                && is_p049_eligible(run)
        })
        .cloned()
        .collect()
}

pub fn classify_quality(runs: &[Run]) -> CohortQuality {
    let has_untagged = runs.iter().any(|run| {
        run.project_key
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty() || value == "untagged")
    });
    if runs.len() < 5 || has_untagged {
        return CohortQuality::Weak;
    }

    let has_unknown_stack = runs.iter().any(|run| {
        run.stack
            .as_deref()
            .map(str::trim)
            .is_none_or(|value| value.is_empty() || value == "unknown")
    });
    if runs.len() >= 10 && !has_unknown_stack {
        CohortQuality::Strong
    } else {
        CohortQuality::Acceptable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};

    fn run(project_key: &str, stack: &str) -> Run {
        Run {
            id: RunId::new(),
            idea_id: IdeaId::new(),
            status: RunStatus::Completed,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
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
            delivery_configuration_json: None,
            delivery_preflight_json: None,
            workflow_family: Some("mvp".into()),
            project_key: Some(project_key.into()),
            risk_class: Some("standard".into()),
            stack: Some(stack.into()),
            workflow_snapshot_hash: Some("a".repeat(64)),
            catalog_snapshot_hash: Some("b".repeat(64)),
            workflow_snapshot_json: Some("{}".into()),
            catalog_snapshot_json: Some("{}".into()),
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        }
    }

    #[test]
    fn steward_cohort_classifier_tests_quality_rules_match_p049() {
        assert_eq!(
            classify_quality(&vec![run("project", "swiftui"); 4]),
            CohortQuality::Weak
        );
        assert_eq!(
            classify_quality(&vec![run("untagged", "swiftui"); 6]),
            CohortQuality::Weak
        );
        assert_eq!(
            classify_quality(&vec![run("project", "unknown"); 6]),
            CohortQuality::Acceptable
        );
        assert_eq!(
            classify_quality(&vec![run("project", "swiftui"); 10]),
            CohortQuality::Strong
        );
    }

    #[test]
    fn steward_cohort_classifier_tests_primary_key_uses_largest_explicit_cohort() {
        let mut first_recent = run("project", "swiftui");
        first_recent.workflow_family = Some("small".into());
        first_recent.risk_class = Some("high".into());

        let mut large_a = run("project", "swiftui");
        large_a.workflow_family = Some("large".into());
        large_a.risk_class = Some("standard".into());

        let mut large_b = large_a.clone();
        large_b.id = RunId::new();

        let key = primary_cohort_key(&[first_recent, large_a, large_b])
            .expect("eligible cohort should be selected");

        assert_eq!(
            key,
            CohortKey {
                workflow_family: "large".into(),
                risk_class: "standard".into()
            }
        );
    }
}
