//! ACP JSON-RPC 2.0 subprocess transport.
//!
//! Implements the wire protocol documented in `ClaudeAgentACPTransport.swift` and
//! `CodexACPTransport.swift`:
//! - ndjson over stdio (one JSON object per line, `\n` delimited)
//! - Three-phase handshake: `initialize` → `session/new` → `session/prompt`
//! - Streaming `session/update` notifications during the prompt phase
//! - Auto-grant for `session/request_permission` (selects `allow_once` first)
//! - `session/close` + graceful SIGTERM/SIGKILL subprocess shutdown
//! - Artifact discovery via workspace filesystem diff (pre- vs post-session)
//!
//! Provider differences are expressed through [`AcpSessionConfig`]:
//! - Claude: `mode = "bypassPermissions"`, includes `_meta.claudeCode.options`
//! - Codex:  `mode = "full-access"`, no `_meta` block
//! - Gemini, Auggie, Junie: `mode = "bypassPermissions"`, no `_meta` block

use anyhow::{bail, Context, Result};
use domain::agent::AgentStatus;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::ExecutionRequest;

// ---------------------------------------------------------------------------
// Per-adapter session configuration
// ---------------------------------------------------------------------------

/// Configuration that adapters inject into the `session/new` request.
///
/// Each ACP provider has a slightly different parameter vocabulary for
/// `session/new`. This struct lets each adapter specify its needs without
/// duplicating the rest of the transport logic.
pub struct AcpSessionConfig<'a> {
    /// Model identifier for the provider's model catalog.
    /// Claude: `"default"` / `"sonnet"` / `"haiku"` / `"opus"`.
    /// Codex: `"gpt-5"` / `"gpt-5-codex"` / `"o4-mini"`.
    pub model: &'a str,

    /// Execution mode.
    /// Claude: `"bypassPermissions"` (autonomous write access).
    /// Codex:  `"full-access"`.
    pub mode: &'a str,

    /// Extra fields merged into `session/new` params.
    /// Claude requires `_meta.claudeCode.options`; Codex uses `None`.
    pub extra: Option<serde_json::Value>,
}

impl Default for AcpSessionConfig<'_> {
    /// Claude-compatible defaults used by the Claude adapter.
    fn default() -> Self {
        Self {
            model: "default",
            mode: "bypassPermissions",
            extra: Some(serde_json::json!({
                "_meta": {
                    "claudeCode": {
                        "options": {
                            "enabledPlugins": {},
                            "mcpServers": {}
                        }
                    }
                }
            })),
        }
    }
}

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const PROMPT_TIMEOUT: Duration = Duration::from_secs(300);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Workspace snapshot — used for artifact discovery
// ---------------------------------------------------------------------------

fn collect_files(dir: &Path, out: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            // Skip hidden directories (e.g. .git, .claude)
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            collect_files(&path, out);
        } else if path.is_file() {
            if let Some(s) = path.to_str() {
                out.insert(s.to_string());
            }
        }
    }
}

fn snapshot_workspace(root: &str) -> HashSet<String> {
    let mut files = HashSet::new();
    collect_files(Path::new(root), &mut files);
    files
}

// ---------------------------------------------------------------------------
// ndjson write
// ---------------------------------------------------------------------------

async fn send_ndjson(
    stdin: &mut tokio::process::ChildStdin,
    msg: &Value,
) -> Result<()> {
    let mut line = serde_json::to_string(msg).context("serialize ACP JSON-RPC message")?;
    line.push('\n');
    stdin
        .write_all(line.as_bytes())
        .await
        .context("write ACP message to subprocess stdin")?;
    debug!(msg = %line.trim_end(), "ACP → subprocess");
    Ok(())
}

// ---------------------------------------------------------------------------
// Handshake response reader — blocks until a response with `expected_id` arrives.
// Notifications (no `id` field) are silently skipped.
// ---------------------------------------------------------------------------

async fn await_response(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    expected_id: u64,
    time_limit: Duration,
) -> Result<Value> {
    let start = Instant::now();
    let mut line = String::new();

    loop {
        let elapsed = start.elapsed();
        if elapsed >= time_limit {
            bail!("ACP handshake timed out waiting for response id={expected_id}");
        }
        let remaining = time_limit - elapsed;

        line.clear();
        let n = timeout(remaining, reader.read_line(&mut line))
            .await
            .context("ACP handshake read timeout")?
            .context("ACP handshake read_line error")?;

        if n == 0 {
            bail!("ACP subprocess stdout closed before responding to id={expected_id}");
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(e) => {
                warn!("ACP non-JSON line during handshake: {:.200} ({e})", trimmed);
                continue;
            }
        };

        // Extract response id — ACP may encode it as integer or string
        let msg_id: Option<u64> = match parsed.get("id") {
            Some(Value::Number(n)) => n.as_u64(),
            Some(Value::String(s)) => s.parse().ok(),
            _ => None,
        };

        let Some(id) = msg_id else {
            // Notification (no id field) — skip during handshake phase
            continue;
        };

        if id != expected_id {
            debug!(
                "ACP: skipping response id={id} (expected {expected_id}) during handshake"
            );
            continue;
        }

        if let Some(err) = parsed.get("error") {
            let msg = err["message"].as_str().unwrap_or("unknown ACP error");
            bail!("ACP error response for id={expected_id}: {msg}");
        }

        return Ok(parsed
            .get("result")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default())));
    }
}

// ---------------------------------------------------------------------------
// Permission auto-grant
// ---------------------------------------------------------------------------

/// Build a JSON-RPC result response that grants the first `allow_once` option
/// (or `approved` as a fallback) found in `session/request_permission` params.
/// Matches ACPProtocolSupport.swift `permissionSelectionResponse`.
fn build_permission_grant(request_id: &Value, params: &Value) -> Option<Value> {
    // Options may be at params["options"] or params["toolCall"]["options"]
    let options: Vec<&Value> = params["options"]
        .as_array()
        .map(|a| a.iter().collect())
        .or_else(|| {
            params["toolCall"]["options"]
                .as_array()
                .map(|a| a.iter().collect())
        })
        .unwrap_or_default();

    let option_id = options
        .iter()
        .find(|o| o["kind"].as_str() == Some("allow_once"))
        .and_then(|o| o["optionId"].as_str())
        .or_else(|| {
            options
                .iter()
                .find(|o| o["optionId"].as_str() == Some("approved"))
                .and_then(|o| o["optionId"].as_str())
        })?;

    Some(serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "result": {
            "outcome": {
                "outcome": "selected",
                "optionId": option_id
            }
        }
    }))
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Execute the full ACP JSON-RPC 2.0 protocol with an already-spawned subprocess.
///
/// This function owns the subprocess stdio from the moment it is called.
/// The caller provides `child` with `stdin`, `stdout`, and optionally `stderr`
/// already piped.
///
/// ## Protocol flow (matches ClaudeAgentACPTransport.swift / CodexACPTransport.swift)
///
/// 1. **`initialize`** — establish protocol version and client identity.
/// 2. **`session/new`** — start an agent session; receive back a `sessionId`.
///    `config` specifies the `model`, `mode`, and optional extra fields (e.g. `_meta`).
/// 3. **`session/prompt`** — submit the prompt and stream `session/update`
///    notifications until the terminal response with the matching id arrives.
///    `session/request_permission` notifications are auto-granted.
/// 4. **`session/close`** — clean shutdown request before dropping stdin.
/// 5. Graceful process wait (up to [`SHUTDOWN_WAIT`]), then `SIGKILL`.
///
/// ## Artifact discovery
///
/// New regular files created inside `req.workspace_root` during the session
/// are returned as `artifact_paths` (workspace diff: post-session − pre-session).
pub async fn run_acp_session(
    child: &mut Child,
    req: &ExecutionRequest,
    config: &AcpSessionConfig<'_>,
) -> Result<(AgentStatus, Vec<String>)> {
    let mut stdin = child
        .stdin
        .take()
        .context("ACP subprocess has no stdin pipe (was it spawned with Stdio::piped()?)")?;
    let stdout = child
        .stdout
        .take()
        .context("ACP subprocess has no stdout pipe")?;
    let mut reader = BufReader::new(stdout);
    let mut req_counter: u64 = 0;

    macro_rules! next_id {
        () => {{
            req_counter += 1;
            req_counter
        }};
    }

    // Snapshot workspace *before* the session so we can diff for new files.
    let pre_files = snapshot_workspace(&req.workspace_root);

    // ── Phase 1: initialize ──────────────────────────────────────────────────
    let init_id = next_id!();
    send_ndjson(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": init_id,
            "method": "initialize",
            "params": {
                "protocolVersion": 1,
                "clientInfo": {
                    "name": "chainworks-control-plane",
                    "version": "0.1.0"
                }
            }
        }),
    )
    .await
    .context("ACP: send initialize")?;

    await_response(&mut reader, init_id, HANDSHAKE_TIMEOUT)
        .await
        .context("ACP: initialize handshake")?;

    // ── Phase 2: session/new ─────────────────────────────────────────────────
    let sn_id = next_id!();
    {
        // Build session/new params: fixed fields + adapter-specific extras.
        let mut sn_params = serde_json::json!({
            "mcpServers": [],
            "cwd": req.workspace_root,
            "model": config.model,
            "mode": config.mode,
        });
        // Merge optional extra fields (e.g. `_meta` for Claude, absent for Codex).
        if let (Some(extra), Some(base_obj)) = (&config.extra, sn_params.as_object_mut()) {
            if let Some(extra_obj) = extra.as_object() {
                for (k, v) in extra_obj {
                    base_obj.insert(k.clone(), v.clone());
                }
            }
        }
        send_ndjson(
            &mut stdin,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": sn_id,
                "method": "session/new",
                "params": sn_params,
            }),
        )
        .await
        .context("ACP: send session/new")?;
    }

    let sn_result = await_response(&mut reader, sn_id, HANDSHAKE_TIMEOUT)
        .await
        .context("ACP: session/new handshake")?;

    let session_id = sn_result["sessionId"]
        .as_str()
        .ok_or_else(|| {
            anyhow::anyhow!("ACP session/new response missing 'sessionId' field")
        })?
        .to_string();

    // ── Phase 3: session/prompt — streaming ──────────────────────────────────
    let prompt_id = next_id!();
    send_ndjson(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": prompt_id,
            "method": "session/prompt",
            "params": {
                "sessionId": session_id,
                "prompt": [{"type": "text", "text": req.prompt}]
            }
        }),
    )
    .await
    .context("ACP: send session/prompt")?;

    let stream_start = Instant::now();
    let mut agent_failed = false;
    let mut line = String::new();

    'streaming: loop {
        let elapsed = stream_start.elapsed();
        if elapsed >= PROMPT_TIMEOUT {
            bail!(
                "ACP session/prompt timed out after {}s (session={session_id})",
                PROMPT_TIMEOUT.as_secs()
            );
        }
        let remaining = PROMPT_TIMEOUT - elapsed;

        line.clear();
        let n = timeout(remaining, reader.read_line(&mut line))
            .await
            .context("ACP prompt stream read timeout")?
            .context("ACP prompt stream read_line error")?;

        if n == 0 {
            // Subprocess closed stdout before sending the terminal result.
            warn!(
                session_id = %session_id,
                "ACP: stdout closed before terminal result — treating as agent failure"
            );
            agent_failed = true;
            break 'streaming;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Terminal response: has an `id` that matches `prompt_id`
        let msg_id: Option<u64> = match parsed.get("id") {
            Some(Value::Number(n)) => n.as_u64(),
            Some(Value::String(s)) => s.parse().ok(),
            _ => None,
        };

        if let Some(id) = msg_id {
            if id == prompt_id {
                if parsed.get("error").is_some() {
                    let err_msg = parsed["error"]["message"]
                        .as_str()
                        .unwrap_or("ACP error");
                    warn!(
                        session_id = %session_id,
                        "ACP session/prompt returned error: {err_msg}"
                    );
                    agent_failed = true;
                }
                break 'streaming;
            }
            // Response for a different id — skip
            continue;
        }

        // Notification: no `id` field (or null id)
        if let Some(method) = parsed["method"].as_str() {
            match method {
                "session/request_permission" => {
                    // Auto-grant permissions so the agent can proceed unblocked.
                    if let Some(req_id) = parsed.get("id") {
                        let params = parsed
                            .get("params")
                            .cloned()
                            .unwrap_or(Value::Null);
                        if let Some(grant) = build_permission_grant(req_id, &params) {
                            if let Err(e) = send_ndjson(&mut stdin, &grant).await {
                                warn!(
                                    session_id = %session_id,
                                    "ACP: failed to send permission grant: {e}"
                                );
                            }
                        }
                    }
                }
                "session/update" => {
                    debug!(session_id = %session_id, "ACP: session/update notification");
                }
                other => {
                    debug!(method = other, session_id = %session_id, "ACP: notification");
                }
            }
        }
    }

    // ── Phase 4: session/close ───────────────────────────────────────────────
    let close_id = next_id!();
    // Best-effort: some providers (e.g. early Gemini versions) don't support
    // session/close, so we silently ignore errors here.
    let _ = send_ndjson(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": close_id,
            "method": "session/close",
            "params": {"sessionId": session_id}
        }),
    )
    .await;

    // Dropping stdin sends EOF — signals the subprocess to exit cleanly.
    drop(stdin);

    // Wait up to SHUTDOWN_WAIT for a graceful exit, then force-kill.
    match timeout(SHUTDOWN_WAIT, child.wait()).await {
        Ok(Ok(status)) => {
            debug!(
                exit_status = ?status,
                session_id = %session_id,
                "ACP subprocess exited gracefully"
            );
        }
        _ => {
            warn!(
                session_id = %session_id,
                "ACP subprocess did not exit within {}s — force-killing",
                SHUTDOWN_WAIT.as_secs()
            );
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }

    if agent_failed {
        return Ok((AgentStatus::Failed, vec![]));
    }

    // ── Artifact discovery: workspace filesystem diff ────────────────────────
    let post_files = snapshot_workspace(&req.workspace_root);
    let mut new_files: Vec<String> = post_files
        .difference(&pre_files)
        .cloned()
        .collect();
    new_files.sort(); // deterministic ordering

    Ok((AgentStatus::Completed, new_files))
}
