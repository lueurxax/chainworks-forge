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

    /// Write a fixture ACP server that emits prompt progress, waits briefly,
    /// and only then sends the terminal `session/prompt` response. Tests use
    /// this to prove live prompt activity is observable before settlement.
    pub fn create_delayed_prompt_progress_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_delayed_prompt_progress.py");
        let code = r#"#!/usr/bin/env python3
import sys, json, os, time

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
send({"jsonrpc": "2.0", "id": msg["id"],
      "result": {"protocolVersion": 1, "serverInfo": {"name": "progress-fixture", "version": "0.0.1"}}})

msg = recv()
if msg is None:
    sys.exit(1)
cwd = msg.get("params", {}).get("cwd", "/tmp")
session_id = "fixture-session-delayed-progress"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

send({"jsonrpc": "2.0", "method": "session/update",
      "params": {"update": {"sessionUpdate": "agent_message_chunk", "content": "working..."}}})
time.sleep(2.0)

artifact_path = os.path.join(cwd, "result.json")
with open(artifact_path, "w") as f:
    f.write('{"ok": true}\n')
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})

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

    pub fn create_set_mode_script(
        tmpdir: &std::path::Path,
        observed_mode_path: &std::path::Path,
    ) -> String {
        let script = tmpdir.join("acp_set_mode.py");
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, pathlib

observed_mode_path = pathlib.Path({observed_mode_path:?})

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
    return json.loads(stripped)

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"protocolVersion": 1}}}})

msg = recv()
if msg is None or msg.get("method") != "session/new":
    sys.exit(1)
session_id = "fixture-session-set-mode"
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": session_id}}}})

msg = recv()
if msg is None or msg.get("method") != "session/set_mode":
    sys.exit(1)
observed_mode_path.write_text(msg.get("params", {{}}).get("modeId", ""))
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{}}}})

msg = recv()
if msg is None or msg.get("method") != "session/prompt":
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": session_id}}}})

recv()
sys.exit(0)
"#,
            observed_mode_path = observed_mode_path.to_string_lossy().to_string(),
        );
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    pub fn create_reject_required_config_option_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_reject_required_config.py");
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
send({"jsonrpc": "2.0", "id": msg["id"],
      "result": {"protocolVersion": 1, "serverInfo": {"name": "required-config-fixture", "version": "0.0.1"}}})

msg = recv()
if msg is None:
    sys.exit(1)
session_id = "fixture-session-required-config"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None or msg.get("method") != "session/set_config_option":
    sys.exit(1)
send({"jsonrpc": "2.0", "id": msg["id"],
      "error": {"code": -32000, "message": "model unavailable"}})
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    pub fn create_model_config_alias_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_model_config_alias.py");
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
send({"jsonrpc": "2.0", "id": msg["id"],
      "result": {"protocolVersion": 1, "serverInfo": {"name": "model-config-fixture", "version": "0.0.1"}}})

msg = recv()
if msg is None:
    sys.exit(1)
session_id = "fixture-session-model-config"
send({"jsonrpc": "2.0", "id": msg["id"],
      "result": {
          "sessionId": session_id,
          "configOptions": [
              {"id": "model", "name": "Model", "options": [
                  {"value": "default", "name": "Default (recommended)", "description": "Opus 4.7 with 1M context - Most capable for complex work"},
                  {"value": "sonnet", "name": "Sonnet", "description": "Sonnet 4.6 - Best for everyday tasks"},
                  {"value": "haiku", "name": "Haiku", "description": "Haiku 4.5 - Fastest for quick answers"}
              ]}
          ]
      }})

msg = recv()
if msg is None or msg.get("method") != "session/set_config_option":
    sys.exit(1)
params = msg.get("params", {})
if params.get("configId") != "model" or params.get("value") != "default":
    send({"jsonrpc": "2.0", "id": msg["id"],
          "error": {"code": -32000, "message": "unexpected config value", "data": params}})
    sys.exit(0)
send({"jsonrpc": "2.0", "id": msg["id"], "result": {}})
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server that emits duplicate session/update chunks
    /// containing a residual absolute Xcode command path.
    pub fn create_residual_xcode_warning_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_residual_xcode_warning.py");
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
session_id = "fixture-session-residual-xcode-warning"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

warning = {
    "jsonrpc": "2.0",
    "method": "session/update",
    "params": {
        "update": {
            "sessionUpdate": "tool_call",
            "content": "running /usr/bin/xcrun simctl list devices from provider shell"
        }
    }
}
send(warning)
send(warning)
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": session_id}})

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
send({"jsonrpc": "2.0", "id": msg["id"], "result": {
    "protocolVersion": 1,
    "mcpCapabilities": {"http": True}
}})

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

    /// Write a fixture ACP server that enters `session/prompt` and waits for
    /// `session/close` instead of sending a terminal prompt response.
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
import sys, json

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
session_id = "fixture-close-during-prompt"
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": session_id}}}})

msg = recv()
if msg is None:
    sys.exit(1)
with open(PROMPT_MARKER, "w") as f:
    f.write("prompt-started\n")

while True:
    close_msg = recv()
    if close_msg is None:
        break
    if close_msg.get("method") == "session/close":
        with open(CLOSE_MARKER, "w") as f:
            f.write("close-seen\n")
        break

sys.exit(0)
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

    /// Write a fixture ACP server script that emits a CHAINWORKS_OUTPUT
    /// envelope only in the terminal session/prompt result output field.
    pub fn create_terminal_output_envelope_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_terminal_output_envelope.py");
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
session_id = "fixture-session-terminal-output-envelope"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

send({
    "jsonrpc": "2.0",
    "id": msg["id"],
    "result": {
        "stopReason": "end_turn",
        "sessionId": session_id,
        "output": [
            {
                "type": "text",
                "text": "<<<CHAINWORKS_OUTPUT:proposal_review>>>{\"status\":\"green\"}<<<END_CHAINWORKS_OUTPUT>>>"
            }
        ]
    }
})
sys.exit(0)
"#;
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture ACP server script that emits P084-like required
    /// outputs as stringified JSON in the terminal session/prompt result
    /// output field.
    pub fn create_p084_like_stringified_terminal_output_script(tmpdir: &std::path::Path) -> String {
        let script = tmpdir.join("acp_p084_like_stringified_terminal_output.py");
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
session_id = "fixture-session-p084-like-stringified-output"
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": session_id}})

msg = recv()
if msg is None:
    sys.exit(1)

payload = {
    "CHAINWORKS_OUTPUT": {
        "implementation_progress": {
            "status": "in_progress",
            "summary": "P084 retry reached output materialization",
            "completed_tasks": [],
            "remaining_tasks": ["finish rollout readback"]
        },
        "implementation_self_assessment": {
            "status": "needs_code_fixes",
            "seemingly_complete": False,
            "remaining_code_tasks": [],
            "handoff_tasks": [],
            "known_risks": [],
            "tests_run": [],
            "docs_impacted": [],
            "verification_green": True
        },
        "tests_result": {
            "status": "passed",
            "commands": ["./scripts/test-gate.sh proposal-084"],
            "failures": []
        }
    }
}
stringified_payload = json.dumps(json.dumps(payload, separators=(",", ":")))
send({
    "jsonrpc": "2.0",
    "id": msg["id"],
    "result": {
        "stopReason": "end_turn",
        "sessionId": session_id,
        "output": [
            {
                "type": "text",
                "text": stringified_payload
            }
        ]
    }
})
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

    /// Write a fixture that supports only the ACP initialize probe. It records
    /// every initialize request and advertises the requested HTTP MCP flag.
    pub fn create_capability_probe_script(tmpdir: &std::path::Path, http_mcp: bool) -> String {
        let script = tmpdir.join(if http_mcp {
            "acp_probe_http.py"
        } else {
            "acp_probe_stdio.py"
        });
        let marker = tmpdir.join(if http_mcp {
            "probe-http-count.txt"
        } else {
            "probe-stdio-count.txt"
        });
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, pathlib

marker = pathlib.Path({marker:?})

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

line = sys.stdin.readline()
if not line:
    sys.exit(1)
msg = json.loads(line)
with marker.open("a") as f:
    f.write("initialize\n")
send({{
    "jsonrpc": "2.0",
    "id": msg["id"],
    "result": {{
        "protocolVersion": 1,
        "serverInfo": {{"name": "probe-fixture", "version": "0.0.1"}},
        "mcpCapabilities": {{"http": {http_mcp}}}
    }}
}})

for _ in sys.stdin:
    pass
sys.exit(0)
"#,
            marker = marker.to_string_lossy(),
            http_mcp = if http_mcp { "True" } else { "False" },
        );
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Write a fixture that advertises HTTP MCP during the probe process, then
    /// captures the real `session/new` params when the manager starts the
    /// provider after broker lease attachment.
    pub fn create_broker_attach_session_script(
        tmpdir: &std::path::Path,
        capture_path: &std::path::Path,
    ) -> String {
        let script = tmpdir.join("acp_broker_attach.py");
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, pathlib

capture_path = pathlib.Path({capture_path:?})

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
    return json.loads(stripped)

msg = recv()
if msg is None:
    sys.exit(1)
send({{
    "jsonrpc": "2.0",
    "id": msg["id"],
    "result": {{
        "protocolVersion": 1,
        "serverInfo": {{"name": "broker-attach-fixture", "version": "0.0.1"}},
        "mcpCapabilities": {{"http": True}}
    }}
}})

msg = recv()
if msg is None:
    sys.exit(0)
capture_path.write_text(json.dumps(msg.get("params", {{}})))
session_id = "fixture-session-broker-attach"
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": session_id}}}})

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": session_id}}}})

try:
    recv()
except Exception:
    pass

sys.exit(0)
"#,
            capture_path = capture_path.to_string_lossy(),
        );
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Like `create_broker_attach_session_script`, but fails the real
    /// `session/new` path unless the broker warmup marker exists first.
    pub fn create_broker_attach_requires_warmup_script(
        tmpdir: &std::path::Path,
        capture_path: &std::path::Path,
        warmup_marker_path: &std::path::Path,
    ) -> String {
        let script = tmpdir.join("acp_broker_attach_requires_warmup.py");
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, pathlib

capture_path = pathlib.Path({capture_path:?})
warmup_marker_path = pathlib.Path({warmup_marker_path:?})

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
    return json.loads(stripped)

msg = recv()
if msg is None:
    sys.exit(1)
send({{
    "jsonrpc": "2.0",
    "id": msg["id"],
    "result": {{
        "protocolVersion": 1,
        "serverInfo": {{"name": "broker-attach-warmup-fixture", "version": "0.0.1"}},
        "mcpCapabilities": {{"http": True}}
    }}
}})

msg = recv()
if msg is None:
    sys.exit(0)
if not warmup_marker_path.exists():
    send({{"jsonrpc": "2.0", "id": msg["id"], "error": {{"code": -32000, "message": "warmup marker missing before session/new"}}}})
    sys.exit(2)
capture_path.write_text(json.dumps(msg.get("params", {{}})))
session_id = "fixture-session-broker-attach-warmup"
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": session_id}}}})

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": session_id}}}})

try:
    recv()
except Exception:
    pass

sys.exit(0)
"#,
            capture_path = capture_path.to_string_lossy(),
            warmup_marker_path = warmup_marker_path.to_string_lossy(),
        );
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }

    /// Like `create_broker_attach_session_script`, but keeps the session
    /// process alive during `session/close` until the test creates
    /// `allow_close_path`.
    pub fn create_broker_attach_blocking_close_script(
        tmpdir: &std::path::Path,
        close_seen_path: &std::path::Path,
        allow_close_path: &std::path::Path,
    ) -> String {
        let script = tmpdir.join("acp_broker_attach_blocking_close.py");
        let code = format!(
            r#"#!/usr/bin/env python3
import sys, json, pathlib, time

close_seen_path = pathlib.Path({close_seen_path:?})
allow_close_path = pathlib.Path({allow_close_path:?})

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
    return json.loads(stripped)

msg = recv()
if msg is None:
    sys.exit(1)
send({{
    "jsonrpc": "2.0",
    "id": msg["id"],
    "result": {{
        "protocolVersion": 1,
        "serverInfo": {{"name": "broker-attach-blocking-close", "version": "0.0.1"}},
        "mcpCapabilities": {{"http": True}}
    }}
}})

msg = recv()
if msg is None:
    sys.exit(1)
session_id = "fixture-session-" + str(msg["id"])
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"sessionId": session_id}}}})

msg = recv()
if msg is None:
    sys.exit(1)
send({{"jsonrpc": "2.0", "id": msg["id"], "result": {{"stopReason": "end_turn", "sessionId": session_id}}}})

msg = recv()
if msg is not None and msg.get("method") == "session/close":
    with close_seen_path.open("a") as f:
        f.write(session_id + "\n")
    while not allow_close_path.exists():
        time.sleep(0.02)

sys.exit(0)
"#,
            close_seen_path = close_seen_path.to_string_lossy(),
            allow_close_path = allow_close_path.to_string_lossy(),
        );
        std::fs::write(&script, code).unwrap();
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
        script.to_string_lossy().into_owned()
    }
}

#[cfg(unix)]
fn brokered_xcode_request(tmp: &tempfile::TempDir, provider: &str) -> acp::ExecutionRequest {
    acp::ExecutionRequest {
        run_id: domain::ids::RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_xcode".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "xcode-agent".into(),
        provider: provider.into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "use xcode".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: vec![acp::AcpMcpServerPayload {
            id: "xcode-broker".into(),
            extension_id: "xcode".into(),
            transport: acp::ResolvedMcpServerTransport::XcodeBrokerIntent {
                intent: acp::BrokeredXcodeMcpIntent {
                    extension_id: "xcode".into(),
                    runtime_id: "xcode-broker".into(),
                    server_id: "xcode".into(),
                    workspace_root: None,
                    xcode_pid_selector: None,
                    runtime_profile_id: Some("profile-xcode".into()),
                    permission_profile_id: None,
                    resolved_tool_allowlist_hash: None,
                    provider_http_required: true,
                },
            },
        }],
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };

    let captured = build_session_new_params(&req, &AcpSessionConfig::default()).unwrap();
    let server = &captured["mcpServers"][0];
    assert_eq!(server["name"], "fs-runtime");
    assert_eq!(server["command"], "mcp-filesystem");
    assert_eq!(server["args"][0], "--root");
    assert_eq!(server["env"][0]["name"], "MCP_TOKEN");
    assert_eq!(server["env"][0]["value"], "secret");
    assert_eq!(server["type"], "stdio");
    assert!(server.get("id").is_none());
    assert!(server.get("extensionId").is_none());
    assert!(server.get("transport").is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn http_mcp_servers_session_new_serialization_tests() {
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
        provider: "codex".into(),
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
            id: "xcode-broker".into(),
            extension_id: "xcode".into(),
            transport: ResolvedMcpServerTransport::Http {
                url: "http://127.0.0.1:8123/xcode-mcp/lease-1".into(),
                headers: [("Authorization".to_string(), "Bearer redacted".to_string())]
                    .into_iter()
                    .collect(),
            },
        }],
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };

    let captured = build_session_new_params(&req, &AcpSessionConfig::default()).unwrap();
    let server = &captured["mcpServers"][0];
    assert_eq!(server["name"], "xcode-broker");
    assert_eq!(server["type"], "http");
    assert_eq!(server["url"], "http://127.0.0.1:8123/xcode-mcp/lease-1");
    assert_eq!(server["headers"][0]["name"], "Authorization");
    assert_eq!(server["headers"][0]["value"], "Bearer redacted");
    assert!(server.get("command").is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn transport_sends_set_mode_after_session_new_when_configured() {
    use acp::transport::{AcpSessionConfig, AcpTransportSession};
    use acp::ExecutionRequest;
    use domain::ids::RunId;
    use std::process::Stdio;
    use tokio::process::Command;

    let tmp = tempfile::tempdir().unwrap();
    let observed_mode_path = tmp.path().join("observed-mode.txt");
    let script = fixture::create_set_mode_script(tmp.path(), &observed_mode_path);
    let child = Command::new(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
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
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };
    let config = AcpSessionConfig {
        set_mode_after_session_new: true,
        ..AcpSessionConfig::default()
    };

    let mut session = AcpTransportSession::start(child, &req, &config)
        .await
        .unwrap();
    let result = session.prompt(&req).await.unwrap();
    assert_eq!(result.0, domain::agent::AgentStatus::Completed);
    session.close().await.unwrap();
    assert_eq!(
        std::fs::read_to_string(observed_mode_path).unwrap(),
        "bypassPermissions"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn transport_fails_when_required_config_option_is_rejected() {
    use acp::transport::{AcpSessionConfig, AcpTransportSession};
    use acp::ExecutionRequest;
    use domain::ids::RunId;
    use std::process::Stdio;
    use tokio::process::Command;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_reject_required_config_option_script(tmp.path());
    let child = Command::new(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_test".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: Some("haiku".into()),
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };
    let config = AcpSessionConfig {
        required_config_options: vec![("model".to_string(), "haiku".to_string())],
        ..AcpSessionConfig::default()
    };

    let err = match AcpTransportSession::start(child, &req, &config).await {
        Ok(_) => panic!("required model config rejection must fail session startup"),
        Err(err) => err,
    };
    assert!(
        err.to_string()
            .contains("required session/set_config_option rejected for model"),
        "unexpected error: {err:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn transport_resolves_required_model_alias_from_session_config_options() {
    use acp::transport::{AcpSessionConfig, AcpTransportSession};
    use acp::ExecutionRequest;
    use domain::ids::RunId;
    use std::process::Stdio;
    use tokio::process::Command;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_model_config_alias_script(tmp.path());
    let child = Command::new(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_test".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: Some("opus".into()),
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };
    let config = AcpSessionConfig {
        required_config_options: vec![("model".to_string(), "opus".to_string())],
        ..AcpSessionConfig::default()
    };

    AcpTransportSession::start(child, &req, &config)
        .await
        .expect("model alias should resolve to the provider-advertised option value");
}

#[cfg(unix)]
#[tokio::test]
async fn adapter_launch_and_session_specs_are_prepared_separately() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::codex::CodexAdapter;
    use acp::adapters::gemini::GeminiCliAdapter;
    use acp::adapters::{AcpAdapter, LaunchResourceGuard};
    use acp::ExecutionRequest;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_test".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "codex".into(),
        model: Some("gpt-5.4/high".into()),
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };

    let codex = CodexAdapter::new_with_binary("/bin/codex-fixture");
    let session_spec = codex.prepare_session_new_spec(&req).unwrap();
    assert_eq!(session_spec.model, "gpt-5.4");
    assert_eq!(session_spec.mode, "full-access");
    assert_eq!(
        session_spec.config_options,
        vec![("reasoning_effort".to_string(), "high".to_string())]
    );

    let mut resources = LaunchResourceGuard::default();
    let launch_spec = codex.prepare_launch_spec(&req, &mut resources).unwrap();
    assert_eq!(launch_spec.binary_path, "/bin/codex-fixture");
    assert!(launch_spec
        .env
        .iter()
        .any(|(name, value)| name == "RUST_LOG" && value == "warn"));
    let cleanup_paths = resources.commit();
    assert_eq!(cleanup_paths.len(), 1);
    assert!(cleanup_paths[0].exists());
    std::fs::remove_dir_all(&cleanup_paths[0]).unwrap();

    let gemini = GeminiCliAdapter::new_with_binary("/bin/gemini-fixture");
    let mut resources = LaunchResourceGuard::default();
    let launch_spec = gemini.prepare_launch_spec(&req, &mut resources).unwrap();
    assert_eq!(launch_spec.binary_path, "/bin/gemini-fixture");
    assert_eq!(launch_spec.args, vec!["--acp"]);
    assert!(resources.commit().is_empty());

    let claude = ClaudeAgentAdapter::new_with_binary("/bin/claude-fixture");
    for model in ["sonnet", "haiku"] {
        let mut claude_req = req.clone();
        claude_req.provider = "claude".into();
        claude_req.model = Some(model.into());
        let session_spec = claude.prepare_session_new_spec(&claude_req).unwrap();
        assert_eq!(session_spec.model, model);
        assert!(
            session_spec
                .required_config_options
                .contains(&("model".to_string(), model.to_string())),
            "Claude ACP ignores top-level session/new model on current adapters; Chainworks must enforce the requested model through session/set_config_option"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn launch_resource_guard_rolls_back_uncommitted_paths() {
    use acp::adapters::LaunchResourceGuard;

    let tmp = tempfile::tempdir().unwrap();
    let rollback_path = tmp.path().join("runtime-home");
    std::fs::create_dir_all(&rollback_path).unwrap();
    {
        let mut guard = LaunchResourceGuard::default();
        guard.add_cleanup_path(&rollback_path);
    }
    assert!(!rollback_path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn launch_resources_are_cleaned_when_spawn_fails() {
    use acp::adapters::codex::CodexAdapter;
    use acp::adapters::{AcpAdapter, LaunchResourceGuard};
    use acp::ExecutionRequest;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let missing_binary = tmp.path().join("missing-codex-acp");
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_test".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "codex".into(),
        model: Some("gpt-5.4".into()),
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };

    let adapter = CodexAdapter::new_with_binary(missing_binary.to_string_lossy().into_owned());
    let mut resources = LaunchResourceGuard::default();
    let mut launch_spec = adapter.prepare_launch_spec(&req, &mut resources).unwrap();
    let cleanup_paths = resources.commit();
    let cleanup_path = cleanup_paths[0].clone();
    assert!(cleanup_path.exists());
    launch_spec.cleanup_paths.extend(cleanup_paths);

    let session_spec = adapter.prepare_session_new_spec(&req).unwrap();
    let err = match adapter
        .open_session_with_specs(&req, launch_spec, session_spec)
        .await
    {
        Ok(_) => panic!("missing binary should fail before opening an ACP session"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("spawn codex ACP subprocess"));
    assert!(!cleanup_path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn brokered_xcode_probe_fails_closed_when_provider_lacks_http_mcp() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_capability_probe_script(tmp.path(), false);
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = brokered_xcode_request(&tmp, "claude");

    let err = match adapter.open_session(&req).await {
        Ok(_) => panic!("stdio-only provider must not receive brokered Xcode MCP"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("provider_http_mcp_unsupported"),
        "unexpected error: {err:#}"
    );
    let marker = tmp.path().join("probe-stdio-count.txt");
    let count = std::fs::read_to_string(marker).unwrap();
    assert_eq!(count.lines().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn brokered_xcode_probe_accepts_http_but_requires_lease_conversion() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_capability_probe_script(tmp.path(), true);
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = brokered_xcode_request(&tmp, "claude");

    let err = match adapter.open_session(&req).await {
        Ok(_) => panic!("broker intent must be converted before session/new"),
        Err(err) => err,
    };

    assert!(
        err.to_string()
            .contains("must be converted to an HTTP lease before session/new"),
        "unexpected error: {err:#}"
    );
    let marker = tmp.path().join("probe-http-count.txt");
    let count = std::fs::read_to_string(marker).unwrap();
    assert_eq!(
        count.lines().count(),
        1,
        "successful capability probe must not fall through into a real provider session"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn brokered_xcode_capability_cache_avoids_repeated_probe() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::{AcpAdapter, ProviderCapabilityCache};

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_capability_probe_script(tmp.path(), true);
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let cache = ProviderCapabilityCache::default();
    let req = brokered_xcode_request(&tmp, "claude");

    for _ in 0..2 {
        let err = match adapter
            .open_session_with_capability_cache(&req, &cache)
            .await
        {
            Ok(_) => panic!("broker intent must be converted before session/new"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("must be converted to an HTTP lease before session/new"),
            "unexpected error: {err:#}"
        );
    }

    let marker = tmp.path().join("probe-http-count.txt");
    let count = std::fs::read_to_string(marker).unwrap();
    assert_eq!(count.lines().count(), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_attaches_capacity_checks_and_releases_leases() {
    use acp::{
        ResolvedMcpServerTransport, XcodeBrokerLeaseAttacher, XcodeMcpBridgePool,
        XcodeMcpBridgePoolConfig, XcodeMcpLeaseState, XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: None,
            use_local_host_probe: false,
        },
        sink.clone(),
    );
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());

    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    assert_eq!(pool.active_lease_count().await, 1);
    assert_eq!(attachment.lease_ids.len(), 1);
    assert_eq!(
        pool.lease_state(&attachment.lease_ids[0]).await,
        Some(XcodeMcpLeaseState::Reserved)
    );
    assert!(attachment.request.brokered_xcode_intents().is_empty());
    let ResolvedMcpServerTransport::Http { url, headers } =
        &attachment.request.mcp_servers[0].transport
    else {
        panic!("brokered Xcode intent must be converted to HTTP transport");
    };
    assert!(url.starts_with("http://127.0.0.1:8123/xcode-mcp/lease-"));
    assert!(headers
        .get("Authorization")
        .is_some_and(|value| value.starts_with("Bearer xcode-lease-")));

    let err = pool.attach_brokered_xcode_leases(&req).await.unwrap_err();
    assert!(
        err.to_string().contains("xcode_mcp_capacity_exhausted"),
        "unexpected error: {err:#}"
    );

    pool.mark_lease_active(&attachment.lease_ids[0])
        .await
        .unwrap();
    assert_eq!(
        pool.lease_state(&attachment.lease_ids[0]).await,
        Some(XcodeMcpLeaseState::Active)
    );
    pool.release_brokered_xcode_leases(&attachment.lease_ids)
        .await
        .unwrap();
    assert_eq!(pool.active_lease_count().await, 0);
    assert_eq!(pool.lease_state(&attachment.lease_ids[0]).await, None);

    let updates = sink.updates.lock().await;
    assert_eq!(updates.len(), 5);
    let reserved = match &updates[0] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(reserved.backend_start_disposition, "lease_reserved");
    assert_eq!(reserved.pool_id.as_deref(), Some("fixture-pool"));
    assert_eq!(reserved.backend_failure_class, None);
    assert!(reserved
        .http_endpoint
        .as_deref()
        .is_some_and(|endpoint| !endpoint.contains("Bearer")));

    let capacity = match &updates[1] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(capacity.backend_start_disposition, "capacity_rejected");
    assert_eq!(
        capacity.backend_failure_class,
        Some(XcodeRuntimeFailureClass::XcodeMcpCapacityExhausted)
    );

    let active = match &updates[2] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(active.backend_start_disposition, "lease_active");

    let closing = match &updates[3] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(closing.backend_start_disposition, "lease_closing");

    let released = match &updates[4] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(released.backend_start_disposition, "lease_released");
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_resolves_target_snapshot_before_reserving_lease() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeProcessCandidate, XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace.clone()),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host.clone()),
            use_local_host_probe: false,
        },
        sink.clone(),
    );
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());

    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let snapshot = pool
        .lease_target_snapshot(&attachment.lease_ids[0])
        .await
        .expect("lease target snapshot");
    assert_eq!(snapshot.xcode_pid, 4242);
    assert_eq!(snapshot.workspace_identity, workspace);
    assert_eq!(snapshot.operator_home, "/Users/gui");

    let updates = sink.updates.lock().await;
    let reserved = match &updates[0] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(reserved.backend_start_disposition, "lease_reserved");
    assert_eq!(reserved.xcode_pid.as_deref(), Some("4242"));
    assert!(reserved
        .status_update
        .as_deref()
        .is_some_and(|status| status.contains("targeting Xcode pid 4242")));
    drop(updates);
    pool.release_brokered_xcode_leases(&attachment.lease_ids)
        .await
        .unwrap();

    let failure_sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let failure_pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(HostProbeContext {
                candidate_xcodes: vec![XcodeProcessCandidate {
                    workspace_identity: Some("/other".to_string()),
                    ..host.candidate_xcodes[0].clone()
                }],
                ..host
            }),
            use_local_host_probe: false,
        },
        failure_sink.clone(),
    );
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());
    let err = failure_pool
        .attach_brokered_xcode_leases(&req)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("xcode_target_not_found"),
        "unexpected error: {err:#}"
    );
    assert_eq!(failure_pool.active_lease_count().await, 0);
    let updates = failure_sink.updates.lock().await;
    let failed = match &updates[0] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(failed.backend_start_disposition, "target_resolution_failed");
    assert_eq!(
        failed.backend_failure_class,
        Some(XcodeRuntimeFailureClass::XcodeTargetNotFound)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_closes_drifted_pid_and_targets_refreshed_snapshot() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeMcpLeaseState, XcodeProcessCandidate, XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace.clone()),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host.clone()),
            use_local_host_probe: false,
        },
        sink.clone(),
    );

    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());
    let first_attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    pool.mark_lease_active(&first_attachment.lease_ids[0])
        .await
        .unwrap();

    let refreshed_host = HostProbeContext {
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 5252,
            workspace_identity: Some(workspace.clone()),
            ..host.candidate_xcodes[0].clone()
        }],
        ..host
    };
    pool.replace_target_probe_context(Some(refreshed_host))
        .await;

    let drifted = pool.cleanup_pid_drift().await.unwrap();
    assert_eq!(drifted, first_attachment.lease_ids);
    assert_eq!(pool.active_lease_count().await, 0);
    assert_eq!(pool.lease_state(&drifted[0]).await, None);

    let second_attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let snapshot = pool
        .lease_target_snapshot(&second_attachment.lease_ids[0])
        .await
        .expect("refreshed target snapshot");
    assert_eq!(snapshot.xcode_pid, 5252);
    assert_eq!(
        pool.lease_state(&second_attachment.lease_ids[0]).await,
        Some(XcodeMcpLeaseState::Reserved)
    );

    let updates = sink.updates.lock().await;
    let drift = updates
        .iter()
        .find_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "pool_pid_drift" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .expect("pool pid drift observation");
    assert_eq!(
        drift.backend_failure_class,
        Some(XcodeRuntimeFailureClass::PoolPidDrift)
    );
    assert_eq!(drift.xcode_pid.as_deref(), Some("4242"));
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_serializes_initialize_per_xcode_pid() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeProcessCandidate,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::XcodeRuntimeObservationUpdate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    struct SerializingBackend {
        initialize_in_flight: AtomicUsize,
        max_initialize_in_flight: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl XcodeMcpBackend for SerializingBackend {
        async fn forward_json_rpc(
            &self,
            context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            assert_eq!(
                context
                    .target_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.xcode_pid),
                Some(4242)
            );
            if request.get("method").and_then(|method| method.as_str()) == Some("initialize") {
                let in_flight = self.initialize_in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_initialize_in_flight
                    .fetch_max(in_flight, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(25)).await;
                self.initialize_in_flight.fetch_sub(1, Ordering::SeqCst);
            }
            Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": {"ok": true}
            }))
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let backend = Arc::new(SerializingBackend {
        initialize_in_flight: AtomicUsize::new(0),
        max_initialize_in_flight: AtomicUsize::new(0),
    });
    let pool = Arc::new(XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 2,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(1),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        sink.clone(),
        backend.clone(),
    ));

    let mut first_req = brokered_xcode_request(&tmp, "claude");
    first_req.agent_execution_id = Some(AgentExecutionId::new());
    let first = pool.attach_brokered_xcode_leases(&first_req).await.unwrap();
    let mut second_req = brokered_xcode_request(&tmp, "claude");
    second_req.agent_execution_id = Some(AgentExecutionId::new());
    let second = pool
        .attach_brokered_xcode_leases(&second_req)
        .await
        .unwrap();

    let first_pool = pool.clone();
    let first_lease = first.lease_ids[0].clone();
    let first_forward = tokio::spawn(async move {
        first_pool
            .forward_json_rpc_request(
                &first_lease,
                serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
            )
            .await
            .unwrap()
    });
    let second_pool = pool.clone();
    let second_lease = second.lease_ids[0].clone();
    let second_forward = tokio::spawn(async move {
        second_pool
            .forward_json_rpc_request(
                &second_lease,
                serde_json::json!({"jsonrpc":"2.0","id":2,"method":"initialize"}),
            )
            .await
            .unwrap()
    });

    let (first_response, second_response) = tokio::join!(first_forward, second_forward);
    assert_eq!(first_response.unwrap()["result"]["ok"], true);
    assert_eq!(second_response.unwrap()["result"]["ok"], true);
    assert_eq!(backend.max_initialize_in_flight.load(Ordering::SeqCst), 1);

    let updates = sink.updates.lock().await;
    let initialize_observations = updates
        .iter()
        .filter_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "initialize_lock_acquired" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(initialize_observations.len(), 2);
    assert!(initialize_observations
        .iter()
        .all(|observation| observation.xcode_pid.as_deref() == Some("4242")));
}

#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn xcode_mcp_bridge_pool_records_action_required_after_initialize_lock_wait() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeProcessCandidate,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{Mutex, Notify};

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    struct BlockingInitializeBackend {
        first_initialize_started: Notify,
        initialize_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl XcodeMcpBackend for BlockingInitializeBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            if request.get("method").and_then(|method| method.as_str()) == Some("initialize") {
                let call_index = self.initialize_calls.fetch_add(1, Ordering::SeqCst);
                if call_index == 0 {
                    self.first_initialize_started.notify_waiters();
                    tokio::time::sleep(Duration::from_secs(6)).await;
                }
            }
            Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": {"ok": true}
            }))
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let backend = Arc::new(BlockingInitializeBackend {
        first_initialize_started: Notify::new(),
        initialize_calls: AtomicUsize::new(0),
    });
    let pool = Arc::new(XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 2,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(10),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        sink.clone(),
        backend.clone(),
    ));

    let mut first_req = brokered_xcode_request(&tmp, "claude");
    first_req.agent_execution_id = Some(AgentExecutionId::new());
    let first = pool.attach_brokered_xcode_leases(&first_req).await.unwrap();
    let mut second_req = brokered_xcode_request(&tmp, "claude");
    second_req.agent_execution_id = Some(AgentExecutionId::new());
    let second = pool
        .attach_brokered_xcode_leases(&second_req)
        .await
        .unwrap();

    let first_pool = pool.clone();
    let first_lease = first.lease_ids[0].clone();
    let first_forward = tokio::spawn(async move {
        first_pool
            .forward_json_rpc_request(
                &first_lease,
                serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
            )
            .await
            .unwrap()
    });
    backend.first_initialize_started.notified().await;

    let second_pool = pool.clone();
    let second_lease = second.lease_ids[0].clone();
    let second_forward = tokio::spawn(async move {
        second_pool
            .forward_json_rpc_request(
                &second_lease,
                serde_json::json!({"jsonrpc":"2.0","id":2,"method":"initialize"}),
            )
            .await
            .unwrap()
    });

    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(5)).await;
    tokio::task::yield_now().await;

    let updates = sink.updates.lock().await;
    let action_required = updates
        .iter()
        .filter_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "initialize_action_required" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(action_required.len(), 1);
    assert_eq!(
        action_required[0].backend_failure_class,
        Some(XcodeRuntimeFailureClass::XcodeMcpActionRequired)
    );
    assert!(action_required[0]
        .status_update
        .as_deref()
        .unwrap_or_default()
        .contains("Action Required: Check Xcode"));
    drop(updates);

    tokio::time::advance(Duration::from_secs(1)).await;
    let (first_response, second_response) = tokio::join!(first_forward, second_forward);
    assert_eq!(first_response.unwrap()["result"]["ok"], true);
    assert_eq!(second_response.unwrap()["result"]["ok"], true);
}

#[cfg(unix)]
#[tokio::test(start_paused = true)]
async fn xcode_mcp_bridge_pool_records_action_required_during_slow_tools_list() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeProcessCandidate,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    struct SlowToolsListBackend;

    #[async_trait::async_trait]
    impl XcodeMcpBackend for SlowToolsListBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            if request.get("method").and_then(|method| method.as_str()) == Some("tools/list") {
                tokio::time::sleep(Duration::from_secs(6)).await;
            }
            Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": {"tools": []}
            }))
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = Arc::new(XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(10),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        sink.clone(),
        Arc::new(SlowToolsListBackend),
    ));
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let lease_id = attachment.lease_ids[0].clone();

    let forward = {
        let pool = pool.clone();
        tokio::spawn(async move {
            pool.forward_json_rpc_request(
                &lease_id,
                serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            )
            .await
            .unwrap()
        })
    };
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(5)).await;
    tokio::task::yield_now().await;

    let updates = sink.updates.lock().await;
    let action_required = updates
        .iter()
        .filter_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "backend_request_action_required" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(action_required.len(), 1);
    assert_eq!(
        action_required[0].backend_failure_class,
        Some(XcodeRuntimeFailureClass::XcodeMcpActionRequired)
    );
    assert!(action_required[0]
        .status_update
        .as_deref()
        .unwrap_or_default()
        .contains("Action Required: Check Xcode"));
    drop(updates);

    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(
        forward.await.unwrap()["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test(start_paused = true)]
async fn xcode_mcp_bridge_pool_times_out_backend_request_after_action_required_budget() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeProcessCandidate,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    struct HungToolsListBackend;

    #[async_trait::async_trait]
    impl XcodeMcpBackend for HungToolsListBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            if request.get("method").and_then(|method| method.as_str()) == Some("tools/list") {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": {"tools": []}
            }))
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = Arc::new(XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(10),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        sink.clone(),
        Arc::new(HungToolsListBackend),
    ));
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let lease_id = attachment.lease_ids[0].clone();

    let forward = {
        let pool = pool.clone();
        tokio::spawn(async move {
            pool.forward_json_rpc_request(
                &lease_id,
                serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            )
            .await
        })
    };
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(10)).await;
    tokio::task::yield_now().await;

    let err = forward
        .await
        .expect("backend request task should settle")
        .expect_err("hung backend request should fail closed");
    assert!(err.to_string().contains("xcode_mcp_initialize_timeout"));

    let updates = sink.updates.lock().await;
    let timed_out = updates
        .iter()
        .filter_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "backend_request_timeout" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(timed_out.len(), 1);
    assert_eq!(
        timed_out[0].backend_failure_class,
        Some(XcodeRuntimeFailureClass::XcodeMcpInitializeTimeout)
    );
}

#[cfg(unix)]
#[test]
fn xcode_mcp_broker_default_timeouts_allow_human_consent_without_provider_race() {
    use acp::{XcodeMcpBridgePoolConfig, XcodeMcpProcessBackendConfig};
    use std::time::Duration;

    assert!(XcodeMcpBridgePoolConfig::default().spawn_init_timeout >= Duration::from_secs(10 * 60));
    assert!(
        XcodeMcpBridgePoolConfig::default().first_connect_timeout >= Duration::from_secs(10 * 60)
    );
    assert!(
        XcodeMcpBridgePoolConfig::default().first_connect_timeout
            > XcodeMcpBridgePoolConfig::default().spawn_init_timeout,
        "first-connect cleanup must not win the race against warmup/backend timeout"
    );
    assert!(
        XcodeMcpProcessBackendConfig::default().request_timeout >= Duration::from_secs(10 * 60)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_process_backend_spawns_with_target_env_and_rewrites_ids() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeMcpProcessBackend, XcodeMcpProcessBackendConfig, XcodeProcessCandidate,
    };
    use std::sync::Arc;
    use std::time::Duration;

    let tmp = tempfile::tempdir().unwrap();
    let backend_script = tmp.path().join("mcp_backend.py");
    let code = r#"#!/usr/bin/env python3
import json
import os
import sys

for line in sys.stdin:
    if not line.strip():
        continue
    request = json.loads(line)
    sys.stdout.write(json.dumps({
        "jsonrpc": "2.0",
        "id": request.get("id"),
            "result": {
                "received_id": request.get("id"),
                "method": request.get("method"),
                "home": os.environ.get("HOME"),
                "tmpdir": os.environ.get("TMPDIR"),
                "developer_dir": os.environ.get("DEVELOPER_DIR"),
                "user": os.environ.get("USER"),
                "logname": os.environ.get("LOGNAME"),
                "path": os.environ.get("PATH"),
                "mcp_xcode_pid": os.environ.get("MCP_XCODE_PID"),
                "chainworks_env": sorted([key for key in os.environ if key.startswith("CHAINWORKS_")])
            }
        }) + "\n")
    sys.stdout.flush()
"#;
    std::fs::write(&backend_script, code).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&backend_script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&backend_script, permissions).unwrap();
    }

    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let backend = Arc::new(XcodeMcpProcessBackend::new(XcodeMcpProcessBackendConfig {
        command: backend_script.to_string_lossy().into_owned(),
        args: Vec::new(),
        request_timeout: Duration::from_secs(15),
    }));
    let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(1),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        Arc::new(acp::NoopXcodeRuntimeObservationSink),
        backend,
    );
    let req = brokered_xcode_request(&tmp, "claude");
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let lease_id = &attachment.lease_ids[0];

    let initialize = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":"client-init","method":"initialize"}),
        )
        .await
        .unwrap();
    assert_eq!(initialize["id"], "client-init");
    assert_eq!(initialize["result"]["received_id"], 1);
    assert_eq!(initialize["result"]["home"], "/Users/gui");
    assert_eq!(initialize["result"]["tmpdir"], "/var/folders/t/tmp");
    assert_eq!(
        initialize["result"]["developer_dir"],
        "/Applications/Xcode.app/Contents/Developer"
    );
    assert_eq!(initialize["result"]["mcp_xcode_pid"], "4242");
    assert_eq!(initialize["result"]["user"], "gui");
    assert_eq!(initialize["result"]["logname"], "gui");
    assert_eq!(
        initialize["result"]["path"],
        "/usr/bin:/bin:/usr/sbin:/sbin:/Applications/Xcode.app/Contents/Developer/usr/bin"
    );
    assert_eq!(
        initialize["result"]["chainworks_env"]
            .as_array()
            .unwrap()
            .len(),
        0
    );

    let tools = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":99,"method":"tools/list"}),
        )
        .await
        .unwrap();
    assert_eq!(tools["id"], 99);
    assert_eq!(tools["result"]["received_id"], 2);

    pool.release_brokered_xcode_leases(&attachment.lease_ids)
        .await
        .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_process_backend_shares_initialized_backend_per_xcode_pid() {
    // P051 intentionally shares one initialized backend per Xcode target to
    // bound Xcode consent prompts across batched run retries; lease policy
    // remains enforced before forward.
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBridgePool,
        XcodeMcpBridgePoolConfig, XcodeMcpProcessBackend, XcodeMcpProcessBackendConfig,
        XcodeProcessCandidate,
    };
    use domain::ids::RunId;
    use std::sync::Arc;
    use std::time::Duration;

    let tmp = tempfile::tempdir().unwrap();
    let backend_script = tmp.path().join("mcp_backend_shared.py");
    let spawn_count_path = tmp.path().join("spawn_count.txt");
    let request_log = tmp.path().join("requests.jsonl");
    let code = r#"#!/usr/bin/env python3
import json
import sys

counter_path = sys.argv[1]
request_log = sys.argv[2]
try:
    with open(counter_path, "r", encoding="utf-8") as fh:
        spawn_count = int(fh.read().strip() or "0") + 1
except FileNotFoundError:
    spawn_count = 1
with open(counter_path, "w", encoding="utf-8") as fh:
    fh.write(str(spawn_count))

for line in sys.stdin:
    if not line.strip():
        continue
    request = json.loads(line)
    with open(request_log, "a", encoding="utf-8") as fh:
        fh.write(json.dumps({"spawn_count": spawn_count, "method": request.get("method"), "id": request.get("id")}) + "\n")
    sys.stdout.write(json.dumps({
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": {
            "spawn_count": spawn_count,
            "received_id": request.get("id"),
            "method": request.get("method")
        }
    }) + "\n")
    sys.stdout.flush()
"#;
    std::fs::write(&backend_script, code).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&backend_script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&backend_script, permissions).unwrap();
    }

    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let backend = Arc::new(XcodeMcpProcessBackend::new(XcodeMcpProcessBackendConfig {
        command: backend_script.to_string_lossy().into_owned(),
        args: vec![
            spawn_count_path.to_string_lossy().into_owned(),
            request_log.to_string_lossy().into_owned(),
        ],
        request_timeout: Duration::from_secs(15),
    }));
    let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 3,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(1),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        Arc::new(acp::NoopXcodeRuntimeObservationSink),
        backend.clone(),
    );
    let run_id = RunId::new();
    let mut first_req = brokered_xcode_request(&tmp, "gemini");
    first_req.run_id = run_id;
    first_req.agent_id = "p051_gemini_ux_xcode".into();
    let mut second_req = brokered_xcode_request(&tmp, "gemini");
    second_req.run_id = run_id;
    second_req.agent_id = "p051_gemini_ui_xcode".into();
    let mut different_run_req = brokered_xcode_request(&tmp, "gemini");
    different_run_req.agent_id = "p051_gemini_other_run_xcode".into();

    let first_attachment = pool.attach_brokered_xcode_leases(&first_req).await.unwrap();
    let second_attachment = pool
        .attach_brokered_xcode_leases(&second_req)
        .await
        .unwrap();
    let different_run_attachment = pool
        .attach_brokered_xcode_leases(&different_run_req)
        .await
        .unwrap();
    let first_lease = &first_attachment.lease_ids[0];
    let second_lease = &second_attachment.lease_ids[0];
    let different_run_lease = &different_run_attachment.lease_ids[0];

    let first_initialize = pool
        .forward_json_rpc_request(
            first_lease,
            serde_json::json!({"jsonrpc":"2.0","id":"first-init","method":"initialize"}),
        )
        .await
        .unwrap();
    let first_backend_pid = backend.backend_process_id(first_lease).await;
    assert!(first_backend_pid.is_some());

    let second_initialize = pool
        .forward_json_rpc_request(
            second_lease,
            serde_json::json!({"jsonrpc":"2.0","id":"second-init","method":"initialize"}),
        )
        .await
        .unwrap();
    assert_eq!(first_initialize["result"]["spawn_count"], 1);
    assert_eq!(second_initialize["id"], "second-init");
    assert_eq!(second_initialize["result"]["spawn_count"], 1);
    assert_eq!(second_initialize["result"]["received_id"], 1);
    assert_eq!(
        backend.backend_process_id(second_lease).await,
        first_backend_pid
    );

    let different_run_initialize = pool
        .forward_json_rpc_request(
            different_run_lease,
            serde_json::json!({"jsonrpc":"2.0","id":"different-run-init","method":"initialize"}),
        )
        .await
        .unwrap();
    let different_run_backend_pid = backend.backend_process_id(different_run_lease).await;
    assert_eq!(different_run_backend_pid, first_backend_pid);
    assert_eq!(different_run_initialize["id"], "different-run-init");
    assert_eq!(different_run_initialize["result"]["spawn_count"], 1);

    let tools = pool
        .forward_json_rpc_request(
            second_lease,
            serde_json::json!({"jsonrpc":"2.0","id":"second-tools","method":"tools/list"}),
        )
        .await
        .unwrap();
    assert_eq!(tools["id"], "second-tools");
    assert_eq!(tools["result"]["received_id"], 2);

    let log = std::fs::read_to_string(&request_log).unwrap();
    assert_eq!(log.matches("\"method\": \"initialize\"").count(), 1);
    assert_eq!(std::fs::read_to_string(&spawn_count_path).unwrap(), "1");

    pool.release_brokered_xcode_leases(&first_attachment.lease_ids)
        .await
        .unwrap();
    assert_eq!(backend.backend_process_id(first_lease).await, None);
    assert_eq!(
        backend.backend_process_id(second_lease).await,
        first_backend_pid
    );
    assert_eq!(
        backend.backend_process_id(different_run_lease).await,
        different_run_backend_pid
    );

    pool.release_brokered_xcode_leases(&second_attachment.lease_ids)
        .await
        .unwrap();
    assert_eq!(backend.backend_process_id(second_lease).await, None);
    assert_eq!(
        backend.backend_process_id(different_run_lease).await,
        different_run_backend_pid
    );

    let mut retry_same_run_req = brokered_xcode_request(&tmp, "gemini");
    retry_same_run_req.run_id = run_id;
    retry_same_run_req.agent_id = "p051_gemini_retry_same_run_xcode".into();
    let retry_same_run_attachment = pool
        .attach_brokered_xcode_leases(&retry_same_run_req)
        .await
        .unwrap();
    let retry_same_run_lease = &retry_same_run_attachment.lease_ids[0];
    let retry_same_run_initialize = pool
        .forward_json_rpc_request(
            retry_same_run_lease,
            serde_json::json!({"jsonrpc":"2.0","id":"retry-same-run-init","method":"initialize"}),
        )
        .await
        .unwrap();
    assert_eq!(retry_same_run_initialize["id"], "retry-same-run-init");
    assert_eq!(retry_same_run_initialize["result"]["spawn_count"], 1);
    assert_eq!(
        backend.backend_process_id(retry_same_run_lease).await,
        first_backend_pid
    );
    assert_eq!(
        std::fs::read_to_string(&spawn_count_path).unwrap(),
        "1",
        "retrying any run against the same Xcode target must reuse the warm consent-granted backend"
    );
    pool.release_brokered_xcode_leases(&retry_same_run_attachment.lease_ids)
        .await
        .unwrap();
    assert_eq!(backend.backend_process_id(retry_same_run_lease).await, None);

    pool.release_brokered_xcode_leases(&different_run_attachment.lease_ids)
        .await
        .unwrap();
    assert_eq!(backend.backend_process_id(different_run_lease).await, None);
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_process_backend_forwards_notifications_without_backend_ids() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeMcpProcessBackend, XcodeMcpProcessBackendConfig, XcodeProcessCandidate,
    };
    use std::sync::Arc;
    use std::time::Duration;

    let tmp = tempfile::tempdir().unwrap();
    let backend_script = tmp.path().join("mcp_backend_notifications.py");
    let notification_log = tmp.path().join("notifications.jsonl");
    let code = r#"#!/usr/bin/env python3
import json
import sys

notification_log = sys.argv[1]
for line in sys.stdin:
    if not line.strip():
        continue
    request = json.loads(line)
    if "id" not in request:
        with open(notification_log, "a", encoding="utf-8") as fh:
            fh.write(json.dumps(request) + "\n")
        continue
    sys.stdout.write(json.dumps({
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": {
            "received_id": request.get("id"),
            "method": request.get("method")
        }
    }) + "\n")
    sys.stdout.flush()
"#;
    std::fs::write(&backend_script, code).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&backend_script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&backend_script, permissions).unwrap();
    }

    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let backend = Arc::new(XcodeMcpProcessBackend::new(XcodeMcpProcessBackendConfig {
        command: backend_script.to_string_lossy().into_owned(),
        args: vec![notification_log.to_string_lossy().into_owned()],
        request_timeout: Duration::from_secs(15),
    }));
    let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(1),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        Arc::new(acp::NoopXcodeRuntimeObservationSink),
        backend,
    );
    let req = brokered_xcode_request(&tmp, "claude");
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let lease_id = &attachment.lease_ids[0];

    let initialize = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":"client-init","method":"initialize"}),
        )
        .await
        .unwrap();
    assert_eq!(initialize["id"], "client-init");
    assert_eq!(initialize["result"]["received_id"], 1);

    let notification = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        )
        .await
        .unwrap();
    assert_eq!(notification["id"], serde_json::Value::Null);
    assert_eq!(notification["result"], serde_json::Value::Null);

    let tools = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":99,"method":"tools/list"}),
        )
        .await
        .unwrap();
    assert_eq!(tools["id"], 99);
    assert_eq!(tools["result"]["received_id"], 2);

    let log = std::fs::read_to_string(&notification_log).unwrap();
    assert!(log.contains("notifications/initialized"));
    assert!(
        !log.contains("\"id\""),
        "notification must not receive a backend id: {log}"
    );

    pool.release_brokered_xcode_leases(&attachment.lease_ids)
        .await
        .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_warmup_runs_full_handshake_before_provider_receives_endpoint() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeProcessCandidate,
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct RecordingBackend {
        methods: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl XcodeMcpBackend for RecordingBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            let method = request
                .get("method")
                .and_then(|method| method.as_str())
                .unwrap_or("<missing>")
                .to_string();
            self.methods.lock().await.push(method.clone());
            let id = request
                .get("id")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let result = if method == "tools/list" {
                serde_json::json!({"tools": []})
            } else {
                serde_json::json!({"ok": true})
            };
            Ok(serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result}))
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let backend = Arc::new(RecordingBackend {
        methods: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(10),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        Arc::new(acp::NoopXcodeRuntimeObservationSink),
        backend.clone(),
    );
    let req = brokered_xcode_request(&tmp, "claude");
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();

    pool.warm_up_brokered_xcode_leases(&attachment.lease_ids)
        .await
        .unwrap();

    assert_eq!(
        backend.methods.lock().await.as_slice(),
        ["initialize", "notifications/initialized", "tools/list"]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_warmup_refreshes_first_connect_deadline() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeProcessCandidate,
    };
    use std::sync::Arc;
    use std::time::Duration;

    struct ReadyBackend;

    #[async_trait::async_trait]
    impl XcodeMcpBackend for ReadyBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            let method = request.get("method").and_then(|method| method.as_str());
            let result = if method == Some("tools/list") {
                serde_json::json!({"tools": []})
            } else {
                serde_json::json!({"ok": true})
            };
            Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": result
            }))
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(10),
            first_connect_timeout: Duration::from_millis(10),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        Arc::new(acp::NoopXcodeRuntimeObservationSink),
        Arc::new(ReadyBackend),
    );
    let req = brokered_xcode_request(&tmp, "claude");
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();

    tokio::time::sleep(Duration::from_millis(20)).await;
    pool.warm_up_brokered_xcode_leases(&attachment.lease_ids)
        .await
        .unwrap();

    assert!(
        pool.cleanup_first_connect_timeouts().await.unwrap().is_empty(),
        "successful broker warmup must not leave the reserved lease expired before provider session/new"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_process_backend_drops_crashed_session_before_retry() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBridgePool,
        XcodeMcpBridgePoolConfig, XcodeMcpProcessBackend, XcodeMcpProcessBackendConfig,
        XcodeProcessCandidate,
    };
    use std::sync::Arc;
    use std::time::Duration;

    let tmp = tempfile::tempdir().unwrap();
    let backend_script = tmp.path().join("mcp_backend_crash_once.py");
    let spawn_count_path = tmp.path().join("spawn_count.txt");
    let code = r#"#!/usr/bin/env python3
import json
import os
import sys

counter_path = sys.argv[1]
try:
    with open(counter_path, "r", encoding="utf-8") as fh:
        spawn_count = int(fh.read().strip() or "0") + 1
except FileNotFoundError:
    spawn_count = 1
with open(counter_path, "w", encoding="utf-8") as fh:
    fh.write(str(spawn_count))

for line in sys.stdin:
    if not line.strip():
        continue
    request = json.loads(line)
    if spawn_count == 1:
        sys.exit(7)
    sys.stdout.write(json.dumps({
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": {"spawn_count": spawn_count}
    }) + "\n")
    sys.stdout.flush()
"#;
    std::fs::write(&backend_script, code).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&backend_script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&backend_script, permissions).unwrap();
    }

    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let backend = Arc::new(XcodeMcpProcessBackend::new(XcodeMcpProcessBackendConfig {
        command: backend_script.to_string_lossy().into_owned(),
        args: vec![spawn_count_path.to_string_lossy().into_owned()],
        request_timeout: Duration::from_secs(15),
    }));
    let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(1),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        Arc::new(acp::NoopXcodeRuntimeObservationSink),
        backend.clone(),
    );
    let req = brokered_xcode_request(&tmp, "claude");
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let lease_id = &attachment.lease_ids[0];

    let err = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":"first","method":"initialize"}),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("xcode_mcp_backend_crashed"),
        "unexpected error: {err:#}"
    );
    assert_eq!(backend.backend_process_id(lease_id).await, None);

    let retried = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":"second","method":"initialize"}),
        )
        .await
        .unwrap();
    assert_eq!(retried["id"], "second");
    assert_eq!(retried["result"]["spawn_count"], 2);

    pool.release_brokered_xcode_leases(&attachment.lease_ids)
        .await
        .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_records_backend_request_observations() {
    use acp::{
        HostProbeContext, XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeProcessCandidate,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    struct ObservedBackend;

    #[async_trait::async_trait]
    impl XcodeMcpBackend for ObservedBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            if request.get("method").and_then(|method| method.as_str()) == Some("tools/list") {
                anyhow::bail!("xcode_mcp_backend_crashed: fixture backend stopped");
            }
            Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                "result": {"ok": true}
            }))
        }

        async fn backend_process_id(&self, _lease_id: &str) -> Option<i64> {
            Some(54321)
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let host = HostProbeContext {
        expected_gui_uid: Some(501),
        operator_home: Some("/Users/gui".to_string()),
        darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
        developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
        candidate_xcodes: vec![XcodeProcessCandidate {
            pid: 4242,
            uid: 501,
            workspace_identity: Some(workspace),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive: true,
        }],
    };
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(1),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: Some(host),
            use_local_host_probe: false,
        },
        sink.clone(),
        Arc::new(ObservedBackend),
    );
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());
    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let lease_id = &attachment.lease_ids[0];

    let initialized = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
        )
        .await
        .unwrap();
    assert_eq!(initialized["result"]["ok"], true);

    let err = pool
        .forward_json_rpc_request(
            lease_id,
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .await
        .unwrap_err();
    assert!(err.to_string().contains("xcode_mcp_backend_crashed"));

    let updates = sink.updates.lock().await;
    let completed = updates
        .iter()
        .filter_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "backend_request_completed" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].backend_process_id, Some(54321));
    assert_eq!(completed[0].xcode_pid.as_deref(), Some("4242"));
    assert!(completed[0].backend_startup_latency_ms.is_some());

    let failed = updates
        .iter()
        .filter_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "backend_request_failed" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(failed.len(), 1);
    assert_eq!(
        failed[0].backend_failure_class,
        Some(XcodeRuntimeFailureClass::PerLeaseBackend)
    );
    assert_eq!(failed[0].backend_process_id, Some(54321));
    assert!(failed[0]
        .status_update
        .as_deref()
        .is_some_and(|status| status.contains("tools/list")));
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_expires_reserved_first_connect_leases() {
    use acp::{
        XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 2,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_millis(0),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: None,
            use_local_host_probe: false,
        },
        sink.clone(),
    );
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());

    let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
    let expired = pool.cleanup_first_connect_timeouts().await.unwrap();

    assert_eq!(expired, attachment.lease_ids);
    assert_eq!(pool.active_lease_count().await, 0);

    let updates = sink.updates.lock().await;
    assert_eq!(updates.len(), 2);
    let timeout = match &updates[1] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(timeout.backend_start_disposition, "first_connect_timeout");
    assert_eq!(
        timeout.backend_failure_class,
        Some(XcodeRuntimeFailureClass::XcodeMcpFirstConnectTimeout)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_rejects_when_disabled() {
    use acp::{
        XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 1,
            queue_timeout: Duration::from_secs(1),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: true,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: None,
            use_local_host_probe: false,
        },
        sink.clone(),
    );
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());

    let err = pool.attach_brokered_xcode_leases(&req).await.unwrap_err();
    assert!(
        err.to_string().contains("xcode_mcp_broker_disabled"),
        "unexpected error: {err:#}"
    );
    assert_eq!(pool.active_lease_count().await, 0);
    assert_eq!(pool.queued_lease_count(), 0);

    let updates = sink.updates.lock().await;
    assert_eq!(updates.len(), 1);
    let disabled = match &updates[0] {
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation) => observation,
        update => panic!("unexpected update: {update:?}"),
    };
    assert_eq!(disabled.backend_start_disposition, "broker_disabled");
    assert_eq!(
        disabled.backend_failure_class,
        Some(XcodeRuntimeFailureClass::BrokerInfrastructure)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_waits_for_queued_capacity() {
    use acp::{
        XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::XcodeRuntimeObservationUpdate;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = Arc::new(XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 1,
            queue_timeout: Duration::from_secs(1),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: None,
            use_local_host_probe: false,
        },
        sink.clone(),
    ));
    let mut first_req = brokered_xcode_request(&tmp, "claude");
    first_req.agent_execution_id = Some(AgentExecutionId::new());
    let first_attachment = pool.attach_brokered_xcode_leases(&first_req).await.unwrap();

    let mut second_req = brokered_xcode_request(&tmp, "claude");
    second_req.agent_execution_id = Some(AgentExecutionId::new());
    let queued_pool = pool.clone();
    let queued = tokio::spawn(async move {
        queued_pool
            .attach_brokered_xcode_leases(&second_req)
            .await
            .unwrap()
    });

    for _ in 0..50 {
        if pool.queued_lease_count() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(pool.queued_lease_count(), 1);

    pool.release_brokered_xcode_leases(&first_attachment.lease_ids)
        .await
        .unwrap();
    let second_attachment = tokio::time::timeout(Duration::from_secs(1), queued)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(pool.queued_lease_count(), 0);
    assert_eq!(pool.active_lease_count().await, 1);
    assert_eq!(second_attachment.lease_ids.len(), 1);
    pool.release_brokered_xcode_leases(&second_attachment.lease_ids)
        .await
        .unwrap();

    let updates = sink.updates.lock().await;
    assert!(updates.iter().any(|update| matches!(
        update,
        XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
            if observation.backend_start_disposition == "queue_waiting"
    )));
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_times_out_queued_capacity() {
    use acp::{
        XcodeBrokerLeaseAttacher, XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
        XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 1,
            max_queued_leases: 1,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: Default::default(),
            target_probe_context: None,
            use_local_host_probe: false,
        },
        sink.clone(),
    );
    let mut req = brokered_xcode_request(&tmp, "claude");
    req.agent_execution_id = Some(AgentExecutionId::new());
    let first_attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();

    let err = pool.attach_brokered_xcode_leases(&req).await.unwrap_err();
    assert!(
        err.to_string().contains("xcode_mcp_capacity_exhausted"),
        "unexpected error: {err:#}"
    );
    assert_eq!(pool.queued_lease_count(), 0);
    pool.release_brokered_xcode_leases(&first_attachment.lease_ids)
        .await
        .unwrap();

    let updates = sink.updates.lock().await;
    let timeout = updates
        .iter()
        .find_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "queue_timeout" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .expect("queue timeout observation");
    assert_eq!(
        timeout.backend_failure_class,
        Some(XcodeRuntimeFailureClass::XcodeMcpCapacityExhausted)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn xcode_mcp_bridge_pool_enforces_per_lease_tool_policy() {
    use acp::{
        ResolvedMcpServerTransport, XcodeBrokerLeaseAttacher, XcodeMcpBridgePool,
        XcodeMcpBridgePoolConfig, XcodeRuntimeObservationSink,
    };
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::XcodeRuntimeObservationUpdate;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;

    struct CollectingSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for CollectingSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push(update);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(CollectingSink {
        updates: Mutex::new(Vec::new()),
    });
    let allowlists = BTreeMap::from([
        (
            "build-only".to_string(),
            BTreeSet::from(["xcode.build".to_string()]),
        ),
        (
            "test-only".to_string(),
            BTreeSet::from(["xcode.test".to_string()]),
        ),
    ]);
    let pool = XcodeMcpBridgePool::new_with_sink(
        XcodeMcpBridgePoolConfig {
            pool_id: "fixture-pool".to_string(),
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            max_active_leases: 2,
            max_queued_leases: 0,
            queue_timeout: Duration::from_millis(0),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: allowlists,
            target_probe_context: None,
            use_local_host_probe: false,
        },
        sink.clone(),
    );

    let mut build_req = brokered_xcode_request(&tmp, "claude");
    build_req.agent_execution_id = Some(AgentExecutionId::new());
    let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } =
        &mut build_req.mcp_servers[0].transport
    else {
        panic!("expected broker intent");
    };
    intent.resolved_tool_allowlist_hash = Some("build-only".to_string());
    let build_attachment = pool.attach_brokered_xcode_leases(&build_req).await.unwrap();
    let build_lease = &build_attachment.lease_ids[0];

    let mut test_req = brokered_xcode_request(&tmp, "claude");
    test_req.agent_execution_id = Some(AgentExecutionId::new());
    let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } =
        &mut test_req.mcp_servers[0].transport
    else {
        panic!("expected broker intent");
    };
    intent.resolved_tool_allowlist_hash = Some("test-only".to_string());
    let test_attachment = pool.attach_brokered_xcode_leases(&test_req).await.unwrap();
    let test_lease = &test_attachment.lease_ids[0];

    let tools = serde_json::json!({
        "tools": [
            {"name": "xcode.build"},
            {"name": "xcode.test"},
            {"name": "xcode.clean"}
        ]
    });
    let build_filtered = pool
        .filter_tools_list_result(build_lease, tools.clone())
        .await
        .unwrap();
    assert_eq!(build_filtered["tools"].as_array().unwrap().len(), 1);
    assert_eq!(build_filtered["tools"][0]["name"], "xcode.build");
    let test_filtered = pool
        .filter_tools_list_result(test_lease, tools)
        .await
        .unwrap();
    assert_eq!(test_filtered["tools"].as_array().unwrap().len(), 1);
    assert_eq!(test_filtered["tools"][0]["name"], "xcode.test");

    pool.authorize_json_rpc_request(
        build_lease,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "xcode.build"}
        }),
    )
    .await
    .unwrap();
    let err = pool
        .authorize_json_rpc_request(
            build_lease,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {"name": "xcode.test"}
            }),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("xcode_mcp_tool_denied"),
        "unexpected error: {err:#}"
    );

    let updates = sink.updates.lock().await;
    let denied = updates
        .iter()
        .find_map(|update| match update {
            XcodeRuntimeObservationUpdate::McpBrokerObservation(observation)
                if observation.backend_start_disposition == "tool_call_denied" =>
            {
                Some(observation)
            }
            _ => None,
        })
        .expect("denied tool call observation");
    assert_eq!(denied.lease_id.as_deref(), Some(build_lease.as_str()));
    assert!(denied
        .status_update
        .as_deref()
        .is_some_and(|status| status.contains("xcode.test")));

    pool.authorize_json_rpc_request(
        test_lease,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "xcode.test"}
        }),
    )
    .await
    .unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_manager_attaches_brokered_xcode_http_lease_before_session_new() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::{
        AcpMcpServerPayload, AcpRuntimeManager, BrokeredXcodeLeaseAttachment, ExecutionRequest,
        ResolvedMcpServerTransport, XcodeBrokerLeaseAttacher,
    };
    use domain::agent::AgentStatus;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    struct FixtureLeaseAttacher;

    #[async_trait::async_trait]
    impl XcodeBrokerLeaseAttacher for FixtureLeaseAttacher {
        async fn attach_brokered_xcode_leases(
            &self,
            req: &ExecutionRequest,
        ) -> anyhow::Result<BrokeredXcodeLeaseAttachment> {
            let mut attached = req.clone();
            let mut lease_ids = Vec::new();
            for server in &mut attached.mcp_servers {
                let replacement = match &server.transport {
                    ResolvedMcpServerTransport::XcodeBrokerIntent { intent } => {
                        lease_ids.push("lease-fixture".to_string());
                        let mut headers = BTreeMap::new();
                        headers.insert(
                            "Authorization".to_string(),
                            "Bearer fixture-redacted".to_string(),
                        );
                        Some(AcpMcpServerPayload {
                            id: intent.runtime_id.clone(),
                            extension_id: intent.extension_id.clone(),
                            transport: ResolvedMcpServerTransport::Http {
                                url: "http://127.0.0.1:8123/xcode-mcp/lease-fixture".to_string(),
                                headers,
                            },
                        })
                    }
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    *server = replacement;
                }
            }
            Ok(BrokeredXcodeLeaseAttachment {
                request: attached,
                lease_ids,
            })
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let capture_path = tmp.path().join("captured-session-new.json");
    let script = fixture::create_broker_attach_session_script(tmp.path(), &capture_path);
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script));
    let manager = AcpRuntimeManager::new_with_adapters(vec![adapter]);
    manager.set_xcode_broker_lease_attacher(Arc::new(FixtureLeaseAttacher));

    let req = brokered_xcode_request(&tmp, "claude");
    let result = manager.start_session(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    let captured: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(capture_path).unwrap()).unwrap();
    let server = &captured["mcpServers"][0];
    assert_eq!(server["name"], "xcode-broker");
    assert_eq!(server["type"], "http");
    assert_eq!(
        server["url"],
        "http://127.0.0.1:8123/xcode-mcp/lease-fixture"
    );
    assert_eq!(server["headers"][0]["name"], "Authorization");
    assert_eq!(server["headers"][0]["value"], "Bearer fixture-redacted");
    assert!(server.get("transport").is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_manager_warms_brokered_xcode_lease_before_session_new() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::{
        AcpMcpServerPayload, AcpRuntimeManager, BrokeredXcodeLeaseAttachment, ExecutionRequest,
        ResolvedMcpServerTransport, XcodeBrokerLeaseAttacher,
    };
    use domain::agent::AgentStatus;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    struct WarmupLeaseAttacher {
        warmup_marker: std::path::PathBuf,
    }

    #[async_trait::async_trait]
    impl XcodeBrokerLeaseAttacher for WarmupLeaseAttacher {
        async fn attach_brokered_xcode_leases(
            &self,
            req: &ExecutionRequest,
        ) -> anyhow::Result<BrokeredXcodeLeaseAttachment> {
            let mut attached = req.clone();
            for server in &mut attached.mcp_servers {
                if let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } = &server.transport
                {
                    let mut headers = BTreeMap::new();
                    headers.insert(
                        "Authorization".to_string(),
                        "Bearer fixture-redacted".to_string(),
                    );
                    *server = AcpMcpServerPayload {
                        id: intent.runtime_id.clone(),
                        extension_id: intent.extension_id.clone(),
                        transport: ResolvedMcpServerTransport::Http {
                            url: "http://127.0.0.1:8123/xcode-mcp/lease-warmup".to_string(),
                            headers,
                        },
                    };
                }
            }
            Ok(BrokeredXcodeLeaseAttachment {
                request: attached,
                lease_ids: vec!["lease-warmup".to_string()],
            })
        }

        async fn warm_up_brokered_xcode_leases(&self, lease_ids: &[String]) -> anyhow::Result<()> {
            std::fs::write(&self.warmup_marker, lease_ids.join("\n"))?;
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let capture_path = tmp.path().join("captured-session-new.json");
    let warmup_marker = tmp.path().join("warmup-ready.txt");
    let script = fixture::create_broker_attach_requires_warmup_script(
        tmp.path(),
        &capture_path,
        &warmup_marker,
    );
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script));
    let manager = AcpRuntimeManager::new_with_adapters(vec![adapter]);
    manager.set_xcode_broker_lease_attacher(Arc::new(WarmupLeaseAttacher {
        warmup_marker: warmup_marker.clone(),
    }));

    let req = brokered_xcode_request(&tmp, "claude");
    let result = manager.start_session(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert_eq!(
        std::fs::read_to_string(warmup_marker).unwrap(),
        "lease-warmup"
    );
    let captured: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(capture_path).unwrap()).unwrap();
    assert_eq!(captured["mcpServers"][0]["type"], "http");
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_manager_close_all_sessions_releases_brokered_xcode_leases_before_blocked_close() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::{
        AcpMcpServerPayload, AcpRuntimeManager, BrokeredXcodeLeaseAttachment, ExecutionRequest,
        ResolvedMcpServerTransport, XcodeBrokerLeaseAttacher,
    };
    use domain::agent::AgentStatus;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct RecordingLeaseAttacher {
        released: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl XcodeBrokerLeaseAttacher for RecordingLeaseAttacher {
        async fn attach_brokered_xcode_leases(
            &self,
            req: &ExecutionRequest,
        ) -> anyhow::Result<BrokeredXcodeLeaseAttachment> {
            let generation_id = req
                .session_generation_id
                .as_deref()
                .unwrap_or("missing-generation");
            let lease_id = format!("lease-{generation_id}");
            let mut attached = req.clone();
            for server in &mut attached.mcp_servers {
                if let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } = &server.transport
                {
                    let mut headers = BTreeMap::new();
                    headers.insert(
                        "Authorization".to_string(),
                        format!("Bearer token-{generation_id}"),
                    );
                    *server = AcpMcpServerPayload {
                        id: intent.runtime_id.clone(),
                        extension_id: intent.extension_id.clone(),
                        transport: ResolvedMcpServerTransport::Http {
                            url: format!("http://127.0.0.1:8123/xcode-mcp/{lease_id}"),
                            headers,
                        },
                    };
                }
            }
            Ok(BrokeredXcodeLeaseAttachment {
                request: attached,
                lease_ids: vec![lease_id],
            })
        }

        async fn release_brokered_xcode_leases(&self, lease_ids: &[String]) -> anyhow::Result<()> {
            self.released.lock().await.extend_from_slice(lease_ids);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let close_seen = tmp.path().join("close-seen.txt");
    let allow_close = tmp.path().join("allow-close.txt");
    let script =
        fixture::create_broker_attach_blocking_close_script(tmp.path(), &close_seen, &allow_close);
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script));
    let manager = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let released = Arc::new(Mutex::new(Vec::new()));
    manager.set_xcode_broker_lease_attacher(Arc::new(RecordingLeaseAttacher {
        released: Arc::clone(&released),
    }));

    for generation_id in ["generation-a", "generation-b"] {
        let mut req = brokered_xcode_request(&tmp, "claude");
        req.keep_session_alive = true;
        req.session_generation_id = Some(generation_id.to_string());
        let result = manager.start_session(req).await.unwrap();
        assert_eq!(result.status, AgentStatus::Completed);
        assert_eq!(result.session_generation_id.as_deref(), Some(generation_id));
    }

    let close_task = {
        let manager = Arc::clone(&manager);
        tokio::spawn(async move { manager.close_all_sessions().await })
    };

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let mut released_ids = released.lock().await.clone();
            released_ids.sort();
            if released_ids == ["lease-generation-a", "lease-generation-b"] {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("close_all_sessions should release leases before blocked session close completes");
    assert!(
        !close_task.is_finished(),
        "session close should still be blocked while leases are already released"
    );

    std::fs::write(&allow_close, "ok").unwrap();
    let closed = tokio::time::timeout(std::time::Duration::from_secs(5), close_task)
        .await
        .expect("close_all_sessions should finish after close unblocks")
        .expect("close task should not panic");
    assert_eq!(closed, 2);
    let close_lines = std::fs::read_to_string(close_seen).unwrap();
    assert_eq!(close_lines.lines().count(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_manager_releases_brokered_xcode_lease_when_kept_session_fails() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::{
        AcpMcpServerPayload, AcpRuntimeManager, BrokeredXcodeLeaseAttachment, ExecutionRequest,
        ResolvedMcpServerTransport, XcodeBrokerLeaseAttacher,
    };
    use domain::agent::AgentStatus;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct RecordingLeaseAttacher {
        released: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl XcodeBrokerLeaseAttacher for RecordingLeaseAttacher {
        async fn attach_brokered_xcode_leases(
            &self,
            req: &ExecutionRequest,
        ) -> anyhow::Result<BrokeredXcodeLeaseAttachment> {
            let mut attached = req.clone();
            for server in &mut attached.mcp_servers {
                if let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } = &server.transport
                {
                    let mut headers = BTreeMap::new();
                    headers.insert(
                        "Authorization".to_string(),
                        "Bearer token-failed".to_string(),
                    );
                    *server = AcpMcpServerPayload {
                        id: intent.runtime_id.clone(),
                        extension_id: intent.extension_id.clone(),
                        transport: ResolvedMcpServerTransport::Http {
                            url: "http://127.0.0.1:8123/xcode-mcp/lease-failed".to_string(),
                            headers,
                        },
                    };
                }
            }
            Ok(BrokeredXcodeLeaseAttachment {
                request: attached,
                lease_ids: vec!["lease-failed".to_string()],
            })
        }

        async fn release_brokered_xcode_leases(&self, lease_ids: &[String]) -> anyhow::Result<()> {
            self.released.lock().await.extend_from_slice(lease_ids);
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_fail_script(tmp.path());
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(script));
    let manager = AcpRuntimeManager::new_with_adapters(vec![adapter]);
    let released = Arc::new(Mutex::new(Vec::new()));
    manager.set_xcode_broker_lease_attacher(Arc::new(RecordingLeaseAttacher {
        released: Arc::clone(&released),
    }));

    let mut req = brokered_xcode_request(&tmp, "claude");
    req.keep_session_alive = true;
    req.session_generation_id = Some("generation-failed".to_string());
    let result = manager.start_session(req).await;

    assert!(
        result
            .as_ref()
            .is_ok_and(|result| result.status == AgentStatus::Failed)
            || result
                .as_ref()
                .is_err_and(|error| error.to_string().contains("session/close")),
        "failed prompt should not be kept alive even if close reports process exit: {result:?}"
    );
    assert_eq!(
        released.lock().await.as_slice(),
        ["lease-failed"],
        "failed kept session must release its brokered Xcode lease"
    );
    assert!(
        !manager.has_live_session("generation-failed", None).await,
        "failed prompt must not leave a reusable live session"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn unsupported_brokered_xcode_provider_fails_before_probe_spawn() {
    use acp::adapters::auggie::AuggieAdapter;
    use acp::adapters::AcpAdapter;

    let tmp = tempfile::tempdir().unwrap();
    let marker = tmp.path().join("should-not-spawn.txt");
    let script = tmp.path().join("auggie_should_not_spawn.sh");
    std::fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        let mut p = std::fs::metadata(&script).unwrap().permissions();
        p.set_mode(0o755);
        std::fs::set_permissions(&script, p).unwrap();
    }

    let adapter = AuggieAdapter::new_with_binary(script.to_string_lossy().into_owned());
    let req = brokered_xcode_request(&tmp, "auggie");
    let err = match adapter.open_session(&req).await {
        Ok(_) => panic!("Auggie must fail closed for brokered Xcode MCP"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("provider_http_mcp_unsupported"),
        "unexpected error: {err:#}"
    );
    assert!(
        !marker.exists(),
        "unsupported provider must fail before capability probe subprocess launch"
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
    let agent_execution_id = domain::ids::AgentExecutionId::new();
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: Some("stage-exec-typed".into()),
        stage_id: "stage_typed_expected_outputs".into(),
        attempt_number: 2,
        agent_execution_id: Some(agent_execution_id),
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        agent_execution_id.to_string()
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
async fn test_claude_adapter_extracts_chainworks_output_from_terminal_result_output_field() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_terminal_output_envelope_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_terminal_output_envelope".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "test-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "emit proposal review in terminal result output".into(),
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert!(
        result.artifact_paths.is_empty(),
        "terminal output envelope should not require filesystem artifacts: {:?}",
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
async fn p084_like_stringified_terminal_output_materializes_required_outputs() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::ExecutionRequest;
    use domain::agent::AgentStatus;
    use domain::discovery::{
        ExpectedOutputRole, ExpectedOutputSpec, OutputReusePolicy, SourceGenerationOwner,
    };
    use domain::ids::RunId;

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_p084_like_stringified_terminal_output_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let expected_outputs = vec![
        ExpectedOutputSpec {
            output_name: "implementation_progress".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: tmp
                .path()
                .join("implementation/progress.json")
                .to_string_lossy()
                .into_owned(),
            companion_of: None,
            display_label: "implementation progress".to_string(),
            contract_id: Some("implementation_progress".to_string()),
            required: true,
            reuse_policy: OutputReusePolicy::MustProduce,
            max_bytes: 16 * 1024,
            aggregate_acceptance_cap_bytes: 128 * 1024,
            authorized_roots: Vec::new(),
            source_generation_owner: SourceGenerationOwner::Agent,
        },
        ExpectedOutputSpec {
            output_name: "implementation_self_assessment".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: tmp
                .path()
                .join("implementation/self-assessment.json")
                .to_string_lossy()
                .into_owned(),
            companion_of: None,
            display_label: "implementation self assessment".to_string(),
            contract_id: Some("implementation_self_assessment_v2".to_string()),
            required: true,
            reuse_policy: OutputReusePolicy::MustProduce,
            max_bytes: 64 * 1024,
            aggregate_acceptance_cap_bytes: 128 * 1024,
            authorized_roots: Vec::new(),
            source_generation_owner: SourceGenerationOwner::Agent,
        },
        ExpectedOutputSpec {
            output_name: "tests_result".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: tmp
                .path()
                .join("implementation/tests-result.json")
                .to_string_lossy()
                .into_owned(),
            companion_of: None,
            display_label: "tests result".to_string(),
            contract_id: Some("tests_result".to_string()),
            required: true,
            reuse_policy: OutputReusePolicy::MustProduce,
            max_bytes: 16 * 1024,
            aggregate_acceptance_cap_bytes: 128 * 1024,
            authorized_roots: Vec::new(),
            source_generation_owner: SourceGenerationOwner::Agent,
        },
    ];
    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "state_8_implementation_continued".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "code_writer".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "emit P084-like required outputs".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs,
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };

    let result = adapter.execute(req).await.unwrap();

    assert_eq!(result.status, AgentStatus::Completed);
    assert!(result.artifact_paths.is_empty());
    assert_eq!(result.discovered_artifacts.len(), 3);
    for output_name in [
        "implementation_progress",
        "implementation_self_assessment",
        "tests_result",
    ] {
        assert!(
            result
                .discovered_artifacts
                .iter()
                .any(|artifact| artifact.name == output_name),
            "missing discovered output {output_name}: {:?}",
            result.discovered_artifacts
        );
    }
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
async fn runtime_manager_reports_prompt_progress_before_terminal_response() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::adapters::AcpAdapter;
    use acp::{
        AcpPromptProgressKind, AcpPromptProgressSink, AcpPromptProgressUpdate, AcpRuntimeManager,
        ExecutionRequest,
    };
    use anyhow::Result;
    use domain::ids::RunId;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio::time::{timeout, Duration};

    struct RecordingPromptProgressSink {
        tx: mpsc::UnboundedSender<AcpPromptProgressUpdate>,
    }

    #[async_trait::async_trait]
    impl AcpPromptProgressSink for RecordingPromptProgressSink {
        async fn record_acp_prompt_progress(&self, update: AcpPromptProgressUpdate) -> Result<()> {
            self.tx.send(update).unwrap();
            Ok(())
        }
    }

    let tmp = tempfile::tempdir().unwrap();
    let script = fixture::create_delayed_prompt_progress_script(tmp.path());
    let adapter = ClaudeAgentAdapter::new_with_binary(script);
    let manager = Arc::new(AcpRuntimeManager::new_with_adapters(vec![
        Arc::new(adapter) as Arc<dyn AcpAdapter>,
    ]));
    let (tx, mut rx) = mpsc::unbounded_channel();
    manager.set_prompt_progress_sink(Arc::new(RecordingPromptProgressSink { tx }));

    let req = ExecutionRequest {
        run_id: RunId::new(),
        stage_execution_id: None,
        stage_id: "stage_progress".into(),
        attempt_number: 1,
        agent_execution_id: None,
        agent_id: "progress-agent".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: tmp.path().to_string_lossy().into_owned(),
        prompt: "stream progress before terminal response".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: Vec::new(),
        expected_outputs: Vec::new(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: Some("generation-progress".into()),
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: None,
        legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::WorkflowOptIn,
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
    };

    let manager_for_task = Arc::clone(&manager);
    let task = tokio::spawn(async move { manager_for_task.start_session(req).await });
    let update = timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("prompt progress should be reported before terminal response")
        .expect("progress sink should receive an update");
    assert!(
        !task.is_finished(),
        "progress must be observable while session/prompt is still running"
    );
    assert_eq!(update.stage_id, "stage_progress");
    assert_eq!(update.agent_id, "progress-agent");
    assert_eq!(
        update.session_generation_id.as_deref(),
        Some("generation-progress")
    );
    assert!(matches!(
        update.kind,
        AcpPromptProgressKind::PromptSent
            | AcpPromptProgressKind::MessageReceived
            | AcpPromptProgressKind::MeaningfulProgress
            | AcpPromptProgressKind::ProviderLocalActivity
    ));

    let result = task
        .await
        .expect("prompt task should not panic")
        .expect("prompt should finish after delayed progress");
    assert_eq!(result.status, domain::agent::AgentStatus::Completed);
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: None,
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,
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
