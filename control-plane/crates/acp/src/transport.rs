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
use tracing::{debug, error, warn};

use crate::{DiscoveredArtifact, ExecutionRequest, McpActualObservation, UsageSnapshot};

/// Strip ANSI escape sequences from a string for clean log output.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip ESC [ ... m sequences
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

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
    /// Codex: base model id without effort suffix (e.g. `"gpt-5.4"`).
    ///        Reasoning effort is set separately via `config_options`.
    pub model: &'a str,

    /// Execution mode.
    /// Claude: `"bypassPermissions"` (autonomous write access).
    /// Codex:  `"full-access"`.
    pub mode: &'a str,

    /// Extra fields merged into `session/new` params.
    /// Claude requires `_meta.claudeCode.options`; Codex uses `None`.
    pub extra: Option<serde_json::Value>,

    /// Session config options to apply after `session/new` via
    /// `session/set_config_option`.
    ///
    /// Used by the Codex adapter to set `reasoning_effort` (the only reliable
    /// way to pin it — passing `gpt-5.4/high` in the `model` field silently
    /// falls back to `gpt-5.4/medium`).
    ///
    /// Each entry is `(configId, value)` sent as:
    /// `{"sessionId": "...", "configId": "<id>", "value": "<value>"}`.
    /// Errors are logged but do not fail the session (providers that don't
    /// support the method respond with `Method not found`).
    pub config_options: Vec<(String, String)>,
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
            config_options: Vec::new(),
        }
    }
}

pub fn build_session_new_params(
    req: &ExecutionRequest,
    config: &AcpSessionConfig<'_>,
) -> Result<Value> {
    let uses_worktree_cwd = req.worktree_write_enabled
        || matches!(
            req.worktree_strategy.as_deref(),
            Some("dedicated") | Some("shared_implementation_worktree")
        );
    let effective_cwd = if uses_worktree_cwd {
        req.worktree_root.as_deref().unwrap_or(&req.workspace_root)
    } else {
        &req.workspace_root
    };
    let mut sn_params = serde_json::json!({
        "mcpServers": serde_json::to_value(&req.mcp_servers)
            .context("ACP: serialize resolved MCP server payloads")?,
        "cwd": effective_cwd,
        "model": config.model,
        "mode": config.mode,
    });
    if let (Some(extra), Some(base_obj)) = (&config.extra, sn_params.as_object_mut()) {
        if let Some(extra_obj) = extra.as_object() {
            for (k, v) in extra_obj {
                base_obj.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(sn_params)
}

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// Max silence between messages before we consider the session hung.
/// Reset on every received line (including notifications).
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

const OUTPUT_START_MARKER: &str = "<<<CHAINWORKS_OUTPUT:";
const OUTPUT_END_MARKER: &str = "<<<END_CHAINWORKS_OUTPUT>>>";

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

fn extract_text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.to_string()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(extract_text_from_value)
                .filter(|part| !part.is_empty())
                .collect();
            (!parts.is_empty()).then(|| parts.join(""))
        }
        Value::Object(map) => {
            for key in ["text", "content", "message", "delta", "parts"] {
                if let Some(text) = map.get(key).and_then(extract_text_from_value) {
                    return Some(text);
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_text_chunk(parsed: &Value) -> Option<String> {
    let candidates = [
        parsed.pointer("/params/update"),
        parsed.pointer("/params"),
        parsed.pointer("/result"),
    ];

    candidates
        .into_iter()
        .flatten()
        .find_map(extract_text_from_value)
}

fn extract_int_from_value(value: &Value, keys: &[&str]) -> Option<i64> {
    match value {
        Value::Array(items) => items
            .iter()
            .find_map(|item| extract_int_from_value(item, keys)),
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(extract_scalar_i64) {
                    return Some(found);
                }
            }
            map.values()
                .find_map(|nested| extract_int_from_value(nested, keys))
        }
        _ => None,
    }
}

fn extract_scalar_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn extract_usage_snapshot(parsed: &Value) -> Option<UsageSnapshot> {
    let snapshot = UsageSnapshot {
        cost_cents: extract_int_from_value(parsed, &["cost_cents", "costCents"]),
        input_tokens: extract_int_from_value(
            parsed,
            &["input_tokens", "inputTokens", "token_count"],
        ),
        cached_input_tokens: extract_int_from_value(
            parsed,
            &["cached_input_tokens", "cachedInputTokens"],
        ),
        output_tokens: extract_int_from_value(parsed, &["output_tokens", "outputTokens"]),
        model_context_window: extract_int_from_value(
            parsed,
            &["model_context_window", "modelContextWindow"],
        ),
    };

    (snapshot.cost_cents.is_some()
        || snapshot.input_tokens.is_some()
        || snapshot.cached_input_tokens.is_some()
        || snapshot.output_tokens.is_some()
        || snapshot.model_context_window.is_some())
    .then_some(snapshot)
}

fn merge_usage_snapshot(existing: &mut Option<UsageSnapshot>, incoming: UsageSnapshot) {
    let current = existing.get_or_insert_with(UsageSnapshot::default);
    current.cost_cents = incoming.cost_cents.or(current.cost_cents);
    current.input_tokens = incoming.input_tokens.or(current.input_tokens);
    current.cached_input_tokens = incoming.cached_input_tokens.or(current.cached_input_tokens);
    current.output_tokens = incoming.output_tokens.or(current.output_tokens);
    current.model_context_window = incoming
        .model_context_window
        .or(current.model_context_window);
}

fn observe_mcp_actuals(
    session_new_result: &Value,
    req: &ExecutionRequest,
    provider_session_id: &str,
) -> Option<McpActualObservation> {
    if req.mcp_servers.is_empty() {
        return None;
    }

    let predicted_extensions: Vec<String> = req
        .mcp_servers
        .iter()
        .map(|server| server.extension_id.clone())
        .collect();
    let predicted_runtime_ids: Vec<String> = req
        .mcp_servers
        .iter()
        .map(|server| server.id.clone())
        .collect();

    let accepted_servers = session_new_result
        .get("acceptedMcpServers")
        .or_else(|| session_new_result.get("actualMcpServers"))
        .or_else(|| session_new_result.get("mcpServers"));

    if let Some(Value::Array(servers)) = accepted_servers {
        let mut actual_extensions = Vec::new();
        let mut actual_runtime_ids = Vec::new();
        for server in servers {
            match server {
                Value::String(id) => actual_runtime_ids.push(id.clone()),
                Value::Object(map) => {
                    if let Some(id) = map
                        .get("id")
                        .or_else(|| map.get("runtimeId"))
                        .or_else(|| map.get("runtime_id"))
                        .and_then(Value::as_str)
                    {
                        actual_runtime_ids.push(id.to_string());
                    }
                    if let Some(extension_id) = map
                        .get("extensionId")
                        .or_else(|| map.get("extension_id"))
                        .or_else(|| map.get("extension"))
                        .and_then(Value::as_str)
                    {
                        actual_extensions.push(extension_id.to_string());
                    }
                }
                _ => {}
            }
        }
        if actual_extensions.is_empty() {
            actual_extensions = actual_runtime_ids
                .iter()
                .filter_map(|runtime_id| {
                    req.mcp_servers
                        .iter()
                        .find(|server| &server.id == runtime_id)
                        .map(|server| server.extension_id.clone())
                })
                .collect();
        }
        let actual_equals_predicted = actual_extensions == predicted_extensions
            && actual_runtime_ids == predicted_runtime_ids;
        return Some(McpActualObservation {
            source: "provider_session_new_response".to_string(),
            trust_level: "provider_reported".to_string(),
            actual_equals_predicted,
            provider_session_id: Some(provider_session_id.to_string()),
            actual_extensions,
            actual_runtime_ids,
            notes: Vec::new(),
        });
    }

    Some(McpActualObservation {
        source: "predicted_after_successful_session_new".to_string(),
        trust_level: "assumed_after_successful_session_new".to_string(),
        actual_equals_predicted: true,
        provider_session_id: Some(provider_session_id.to_string()),
        actual_extensions: predicted_extensions,
        actual_runtime_ids: predicted_runtime_ids,
        notes: vec![
            "ACP provider did not return an accepted MCP server list; actual truth is an explicit fallback to the predicted payload after session/new succeeded."
                .to_string(),
        ],
    })
}

fn extract_output_envelopes(stream_text: &str) -> Vec<DiscoveredArtifact> {
    let mut artifacts = Vec::new();
    let mut cursor = 0usize;

    while let Some(start_rel) = stream_text[cursor..].find(OUTPUT_START_MARKER) {
        let start = cursor + start_rel;
        let header_start = start + OUTPUT_START_MARKER.len();
        let Some(header_end_rel) = stream_text[header_start..].find(">>>") else {
            break;
        };
        let header_end = header_start + header_end_rel;
        let output_name = stream_text[header_start..header_end].trim();
        if output_name.is_empty() {
            cursor = header_end + 3;
            continue;
        }

        let content_start = header_end + 3;
        let Some(end_rel) = stream_text[content_start..].find(OUTPUT_END_MARKER) else {
            break;
        };
        let content_end = content_start + end_rel;
        let content = stream_text[content_start..content_end].to_string();
        artifacts.push(DiscoveredArtifact {
            name: output_name.to_string(),
            content: content.into_bytes(),
            source_path: None,
        });
        cursor = content_end + OUTPUT_END_MARKER.len();
    }

    artifacts
}

// ---------------------------------------------------------------------------
// ndjson write
// ---------------------------------------------------------------------------

async fn send_ndjson(stdin: &mut tokio::process::ChildStdin, msg: &Value) -> Result<()> {
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
                debug!("ACP non-JSON line during handshake: {:.200} ({e})", trimmed);
                continue;
            }
        };
        debug!(msg = %trimmed, "ACP ← subprocess (handshake)");

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
            debug!("ACP: skipping response id={id} (expected {expected_id}) during handshake");
            continue;
        }

        if let Some(err) = parsed.get("error") {
            let msg = err["message"].as_str().unwrap_or("unknown ACP error");
            error!("ACP error response for id={expected_id}: {msg}");
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
// Live transport-backed sessions
// ---------------------------------------------------------------------------

pub struct AcpTransportSession {
    child: Child,
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    session_id: String,
    mcp_observation: Option<McpActualObservation>,
    mcp_session_startup_latency_ms: Option<i64>,
    snapshot_root: String,
    baseline_files: HashSet<String>,
    request_counter: u64,
    closed: bool,
}

impl AcpTransportSession {
    pub async fn start(
        mut child: Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
    ) -> Result<Self> {
        let startup_started = Instant::now();
        let mut stdin = child
            .stdin
            .take()
            .context("ACP subprocess has no stdin pipe (was it spawned with Stdio::piped()?)")?;
        let stdout = child
            .stdout
            .take()
            .context("ACP subprocess has no stdout pipe")?;
        if let Some(stderr) = child.stderr.take() {
            let run_id = req.run_id;
            let stage_id = req.stage_id.clone();
            let provider = req.provider.clone();
            let log_path = format!("{}/.chainworks/acp-stderr.log", req.workspace_root);
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                let mut log_file = match tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .await
                {
                    Ok(f) => f,
                    Err(_) => {
                        return;
                    }
                };
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            let clean = strip_ansi(trimmed);
                            if clean.is_empty() {
                                continue;
                            }
                            if clean.contains("ERROR") || clean.contains("Unhandled error") {
                                error!(
                                    run_id = %run_id,
                                    stage_id = %stage_id,
                                    provider = %provider,
                                    "{clean}"
                                );
                            } else if clean.contains("WARN") || clean.contains("usage_limit") {
                                warn!(run_id = %run_id, provider = %provider, "{clean}");
                            } else {
                                debug!(run_id = %run_id, provider = %provider, "{clean}");
                            }
                            let timestamp = chrono::Utc::now().to_rfc3339();
                            let _ = tokio::io::AsyncWriteExt::write_all(
                                &mut log_file,
                                format!("[{timestamp}] [{provider}] {clean}\n").as_bytes(),
                            )
                            .await;
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        let mut reader = BufReader::new(stdout);
        let mut req_counter: u64 = 0;
        macro_rules! next_id {
            () => {{
                req_counter += 1;
                req_counter
            }};
        }

        let snapshot_root = if req.worktree_write_enabled {
            req.worktree_root
                .as_deref()
                .unwrap_or(&req.workspace_root)
                .to_string()
        } else {
            req.workspace_root.clone()
        };
        let baseline_files = snapshot_workspace(&snapshot_root);

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

        let sn_id = next_id!();
        let sn_params =
            build_session_new_params(req, config).context("ACP: build session/new params")?;
        {
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
            .ok_or_else(|| anyhow::anyhow!("ACP session/new response missing 'sessionId' field"))?
            .to_string();
        let mcp_observation = observe_mcp_actuals(&sn_result, req, &session_id);
        let mcp_session_startup_latency_ms = mcp_observation
            .as_ref()
            .map(|_| startup_started.elapsed().as_millis() as i64);

        for (config_id, value) in &config.config_options {
            let sco_id = next_id!();
            if let Err(e) = send_ndjson(
                &mut stdin,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": sco_id,
                    "method": "session/set_config_option",
                    "params": {
                        "sessionId": session_id,
                        "configId": config_id,
                        "value": value,
                    }
                }),
            )
            .await
            {
                warn!(
                    session_id = %session_id,
                    config_id = %config_id,
                    "ACP: failed to send session/set_config_option: {e}"
                );
                continue;
            }

            match await_response(&mut reader, sco_id, HANDSHAKE_TIMEOUT).await {
                Ok(_) => {
                    debug!(
                        session_id = %session_id,
                        config_id = %config_id,
                        value = %value,
                        "ACP: session/set_config_option applied"
                    );
                }
                Err(e) => {
                    warn!(
                        session_id = %session_id,
                        config_id = %config_id,
                        value = %value,
                        "ACP: session/set_config_option rejected: {e}"
                    );
                }
            }
        }

        Ok(Self {
            child,
            stdin,
            reader,
            session_id,
            mcp_observation,
            mcp_session_startup_latency_ms,
            snapshot_root,
            baseline_files,
            request_counter: req_counter,
            closed: false,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn mcp_observation(&self) -> Option<McpActualObservation> {
        self.mcp_observation.clone()
    }

    pub fn mcp_session_startup_latency_ms(&self) -> Option<i64> {
        self.mcp_session_startup_latency_ms
    }

    pub async fn prompt(
        &mut self,
        req: &ExecutionRequest,
    ) -> Result<(
        AgentStatus,
        Vec<String>,
        Vec<DiscoveredArtifact>,
        Option<UsageSnapshot>,
    )> {
        self.request_counter += 1;
        let prompt_id = self.request_counter;
        send_ndjson(
            &mut self.stdin,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": prompt_id,
                "method": "session/prompt",
                "params": {
                    "sessionId": self.session_id,
                    "prompt": [{"type": "text", "text": req.prompt}]
                }
            }),
        )
        .await
        .context("ACP: send session/prompt")?;

        let mut line = String::new();
        let mut last_activity = Instant::now();
        let mut streamed_text = String::new();
        let mut latest_usage_snapshot = None;

        'streaming: loop {
            let idle = last_activity.elapsed();
            if idle >= IDLE_TIMEOUT {
                bail!(
                    "ACP session idle timeout: no message for {}s (session={})",
                    IDLE_TIMEOUT.as_secs(),
                    self.session_id
                );
            }
            let remaining = IDLE_TIMEOUT - idle;

            line.clear();
            let n = timeout(remaining, self.reader.read_line(&mut line))
                .await
                .context("ACP session idle timeout — no message received")?
                .context("ACP prompt stream read_line error")?;

            if n == 0 {
                bail!(
                    "ACP stdout closed before terminal response (session={})",
                    self.session_id
                );
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parsed: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue,
            };
            last_activity = Instant::now();
            debug!(msg = %trimmed, "ACP ← subprocess (stream)");
            if let Some(snapshot) = extract_usage_snapshot(&parsed) {
                merge_usage_snapshot(&mut latest_usage_snapshot, snapshot);
            }

            let msg_id: Option<u64> = match parsed.get("id") {
                Some(Value::Number(n)) => n.as_u64(),
                Some(Value::String(s)) => s.parse().ok(),
                _ => None,
            };

            if let Some(method) = parsed["method"].as_str() {
                match method {
                    "session/request_permission" => {
                        if let Some(req_id) = parsed.get("id") {
                            let params = parsed.get("params").cloned().unwrap_or(Value::Null);
                            debug!(
                                session_id = %self.session_id,
                                "ACP: auto-granting permission request id={req_id}"
                            );
                            if let Some(grant) = build_permission_grant(req_id, &params) {
                                if let Err(e) = send_ndjson(&mut self.stdin, &grant).await {
                                    warn!(
                                        session_id = %self.session_id,
                                        "ACP: failed to send permission grant: {e}"
                                    );
                                }
                            }
                        }
                        continue;
                    }
                    "session/update" => {
                        debug!(session_id = %self.session_id, "ACP: session/update notification");
                        if let Some(chunk) = extract_text_chunk(&parsed) {
                            streamed_text.push_str(&strip_ansi(&chunk));
                        }
                        continue;
                    }
                    _ => {
                        debug!(method = method, session_id = %self.session_id, "ACP: notification");
                        continue;
                    }
                }
            }

            if let Some(id) = msg_id {
                if id == prompt_id {
                    if parsed.get("error").is_some() {
                        let err_msg = parsed["error"]["message"].as_str().unwrap_or("ACP error");
                        warn!(
                            session_id = %self.session_id,
                            "ACP session/prompt returned error: {err_msg}"
                        );
                        return Ok((AgentStatus::Failed, vec![], vec![], latest_usage_snapshot));
                    }
                    if let Some(chunk) = extract_text_chunk(&parsed) {
                        streamed_text.push_str(&strip_ansi(&chunk));
                    }
                    break 'streaming;
                }
                continue;
            }
        }

        let post_files = snapshot_workspace(&self.snapshot_root);
        let mut new_files: Vec<String> = post_files
            .difference(&self.baseline_files)
            .cloned()
            .collect();
        for path in &req.expected_output_paths {
            if std::path::Path::new(path).is_file() && !new_files.iter().any(|p| p == path) {
                new_files.push(path.clone());
            }
        }
        new_files.sort();
        self.baseline_files = post_files;

        let mut discovered_artifacts = extract_output_envelopes(&streamed_text);
        for path in &new_files {
            let path_obj = Path::new(path);
            let name = path_obj
                .file_stem()
                .or_else(|| path_obj.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("artifact")
                .to_string();
            if discovered_artifacts
                .iter()
                .any(|artifact| artifact.name == name)
            {
                continue;
            }
            if let Ok(content) = std::fs::read(path_obj) {
                discovered_artifacts.push(DiscoveredArtifact {
                    name,
                    content,
                    source_path: Some(path.clone()),
                });
            }
        }

        Ok((
            AgentStatus::Completed,
            new_files,
            discovered_artifacts,
            latest_usage_snapshot,
        ))
    }

    pub async fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }

        self.request_counter += 1;
        let close_id = self.request_counter;
        let _ = send_ndjson(
            &mut self.stdin,
            &serde_json::json!({
                "jsonrpc": "2.0",
                "id": close_id,
                "method": "session/close",
                "params": {"sessionId": self.session_id}
            }),
        )
        .await;

        let _ = AsyncWriteExt::shutdown(&mut self.stdin).await;

        let exit_success = match timeout(SHUTDOWN_WAIT, self.child.wait()).await {
            Ok(Ok(status)) => {
                debug!(exit_status = ?status, session_id = %self.session_id, "ACP subprocess exited");
                status.success()
            }
            _ => {
                debug!(
                    session_id = %self.session_id,
                    "ACP subprocess did not exit within {}s — force-killing",
                    SHUTDOWN_WAIT.as_secs()
                );
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
                false
            }
        };

        self.closed = true;
        if !exit_success {
            warn!(session_id = %self.session_id, "ACP subprocess exited with non-zero status");
        }
        Ok(())
    }
}

/// Execute the full ACP JSON-RPC 2.0 protocol with an already-spawned subprocess.
///
/// This remains the one-shot convenience wrapper used by existing adapters.
pub async fn run_acp_session(
    child: Child,
    req: &ExecutionRequest,
    config: &AcpSessionConfig<'_>,
) -> Result<(AgentStatus, Vec<String>, Vec<DiscoveredArtifact>)> {
    let mut session = AcpTransportSession::start(child, req, config).await?;
    let (status, paths, artifacts, _usage) = session.prompt(req).await?;
    let _ = session.close().await;
    Ok((status, paths, artifacts))
}
