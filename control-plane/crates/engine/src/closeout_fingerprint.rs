use chrono::Utc;
use domain::closeout_readiness::CloseoutFingerprint;
use domain::run::Run;

pub fn build_closeout_fingerprint(
    run: &Run,
    stage_id: &str,
    worktree_head: impl Into<String>,
    dirty_or_changed_file_digest: impl Into<String>,
    upstream_active_generation_ids: Vec<String>,
    latency_ms: u64,
) -> CloseoutFingerprint {
    let workflow_digest = run
        .workflow_snapshot_hash
        .clone()
        .or_else(|| {
            run.workflow_id
                .strip_prefix("sha256:")
                .map(|_| run.workflow_id.clone())
        })
        .unwrap_or_else(|| "sha256:unknown-workflow".into());
    let proposal_or_freeze_digest = run
        .workflow_snapshot_hash
        .clone()
        .or_else(|| run.catalog_snapshot_hash.clone())
        .or_else(|| run.base_revision.clone())
        .unwrap_or_else(|| "sha256:unknown-proposal".into());

    CloseoutFingerprint {
        proposal_or_freeze_digest,
        run_id: run.id.to_string(),
        stage_id: stage_id.to_string(),
        workflow_digest,
        worktree_head: worktree_head.into(),
        dirty_or_changed_file_digest: dirty_or_changed_file_digest.into(),
        upstream_active_generation_ids,
        contract_version: "implementation_closeout_readiness_v1".into(),
        computed_at: Utc::now(),
        latency_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};

    fn run() -> Run {
        Run {
            id: RunId::new(),
            idea_id: IdeaId::new(),
            status: RunStatus::Running,
            workflow_id: "workflow".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/repo".into(),
            artifact_root: ".chainworks/run".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_9_implementation_reviewed".into()),
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: Some("/tmp/repo/.chainworks/worktrees/run".into()),
            base_branch: Some("main".into()),
            base_revision: Some("sha256:base".into()),
            target_branch: Some("cw/p077".into()),
            delivery_configuration_json: None,
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: Some("sha256:workflow".into()),
            catalog_snapshot_hash: Some("sha256:catalog".into()),
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: Some("enforcement".into()),
        }
    }

    #[test]
    fn closeout_fingerprint_uses_run_and_generation_truth() {
        let run = run();
        let fingerprint = build_closeout_fingerprint(
            &run,
            "state_9_implementation_reviewed",
            "abcdef",
            "sha256:dirty",
            vec!["gen-a".into(), "gen-b".into()],
            12,
        );

        assert_eq!(fingerprint.run_id, run.id.to_string());
        assert_eq!(fingerprint.stage_id, "state_9_implementation_reviewed");
        assert_eq!(fingerprint.workflow_digest, "sha256:workflow");
        assert_eq!(fingerprint.proposal_or_freeze_digest, "sha256:workflow");
        assert_eq!(fingerprint.worktree_head, "abcdef");
        assert_eq!(fingerprint.dirty_or_changed_file_digest, "sha256:dirty");
        assert_eq!(
            fingerprint.upstream_active_generation_ids,
            vec!["gen-a", "gen-b"]
        );
        assert_eq!(fingerprint.latency_ms, 12);
        assert_eq!(fingerprint.short_hash().len(), 8);
    }
}
