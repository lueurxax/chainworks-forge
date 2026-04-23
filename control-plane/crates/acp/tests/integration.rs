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

    /// Write a fixture ACP server script whose prompt stays active until the
    /// client sends session/close. This proves runtime cleanup can interrupt a
    /// one-shot session while it is still executing.
    pub fn create_close_during_prompt_script(
        tmpdir: &std::path::Path,
        prompt_marker_path: &std::path::Path,
        close_marker_path: &std::path::Path,
    ) -> String {
        let script = tmpdir.join("acp_close_during_prompt.py");
        let prompt_marker = prompt_marker_path.to_string_lossy();
        let close_marker = close_marker_path.to_string_lossy();
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, os

PROMPT_MARKER = {prompt_marker:?}
CLOSE_MARKER = {close_marker:?}

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
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": "fixture-close-during-prompt"}}}})

msg = recv()
if msg is None:
    sys.exit(1)
with open(PROMPT_MARKER, "w") as f:
    f.write("prompt-started\n")

while True:
    msg = recv()
    if msg is None:
        sys.exit(1)
    if msg.get("method") == "session/close":
        with open(CLOSE_MARKER, "w") as f:
            f.write("close-seen\n")
        sys.exit(0)
"#
        );
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

    /// Write a fixture ACP server script that completes session/new, then
    /// closes stdout during session/prompt while keeping stdin alive. The
    /// adapter should still send session/close before returning the prompt
    /// transport error.
    pub fn create_prompt_stdout_close_script(
        tmpdir: &std::path::Path,
        marker_path: &std::path::Path,
    ) -> String {
        let script = tmpdir.join("acp_prompt_stdout_close.py");
        let marker = marker_path.to_string_lossy();
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, os

MARKER = {marker:?}

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
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": "fixture-stdout-close"}}}})

msg = recv()
if msg is None:
    sys.exit(1)

os.close(sys.stdout.fileno())
close_msg = recv()
if close_msg and close_msg.get("method") == "session/close":
    with open(MARKER, "w") as f:
        f.write("closed\n")
"#
        );
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

    /// Write a fixture ACP server script that leaves an existing declared
    /// output untouched during `session/prompt`.
    pub fn create_noop_prompt_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_noop_prompt.py");
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
session_id = "fixture-session-noop"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server script that creates one file during
    /// initialize and one file during the prompt. The initialize-time file
    /// must be part of the post-handshake baseline, not a prompt artifact.
    pub fn create_initialize_artifact_script(
        tmpdir: &std::path::Path,
        initialize_artifact: &std::path::Path,
    ) -> String {
        let script = tmpdir.join("acp_initialize_artifact.py");
        let initialize_artifact = initialize_artifact.to_string_lossy();
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, os

INITIALIZE_ARTIFACT = {initialize_artifact:?}

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
with open(INITIALIZE_ARTIFACT, "w") as f:
    f.write('{{"phase": "initialize"}}\n')
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None:
    sys.exit(1)
cwd = msg.get("params", {{}}).get("cwd", "/tmp")
session_id = "fixture-session-initialize-artifact"
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": session_id}}}})

msg = recv()
if msg is None:
    sys.exit(1)
with open(os.path.join(cwd, "prompt_created.json"), "w") as f:
    f.write('{{"phase": "prompt"}}\n')
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": session_id}}}})
sys.exit(0)
"#
        );
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

    /// Write a fixture ACP server script that emits a JSON-object
    /// CHAINWORKS_OUTPUT envelope over `session/update`.
    pub fn create_json_object_envelope_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_json_object_envelope.py");
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
session_id = "fixture-session-json-envelope"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

payload = {
    "CHAINWORKS_OUTPUT": {
        "proposal_review": {"status": "green"},
        "/tmp/run/implementation/progress.md": {"seemingly_complete": True}
    }
}
send({
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {
        "update": {
            "sessionUpdate": "agent_message_chunk",
            "content": {"type": "text", "text": json.dumps(payload)}
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

    /// Write a fixture ACP server script that exits cleanly after the first
    /// prompt even though the manager was asked to keep the session alive.
    pub fn create_exits_after_first_prompt_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_exits_after_first_prompt.py");
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
session_id = "fixture-session-exits-after-first-prompt"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)
with open(os.path.join(cwd, "first.json"), "w") as f:
    f.write('{"turn": 1}\n')
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})

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
        stage_execution_id: None,
        stage_id: "stage_test".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
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
    let pre_initialize_latency_ms = result
        .acp_pre_initialize_local_latency_ms
        .expect("P053 fixture must report pre-initialize local latency");
    println!("observed acp_pre_initialize_local_latency_ms={pre_initialize_latency_ms}");
    assert!(
        result.artifact_paths[0].ends_with("result.json"),
        "discovered artifact must be result.json, got: {:?}",
        result.artifact_paths[0]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_legacy_broad_discovery_ignores_preexisting_files_on_first_prompt() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let stale = tmp.path().join("stale.json");
    std::fs::write(&stale, "{\"stale\": true}\n").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1200));

    let script = fixture::create_success_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);

    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_legacy_post_prompt_only".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "test prompt".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert_eq!(
        result.artifact_paths.len(),
        1,
        "legacy broad discovery should report only files modified by the prompt: {:?}",
        result.artifact_paths
    );
    assert!(result.artifact_paths[0].ends_with("result.json"));
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_keeps_legacy_broad_discovery_disabled_by_default() {
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
        stage_execution_id: None,
        stage_id: "stage_default_no_broad".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "test prompt".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
    };

    let result = adapter.execute(req).await.unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result.artifact_paths.is_empty(),
        "implicit broad discovery must be disabled unless the frozen policy opts in: {:?}",
        result.artifact_paths
    );
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "manual P053 reference-workspace latency spot-check; set CHAINWORKS_P053_REFERENCE_WORKSPACE_ROOT"]
async fn p053_manual_reference_workspace_pre_initialize_latency() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let workspace_root = std::env::var("CHAINWORKS_P053_REFERENCE_WORKSPACE_ROOT")
        .expect("set CHAINWORKS_P053_REFERENCE_WORKSPACE_ROOT to the reference workspace root");
    assert!(
        std::path::Path::new(&workspace_root).is_dir(),
        "reference workspace root must be a directory: {workspace_root}"
    );

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_noop_prompt_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);

    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "p053_manual_reference_workspace".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: workspace_root.clone(),
        prompt: "P053 manual reference workspace latency spot-check".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
    };

    let result = adapter.execute(req).await.unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result.artifact_paths.is_empty(),
        "manual reference check must not infer artifacts through broad discovery: {:?}",
        result.artifact_paths
    );
    let pre_initialize_latency_ms = result
        .acp_pre_initialize_local_latency_ms
        .expect("P053 manual check must report pre-initialize local latency");
    println!(
        "p053_manual_reference_workspace={workspace_root} acp_pre_initialize_local_latency_ms={pre_initialize_latency_ms}"
    );
    assert!(
        pre_initialize_latency_ms < 1000,
        "P053 pre-initialize local latency should stay below the reference target"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn mcp_servers_session_new_serialization_tests() {
    use acp::transport::{build_session_new_params, AcpSessionConfig};
    use acp::{AcpMcpServerPayload, ExecutionRequest, ResolvedMcpServerTransport};
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_test".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "test prompt".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: vec![AcpMcpServerPayload {
            id: "fs-runtime".into(),
            extension_id: "filesystem".into(),
            transport: ResolvedMcpServerTransport::Stdio {
                command: "mcp-filesystem".into(),
                args: vec!["--root".into(), "/tmp".into()],
                env: [("MCP_TOKEN".to_string(), "secret".to_string())]
                    .into_iter()
                    .collect(),
            },
        }],
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let captured = build_session_new_params(&req, &AcpSessionConfig::default()).unwrap();
    let server = &captured["mcpServers"][0];
    assert_eq!(server["name"], "fs-runtime");
    assert_eq!(server["command"], "mcp-filesystem");
    assert_eq!(server["args"][0], "--root");
    assert_eq!(server["env"][0]["name"], "MCP_TOKEN");
    assert_eq!(server["env"][0]["value"], "secret");
    assert!(server.get("id").is_none());
    assert!(server.get("extensionId").is_none());
    assert!(server.get("transport").is_none());
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
        stage_execution_id: None,
        stage_id: "stage_fail".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
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

#[cfg(unix)]
#[tokio::test]
async fn adapter_execute_closes_session_after_prompt_transport_error() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let close_marker = tmp.path().join("session_close_seen.txt");
    let script = fixture::create_prompt_stdout_close_script(tmp.path(), &close_marker);

    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_prompt_transport_error".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "trigger stdout close".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let error = adapter
        .execute(req)
        .await
        .expect_err("closed stdout during prompt should surface a transport error");

    assert!(
        error
            .to_string()
            .contains("ACP stdout closed before terminal response"),
        "unexpected prompt error: {error:#}"
    );
    assert!(
        close_marker.is_file(),
        "adapter must send session/close even after prompt transport errors"
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
        stage_execution_id: None,
        stage_id: "gemini_stage".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
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
        stage_execution_id: None,
        stage_id: "stage_overwrite".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let result = adapter.execute(req).await.unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("canonical.json")),
        "expected overwritten canonical output to be reported: {:?}",
        result.artifact_paths
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_does_not_report_unchanged_expected_output_path() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let existing = tmp.path().join("canonical.json");
    std::fs::write(&existing, "{\"stale\": true}\n").unwrap();

    let script = fixture::create_noop_prompt_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_stale_expected_output".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "leave canonical output untouched".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: vec![existing.to_string_lossy().into_owned()],
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let result = adapter.execute(req).await.unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result.artifact_paths.is_empty(),
        "unchanged expected output path should not be reported as current prompt output: {:?}",
        result.artifact_paths
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_prefers_typed_expected_outputs_for_baseline_capture() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::discovery::{
        AuthorizedRoot, ExpectedOutputRole, ExpectedOutputSpec, ExpectedPathBaselineStatus,
        OutputReusePolicy, OutputRootClass, SourceGenerationOwner,
    };
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let stale_legacy_path = tmp.path().join("legacy-stale.json");
    std::fs::write(&stale_legacy_path, "{\"stale\": true}\n").unwrap();
    let typed_path = tmp.path().join("result.json");
    std::fs::write(&typed_path, "{\"ok\": false}\n").unwrap();

    let script = fixture::create_success_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let typed_path_string = typed_path.to_string_lossy().into_owned();
    let stale_legacy_path_string = stale_legacy_path.to_string_lossy().into_owned();
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: Some("stage-exec-typed".into()),
        stage_id: "stage_typed_expected_outputs".into(),
        attempt_number: 2,
        agent_execution_id: Some("agent-exec-typed".into()),
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: workspace_root.clone(),
        prompt: "overwrite typed expected output".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: vec![stale_legacy_path_string.clone()],
        expected_outputs: vec![ExpectedOutputSpec {
            output_name: "result".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: typed_path_string.clone(),
            companion_of: None,
            display_label: "result".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: OutputReusePolicy::MustProduce,
            max_bytes: 10 * 1024 * 1024,
            aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
            authorized_roots: vec![AuthorizedRoot {
                root_class: OutputRootClass::Workspace,
                root_path: workspace_root,
            }],
            source_generation_owner: SourceGenerationOwner::Agent,
        }],
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let result = adapter.execute(req).await.unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result.artifact_paths.iter().any(|path| path == &typed_path_string),
        "changed typed expected output should be reported even when the legacy path list is stale: {:?}",
        result.artifact_paths
    );
    assert!(
        !result
            .artifact_paths
            .iter()
            .any(|path| path == &stale_legacy_path_string),
        "unchanged legacy-only path should not be reported: {:?}",
        result.artifact_paths
    );
    assert_eq!(result.pre_prompt_expected_outputs.len(), 1);
    assert_eq!(result.pre_prompt_expected_outputs[0].output_name, "result");
    assert_eq!(result.pre_prompt_expected_outputs[0].attempt_number, 2);
    assert_eq!(
        result.pre_prompt_expected_outputs[0].agent_execution_id,
        "agent-exec-typed"
    );
    assert_eq!(
        result.pre_prompt_expected_outputs[0].stage_execution_id,
        "stage-exec-typed"
    );
    assert_eq!(
        result.pre_prompt_expected_outputs[0].baseline_status,
        ExpectedPathBaselineStatus::RegularContentCaptured
    );
    assert!(result.pre_prompt_expected_outputs[0]
        .content_digest
        .as_deref()
        .is_some_and(|digest| digest.starts_with("sha256:")));
}

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_excludes_initialize_created_file_from_prompt_artifacts() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let initialize_artifact = tmp.path().join("initialize_created.json");
    let script = fixture::create_initialize_artifact_script(tmp.path(), &initialize_artifact);
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_initialize_artifact".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "create prompt artifact".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let result = adapter.execute(req).await.unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("prompt_created.json")),
        "prompt-created artifact should still be reported: {:?}",
        result.artifact_paths
    );
    assert!(
        !result
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("initialize_created.json")),
        "initialize-created artifact must be part of the post-handshake baseline: {:?}",
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
        stage_execution_id: None,
        stage_id: "stage_envelope".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
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

#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_extracts_json_object_chainworks_output_envelope() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_json_object_envelope_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_json_envelope".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "emit json object envelope".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert!(result.artifact_paths.is_empty());
    assert_eq!(result.discovered_artifacts.len(), 2);
    assert!(result
        .discovered_artifacts
        .iter()
        .any(|artifact| artifact.name == "proposal_review"
            && artifact.content == br#"{"status":"green"}"#));
    assert!(result
        .discovered_artifacts
        .iter()
        .any(
            |artifact| artifact.name == "/tmp/run/implementation/progress.md"
                && artifact.content == br#"{"seemingly_complete":true}"#
        ));
    assert!(
        result
            .transcript_text
            .as_deref()
            .is_some_and(|text| text.contains("CHAINWORKS_OUTPUT")),
        "stream transcript should preserve the emitted JSON envelope"
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
    let manager =
        AcpRuntimeManager::new_with_adapters(vec![Arc::new(adapter) as Arc<dyn AcpAdapter>]);

    let first_req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_first".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: true,
        reuse_existing_session: false,
        session_generation_id: Some("generation-1".into()),
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
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
        stage_execution_id: None,
        stage_id: "stage_second".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: true,
        session_generation_id: Some(session_generation_id.clone()),
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
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
async fn test_runtime_manager_closes_inflight_one_shot_session_by_generation_id() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::{AcpRuntimeManager, ExecutionRequest};
    use domain::ids::RunId;
    use std::sync::Arc;
    use std::time::Duration;

    let tmp = tempfile::tempdir().unwrap();
    let prompt_marker = tmp.path().join("prompt-started.txt");
    let close_marker = tmp.path().join("close-seen.txt");
    let script =
        fixture::create_close_during_prompt_script(tmp.path(), &prompt_marker, &close_marker);
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let manager = Arc::new(AcpRuntimeManager::new_with_adapters(vec![
        Arc::new(adapter) as Arc<dyn AcpAdapter>,
    ]));
    let generation_id = "generation-one-shot-close";

    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_one_shot".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "one-shot-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "stay running until close".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: Some(generation_id.into()),
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: Default::default(),
    };

    let execution = {
        let manager = Arc::clone(&manager);
        tokio::spawn(async move { manager.execute(req).await })
    };
    tokio::time::timeout(Duration::from_secs(10), async {
        while !prompt_marker.is_file() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture should enter session/prompt");

    manager
        .close_session(generation_id)
        .await
        .expect("manager should close an in-flight one-shot session by generation id");

    let error = execution
        .await
        .expect("execution task should not panic")
        .expect_err("interrupted prompt should return an execution error");
    assert!(
        error.to_string().contains("closed during active prompt"),
        "unexpected interrupted prompt error: {error:#}"
    );
    assert!(
        close_marker.is_file(),
        "fixture must receive session/close before the one-shot subprocess exits"
    );
    assert!(
        !manager.has_live_session(generation_id, None).await,
        "one-shot generation handle should be removed after close"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_runtime_manager_healthcheck_rejects_exited_live_session() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::{AcpRuntimeManager, ExecutionRequest};
    use domain::ids::RunId;
    use std::sync::Arc;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_exits_after_first_prompt_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let manager =
        AcpRuntimeManager::new_with_adapters(vec![Arc::new(adapter) as Arc<dyn AcpAdapter>]);

    let first_req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_first".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: true,
        reuse_existing_session: false,
        session_generation_id: Some("generation-1".into()),
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
    };

    let first_result = manager.execute(first_req).await.unwrap();
    let session_generation_id = first_result
        .session_generation_id
        .clone()
        .expect("live session result should include a generation id");
    let provider_session_id = first_result.provider_session_id.clone();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert!(
        !manager
            .has_live_session(&session_generation_id, provider_session_id.as_deref())
            .await,
        "exited ACP subprocess must not pass live-session reuse healthcheck"
    );

    let reuse_req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_second".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: true,
        session_generation_id: Some(session_generation_id),
        provider_session_id,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
    };

    let error = manager.execute(reuse_req).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("No live ACP session registered for generation id"),
        "stale live handle should be removed before prompt reuse: {error}"
    );
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
        stage_execution_id: None,
        stage_id: "stage_usage".into(),
        attempt_number: 1,
        agent_execution_id: None,
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
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
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

// ---------------------------------------------------------------------------
// P050: Non-Codex adapter receives CHAINWORKS_META_ROOT env var
// ---------------------------------------------------------------------------

/// P050 proof: Claude adapter subprocess receives CHAINWORKS_META_ROOT env var
/// when ExecutionRequest.chainworks_meta_root is set.
#[cfg(unix)]
#[tokio::test]
async fn test_claude_adapter_receives_chainworks_meta_root_env() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::ids::RunId;

    // Create a fixture that writes the CHAINWORKS_META_ROOT env to a file.
    let tmp = tempfile::tempdir().unwrap();
    let env_probe_path = tmp.path().join("meta_root_env.txt");
    let script = tmp.path().join("acp_env_probe.py");
    let code = format!(
        r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line: return None
    try: return json.loads(line.strip())
    except: return None

# Record the env var before any protocol
meta_root = os.environ.get('CHAINWORKS_META_ROOT', '<NOT SET>')
with open('{}', 'w') as f:
    f.write(meta_root)

msg = recv()
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})
msg = recv()
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": "probe"}}}})
msg = recv()
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": "probe"}}}})
sys.exit(0)
"#,
        env_probe_path.to_string_lossy().replace('\\', "\\\\")
    );
    std::fs::write(&script, code).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
    }

    let adapter = ClaudeAgentAdapter::new_with_binary(script.to_str().unwrap());
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "env_probe".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "env-probe-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "probe env".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: vec![],
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: vec![],
        chainworks_meta_root: Some(".chainworks/runs/env-test-run".into()),
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
    };

    let _ = adapter.execute(req).await;

    // The fixture wrote the env var value to a file.
    let recorded =
        std::fs::read_to_string(&env_probe_path).unwrap_or_else(|_| "<file not found>".into());
    assert!(
        recorded.contains("/.chainworks/runs/env-test-run"),
        "Claude adapter subprocess must receive CHAINWORKS_META_ROOT env var, got: {recorded}"
    );
}
