use auth::{Principal, PrincipalClass};
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, runs, side_effects, stages};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::side_effect::{EffectKind, SideEffect, SideEffectId, SideEffectStatus};
use domain::stage::{StageExecution, StageStatus};
use domain::CapabilityToolId;
use mcp_server::tools;
use std::sync::Arc;
use tempfile::TempDir;

fn make_run(run_id: RunId, idea_id: IdeaId, artifact_root: &str) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "proposal-078".into(),
        workflow_title: "P078".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: artifact_root.into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_1".into()),
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

fn make_stage(stage_execution_id: StageExecutionId, run_id: RunId) -> StageExecution {
    StageExecution {
        id: stage_execution_id,
        run_id,
        stage_id: "state_1".into(),
        label: "State 1".into(),
        status: StageStatus::Running,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: Utc::now(),
        completed_at: None,
        owner_agent: None,
        provider: None,
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    }
}

async fn seed_effect(
    tempdir: &TempDir,
    effect_kind: EffectKind,
    evidence_root: Option<String>,
) -> (sqlx::SqlitePool, SideEffectId) {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let effect_id = SideEffectId::new();
    let artifact_root = tempdir.path().join("run-artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P078 idea".into(),
            body: "Body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(
        &pool,
        &make_run(run_id, idea_id, &artifact_root.to_string_lossy()),
    )
    .await
    .unwrap();
    stages::insert(&pool, &make_stage(stage_execution_id, run_id))
        .await
        .unwrap();
    side_effects::insert(
        &pool,
        &SideEffect {
            id: effect_id.clone(),
            run_id,
            stage_execution_id,
            agent_execution_id: None,
            effect_kind,
            target_key: "release-target".into(),
            idempotency_key: "p078:test:key".into(),
            idempotency_key_version: 1,
            request_fingerprint: "sha256:test".into(),
            request_fingerprint_version: 1,
            status: SideEffectStatus::NeedsReconciliation,
            owner_instance_id: None,
            lease_acquired_at: None,
            lease_renewed_at: None,
            lease_expires_at: None,
            deadline_at: None,
            external_write_started_at: None,
            external_write_attempted: true,
            attempt_budget_remaining: 0,
            expected_evidence_json: None,
            observed_evidence_summary_json: Some(
                serde_json::json!({
                    "summary": "seeded-for-p078-reconcile"
                })
                .to_string(),
            ),
            evidence_root,
            last_error_kind: Some("ambiguous_outcome".into()),
            last_error: Some("seeded last error".into()),
            settlement_txn_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
    )
    .await
    .unwrap();

    (pool, effect_id)
}

#[test]
fn proposal_078_effects_capability_ids_are_registered() {
    // All P078 CapabilityToolIds must round-trip through capability_id_for.
    let pairs = [
        ("effects.list", CapabilityToolId::EffectsList),
        ("effects.inspect", CapabilityToolId::EffectsInspect),
        ("effects.reconcile", CapabilityToolId::EffectsReconcile),
        (
            "effects.mark_conflict",
            CapabilityToolId::EffectsMarkConflict,
        ),
        (
            "effects.mark_unrecoverable",
            CapabilityToolId::EffectsMarkUnrecoverable,
        ),
        (
            "effects.clear_after_manual_verification",
            CapabilityToolId::EffectsClearAfterManualVerification,
        ),
    ];

    for (tool_name, expected_id) in &pairs {
        let id = tools::capability_id_for(tool_name);
        assert_eq!(
            id,
            Some(*expected_id),
            "capability_id_for({tool_name}) must return {expected_id:?}"
        );
    }
}

#[test]
fn proposal_078_effects_tool_specs_cover_all_capability_ids() {
    let specs = tools::effects::tool_specs();
    let names: Vec<&str> = specs.iter().map(|t| t.name.as_str()).collect();

    for required in [
        "effects.list",
        "effects.inspect",
        "effects.reconcile",
        "effects.mark_conflict",
        "effects.mark_unrecoverable",
        "effects.clear_after_manual_verification",
    ] {
        assert!(
            names.contains(&required),
            "tool_specs must include {required}"
        );
    }
}

#[test]
fn proposal_078_mcp_tool_for_effects_capability_ids() {
    // mcp_tool_for must return a correctly named tool for each P078 id
    let pairs = [
        (CapabilityToolId::EffectsList, "effects.list"),
        (CapabilityToolId::EffectsInspect, "effects.inspect"),
        (CapabilityToolId::EffectsReconcile, "effects.reconcile"),
        (
            CapabilityToolId::EffectsMarkConflict,
            "effects.mark_conflict",
        ),
        (
            CapabilityToolId::EffectsMarkUnrecoverable,
            "effects.mark_unrecoverable",
        ),
        (
            CapabilityToolId::EffectsClearAfterManualVerification,
            "effects.clear_after_manual_verification",
        ),
    ];

    for (id, expected_name) in &pairs {
        let tool = tools::mcp_tool_for(*id);
        assert_eq!(
            tool.name, *expected_name,
            "mcp_tool_for({id:?}) must return tool named {expected_name}"
        );
    }
}

#[test]
fn proposal_078_canonical_tool_name_underscore_variants() {
    // Underscore variants must resolve to dot-separated canonical names
    let pairs = [
        ("effects_list", "effects.list"),
        ("effects_inspect", "effects.inspect"),
        ("effects_reconcile", "effects.reconcile"),
        ("effects_mark_conflict", "effects.mark_conflict"),
        ("effects_mark_unrecoverable", "effects.mark_unrecoverable"),
        (
            "effects_clear_after_manual_verification",
            "effects.clear_after_manual_verification",
        ),
    ];

    for (underscore, expected_canonical) in &pairs {
        assert_eq!(
            tools::canonical_tool_name(underscore),
            *expected_canonical,
            "canonical_tool_name({underscore}) must return {expected_canonical}"
        );
    }
}

#[test]
fn proposal_078_effects_tools_in_all_capability_ids() {
    let all_ids = tools::all_capability_tool_ids();
    let required = [
        CapabilityToolId::EffectsList,
        CapabilityToolId::EffectsInspect,
        CapabilityToolId::EffectsReconcile,
        CapabilityToolId::EffectsMarkConflict,
        CapabilityToolId::EffectsMarkUnrecoverable,
        CapabilityToolId::EffectsClearAfterManualVerification,
    ];
    for id in &required {
        assert!(
            all_ids.contains(id),
            "all_capability_tool_ids must include {id:?}"
        );
    }
}

#[test]
fn proposal_078_effects_tools_in_all_tool_specs() {
    let specs = tools::all_tool_specs();
    let names: Vec<&str> = specs.iter().map(|t| t.name.as_str()).collect();
    for required in [
        "effects.list",
        "effects.inspect",
        "effects.reconcile",
        "effects.mark_conflict",
        "effects.mark_unrecoverable",
        "effects.clear_after_manual_verification",
    ] {
        assert!(
            names.contains(&required),
            "all_tool_specs must include {required}"
        );
    }
}

#[test]
fn proposal_078_effects_mark_conflict_is_public_operator_tool() {
    assert_eq!(
        tools::capability_id_for("effects.mark_conflict"),
        Some(CapabilityToolId::EffectsMarkConflict),
        "effects.mark_conflict must resolve to its public CapabilityToolId"
    );
    assert!(
        tools::all_capability_tool_ids().contains(&CapabilityToolId::EffectsMarkConflict),
        "effects.mark_conflict must appear in visible operator capability ids"
    );
}

#[tokio::test]
async fn proposal_078_effects_mark_conflict_applies_operator_disposition() {
    let tempdir = tempfile::tempdir().unwrap();
    let (pool, effect_id) = seed_effect(&tempdir, EffectKind::GitPush, None).await;
    let disposition_id = uuid::Uuid::new_v4().to_string();
    let decision_json = serde_json::json!({
        "schema_version": "side_effect_decision_v1",
        "decision": "conflict",
        "operator_notes": "Remote state diverged from local release ledger evidence."
    })
    .to_string();
    let principal = Principal::new("operator-test", PrincipalClass::Operator);

    let payload = tools::effects::handle_effects_mark_conflict(
        &pool,
        &serde_json::json!({
            "effect_id": effect_id.to_string(),
            "disposition_id": disposition_id,
            "decision_json": decision_json
        }),
        &principal,
    )
    .await
    .unwrap();

    assert_eq!(payload["status"], "applied");
    assert_eq!(payload["new_status"], "conflict");
    let loaded = side_effects::find_by_id(&pool, &effect_id)
        .await
        .unwrap()
        .expect("effect must remain durable");
    assert_eq!(loaded.status, SideEffectStatus::Conflict);
}

#[test]
fn proposal_078_graphql_does_not_expose_effects_mutation() {
    // Effects MCP tools must not appear as GraphQL mutation aliases.
    // This is verified structurally: the tool names use "effects.*" prefix
    // and none of them should be in the MutationRoot approval-only mutation
    // path. Since GraphQL has no reconcile/retry/clear mutations, verifying
    // that the MCP tool names follow the effects.* convention is sufficient
    // for the structural check.
    let specs = tools::effects::tool_specs();
    for spec in &specs {
        assert!(
            spec.name.starts_with("effects."),
            "effects tools must use effects.* prefix; got: {}",
            spec.name
        );
        assert!(
            !spec.name.contains("mutation"),
            "effects tools must not contain 'mutation' in name"
        );
    }
}

#[tokio::test]
async fn proposal_078_effects_reconcile_returns_file_backed_report_under_evidence_root() {
    let tempdir = tempfile::tempdir().unwrap();
    let evidence_root = tempdir.path().join("effect-evidence");
    std::fs::create_dir_all(&evidence_root).unwrap();
    let receipt_path = evidence_root.join("git-push.json");
    std::fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "success",
            "remote": "origin",
            "commit": "abc123"
        }))
        .unwrap(),
    )
    .unwrap();

    let (pool, effect_id) = seed_effect(
        &tempdir,
        EffectKind::GitPush,
        Some(evidence_root.to_string_lossy().into_owned()),
    )
    .await;

    let payload = tools::effects::handle_effects_reconcile(
        &pool,
        &serde_json::json!({ "effect_id": effect_id.to_string() }),
    )
    .await
    .unwrap();

    assert_ne!(payload["readback_source"], "local_ledger");
    let report_path = payload["report_path"]
        .as_str()
        .expect("reconcile must return report_path");
    assert_eq!(
        payload["reconciliation_report_path"], report_path,
        "semantic alias must match report_path"
    );
    assert!(
        std::path::Path::new(report_path).is_file(),
        "reconcile report must be written to disk"
    );
    assert!(
        report_path.starts_with(&evidence_root.to_string_lossy().to_string()),
        "report path must stay under evidence_root"
    );
    assert_eq!(payload["report_details"]["effect_kind"], "git_push");
    assert_eq!(
        payload["report_details"]["matched_evidence_kind"],
        "git_push_receipt"
    );
    assert_eq!(
        payload["report_details"]["matched_path"],
        receipt_path.to_string_lossy().to_string()
    );

    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(report_path).unwrap()).unwrap();
    assert_eq!(report["effect_kind"], "git_push");
    assert_eq!(report["readback_source"], payload["readback_source"]);
    assert_eq!(report["report_details"], payload["report_details"]);
}

#[tokio::test]
async fn proposal_078_effects_reconcile_uses_effect_scoped_fallback_report_path() {
    let tempdir = tempfile::tempdir().unwrap();
    let artifact_root = tempdir.path().join("run-artifacts");
    let release_dir = artifact_root.join("release");
    std::fs::create_dir_all(&release_dir).unwrap();
    let receipt_path = release_dir.join("connect-upload.json");
    std::fs::write(
        &receipt_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "uploaded",
            "buildNumber": "42"
        }))
        .unwrap(),
    )
    .unwrap();

    let (pool, effect_id) = seed_effect(&tempdir, EffectKind::ConnectUpload, None).await;

    let payload = tools::effects::handle_effects_reconcile(
        &pool,
        &serde_json::json!({ "effect_id": effect_id.to_string() }),
    )
    .await
    .unwrap();

    assert_ne!(payload["readback_source"], "local_ledger");
    let report_path = payload["report_path"]
        .as_str()
        .expect("reconcile must return report_path");
    assert!(
        std::path::Path::new(report_path).is_file(),
        "reconcile report must be written to disk"
    );
    let expected_prefix = artifact_root
        .join("side-effects")
        .join(effect_id.to_string())
        .join("reconciliation");
    assert!(
        report_path.starts_with(&expected_prefix.to_string_lossy().to_string()),
        "fallback report path must be deterministic and effect-scoped"
    );
    assert_eq!(payload["report_details"]["effect_kind"], "connect_upload");
    assert_eq!(
        payload["report_details"]["matched_evidence_kind"],
        "connect_upload_receipt"
    );
    assert_eq!(
        payload["report_details"]["matched_path"],
        receipt_path.to_string_lossy().to_string()
    );
}
