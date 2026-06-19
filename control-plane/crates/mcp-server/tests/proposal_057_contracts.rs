use chrono::Utc;
use db::pool::create_pool;
use db::repos::{artifact_contracts, ideas, rollout_contract_checks, runs};
use domain::artifact_contracts::{ActiveArtifactGenerationInput, ArtifactContractOverrideInput};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::run::{Run, RunStatus};

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    pool
}

fn principal(class: auth::PrincipalClass) -> auth::Principal {
    auth::Principal::new("p", class)
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Ready,
        workflow_id: "wf".into(),
        workflow_title: "Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_10".into()),
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

async fn seed_p094_boundary_status(pool: &sqlx::SqlitePool, run_id: RunId) {
    let dir = std::env::temp_dir().join(format!("p094-mcp-{run_id}"));
    std::fs::create_dir_all(&dir).unwrap();
    let raw_path = dir.join("blocker-boundary-status.json");
    std::fs::write(
        &raw_path,
        serde_json::json!({
            "schema_version": "blocker_boundary_status_v1",
            "status": "awaiting_human_boundary_approval",
            "followup_proposal_required": false,
            "has_release_blocking_external_blockers": true,
            "has_no_release_blocking_external_blockers": false,
            "projection_integrity": "valid",
            "primary_owner_class": "external_evidence",
            "workflow_route_hint": "human_boundary_approval",
            "blocker_freshness": "fresh",
            "allowed_workflow_routes": ["state_9_blocker_boundary_approval"],
            "blockers": [{
                "id": "external-proof",
                "summary": "external proof required",
                "blocker_signature_id": "sig-external-proof",
                "evidence_fingerprint": "fingerprint-external-proof",
                "source_artifact_generation_id": "generation-external-proof",
                "observed_after_stage_execution_id": "stage-exec-1",
                "observed_after_agent_execution_id": "agent-exec-1",
                "owner_class": "external_environment",
                "class": "remote_host",
                "evidence_freshness": "fresh",
                "allowed_workflow_routes": ["state_9_blocker_boundary_approval"]
            }],
            "hard_blockers": [{
                "id": "external-proof",
                "blocker_signature_id": "sig-external-proof",
                "evidence_fingerprint": "fingerprint-external-proof"
            }]
        })
        .to_string(),
    )
    .unwrap();
    artifact_contracts::upsert_generation_and_rebuild(
        pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "blocker_boundary_status_v1".into(),
            canonical_path: "quality-gate/blocker-boundary-status.json".into(),
            raw_path: raw_path.to_string_lossy().into_owned(),
            raw_status: "unknown".into(),
            generation_id: format!("p094-boundary-{run_id}"),
            source_agent_execution_id: Some("system.quality_gate_boundary".into()),
            source_stage_execution_id: Some("state_9_quality_gate_boundary_evaluated".into()),
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let request_path = dir.join("blocker-boundary-approval-request.json");
    std::fs::write(
        &request_path,
        serde_json::json!({
            "schema_version": "blocker_boundary_approval_request_v1",
            "status": "requested",
            "question": "Accept the server-evaluated boundary?",
            "allowed_decisions": ["accept", "reject"],
            "label_to_approval_state": {
                "accept": "granted",
                "reject": "rejected"
            }
        })
        .to_string(),
    )
    .unwrap();
    artifact_contracts::upsert_generation_and_rebuild(
        pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "blocker_boundary_approval_request_v1".into(),
            canonical_path: "quality-gate/blocker-boundary-approval-request.json".into(),
            raw_path: request_path.to_string_lossy().into_owned(),
            raw_status: "requested".into(),
            generation_id: format!("p094-approval-request-{run_id}"),
            source_agent_execution_id: Some("system.quality_gate_boundary".into()),
            source_stage_execution_id: Some("state_9_quality_gate_boundary_evaluated".into()),
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
}

async fn seed_p094_release_rollout_contract(pool: &sqlx::SqlitePool, run_id: RunId) {
    use rollout_contract_checks::{
        ProjectionIntegrity, RolloutContractDecision, RolloutContractEnforcementMode,
        RolloutContractLifecycleState, RolloutContractStatus, UpsertRolloutContractCheck,
    };

    rollout_contract_checks::upsert_rollout_contract_check(
        pool,
        &UpsertRolloutContractCheck {
            id: uuid::Uuid::new_v4(),
            run_id: run_id.inner(),
            proposal_id: "P094".to_string(),
            proposal_revision_id: "p094-r1".to_string(),
            proposal_content_hash: "sha256:p094-proposal".to_string(),
            contract_object_hash: "sha256:p094-contract".to_string(),
            content_snapshot_id: "artifact-p094".to_string(),
            checker_version: "p094-test-checker".to_string(),
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
                "steps": ["Return P094 enforcement to dry-run through an audited rollout contract mutation."]
            }),
            projection_integrity: ProjectionIntegrity::Valid,
            cutover_policy_revision: Some("cutover-p094-test".to_string()),
            redaction_state: "partial".to_string(),
            retry_count: 0,
            preflight_timeout_seconds: 45,
        },
        Utc::now(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn proposal_057_reports_get_exposes_canonical_statuses_and_overrides() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "Idea".into(),
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
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS_WITH_NOTES".into(),
            generation_id: "gen-1".into(),
            source_agent_execution_id: None,
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    artifact_contracts::create_override_and_rebuild(
        &pool,
        ArtifactContractOverrideInput {
            run_id,
            contract_id: "audit_report_v1".into(),
            override_type: "implementation_status".into(),
            from_status: "needs_code_fixes".into(),
            to_status: "implemented".into(),
            reason: "operator verified".into(),
            owner: "operator".into(),
            source_artifacts: vec![],
            expires_at_stage: "state_11_manual_release".into(),
            journal_id: "journal-1".into(),
        },
    )
    .await
    .unwrap();

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &engine::command_handler::CommandHandler::new(
            pool.clone(),
            engine::event_bus::new_bus(16),
            engine::work_queue::WorkQueue::new(pool.clone()),
        ),
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = payload["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["report_kind"] == "canonical_artifact_contracts")
        .unwrap();
    assert_eq!(
        canonical["active_index"]["contracts"]["prepush_review_v1"]["status"],
        "pass"
    );
    assert_eq!(
        canonical["operator_overrides"][0]["to_status"],
        "implemented"
    );
}

#[tokio::test]
async fn proposal_057_runs_get_exposes_canonical_artifact_contract_parity() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "Idea".into(),
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
    let boundary_root = std::env::temp_dir().join(format!("p094-mcp-{run_id}"));
    let mut run = make_run(run_id, idea_id);
    run.chainworks_meta_root = Some(boundary_root.to_string_lossy().into_owned());
    run.artifact_root = boundary_root.to_string_lossy().into_owned();
    runs::insert(&pool, &run).await.unwrap();
    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS_WITH_NOTES".into(),
            generation_id: "gen-runs-get".into(),
            source_agent_execution_id: None,
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromFailedExecution,
            partial: true,
            warnings: vec!["accepted by degraded-output policy".into()],
        },
    )
    .await
    .unwrap();
    artifact_contracts::create_override_and_rebuild(
        &pool,
        ArtifactContractOverrideInput {
            run_id,
            contract_id: "audit_report_v1".into(),
            override_type: "implementation_status".into(),
            from_status: "needs_code_fixes".into(),
            to_status: "implemented".into(),
            reason: "operator verified".into(),
            owner: "operator".into(),
            source_artifacts: vec![],
            expires_at_stage: "state_11_manual_release".into(),
            journal_id: "journal-runs-get".into(),
        },
    )
    .await
    .unwrap();

    let payload = mcp_server::tools::runs::execute(
        "runs.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &engine::command_handler::CommandHandler::new(
            pool.clone(),
            engine::event_bus::new_bus(16),
            engine::work_queue::WorkQueue::new(pool.clone()),
        ),
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    assert_eq!(
        payload["active_artifact_index"]["contracts"]["prepush_review_v1"]["status"],
        "pass"
    );
    assert_eq!(
        payload["active_artifact_index"]["contracts"]["prepush_review_v1"]["output_settlement"],
        "valid_outputs_from_failed_execution"
    );
    assert_eq!(
        payload["run_state_projection"]["active_index_owner"],
        "sqlite"
    );
    assert_eq!(payload["operator_overrides"][0]["to_status"], "implemented");
}

#[tokio::test]
async fn proposal_094_mcp_surfaces_expose_boundary_readback_to_operator() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "Idea".into(),
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
    let boundary_root = std::env::temp_dir().join(format!("p094-mcp-{run_id}"));
    let mut run = make_run(run_id, idea_id);
    run.chainworks_meta_root = Some(boundary_root.to_string_lossy().into_owned());
    run.artifact_root = boundary_root.to_string_lossy().into_owned();
    runs::insert(&pool, &run).await.unwrap();
    seed_p094_boundary_status(&pool, run_id).await;
    seed_p094_release_rollout_contract(&pool, run_id).await;

    let handler = engine::command_handler::CommandHandler::new(
        pool.clone(),
        engine::event_bus::new_bus(16),
        engine::work_queue::WorkQueue::new(pool.clone()),
    );
    let operator = principal(auth::PrincipalClass::Operator);
    let runs_get = mcp_server::tools::runs::execute(
        "runs.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &handler,
        &operator,
    )
    .await
    .unwrap();
    assert_eq!(
        runs_get["p094_boundary_readback"]["blocker_boundary_status"]["status"],
        "awaiting_human_boundary_approval"
    );
    assert_eq!(
        runs_get["p094_boundary_readback"]["blocker_boundary_status"]["blockers"][0]
            ["blocker_signature_id"],
        "sig-external-proof"
    );
    assert_eq!(
        runs_get["p094_boundary_readback"]["blocker_boundary_approval_request"]["status"],
        "requested"
    );

    let reports_get = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &handler,
        &operator,
    )
    .await
    .unwrap();
    let execution_truth = reports_get["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|report| report["report_kind"] == "mcp_execution_truth")
        .expect("reports.get should include mcp_execution_truth lane");
    assert_eq!(
        execution_truth["p094_boundary_readback"]["blocker_boundary_status"]
            ["allowed_workflow_routes"],
        serde_json::json!(["state_9_blocker_boundary_approval"])
    );
    assert_eq!(
        execution_truth["p094_rollout_decision"]["schemaVersion"],
        "p094_rollout_decision_readback_v1"
    );
    assert_eq!(
        execution_truth["p094_rollout_decision"]["ownerDecision"]["state"],
        "release"
    );
    assert_eq!(
        execution_truth["p094_rollout_decision"]["ownerDecision"]["source"],
        "rollout_contract_checks"
    );
    assert_eq!(
        execution_truth["p094_rollout_decision"]["promotionReadiness"]["enforcingAllowed"],
        true
    );
    assert!(
        execution_truth["p094_rollout_decision"]["ownerDecision"]["rolloutContractEntryId"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(
        execution_truth["p094_rollout_decision"]["metricValues"]["false_external_blocker_rate"]
            ["kind"],
        "gauge"
    );
    assert_eq!(
        execution_truth["p094_rollout_decision"]["rolloutContractReadback"]["proposal_id"],
        "P094"
    );
    let canonical = reports_get["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|report| report["report_kind"] == "canonical_artifact_contracts")
        .expect("reports.get should include canonical artifact contracts lane");
    assert_eq!(
        canonical["p094_boundary_readback"]["blocker_boundary_status"]["workflow_route_hint"],
        "human_boundary_approval"
    );
}

#[tokio::test]
async fn proposal_094_runtime_health_exposes_quality_gate_boundary_mode() {
    let pool = test_pool().await;
    let payload = mcp_server::tools::runtime::execute(serde_json::json!({}), &pool, None)
        .await
        .unwrap();

    assert_eq!(payload["schemaVersion"], "runtime_health.v1");
    assert_eq!(
        payload["qualityGateBoundary"]["schemaVersion"],
        "quality_gate_boundary_runtime_v1"
    );
    assert_eq!(payload["qualityGateBoundary"]["proposalId"], "P094");
    assert_eq!(payload["qualityGateBoundary"]["mode"], "approval_dry_run");
    assert_eq!(payload["qualityGateBoundary"]["status"], "available");
    assert!(
        payload["qualityGateBoundary"]["allowedModes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|mode| mode == "held"),
        "runtime.health must expose the P094 mode vocabulary for hold/rollback decisions"
    );
    assert_eq!(
        payload["qualityGateBoundary"]["rolloutDecision"]["schemaVersion"],
        "p094_rollout_decision_readback_v1"
    );
    assert_eq!(
        payload["qualityGateBoundary"]["rolloutDecision"]["auditability"]
            ["enforcementModeChangesRequire"],
        "command_journal_or_rollout_contract_entry"
    );
    assert_eq!(
        payload["qualityGateBoundary"]["rolloutDecision"]["promotionReadiness"]["enforcingAllowed"],
        false
    );
    assert_eq!(
        payload["qualityGateBoundary"]["rolloutDecision"]["metricValues"]
            ["accepted_boundary_later_rejected_percent"]["unit"],
        "percent"
    );
}

#[tokio::test]
async fn proposal_057_override_tool_is_operator_only() {
    assert_eq!(
        mcp_server::tools::capability_id_for("artifacts.override_contract"),
        Some(domain::CapabilityToolId::ArtifactsOverrideContract)
    );
    assert!(
        auth::filter_tools(
            &principal(auth::PrincipalClass::Operator),
            &[domain::CapabilityToolId::ArtifactsOverrideContract]
        )
        .len()
            == 1
    );
    assert!(auth::filter_tools(
        &principal(auth::PrincipalClass::Observer),
        &[domain::CapabilityToolId::ArtifactsOverrideContract]
    )
    .is_empty());

    let pool = test_pool().await;
    let result = mcp_server::tools::artifacts::execute(
        "artifacts.override_contract",
        serde_json::json!({
            "run_id": RunId::new().to_string(),
            "contract_id": "audit_report_v1",
            "override_type": "implementation_status",
            "from_status": "needs_code_fixes",
            "to_status": "implemented",
            "reason": "operator verified",
            "source_artifacts": [],
            "expires_at_stage": "state_11_manual_release",
        }),
        &pool,
        &engine::command_handler::CommandHandler::new(
            pool.clone(),
            engine::event_bus::new_bus(16),
            engine::work_queue::WorkQueue::new(pool.clone()),
        ),
        &principal(auth::PrincipalClass::Observer),
    )
    .await;
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("requires operator principal"),
        "public artifacts.override_contract tool path must fail closed for non-operators"
    );
}

/// R12 API-002 / §9.3 AC-15: `artifacts.override_contract` must run
/// through `mcp_caller` so the ambient `MCP_REQUEST_ID` lands in the
/// command-journal row. Before this fix the tool built
/// `CallerContext::mcp(...)` directly and silently dropped the id.
#[tokio::test]
async fn artifacts_override_contract_attaches_ambient_mcp_request_id_to_journal() {
    use mcp_server::request_context::scope_request_id;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "Idea".into(),
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
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "audit_report_v1".into(),
            canonical_path: "review/audit.json".into(),
            raw_path: "review/audit.json".into(),
            raw_status: "needs_code_fixes".into(),
            generation_id: "gen-1".into(),
            source_agent_execution_id: None,
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();

    let handler = engine::command_handler::CommandHandler::new(
        pool.clone(),
        engine::event_bus::new_bus(16),
        engine::work_queue::WorkQueue::new(pool.clone()),
    );

    // Scope the MCP task-local the same way `handle_mcp_post` does
    // when an inbound HTTP request carries `X-Request-ID: rid-42`.
    let payload = scope_request_id(Some("rid-42".into()), async {
        mcp_server::tools::artifacts::execute(
            "artifacts.override_contract",
            serde_json::json!({
                "run_id": run_id.to_string(),
                "contract_id": "audit_report_v1",
                "override_type": "implementation_status",
                "from_status": "needs_code_fixes",
                "to_status": "implemented",
                "reason": "operator verified end-to-end",
                "source_artifacts": [],
                "expires_at_stage": "state_11_manual_release",
            }),
            &pool,
            &handler,
            &principal(auth::PrincipalClass::Operator),
        )
        .await
        .unwrap()
    })
    .await;

    let journal_id = payload["journal_id"]
        .as_str()
        .expect("journal_id on successful override response");
    let recorded = db::repos::command_journal::find_request_id(&pool, journal_id)
        .await
        .unwrap();
    assert_eq!(
        recorded.as_deref(),
        Some("rid-42"),
        "artifacts.override_contract must thread the ambient MCP request id \
         into command_journal.request_id via mcp_caller"
    );
}
