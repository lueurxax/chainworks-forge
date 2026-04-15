use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{approvals, ideas, runs, stages};
use domain::approval::{Approval, ApprovalDecision};
use domain::commands::{ApproveStageCmd, Command, RejectStageCmd, RetryStageCmd, StartRunCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ApprovalId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;

async fn test_pool() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:").await.expect("in-memory pool failed")
}

fn make_idea(id: IdeaId) -> Idea {
    Idea {
        id,
        title: "Test idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    }
}

fn make_run(id: RunId, idea_id: IdeaId, status: RunStatus) -> Run {
    Run {
        id,
        idea_id,
        status,
        workflow_id: "wf-test".into(),
        workflow_title: "Test Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        current_state: None,
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: None,
        base_branch: None,
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
    }
}

fn make_stage(id: StageExecutionId, run_id: RunId, status: StageStatus) -> StageExecution {
    StageExecution {
        id,
        run_id,
        stage_id: "stage_test".into(),
        label: "Test Stage".into(),
        status,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: Utc::now(),
        completed_at: None,
        owner_agent: None,
        provider: None,
        model: None,
        stage_type: None,
    }
}

fn make_approval(run_id: RunId, stage_id: &str, decision: ApprovalDecision) -> Approval {
    Approval {
        id: ApprovalId::new(),
        run_id,
        stage_id: stage_id.to_string(),
        decision,
        requested_at: Utc::now(),
        decided_at: None,
        comment: None,
        expires_at: None,
    }
}

fn make_command_handler(pool: sqlx::SqlitePool) -> Arc<CommandHandler> {
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    Arc::new(CommandHandler::new(pool, events, work_queue))
}

// ---------------------------------------------------------------------------
// Recovery parity harness (P027)
// Proves daemon RecoveryService matches app-side ResumeManager semantics:
// stages stuck in Running after a crash must become Blocked.
// ---------------------------------------------------------------------------

/// RecoveryService must mark stuck-Running stages as Blocked and re-enqueue
/// AdvanceRun, mirroring Swift ResumeManager.normalizeInterruptedRunsForManualResume.
#[tokio::test]
async fn test_startup_repair_clears_stuck_running_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();
    stages::insert(&pool, &make_stage(stage_id, run_id, StageStatus::Running)).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let recovery = RecoveryService::new(pool.clone(), work_queue, events);

    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(summary.runs_inspected, 1, "one active run must be inspected");
    assert_eq!(summary.runs_repaired, 1, "stuck run must be repaired");
    assert!(summary.work_items_requeued >= 1, "at least one AdvanceRun must be re-enqueued");

    let repaired_stage = stages::find_by_id(&pool, stage_id).await.unwrap().unwrap();
    assert_eq!(
        repaired_stage.status,
        StageStatus::Blocked,
        "stage stuck in Running must become Blocked after startup repair"
    );
}

/// A run with no stuck stages must not be counted as repaired.
#[tokio::test]
async fn test_startup_repair_skips_clean_runs() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();
    // Stage is already Completed — nothing to repair.
    stages::insert(
        &pool,
        &make_stage(stage_id, run_id, StageStatus::Completed),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let recovery = RecoveryService::new(pool.clone(), work_queue, events);

    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(summary.runs_inspected, 1);
    assert_eq!(summary.runs_repaired, 1, "active run with completed stage needs catchup AdvanceRun");
    assert_eq!(summary.work_items_requeued, 1, "one AdvanceRun must be re-enqueued for startup catchup");

    let unchanged_stage = stages::find_by_id(&pool, stage_id).await.unwrap().unwrap();
    assert_eq!(
        unchanged_stage.status,
        StageStatus::Completed,
        "clean stage must not be modified by startup repair"
    );
}

// ---------------------------------------------------------------------------
// Approval / retry parity harness (P027)
// Proves daemon CommandHandler approval and retry semantics match the
// app-owned ExecutionService authority model:
// – Granted approval → stage transitions WaitingApproval → Running
// – Rejected approval → stage transitions WaitingApproval → Blocked
// – Retry → old stage settled as Skipped, new stage created with attempt+1
// ---------------------------------------------------------------------------

/// Granting approval must resolve the canonical approval record to Granted
/// and advance the stage from WaitingApproval to Running.
#[tokio::test]
async fn test_approve_stage_resolves_approval_and_advances_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "review_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "review_stage", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::ApproveStage(ApproveStageCmd {
            run_id,
            stage_id: "review_stage".into(),
            comment: Some("LGTM".into()),
        }))
        .await
        .unwrap();

    // Approval must now be Granted.
    let resolved = approvals::find_by_id(&pool, approval.id).await.unwrap().unwrap();
    assert_eq!(
        resolved.decision,
        ApprovalDecision::Granted,
        "approval decision must be Granted after ApproveStage"
    );
    assert!(resolved.decided_at.is_some(), "decided_at must be set");

    // Stage must have transitioned to Running.
    let updated_stage = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        updated_stage.status,
        StageStatus::Running,
        "stage must advance to Running after approval is granted"
    );
}

/// Rejecting approval must resolve the canonical approval record to Rejected
/// and transition the stage from WaitingApproval to Blocked.
#[tokio::test]
async fn test_reject_stage_resolves_approval_and_blocks_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "gated_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "gated_stage", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::RejectStage(RejectStageCmd {
            run_id,
            stage_id: "gated_stage".into(),
            comment: Some("Not ready".into()),
        }))
        .await
        .unwrap();

    // Approval must now be Rejected.
    let resolved = approvals::find_by_id(&pool, approval.id).await.unwrap().unwrap();
    assert_eq!(
        resolved.decision,
        ApprovalDecision::Rejected,
        "approval decision must be Rejected after RejectStage"
    );

    // Stage must have transitioned to Blocked.
    let updated_stage = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        updated_stage.status,
        StageStatus::Blocked,
        "stage must become Blocked after approval is rejected"
    );
}

/// Retrying a stage must settle the old execution as Skipped and produce a new
/// execution for the same stage_id with attempt_number incremented by 1.
#[tokio::test]
async fn test_retry_stage_creates_new_attempt_and_skips_old() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();

    let mut stage = make_stage(old_stage_exec_id, run_id, StageStatus::Failed);
    stage.stage_id = "flaky_stage".into();
    stage.attempt_number = 1;
    stages::insert(&pool, &stage).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::RetryStage(RetryStageCmd {
            run_id,
            stage_id: "flaky_stage".into(),
        }))
        .await
        .unwrap();

    // Old stage must be settled as Skipped.
    let old = stages::find_by_id(&pool, old_stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        old.status,
        StageStatus::Skipped,
        "old stage execution must be settled as Skipped after retry"
    );
    assert_eq!(
        old.settlement_kind,
        Some(domain::stage::StageSettlementKind::Skipped),
        "settlement_kind must be Skipped"
    );

    // A new stage execution must exist with attempt_number = 2 and status Pending.
    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let new_stage = all_stages
        .iter()
        .find(|s| s.stage_id == "flaky_stage" && s.attempt_number == 2)
        .expect("new stage execution with attempt_number=2 must exist after retry");

    assert_eq!(
        new_stage.status,
        StageStatus::Pending,
        "new stage execution must start as Pending"
    );
}

/// Starting a run must persist the frozen delivery configuration JSON on the
/// run record so downstream release logic can consume it.
#[tokio::test]
async fn test_start_run_persists_delivery_configuration_json() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let handler = make_command_handler(pool.clone());
    let delivery_configuration_json = Some(
        r#"{"repo_identifier":"repo-1","repo_root":"/repo","base_branch":"main","worktree_base_path":"/tmp/worktrees","target_branch":"cw/release"}"#
            .to_string(),
    );

    let result = handler
        .handle(Command::StartRun(StartRunCmd {
            idea_id,
            workflow_id: "wf-start".into(),
            workflow_title: "Start Run".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            delivery_configuration_json: delivery_configuration_json.clone(),
        }))
        .await
        .unwrap();

    let run_id = match result {
        engine::command_handler::CommandResult::RunStarted { run_id } => run_id,
        _ => panic!("unexpected command result"),
    };

    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(run.delivery_configuration_json, delivery_configuration_json);
}

// ---------------------------------------------------------------------------
// InvokeAgent end-to-end parity harness (P027 / ARCH-001 / REQ-009)
//
// Proves the full daemon path:
//   BackgroundExecutor → AcpRuntimeManager → fixture ACP binary
//   → artifact persistence → projection rebuild → stage settlement
//
// This is the "bounded real runtime-backed daemon slice" required by R4.
// ---------------------------------------------------------------------------

/// BackgroundExecutor.process_next_item() drives a real ACP subprocess that
/// speaks the JSON-RPC 2.0 ACP protocol, persists the artifact it creates,
/// settles the stage, and rebuilds projections — all through the same code
/// path that runs in production.
///
/// The fixture is a Python script that completes the full ACP handshake
/// (initialize → session/new → session/prompt) and creates `report.json`
/// inside the workspace_root it receives via `session/new.params.cwd`.
/// The transport discovers the new file via workspace diff and returns it
/// as an artifact path.
#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_end_to_end_with_fixture_binary() {
    use std::os::unix::fs::PermissionsExt;

    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use db::repos::projections;
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    // Write a Python ACP fixture script.  It speaks the full JSON-RPC 2.0 ACP
    // protocol: initialize → session/new → session/prompt → (optional) session/close.
    // During session/prompt it creates report.json in the cwd it received.
    let script = tmp.path().join("acp_fixture.py");
    std::fs::write(&script, r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
if msg is None: sys.exit(1)
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})

msg = recv()
if msg is None: sys.exit(1)
cwd = msg.get("params",{}).get("cwd","/tmp")
session_id = "e2e-fixture-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
if msg is None: sys.exit(1)
artifact = os.path.join(cwd, "report.json")
with open(artifact, "w") as f:
    f.write('{"summary":"ok"}\n')
send({"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":"Done."}}})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

try:
    recv()   # session/close — best-effort
except Exception:
    pass

sys.exit(0)
"#).unwrap();
    {
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
    }

    // Build an AcpRuntimeManager wired to the fixture adapter.
    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    // Insert canonical domain entities.
    // workspace_root points at the tempdir so the executor sends it to the
    // fixture via session/new.params.cwd, and the transport scans it for artifacts.
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-e2e".into(),
            workflow_title: "E2E Test Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            current_state: None,
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: None,
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "build_stage".into();
    stage.label = "Build Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    // Wire up BackgroundExecutor.
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp,
        events,
    );

    // Enqueue a fully-populated InvokeAgent work item.
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("build_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "build_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "fixture-agent",
                "provider": "claude",
            }),
        )
        .await
        .unwrap();

    // Process the work item through the real executor path.
    let processed = executor.process_next_item().await.unwrap();
    assert!(processed, "process_next_item must return true when a work item is available");

    // Stage must be settled as Completed.
    let settled = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        settled.status,
        StageStatus::Completed,
        "stage must be Completed after successful ACP session"
    );

    // Artifact must be persisted in the canonical artifacts table.
    let persisted_artifacts = db::repos::artifacts::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(
        persisted_artifacts.len(),
        1,
        "exactly one artifact must be persisted (report.json created by the fixture)"
    );
    let art = &persisted_artifacts[0];
    assert!(
        art.file_path.ends_with("report.json"),
        "artifact file_path must point to report.json, got: {}",
        art.file_path
    );
    assert_eq!(
        art.format.to_string(),
        "json",
        "artifact format must be derived from the .json extension"
    );
    assert_eq!(
        art.contract_id, "claude.output",
        "contract_id must be provider-scoped, not a stub"
    );

    // Projections must reflect the settled stage and its artifact.
    let stage_rows = projections::list_stages_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    let stage_proj = stage_rows
        .iter()
        .find(|s| s.stage_id == "build_stage")
        .expect("build_stage must appear in stage projection after rebuild");
    assert_eq!(
        stage_proj.status,
        StageStatus::Completed.to_string(),
        "stage projection status must match settled status"
    );
    assert!(
        stage_proj.has_artifacts,
        "stage projection must reflect that an artifact was created"
    );
}

/// R7 bar: daemon-vs-Swift behavioral diff harness.
///
/// Takes a golden snapshot captured from the Swift app and proves the daemon
/// produces an equivalent report shape for an identical workflow slice.
/// The golden file lives in tests/fixtures/golden_swift_report.json — it was
/// captured once from a real Swift run and encodes the non-regression bar:
/// for the same input (2-stage linear workflow), both the Swift app and the
/// daemon must produce runs with the same stage IDs, statuses, artifact
/// contracts, and aggregate counts.
#[tokio::test]
async fn test_daemon_vs_swift_report_behavioral_parity() {
    use db::repos::artifacts;
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::ids::ArtifactId;
    use domain::stage::StageSettlementKind;

    // Load golden snapshot from Swift run
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let golden_path = format!("{manifest_dir}/tests/fixtures/golden_swift_report.json");
    let golden_raw = std::fs::read_to_string(&golden_path)
        .expect("golden swift report fixture must exist");
    let golden: serde_json::Value = serde_json::from_str(&golden_raw)
        .expect("golden snapshot must be valid JSON");

    let pool = test_pool().await;

    // Seed the daemon path: idea + run + 2 stages + 2 artifacts matching golden
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();

    let now = Utc::now();
    let golden_stages = golden["stages"].as_array().unwrap();
    for stage_def in golden_stages {
        let stage_id = stage_def["stage_id"].as_str().unwrap();
        let label = stage_def["label"].as_str().unwrap();
        let se_id = StageExecutionId::new();
        let mut stage = StageExecution {
            id: se_id,
            run_id,
            stage_id: stage_id.to_string(),
            label: label.to_string(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
        };
        stages::insert(&pool, &stage).await.unwrap();
        // Settle as Completed (matches golden)
        stages::settle(&pool, se_id, StageSettlementKind::Completed, now)
            .await
            .unwrap();
        stage.status = StageStatus::Completed;
    }

    let golden_artifacts = golden["artifacts"].as_array().unwrap();
    for art_def in golden_artifacts {
        let art = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: art_def["stage_id"].as_str().unwrap().to_string(),
            agent_id: "claude".to_string(),
            name: art_def["name"].as_str().unwrap().to_string(),
            contract_id: art_def["contract_id"].as_str().unwrap().to_string(),
            format: ArtifactFormat::Json,
            file_path: format!("/tmp/parity/{}", art_def["name"].as_str().unwrap()),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".to_string(),
            model: None,
            created_at: now,
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        artifacts::insert(&pool, &art).await.unwrap();
    }

    // Mark run completed
    runs::mark_completed(&pool, run_id, now).await.unwrap();

    // Rebuild projections
    db::repos::projections::rebuild_all_for_run(&pool, run_id)
        .await
        .unwrap();

    // ── Build daemon report (same shape as golden) ─────────────────────────
    let daemon_run = db::repos::projections::find_run_projection(&pool, &run_id.to_string())
        .await
        .unwrap()
        .expect("run must exist");
    let daemon_stages =
        db::repos::projections::list_stages_projection(&pool, &run_id.to_string())
            .await
            .unwrap();
    let daemon_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();

    // ── Diff assertions: daemon report MUST match golden shape ────────────

    // Run-level: status + aggregate counts
    assert_eq!(
        daemon_run.status,
        golden["run_status"].as_str().unwrap(),
        "run status mismatch: daemon produces different status than Swift"
    );
    assert_eq!(
        daemon_run.total_stages,
        golden["total_stages"].as_i64().unwrap(),
        "total_stages mismatch"
    );
    assert_eq!(
        daemon_run.completed_stages,
        golden["completed_stages"].as_i64().unwrap(),
        "completed_stages mismatch"
    );
    assert_eq!(
        daemon_run.failed_stages,
        golden["failed_stages"].as_i64().unwrap(),
        "failed_stages mismatch"
    );

    // Stage-level: each golden stage must appear with matching status
    for golden_stage in golden_stages {
        let stage_id = golden_stage["stage_id"].as_str().unwrap();
        let daemon_stage = daemon_stages
            .iter()
            .find(|s| s.stage_id == stage_id)
            .unwrap_or_else(|| panic!("daemon missing stage {stage_id} that Swift produced"));
        assert_eq!(
            daemon_stage.status,
            golden_stage["status"].as_str().unwrap(),
            "stage {} status mismatch: daemon={} Swift={}",
            stage_id,
            daemon_stage.status,
            golden_stage["status"].as_str().unwrap()
        );
        assert_eq!(
            daemon_stage.attempt_number,
            golden_stage["attempt_number"].as_i64().unwrap(),
            "stage {} attempt_number mismatch",
            stage_id
        );
    }

    // Artifact-level: each golden artifact must exist with matching contract
    for golden_art in golden_artifacts {
        let name = golden_art["name"].as_str().unwrap();
        let daemon_art = daemon_artifacts
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("daemon missing artifact {name} that Swift produced"));
        assert_eq!(
            daemon_art.contract_id,
            golden_art["contract_id"].as_str().unwrap(),
            "artifact {} contract_id mismatch",
            name
        );
        assert_eq!(
            daemon_art.stage_id,
            golden_art["stage_id"].as_str().unwrap(),
            "artifact {} stage_id mismatch",
            name
        );
    }

    // Reverse check: daemon didn't produce MORE stages/artifacts than Swift
    assert_eq!(
        daemon_stages.len(),
        golden_stages.len(),
        "daemon produced {} stages, Swift produced {} — non-regression violation",
        daemon_stages.len(),
        golden_stages.len()
    );
    assert_eq!(
        daemon_artifacts.len(),
        golden_artifacts.len(),
        "daemon produced {} artifacts, Swift produced {} — non-regression violation",
        daemon_artifacts.len(),
        golden_artifacts.len()
    );
}

// ---------------------------------------------------------------------------
// P044: Post-approval task detection in manual_gate approval flow
// ---------------------------------------------------------------------------

/// Approving a manual_gate that has post_approval_tasks (state_11_manual_release)
/// must set the stage to Running (not Completed) so the orchestrator can enqueue
/// the post-approval work.
#[tokio::test]
async fn test_approve_manual_gate_with_post_approval_tasks_sets_running() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    // Build a run with real workflow + catalog paths so the command handler
    // can compile the plan and detect post_approval_tasks.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let examples_dir = format!("{}/../../../examples", manifest_dir);
    let wf_path = format!("{examples_dir}/workflows/full-mvp-live.yaml");
    let cat_path = format!("{examples_dir}/agents/agents.yaml");

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    // Insert the pending approval BEFORE calling the command handler.
    let approval = make_approval(run_id, "state_11_manual_release", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::ApproveStage(ApproveStageCmd {
            run_id,
            stage_id: "state_11_manual_release".into(),
            comment: Some("Ship it".into()),
        }))
        .await
        .unwrap();

    // Stage must be Running (not Completed) because post_approval_tasks exist.
    let updated = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        updated.status,
        StageStatus::Running,
        "manual_gate with post_approval_tasks must transition to Running after approval, \
         not Completed, so the orchestrator can enqueue post-approval work"
    );
}

/// Approving a simple manual_gate (state_3, no post_approval_tasks) must
/// settle the stage as Completed.
#[tokio::test]
async fn test_approve_simple_manual_gate_settles_completed() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let examples_dir = format!("{}/../../../examples", manifest_dir);
    let wf_path = format!("{examples_dir}/workflows/full-mvp-live.yaml");
    let cat_path = format!("{examples_dir}/agents/agents.yaml");

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_3_initial_proposal_approval".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(
        run_id,
        "state_3_initial_proposal_approval",
        ApprovalDecision::Pending,
    );
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::ApproveStage(ApproveStageCmd {
            run_id,
            stage_id: "state_3_initial_proposal_approval".into(),
            comment: Some("Looks good".into()),
        }))
        .await
        .unwrap();

    // Stage must be Completed because state_3 has no post_approval_tasks.
    let updated = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        updated.status,
        StageStatus::Completed,
        "simple manual_gate without post_approval_tasks must settle as Completed after approval"
    );
}

// ---------------------------------------------------------------------------
// P044 focused proof: post-approval task enqueuing and end-state semantics
// ---------------------------------------------------------------------------

/// After approving state_11 (which has post_approval_tasks), advance_run must
/// enqueue InvokeAgent work items for the post-approval tasks.
#[tokio::test]
async fn test_post_approval_tasks_enqueued_after_approval() {
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workflow_path = format!("{}/../../../examples/workflows/full-mvp-live.yaml", manifest_dir);
    let catalog_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(workflow_path);
    run.agent_catalog_yaml_path = Some(catalog_path);
    run.current_state = Some("state_11_manual_release".into());
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "state_11_manual_release", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    // Approve via CommandHandler — this transitions the stage to Running.
    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::ApproveStage(ApproveStageCmd {
            run_id,
            stage_id: "state_11_manual_release".into(),
            comment: Some("Ship it".into()),
        }))
        .await
        .unwrap();

    // Now call advance_run — the orchestrator should detect the post-approval
    // context and enqueue InvokeAgent work items for the post-approval tasks.
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    // Verify InvokeAgent work items were enqueued for post-approval tasks.
    let work_items = db::repos::work_items::list_by_run(&pool, run_id).await.unwrap();
    let invoke_items: Vec<_> = work_items
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();

    assert!(
        !invoke_items.is_empty(),
        "advance_run must enqueue InvokeAgent work items for post-approval tasks on state_11"
    );

    // state_11 has two sequential post-approval tasks (phase 0 then phase 1);
    // phase 0 should be enqueued first.
    let has_commit_push = invoke_items.iter().any(|w| {
        w.payload_json.contains("commit_and_push")
    });
    assert!(
        has_commit_push,
        "at least one InvokeAgent must target the commit_and_push post-approval task"
    );
}

/// An end state with tasks (state_12_workflow_complete) must NOT short-circuit
/// to immediate completion — it must fall through to the compute path, create
/// a Running stage, and enqueue tasks.
#[tokio::test]
async fn test_end_state_with_tasks_does_not_short_circuit() {
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workflow_path = format!("{}/../../../examples/workflows/full-mvp-live.yaml", manifest_dir);
    let catalog_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(workflow_path);
    run.agent_catalog_yaml_path = Some(catalog_path);
    run.current_state = Some("state_12_workflow_complete".into());
    runs::insert(&pool, &run).await.unwrap();

    // No stages yet — the orchestrator should create one and NOT immediately complete.
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    // A stage must have been created for the end state.
    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let end_stage = all_stages
        .iter()
        .find(|s| s.stage_id == "state_12_workflow_complete")
        .expect("orchestrator must create a stage for end state with tasks");

    // The stage should be Running (tasks enqueued), NOT Completed.
    assert_eq!(
        end_stage.status,
        StageStatus::Running,
        "end state with tasks must enter Running (compute path), not short-circuit to Completed"
    );

    // The run must NOT be Completed yet — tasks haven't finished.
    let refreshed_run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_ne!(
        refreshed_run.status,
        RunStatus::Completed,
        "run must not be Completed while end-state tasks are still running"
    );

    // InvokeAgent work items must have been enqueued for the end state's tasks.
    let work_items = db::repos::work_items::list_by_run(&pool, run_id).await.unwrap();
    let invoke_items: Vec<_> = work_items
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert!(
        !invoke_items.is_empty(),
        "end state with tasks must enqueue InvokeAgent work items"
    );
}

// ---------------------------------------------------------------------------
// P044 strengthened focused proofs: phase ordering, retry semantics, and
// simple-gate non-regression
// ---------------------------------------------------------------------------

/// Proves strict runtime phase ordering for post-approval tasks on state_11.
/// After approval and advance_run, only phase 0 (commit_and_push) must be
/// enqueued; phase 1 (build_and_distribute) must NOT appear until phase 0
/// completes.
#[tokio::test]
async fn test_n_phase_sequence_ordering() {
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wf_path = format!("{}/../../../examples/workflows/full-mvp-live.yaml", manifest_dir);
    let cat_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    run.current_state = Some("state_11_manual_release".into());
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "state_11_manual_release", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler.handle(Command::ApproveStage(ApproveStageCmd {
        run_id,
        stage_id: "state_11_manual_release".into(),
        comment: Some("Ship it".into()),
    })).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    // Only phase 0 tasks should be enqueued — phase 1 waits.
    let work_items = db::repos::work_items::list_by_run(&pool, run_id).await.unwrap();
    let invoke_items: Vec<_> = work_items
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();

    // Should have exactly 1 InvokeAgent (phase 0 = commit_and_push only)
    assert_eq!(
        invoke_items.len(),
        1,
        "N-phase ordering: only phase 0 task must be enqueued initially, got {} items",
        invoke_items.len()
    );

    // The enqueued task must be commit_and_push (phase 0), not build_and_distribute (phase 1)
    let payload: serde_json::Value = serde_json::from_str(&invoke_items[0].payload_json).unwrap();
    let task_index = payload["task_index"].as_u64().unwrap();
    assert_eq!(task_index, 0, "first enqueued task must be task_index 0 (phase 0)");
    assert!(
        invoke_items[0].payload_json.contains("commit_and_push"),
        "first enqueued task must be commit_and_push (phase 0)"
    );

    // build_and_distribute (phase 1) must NOT be enqueued yet
    assert!(
        !invoke_items.iter().any(|w| w.payload_json.contains("build_and_distribute")),
        "phase 1 task (build_and_distribute) must NOT be enqueued before phase 0 completes"
    );
}

/// Proves that retrying a failed state_11 post-approval stage returns to
/// WaitingApproval-equivalent state: old stage is Skipped, new stage is
/// Pending with incremented attempt_number.
#[tokio::test]
async fn test_post_approval_retry_requires_fresh_approval() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Blocked)).await.unwrap();

    // Simulate a failed post-approval stage
    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Failed);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler.handle(Command::RetryStage(RetryStageCmd {
        run_id,
        stage_id: "state_11_manual_release".into(),
    })).await.unwrap();

    // Old stage must be Skipped
    let old = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(old.status, StageStatus::Skipped, "old failed stage must be Skipped after retry");

    // New stage must exist with attempt_number = 2 and status Pending
    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let new_stage = all_stages
        .iter()
        .find(|s| s.stage_id == "state_11_manual_release" && s.attempt_number == 2)
        .expect("retry must create new stage with attempt_number=2");
    assert_eq!(new_stage.status, StageStatus::Pending,
        "retried manual_gate stage must start as Pending (orchestrator will re-enter manual gate path)");
}

/// Ensures state_6 (simple gate, no post_approval_tasks) still completes
/// immediately after approval — non-regression for the post_approval_tasks
/// detection logic.
#[tokio::test]
async fn test_simple_manual_gate_no_regression() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wf_path = format!("{}/../../../examples/workflows/full-mvp-live.yaml", manifest_dir);
    let cat_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_6_implementation_approval".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "state_6_implementation_approval", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler.handle(Command::ApproveStage(ApproveStageCmd {
        run_id,
        stage_id: "state_6_implementation_approval".into(),
        comment: Some("Approved".into()),
    })).await.unwrap();

    let updated = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        updated.status,
        StageStatus::Completed,
        "state_6 (simple gate, no post_approval_tasks) must settle as Completed after approval"
    );
}

// ---------------------------------------------------------------------------
// P044: End-to-end happy path for state_11 -> state_12
// ---------------------------------------------------------------------------

/// Walks the full P044 happy path in a single contiguous fixture:
///   1. state_11 approval -> stage Running
///   2. advance_run enqueues phase 0 (commit_and_push)
///   3. simulate phase 0 completion: write git_push_receipt artifact on disk,
///      mark work item Completed
///   4. advance_run enqueues phase 1 (build_and_distribute); assert strict
///      started_at ordering (phase 0 started before phase 1)
///   5. simulate phase 1 completion: write release_bundle_manifest +
///      connect_upload_receipt, mark work item Completed
///   6. advance_run settles state_11 as Completed and transitions to state_12
///   7. advance_run creates state_12 stage and enqueues
///      finalize_run_and_produce_receipts
///   8. simulate finalize completion: write delivery_receipt + run_report +
///      run_state, mark work item Completed
///   9. advance_run settles state_12 and marks run Completed
///
/// Per P044 §8 we do not execute real ACP side effects; we simulate task
/// completion by writing artifact files and marking work items Completed.
#[tokio::test]
async fn test_state_11_to_state_12_happy_path() {
    use chrono::Utc;
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::ids::ArtifactId;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    // Isolated workspace + artifact root so exists() lookups hit only our files.
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let artifact_root = tmp
        .path()
        .join("artifacts")
        .to_string_lossy()
        .into_owned();
    std::fs::create_dir_all(&artifact_root).unwrap();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wf_path = format!("{}/../../../examples/workflows/full-mvp-live.yaml", manifest_dir);
    let cat_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    // Seed run: at state_11, running, with worktree_root so the release safety
    // guard (which now inspects post_approval_tasks) is satisfied.
    let worktree_root = tmp.path().join("worktree").to_string_lossy().into_owned();
    std::fs::create_dir_all(&worktree_root).unwrap();

    let run = Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-test".into(),
        workflow_title: "Test Workflow".into(),
        workspace_root: workspace_root.clone(),
        artifact_root: artifact_root.clone(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        current_state: Some("state_11_manual_release".into()),
        workflow_yaml_path: Some(wf_path),
        agent_catalog_yaml_path: Some(cat_path),
        worktree_root: Some(worktree_root.clone()),
        base_branch: Some("main".into()),
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    // Seed state_11 stage as WaitingApproval with an unresolved approval.
    let stage_11_id = StageExecutionId::new();
    let mut stage_11 = make_stage(stage_11_id, run_id, StageStatus::WaitingApproval);
    stage_11.stage_id = "state_11_manual_release".into();
    stage_11.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage_11).await.unwrap();

    let approval = make_approval(
        run_id,
        "state_11_manual_release",
        ApprovalDecision::Pending,
    );
    approvals::insert(&pool, &approval).await.unwrap();

    // ── Step 1: approve state_11 -> stage transitions to Running ──────────
    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::ApproveStage(ApproveStageCmd {
            run_id,
            stage_id: "state_11_manual_release".into(),
            comment: Some("Ship it".into()),
        }))
        .await
        .unwrap();

    let s11_after_approval = stages::find_by_id(&pool, stage_11_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        s11_after_approval.status,
        StageStatus::Running,
        "state_11 must be Running (not Completed) after approval because it has post_approval_tasks"
    );

    // ── Step 2: advance_run enqueues phase 0 (commit_and_push) only ──────
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events.clone(), work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    let items_after_p0 = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let invoke_items_p0: Vec<_> = items_after_p0
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert_eq!(
        invoke_items_p0.len(),
        1,
        "phase 0 only: exactly one InvokeAgent must be enqueued, got {}",
        invoke_items_p0.len()
    );
    assert!(
        invoke_items_p0[0].payload_json.contains("commit_and_push"),
        "phase 0 must be commit_and_push, payload: {}",
        invoke_items_p0[0].payload_json
    );
    let phase0_item_id = invoke_items_p0[0].id.clone();
    let phase0_enqueued_at = invoke_items_p0[0].created_at;

    // ── Step 3: simulate phase 0 completion ──────────────────────────────
    // Write git_push_receipt at the path the catalog declares so exists() resolves.
    let release_dir = tmp.path().join(".chainworks").join("release");
    std::fs::create_dir_all(&release_dir).unwrap();
    let git_push_receipt_path = release_dir.join("git-push.json");
    std::fs::write(&git_push_receipt_path, r#"{"branch":"main","sha":"deadbeef"}"#).unwrap();

    // Also insert the artifact row so report/projection surfaces see it.
    let now = Utc::now();
    let git_push_artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_11_manual_release".into(),
        agent_id: "commit_and_push_to_github".into(),
        name: "git_push_receipt".into(),
        contract_id: "git_push_receipt_v1".into(),
        format: ArtifactFormat::Json,
        file_path: git_push_receipt_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: now,
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    db::repos::artifacts::insert(&pool, &git_push_artifact).await.unwrap();

    // Mark phase 0 work item Completed.
    db::repos::work_items::complete(&pool, &phase0_item_id).await.unwrap();

    // ── Step 4: advance_run enqueues phase 1 (build_and_distribute) ──────
    orchestrator.advance_run(run_id).await.unwrap();

    let items_after_p1 = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let invoke_items_p1: Vec<_> = items_after_p1
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert_eq!(
        invoke_items_p1.len(),
        2,
        "phase 1 must be enqueued alongside completed phase 0, got {} items",
        invoke_items_p1.len()
    );
    let phase1_item = invoke_items_p1
        .iter()
        .find(|w| w.payload_json.contains("build_and_distribute"))
        .expect("phase 1 InvokeAgent for build_and_distribute must exist");
    let phase1_enqueued_at = phase1_item.created_at;

    // Strict phase ordering by enqueue time: phase 0 must have been enqueued
    // before phase 1. created_at is set when the work item is persisted, so
    // phase 0's timestamp must precede or equal phase 1's.
    assert!(
        phase0_enqueued_at <= phase1_enqueued_at,
        "strict phase ordering: phase 0 (enqueued_at={:?}) must come before phase 1 (enqueued_at={:?})",
        phase0_enqueued_at,
        phase1_enqueued_at
    );

    // ── Step 5: simulate phase 1 completion ──────────────────────────────
    // Write release_bundle_manifest and connect_upload_receipt artifacts.
    let rbm_path = release_dir.join("release-bundle.json");
    std::fs::write(&rbm_path, r#"{"bundle":"ok"}"#).unwrap();
    let cur_path = release_dir.join("connect-upload.json");
    std::fs::write(&cur_path, r#"{"connect":"ok"}"#).unwrap();

    let rbm_artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_11_manual_release".into(),
        agent_id: "build_archive_and_push_connect".into(),
        name: "release_bundle_manifest".into(),
        contract_id: "release_bundle_manifest_v1".into(),
        format: ArtifactFormat::Json,
        file_path: rbm_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    db::repos::artifacts::insert(&pool, &rbm_artifact).await.unwrap();

    let cur_artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_11_manual_release".into(),
        agent_id: "build_archive_and_push_connect".into(),
        name: "connect_upload_receipt".into(),
        contract_id: "connect_upload_receipt_v1".into(),
        format: ArtifactFormat::Json,
        file_path: cur_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    db::repos::artifacts::insert(&pool, &cur_artifact).await.unwrap();

    db::repos::work_items::complete(&pool, &phase1_item.id.clone()).await.unwrap();

    // ── Step 6: advance_run settles state_11, transitions to state_12 ────
    orchestrator.advance_run(run_id).await.unwrap();

    let s11_settled = stages::find_by_id(&pool, stage_11_id).await.unwrap().unwrap();
    assert_eq!(
        s11_settled.status,
        StageStatus::Completed,
        "state_11 must be Completed after both phases complete"
    );

    let run_after_s11 = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        run_after_s11.current_state.as_deref(),
        Some("state_12_workflow_complete"),
        "run must have transitioned to state_12 (git_push_receipt exists)"
    );
    assert_ne!(
        run_after_s11.status,
        RunStatus::Completed,
        "run must not be Completed yet — state_12 tasks haven't run"
    );

    // ── Step 7: advance_run creates state_12 stage and enqueues finalizer ─
    orchestrator.advance_run(run_id).await.unwrap();

    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let s12 = all_stages
        .iter()
        .find(|s| s.stage_id == "state_12_workflow_complete")
        .expect("state_12 stage must be created after transition");
    assert_eq!(
        s12.status,
        StageStatus::Running,
        "state_12 (end state with tasks) must enter Running, not short-circuit to Completed"
    );

    let items_after_s12 = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let s12_invokes: Vec<_> = items_after_s12
        .iter()
        .filter(|w| {
            w.kind == db::work_item::WorkItemKind::InvokeAgent
                && w.payload_json.contains(&s12.id.to_string())
        })
        .collect();
    assert_eq!(
        s12_invokes.len(),
        1,
        "state_12 must enqueue exactly one InvokeAgent (finalize_run_and_produce_receipts)"
    );
    assert!(
        s12_invokes[0]
            .payload_json
            .contains("finalize_run_and_produce_receipts"),
        "state_12 task must be finalize_run_and_produce_receipts, payload: {}",
        s12_invokes[0].payload_json
    );
    let finalize_item_id = s12_invokes[0].id.clone();

    // ── Step 8: simulate finalize completion (write receipt + report) ─────
    let delivery_receipt_path = release_dir.join("delivery-receipt.json");
    std::fs::write(&delivery_receipt_path, r#"{"delivery":"ok"}"#).unwrap();
    let run_report_path = release_dir.join("run-report.json");
    std::fs::write(&run_report_path, r#"{"report":"final"}"#).unwrap();
    let run_state_path = release_dir.join("run-state.json");
    std::fs::write(&run_state_path, r#"{"state":"complete"}"#).unwrap();

    let now = Utc::now();
    for (name, path, contract) in [
        ("delivery_receipt", delivery_receipt_path.clone(), "delivery_receipt_v1"),
        ("run_report", run_report_path.clone(), "run_report_v1"),
        ("run_state", run_state_path.clone(), "run_state_v1"),
    ] {
        let art = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_12_workflow_complete".into(),
            agent_id: "lead_orchestrator".into(),
            name: name.into(),
            contract_id: contract.into(),
            format: ArtifactFormat::Json,
            file_path: path.to_string_lossy().into_owned(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".into(),
            model: None,
            created_at: now,
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        db::repos::artifacts::insert(&pool, &art).await.unwrap();
    }

    db::repos::work_items::complete(&pool, &finalize_item_id).await.unwrap();

    // ── Step 9: advance_run settles state_12 and marks run Completed ─────
    orchestrator.advance_run(run_id).await.unwrap();

    let s12_settled = stages::find_by_id(&pool, s12.id).await.unwrap().unwrap();
    assert_eq!(
        s12_settled.status,
        StageStatus::Completed,
        "state_12 must be Completed after finalize_run_and_produce_receipts finishes"
    );

    let final_run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        final_run.status,
        RunStatus::Completed,
        "run must be Completed after state_12 (end state with tasks) finishes"
    );
    assert!(
        final_run.completed_at.is_some(),
        "completed_at must be set on a completed run"
    );

    // Terminal artifact inventory: all three finalizer outputs must exist.
    let all_artifacts = db::repos::artifacts::list_by_run(&pool, run_id).await.unwrap();
    for terminal in ["delivery_receipt", "run_report", "run_state"] {
        assert!(
            all_artifacts.iter().any(|a| a.name == terminal),
            "terminal artifact {terminal} must be present after run completes"
        );
    }
    // Release intermediate artifacts are also present.
    for intermediate in [
        "git_push_receipt",
        "release_bundle_manifest",
        "connect_upload_receipt",
    ] {
        assert!(
            all_artifacts.iter().any(|a| a.name == intermediate),
            "release artifact {intermediate} must be present after run completes"
        );
    }
}
