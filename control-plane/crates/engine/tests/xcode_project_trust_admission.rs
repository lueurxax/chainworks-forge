use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use acp::{
    execution_root::resolve_execution_root,
    xcode_coordinator::{CoordinatorLimits, FixtureJournalAuthority, WorkspaceAccessCoordinator},
    xcode_headless::{HeadlessPeer, HeadlessPeerFactory, HeadlessWorkspaceController},
    xcode_headless_host::{HeadlessHostInspector, HostSnapshot, ServiceGeneration, TrustedProject},
    xcode_headless_runtime::{
        HeadlessPreparationInput, HeadlessProjectTrust, HeadlessRuntime,
        ProjectTrustAdmissionFailure,
    },
    xcode_project_trust::ProjectTrustStore,
    AcpRuntimeManager,
};
use anyhow::Result;
use chrono::Utc;
use db::repos::{agent_execution_runtime_facts, agent_executions, ideas, runs, stages, work_items};
use db::work_item::{WorkItemKind, WorkItemStatus};
use db::writer::DbWriter;
use domain::{
    agent::{AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement, AgentStatus},
    commands::{CallerContext, Command, RetryStageCmd},
    idea::{Idea, IdeaStatus},
    ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId},
    run::{Run, RunStatus},
    stage::{StageExecution, StageStatus},
};
use engine::{
    command_handler::CommandHandler,
    event_bus,
    executor::BackgroundExecutor,
    orchestrator::Orchestrator,
    work_queue::WorkQueue,
    xcode_effect_journal::{DbXcodeEffectJournal, DbXcodeProjectHoldCheck},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

// This catches admission failures that escape before durable classification and
// stage advancement, or that incorrectly reach provider/Xcode launch boundaries.
#[tokio::test]
async fn missing_project_trust_blocks_before_launch_and_records_exact_operator_recovery() {
    assert_trust_admission(false).await;
}

#[tokio::test]
async fn cancellation_during_project_trust_check_retains_terminal_status_and_diagnostics() {
    assert_trust_admission(true).await;
}

async fn assert_trust_admission(cancel_during_check: bool) {
    let dir = tempfile::tempdir().unwrap();
    let repository = dir.path().join("repository");
    let worktree = dir.path().join("worktree");
    std::fs::create_dir(&repository).unwrap();
    let package = worktree.join("Fixture.xcodeproj");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::write(package.join("project.pbxproj"), "// fixture").unwrap();
    let root = resolve_execution_root(
        repository.to_str().unwrap(),
        Some(worktree.to_str().unwrap()),
        true,
        Some("dedicated"),
    )
    .unwrap();
    let project = TrustedProject::resolve(root.clone(), None, acp::current_process_uid()).unwrap();
    let trust_directory = dir.path().canonicalize().unwrap().join("project-trust");
    let trust = Arc::new(ProjectTrustStore::new(trust_directory.clone()));
    let database = dir.path().join("db.sqlite");
    let pool = db::pool::create_pool(&format!("sqlite://{}?mode=rwc", database.display()))
        .await
        .unwrap();
    let writer = Arc::new(DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer.clone())
        .await
        .unwrap();

    let host = Arc::new(FixtureHost::new());
    let controller = Arc::new(HeadlessWorkspaceController::new(
        host.clone(),
        Arc::new(FixturePeers(host.clone())),
    ));
    let coordinator = WorkspaceAccessCoordinator::new(
        CoordinatorLimits {
            queue_capacity: 8,
            queue_timeout: Duration::from_secs(1),
        },
        Arc::new(DbXcodeProjectHoldCheck(pool.clone())),
    )
    .unwrap();
    let authority = FixtureJournalAuthority::open(
        &dir.path().canonicalize().unwrap().join("authority"),
        &database,
    )
    .unwrap();
    let runtime_trust: Arc<dyn HeadlessProjectTrust> = if cancel_during_check {
        Arc::new(CancelDuringTrustCheck {
            pool: pool.clone(),
            store: trust.clone(),
        })
    } else {
        trust.clone()
    };
    let runtime =
        HeadlessRuntime::new_fixture(controller.clone(), coordinator, runtime_trust, authority);
    let provider_prepare_calls = Arc::new(AtomicUsize::new(0));
    let prepared_attempts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![Arc::new(
        LaunchProbe {
            calls: provider_prepare_calls.clone(),
            attempts: prepared_attempts.clone(),
        },
    )]));
    acp.set_headless_xcode_runtime(runtime.clone());

    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    seed_run(&pool, run_id, stage_id, &root).await;
    let events = event_bus::new_bus(64);
    let mut observed_events = events.subscribe();
    let queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        queue.clone(),
        orchestrator,
        acp,
        events.clone(),
    );
    queue.enqueue(WorkItemKind::InvokeAgent, Some(run_id), Some("review".into()), json!({
        "run_id": run_id, "stage_id": "review", "stage_execution_id": stage_id,
        "agent_id": "reviewer", "provider": "claude", "model": "sonnet",
        "permission_profile": "VERIFY", "prompt": "Read the fixture Xcode project",
        "requires_xcode_host_execution": true,
        "worktree_write_enabled": true, "worktree_strategy": "dedicated",
        "session_reuse_scope": "same_agent_family_within_run", "session_family_id": "reviewer"
    })).await.unwrap();
    make_pending_items_due(&pool, run_id).await;

    let error = executor
        .process_next_item()
        .await
        .expect_err("untrusted project must reject invocation");
    assert!(
        format!("{error:#}").contains("project_trust_required"),
        "{error:#}"
    );
    assert!(
        !trust_directory.exists(),
        "read-only admission must not create a trust store"
    );
    assert_eq!(provider_prepare_calls.load(Ordering::SeqCst), 0);
    assert_eq!(host.opens.load(Ordering::SeqCst), 0);
    assert_eq!(host.inspections.load(Ordering::SeqCst), 0);
    for (table, query) in [
        (
            "provider_sessions",
            "SELECT count(*) FROM provider_sessions",
        ),
        (
            "session_generations",
            "SELECT count(*) FROM session_generations",
        ),
        ("session_events", "SELECT count(*) FROM session_events"),
        (
            "xcode_effect_attempts",
            "SELECT count(*) FROM xcode_effect_attempts",
        ),
        (
            "xcode_project_holds",
            "SELECT count(*) FROM xcode_project_holds",
        ),
    ] {
        let count: i64 = sqlx::query_scalar(query).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 0, "admission denial must not create {table}");
    }
    while let Ok(event) = observed_events.try_recv() {
        if let domain::events::DomainEvent::RuntimeStatusChanged { event_kind, .. } = event {
            assert_ne!(
                event_kind, "session_started",
                "no provider session event is allowed"
            );
        }
    }

    let executions = agent_executions::find_by_stage(&pool, stage_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 1);
    let execution = &executions[0];
    assert_eq!(
        execution.status,
        if cancel_during_check {
            AgentStatus::Cancelled
        } else {
            AgentStatus::Failed
        }
    );
    assert!(execution.completed_at.is_some());
    let facts = agent_execution_runtime_facts::find_by_execution_id(&pool, execution.id)
        .await
        .unwrap()
        .unwrap();
    if cancel_during_check {
        assert_eq!(
            facts.supervision_classification.as_deref(),
            Some("fixture_operator_cancelled")
        );
        assert_eq!(
            facts.failure_message_redacted.as_deref(),
            Some("Cancelled while trust admission was pending")
        );
        assert_eq!(facts.runtime_preflight_phase, None);
        assert_eq!(facts.failure_kind, None);
        let items = work_items::list_by_run(&pool, run_id).await.unwrap();
        assert_eq!(
            items.len(),
            1,
            "stale trust completion must not enqueue advance or retry work"
        );
        assert_eq!(items[0].kind, WorkItemKind::InvokeAgent);
        assert_ne!(items[0].status, WorkItemStatus::Pending);
        assert_eq!(items[0].attempt_count, 1);
        assert_eq!(provider_prepare_calls.load(Ordering::SeqCst), 0);
        drop((executor, runtime));
        controller.shutdown().await;
        writer.shutdown().await;
        pool.close().await;
        return;
    }
    assert_eq!(
        facts.failure_kind,
        Some(AgentFailureKind::XcodeHostEnvironmentError)
    );
    assert_eq!(
        facts.supervision_classification.as_deref(),
        Some("xcode_headless_prelaunch_failed")
    );
    assert_eq!(
        facts.runtime_preflight_phase.as_deref(),
        Some("failed_no_launch")
    );
    assert_eq!(facts.runtime_preflight_provider_launched, Some(false));
    assert_eq!(facts.retry_after, None);
    assert_eq!(facts.output_settlement, AgentOutputSettlement::None);
    let operator_message = facts
        .failure_message_redacted
        .as_deref()
        .expect("actionable failure message")
        .to_ascii_lowercase();
    assert!(operator_message.contains("trust"), "{operator_message}");
    assert!(operator_message.contains("retry"), "{operator_message}");
    let preflight: Value = serde_json::from_str(
        facts
            .runtime_preflight_json
            .as_deref()
            .expect("structured admission diagnostics"),
    )
    .unwrap();
    assert_eq!(preflight["schema_version"], 1);
    assert_eq!(preflight["boundary"], "xcode_project_trust");
    assert_eq!(preflight["error_code"], "project_trust_required");
    assert_eq!(preflight["root"], serde_json::to_value(&root).unwrap());
    assert_eq!(
        preflight["project_key"],
        serde_json::to_value(project.key()).unwrap()
    );
    assert!(preflight["project_selector"].is_null());
    assert_eq!(preflight["provider_launched"], false);
    assert_eq!(preflight["automatic_retry_allowed"], false);
    assert_eq!(
        preflight["operator_disposition"],
        "review_exact_project_trust"
    );
    assert_eq!(
        preflight["recovery"],
        "explicit_trust_decision_then_stage_retry"
    );
    assert!(preflight["operator_message"]
        .as_str()
        .is_some_and(|message| !message.is_empty()));

    let items = work_items::list_by_run(&pool, run_id).await.unwrap();
    assert!(items.iter().any(
        |item| item.kind == WorkItemKind::InvokeAgent && item.status == WorkItemStatus::Failed
    ));
    assert!(items.iter().any(
        |item| item.kind == WorkItemKind::AdvanceRun && item.status == WorkItemStatus::Pending
    ));
    make_pending_items_due(&pool, run_id).await;
    assert!(
        executor.process_next_item().await.unwrap(),
        "normal AdvanceRun must consume failure"
    );
    let failed_stage = stages::find_by_id(&pool, stage_id).await.unwrap().unwrap();
    assert_eq!(failed_stage.status, StageStatus::Failed);
    let recovery: Value = serde_json::from_str(
        failed_stage
            .recovery_snapshot_json
            .as_deref()
            .expect("operator recovery snapshot"),
    )
    .unwrap();
    assert_eq!(recovery["action"], "review_exact_project_trust_then_retry");
    assert_eq!(recovery["reason"], "xcode_project_trust_admission");
    assert_eq!(
        runs::find_by_id(&pool, run_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        RunStatus::Blocked
    );
    let items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let invokes: Vec<_> = items
        .iter()
        .filter(|item| item.kind == WorkItemKind::InvokeAgent)
        .collect();
    assert_eq!(
        invokes.len(),
        1,
        "no automatic provider retry may be enqueued"
    );
    assert_eq!(invokes[0].attempt_count, 1);
    assert_eq!(invokes[0].status, WorkItemStatus::Failed);
    assert_eq!(stages::list_by_run(&pool, run_id).await.unwrap().len(), 1);
    assert_eq!(provider_prepare_calls.load(Ordering::SeqCst), 0);

    // Reuse precisely the denied identity: trust is a separate explicit fixture
    // action, and cannot be inferred from the failed run or a provider retry.
    let input = || HeadlessPreparationInput {
        run_id,
        invocation_id: AgentExecutionId::new(),
        owner_lineage: stage_id.to_string(),
        root: root.clone(),
        project_selector: None,
        permission_policy: json!({"xcode_headless":{"read":true,"gates":[]}}),
        timeout: Duration::from_secs(10),
    };
    let journal = Arc::new(DbXcodeEffectJournal::new(
        pool.clone(),
        writer.clone(),
        run_id.inner(),
        stage_id.to_string(),
    ));
    let denied = runtime
        .prepare(input(), journal.clone())
        .await
        .err()
        .expect("still untrusted before an explicit grant");
    let admission = denied
        .downcast_ref::<ProjectTrustAdmissionFailure>()
        .expect("typed trust failure");
    assert_eq!(admission.reason_code, "project_trust_required");
    assert_eq!(admission.root, root);
    assert_eq!(admission.project_key, *project.key());
    trust.grant(&project, "fixture-operator").unwrap();
    let prepared = runtime
        .prepare(input(), journal)
        .await
        .expect("exact identity must pass after explicit fixture trust");
    assert_eq!(host.opens.load(Ordering::SeqCst), 1);
    assert_eq!(provider_prepare_calls.load(Ordering::SeqCst), 0);
    drop(prepared);

    // Use the supported targeted stages.retry command after explicit trust. Its
    // new attempt must reach the adapter boundary; the probe intentionally stops
    // before creating a provider process or sending an ACP prompt.
    let handler = CommandHandler::new(pool.clone(), events, queue.clone());
    handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "review".into(),
                consume_quota_budget_now: false,
                agent_execution_id: Some(execution.id),
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: None,
                request_id: Some(uuid::Uuid::new_v4().to_string()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .expect("explicit stage retry after project trust");
    let attempts = stages::list_by_run(&pool, run_id).await.unwrap();
    assert_eq!(attempts.len(), 2);
    let retry_stage = attempts
        .iter()
        .find(|stage| stage.attempt_number == 2)
        .expect("retry creates attempt two");
    assert_ne!(retry_stage.id, stage_id);
    make_pending_items_due(&pool, run_id).await;
    let stopped = executor
        .process_next_item()
        .await
        .expect_err("fixture stops at provider adapter boundary");
    assert!(
        format!("{stopped:#}").contains("fixture_provider_launch_spec_reached"),
        "{stopped:#}"
    );
    assert_eq!(provider_prepare_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        *prepared_attempts.lock().unwrap(),
        vec![(retry_stage.id.to_string(), 2)]
    );
    let retry_executions = agent_executions::find_by_stage(&pool, retry_stage.id)
        .await
        .unwrap();
    assert_eq!(retry_executions.len(), 1);
    assert_ne!(retry_executions[0].id, execution.id);
    let retry_facts =
        agent_execution_runtime_facts::find_by_execution_id(&pool, retry_executions[0].id)
            .await
            .unwrap()
            .unwrap();
    assert_ne!(
        retry_facts.supervision_classification.as_deref(),
        Some("xcode_headless_prelaunch_failed")
    );
    let provider_sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM provider_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        provider_sessions, 0,
        "the adapter probe never launches a provider"
    );
    drop((handler, executor, runtime));
    controller.shutdown().await;
    writer.shutdown().await;
    pool.close().await;
}

async fn make_pending_items_due(pool: &sqlx::SqlitePool, run_id: RunId) {
    sqlx::query("UPDATE work_items SET scheduled_at = ?1 WHERE run_id = ?2 AND status = 'pending'")
        .bind((Utc::now() - chrono::Duration::seconds(2)).to_rfc3339())
        .bind(run_id.to_string())
        .execute(pool)
        .await
        .unwrap();
}

async fn seed_run(
    pool: &sqlx::SqlitePool,
    run_id: RunId,
    stage_id: StageExecutionId,
    root: &domain::execution_root::ResolvedExecutionRoot,
) {
    let idea_id = IdeaId::new();
    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "Trust admission fixture".into(),
            body: "fixture".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    let workflow = json!({
        "workflow": {"id": "trust_admission"}, "initial_state": "review",
        "states": {"review": {"label": "Review", "owner": "reviewer", "type": "end",
            "run": {"sequence": [{"agent": "reviewer", "task": "Read fixture"}]}}}
    })
    .to_string();
    let catalog = json!({
        "backend_profiles": {"review_profile": {"provider": "claude", "model": "sonnet"}},
        "permission_profiles": {"VERIFY": {"xcode_headless": {"read": true, "gates": []}}},
        "contracts": {"lead_contract": {"format": "json"}},
        "agents": [
            {"id": "reviewer", "backend_profile": "review_profile", "permission_profile": "VERIFY"},
            {"id": "lead_orchestrator", "system_role": "lead", "backend_profile": "review_profile",
                "permission_profile": "VERIFY", "lead_resolution_contract": "lead_contract"}
        ]
    })
    .to_string();
    runs::insert(
        pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "trust_admission".into(),
            workflow_title: "Trust admission fixture".into(),
            workspace_root: root.repository.clone(),
            artifact_root: root.effective.clone(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("review".into()),
            workflow_yaml_path: Some("frozen-workflow.yaml".into()),
            agent_catalog_yaml_path: Some("frozen-agents.yaml".into()),
            worktree_root: Some(root.effective.clone()),
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: None,
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: Some(format!("{:x}", Sha256::digest(workflow.as_bytes()))),
            catalog_snapshot_hash: Some(format!("{:x}", Sha256::digest(catalog.as_bytes()))),
            workflow_snapshot_json: Some(workflow),
            catalog_snapshot_json: Some(catalog),
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        },
    )
    .await
    .unwrap();
    stages::insert(
        pool,
        &StageExecution {
            id: stage_id,
            run_id,
            stage_id: "review".into(),
            label: "Review fixture".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
            owner_agent: Some("reviewer".into()),
            provider: Some("claude".into()),
            model: Some("sonnet".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();
}

struct LaunchProbe {
    calls: Arc<AtomicUsize>,
    attempts: Arc<std::sync::Mutex<Vec<(String, u32)>>>,
}
#[async_trait::async_trait]
impl acp::adapters::AcpAdapter for LaunchProbe {
    fn provider_name(&self) -> &str {
        "claude"
    }
    fn prepare_launch_spec(
        &self,
        req: &acp::ExecutionRequest,
        _resources: &mut acp::adapters::LaunchResourceGuard,
    ) -> Result<acp::adapters::AcpLaunchSpec> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.attempts.lock().unwrap().push((
            req.stage_execution_id.clone().expect("stage-owned fixture"),
            req.attempt_number,
        ));
        anyhow::bail!("fixture_provider_launch_spec_reached")
    }
    async fn execute(&self, _req: acp::ExecutionRequest) -> Result<acp::ExecutionResult> {
        anyhow::bail!("unexpected direct provider execution")
    }
}

struct FixtureHost {
    snapshot: Mutex<HostSnapshot>,
    opens: AtomicUsize,
    inspections: AtomicUsize,
}
impl FixtureHost {
    fn new() -> Self {
        Self {
            snapshot: Mutex::new(HostSnapshot {
                generation: ServiceGeneration {
                    uid: acp::current_process_uid(),
                    pid: 42,
                    start_sec: 10,
                    start_usec: 1,
                    boot_id: "fixture-boot".into(),
                    developer_dir: "/fixture/Developer".into(),
                    executable: "/fixture/Xcode Service".into(),
                    bundle_id: "com.apple.dt.mcp-server".into(),
                    build: "fixture".into(),
                    xcode_build: "27A266a".into(),
                },
                operator_home: "/fixture/home".into(),
                account_name: "fixture".into(),
                darwin_tmpdir: "/fixture/tmp".into(),
                open_projects: vec![],
            }),
            opens: AtomicUsize::new(0),
            inspections: AtomicUsize::new(0),
        }
    }
}
#[async_trait::async_trait]
impl HeadlessHostInspector for FixtureHost {
    async fn inspect(&self) -> Result<HostSnapshot> {
        self.inspections.fetch_add(1, Ordering::SeqCst);
        Ok(self.snapshot.lock().await.clone())
    }
}
struct FixturePeers(Arc<FixtureHost>);
struct FixturePeer(Arc<FixtureHost>);
#[async_trait::async_trait]
impl HeadlessPeerFactory for FixturePeers {
    async fn connect(
        &self,
        _host: &HostSnapshot,
        _deadline: tokio::time::Instant,
    ) -> Result<Box<dyn HeadlessPeer>> {
        Ok(Box::new(FixturePeer(self.0.clone())))
    }
}
#[async_trait::async_trait]
impl HeadlessPeer for FixturePeer {
    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }
    async fn open(&mut self, path: &str) -> Result<Value> {
        self.0.opens.fetch_add(1, Ordering::SeqCst);
        self.0.snapshot.lock().await.open_projects.push(path.into());
        Ok(
            json!({"isError": false, "structuredContent": {"workspaceIdentifier": "workspace-fixture", "workspacePath": path}}),
        )
    }
    async fn read(&mut self, _arguments: Value) -> Result<Value> {
        anyhow::bail!("unexpected project read")
    }
}

// Interleave real cancellation persistence at the asynchronous trust boundary;
// the trust decision itself still comes from the real, absent fixture store.
struct CancelDuringTrustCheck {
    pool: sqlx::SqlitePool,
    store: Arc<ProjectTrustStore>,
}
#[async_trait::async_trait]
impl HeadlessProjectTrust for CancelDuringTrustCheck {
    async fn check(&self, project: &TrustedProject) -> Result<String> {
        let execution_id: String =
            sqlx::query_scalar("SELECT id FROM agent_executions WHERE status = 'running'")
                .fetch_one(&self.pool)
                .await?;
        let execution_id = AgentExecutionId::from(uuid::Uuid::parse_str(&execution_id)?);
        let now = Utc::now();
        agent_executions::update_completed(&self.pool, execution_id, AgentStatus::Cancelled, now)
            .await?;
        let mut facts = AgentExecutionRuntimeFacts::defaults_for(execution_id, now);
        facts.supervision_classification = Some("fixture_operator_cancelled".into());
        facts.failure_message_redacted = Some("Cancelled while trust admission was pending".into());
        agent_execution_runtime_facts::upsert(&self.pool, &facts).await?;
        self.store.check(project).await
    }
}
