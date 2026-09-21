//! Provider-free ordinary approval integration against a real DB and Git checkout.

use chrono::Utc;
use db::repos::{
    approvals, run_continuation_approvals as bindings, run_continuations as continuations, runs,
    stages,
};
use domain::{
    approval::ApprovalDecision,
    commands::{ApprovalResolutionDecision, CallerContext, Command, ResolveApprovalCmd},
    ids::{RunId, StageExecutionId},
    run::{DeliveryConfiguration, Run},
    run_carry_forward::ContentDigest,
};
use engine::run_carry_forward::{
    preview::{
        preview, InputReference, PreviewInput, PreviewOutcome, PreviewSelection, PreviewTarget,
    },
    read_verified_manifest, CarryForwardFinalizer, PreparationJob, PreparationLane,
};
use engine::{command_handler::CommandHandler, orchestrator::Orchestrator, work_queue::WorkQueue};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command as Process,
    sync::Arc,
};
use uuid::Uuid;

const REVIEW: &str = "state_4_proposal_reviewed";
const GATE: &str = "state_6_implementation_approval";
const PREPARE: &str = "state_7_implementation_started";

fn git(root: &Path, args: &[&str]) {
    let output = Process::new("/usr/bin/git")
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for item in fs::read_dir(source).unwrap() {
        let item = item.unwrap();
        if item.file_type().unwrap().is_dir() {
            copy_tree(&item.path(), &destination.join(item.file_name()));
        } else {
            fs::copy(item.path(), destination.join(item.file_name())).unwrap();
        }
    }
}

struct Fixture {
    _temp: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    run: Run,
    orchestrator: Orchestrator,
    handler: CommandHandler,
}

struct PromptFixtureAdapter {
    provider: String,
    script: PathBuf,
    marker: PathBuf,
    fail_session_new: bool,
}

#[async_trait::async_trait]
impl acp::adapters::AcpAdapter for PromptFixtureAdapter {
    fn provider_name(&self) -> &str {
        &self.provider
    }

    fn prepare_launch_spec(
        &self,
        req: &acp::ExecutionRequest,
        _: &mut acp::adapters::LaunchResourceGuard,
    ) -> anyhow::Result<acp::adapters::AcpLaunchSpec> {
        anyhow::ensure!(
            !req.expected_outputs
                .iter()
                .any(|output| output.output_name == "approved_proposal")
                && !req
                    .expected_output_paths
                    .iter()
                    .any(|path| { Path::new(path).ends_with("proposals/approved/proposal.md") }),
            "P039 engine snapshot must not be requested as a provider-written output"
        );
        Ok(acp::adapters::AcpLaunchSpec::new("/usr/bin/python3")
            .with_arg(self.script.to_string_lossy())
            .with_arg(self.marker.to_string_lossy())
            .with_arg(serde_json::to_string(&req.expected_output_paths)?)
            .with_arg(if self.fail_session_new { "fail" } else { "ok" }))
    }

    fn prepare_session_new_spec(
        &self,
        _: &acp::ExecutionRequest,
    ) -> anyhow::Result<acp::adapters::AcpSessionNewSpec> {
        Ok(acp::adapters::AcpSessionNewSpec::new("fixture", "default"))
    }
}

impl Fixture {
    async fn execute_queued_preparation(&self) -> PathBuf {
        Box::pin(self.execute_preparation(false)).await
    }

    async fn execute_preparation(&self, fail_session_new: bool) -> PathBuf {
        // Prior transitions were driven through advance_run. Drain only their
        // redundant wakeups so process_next_item claims the ordinary InvokeAgent.
        sqlx::query(
            "UPDATE work_items SET status='completed' WHERE run_id=? AND kind='advance_run'",
        )
        .bind(self.run.id.to_string())
        .execute(&self.pool)
        .await
        .unwrap();
        let provider: String = sqlx::query_scalar("SELECT json_extract(payload_json,'$.provider') FROM work_items WHERE run_id=? AND kind='invoke_agent' AND status='pending' ORDER BY created_at LIMIT 1")
            .bind(self.run.id.to_string()).fetch_one(&self.pool).await.unwrap();
        let provider = domain::provider::ProviderFamily::canonicalize_known_alias(&provider)
            .unwrap_or(provider);
        let script = self._temp.path().join("prompt_fixture.py");
        let marker = self._temp.path().join("prompt_received");
        fs::write(&script, r#"import json, pathlib, sys
marker = pathlib.Path(sys.argv[1])
outputs = json.loads(sys.argv[2])
for line in sys.stdin:
    msg = json.loads(line)
    method = msg.get('method')
    result = {}
    if method == 'initialize':
        result = {'protocolVersion': 1}
    elif method == 'session/new':
        if sys.argv[3] == 'fail':
            print(json.dumps({'jsonrpc': '2.0', 'id': msg['id'], 'error': {'code': -32603, 'message': 'fixture interrupted before prompt'}}), flush=True)
            break
        result = {'sessionId': 'p039-ordinary-fixture'}
    elif method == 'session/prompt':
        marker.write_text('actual session/prompt received')
        for output in outputs:
            path = pathlib.Path(output)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('tasks: []\n' if path.suffix == '.yaml' else 'Implement the freshly approved proposal\n')
        result = {'stopReason': 'end_turn', 'sessionId': 'p039-ordinary-fixture'}
    if 'id' in msg:
        print(json.dumps({'jsonrpc': '2.0', 'id': msg['id'], 'result': result}), flush=True)
"#).unwrap();
        let acp = Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![Arc::new(
            PromptFixtureAdapter {
                provider,
                script,
                marker: marker.clone(),
                fail_session_new,
            },
        )]));
        let events = engine::event_bus::new_bus(64);
        let queue = WorkQueue::new(self.pool.clone());
        let orchestrator = Arc::new(self.restarted_orchestrator().await);
        let executor = engine::executor::BackgroundExecutor::new(
            self.pool.clone(),
            queue,
            orchestrator,
            acp.clone(),
            events,
        );
        // The production due-query deliberately rounds back one second.
        // Poll the ordinary worker rather than rewriting scheduling state.
        let due_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        let processed = tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                match Box::pin(executor.process_next_item()).await {
                    Ok(false) => {
                        assert!(
                            tokio::time::Instant::now() < due_deadline,
                            "ordinary InvokeAgent must become due within five seconds"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    result => break result,
                }
            }
        })
        .await;
        acp.close_all_sessions().await;
        let processed = processed
            .expect("ordinary InvokeAgent must finish the local protocol within sixty seconds");
        match processed {
            Ok(processed) => assert!(processed),
            Err(error) => {
                let detail = format!("{error:#}");
                assert!(
                    detail.contains("stale_approval_binding")
                        || detail.contains("artifact_provenance_invalid")
                        || (fail_session_new
                            && detail.contains("fixture interrupted before prompt")),
                    "unexpected executor failure: {detail}"
                );
            }
        }
        marker
    }

    async fn restarted_orchestrator(&self) -> Orchestrator {
        let base = fs::canonicalize(self._temp.path()).unwrap();
        let pool = db::pool::create_pool(&format!(
            "sqlite://{}?mode=rwc",
            base.join("fixture.db").display()
        ))
        .await
        .unwrap();
        let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer.clone())
            .await
            .unwrap();
        Orchestrator::new_with_db_writer(
            pool.clone(),
            engine::event_bus::new_bus(32),
            WorkQueue::new(pool),
            writer,
        )
    }

    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let root = base.join("repo");
        let owned = base.join("operations");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir(&owned).unwrap();
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        copy_tree(
            &repository.join("examples/agents"),
            &root.join("examples/agents"),
        );
        fs::create_dir_all(root.join("examples/workflows")).unwrap();
        fs::copy(
            repository.join("examples/workflows/workflow.yaml"),
            root.join("examples/workflows/current.yaml"),
        )
        .unwrap();
        fs::write(root.join(".gitignore"), ".chainworks/\n").unwrap();
        fs::write(root.join("product.txt"), "base\n").unwrap();
        let contract_path = "docs/evidence/rollout-contract/p039-rollout-contract.json";
        let contract: Value =
            serde_json::from_slice(&fs::read(repository.join(contract_path)).unwrap()).unwrap();
        let mut evidence_paths = vec![
            contract_path.to_owned(),
            contract["readback_fixture"].as_str().unwrap().to_owned(),
        ];
        evidence_paths.extend(
            contract["negative_fixtures"]
                .as_object()
                .unwrap()
                .values()
                .map(|p| p.as_str().unwrap().to_owned()),
        );
        for path in evidence_paths {
            fs::create_dir_all(root.join(&path).parent().unwrap()).unwrap();
            fs::copy(repository.join(&path), root.join(path)).unwrap();
        }
        git(&root, &["init", "-q"]);
        git(&root, &["add", "."]);
        git(
            &root,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "fixture base",
            ],
        );
        let pool = db::pool::create_pool(&format!(
            "sqlite://{}?mode=rwc",
            base.join("fixture.db").display()
        ))
        .await
        .unwrap();
        db::writer::register_shared_writer(
            &pool,
            Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .unwrap();
        let source = RunId::new();
        let idea = Uuid::new_v4();
        let metadata = root.join(format!(".chainworks/runs/{source}"));
        fs::create_dir_all(metadata.join("proposals/current")).unwrap();
        let seed = serde_json::to_vec(&json!({"title":"Continue bounded implementation work", "rollout_contract_v1":contract})).unwrap();
        fs::write(metadata.join("proposals/current/proposal.md"), &seed).unwrap();
        let plan = workflow::compiler::compile_for_new_run_v1(
            root.join("examples/workflows/current.yaml")
                .to_str()
                .unwrap(),
            root.join("examples/agents/agents.yaml").to_str().unwrap(),
        )
        .unwrap()
        .into_plan();
        let delivery = DeliveryConfiguration {
            repo_identifier: "fixture".into(),
            repo_root: root.to_str().unwrap().into(),
            base_branch: "HEAD".into(),
            worktree_base_path: owned.to_str().unwrap().into(),
            target_branch: "fixture-delivery".into(),
            release_target_id: Some("fixture".into()),
            release_mode: Some("sandbox".into()),
        };
        sqlx::query("INSERT INTO ideas(id,title,body,workspace_root_path,created_at) VALUES (?,'Fixture','Body',?,?)")
            .bind(idea.to_string()).bind(root.to_str().unwrap()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,worktree_root,chainworks_meta_root,workflow_snapshot_json,catalog_snapshot_json,workflow_snapshot_hash,catalog_snapshot_hash,delivery_configuration_json,agent_catalog_yaml_path) VALUES (?,?,'blocked','proposal_to_release','Workflow',?,?,?,'state_7_implementation_started',?,?,?,?,?,?,?,?)")
            .bind(source.to_string()).bind(idea.to_string()).bind(root.to_str().unwrap()).bind(metadata.to_str().unwrap()).bind(Utc::now().to_rfc3339())
            .bind(root.to_str().unwrap()).bind(metadata.to_str().unwrap()).bind(plan.workflow_snapshot_json).bind(plan.catalog_snapshot_json)
            .bind(plan.workflow_snapshot_hash).bind(plan.catalog_snapshot_hash).bind(serde_json::to_string(&delivery).unwrap()).bind(root.join("examples/agents/agents.yaml").to_str().unwrap())
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,'state_7_implementation_started','Implementation','blocked',0,1,?)")
            .bind(Uuid::new_v4().to_string()).bind(source.to_string()).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at) VALUES (?,?,'state_6_implementation_approval','granted','2026-09-19T00:00:00Z','2026-09-19T01:00:00Z')")
            .bind(Uuid::new_v4().to_string()).bind(source.to_string()).execute(&pool).await.unwrap();
        let head = Process::new("/usr/bin/git")
            .current_dir(&root)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        sqlx::query("UPDATE runs SET base_branch='HEAD',base_revision=? WHERE id=?")
            .bind(String::from_utf8(head.stdout).unwrap().trim())
            .bind(source.to_string())
            .execute(&pool)
            .await
            .unwrap();
        let seed_id = Uuid::new_v4();
        sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,created_at) VALUES (?,?,'historical','fixture','proposal_current','proposal_current','markdown',?,?,?,'fixture',?)")
            .bind(seed_id.to_string()).bind(source.to_string()).bind(metadata.join("proposals/current/proposal.md").to_str().unwrap())
            .bind(ContentDigest::of(&seed).as_str().trim_start_matches("sha256:")).bind(seed.len() as i64).bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
        let preview = preview(
            &pool,
            PreviewInput {
                source_run_id: source,
                target: PreviewTarget {
                    workflow_yaml_path: "examples/workflows/current.yaml".into(),
                    agent_catalog_yaml_path: "examples/agents/agents.yaml".into(),
                    expected_workflow_snapshot_hash: None,
                    expected_catalog_snapshot_hash: None,
                    delivery_configuration_json: serde_json::to_string(&delivery).unwrap(),
                },
                selection: PreviewSelection {
                    proposal_input: InputReference::Artifact { id: seed_id },
                    reference_inputs: vec![],
                    include_dirty_work: true,
                    excluded_workspace_paths: BTreeMap::new(),
                },
                profile: "implementation_restart_v1".into(),
                cursor: None,
                limit: None,
            },
        )
        .await
        .unwrap();
        let prepared = match preview {
            PreviewOutcome::Page { prepared, .. } => prepared,
            other => panic!("{other:?}"),
        };
        let op = continuations::reserve_checked(
            &pool,
            &continuations::Reservation {
                source_run_id: source.to_string(),
                caller_fingerprint: "approval-test".into(),
                caller_request_id: Uuid::new_v4().to_string(),
                intent_sha256: "a".repeat(64),
                plan_ref: "plan.json".into(),
                plan_sha256: prepared
                    .plan()
                    .plan_sha256
                    .as_str()
                    .trim_start_matches("sha256:")
                    .into(),
                target_ref: "target.json".into(),
                source_witness_sha256: prepared
                    .source_witness()
                    .semantic_sha256
                    .as_str()
                    .trim_start_matches("sha256:")
                    .into(),
                reason: "fixture".into(),
            },
            &owned,
        )
        .await
        .unwrap();
        let finalizer = Arc::new(CarryForwardFinalizer::new(prepared).unwrap());
        let lane = PreparationLane::new(pool.clone());
        let op = lane
            .try_submit(PreparationJob::new(
                Uuid::parse_str(&op.operation_id).unwrap(),
                finalizer.workspace(),
                owned,
                finalizer,
            ))
            .unwrap()
            .wait()
            .await
            .unwrap();
        lane.shutdown().await.unwrap();
        assert_eq!(op.phase, "prepared", "{op:?}");
        let manifest = read_verified_manifest(&op).await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        db::repos::run_continuation_inputs::insert_tx(&mut tx, &op, &manifest.inputs)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        continuations::activate(
            &pool,
            &op.operation_id,
            op.version,
            op.manifest_sha256.as_deref().unwrap(),
            &manifest.successor,
        )
        .await
        .unwrap();
        let run = runs::find_by_id(&pool, manifest.successor.id)
            .await
            .unwrap()
            .unwrap();
        let events = engine::event_bus::new_bus(128);
        Self {
            _temp: temp,
            orchestrator: Orchestrator::new(
                pool.clone(),
                events.clone(),
                WorkQueue::new(pool.clone()),
            ),
            handler: CommandHandler::new(pool.clone(), events, WorkQueue::new(pool.clone())),
            pool,
            run,
        }
    }

    async fn enter_gate(&self) -> domain::approval::Approval {
        self.finish_review().await;
        self.orchestrator.advance_run(self.run.id).await.unwrap();
        approvals::list_by_run(&self.pool, self.run.id)
            .await
            .unwrap()
            .into_iter()
            .find(|a| a.stage_id == GATE && a.decision == ApprovalDecision::Requested)
            .unwrap()
    }

    async fn finish_review(&self) {
        assert_eq!(
            runs::find_by_id(&self.pool, self.run.id)
                .await
                .unwrap()
                .unwrap()
                .current_state
                .as_deref(),
            Some(REVIEW)
        );
        // A provider-free completed review result; the production evaluator, not
        // this fixture, chooses and persists the review -> gate transition.
        let stage = StageExecutionId::new();
        sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at,completed_at) VALUES (?,?,?,'Fresh review','completed',0,1,?,?)")
            .bind(stage.to_string()).bind(self.run.id.to_string()).bind(REVIEW).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&self.pool).await.unwrap();
        let path = Path::new(&self.run.artifact_root).join("reviews/proposal/summary.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes = serde_json::to_vec(&json!({"average_score":10,"aggregate_score":10,"min_individual_score":10,"blocker_count":0,"blocking_issues":[],"required_changes":[],"blocking_required_changes":[],"advisory_follow_ups":[],"recurring_themes":[],"summary":"Fresh review passed", "decision":"pass","pass":true})).unwrap();
        self.output(stage, REVIEW, "proposal_review_summary", &path, &bytes)
            .await;
        self.orchestrator.advance_run(self.run.id).await.unwrap();
        assert_eq!(
            runs::find_by_id(&self.pool, self.run.id)
                .await
                .unwrap()
                .unwrap()
                .current_state
                .as_deref(),
            Some(GATE)
        );
    }

    async fn output(
        &self,
        stage: StageExecutionId,
        logical_stage: &str,
        name: &str,
        path: &Path,
        bytes: &[u8],
    ) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
        let agent = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,status,provider,started_at,completed_at) VALUES (?,?,'lead_orchestrator','completed','fixture',?,?)")
            .bind(agent.to_string()).bind(stage.to_string()).bind(&now).bind(&now).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO agent_execution_runtime_facts(agent_execution_id,output_settlement,valid_required_outputs,created_at,updated_at) VALUES (?,'valid_outputs_from_completed_execution',1,?,?)")
            .bind(agent.to_string()).bind(&now).bind(&now).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,agent_execution_id,created_at) VALUES (?,?,?,'lead_orchestrator',?,?,'json',?,?,?,'fixture',?,?)")
            .bind(Uuid::new_v4().to_string()).bind(self.run.id.to_string()).bind(logical_stage).bind(name).bind(name)
            .bind(path.to_str().unwrap()).bind(ContentDigest::of(bytes).as_str().trim_start_matches("sha256:")).bind(bytes.len() as i64).bind(agent.to_string()).bind(now).execute(&self.pool).await.unwrap();
    }

    async fn resolve(
        &self,
        approval: &domain::approval::Approval,
    ) -> anyhow::Result<engine::command_handler::Commanded> {
        self.decide(approval, ApprovalResolutionDecision::Approved)
            .await
    }

    async fn decide(
        &self,
        approval: &domain::approval::Approval,
        decision: ApprovalResolutionDecision,
    ) -> anyhow::Result<engine::command_handler::Commanded> {
        self.handler
            .handle(
                Command::ResolveApproval(ResolveApprovalCmd {
                    run_id: self.run.id,
                    stage_id: GATE.into(),
                    approval_id: approval.id,
                    decision,
                    rationale: Some("Fresh review accepted".into()),
                    idempotency_key: None,
                    request_id: Some(Uuid::new_v4().to_string()),
                }),
                CallerContext::mcp(
                    "operator",
                    &domain::PrincipalClass::Operator,
                    "approvals.resolve",
                ),
            )
            .await
    }
}

#[tokio::test]
async fn ordinary_gate_creates_exact_binding_and_stale_code_cannot_commit_decision() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    let binding = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .expect("ordinary gate must atomically bind approval");
    assert_eq!(binding.approval_id, approval.id.to_string());
    let stage = stages::find_by_id(&f.pool, binding.stage_execution_id.parse().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stage.status, domain::stage::StageStatus::WaitingApproval);
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "changed after review",
    )
    .unwrap();
    assert!(f.resolve(&approval).await.is_err());
    assert_eq!(
        approvals::find_by_id(&f.pool, approval.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Requested
    );
}

#[tokio::test]
async fn ordinary_waiting_gate_repairs_atomic_creation_gap_after_restart() {
    let f = Fixture::new().await;
    f.finish_review().await;
    sqlx::query("CREATE TRIGGER fail_approval_binding BEFORE INSERT ON run_continuation_approval_bindings BEGIN SELECT RAISE(ABORT,'crash before binding publication'); END")
        .execute(&f.pool).await.unwrap();
    assert!(f.orchestrator.advance_run(f.run.id).await.is_err());
    assert!(approvals::list_by_run(&f.pool, f.run.id)
        .await
        .unwrap()
        .is_empty());
    assert!(bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .is_none());
    let waiting = stages::list_by_run(&f.pool, f.run.id)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.stage_id == GATE)
        .unwrap();
    assert_eq!(waiting.status, domain::stage::StageStatus::WaitingApproval);
    sqlx::query("DROP TRIGGER fail_approval_binding")
        .execute(&f.pool)
        .await
        .unwrap();
    let restarted = f.restarted_orchestrator().await;
    restarted.advance_run(f.run.id).await.unwrap();
    let approvals = approvals::list_by_run(&f.pool, f.run.id).await.unwrap();
    assert_eq!(
        approvals.len(),
        1,
        "ordinary retry must repair the atomic publication gap"
    );
    assert_eq!(approvals[0].decision, ApprovalDecision::Requested);
    let binding = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.stage_execution_id, waiting.id.to_string());
    assert_eq!(binding.approval_id, approvals[0].id.to_string());
    restarted.advance_run(f.run.id).await.unwrap();
    assert_eq!(
        approvals::list_by_run(&f.pool, f.run.id)
            .await
            .unwrap()
            .len(),
        1
    );
    f.resolve(&approvals[0]).await.unwrap();
    restarted.advance_run(f.run.id).await.unwrap();
    restarted.advance_run(f.run.id).await.unwrap();
    assert!(bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap()
        .consumed_transition_id
        .is_some());
    assert!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent'"
        )
        .bind(f.run.id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap()
            > 0
    );
}

#[tokio::test]
async fn ordinary_waiting_gate_holds_inconsistent_existing_binding() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    let before = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "unreviewed change",
    )
    .unwrap();
    assert!(f.orchestrator.advance_run(f.run.id).await.is_err());
    let after = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(before.approval_id, after.approval_id);
    assert_eq!(before.generation, after.generation);
    assert_eq!(
        approvals::find_by_id(&f.pool, approval.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Requested
    );
}

#[tokio::test]
async fn ordinary_grant_transitions_and_first_dispatch_consumes_entry_once() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    assert_eq!(
        runs::find_by_id(&f.pool, f.run.id)
            .await
            .unwrap()
            .unwrap()
            .current_state
            .as_deref(),
        Some(PREPARE)
    );
    let binding = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert!(binding.consumed_transition_id.is_none());
    let snapshot_path = Path::new(&f.run.artifact_root).join("proposals/approved/proposal.md");
    let before = serde_json::to_value(
        engine::run_carry_forward::WorkspacePlanner::preview(
            Path::new(f.run.worktree_root.as_deref().unwrap()),
            Default::default(),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    fs::create_dir_all(snapshot_path.parent().unwrap()).unwrap();
    fs::write(&snapshot_path, b"unregistered approved proposal").unwrap();
    assert!(f.orchestrator.advance_run(f.run.id).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM artifacts WHERE run_id=? AND name='approved_proposal'"
        )
        .bind(f.run.id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent'"
        )
        .bind(f.run.id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    fs::remove_file(&snapshot_path).unwrap();
    let after = serde_json::to_value(
        engine::run_carry_forward::WorkspacePlanner::preview(
            Path::new(f.run.worktree_root.as_deref().unwrap()),
            Default::default(),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        before, after,
        "engine preflight must not change the approved checkout"
    );
    let retry = Box::pin(f.orchestrator.advance_run(f.run.id)).await;
    let after_retry = serde_json::to_value(
        engine::run_carry_forward::WorkspacePlanner::preview(
            Path::new(f.run.worktree_root.as_deref().unwrap()),
            Default::default(),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        before, after_retry,
        "successful snapshot/pre-enqueue checks must not change approved checkout"
    );
    retry.unwrap();
    let binding = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert!(
        binding.consumed_transition_id.is_some(),
        "first ordinary post-gate dispatch consumes entry"
    );
    assert!(
        engine::run_carry_forward::approval::consumed_entry_evidence(&f.pool, f.run.id)
            .await
            .unwrap()
            .is_some()
    );
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent' AND stage_id=?",
    )
    .bind(f.run.id.to_string())
    .bind(PREPARE)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert!(
        queued > 0,
        "ordinary dispatch missing: rollout={:?}; stages={:?}",
        db::repos::rollout_contract_checks::find_terminal_rollout_contract_check_for_run(
            &f.pool,
            f.run.id.inner()
        )
        .await
        .unwrap(),
        stages::list_by_run(&f.pool, f.run.id).await.unwrap()
    );
    for (stage, task) in [
        (PREPARE, "another_task"),
        (REVIEW, "freeze_approved_proposal_and_prepare_worktree"),
    ] {
        assert!(
            !engine::run_carry_forward::approval::owns_preparation_snapshot(
                &f.pool, f.run.id, stage, task,
            )
            .await
            .unwrap()
        );
    }
    let approved_bytes = fs::read(&snapshot_path).unwrap();
    let marker = Box::pin(f.execute_queued_preparation()).await;
    assert!(
        marker.exists(),
        "ordinary executor must reach the ACP session/prompt"
    );
    let started = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert!(started.first_execution_id.is_some());
    assert!(
        started.first_execution_started_at.is_some(),
        "production PromptSent sink must persist started evidence"
    );
    assert_eq!(
        fs::read(&snapshot_path).unwrap(),
        approved_bytes,
        "compliant preparation must preserve the engine snapshot bytes"
    );
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "legitimate implementation edit",
    )
    .unwrap();
    // A new pool/orchestrator must use persisted consumption, not process memory.
    let restarted = f.restarted_orchestrator().await;
    if let Err(error) = restarted.advance_run(f.run.id).await {
        let artifact_sizes: Vec<_> = db::repos::artifacts::list_by_run(&f.pool, f.run.id)
            .await
            .unwrap()
            .into_iter()
            .map(|artifact| {
                let path = Path::new(&f.run.workspace_root).join(&artifact.file_path);
                (
                    artifact.name,
                    artifact.size_bytes,
                    fs::metadata(path).ok().map(|metadata| metadata.len()),
                    artifact.file_path,
                )
            })
            .collect();
        panic!("ordinary resumed dispatch failed: {error:#}; artifact sizes: {artifact_sizes:?}");
    }
    let code_writers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent' AND json_extract(payload_json,'$.agent_id')='code_writer'")
        .bind(f.run.id.to_string()).fetch_one(&f.pool).await.unwrap();
    assert_eq!(
        code_writers, 1,
        "legitimate changed code must not block the next ordinary phase after restart"
    );
    let snapshots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifacts WHERE run_id=? AND name='approved_proposal' AND provider='engine'")
        .bind(f.run.id.to_string()).fetch_one(&f.pool).await.unwrap();
    assert_eq!(
        snapshots, 1,
        "ordinary handoff must publish only the fresh proof-bound snapshot"
    );
    assert_eq!(
        bindings::current(&f.pool, f.run.id, GATE)
            .await
            .unwrap()
            .unwrap()
            .consumed_transition_id,
        binding.consumed_transition_id
    );
}

#[tokio::test]
async fn queued_but_unclaimed_preparation_rechecks_code_and_snapshot_identity_bytes() {
    for change in ["code", "snapshot_bytes", "snapshot_identity"] {
        let f = Fixture::new().await;
        let approval = f.enter_gate().await;
        f.resolve(&approval).await.unwrap();
        Box::pin(f.orchestrator.advance_run(f.run.id))
            .await
            .unwrap();
        Box::pin(f.orchestrator.advance_run(f.run.id))
            .await
            .unwrap();
        if change == "snapshot_identity" {
            let snapshot = bindings::current(&f.pool, f.run.id, GATE)
                .await
                .unwrap()
                .unwrap()
                .snapshot_artifact_id
                .unwrap();
            sqlx::query("UPDATE artifacts SET is_pinned=0 WHERE id=?")
                .bind(&snapshot)
                .execute(&f.pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,is_pinned,created_at) SELECT ?,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,1,created_at FROM artifacts WHERE id=?")
                .bind(Uuid::new_v4().to_string()).bind(snapshot).execute(&f.pool).await.unwrap();
        } else {
            let path = if change == "snapshot_bytes" {
                Path::new(&f.run.artifact_root).join("proposals/approved/proposal.md")
            } else {
                Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt")
            };
            fs::write(path, "changed after enqueue, before claim").unwrap();
        }
        let marker = Box::pin(f.execute_queued_preparation()).await;
        assert!(
            !marker.exists(),
            "stale queued request reached the provider prompt"
        );
        assert!(bindings::current(&f.pool, f.run.id, GATE)
            .await
            .unwrap()
            .unwrap()
            .first_execution_started_at
            .is_none());
    }
}

#[tokio::test]
async fn admitted_execution_without_prompt_sent_keeps_original_tuple_after_restart() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    Box::pin(f.orchestrator.advance_run(f.run.id))
        .await
        .unwrap();
    Box::pin(f.orchestrator.advance_run(f.run.id))
        .await
        .unwrap();
    let marker = Box::pin(f.execute_preparation(true)).await;
    assert!(!marker.exists());
    let binding = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert!(
        binding.first_execution_id.is_some(),
        "executor admission must precede session/new"
    );
    assert!(binding.first_execution_started_at.is_none());
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "changed after failed first admission",
    )
    .unwrap();
    let restarted = f.restarted_orchestrator().await;
    let _ = Box::pin(restarted.advance_run(f.run.id)).await;
    assert!(
        engine::run_carry_forward::approval::consumed_entry_evidence(&f.pool, f.run.id)
            .await
            .is_err()
    );
    assert!(bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap()
        .first_execution_started_at
        .is_none());
}

#[tokio::test]
async fn prompt_sink_failure_does_not_turn_admission_into_started_authority() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    Box::pin(f.orchestrator.advance_run(f.run.id))
        .await
        .unwrap();
    Box::pin(f.orchestrator.advance_run(f.run.id))
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER fail_started_evidence BEFORE UPDATE OF first_execution_started_at ON run_continuation_approval_bindings BEGIN SELECT RAISE(ABORT,'started evidence unavailable'); END")
        .execute(&f.pool).await.unwrap();
    let marker = Box::pin(f.execute_queued_preparation()).await;
    assert!(marker.exists());
    let binding = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert!(
        binding.first_execution_id.is_some(),
        "actual executor must persist admission"
    );
    assert!(binding.first_execution_started_at.is_none());
    sqlx::query("DROP TRIGGER fail_started_evidence")
        .execute(&f.pool)
        .await
        .unwrap();
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "changed without durable PromptSent",
    )
    .unwrap();
    let restarted = f.restarted_orchestrator().await;
    assert!(Box::pin(restarted.advance_run(f.run.id)).await.is_err());
    assert!(bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap()
        .first_execution_started_at
        .is_none());
}

#[tokio::test]
async fn failed_snapshot_publication_then_restart_does_not_spend_code_freshness() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    let snapshot = Path::new(&f.run.artifact_root).join("proposals/approved/proposal.md");
    fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
    fs::write(&snapshot, b"unregistered snapshot blocks publication").unwrap();
    assert!(Box::pin(f.orchestrator.advance_run(f.run.id))
        .await
        .is_err());
    fs::remove_file(snapshot).unwrap();
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "unreviewed after failed publication",
    )
    .unwrap();
    let restarted = f.restarted_orchestrator().await;
    assert!(
        restarted.advance_run(f.run.id).await.is_err(),
        "failed snapshot authorization must not exempt code drift"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent'"
        )
        .bind(f.run.id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn changed_code_after_grant_cannot_authorize_transition() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "changed before transition",
    )
    .unwrap();
    let _ = f.orchestrator.advance_run(f.run.id).await;
    assert_eq!(
        runs::find_by_id(&f.pool, f.run.id)
            .await
            .unwrap()
            .unwrap()
            .current_state
            .as_deref(),
        Some(GATE)
    );
    assert!(bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap()
        .consumed_transition_id
        .is_none());
    assert_eq!(
        approvals::find_by_id(&f.pool, approval.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Granted
    );
}

#[tokio::test]
async fn changed_code_after_transition_cannot_authorize_first_dispatch() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    fs::write(
        Path::new(f.run.worktree_root.as_deref().unwrap()).join("product.txt"),
        "changed before first dispatch",
    )
    .unwrap();
    let _ = f.orchestrator.advance_run(f.run.id).await;
    let queued: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent' AND stage_id=?",
    )
    .bind(f.run.id.to_string())
    .bind(PREPARE)
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(queued, 0);
    assert!(bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap()
        .consumed_transition_id
        .is_none());
}

#[tokio::test]
async fn altered_seed_or_manifest_cannot_commit_an_ordinary_decision() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    let seed = Path::new(&f.run.artifact_root).join("proposals/current/proposal.md");
    let original = fs::read(&seed).unwrap();
    fs::write(&seed, "unreviewed replacement").unwrap();
    assert!(f.resolve(&approval).await.is_err());
    fs::write(&seed, original).unwrap();
    let op = continuations::links(&f.pool, &f.run.id.to_string())
        .await
        .unwrap()
        .incoming
        .unwrap();
    fs::write(op.manifest_ref.unwrap(), b"{}").unwrap();
    assert!(f.resolve(&approval).await.is_err());
    assert_eq!(
        approvals::find_by_id(&f.pool, approval.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Requested
    );
}

#[tokio::test]
async fn rejection_refinement_and_new_review_create_a_new_gate_binding() {
    let f = Fixture::new().await;
    let historical_grant = domain::approval::Approval {
        id: domain::ids::ApprovalId::new(),
        run_id: f.run.id,
        stage_id: GATE.into(),
        decision: ApprovalDecision::Granted,
        requested_at: Utc::now() - chrono::Duration::days(1),
        decided_at: Some(Utc::now() - chrono::Duration::hours(1)),
        comment: Some("Historical grant is not this gate's authority".into()),
        expires_at: None,
    };
    approvals::insert(&f.pool, &historical_grant).await.unwrap();
    let first = f.enter_gate().await;
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    assert_eq!(
        runs::find_by_id(&f.pool, f.run.id)
            .await
            .unwrap()
            .unwrap()
            .current_state
            .as_deref(),
        Some(GATE)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent'"
        )
        .bind(f.run.id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    let previous = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    f.decide(&first, ApprovalResolutionDecision::Rejected)
        .await
        .unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    assert_eq!(
        runs::find_by_id(&f.pool, f.run.id)
            .await
            .unwrap()
            .unwrap()
            .current_state
            .as_deref(),
        Some("state_5_proposal_refined")
    );
    let refinement = StageExecutionId::new();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at,completed_at) VALUES (?,?,'state_5_proposal_refined','Refined','completed',0,1,?,?)")
        .bind(refinement.to_string()).bind(f.run.id.to_string()).bind(Utc::now().to_rfc3339()).bind(Utc::now().to_rfc3339()).execute(&f.pool).await.unwrap();
    let proposal = Path::new(&f.run.artifact_root).join("proposals/current/proposal.md");
    let mut content: Value = serde_json::from_slice(&fs::read(&proposal).unwrap()).unwrap();
    content["title"] = json!("Fresh refinement following rejection");
    f.output(
        refinement,
        "state_5_proposal_refined",
        "proposal_current",
        &proposal,
        &serde_json::to_vec(&content).unwrap(),
    )
    .await;
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    f.finish_review().await;
    sqlx::query("CREATE TRIGGER fail_later_binding BEFORE INSERT ON run_continuation_approval_bindings BEGIN SELECT RAISE(ABORT,'later gate publication crash'); END")
        .execute(&f.pool).await.unwrap();
    assert!(f.orchestrator.advance_run(f.run.id).await.is_err());
    assert_eq!(
        bindings::current(&f.pool, f.run.id, GATE)
            .await
            .unwrap()
            .unwrap()
            .approval_id,
        previous.approval_id
    );
    sqlx::query("DROP TRIGGER fail_later_binding")
        .execute(&f.pool)
        .await
        .unwrap();
    let restarted = f.restarted_orchestrator().await;
    Box::pin(restarted.advance_run(f.run.id)).await.unwrap();
    Box::pin(restarted.advance_run(f.run.id)).await.unwrap();
    let second = approvals::list_by_run(&f.pool, f.run.id)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.decision == ApprovalDecision::Requested)
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM approvals WHERE run_id=? AND decision='requested'"
        )
        .bind(f.run.id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
    let current = bindings::current(&f.pool, f.run.id, GATE)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_ne!(previous.stage_execution_id, current.stage_execution_id);
    assert_eq!(current.generation, previous.generation + 1);
    assert_eq!(
        approvals::find_by_id(&f.pool, first.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Rejected
    );
    assert!(f.resolve(&first).await.is_err());
    assert_eq!(
        approvals::find_by_id(&f.pool, second.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Requested
    );
    f.resolve(&second).await.unwrap();
    assert_eq!(
        approvals::find_by_id(&f.pool, historical_grant.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Granted,
        "historical decision must not be rewritten"
    );
}

#[tokio::test]
async fn successor_with_ordinary_fresh_grant_can_preview_a_linear_fork_from_its_seed() {
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    assert!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent'"
        )
        .bind(f.run.id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap()
            > 0,
        "linear-fork fixture must first admit real ordinary successor work"
    );
    let run = runs::find_by_id(&f.pool, f.run.id).await.unwrap().unwrap();
    assert_eq!(run.current_state.as_deref(), Some(PREPARE));
    let seed =
        db::repos::run_continuation_inputs::execution_seed(&f.pool, run.id, "proposal_current")
            .await
            .unwrap()
            .unwrap();
    let incoming = continuations::links(&f.pool, &run.id.to_string())
        .await
        .unwrap()
        .incoming
        .unwrap();
    // Provider-free terminal failure settlement. No proposal output was produced
    // in B and no run state is jumped: it stays at its ordinary reached frontier.
    sqlx::query("UPDATE work_items SET status='completed' WHERE run_id=?")
        .bind(run.id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE stage_executions SET status='blocked' WHERE run_id=? AND stage_id=?")
        .bind(run.id.to_string())
        .bind(PREPARE)
        .execute(&f.pool)
        .await
        .unwrap();
    runs::update_status(&f.pool, run.id, domain::run::RunStatus::Blocked)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artifacts")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let result = preview(
        &f.pool,
        PreviewInput {
            source_run_id: run.id,
            target: PreviewTarget {
                workflow_yaml_path: "examples/workflows/current.yaml".into(),
                agent_catalog_yaml_path: "examples/agents/agents.yaml".into(),
                expected_workflow_snapshot_hash: None,
                expected_catalog_snapshot_hash: None,
                delivery_configuration_json: run.delivery_configuration_json.clone().unwrap(),
            },
            selection: PreviewSelection {
                proposal_input: InputReference::CarriedInput {
                    id: Uuid::parse_str(&seed.input_id).unwrap(),
                },
                reference_inputs: vec![],
                include_dirty_work: true,
                excluded_workspace_paths: BTreeMap::new(),
            },
            profile: "implementation_restart_v1".into(),
            cursor: None,
            limit: None,
        },
    )
    .await
    .unwrap();
    let PreviewOutcome::Page { prepared, .. } = result else {
        panic!("expected linear continuation preview: {result:?}")
    };
    let proposal = prepared
        .inputs()
        .iter()
        .find(|input| input.logical_name == "proposal_current")
        .unwrap();
    assert_eq!(
        proposal.reference,
        InputReference::CarriedInput {
            id: Uuid::parse_str(&seed.input_id).unwrap()
        }
    );
    assert_eq!(
        proposal.original_artifact_id.to_string(),
        seed.original_artifact_id
    );
    assert_eq!(
        proposal.ancestor_manifest_sha256,
        vec![ContentDigest::from_sha256_hex(incoming.manifest_sha256.as_deref().unwrap()).unwrap()]
    );
    assert_eq!(
        before,
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artifacts")
            .fetch_one(&f.pool)
            .await
            .unwrap()
    );
    assert!(
        continuations::links(&f.pool, &run.id.to_string())
            .await
            .unwrap()
            .outgoing
            .is_none(),
        "preview must not reserve or activate C"
    );
}

#[tokio::test]
async fn rollout_preflight_rejects_external_forged_and_tampered_approval_snapshots() {
    use engine::rollout_contract_preflight::implementation_run_start_rollout_contract_preflight as preflight;
    let f = Fixture::new().await;
    let approval = f.enter_gate().await;
    f.resolve(&approval).await.unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    f.orchestrator.advance_run(f.run.id).await.unwrap();
    let run = runs::find_by_id(&f.pool, f.run.id).await.unwrap().unwrap();
    let snapshot = db::repos::artifacts::list_by_run(&f.pool, run.id)
        .await
        .unwrap()
        .into_iter()
        .find(|a| a.name == "approved_proposal")
        .unwrap();
    assert_eq!(
        preflight(&f.pool, &run, Some(&snapshot))
            .await
            .unwrap()
            .action,
        engine::rollout_contract_preflight::RolloutContractPreflightAction::Allow
    );

    let original = fs::read(&snapshot.file_path).unwrap();
    let external = f._temp.path().join("unrelated-proposal.json");
    fs::write(&external, &original).unwrap();
    let mut forged = snapshot.clone();
    forged.file_path = external.to_string_lossy().into_owned();
    assert!(
        preflight(&f.pool, &run, Some(&forged)).await.is_err(),
        "a known artifact ID must not authorize an unrelated external path"
    );

    let mut changed_root = run.clone();
    changed_root.artifact_root = f._temp.path().to_string_lossy().into_owned();
    assert!(
        preflight(&f.pool, &changed_root, Some(&snapshot))
            .await
            .is_err(),
        "caller metadata must not broaden the canonical operation root"
    );

    sqlx::query("UPDATE artifacts SET stage_id=? WHERE id=?")
        .bind(GATE)
        .bind(snapshot.id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();
    let old_gate = db::repos::artifacts::find_by_id(&f.pool, snapshot.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        preflight(&f.pool, &run, Some(&old_gate)).await.is_err(),
        "an engine-labelled artifact from an old gate is not a fresh snapshot"
    );
    sqlx::query("UPDATE artifacts SET stage_id=? WHERE id=?")
        .bind(PREPARE)
        .bind(snapshot.id.to_string())
        .execute(&f.pool)
        .await
        .unwrap();

    fs::write(&snapshot.file_path, b"tampered snapshot").unwrap();
    assert!(
        preflight(&f.pool, &run, Some(&snapshot)).await.is_err(),
        "a cached successful rollout check must not hide tampered snapshot bytes"
    );
    fs::remove_file(&snapshot.file_path).unwrap();
    std::os::unix::fs::symlink(&external, &snapshot.file_path).unwrap();
    assert!(
        preflight(&f.pool, &run, Some(&snapshot)).await.is_err(),
        "matching bytes behind a symlink are not operation-owned snapshot evidence"
    );
}
