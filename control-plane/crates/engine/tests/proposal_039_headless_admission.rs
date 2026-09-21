//! CF-18/24 offline composition. Only the external host/peer are fixtures;
//! carry-forward, root selection, trust, admission, and effect persistence are real.
#![cfg(unix)]

use acp::{
    execution_root::resolve_execution_root,
    xcode_coordinator::{CoordinatorLimits, FixtureJournalAuthority, WorkspaceAccessCoordinator},
    xcode_headless::{HeadlessPeer, HeadlessPeerFactory, HeadlessWorkspaceController},
    xcode_headless_host::{HeadlessHostInspector, HostSnapshot, ServiceGeneration, TrustedProject},
    xcode_headless_runtime::{HeadlessPreparationInput, HeadlessProjectTrust, HeadlessRuntime},
    xcode_project_trust::ProjectTrustStore,
    ExecutionRequest, XcodeEffectJournal,
};
use anyhow::{ensure, Result};
use db::{
    repos::{run_continuations as continuations, runs},
    writer::DbWriter,
};
use domain::{
    execution_root::ExecutionRootKind,
    ids::{AgentExecutionId, RunId},
    run::{DeliveryConfiguration, Run},
    run_carry_forward::ContentDigest,
};
use engine::{
    run_carry_forward::{
        preview::{
            preview, InputReference, PreviewInput, PreviewOutcome, PreviewSelection, PreviewTarget,
        },
        read_verified_manifest, CarryForwardFinalizer, PreparationJob, PreparationLane,
    },
    xcode_effect_journal::{
        frozen_xcode_permission_policy, DbXcodeEffectJournal, DbXcodeProjectHoldCheck,
    },
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use uuid::Uuid;
use workflow::plan::ResolvedAgent;

const PROJECT: &str = "Fixture.xcodeproj";
const SENTINEL: &str = "Sentinel.txt";

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("/usr/bin/git")
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
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
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

#[derive(Default)]
struct Calls {
    inspections: usize,
    connections: usize,
    initializations: usize,
    opens: Vec<String>,
    reads: Vec<Value>,
    consent_denied: bool,
}

struct FixtureHost {
    snapshot: Mutex<HostSnapshot>,
    calls: Mutex<Calls>,
    pool: sqlx::SqlitePool,
}

#[async_trait::async_trait]
impl HeadlessHostInspector for FixtureHost {
    async fn inspect(&self) -> Result<HostSnapshot> {
        self.calls.lock().await.inspections += 1;
        Ok(self.snapshot.lock().await.clone())
    }
}

struct FixturePeers(Arc<FixtureHost>);
struct FixturePeer {
    host: Arc<FixtureHost>,
    opened: Option<PathBuf>,
}

#[async_trait::async_trait]
impl HeadlessPeerFactory for FixturePeers {
    async fn connect(
        &self,
        _: &HostSnapshot,
        _: tokio::time::Instant,
    ) -> Result<Box<dyn HeadlessPeer>> {
        self.0.calls.lock().await.connections += 1;
        Ok(Box::new(FixturePeer {
            host: self.0.clone(),
            opened: None,
        }))
    }
}

#[async_trait::async_trait]
impl HeadlessPeer for FixturePeer {
    async fn initialize(&mut self) -> Result<()> {
        let mut calls = self.host.calls.lock().await;
        calls.initializations += 1;
        // Inject an external denial; this does not simulate Apple's consent store.
        ensure!(!calls.consent_denied, "fixture_native_consent_required");
        Ok(())
    }

    async fn open(&mut self, path: &str) -> Result<Value> {
        self.host.calls.lock().await.opens.push(path.to_owned());
        let held: i64 =
            sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds WHERE canonical_path=?")
                .bind(path)
                .fetch_one(&self.host.pool)
                .await?;
        ensure!(held == 1, "fixture_open_without_exact_durable_fence");
        self.opened = Some(PathBuf::from(path));
        self.host
            .snapshot
            .lock()
            .await
            .open_projects
            .push(path.into());
        Ok(json!({"isError":false,"structuredContent":{
            "workspaceIdentifier":"successor-fixture", "workspacePath":path
        }}))
    }

    async fn read(&mut self, arguments: Value) -> Result<Value> {
        self.host.calls.lock().await.reads.push(arguments.clone());
        ensure!(
            arguments["workspaceIdentifier"] == "successor-fixture",
            "wrong_workspace"
        );
        ensure!(arguments["filePath"] == SENTINEL, "unexpected_fixture_read");
        let project = self.opened.as_ref().expect("read follows open");
        let bytes = fs::read(project.parent().unwrap().join(SENTINEL))?;
        let text = std::str::from_utf8(&bytes)?.trim_end();
        Ok(json!({"isError":false,"structuredContent":{
            "content":format!("1\t{text}"), "filePath":SENTINEL, "fileSize":bytes.len(),
            "totalLines":1,"linesRead":1,"startLine":1
        }}))
    }
}

struct Fixture {
    _temp: tempfile::TempDir,
    main: PathBuf,
    source: PathBuf,
    checkout: PathBuf,
    pool: sqlx::SqlitePool,
    writer: Arc<DbWriter>,
    run: Run,
    agent: ResolvedAgent,
    trust: Arc<ProjectTrustStore>,
    source_project: TrustedProject,
    successor_project: TrustedProject,
    source_grant: String,
    host: Arc<FixtureHost>,
    controller: Arc<HeadlessWorkspaceController>,
    runtime: Arc<HeadlessRuntime>,
    journal: Arc<dyn XcodeEffectJournal>,
}

impl Fixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().canonicalize().unwrap();
        let main = base.join("main");
        let source = base.join("source");
        let owned = base.join("operations");
        fs::create_dir(&main).unwrap();
        fs::create_dir(&owned).unwrap();
        let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        copy_tree(
            &repository.join("examples/agents"),
            &main.join("examples/agents"),
        );
        fs::create_dir_all(main.join("examples/workflows")).unwrap();
        fs::copy(
            repository.join("examples/workflows/workflow.yaml"),
            main.join("examples/workflows/current.yaml"),
        )
        .unwrap();
        fs::create_dir(main.join(PROJECT)).unwrap();
        fs::write(
            main.join(PROJECT).join("project.pbxproj"),
            "// offline fixture\n",
        )
        .unwrap();
        fs::write(main.join(SENTINEL), "main checkout\n").unwrap();
        fs::write(main.join(".gitignore"), ".chainworks/\n").unwrap();
        git(&main, &["init", "-q"]);
        git(&main, &["add", "."]);
        git(
            &main,
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
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "source",
                source.to_str().unwrap(),
            ],
        );
        fs::write(source.join(SENTINEL), "carried successor\n").unwrap();
        let source_root = resolve_execution_root(
            main.to_str().unwrap(),
            Some(source.to_str().unwrap()),
            false,
            Some("shared_implementation_worktree"),
        )
        .unwrap();
        let source_project =
            TrustedProject::resolve(source_root, Some(PROJECT), acp::current_process_uid())
                .unwrap();
        let trust = Arc::new(ProjectTrustStore::new(base.join("operator-trust")));
        let source_grant = trust.grant(&source_project, "source-operator").unwrap();
        let db_path = base.join("fixture.sqlite");
        let pool = db::pool::create_pool(&format!("sqlite://{}?mode=rwc", db_path.display()))
            .await
            .unwrap();
        let writer = Arc::new(DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer.clone())
            .await
            .unwrap();
        let source_run = RunId::new();
        let idea = Uuid::new_v4();
        let metadata = source.join(format!(".chainworks/runs/{source_run}"));
        fs::create_dir_all(metadata.join("proposals/current")).unwrap();
        let seed = b"Bounded offline headless admission proof\n";
        fs::write(metadata.join("proposals/current/proposal.md"), seed).unwrap();
        let plan = workflow::compiler::compile_for_new_run_v1(
            source
                .join("examples/workflows/current.yaml")
                .to_str()
                .unwrap(),
            source.join("examples/agents/agents.yaml").to_str().unwrap(),
        )
        .unwrap()
        .into_plan();
        let delivery = DeliveryConfiguration {
            repo_identifier: "headless-fixture".into(),
            repo_root: main.to_str().unwrap().into(),
            base_branch: "HEAD".into(),
            worktree_base_path: owned.to_str().unwrap().into(),
            target_branch: "fixture-delivery".into(),
            release_target_id: Some("fixture".into()),
            release_mode: Some("sandbox".into()),
        };
        sqlx::query("INSERT INTO ideas(id,title,body,workspace_root_path,created_at) VALUES (?,'Fixture','Body',?,'2026-09-20T00:00:00Z')")
            .bind(idea.to_string()).bind(main.to_str().unwrap()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,worktree_root,chainworks_meta_root,workflow_snapshot_json,catalog_snapshot_json,workflow_snapshot_hash,catalog_snapshot_hash,delivery_configuration_json,agent_catalog_yaml_path,base_branch,base_revision) VALUES (?,?,'blocked','proposal_to_release','Workflow',?,?,'2026-09-20T00:00:00Z','state_7_implementation_started',?,?,?,?,?,?,?,?,'HEAD',?)")
            .bind(source_run.to_string()).bind(idea.to_string()).bind(main.to_str().unwrap()).bind(metadata.to_str().unwrap())
            .bind(source.to_str().unwrap()).bind(metadata.to_str().unwrap()).bind(plan.workflow_snapshot_json).bind(plan.catalog_snapshot_json)
            .bind(plan.workflow_snapshot_hash).bind(plan.catalog_snapshot_hash).bind(serde_json::to_string(&delivery).unwrap()).bind(source.join("examples/agents/agents.yaml").to_str().unwrap())
            .bind(git(&source, &["rev-parse", "HEAD"])).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at) VALUES (?,?,'state_7_implementation_started','Implementation','blocked',0,1,'2026-09-20T00:00:00Z')")
            .bind(Uuid::new_v4().to_string()).bind(source_run.to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at) VALUES (?,?,'state_6_implementation_approval','granted','2026-09-19T00:00:00Z','2026-09-19T01:00:00Z')")
            .bind(Uuid::new_v4().to_string()).bind(source_run.to_string()).execute(&pool).await.unwrap();
        let seed_id = Uuid::new_v4();
        sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,checksum_sha256,size_bytes,provider,created_at) VALUES (?,?,'historical','fixture','proposal_current','proposal_current','markdown',?,?,?,'fixture','2026-09-20T00:00:00Z')")
            .bind(seed_id.to_string()).bind(source_run.to_string()).bind(metadata.join("proposals/current/proposal.md").to_str().unwrap())
            .bind(ContentDigest::of(seed).as_str().trim_start_matches("sha256:")).bind(seed.len() as i64).execute(&pool).await.unwrap();
        let result = preview(
            &pool,
            PreviewInput {
                source_run_id: source_run,
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
        let prepared = match result {
            PreviewOutcome::Page { prepared, .. } => prepared,
            other => panic!("{other:?}"),
        };
        let op = continuations::reserve_checked(
            &pool,
            &continuations::Reservation {
                source_run_id: source_run.to_string(),
                caller_fingerprint: "headless-test".into(),
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
                reason: "offline CF-18/24 fixture".into(),
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
        assert_eq!(manifest.workspace.source_root, source);
        let checkout = manifest.workspace.checkout_root.clone();
        assert_ne!(checkout, main);
        assert_ne!(checkout, source);
        assert_eq!(
            fs::read(checkout.join(SENTINEL)).unwrap(),
            b"carried successor\n"
        );
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
        assert_ne!(run.id, source_run);
        // Model the first headless invocation using an actual frozen dynamic
        // reviewer, without running the router, provider, or background executor.
        let binding = manifest
            .target
            .plan
            .dynamic_candidate_bindings
            .iter()
            .find(|binding| {
                binding.enabled_for_proposal_review
                    && binding.agent_id == "proposal_reviewer_apple_architect"
            })
            .expect("frozen Apple reviewer binding");
        let agent: ResolvedAgent =
            serde_json::from_str(&binding.resolved_agent_snapshot_json).unwrap();
        let successor_root = resolve_execution_root(
            &run.workspace_root,
            run.worktree_root.as_deref(),
            agent.worktree_write_enabled,
            agent.worktree_strategy.as_deref(),
        )
        .unwrap();
        let successor_project =
            TrustedProject::resolve(successor_root, Some(PROJECT), acp::current_process_uid())
                .unwrap();
        let host = Arc::new(FixtureHost {
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
                operator_home: base.clone(),
                account_name: "fixture".into(),
                darwin_tmpdir: base.clone(),
                open_projects: vec![source.join(PROJECT), main.join(PROJECT)],
            }),
            calls: Mutex::new(Calls::default()),
            pool: pool.clone(),
        });
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
        let authority = FixtureJournalAuthority::open(&base.join("authority"), &db_path).unwrap();
        let runtime =
            HeadlessRuntime::new_fixture(controller.clone(), coordinator, trust.clone(), authority);
        let journal = Arc::new(DbXcodeEffectJournal::new(
            pool.clone(),
            writer.clone(),
            run.id.inner(),
            "successor-first-task".into(),
        ));
        Self {
            _temp: temp,
            main,
            source,
            checkout,
            pool,
            writer,
            run,
            agent,
            trust,
            source_project,
            successor_project,
            source_grant,
            host,
            controller,
            runtime,
            journal,
        }
    }

    fn input(&self) -> HeadlessPreparationInput {
        HeadlessPreparationInput {
            run_id: self.run.id,
            invocation_id: AgentExecutionId::new(),
            owner_lineage: "successor-first-task".into(),
            root: resolve_execution_root(
                &self.run.workspace_root,
                self.run.worktree_root.as_deref(),
                self.agent.worktree_write_enabled,
                self.agent.worktree_strategy.as_deref(),
            )
            .unwrap(),
            project_selector: Some(PROJECT.into()),
            permission_policy: frozen_xcode_permission_policy(
                self.run.catalog_snapshot_json.as_deref(),
                self.agent.permission_profile.as_deref(),
            )
            .unwrap(),
            timeout: Duration::from_secs(10),
        }
    }

    fn request(&self, invocation: AgentExecutionId) -> ExecutionRequest {
        serde_json::from_value(json!({"run_id":self.run.id,"agent_execution_id":invocation,
            "stage_id":self.run.current_state,"agent_id":self.agent.agent_id,"provider":"fixture",
            "workspace_root":self.run.workspace_root,"worktree_root":self.run.worktree_root,
            "worktree_write_enabled":self.agent.worktree_write_enabled,"worktree_strategy":self.agent.worktree_strategy,
            "prompt":"Read carried sentinel", "session_generation_id":"successor-only-generation"})).unwrap()
    }

    async fn assert_no_open_or_effect(&self) {
        let calls = self.host.calls.lock().await;
        assert!(calls.opens.is_empty());
        assert!(calls.reads.is_empty());
        drop(calls);
        let effects: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_effect_attempts")
            .fetch_one(&self.pool)
            .await
            .unwrap();
        let holds: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&self.pool)
            .await
            .unwrap();
        assert_eq!(
            (effects, holds),
            (0, 0),
            "pre-dispatch refusal must not create an ambiguous effect"
        );
    }

    async fn close(self) {
        self.controller.shutdown().await;
        self.writer.shutdown().await;
        self.pool.close().await;
    }
}

#[tokio::test]
async fn cf24_carried_project_does_not_inherit_source_trust_or_open() {
    let f = Fixture::new().await;
    assert_eq!(
        f.trust.check(&f.source_project).await.unwrap(),
        f.source_grant
    );
    assert_ne!(f.successor_project.key(), f.source_project.key());
    assert!(f.trust.check(&f.successor_project).await.is_err());
    let error = f
        .runtime
        .prepare(f.input(), f.journal.clone())
        .await
        .err()
        .expect("successor requires its own grant");
    assert_eq!(error.to_string(), "project_trust_required");
    f.assert_no_open_or_effect().await;
    let calls = f.host.calls.lock().await;
    assert_eq!(
        (calls.inspections, calls.connections, calls.initializations),
        (0, 0, 0)
    );
    drop(calls);
    assert!(f.trust.check(&f.successor_project).await.is_err());
    assert_eq!(
        f.trust.check(&f.source_project).await.unwrap(),
        f.source_grant
    );
    f.close().await;
}

#[tokio::test]
async fn cf24_native_consent_denial_holds_before_open_without_fallback() {
    let f = Fixture::new().await;
    f.trust
        .grant(&f.successor_project, "successor-operator")
        .unwrap();
    f.host.calls.lock().await.consent_denied = true;
    let input = f.input();
    let request = f.request(input.invocation_id);
    let error = f
        .runtime
        .prepare(input, f.journal.clone())
        .await
        .err()
        .expect("external consent denial must propagate");
    assert_eq!(error.to_string(), "fixture_native_consent_required");
    assert!(f.runtime.begin_prompt(&request).is_err());
    f.assert_no_open_or_effect().await;
    let calls = f.host.calls.lock().await;
    assert_eq!(
        (calls.connections, calls.initializations),
        (1, 1),
        "no retry/fallback peer after denial"
    );
    drop(calls);
    f.close().await;
}

#[tokio::test]
async fn cf18_exact_successor_grant_admits_carried_root_and_read_not_source_or_main() {
    let f = Fixture::new().await;
    let input = f.input();
    assert!(!f.agent.worktree_write_enabled);
    assert_eq!(input.root.kind, ExecutionRootKind::Worktree);
    assert_eq!(Path::new(&input.root.effective), f.checkout);
    assert_ne!(Path::new(&input.root.effective), f.main);
    assert_ne!(Path::new(&input.root.effective), f.source);
    assert_eq!(
        Path::new(&f.successor_project.key().canonical_path),
        f.checkout.join(PROJECT)
    );
    let grant = f
        .trust
        .grant(&f.successor_project, "successor-operator")
        .unwrap();
    assert_ne!(grant, f.source_grant);
    assert_eq!(f.trust.check(&f.successor_project).await.unwrap(), grant);
    // Diverge the historical source after verified activation so a wrong read root
    // cannot pass merely because the carried files started with identical bytes.
    fs::write(f.source.join(SENTINEL), "historical source after carry\n").unwrap();
    let request = f.request(input.invocation_id);
    let prepared = f.runtime.prepare(input, f.journal.clone()).await.unwrap();
    for wrong_root in [&f.source, &f.main] {
        let mut wrong = request.clone();
        wrong.worktree_root = Some(wrong_root.to_str().unwrap().into());
        assert!(
            f.runtime.commit(&prepared, &wrong).is_err(),
            "foreign checkout must not bind prepared authority"
        );
    }
    f.runtime.commit(&prepared, &request).unwrap();
    let active = f.runtime.begin_prompt(&request).unwrap();
    let read = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"XcodeRead","arguments":{"filePath":SENTINEL}}});
    let response = f
        .runtime
        .forward(request.agent_execution_id.unwrap(), read.clone())
        .await
        .unwrap();
    assert_eq!(
        response["result"]["structuredContent"]["content"], "1\tcarried successor",
        "{response:#}"
    );
    assert_eq!(
        f.host.calls.lock().await.opens,
        [f.checkout.join(PROJECT).to_str().unwrap()]
    );
    let intents: Vec<(String, String)> =
        sqlx::query_as("SELECT state,intent_json FROM xcode_effect_attempts ORDER BY sequence")
            .fetch_all(&f.pool)
            .await
            .unwrap();
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].0, "succeeded");
    let intent: Value = serde_json::from_str(&intents[0].1).unwrap();
    assert_eq!(intent["run_id"], f.run.id.to_string());
    assert_eq!(
        intent["invocation_id"],
        request.agent_execution_id.unwrap().to_string()
    );
    assert_eq!(intent["operation"], "workspace_open");
    assert_eq!(
        intent["project_key"],
        serde_json::to_value(f.successor_project.key()).unwrap()
    );
    let holds: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(holds, 0);
    assert_eq!(
        f.trust.check(&f.source_project).await.unwrap(),
        f.source_grant
    );
    f.trust
        .revoke(&f.successor_project, "successor-operator")
        .unwrap();
    assert!(f
        .runtime
        .forward(request.agent_execution_id.unwrap(), read)
        .await
        .is_err());
    assert_eq!(f.host.calls.lock().await.reads.len(), 1);
    drop((active, prepared));
    f.close().await;
}

#[tokio::test]
async fn cf18_missing_carried_checkout_never_falls_back_to_trusted_main() {
    let f = Fixture::new().await;
    let main_root = resolve_execution_root(f.main.to_str().unwrap(), None, false, None).unwrap();
    let main_project =
        TrustedProject::resolve(main_root, Some(PROJECT), acp::current_process_uid()).unwrap();
    f.trust.grant(&main_project, "main-operator").unwrap();
    f.trust
        .grant(&f.successor_project, "successor-operator")
        .unwrap();
    let stale_input = f.input();
    let error = resolve_execution_root(
        &f.run.workspace_root,
        None,
        f.agent.worktree_write_enabled,
        f.agent.worktree_strategy.as_deref(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("missing_required_worktree"));
    fs::rename(&f.checkout, f.checkout.with_extension("unavailable")).unwrap();
    assert!(resolve_execution_root(
        &f.run.workspace_root,
        f.run.worktree_root.as_deref(),
        f.agent.worktree_write_enabled,
        f.agent.worktree_strategy.as_deref()
    )
    .is_err());
    assert!(
        f.runtime
            .prepare(stale_input, f.journal.clone())
            .await
            .is_err(),
        "already-resolved input must revalidate the missing successor"
    );
    f.assert_no_open_or_effect().await;
    let calls = f.host.calls.lock().await;
    assert_eq!((calls.inspections, calls.connections), (0, 0));
    drop(calls);
    f.close().await;
}
