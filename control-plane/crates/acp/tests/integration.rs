//! ACP adapter integration tests.
//!
//! Each test spawns a Python fixture that speaks the real ACP JSON-RPC 2.0
//! protocol (initialize → session/new → session/prompt → session/close)
//! and verifies that the adapter's subprocess transport path works end-to-end.
//!
//! The Python fixture creates an artifact file inside the workspace_root
//! it receives via `session/new.params.cwd`, allowing the workspace-diff
//! artifact discovery mechanism to be exercised.

#[cfg(unix)]
mod fixture {
    use std::os::unix::fs::PermissionsExt;

    /// Write a fixture ACP server script that:
    /// 1. Completes the ACP handshake (initialize → session/new → session/prompt)
    /// 2. Creates `result.json` inside the `cwd` it receives from `session/new`
    /// 3. Emits a `session/update` text chunk notification
    /// 4. Sends the terminal `session/prompt` response
    /// 5. Reads (and ignores) `session/close`, then exits 0
    pub fn create_success_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_success.py");
        let code = r#"#!/usr/bin/env python3
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

# Phase 1: initialize
msg = recv()
if msg is None:
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"],
      "result": {"protocolVersion": 1, "serverInfo": {"name": "test-fixture", "version": "0.0.1"}}})

# Phase 2: session/new  — capture cwd for artifact creation
msg = recv()
if msg is None:
    sys.exit(1)
cwd = msg.get("params", {}).get("cwd", "/tmp")
session_id = "fixture-session-success"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

# Phase 3: session/prompt — create artifact then respond
msg = recv()
if msg is None:
    sys.exit(1)

artifact_path = os.path.join(cwd, "result.json")
try:
    with open(artifact_path, "w") as f:
        f.write('{"ok": true}\n')
except OSError as e:
    sys.stderr.write(f"fixture: could not write artifact: {e}\n")

# Emit a text chunk notification
send({"jsonrpc": "2.0", "method": "session/update",
      "params": {"update": {"sessionUpdate": "agent_message_chunk", "content": "Done."}}})

# Terminal response
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})

# Phase 4: session/close (best-effort read)
try:
    recv()
except Exception:
    pass

sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server script that completes the handshake but
    /// returns a JSON-RPC error for `session/prompt`, triggering `AgentStatus::Failed`.
    pub fn create_fail_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_fail.py");
        let code = r#"#!/usr/bin/env python3
import sys, json

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

# Phase 1: initialize
msg = recv()
if msg is None:
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"protocolVersion": 1}})

# Phase 2: session/new
msg = recv()
if msg is None:
    sys.exit(1)
session_id = "fixture-session-fail"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

# Phase 3: session/prompt — return error response
msg = recv()
if msg is None:
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"],
      "error": {"code": -32000, "message": "Agent execution failed (fixture)"}})

sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server script that overwrites a pre-existing
    /// canonical output file instead of creating a brand-new one.
    pub fn create_overwrite_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_overwrite.py");
        let code = r#"#!/usr/bin/env python3
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
if msg is None:
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"protocolVersion": 1}})

msg = recv()
if msg is None:
    sys.exit(1)
cwd = msg.get("params", {}).get("cwd", "/tmp")
session_id = "fixture-session-overwrite"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

artifact_path = os.path.join(cwd, "canonical.json")
with open(artifact_path, "w") as f:
    f.write('{"overwritten": true}\n')

send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server script that emits a CHAINWORKS_OUTPUT
    /// envelope over `session/update` without writing any filesystem artifact.
    pub fn create_envelope_only_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_envelope_only.py");
        let code = r#"#!/usr/bin/env python3
import sys, json

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
if msg is None:
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"protocolVersion": 1}})

msg = recv()
if msg is None:
    sys.exit(1)
session_id = "fixture-session-envelope"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

send({
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {
        "update": {
            "sessionUpdate": "agent_message_chunk",
            "message": {
                "content": [
                    {
                        "type": "text",
                        "text": "<<<CHAINWORKS_OUTPUT:proposal_review>>>{\"status\":\"green\"}<<<END_CHAINWORKS_OUTPUT>>>"
                    }
                ]
            }
        }
    }
})

send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server script that keeps a single session alive
    /// across two prompt turns and exits only after `session/close`.
    pub fn create_reuse_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_reuse.py");
        let code = r#"#!/usr/bin/env python3
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
if msg is None:
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"protocolVersion": 1}})

msg = recv()
if msg is None:
    sys.exit(1)
cwd = msg.get("params", {}).get("cwd", "/tmp")
session_id = "fixture-session-reuse"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)
with open(os.path.join(cwd, "first.json"), "w") as f:
    f.write('{"turn": 1}\n')
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)
with open(os.path.join(cwd, "second.json"), "w") as f:
    f.write('{"turn": 2}\n')
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})

msg = recv()
if msg is None or msg.get("method") != "session/close":
    sys.exit(1)
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server script that emits provider usage telemetry
    /// over a session/update notification before completing the prompt.
    pub fn create_usage_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_usage.py");
        let code = r#"#!/usr/bin/env python3
import sys, json

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
if msg is None:
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"protocolVersion": 1}})

msg = recv()
if msg is None:
    sys.exit(1)
session_id = "fixture-session-usage"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

send({
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {
        "update": {
            "kind": "usage",
            "usage": {
                "cost_cents": 42,
                "input_tokens": 60000,
                "cached_input_tokens": 6000,
                "output_tokens": 1200,
                "model_context_window": 200000
            }
        }
    }
})

send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }
}

/// ClaudeAgentAdapter drives the full ACP protocol and discovers artifacts
/// created by the agent via workspace diff.
#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_executes_subprocess_and_returns_artifacts() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_success_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);

    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "stage_test".into(),
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        // workspace_root == cwd the fixture receives; it creates result.json there
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "test prompt".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(
        result.status,
        AgentStatus::Completed,
        "adapter must report Completed when ACP session ends with stopReason=end_turn"
    );
    assert_eq!(
        result.artifact_paths.len(),
        1,
        "workspace diff must find exactly the one file created by the fixture"
    );
    assert!(
        result.artifact_paths[0].ends_with("result.json"),
        "discovered artifact must be result.json, got: {:?}",
        result.artifact_paths[0]
    );
}

/// ClaudeAgentAdapter returns AgentStatus::Failed when the ACP session returns
/// a JSON-RPC error response for session/prompt.
#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_returns_failed_on_session_error() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_fail_script(tmp.path());

    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "stage_fail".into(),
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "fail".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(
        result.status,
        AgentStatus::Failed,
        "ACP error response must produce AgentStatus::Failed"
    );
    assert!(
        result.artifact_paths.is_empty(),
        "failed session must have no artifact paths"
    );
}

/// GeminiCliAdapter drives the same ACP protocol through its adapter.
/// (The fixture is a plain executable so the --acp flag is harmlessly appended.)
#[cfg(unix)]
#[tokio::test]
async fn test_gemini_adapter_executes_subprocess_and_returns_artifacts() {
    use acp::adapters::gemini::GeminiCliAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();

    // Wrap the Python script in a shell wrapper that ignores the --acp flag
    // so the same fixture logic works regardless of how it's invoked.
    let py_script = fixture::create_success_script(tmp.path());
    let wrapper = tmp.path().join("gemini_wrapper.sh");
    std::fs::write(
        &wrapper,
        format!("#!/bin/sh\nexec python3 '{}'\n", py_script),
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(&wrapper).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&wrapper, p).unwrap();
    }

    let adapter = GeminiCliAdapter::new_with_binary(wrapper.to_str().unwrap());

    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "gemini_stage".into(),
        agent_id: "gemini-agent".into(),
        provider: "gemini".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "generate report".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert_eq!(result.artifact_paths.len(), 1);
    assert!(result.artifact_paths[0].ends_with("result.json"));
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_reports_expected_output_paths_when_overwriting_existing_file() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let existing = tmp.path().join("canonical.json");
    std::fs::write(&existing, "{\"stale\": true}\n").unwrap();

    let script = fixture::create_overwrite_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "stage_overwrite".into(),
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "overwrite canonical output".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: vec![existing.to_string_lossy().into_owned()],
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
    };

    let result = adapter.execute(req).await.unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result.artifact_paths
            .iter()
            .any(|path| path.ends_with("canonical.json")),
        "expected overwritten canonical output to be reported: {:?}",
        result.artifact_paths
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_extracts_chainworks_output_envelopes_without_filesystem_artifacts() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_envelope_only_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "stage_envelope".into(),
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "emit proposal review envelope".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result.artifact_paths.is_empty(),
        "envelope-only execution should not require filesystem artifacts: {:?}",
        result.artifact_paths
    );
    assert_eq!(result.discovered_artifacts.len(), 1);
    assert_eq!(result.discovered_artifacts[0].name, "proposal_review");
    assert_eq!(
        std::str::from_utf8(&result.discovered_artifacts[0].content).unwrap(),
        "{\"status\":\"green\"}"
    );
}

/// AcpRuntimeManager should keep a live session handle and reuse it for a
/// second prompt without starting a fresh ACP session.
#[cfg(unix)]
#[tokio::test]
async fn test_runtime_manager_reuses_live_session_handle() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::{AcpRuntimeManager, ExecutionRequest};
    use domain::ids::RunId;
    use std::sync::Arc;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_reuse_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let manager = AcpRuntimeManager::new_with_adapters(vec![Arc::new(adapter) as Arc<dyn AcpAdapter>]);

    let first_req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "stage_first".into(),
        agent_id: "reuse-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "first turn".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        keep_session_alive: true,
        reuse_existing_session: false,
        session_generation_id: Some("generation-1".into()),
        provider_session_id: None,
    };

    let first_result = manager.execute(first_req).await.unwrap();
    let session_generation_id = first_result
        .session_generation_id
        .clone()
        .expect("live session result should include a generation id");
    assert!(
        first_result
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("first.json")),
        "first prompt should discover first.json: {:?}",
        first_result.artifact_paths
    );

    let second_req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "stage_second".into(),
        agent_id: "reuse-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "second turn".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: true,
        session_generation_id: Some(session_generation_id.clone()),
        provider_session_id: None,
    };

    let second_result = manager.execute(second_req).await.unwrap();
    assert!(
        second_result
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("second.json")),
        "reused prompt should discover second.json: {:?}",
        second_result.artifact_paths
    );

    manager
        .close_session(&session_generation_id)
        .await
        .expect("manager should close the reused live session");
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_surfaces_usage_snapshot_from_stream_updates() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_usage_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);

    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "stage_usage".into(),
        agent_id: "usage-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "stream usage".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert_eq!(result.cost_cents, Some(42));
    let usage = result.usage.expect("usage snapshot should be surfaced");
    assert_eq!(usage.cost_cents, Some(42));
    assert_eq!(usage.input_tokens, Some(60_000));
    assert_eq!(usage.cached_input_tokens, Some(6_000));
    assert_eq!(usage.output_tokens, Some(1_200));
    assert_eq!(usage.model_context_window, Some(200_000));
}
