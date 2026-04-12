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
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert_eq!(result.artifact_paths.len(), 1);
    assert!(result.artifact_paths[0].ends_with("result.json"));
}
