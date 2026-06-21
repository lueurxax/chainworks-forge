use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use acp::adapters::claude::ClaudeAgentAdapter;
use acp::adapters::AcpAdapter;
use acp::{AcpRuntimeManager, ExecutionRequest};
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{agent_executions, artifacts, ideas, runs, sessions, stages};
use domain::agent::{AgentExecution, AgentStatus};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::session::{SessionGeneration, SessionGenerationStatus, SessionLineage};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::executor::BackgroundExecutor;
use engine::orchestrator::Orchestrator;
use engine::work_queue::WorkQueue;
use mcp_server::protocol::JsonRpcRequest;
use mcp_server::server::McpServer;
use tokio::time::{timeout, Duration};

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .expect("register shared writer");
    pool
}

fn make_reuse_fixture_script(tmpdir: &std::path::Path) -> String {
    let script = tmpdir.join("p086_acp_reuse.py");
    let marker = tmpdir.join("markers.jsonl");
    let code = format!(
        r#"#!/usr/bin/env python3
import json, os, sys

marker_path = {marker_path:?}

def mark(obj):
    with open(marker_path, "a") as f:
        f.write(json.dumps(obj, sort_keys=True) + "\n")

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    return json.loads(line)

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None:
    sys.exit(1)
cwd = msg.get("params", {{}}).get("cwd", "/tmp")
session_id = "fixture-session-reuse"
mark({{"event": "session_new", "cwd": cwd}})
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": session_id}}}})

turn = 0

while True:
    msg = recv()
    if msg is None:
        sys.exit(0)
    if msg.get("method") == "session/close":
        mark({{"event": "close"}})
        send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        sys.exit(0)
    if msg.get("method") != "session/prompt":
        if "id" in msg:
            send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        continue
    turn += 1
    prompt = msg.get("params", {{}}).get("prompt", "")
    mark({{"event": "prompt", "turn": turn, "prompt": prompt}})
    if turn == 1:
        send({{"jsonrpc": "2.0", "method": "session/update", "params": {{"update": {{"sessionUpdate": "agent_message_chunk", "content": "first turn completed"}}}}}})
    else:
        with open(os.path.join(cwd, "p086-second-turn.txt"), "w") as f:
            f.write("continuation wrote this file\n")
        send({{"jsonrpc": "2.0", "method": "session/update", "params": {{"update": {{"sessionUpdate": "agent_message_chunk", "content": "continuation turn ran ./scripts/test-gate.sh proposal-086 passed"}}}}}})
    send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": session_id}}}})
"#,
        marker_path = marker.to_string_lossy()
    );
    std::fs::write(&script, code).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    script.to_string_lossy().into_owned()
}

fn make_resurrection_fixture_script(tmpdir: &std::path::Path, actual_session_id: &str) -> String {
    let script = tmpdir.join(format!("p086_acp_resurrection_{actual_session_id}.py"));
    let marker = tmpdir.join("markers.jsonl");
    let code = format!(
        r#"#!/usr/bin/env python3
import json, os, sys

marker_path = {marker_path:?}
actual_session_id = {actual_session_id:?}

def mark(obj):
    with open(marker_path, "a") as f:
        f.write(json.dumps(obj, sort_keys=True) + "\n")

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    return json.loads(line)

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None:
    sys.exit(1)
params = msg.get("params", {{}})
cwd = params.get("cwd", "/tmp")
extra = params.get("sessionMode", {{}})
resume_session_id = params.get("resumeSessionId")
if resume_session_id is None:
    # Some ACP clients may nest adapter extras under config.
    resume_session_id = params.get("config", {{}}).get("resumeSessionId")
mark({{"event": "session_new", "cwd": cwd, "resumeSessionId": resume_session_id, "actualSessionId": actual_session_id}})
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": actual_session_id}}}})

while True:
    msg = recv()
    if msg is None:
        sys.exit(0)
    if msg.get("method") == "session/close":
        mark({{"event": "close"}})
        send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        sys.exit(0)
    if msg.get("method") != "session/prompt":
        if "id" in msg:
            send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        continue
    prompt_obj = msg.get("params", {{}}).get("prompt", "")
    if isinstance(prompt_obj, list):
        prompt = "\n".join(
            part.get("text", "") if isinstance(part, dict) else str(part)
            for part in prompt_obj
        )
    elif isinstance(prompt_obj, dict):
        prompt = prompt_obj.get("text", json.dumps(prompt_obj, sort_keys=True))
    else:
        prompt = str(prompt_obj)
    mark({{"event": "prompt", "prompt": prompt}})
    prompt_marker = ""
    request_fingerprint = ""
    target_stage_execution_id = ""
    target_agent_execution_id = ""
    for line in prompt.splitlines():
        if line.startswith("- Prompt turn marker id: "):
            prompt_marker = line.split(": ", 1)[1]
        if line.startswith("- Request fingerprint sha256: "):
            request_fingerprint = line.split(": ", 1)[1]
        if line.startswith("- Stage execution id: "):
            target_stage_execution_id = line.split(": ", 1)[1]
        if line.startswith("- Agent execution id: "):
            target_agent_execution_id = line.split(": ", 1)[1]
    with open(os.path.join(cwd, "p086-resurrection-turn.txt"), "w") as f:
        f.write("resurrection continuation wrote this file\n")
    send({{"jsonrpc": "2.0", "method": "session/update", "params": {{"update": {{"sessionUpdate": "agent_message_chunk", "content": "resurrection turn marker " + prompt_marker + " request " + request_fingerprint + " stage " + target_stage_execution_id + " agent " + target_agent_execution_id + " ran ./scripts/test-gate.sh proposal-086 passed"}}}}}})
    send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": actual_session_id}}}})
"#,
        marker_path = marker.to_string_lossy(),
        actual_session_id = actual_session_id
    );
    std::fs::write(&script, code).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    script.to_string_lossy().into_owned()
}

fn make_uncorrelated_resurrection_fixture_script(
    tmpdir: &std::path::Path,
    actual_session_id: &str,
) -> String {
    let script = tmpdir.join(format!(
        "p086_acp_resurrection_uncorrelated_{actual_session_id}.py"
    ));
    let marker = tmpdir.join("markers.jsonl");
    let code = format!(
        r#"#!/usr/bin/env python3
import json, os, sys

marker_path = {marker_path:?}
actual_session_id = {actual_session_id:?}

def mark(obj):
    with open(marker_path, "a") as f:
        f.write(json.dumps(obj, sort_keys=True) + "\n")

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    return json.loads(line)

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None:
    sys.exit(1)
params = msg.get("params", {{}})
cwd = params.get("cwd", "/tmp")
resume_session_id = params.get("resumeSessionId")
if resume_session_id is None:
    resume_session_id = params.get("config", {{}}).get("resumeSessionId")
mark({{"event": "session_new", "cwd": cwd, "resumeSessionId": resume_session_id, "actualSessionId": actual_session_id}})
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": actual_session_id}}}})

while True:
    msg = recv()
    if msg is None:
        sys.exit(0)
    if msg.get("method") == "session/close":
        mark({{"event": "close"}})
        send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        sys.exit(0)
    if msg.get("method") != "session/prompt":
        if "id" in msg:
            send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        continue
    prompt = msg.get("params", {{}}).get("prompt", "")
    mark({{"event": "prompt", "prompt": prompt}})
    with open(os.path.join(cwd, "p086-resurrection-turn.txt"), "w") as f:
        f.write("uncorrelated resurrection continuation wrote this file\n")
    send({{"jsonrpc": "2.0", "method": "session/update", "params": {{"update": {{"sessionUpdate": "agent_message_chunk", "content": "uncorrelated resurrection turn ran ./scripts/test-gate.sh proposal-086 passed"}}}}}})
    send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": actual_session_id}}}})
"#,
        marker_path = marker.to_string_lossy(),
        actual_session_id = actual_session_id
    );
    std::fs::write(&script, code).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    script.to_string_lossy().into_owned()
}

fn make_lost_terminal_resurrection_fixture_script(
    tmpdir: &std::path::Path,
    actual_session_id: &str,
) -> String {
    let script = tmpdir.join(format!(
        "p086_acp_resurrection_lost_terminal_{actual_session_id}.py"
    ));
    let marker = tmpdir.join("markers.jsonl");
    let code = format!(
        r#"#!/usr/bin/env python3
import json, sys

marker_path = {marker_path:?}
actual_session_id = {actual_session_id:?}

def mark(obj):
    with open(marker_path, "a") as f:
        f.write(json.dumps(obj, sort_keys=True) + "\n")

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    return json.loads(line)

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None:
    sys.exit(1)
params = msg.get("params", {{}})
cwd = params.get("cwd", "/tmp")
resume_session_id = params.get("resumeSessionId")
if resume_session_id is None:
    resume_session_id = params.get("config", {{}}).get("resumeSessionId")
mark({{"event": "session_new", "cwd": cwd, "resumeSessionId": resume_session_id, "actualSessionId": actual_session_id}})
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": actual_session_id}}}})

while True:
    msg = recv()
    if msg is None:
        sys.exit(0)
    if msg.get("method") == "session/close":
        mark({{"event": "close"}})
        send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        sys.exit(0)
    if msg.get("method") != "session/prompt":
        if "id" in msg:
            send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})
        continue
    mark({{"event": "prompt_then_drop"}})
    sys.exit(42)
"#,
        marker_path = marker.to_string_lossy(),
        actual_session_id = actual_session_id
    );
    std::fs::write(&script, code).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    script.to_string_lossy().into_owned()
}

fn continuation_catalog_snapshot_json() -> String {
    serde_json::json!({
        "agents": [{
            "id": "code_writer",
            "continuation_capability": {
                "enabled": true,
                "allowed_triggers": ["operator_mcp", "lead_auto"],
                "live_handle_continuation": {
                    "enabled": true,
                    "require_no_unresolved_side_effects": true
                },
                "provider_session_resurrection": {
                    "enabled": true,
                    "allowed_triggers": ["operator_mcp"],
                    "require_recorded_provider_session_id": true,
                    "fail_closed_when_unsupported": true
                }
            }
        }]
    })
    .to_string()
}

async fn seed_run_with_completed_code_writer(
    pool: &sqlx::SqlitePool,
    workspace_root: &str,
) -> (RunId, StageExecutionId, AgentExecutionId) {
    let now = Utc::now();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P086 continuation e2e".into(),
            body: "body".into(),
            workspace_root_path: Some(workspace_root.to_string()),
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();

    runs::insert(
        pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-p086".into(),
            workflow_title: "P086 Workflow".into(),
            workspace_root: workspace_root.to_string(),
            artifact_root: workspace_root.to_string(),
            started_at: now,
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_10_implementation_refined".into()),
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: Some(workspace_root.to_string()),
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
            catalog_snapshot_hash: Some("b".repeat(64)),
            workflow_snapshot_json: None,
            catalog_snapshot_json: Some(continuation_catalog_snapshot_json()),
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: Some(format!("{workspace_root}/.chainworks/runs/{run_id}")),
            review_routing_json: None,
            closeout_readiness_mode: None,
        },
    )
    .await
    .unwrap();

    stages::insert(
        pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "state_10_implementation_refined".into(),
            label: "Implementation refined".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: Some(StageSettlementKind::Completed),
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("code_writer".into()),
            provider: Some("claude".into()),
            model: Some("fixture-model".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();

    let lineage = SessionLineage {
        id: "lineage-p086".into(),
        run_id: run_id.to_string(),
        agent_id: "code_writer".into(),
        lineage_id: "lineage-p086".into(),
        session_reuse_scope: "same_agent_family_within_run".into(),
        session_family_id: Some("code_writer".into()),
        active_generation_id: Some("p086-generation".into()),
        created_at: now,
        closed_at: None,
    };
    sessions::insert_lineage(pool, &lineage).await.unwrap();
    sessions::insert_generation(
        pool,
        &SessionGeneration {
            id: "p086-generation".into(),
            lineage_id: lineage.id.clone(),
            generation: 1,
            invocation_owner_key: "code_writer".into(),
            provider_session_id: Some("fixture-session-reuse".into()),
            binding_fingerprint: "c".repeat(64),
            rehydrated_from_checkpoint_artifact_id: None,
            working_directory: workspace_root.to_string(),
            workspace_mode: "write".into(),
            runtime_provider: "claude".into(),
            runtime_model: "fixture-model".into(),
            status: SessionGenerationStatus::Active,
            turn_count: 1,
            estimated_input_tokens: 0,
            latest_cached_input_tokens: None,
            latest_output_tokens: None,
            latest_model_context_window: None,
            cumulative_prompt_tokens: 0,
            cumulative_cost_cents: 0,
            created_at: now,
            last_activity_at: Some(now),
            ended_at: None,
            end_reason: None,
        },
    )
    .await
    .unwrap();

    agent_executions::insert(
        pool,
        &AgentExecution {
            id: agent_execution_id,
            stage_execution_id: Some(stage_execution_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("fixture-model".into()),
            started_at: now,
            completed_at: Some(now),
            status: AgentStatus::Completed,
            owner_execution_lineage_id: None,
            session_lineage_id: Some(lineage.id),
            session_generation_id: Some("p086-generation".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("code_writer".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("code_writer".into()),
            session_reuse_disposition: Some("reused".into()),
            session_reset_reason: None,
            backend_profile_id: Some("claude_code_writer".into()),
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
            owner_kind: Some("stage_execution".into()),
            owner_id: Some(stage_execution_id.to_string()),
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        },
    )
    .await
    .unwrap();

    (run_id, stage_execution_id, agent_execution_id)
}

async fn call_continue_work(
    server: &McpServer,
    agent_execution_id: AgentExecutionId,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
) -> serde_json::Value {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(serde_json::json!(1)),
        method: "tools/call".into(),
        params: Some(serde_json::json!({
            "name": "agents.continue_work",
            "arguments": {
                "agent_execution_id": agent_execution_id.to_string(),
                "run_id": run_id.to_string(),
                "stage_execution_id": stage_execution_id.to_string(),
                "session_generation_id": "p086-generation",
                "provider_session_id": "fixture-session-reuse",
                "mode": "live_handle_continuation",
                "trigger_kind": "operator_mcp",
                "idempotency_key": "01890f3d-7df9-7cc8-98c4-dc0c0c073981",
                "operator_instruction": "Continue the same live ACP session and report test-gate evidence.",
                "max_turns": 2,
                "max_wall_clock_seconds": 120,
                "blockers": ["need live continuation proof"]
            }
        })),
    };
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let response = server.handle_request(req, &principal).await;
    assert!(
        response.error.is_none(),
        "MCP response must not be JSON-RPC error: {:?}",
        response.error
    );
    let result = response.result.expect("tools/call should return result");
    serde_json::from_str(
        result["content"][0]["text"]
            .as_str()
            .expect("content text should be JSON string"),
    )
    .expect("inner tools/call payload should parse")
}

async fn call_continue_work_with_mode(
    server: &McpServer,
    agent_execution_id: AgentExecutionId,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    mode: &str,
    idempotency_key: &str,
) -> serde_json::Value {
    let req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(serde_json::json!(1)),
        method: "tools/call".into(),
        params: Some(serde_json::json!({
            "name": "agents.continue_work",
            "arguments": {
                "agent_execution_id": agent_execution_id.to_string(),
                "run_id": run_id.to_string(),
                "stage_execution_id": stage_execution_id.to_string(),
                "session_generation_id": "p086-generation",
                "provider_session_id": "fixture-session-reuse",
                "mode": mode,
                "trigger_kind": "operator_mcp",
                "idempotency_key": idempotency_key,
                "operator_instruction": "Continue from the recorded provider session and report test-gate evidence.",
                "max_turns": 2,
                "max_wall_clock_seconds": 120,
                "blockers": ["need provider session resurrection proof"]
            }
        })),
    };
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let response = server.handle_request(req, &principal).await;
    assert!(
        response.error.is_none(),
        "MCP response must not be JSON-RPC error: {:?}",
        response.error
    );
    let result = response.result.expect("tools/call should return result");
    serde_json::from_str(
        result["content"][0]["text"]
            .as_str()
            .expect("content text should be JSON string"),
    )
    .expect("inner tools/call payload should parse")
}

#[cfg(unix)]
#[tokio::test]
async fn p086_mcp_continue_work_reuses_live_acp_session_and_materializes_terminal_artifacts() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root_path = tmp.path().canonicalize().unwrap();
    let workspace_root = workspace_root_path.to_string_lossy().into_owned();
    let script = make_reuse_fixture_script(&workspace_root_path);
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script)) as Arc<dyn AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    let (run_id, stage_execution_id, agent_execution_id) =
        seed_run_with_completed_code_writer(&pool, &workspace_root).await;

    let first_result = acp
        .start_session(ExecutionRequest {
            run_id,
            stage_execution_id: Some(stage_execution_id.to_string()),
            stage_id: "state_10_implementation_refined".into(),
            attempt_number: 1,
            agent_execution_id: Some(agent_execution_id),
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("fixture-model".into()),
            effort: None,
            workspace_root: workspace_root.clone(),
            prompt: "first turn".into(),
            worktree_root: Some(workspace_root.clone()),
            worktree_write_enabled: true,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: true,
            reuse_existing_session: false,
            session_generation_id: Some("p086-generation".into()),
            provider_session_id: None,
            provider_runtime_home: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: Some(format!("{workspace_root}/.chainworks/runs/{run_id}")),
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".into(),
            owner_id: Some(stage_execution_id.to_string()),
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,

            p079_repair_canonical_paths: None,
        })
        .await
        .unwrap();
    assert_eq!(first_result.status, AgentStatus::Completed);
    assert_eq!(
        first_result.provider_session_id.as_deref(),
        Some("fixture-session-reuse")
    );

    let command_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let server = McpServer::new(
        pool.clone(),
        command_handler,
        auth::PrincipalTable::test_fixture(),
    )
    .with_acp_runtime(Arc::clone(&acp));
    let admitted =
        call_continue_work(&server, agent_execution_id, run_id, stage_execution_id).await;
    assert_eq!(admitted["outcome"], "accepted");
    let continuation_id = admitted["continuation_id"]
        .as_str()
        .expect("accepted response must include continuation_id")
        .to_string();

    let pending_kind: String = sqlx::query_scalar(
        "SELECT kind FROM work_items WHERE status='pending' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending_kind, "process_continuation");

    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );
    let processed = timeout(Duration::from_secs(10), executor.process_next_item())
        .await
        .unwrap_or_else(|_| {
            let markers_path = tmp.path().join("markers.jsonl");
            panic!(
                "P086 continuation worker timed out; fixture markers:\n{}",
                std::fs::read_to_string(markers_path).unwrap_or_else(|error| error.to_string())
            )
        })
        .unwrap();
    assert!(processed);

    let markers_path = tmp.path().join("markers.jsonl");
    let markers = std::fs::read_to_string(markers_path).unwrap();
    let session_new_count = markers
        .lines()
        .filter(|line| line.contains(r#""event": "session_new""#))
        .count();
    let prompt_count = markers
        .lines()
        .filter(|line| line.contains(r#""event": "prompt""#))
        .count();
    assert_eq!(
        session_new_count, 1,
        "continuation must not start a fresh ACP session"
    );
    assert_eq!(
        prompt_count, 2,
        "fixture should receive original + continuation prompts"
    );
    assert!(
        markers.contains("# P086 Continuation Mode Reset"),
        "continuation prompt must use canonical mode-reset text: {markers}"
    );

    let row = db::repos::agent_work_continuations::find_by_id(&pool, &continuation_id)
        .await
        .unwrap()
        .expect("continuation row should exist");
    assert_eq!(row.status, "succeeded");
    assert!(row.canonical_request_artifact_id.is_some());
    assert!(row.attach_receipt_artifact_id.is_some());
    assert!(row.response_artifact_id.is_some());
    assert!(row.result_or_no_progress_artifact_id.is_some());
    assert!(row.evidence_bundle_artifact_id.is_some());
    assert!(row.worktree_readback_artifact_id.is_some());
    assert!(row.continuation_report_artifact_id.is_some());

    let ledger_kinds: Vec<String> = sqlx::query_scalar(
        "SELECT side_effect_kind FROM agent_external_side_effect_ledger
         WHERE continuation_id = ? ORDER BY sequence_number ASC",
    )
    .bind(&continuation_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        ledger_kinds,
        vec![
            "provider_session_attach",
            "runtime_lease",
            "worktree_lease",
            "provider_send"
        ]
    );

    let response_artifact = artifacts::find_by_id(
        &pool,
        row.response_artifact_id.as_ref().unwrap().parse().unwrap(),
    )
    .await
    .unwrap()
    .expect("response artifact should exist");
    assert_eq!(
        response_artifact.contract_id,
        "continuation_response_snapshot_v1"
    );
    let response_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&response_artifact.file_path).unwrap())
            .unwrap();
    assert_eq!(
        response_json["payload"]["response_artifact_id"].as_str(),
        row.response_artifact_id.as_deref()
    );
    assert_eq!(response_json["payload"]["reused_existing_session"], true);

    let result_artifact = artifacts::find_by_id(
        &pool,
        row.result_or_no_progress_artifact_id
            .as_ref()
            .unwrap()
            .parse()
            .unwrap(),
    )
    .await
    .unwrap()
    .expect("result artifact should exist");
    assert_eq!(result_artifact.contract_id, "continuation_result_v1");
    let result_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&result_artifact.file_path).unwrap())
            .unwrap();
    let gate_rows = result_json["payload"]["tests_or_gates"]
        .as_array()
        .expect("tests_or_gates should be array");
    assert!(
        gate_rows.iter().any(|row| row["name"] == "test-gate"
            && row["status"] == "passed"
            && row["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("proposal-086"))),
        "result artifact should capture provider-emitted test gate evidence: {result_json}"
    );

    let evidence_artifact = artifacts::find_by_id(
        &pool,
        row.evidence_bundle_artifact_id
            .as_ref()
            .unwrap()
            .parse()
            .unwrap(),
    )
    .await
    .unwrap()
    .expect("evidence artifact should exist");
    assert_eq!(
        evidence_artifact.contract_id,
        "agent_continuation_evidence_bundle_v1"
    );
    let evidence_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&evidence_artifact.file_path).unwrap())
            .unwrap();
    assert_eq!(evidence_json["payload"]["reused_existing_session"], true);
    assert_eq!(
        evidence_json["payload"]["provider_session_id"],
        "fixture-session-reuse"
    );

    acp.close_session("p086-generation").await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn p086_mcp_continue_work_resurrects_provider_session_and_records_v2_receipt() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root_path = tmp.path().canonicalize().unwrap();
    let workspace_root = workspace_root_path.to_string_lossy().into_owned();
    let script = make_resurrection_fixture_script(&workspace_root_path, "fixture-session-reuse");
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script)) as Arc<dyn AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    let (run_id, stage_execution_id, agent_execution_id) =
        seed_run_with_completed_code_writer(&pool, &workspace_root).await;
    let command_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let server = McpServer::new(
        pool.clone(),
        command_handler,
        auth::PrincipalTable::test_fixture(),
    )
    .with_acp_runtime(Arc::clone(&acp));
    let admitted = call_continue_work_with_mode(
        &server,
        agent_execution_id,
        run_id,
        stage_execution_id,
        "provider_session_resurrection",
        "01890f3d-7df9-7cc8-98c4-dc0c0c073982",
    )
    .await;
    assert_eq!(admitted["outcome"], "accepted");
    let continuation_id = admitted["continuation_id"]
        .as_str()
        .expect("accepted response must include continuation_id")
        .to_string();

    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );
    let processed = timeout(Duration::from_secs(10), executor.process_next_item())
        .await
        .unwrap_or_else(|_| {
            let markers_path = workspace_root_path.join("markers.jsonl");
            panic!(
                "P086 resurrection worker timed out; fixture markers:\n{}",
                std::fs::read_to_string(markers_path).unwrap_or_else(|error| error.to_string())
            )
        })
        .unwrap();
    assert!(processed);

    let markers_path = workspace_root_path.join("markers.jsonl");
    let markers = std::fs::read_to_string(markers_path).unwrap();
    assert!(
        markers.contains(r#""resumeSessionId": "fixture-session-reuse""#),
        "session/new must carry requested provider session id: {markers}"
    );
    assert!(
        markers.contains("# P086 Continuation Mode Reset"),
        "resurrection prompt must use canonical mode-reset text: {markers}"
    );
    let session_new_count = markers
        .lines()
        .filter(|line| line.contains(r#""event": "session_new""#))
        .count();
    let prompt_count = markers
        .lines()
        .filter(|line| line.contains(r#""event": "prompt""#))
        .count();
    assert_eq!(session_new_count, 1);
    assert_eq!(prompt_count, 1);

    let row = db::repos::agent_work_continuations::find_by_id(&pool, &continuation_id)
        .await
        .unwrap()
        .expect("continuation row should exist");
    assert_eq!(row.mode, "provider_session_resurrection");
    assert_eq!(
        row.status, "succeeded",
        "unexpected resurrection status: failure_reason={:?}; markers={markers}",
        row.failure_reason
    );
    assert!(row.canonical_request_artifact_id.is_some());
    assert!(row.attach_receipt_artifact_id.is_some());
    assert!(row.response_artifact_id.is_some());
    assert!(row.result_or_no_progress_artifact_id.is_some());

    let receipt =
        db::repos::p086_resurrection_raw_receipts::find_by_continuation_id(&pool, &continuation_id)
            .await
            .unwrap()
            .expect("raw v2 receipt should be stored in DB");
    let receipt_json: serde_json::Value = serde_json::from_str(&receipt.raw_receipt_json).unwrap();
    assert_eq!(receipt_json["schema_version"], 2);
    assert_eq!(
        receipt_json["requested_provider_session_id"],
        "fixture-session-reuse"
    );
    assert_eq!(
        receipt_json["actual_provider_session_id"],
        "fixture-session-reuse"
    );
    assert_eq!(receipt_json["resurrection_phase"], "completed");
    assert!(
        receipt_json["prompt_sent_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "successful resurrection receipt must be refreshed after prompt send: {receipt_json}"
    );

    let phase: String =
        sqlx::query_scalar("SELECT resurrection_phase FROM agent_work_continuations WHERE id = ?")
            .bind(&continuation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(phase, "completed");
}

#[cfg(unix)]
#[tokio::test]
async fn p086_resurrection_rejects_uncorrelated_terminal_response() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root_path = tmp.path().canonicalize().unwrap();
    let workspace_root = workspace_root_path.to_string_lossy().into_owned();
    let script = make_uncorrelated_resurrection_fixture_script(
        &workspace_root_path,
        "fixture-session-reuse",
    );
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script)) as Arc<dyn AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    let (run_id, stage_execution_id, agent_execution_id) =
        seed_run_with_completed_code_writer(&pool, &workspace_root).await;
    let command_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let server = McpServer::new(
        pool.clone(),
        command_handler,
        auth::PrincipalTable::test_fixture(),
    )
    .with_acp_runtime(Arc::clone(&acp));
    let admitted = call_continue_work_with_mode(
        &server,
        agent_execution_id,
        run_id,
        stage_execution_id,
        "provider_session_resurrection",
        "01890f3d-7df9-7cc8-98c4-dc0c0c073984",
    )
    .await;
    assert_eq!(admitted["outcome"], "accepted");
    let continuation_id = admitted["continuation_id"]
        .as_str()
        .expect("accepted response must include continuation_id")
        .to_string();

    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );
    let processed = timeout(Duration::from_secs(10), executor.process_next_item())
        .await
        .unwrap_or_else(|_| {
            let markers_path = workspace_root_path.join("markers.jsonl");
            panic!(
                "P086 resurrection worker timed out; fixture markers:\n{}",
                std::fs::read_to_string(markers_path).unwrap_or_else(|error| error.to_string())
            )
        })
        .unwrap();
    assert!(processed);

    let markers = std::fs::read_to_string(workspace_root_path.join("markers.jsonl")).unwrap();
    assert!(
        markers.contains(r#""event": "prompt""#),
        "negative fixture must prove the prompt was sent before terminal correlation failed: {markers}"
    );

    let row = db::repos::agent_work_continuations::find_by_id(&pool, &continuation_id)
        .await
        .unwrap()
        .expect("continuation row should exist");
    assert_eq!(row.status, "failed");
    assert_eq!(
        row.failure_reason.as_deref(),
        Some("terminal_response_uncorrelated")
    );
    let response_artifact = artifacts::find_by_id(
        &pool,
        row.response_artifact_id.as_ref().unwrap().parse().unwrap(),
    )
    .await
    .unwrap()
    .expect("failure response snapshot should exist");
    let response_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&response_artifact.file_path).unwrap())
            .unwrap();
    assert_eq!(response_json["payload"]["status"], "failed");
    assert_eq!(
        response_json["payload"]["provider_status"],
        serde_json::Value::Null
    );

    let no_progress_artifact = artifacts::find_by_id(
        &pool,
        row.result_or_no_progress_artifact_id
            .as_ref()
            .unwrap()
            .parse()
            .unwrap(),
    )
    .await
    .unwrap()
    .expect("failure no-progress artifact should exist");
    assert_eq!(
        no_progress_artifact.contract_id,
        "continuation_no_progress_report_v1"
    );
    let no_progress_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&no_progress_artifact.file_path).unwrap())
            .unwrap();
    assert_eq!(
        no_progress_json["payload"]["no_progress_reason"],
        "terminal_response_uncorrelated"
    );

    let phase: String =
        sqlx::query_scalar("SELECT resurrection_phase FROM agent_work_continuations WHERE id = ?")
            .bind(&continuation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(phase, "failed_closed");
}

#[cfg(unix)]
#[tokio::test]
async fn p086_resurrection_fail_closes_when_attach_receipt_persistence_fails() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root_path = tmp.path().canonicalize().unwrap();
    let workspace_root = workspace_root_path.to_string_lossy().into_owned();
    let script = make_resurrection_fixture_script(&workspace_root_path, "fixture-session-reuse");
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script)) as Arc<dyn AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    let (run_id, stage_execution_id, agent_execution_id) =
        seed_run_with_completed_code_writer(&pool, &workspace_root).await;
    let command_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let server = McpServer::new(
        pool.clone(),
        command_handler,
        auth::PrincipalTable::test_fixture(),
    )
    .with_acp_runtime(Arc::clone(&acp));
    let admitted = call_continue_work_with_mode(
        &server,
        agent_execution_id,
        run_id,
        stage_execution_id,
        "provider_session_resurrection",
        "01890f3d-7df9-7cc8-98c4-dc0c0c073985",
    )
    .await;
    assert_eq!(admitted["outcome"], "accepted");
    let continuation_id = admitted["continuation_id"]
        .as_str()
        .expect("accepted response must include continuation_id")
        .to_string();

    sqlx::query("DROP TABLE p086_resurrection_raw_receipts")
        .execute(&pool)
        .await
        .unwrap();

    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );
    let processed = timeout(Duration::from_secs(10), executor.process_next_item())
        .await
        .unwrap_or_else(|_| {
            let markers_path = workspace_root_path.join("markers.jsonl");
            panic!(
                "P086 resurrection worker timed out; fixture markers:\n{}",
                std::fs::read_to_string(markers_path).unwrap_or_else(|error| error.to_string())
            )
        })
        .unwrap();
    assert!(processed);

    let markers = std::fs::read_to_string(workspace_root_path.join("markers.jsonl")).unwrap();
    assert!(
        markers.contains(r#""event": "session_new""#),
        "negative fixture must attach before receipt persistence fails: {markers}"
    );
    assert!(
        markers.contains(r#""event": "close""#),
        "receipt persistence failure must close the attached provider session: {markers}"
    );
    assert!(
        !markers.contains(r#""event": "prompt""#),
        "receipt persistence failure must fail closed before prompt: {markers}"
    );

    let row = db::repos::agent_work_continuations::find_by_id(&pool, &continuation_id)
        .await
        .unwrap()
        .expect("continuation row should exist");
    assert_eq!(row.status, "failed");
    assert_eq!(
        row.failure_reason.as_deref(),
        Some("attach_receipt_persist_failed")
    );
    assert!(row.attach_receipt_artifact_id.is_none());

    let phase: String =
        sqlx::query_scalar("SELECT resurrection_phase FROM agent_work_continuations WHERE id = ?")
            .bind(&continuation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(phase, "failed_closed");
}

#[cfg(unix)]
#[tokio::test]
async fn p086_resurrection_records_claude_session_store_recovery_in_raw_receipt() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root_path = tmp.path().canonicalize().unwrap();
    let workspace_root = workspace_root_path.to_string_lossy().into_owned();
    let claude_projects_root = workspace_root_path.join("claude-projects");
    let script = make_lost_terminal_resurrection_fixture_script(
        &workspace_root_path,
        "fixture-session-reuse",
    );
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script)) as Arc<dyn AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    let (run_id, stage_execution_id, agent_execution_id) =
        seed_run_with_completed_code_writer(&pool, &workspace_root).await;
    let command_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let server = McpServer::new(
        pool.clone(),
        command_handler,
        auth::PrincipalTable::test_fixture(),
    )
    .with_acp_runtime(Arc::clone(&acp));
    let admitted = call_continue_work_with_mode(
        &server,
        agent_execution_id,
        run_id,
        stage_execution_id,
        "provider_session_resurrection",
        "01890f3d-7df9-7cc8-98c4-dc0c0c073986",
    )
    .await;
    assert_eq!(admitted["outcome"], "accepted");
    let continuation_id = admitted["continuation_id"]
        .as_str()
        .expect("accepted response must include continuation_id")
        .to_string();
    let continuation = db::repos::agent_work_continuations::find_by_id(&pool, &continuation_id)
        .await
        .unwrap()
        .expect("continuation row should exist");
    let prompt_marker = format!("p086-prompt-turn:{continuation_id}:provider_session_attach");
    let recovered_text = format!(
        "{prompt_marker} {} {} {} CHAINWORKS_OUTPUT {{\"continuation\":\"recovered\"}} ./scripts/test-gate.sh proposal-086 passed",
        continuation.request_fingerprint_sha256,
        continuation.stage_execution_id,
        continuation.agent_execution_id
    );
    let session_line = serde_json::json!({
        "type": "assistant",
        "message": {
            "id": "msg-recovered-p086",
            "stop_reason": "end_turn",
            "content": [{
                "type": "text",
                "text": recovered_text
            }]
        }
    });
    let transcript_path = claude_projects_root
        .join("-workspace")
        .join("fixture-session-reuse.jsonl");
    std::fs::create_dir_all(transcript_path.parent().unwrap()).unwrap();
    std::fs::write(&transcript_path, format!("{session_line}\n")).unwrap();

    std::env::set_var(
        "CHAINWORKS_CLAUDE_SESSION_STORE_ROOT",
        claude_projects_root.to_string_lossy().as_ref(),
    );
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );
    let processed = timeout(Duration::from_secs(10), executor.process_next_item())
        .await
        .unwrap_or_else(|_| {
            let markers_path = workspace_root_path.join("markers.jsonl");
            panic!(
                "P086 resurrection worker timed out; fixture markers:\n{}",
                std::fs::read_to_string(markers_path).unwrap_or_else(|error| error.to_string())
            )
        })
        .unwrap();
    std::env::remove_var("CHAINWORKS_CLAUDE_SESSION_STORE_ROOT");
    assert!(processed);

    let markers = std::fs::read_to_string(workspace_root_path.join("markers.jsonl")).unwrap();
    assert!(markers.contains(r#""event": "prompt_then_drop""#));

    let row = db::repos::agent_work_continuations::find_by_id(&pool, &continuation_id)
        .await
        .unwrap()
        .expect("continuation row should exist");
    assert_eq!(
        row.status, "succeeded",
        "session-store recovery should settle succeeded: {:?}",
        row.failure_reason
    );

    let receipt =
        db::repos::p086_resurrection_raw_receipts::find_by_continuation_id(&pool, &continuation_id)
            .await
            .unwrap()
            .expect("raw v2 receipt should be stored in DB");
    let receipt_json: serde_json::Value = serde_json::from_str(&receipt.raw_receipt_json).unwrap();
    assert_eq!(
        receipt_json["session_store_recovery_result"],
        "recovered_task_complete"
    );
    assert_eq!(
        receipt_json["session_store_transcript_path"],
        transcript_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        receipt_json["session_store_ownership_source"],
        "provider_session_id"
    );
    assert_eq!(
        receipt_json["session_store_transcript_digest"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert!(receipt_json["session_store_read_at"].as_str().is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn p086_mcp_continue_work_rejects_resurrection_identity_mismatch_before_prompt() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root_path = tmp.path().canonicalize().unwrap();
    let workspace_root = workspace_root_path.to_string_lossy().into_owned();
    let script = make_resurrection_fixture_script(&workspace_root_path, "different-session");
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script)) as Arc<dyn AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    let (run_id, stage_execution_id, agent_execution_id) =
        seed_run_with_completed_code_writer(&pool, &workspace_root).await;
    let command_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let server = McpServer::new(
        pool.clone(),
        command_handler,
        auth::PrincipalTable::test_fixture(),
    )
    .with_acp_runtime(Arc::clone(&acp));
    let admitted = call_continue_work_with_mode(
        &server,
        agent_execution_id,
        run_id,
        stage_execution_id,
        "provider_session_resurrection",
        "01890f3d-7df9-7cc8-98c4-dc0c0c073983",
    )
    .await;
    assert_eq!(admitted["outcome"], "accepted");
    let continuation_id = admitted["continuation_id"]
        .as_str()
        .expect("accepted response must include continuation_id")
        .to_string();

    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );
    let processed = timeout(Duration::from_secs(10), executor.process_next_item())
        .await
        .unwrap()
        .unwrap();
    assert!(processed);

    let markers = std::fs::read_to_string(workspace_root_path.join("markers.jsonl")).unwrap();
    assert!(
        !markers.contains(r#""event": "prompt""#),
        "identity mismatch must fail before prompt: {markers}"
    );

    let row = db::repos::agent_work_continuations::find_by_id(&pool, &continuation_id)
        .await
        .unwrap()
        .expect("continuation row should exist");
    assert_eq!(row.status, "failed");
    assert_eq!(
        row.failure_reason.as_deref(),
        Some("actual_session_mismatch")
    );
    let receipt =
        db::repos::p086_resurrection_raw_receipts::find_by_continuation_id(&pool, &continuation_id)
            .await
            .unwrap();
    assert!(receipt.is_none(), "mismatch must not persist a raw receipt");

    let phase: String =
        sqlx::query_scalar("SELECT resurrection_phase FROM agent_work_continuations WHERE id = ?")
            .bind(&continuation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(phase, "failed_closed");
}
