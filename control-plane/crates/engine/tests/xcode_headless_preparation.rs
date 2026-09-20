use std::{sync::Arc, time::Duration};

use acp::{
    execution_root::resolve_execution_root,
    xcode_headless::{
        HeadlessPeer, HeadlessPeerFactory, HeadlessWorkspaceController, WorkspaceOpenRequest,
    },
    xcode_headless_host::{HeadlessHostInspector, HostSnapshot, ServiceGeneration, TrustedProject},
    XcodeDispatchDecision, XcodeEffectJournal, XcodeJournalError, XcodeJournalResult,
};
use anyhow::Result;
use db::writer::DbWriter;
use domain::xcode_effect::{
    AttemptRevision, Completion, HistoricalResult, NormalizedIntent, StoredAttempt,
};
use engine::xcode_effect_journal::DbXcodeEffectJournal;
use serde_json::{json, Value};
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

#[derive(Default)]
struct Calls {
    cold: bool,
    startup_error: bool,
    startup_installation_drift: bool,
    starts: usize,
    opens: usize,
    reads: Vec<Value>,
    error: bool,
    wrong_mapping: bool,
    pause: bool,
    schema_error: bool,
    pause_init: bool,
    corrupt_read: bool,
    hide_opened: bool,
    pause_inspection: bool,
    pause_read: bool,
}

struct Apple {
    host: Mutex<HostSnapshot>,
    calls: Mutex<Calls>,
    sent: Notify,
    resume: Notify,
    pool: sqlx::SqlitePool,
}

#[async_trait::async_trait]
impl HeadlessHostInspector for Apple {
    async fn prepare_service_startup(
        &self,
    ) -> Result<Option<acp::xcode_headless_host::HeadlessServiceStartupPlan>> {
        Ok(if self.calls.lock().await.cold {
            Some(
                acp::xcode_headless_host::HeadlessServiceStartupPlan::from_fixture_snapshot(
                    &*self.host.lock().await,
                ),
            )
        } else {
            None
        })
    }

    async fn start_service_after_fence(
        &self,
        _plan: acp::xcode_headless_host::HeadlessServiceStartupPlan,
    ) -> Result<()> {
        let held: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&self.pool)
            .await?;
        anyhow::ensure!(held == 1, "fixture_start_before_fence");
        let mut calls = self.calls.lock().await;
        calls.starts += 1;
        anyhow::ensure!(!calls.startup_error, "fixture_start_response_lost");
        calls.cold = false;
        if calls.startup_installation_drift {
            self.host.lock().await.generation.developer_dir =
                "/Other/Xcode.app/Contents/Developer".into();
        }
        Ok(())
    }

    async fn inspect(&self) -> Result<HostSnapshot> {
        anyhow::ensure!(!self.calls.lock().await.cold, "fixture_service_absent");
        if self.calls.lock().await.pause_inspection {
            self.sent.notify_one();
            self.resume.notified().await;
        }
        Ok(self.host.lock().await.clone())
    }
}

#[tokio::test]
async fn cold_start_is_fenced_with_open_and_returns_an_observed_binding() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.cold = true;
    let prepared = f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    assert_eq!(f.apple.calls.lock().await.starts, 1);
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert_eq!(prepared.host_snapshot().generation.pid, 42);
    assert_eq!(f.states().await, ["succeeded"]);
    assert_eq!(f.holds().await, 0);
    drop(prepared);
    f.close().await;
}

#[tokio::test]
async fn cold_start_unknown_is_not_repeated_with_a_fresh_operation_key() {
    let f = Fixture::new().await;
    {
        let mut calls = f.apple.calls.lock().await;
        calls.cold = true;
        calls.startup_error = true;
    }
    assert!(f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    let mut next = f.request.clone();
    next.invocation_id = Uuid::new_v4();
    next.operation_key = Uuid::new_v4();
    assert!(f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), next, f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.starts, 1);
    assert_eq!(f.apple.calls.lock().await.opens, 0);
    assert_eq!(f.states().await, ["unknown"]);
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn cold_start_installation_drift_never_opens_on_another_service() {
    let f = Fixture::new().await;
    {
        let mut calls = f.apple.calls.lock().await;
        calls.cold = true;
        calls.startup_installation_drift = true;
    }
    assert!(f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.starts, 1);
    assert_eq!(f.apple.calls.lock().await.opens, 0);
    assert_eq!(f.states().await, ["unknown"]);
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

struct Peers(Arc<Apple>);
struct Peer(Arc<Apple>);

struct LostDispatchAck(Arc<dyn XcodeEffectJournal>);

#[async_trait::async_trait]
impl XcodeEffectJournal for LostDispatchAck {
    async fn prepare(&self, intent: &NormalizedIntent) -> XcodeJournalResult<StoredAttempt> {
        self.0.prepare(intent).await
    }
    async fn dispatch(
        &self,
        nonce: Uuid,
        digest: &str,
        revision: i64,
    ) -> XcodeJournalResult<XcodeDispatchDecision> {
        self.0.dispatch(nonce, digest, revision).await?;
        Err(XcodeJournalError::StorageUnavailable)
    }
    async fn complete(
        &self,
        attempt: AttemptRevision,
        completion: &Completion,
    ) -> XcodeJournalResult<StoredAttempt> {
        self.0.complete(attempt, completion).await
    }
    async fn cancel(
        &self,
        attempt: AttemptRevision,
        outcome: &HistoricalResult,
    ) -> XcodeJournalResult<StoredAttempt> {
        self.0.cancel(attempt, outcome).await
    }
    async fn get(&self, attempt_id: Uuid) -> XcodeJournalResult<Option<StoredAttempt>> {
        self.0.get(attempt_id).await
    }
}

#[async_trait::async_trait]
impl HeadlessPeerFactory for Peers {
    async fn connect(
        &self,
        _host: &HostSnapshot,
        _deadline: tokio::time::Instant,
    ) -> Result<Box<dyn HeadlessPeer>> {
        Ok(Box::new(Peer(self.0.clone())))
    }
}

#[async_trait::async_trait]
impl HeadlessPeer for Peer {
    async fn initialize(&mut self) -> Result<()> {
        anyhow::ensure!(
            !self.0.calls.lock().await.schema_error,
            "fixture_schema_drift"
        );
        if self.0.calls.lock().await.pause_init {
            self.0.sent.notify_one();
            self.0.resume.notified().await;
        }
        Ok(())
    }

    async fn open(&mut self, path: &str) -> Result<Value> {
        // Count actual outbound effects, and inspect the real durable fence at send.
        let held: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&self.0.pool)
            .await?;
        anyhow::ensure!(held == 1, "fixture_send_before_fence");
        let (error, wrong, pause) = {
            let mut calls = self.0.calls.lock().await;
            calls.opens += 1;
            (calls.error, calls.wrong_mapping, calls.pause)
        };
        self.0.sent.notify_one();
        if pause {
            self.0.resume.notified().await;
        }
        anyhow::ensure!(!error, "fixture_response_lost");
        if !self.0.calls.lock().await.hide_opened {
            let mut host = self.0.host.lock().await;
            if !host.open_projects.contains(&path.into()) {
                host.open_projects.push(path.into());
            }
        }
        Ok(json!({"isError":false,"structuredContent":{
            "workspaceIdentifier":"workspace-fixture", "workspacePath":if wrong {"/foreign/Other.xcodeproj"} else {path}
        }}))
    }

    async fn read(&mut self, arguments: Value) -> Result<Value> {
        let corrupt = {
            let mut calls = self.0.calls.lock().await;
            calls.reads.push(arguments.clone());
            calls.corrupt_read
        };
        if self.0.calls.lock().await.pause_read {
            self.0.sent.notify_one();
            self.0.resume.notified().await;
        }
        Ok(json!({"isError":false,"structuredContent":{
            "content":"1\tknown", "filePath":if corrupt {json!("Foreign/Secret.txt")} else {arguments["filePath"].clone()}, "fileSize":5,
            "totalLines":1,"linesRead":1,"startLine":1
        }}))
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    pool: sqlx::SqlitePool,
    writer: Arc<DbWriter>,
    journal: Arc<dyn XcodeEffectJournal>,
    apple: Arc<Apple>,
    controller: Arc<HeadlessWorkspaceController>,
    project: Arc<TrustedProject>,
    request: WorkspaceOpenRequest,
}

struct Trust;
#[async_trait::async_trait]
impl acp::xcode_headless_runtime::HeadlessProjectTrust for Trust {
    async fn check(&self, _project: &TrustedProject) -> Result<String> {
        Ok("b".repeat(64))
    }
}

#[tokio::test]
async fn runtime_gate_uses_same_authority_and_journal_and_never_replays() {
    use acp::XcodeShimDispatchAuthority;
    let f = Fixture::new().await;
    let root = std::path::Path::new(&f.project.root().effective);
    std::fs::create_dir(root.join("scripts")).unwrap();
    std::fs::write(
        root.join("scripts/test-gate.sh"),
        "#!/bin/bash\nprintf x >> gate-count\nprintf fixture-output\n",
    )
    .unwrap();
    let runtime = f.runtime();
    let mut input = f.preparation_input();
    input.permission_policy["xcode_headless"]["gates"] = json!(["build"]);
    let prepared = runtime.prepare(input, f.journal.clone()).await.unwrap();
    let req = f.execution_request();
    runtime.commit(&prepared, &req).unwrap();
    let active = runtime.begin_prompt(&req).unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    let peer = acp::XcodeShimProcessBinding {
        pid: std::process::id(),
        uid: acp::current_process_uid(),
        parent_pid: None,
        ancestor_pids: vec![],
        start_time_fingerprint: None,
        executable_fingerprint: None,
    };
    let gate = acp::XcodeShimDispatchRequest {
        agent_execution_id: req.agent_execution_id,
        grant: acp::XcodeShimDispatchGrant::new(
            "fixture",
            "secret",
            "lease",
            peer.clone(),
            now - 1000,
            now + 60000,
        ),
        attempt: acp::XcodeShimDispatchAttempt {
            token_id: "fixture".into(),
            token_secret: "secret".into(),
            peer_process: peer,
            now_epoch_ms: now,
            active_prompt: true,
        },
        plan_input: acp::XcodeHostExecutorPlanInput {
            invoked_tool: "chainworks-test-gate".into(),
            args: vec!["build".into()],
            cwd: root.to_string_lossy().into_owned(),
            workspace_root: root.to_string_lossy().into_owned(),
            provider_env: Default::default(),
            simulator_candidates: vec![],
            toolchain_mapping_root: None,
        },
    };
    let key = Uuid::new_v4();
    let outcome = runtime
        .dispatch(
            req.agent_execution_id.unwrap(),
            key,
            gate.clone(),
            &acp::XcodeHostExecutorProcessConfig::default(),
            &acp::NoopXcodeRuntimeObservationSink,
        )
        .await
        .unwrap();
    assert_eq!(outcome.exit_status, 0);
    assert_eq!(outcome.process_output.unwrap().stdout, "fixture-output");
    assert_eq!(f.states().await, ["succeeded", "succeeded"]);
    assert_eq!(f.holds().await, 0);
    assert!(runtime
        .dispatch(
            req.agent_execution_id.unwrap(),
            key,
            gate,
            &acp::XcodeHostExecutorProcessConfig::default(),
            &acp::NoopXcodeRuntimeObservationSink
        )
        .await
        .is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("gate-count")).unwrap(),
        "x"
    );
    drop((active, prepared, runtime));
    f.close().await;
}
struct Holds(sqlx::SqlitePool);
#[async_trait::async_trait]
impl acp::xcode_coordinator::ProjectHoldCheck for Holds {
    async fn is_held(
        &self,
        project: &domain::xcode_effect::ProjectKey,
    ) -> std::result::Result<bool, acp::xcode_coordinator::CoordinatorError> {
        engine::xcode_effect_journal::DbXcodeProjectHoldCheck(self.0.clone())
            .is_held(project)
            .await
    }
}

impl Fixture {
    fn runtime(&self) -> Arc<acp::xcode_headless_runtime::HeadlessRuntime> {
        use acp::xcode_coordinator::*;
        let coordinator = WorkspaceAccessCoordinator::new(
            CoordinatorLimits {
                queue_capacity: 8,
                queue_timeout: Duration::from_secs(1),
            },
            Arc::new(Holds(self.pool.clone())),
        )
        .unwrap();
        let authority = FixtureJournalAuthority::open(
            &self._dir.path().canonicalize().unwrap().join("authority"),
            &self._dir.path().join("db.sqlite"),
        )
        .unwrap();
        acp::xcode_headless_runtime::HeadlessRuntime::new_fixture(
            self.controller.clone(),
            coordinator,
            Arc::new(Trust),
            authority,
        )
    }
    fn preparation_input(&self) -> acp::xcode_headless_runtime::HeadlessPreparationInput {
        acp::xcode_headless_runtime::HeadlessPreparationInput {
            run_id: self.request.run_id.into(),
            invocation_id: self.request.invocation_id.into(),
            owner_lineage: self.request.owner_lineage.clone(),
            root: self.project.root().clone(),
            project_selector: None,
            permission_policy: json!({"xcode_headless":{"read":true,"gates":[]}}),
            timeout: Duration::from_secs(10),
        }
    }
    fn execution_request(&self) -> acp::ExecutionRequest {
        serde_json::from_value(json!({"run_id":self.request.run_id,"agent_execution_id":self.request.invocation_id,
            "stage_id":"s","agent_id":"a","provider":"fixture","workspace_root":self.project.root().effective,
            "prompt":"read","session_generation_id":"fixture-generation"})).unwrap()
    }
}

#[tokio::test]
async fn runtime_requires_commit_and_active_prompt_and_revokes_at_prompt_end() {
    let f = Fixture::new().await;
    let runtime = f.runtime();
    let prepared = runtime
        .prepare(f.preparation_input(), f.journal.clone())
        .await
        .unwrap();
    let req = f.execution_request();
    let read = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"XcodeRead","arguments":{"filePath":"Fixture/Sentinel.txt"}}});
    assert!(runtime.begin_prompt(&req).is_err());
    assert!(runtime
        .forward(req.agent_execution_id.unwrap(), read.clone())
        .await
        .is_err());
    runtime.commit(&prepared, &req).unwrap();
    assert!(runtime
        .forward(req.agent_execution_id.unwrap(), read.clone())
        .await
        .is_err());
    let active = runtime.begin_prompt(&req).unwrap();
    assert!(runtime.begin_prompt(&req).is_err());
    assert_eq!(
        runtime
            .forward(req.agent_execution_id.unwrap(), read.clone())
            .await
            .unwrap()["result"]["structuredContent"]["content"],
        "1\tknown"
    );
    drop(active);
    assert!(runtime
        .forward(req.agent_execution_id.unwrap(), read)
        .await
        .is_err());
    assert!(runtime.begin_prompt(&req).is_err());
    assert_eq!(f.apple.calls.lock().await.reads.len(), 1);
    drop(prepared);
    drop(runtime);
    f.close().await;
}

#[tokio::test]
async fn runtime_commit_rejects_foreign_invocation_root_and_run() {
    let f = Fixture::new().await;
    let runtime = f.runtime();
    let prepared = runtime
        .prepare(f.preparation_input(), f.journal.clone())
        .await
        .unwrap();
    let req = f.execution_request();
    let mut wrong = req.clone();
    wrong.run_id = domain::ids::RunId::new();
    assert!(runtime.commit(&prepared, &wrong).is_err());
    wrong = req.clone();
    wrong.agent_execution_id = Some(domain::ids::AgentExecutionId::new());
    assert!(runtime.commit(&prepared, &wrong).is_err());
    wrong = req.clone();
    wrong.workspace_root = f._dir.path().to_string_lossy().into_owned();
    assert!(runtime.commit(&prepared, &wrong).is_err());
    runtime.commit(&prepared, &req).unwrap();
    wrong = req.clone();
    wrong.session_generation_id = Some("foreign-generation".into());
    assert!(runtime.begin_prompt(&wrong).is_err());
    drop(prepared);
    assert!(runtime.begin_prompt(&req).is_err());
    drop(runtime);
    f.close().await;
}

impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let package = root.join("Fixture.xcodeproj");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("project.pbxproj"), "// fixture").unwrap();
        let root = resolve_execution_root(root.to_str().unwrap(), None, false, None).unwrap();
        let project =
            Arc::new(TrustedProject::resolve(root, None, acp::current_process_uid()).unwrap());
        let pool = db::pool::create_pool(&format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("db.sqlite").display()
        ))
        .await
        .unwrap();
        let writer = Arc::new(DbWriter::new(pool.clone()));
        let request = WorkspaceOpenRequest {
            run_id: Uuid::new_v4(),
            owner_lineage: "fixture-owner".into(),
            invocation_id: Uuid::new_v4(),
            operation_key: Uuid::new_v4(),
            permission_policy_digest: "a".repeat(64),
            trust_policy_id: "trusted_local_project_v1".into(),
            timeout: Duration::from_secs(10),
        };
        let journal = Arc::new(DbXcodeEffectJournal::new(
            pool.clone(),
            writer.clone(),
            request.run_id,
            request.owner_lineage.clone(),
        ));
        let apple = Arc::new(Apple {
            host: Mutex::new(HostSnapshot {
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
            calls: Mutex::new(Calls::default()),
            sent: Notify::new(),
            resume: Notify::new(),
            pool: pool.clone(),
        });
        let controller = Arc::new(HeadlessWorkspaceController::new(
            apple.clone(),
            Arc::new(Peers(apple.clone())),
        ));
        Self {
            _dir: dir,
            pool,
            writer,
            journal,
            apple,
            controller,
            project,
            request,
        }
    }

    async fn states(&self) -> Vec<String> {
        sqlx::query_scalar("SELECT state FROM xcode_effect_attempts ORDER BY sequence")
            .fetch_all(&self.pool)
            .await
            .unwrap()
    }

    async fn holds(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn close(self) {
        self.controller.shutdown().await;
        self.writer.shutdown().await;
        self.pool.close().await;
    }
}

#[tokio::test]
async fn new_invocation_revalidates_live_mapping_without_reopening() {
    let f = Fixture::new().await;
    let first = f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    let mut next = f.request.clone();
    next.invocation_id = Uuid::new_v4();
    next.operation_key = Uuid::new_v4();
    let second = f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), next, f.journal.clone())
        .await
        .unwrap();
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert_eq!(first.binding_digest(), second.binding_digest());
    assert_eq!(first.alias(), second.alias());
    assert_eq!(f.states().await, ["succeeded"]);
    drop((first, second));
    f.close().await;
}

#[tokio::test]
async fn revalidation_observing_close_revokes_cached_mapping_even_after_reopen() {
    let f = Fixture::new().await;
    let mut first = f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    f.apple.host.lock().await.open_projects.clear();
    assert!(first.revalidate().await.is_err());
    f.apple
        .host
        .lock()
        .await
        .open_projects
        .push(f.project.key().canonical_path.clone().into());
    let mut next = f.request.clone();
    next.invocation_id = Uuid::new_v4();
    next.operation_key = Uuid::new_v4();
    assert!(f
        .controller
        .prepare_reusing_verified_mapping(f.project.clone(), next, f.journal.clone())
        .await
        .is_err());
    assert!(first
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert!(f.apple.calls.lock().await.reads.is_empty());
    drop(first);
    f.close().await;
}

#[tokio::test]
async fn active_invocation_can_read_after_preparation_budget_without_renewing_authority() {
    let f = Fixture::new().await;
    let runtime = f.runtime();
    let mut input = f.preparation_input();
    input.timeout = Duration::from_secs(3600);
    let prepared = runtime.prepare(input, f.journal.clone()).await.unwrap();
    let req = f.execution_request();
    runtime.commit(&prepared, &req).unwrap();
    let active = runtime.begin_prompt(&req).unwrap();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(721)).await;
    tokio::time::resume();
    runtime.forward(req.agent_execution_id.unwrap(),json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"XcodeRead","arguments":{"filePath":"Fixture/Sentinel.txt"}}})).await.unwrap();
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert_eq!(f.apple.calls.lock().await.reads.len(), 1);
    drop((active, prepared, runtime));
    f.close().await;
}

#[tokio::test]
async fn exact_open_is_fenced_and_settled_before_scoped_read() {
    let f = Fixture::new().await;
    let mut prepared = f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    assert_eq!(f.states().await, ["succeeded"]);
    assert_eq!(f.holds().await, 0);
    assert_eq!(prepared.binding_digest().len(), 64);
    assert_ne!(prepared.alias(), "workspace-fixture");
    let value = prepared
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .unwrap();
    assert_eq!(value["content"], "1\tknown");
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert_eq!(
        f.apple.calls.lock().await.reads[0]["workspaceIdentifier"],
        "workspace-fixture"
    );
    assert!(prepared
        .read(json!({"filePath":"Fixture/Sentinel.txt","workspaceIdentifier":"workspace-foreign"}))
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.reads.len(), 1);
    drop(prepared);
    f.close().await;
}

#[tokio::test]
async fn repeated_terminal_operation_does_not_reopen_or_reconstruct_a_ticket() {
    let f = Fixture::new().await;
    drop(
        f.controller
            .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
            .await
            .unwrap(),
    );
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert_eq!(f.states().await, ["succeeded"]);
    f.close().await;
}

#[tokio::test]
async fn lost_open_response_preserves_hold_and_blocks_same_and_new_keys() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.error = true;
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.states().await, ["unknown"]);
    assert_eq!(f.holds().await, 1);
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    let mut another = f.request.clone();
    another.operation_key = Uuid::new_v4();
    assert!(f
        .controller
        .prepare(f.project.clone(), another, f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    f.close().await;
}

#[tokio::test]
async fn incorrect_open_mapping_cannot_create_a_binding_or_clear_hold() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.wrong_mapping = true;
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.states().await, ["unknown"]);
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn journal_failure_before_fence_sends_no_open() {
    let f = Fixture::new().await;
    f.writer.shutdown().await;
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.opens, 0);
    assert!(f.states().await.is_empty());
    f.close().await;
}

#[tokio::test]
async fn incompatible_schema_and_unaccepted_trust_send_no_open() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.schema_error = true;
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    f.apple.calls.lock().await.schema_error = false;
    let mut untrusted = f.request.clone();
    untrusted.trust_policy_id = "untrusted".into();
    assert!(f
        .controller
        .prepare(f.project.clone(), untrusted, f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.opens, 0);
    assert!(f.states().await.is_empty());
    f.close().await;
}

#[tokio::test]
async fn outcome_commit_failure_never_returns_binding_or_replays_open() {
    let f = Fixture::new().await;
    sqlx::raw_sql("CREATE TRIGGER fixture_reject_completion BEFORE UPDATE ON xcode_effect_attempts WHEN NEW.state = 'succeeded' BEGIN SELECT RAISE(ABORT, 'fixture'); END;")
        .execute(&f.pool).await.unwrap();
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.states().await, ["dispatched"]);
    assert_eq!(f.holds().await, 1);
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    f.close().await;
}

#[tokio::test]
async fn outcome_commit_failure_revokes_preexisting_bindings_before_releasing_access() {
    let f = Fixture::new().await;
    let mut first = f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fixture_reject_completion BEFORE UPDATE ON xcode_effect_attempts WHEN NEW.state = 'succeeded' BEGIN SELECT RAISE(ABORT, 'fixture'); END;")
        .execute(&f.pool).await.unwrap();
    let mut request = f.request.clone();
    request.operation_key = Uuid::new_v4();
    assert!(f
        .controller
        .prepare(f.project.clone(), request, f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.states().await, ["succeeded", "dispatched"]);
    assert_eq!(f.holds().await, 1);
    assert_eq!(f.apple.calls.lock().await.opens, 2);
    assert!(first
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    assert!(f.apple.calls.lock().await.reads.is_empty());
    drop(first);
    f.close().await;
}

#[tokio::test]
async fn lost_dispatch_ack_revokes_preexisting_bindings_without_sending_open() {
    let f = Fixture::new().await;
    let mut first = f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    let mut request = f.request.clone();
    request.operation_key = Uuid::new_v4();
    assert!(f
        .controller
        .prepare(
            f.project.clone(),
            request,
            Arc::new(LostDispatchAck(f.journal.clone()))
        )
        .await
        .is_err());
    assert_eq!(f.states().await, ["succeeded", "dispatched"]);
    assert_eq!(f.holds().await, 1);
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert!(first
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    assert!(f.apple.calls.lock().await.reads.is_empty());
    drop(first);
    f.close().await;
}

#[tokio::test]
async fn cancelled_waiter_does_not_cancel_outcome_persistence() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.pause = true;
    let (controller, project, request, journal) = (
        f.controller.clone(),
        f.project.clone(),
        f.request.clone(),
        f.journal.clone(),
    );
    let waiter = tokio::spawn(async move { controller.prepare(project, request, journal).await });
    tokio::time::timeout(Duration::from_secs(10), f.apple.sent.notified())
        .await
        .unwrap();
    waiter.abort();
    assert!(waiter.await.is_err_and(|error| error.is_cancelled()));
    f.apple.resume.notify_one();
    f.controller.shutdown().await;
    assert_eq!(f.states().await, ["succeeded"]);
    assert_eq!(f.holds().await, 0);
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    f.close().await;
}

#[tokio::test]
async fn service_restart_or_project_disappearance_prevents_read_and_reopen() {
    for restart in [false, true] {
        let f = Fixture::new().await;
        let mut prepared = f
            .controller
            .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
            .await
            .unwrap();
        if restart {
            f.apple.host.lock().await.generation.start_usec += 1;
        } else {
            f.apple.host.lock().await.open_projects.clear();
        }
        assert!(prepared
            .read(json!({"filePath":"Fixture/Sentinel.txt"}))
            .await
            .is_err());
        assert!(f.apple.calls.lock().await.reads.is_empty());
        assert_eq!(f.apple.calls.lock().await.opens, 1);
        drop(prepared);
        f.close().await;
    }
}

#[tokio::test]
async fn cancelled_waiter_before_dispatch_sends_no_open() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.pause_init = true;
    let (controller, project, request, journal) = (
        f.controller.clone(),
        f.project.clone(),
        f.request.clone(),
        f.journal.clone(),
    );
    let waiter = tokio::spawn(async move { controller.prepare(project, request, journal).await });
    tokio::time::timeout(Duration::from_secs(10), f.apple.sent.notified())
        .await
        .unwrap();
    waiter.abort();
    assert!(waiter.await.is_err_and(|error| error.is_cancelled()));
    f.apple.resume.notify_one();
    f.controller.shutdown().await;
    assert_eq!(f.apple.calls.lock().await.opens, 0);
    assert_eq!(f.holds().await, 0);
    f.close().await;
}

#[tokio::test]
async fn timeout_after_dispatch_is_unknown_and_shutdown_drains_the_driver() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.pause = true;
    let mut request = f.request.clone();
    request.timeout = Duration::from_millis(250);
    assert!(f
        .controller
        .prepare(f.project.clone(), request, f.journal.clone())
        .await
        .is_err());
    f.controller.shutdown().await;
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    assert_eq!(f.states().await, ["unknown"]);
    assert_eq!(f.holds().await, 1);
    assert!(f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.apple.calls.lock().await.opens, 1);
    f.close().await;
}

#[tokio::test]
async fn changed_filesystem_identity_prevents_read_and_malformed_read_revokes_binding() {
    for replace in [false, true] {
        let f = Fixture::new().await;
        let mut prepared = f
            .controller
            .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
            .await
            .unwrap();
        if replace {
            let path = std::path::Path::new(&f.project.key().canonical_path);
            std::fs::rename(path, path.with_extension("old")).unwrap();
            std::fs::create_dir(path).unwrap();
            std::fs::write(path.join("project.pbxproj"), "// replacement").unwrap();
        } else {
            f.apple.calls.lock().await.corrupt_read = true;
        }
        assert!(prepared
            .read(json!({"filePath":"Fixture/Sentinel.txt"}))
            .await
            .is_err());
        assert!(prepared
            .read(json!({"filePath":"Fixture/Sentinel.txt"}))
            .await
            .is_err());
        assert_eq!(
            f.apple.calls.lock().await.reads.len(),
            if replace { 0 } else { 1 }
        );
        assert_eq!(f.apple.calls.lock().await.opens, 1);
        drop(prepared);
        f.close().await;
    }
}

#[tokio::test]
async fn same_generation_cannot_assign_one_observed_workspace_id_to_two_projects() {
    let f = Fixture::new().await;
    let mut first = f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    let package = std::path::Path::new(&f.project.root().effective).join("Other.xcodeproj");
    std::fs::create_dir(&package).unwrap();
    std::fs::write(package.join("project.pbxproj"), "// another").unwrap();
    let other = Arc::new(
        TrustedProject::resolve(f.project.root().clone(), Some("Other.xcodeproj"), 501).unwrap(),
    );
    let mut request = f.request.clone();
    request.operation_key = Uuid::new_v4();
    assert!(f
        .controller
        .prepare(other, request, f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.states().await, ["succeeded", "unknown"]);
    assert_eq!(f.holds().await, 1);
    assert!(first
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    assert!(f.apple.calls.lock().await.reads.is_empty());
    drop(first);
    f.close().await;
}

#[tokio::test]
async fn observed_close_revokes_every_binding_even_after_same_generation_reopen() {
    let f = Fixture::new().await;
    let mut first = f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    let mut second_request = f.request.clone();
    second_request.operation_key = Uuid::new_v4();
    let mut second = f
        .controller
        .prepare(f.project.clone(), second_request, f.journal.clone())
        .await
        .unwrap();
    f.apple.host.lock().await.open_projects.clear();
    assert!(first
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    f.apple
        .host
        .lock()
        .await
        .open_projects
        .push(f.project.key().canonical_path.clone().into());
    assert!(second
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    assert!(f.apple.calls.lock().await.reads.is_empty());
    drop((first, second));
    f.close().await;
}

#[tokio::test]
async fn conflicting_id_with_failed_post_open_status_still_revokes_old_binding() {
    let f = Fixture::new().await;
    let mut first = f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    let package = std::path::Path::new(&f.project.root().effective).join("Other.xcodeproj");
    std::fs::create_dir(&package).unwrap();
    std::fs::write(package.join("project.pbxproj"), "// another").unwrap();
    let other = Arc::new(
        TrustedProject::resolve(f.project.root().clone(), Some("Other.xcodeproj"), 501).unwrap(),
    );
    let mut request = f.request.clone();
    request.operation_key = Uuid::new_v4();
    f.apple.calls.lock().await.hide_opened = true;
    assert!(f
        .controller
        .prepare(other, request, f.journal.clone())
        .await
        .is_err());
    assert_eq!(f.states().await, ["succeeded", "unknown"]);
    assert!(first
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    assert!(f.apple.calls.lock().await.reads.is_empty());
    drop(first);
    f.close().await;
}

#[tokio::test]
async fn preparation_capacity_rejects_excess_waiters_immediately() {
    let f = Fixture::new().await;
    f.apple.calls.lock().await.pause_init = true;
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let (controller, project, request, journal) = (
            f.controller.clone(),
            f.project.clone(),
            f.request.clone(),
            f.journal.clone(),
        );
        tasks.push(tokio::spawn(async move {
            controller.prepare(project, request, journal).await
        }));
    }
    tokio::time::timeout(Duration::from_secs(10), f.apple.sent.notified())
        .await
        .unwrap();
    for _ in 0..32 {
        tokio::task::yield_now().await;
    }
    let excess = tokio::time::timeout(
        Duration::from_millis(100),
        f.controller
            .prepare(f.project.clone(), f.request.clone(), f.journal.clone()),
    )
    .await;
    assert!(
        excess.is_ok_and(|result| result.is_err()),
        "excess admission must reject without queueing"
    );
    for task in tasks {
        task.abort();
        let _ = task.await;
    }
    f.apple.resume.notify_one();
    f.controller.shutdown().await;
    assert_eq!(f.apple.calls.lock().await.opens, 0);
    f.close().await;
}

#[tokio::test]
async fn cancelling_queued_read_poisons_only_that_binding_without_losing_mapping() {
    let f = Fixture::new().await;
    let mut first = f
        .controller
        .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
        .await
        .unwrap();
    let mut request = f.request.clone();
    request.operation_key = Uuid::new_v4();
    let mut second = f
        .controller
        .prepare(f.project.clone(), request, f.journal.clone())
        .await
        .unwrap();
    // Consume the preceding open notification before waiting for read inspection.
    f.apple.sent.notified().await;
    f.apple.calls.lock().await.pause_inspection = true;
    let active = tokio::spawn(async move {
        let result = first.read(json!({"filePath":"Fixture/Sentinel.txt"})).await;
        (first, result)
    });
    tokio::time::timeout(Duration::from_secs(5), f.apple.sent.notified())
        .await
        .unwrap();
    assert!(tokio::time::timeout(
        Duration::from_millis(25),
        second.read(json!({"filePath":"Fixture/Sentinel.txt"}))
    )
    .await
    .is_err());
    f.apple.calls.lock().await.pause_inspection = false;
    f.apple.resume.notify_one();
    let (mut first, result) = active.await.unwrap();
    assert!(result.is_ok());
    assert!(second
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_err());
    assert!(first
        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
        .await
        .is_ok());
    assert_eq!(f.apple.calls.lock().await.reads.len(), 2);
    drop((first, second));
    f.close().await;
}

#[tokio::test]
async fn cancelling_in_flight_read_revokes_all_shared_bindings() {
    for pause_inspection in [false, true] {
        let f = Fixture::new().await;
        let mut first = f
            .controller
            .prepare(f.project.clone(), f.request.clone(), f.journal.clone())
            .await
            .unwrap();
        let mut request = f.request.clone();
        request.operation_key = Uuid::new_v4();
        let mut second = f
            .controller
            .prepare(f.project.clone(), request, f.journal.clone())
            .await
            .unwrap();
        {
            let mut calls = f.apple.calls.lock().await;
            calls.pause_read = !pause_inspection;
            calls.pause_inspection = pause_inspection;
        }
        assert!(tokio::time::timeout(
            Duration::from_millis(25),
            first.read(json!({"filePath":"Fixture/Sentinel.txt"}))
        )
        .await
        .is_err());
        let expected_reads = usize::from(!pause_inspection);
        assert_eq!(f.apple.calls.lock().await.reads.len(), expected_reads);
        {
            let mut calls = f.apple.calls.lock().await;
            calls.pause_read = false;
            calls.pause_inspection = false;
        }
        assert!(second
            .read(json!({"filePath":"Fixture/Sentinel.txt"}))
            .await
            .is_err());
        assert_eq!(f.apple.calls.lock().await.reads.len(), expected_reads);
        drop((first, second));
        f.close().await;
    }
}

#[tokio::test]
async fn long_lived_binding_does_not_allow_an_unbounded_read_or_revalidation() {
    for revalidation in [false, true] {
        let f = Fixture::new().await;
        let mut workspace = f
            .controller
            .prepare_for_invocation(
                f.project.clone(),
                f.request.clone(),
                f.journal.clone(),
                tokio::time::Instant::now() + Duration::from_secs(3600),
            )
            .await
            .unwrap();
        f.apple.sent.notified().await;
        {
            let mut calls = f.apple.calls.lock().await;
            calls.pause_read = !revalidation;
            calls.pause_inspection = revalidation;
        }
        {
            let operation = async {
                if revalidation {
                    workspace.revalidate().await
                } else {
                    workspace
                        .read(json!({"filePath":"Fixture/Sentinel.txt"}))
                        .await
                        .map(|_| ())
                }
            };
            tokio::pin!(operation);
            tokio::select! {
                _ = f.apple.sent.notified() => {}
                _ = &mut operation => panic!("fixture operation should wait"),
            }
            tokio::time::pause();
            tokio::time::advance(Duration::from_secs(61)).await;
            let result = tokio::time::timeout(Duration::from_millis(1), &mut operation).await;
            tokio::time::resume();
            assert!(result.is_ok(), "single operation exceeded its budget");
            assert!(result.unwrap().is_err());
        }
        assert!(workspace
            .read(json!({"filePath":"Fixture/Sentinel.txt"}))
            .await
            .is_err());
        drop(workspace);
        f.close().await;
    }
}
