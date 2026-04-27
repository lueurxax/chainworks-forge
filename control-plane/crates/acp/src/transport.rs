//! ACP JSON-RPC 2.0 subprocess transport.
//!
//! Implements the wire protocol documented in `ClaudeAgentACPTransport.swift` and
//! `CodexACPTransport.swift`:
//! - ndjson over stdio (one JSON object per line, `\n` delimited)
//! - Three-phase handshake: `initialize` → `session/new` → `session/prompt`
//! - Streaming `session/update` notifications during the prompt phase
//! - Auto-grant for `session/request_permission` (selects `allow_once` first)
//! - `session/close` + graceful SIGTERM/SIGKILL subprocess shutdown
//! - Artifact discovery via workspace filesystem diff captured after ACP startup
//!
//! Provider differences are expressed through [`AcpSessionConfig`]:
//! - Claude: `mode = "bypassPermissions"`, includes `_meta.claudeCode.options`
//! - Codex:  `mode = "full-access"`, no `_meta` block
//! - Gemini, Auggie, Junie: `mode = "bypassPermissions"`, no `_meta` block

use anyhow::{bail, Context, Result};
use domain::agent::AgentStatus;
use domain::discovery::{
    DiscoveryFilesystem, ExpectedOutputSpec, ExpectedPathBaseline, ExpectedPathBaselineStatus,
    LegacyBroadDiscoverySnapshot, NoopDiscoveryOperationRecorder, PrePromptExpectedOutputContext,
    PrePromptExpectedOutputMetadata, StdDiscoveryFilesystem,
};
use domain::xcode_runtime::XcodeShimWarningEvent;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::{
    AcpCloseDiagnostic, AcpMcpServerPayload, DiscoveredArtifact, DiscoveredArtifactSourceKind,
    ExecutionRequest, McpActualObservation, ResolvedMcpServerTransport, UsageSnapshot,
};

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

    /// Some ACP providers expose a mode catalog in `session/new` but do not
    /// apply `session/new.params.mode`. For those providers, send
    /// `session/set_mode` with `modeId` immediately after session creation.
    pub set_mode_after_session_new: bool,
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
            set_mode_after_session_new: false,
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
        "mcpServers": mcp_servers_wire_value(&req.mcp_servers)
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

fn mcp_servers_wire_value(servers: &[AcpMcpServerPayload]) -> Result<Value> {
    let mut wire_servers = Vec::with_capacity(servers.len());
    for server in servers {
        match &server.transport {
            ResolvedMcpServerTransport::Stdio { command, args, env } => {
                let env_vars: Vec<Value> = env
                    .iter()
                    .map(|(name, value)| {
                        serde_json::json!({
                            "name": name,
                            "value": value,
                        })
                    })
                    .collect();
                wire_servers.push(serde_json::json!({
                    "name": server.id,
                    "type": "stdio",
                    "command": command,
                    "args": args,
                    "env": env_vars,
                }));
            }
            ResolvedMcpServerTransport::Platform { provider } => {
                bail!(
                    "ACP: MCP extension '{}' resolved to platform provider '{}' but ACP session/new requires concrete server transport",
                    server.extension_id,
                    provider
                );
            }
            ResolvedMcpServerTransport::Http { url, headers } => {
                let header_values: Vec<Value> = headers
                    .iter()
                    .map(|(name, value)| {
                        serde_json::json!({
                            "name": name,
                            "value": value,
                        })
                    })
                    .collect();
                wire_servers.push(serde_json::json!({
                    "name": server.id,
                    "type": "http",
                    "url": url,
                    "headers": header_values,
                }));
            }
            ResolvedMcpServerTransport::XcodeBrokerIntent { intent } => {
                bail!(
                    "ACP: brokered Xcode MCP intent '{}' must be converted to an HTTP lease before session/new",
                    intent.runtime_id
                );
            }
        }
    }

    Ok(Value::Array(wire_servers))
}

const DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(90);
const GEMINI_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(120);
const HANDSHAKE_TIMEOUT: Duration = DEFAULT_HANDSHAKE_TIMEOUT;
/// Max silence between messages before we consider the session hung.
/// Reset on every received line (including notifications).
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
/// Max time without meaningful agent progress while the transport remains alive.
/// Keepalive/status-only ACP messages reset transport idleness, but not progress.
const PROGRESS_TIMEOUT: Duration = Duration::from_secs(300);
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
const LATE_RESPONSE_DIAGNOSTIC_WINDOW: Duration = Duration::from_secs(2);

enum AcpPromptReadOutcome {
    Read(std::result::Result<Result<usize>, tokio::time::error::Elapsed>),
    CloseRequested,
}

const OUTPUT_START_MARKER: &str = "<<<CHAINWORKS_OUTPUT:";
const OUTPUT_END_MARKER: &str = "<<<END_CHAINWORKS_OUTPUT>>>";
const RESIDUAL_XCODE_PATH_PATTERNS: &[&str] = &[
    "/Applications/Xcode.app",
    "/Library/Developer",
    "com.apple.CoreSimulator",
    "xcrun simctl",
    "xcodebuild",
];
const DEFAULT_PROVIDER_ENVELOPE_MAX_BYTES: usize = 10 * 1024 * 1024;
const ACP_NDJSON_LINE_OVERHEAD_BYTES: usize = 64 * 1024;
const MAX_STREAMED_TRANSCRIPT_BYTES: usize = 10 * 1024 * 1024;
const STREAMED_TRANSCRIPT_TRUNCATION_MARKER: &str =
    "\n[chainworks transcript truncated at 10485760 bytes]\n";

pub(crate) fn handshake_timeout_for_provider(provider: &str) -> Duration {
    if provider.eq_ignore_ascii_case("gemini") {
        GEMINI_HANDSHAKE_TIMEOUT
    } else {
        DEFAULT_HANDSHAKE_TIMEOUT
    }
}

/// Put each provider subprocess in its own process group so forced shutdown can
/// reap MCP/plugin children that outlive the ACP parent process.
pub fn isolate_process_group(cmd: &mut tokio::process::Command) {
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }
}

#[cfg(unix)]
pub(crate) fn signal_process_group(pid: u32, signal: libc::c_int) {
    if pid > libc::pid_t::MAX as u32 {
        return;
    }
    // Safety: killpg only observes the numeric process-group id and signal.
    let rc = unsafe { libc::killpg(pid as libc::pid_t, signal) };
    if rc != 0 {
        let error = std::io::Error::last_os_error();
        let errno = error.raw_os_error();
        if errno != Some(libc::ESRCH) {
            warn!(
                pid,
                signal,
                error = %error,
                "ACP process-group signal failed"
            );
        }
    }
}

fn stderr_line_is_diagnostic_warning(line: &str) -> bool {
    line.contains("EPIPE") || line.contains("write EPIPE")
}

fn legacy_broad_file_modified_after_prompt_start(
    path: &str,
    prompt_started_at: SystemTime,
) -> bool {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .is_ok_and(|modified| modified >= prompt_started_at)
}

fn pre_prompt_expected_output_context(
    req: &ExecutionRequest,
    session_id: &str,
    prompt_id: u64,
) -> PrePromptExpectedOutputContext {
    PrePromptExpectedOutputContext {
        agent_execution_id: req
            .agent_execution_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| req.agent_id.clone()),
        stage_execution_id: req
            .stage_execution_id
            .clone()
            .unwrap_or_else(|| req.stage_id.clone()),
        attempt_number: req.attempt_number,
        session_generation_id: req
            .session_generation_id
            .clone()
            .unwrap_or_else(|| session_id.to_string()),
        prompt_turn_id: format!("prompt-{prompt_id}"),
        discovery_generation_id: uuid::Uuid::new_v4().to_string(),
    }
}

fn capture_pre_prompt_expected_outputs(
    filesystem: &dyn DiscoveryFilesystem,
    req: &ExecutionRequest,
    context: &PrePromptExpectedOutputContext,
) -> Vec<PrePromptExpectedOutputMetadata> {
    req.expected_outputs
        .iter()
        .map(|spec| {
            let recorder = NoopDiscoveryOperationRecorder;
            filesystem
                .capture_pre_prompt_expected_output_metadata_with_recorder(spec, context, &recorder)
        })
        .collect()
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

fn residual_xcode_path_warnings_from_update(parsed: &Value) -> Vec<XcodeShimWarningEvent> {
    if parsed.get("method").and_then(Value::as_str) != Some("session/update") {
        return Vec::new();
    }

    let mut warnings = Vec::new();
    if let Some(params) = parsed.get("params") {
        collect_residual_xcode_path_warnings(params, "/params", &mut warnings);
    }
    warnings
}

fn collect_residual_xcode_path_warnings(
    value: &Value,
    source_field: &str,
    warnings: &mut Vec<XcodeShimWarningEvent>,
) {
    match value {
        Value::String(text) => {
            let clean = strip_ansi(text);
            if let Some(matched_substring) = matched_residual_xcode_path(&clean) {
                warnings.push(XcodeShimWarningEvent {
                    ts: chrono::Utc::now(),
                    policy_reason: "p051_residual_xcode_path_warning".to_string(),
                    source_field: source_field.to_string(),
                    matched_substring: matched_substring.to_string(),
                    excerpt: excerpt_around_match(&clean, matched_substring),
                });
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_residual_xcode_path_warnings(
                    item,
                    &format!("{source_field}/{index}"),
                    warnings,
                );
            }
        }
        Value::Object(map) => {
            for (key, nested) in map {
                collect_residual_xcode_path_warnings(
                    nested,
                    &format!("{source_field}/{}", escape_json_pointer_segment(key)),
                    warnings,
                );
            }
        }
        _ => {}
    }
}

fn matched_residual_xcode_path(text: &str) -> Option<&'static str> {
    RESIDUAL_XCODE_PATH_PATTERNS
        .iter()
        .copied()
        .find(|pattern| text.contains(pattern))
}

fn excerpt_around_match(text: &str, matched_substring: &str) -> String {
    const CONTEXT_CHARS: usize = 80;
    let Some(byte_index) = text.find(matched_substring) else {
        return text.chars().take(CONTEXT_CHARS * 2).collect();
    };
    let match_start_char = text[..byte_index].chars().count();
    let match_len_chars = matched_substring.chars().count();
    let start = match_start_char.saturating_sub(CONTEXT_CHARS);
    let end = match_start_char + match_len_chars + CONTEXT_CHARS;
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

fn escape_json_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
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

fn non_empty_transcript(text: String) -> Option<String> {
    (!text.trim().is_empty()).then_some(text)
}

fn truncate_string_to_byte_len(text: &mut String, max_len: usize) {
    if text.len() <= max_len {
        return;
    }
    let mut end = max_len;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
}

fn push_streamed_transcript_chunk(buffer: &mut String, chunk: &str, truncated: &mut bool) {
    if *truncated {
        return;
    }

    let sanitized = strip_ansi(chunk);
    if buffer.len().saturating_add(sanitized.len()) <= MAX_STREAMED_TRANSCRIPT_BYTES {
        buffer.push_str(&sanitized);
        return;
    }

    let max_content_len =
        MAX_STREAMED_TRANSCRIPT_BYTES.saturating_sub(STREAMED_TRANSCRIPT_TRUNCATION_MARKER.len());
    if buffer.len() > max_content_len {
        truncate_string_to_byte_len(buffer, max_content_len);
    } else {
        let remaining = max_content_len - buffer.len();
        let mut end = remaining.min(sanitized.len());
        while end > 0 && !sanitized.is_char_boundary(end) {
            end -= 1;
        }
        buffer.push_str(&sanitized[..end]);
    }
    buffer.push_str(STREAMED_TRANSCRIPT_TRUNCATION_MARKER);
    *truncated = true;
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

fn extract_output_envelopes(
    stream_text: &str,
    expected_outputs: &[ExpectedOutputSpec],
) -> Vec<DiscoveredArtifact> {
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
        let content = &stream_text[content_start..content_end];
        artifacts.push(DiscoveredArtifact {
            name: output_name.to_string(),
            content: bounded_envelope_payload_bytes(
                output_name,
                content.as_bytes(),
                expected_outputs,
            ),
            source_path: None,
            source_kind: DiscoveredArtifactSourceKind::ProviderEnvelope,
        });
        cursor = content_end + OUTPUT_END_MARKER.len();
    }

    let mut seen: HashSet<String> = artifacts
        .iter()
        .map(|artifact| artifact.name.clone())
        .collect();
    for artifact in extract_json_object_output_envelopes(stream_text, expected_outputs) {
        if seen.insert(artifact.name.clone()) {
            artifacts.push(artifact);
        }
    }

    artifacts
}

fn extract_json_object_output_envelopes(
    stream_text: &str,
    expected_outputs: &[ExpectedOutputSpec],
) -> Vec<DiscoveredArtifact> {
    let mut artifacts = Vec::new();
    let mut cursor = 0usize;
    while let Some(found_rel) = stream_text[cursor..].find("\"CHAINWORKS_OUTPUT\"") {
        let found = cursor + found_rel;
        let Some(value) = parse_enclosing_json_object_with_chainworks_output(
            stream_text,
            found,
            expected_outputs,
        ) else {
            cursor = found + "\"CHAINWORKS_OUTPUT\"".len();
            continue;
        };
        artifacts.extend(chainworks_output_artifacts_from_value(
            &value,
            expected_outputs,
        ));
        cursor = found + "\"CHAINWORKS_OUTPUT\"".len();
    }
    artifacts
}

fn parse_enclosing_json_object_with_chainworks_output(
    stream_text: &str,
    marker: usize,
    expected_outputs: &[ExpectedOutputSpec],
) -> Option<Value> {
    let parse_cap = ndjson_line_cap_bytes(expected_outputs);
    let mut starts: Vec<usize> = stream_text[..marker]
        .match_indices('{')
        .map(|(idx, _)| idx)
        .collect();
    starts.push(marker);
    starts.into_iter().rev().find_map(|start| {
        let candidate = &stream_text[start..];
        if candidate.len() > parse_cap {
            return None;
        }
        let mut deserializer = serde_json::Deserializer::from_str(candidate);
        let value = Value::deserialize(&mut deserializer).ok()?;
        value.get("CHAINWORKS_OUTPUT").is_some().then_some(value)
    })
}

fn chainworks_output_artifacts_from_value(
    value: &Value,
    expected_outputs: &[ExpectedOutputSpec],
) -> Vec<DiscoveredArtifact> {
    let Some(Value::Object(outputs)) = value.get("CHAINWORKS_OUTPUT") else {
        return Vec::new();
    };

    outputs
        .iter()
        .filter_map(|(name, payload)| {
            if name.trim().is_empty() {
                return None;
            }
            let content = match payload {
                Value::String(text) => {
                    bounded_envelope_payload_bytes(name, text.as_bytes(), expected_outputs)
                }
                other => {
                    let bytes = serde_json::to_vec(other).ok()?;
                    bounded_envelope_payload_bytes(name, &bytes, expected_outputs)
                }
            };
            Some(DiscoveredArtifact {
                name: name.clone(),
                content,
                source_path: None,
                source_kind: DiscoveredArtifactSourceKind::ChainworksOutput,
            })
        })
        .collect()
}

/// Return cap + 1 bytes on truncation so settlement can distinguish an
/// oversized declared payload without materializing the full provider output.
fn bounded_envelope_payload_bytes(
    output_name: &str,
    bytes: &[u8],
    expected_outputs: &[ExpectedOutputSpec],
) -> Vec<u8> {
    let cap = provider_envelope_cap_bytes(output_name, expected_outputs);
    if bytes.len() <= cap {
        return bytes.to_vec();
    }

    // Returning cap + 1 bytes is intentional: settlement treats len > max_bytes
    // as the truncation/oversize signal while still retaining a bounded sample.
    let truncated_len = cap.saturating_add(1).min(bytes.len());
    bytes[..truncated_len].to_vec()
}

fn provider_envelope_cap_bytes(
    output_name: &str,
    expected_outputs: &[ExpectedOutputSpec],
) -> usize {
    expected_outputs
        .iter()
        .find(|spec| spec.output_name == output_name || spec.target_path == output_name)
        .and_then(|spec| usize::try_from(spec.max_bytes).ok())
        .unwrap_or(DEFAULT_PROVIDER_ENVELOPE_MAX_BYTES)
}

fn ndjson_line_cap_bytes(expected_outputs: &[ExpectedOutputSpec]) -> usize {
    let payload_cap = expected_outputs
        .iter()
        .filter_map(|spec| usize::try_from(spec.max_bytes).ok())
        .max()
        .unwrap_or(DEFAULT_PROVIDER_ENVELOPE_MAX_BYTES)
        .max(DEFAULT_PROVIDER_ENVELOPE_MAX_BYTES);
    payload_cap.saturating_add(ACP_NDJSON_LINE_OVERHEAD_BYTES)
}

async fn read_capped_ndjson_line<R>(
    reader: &mut R,
    line: &mut String,
    max_bytes: usize,
    context: &str,
) -> Result<usize>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    line.clear();
    let mut bytes = Vec::new();

    loop {
        let buffer = reader
            .fill_buf()
            .await
            .with_context(|| format!("{context} fill_buf error"))?;
        if buffer.is_empty() {
            if bytes.is_empty() {
                return Ok(0);
            }
            break;
        }

        let take_len = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|position| position + 1)
            .unwrap_or(buffer.len());
        if bytes.len().saturating_add(take_len) > max_bytes {
            bail!("{context} exceeded bounded ACP NDJSON line cap of {max_bytes} bytes");
        }

        bytes.extend_from_slice(&buffer[..take_len]);
        reader.consume(take_len);
        if bytes.last() == Some(&b'\n') {
            break;
        }
    }

    let n = bytes.len();
    *line = String::from_utf8_lossy(&bytes).into_owned();
    Ok(n)
}

// ---------------------------------------------------------------------------
// ndjson write
// ---------------------------------------------------------------------------

pub(crate) async fn send_ndjson(stdin: &mut tokio::process::ChildStdin, msg: &Value) -> Result<()> {
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

pub(crate) async fn await_response(
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
        let n = match timeout(
            remaining,
            read_capped_ndjson_line(
                reader,
                &mut line,
                ndjson_line_cap_bytes(&[]),
                "ACP handshake read_line",
            ),
        )
        .await
        {
            Ok(Ok(n)) => n,
            Ok(Err(err)) => return Err(err).context("ACP handshake read_line error"),
            Err(_) => {
                return diagnose_late_handshake_response(reader, expected_id, start, time_limit)
                    .await;
            }
        };

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

async fn diagnose_late_handshake_response(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    expected_id: u64,
    start: Instant,
    time_limit: Duration,
) -> Result<Value> {
    let mut line = String::new();
    match timeout(
        LATE_RESPONSE_DIAGNOSTIC_WINDOW,
        read_capped_ndjson_line(
            reader,
            &mut line,
            ndjson_line_cap_bytes(&[]),
            "ACP late handshake diagnostic read_line",
        ),
    )
    .await
    {
        Ok(Ok(0)) => {
            bail!(
                "ACP handshake timed out after {}s waiting for response id={expected_id}; subprocess stdout closed during late-response diagnostic window",
                time_limit.as_secs()
            );
        }
        Ok(Ok(_)) => {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                bail!(
                    "ACP handshake timed out after {}s waiting for response id={expected_id}; only an empty late line arrived within {}s",
                    time_limit.as_secs(),
                    LATE_RESPONSE_DIAGNOSTIC_WINDOW.as_secs()
                );
            }
            let parsed: Value = match serde_json::from_str(trimmed) {
                Ok(parsed) => parsed,
                Err(error) => {
                    bail!(
                        "ACP handshake timed out after {}s waiting for response id={expected_id}; late line was not valid JSON ({error}): {:.200}",
                        time_limit.as_secs(),
                        trimmed
                    );
                }
            };
            let late_id: Option<u64> = match parsed.get("id") {
                Some(Value::Number(number)) => number.as_u64(),
                Some(Value::String(value)) => value.parse().ok(),
                _ => None,
            };
            if late_id != Some(expected_id) {
                bail!(
                    "ACP handshake timed out after {}s waiting for response id={expected_id}; late response carried id={:?} after {:.3}s",
                    time_limit.as_secs(),
                    late_id,
                    start.elapsed().as_secs_f64()
                );
            }
            if let Some(error) = parsed.get("error") {
                let message = error["message"].as_str().unwrap_or("unknown ACP error");
                bail!(
                    "ACP handshake timed out after {}s waiting for response id={expected_id}; late error response arrived after {:.3}s: {message}",
                    time_limit.as_secs(),
                    start.elapsed().as_secs_f64()
                );
            }
            return Ok(parsed
                .get("result")
                .cloned()
                .unwrap_or_else(|| Value::Object(Default::default())));
        }
        Ok(Err(error)) => {
            return Err(error).context("ACP late handshake diagnostic read_line error");
        }
        Err(_) => {
            bail!(
                "ACP handshake timed out after {}s waiting for response id={expected_id}; no late response arrived within {}s",
                time_limit.as_secs(),
                LATE_RESPONSE_DIAGNOSTIC_WINDOW.as_secs()
            );
        }
    }
}

pub async fn probe_initialize(child: Child) -> Result<Value> {
    probe_initialize_with_timeout(child, HANDSHAKE_TIMEOUT).await
}

pub(crate) async fn probe_initialize_with_timeout(
    mut child: Child,
    handshake_timeout: Duration,
) -> Result<Value> {
    let mut stdin = child
        .stdin
        .take()
        .context("ACP capability probe subprocess has no stdin pipe")?;
    let stdout = child
        .stdout
        .take()
        .context("ACP capability probe subprocess has no stdout pipe")?;
    let _ = child.stderr.take();
    let mut reader = BufReader::new(stdout);

    send_ndjson(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
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
    .context("ACP: send capability probe initialize")?;

    let result = await_response(&mut reader, 1, handshake_timeout)
        .await
        .context("ACP: capability probe initialize handshake")?;

    let _ = AsyncWriteExt::shutdown(&mut stdin).await;
    drop(stdin);
    match timeout(SHUTDOWN_WAIT, child.wait()).await {
        Ok(Ok(_)) => {}
        _ => {
            #[cfg(unix)]
            if let Some(pid) = child.id() {
                signal_process_group(pid, libc::SIGTERM);
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }

    Ok(result)
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
    acp_pre_initialize_local_latency_ms: u64,
    acp_initialize_latency_ms: u64,
    acp_session_new_latency_ms: u64,
    snapshot_root: String,
    baseline_files: Option<LegacyBroadDiscoverySnapshot>,
    discovery_filesystem: Box<dyn DiscoveryFilesystem>,
    request_counter: u64,
    closed: bool,
}

impl AcpTransportSession {
    pub async fn start(
        child: Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
    ) -> Result<Self> {
        Self::start_with_discovery_filesystem(child, req, config, Box::new(StdDiscoveryFilesystem))
            .await
    }

    pub async fn start_with_discovery_filesystem(
        mut child: Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
        discovery_filesystem: Box<dyn DiscoveryFilesystem>,
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
        let acp_pre_initialize_local_latency_ms = startup_started.elapsed().as_millis() as u64;
        info!(
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            provider = %req.provider,
            acp_pre_initialize_local_latency_ms = acp_pre_initialize_local_latency_ms,
            "P053 ACP pre-initialize local overhead measured"
        );
        let init_id = next_id!();
        let initialize_started = Instant::now();
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
        let acp_initialize_latency_ms = initialize_started.elapsed().as_millis() as u64;
        info!(
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            provider = %req.provider,
            acp_initialize_latency_ms = acp_initialize_latency_ms,
            "P053 ACP initialize latency measured"
        );

        let sn_id = next_id!();
        let sn_params =
            build_session_new_params(req, config).context("ACP: build session/new params")?;
        let session_new_started = Instant::now();
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
        let acp_session_new_latency_ms = session_new_started.elapsed().as_millis() as u64;
        info!(
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            provider = %req.provider,
            acp_session_new_latency_ms = acp_session_new_latency_ms,
            "P053 ACP session/new latency measured"
        );

        let session_id = sn_result["sessionId"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("ACP session/new response missing 'sessionId' field"))?
            .to_string();
        let mcp_observation = observe_mcp_actuals(&sn_result, req, &session_id);
        let mcp_session_startup_latency_ms = mcp_observation
            .as_ref()
            .map(|_| startup_started.elapsed().as_millis() as i64);

        if config.set_mode_after_session_new {
            let set_mode_id = next_id!();
            send_ndjson(
                &mut stdin,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": set_mode_id,
                    "method": "session/set_mode",
                    "params": {
                        "sessionId": session_id,
                        "modeId": config.mode,
                    }
                }),
            )
            .await
            .context("ACP: send session/set_mode")?;
            await_response(&mut reader, set_mode_id, HANDSHAKE_TIMEOUT)
                .await
                .context("ACP: session/set_mode handshake")?;
        }

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
            acp_pre_initialize_local_latency_ms,
            acp_initialize_latency_ms,
            acp_session_new_latency_ms,
            snapshot_root,
            baseline_files: None,
            discovery_filesystem,
            request_counter: req_counter,
            closed: false,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    pub fn mcp_observation(&self) -> Option<McpActualObservation> {
        self.mcp_observation.clone()
    }

    pub fn mcp_session_startup_latency_ms(&self) -> Option<i64> {
        self.mcp_session_startup_latency_ms
    }

    pub fn acp_pre_initialize_local_latency_ms(&self) -> u64 {
        self.acp_pre_initialize_local_latency_ms
    }

    pub fn acp_initialize_latency_ms(&self) -> u64 {
        self.acp_initialize_latency_ms
    }

    pub fn acp_session_new_latency_ms(&self) -> u64 {
        self.acp_session_new_latency_ms
    }

    pub fn is_alive(&mut self) -> bool {
        if self.closed {
            return false;
        }

        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.closed = true;
                warn!(
                    session_id = %self.session_id,
                    exit_status = ?status,
                    "ACP subprocess exited before session reuse healthcheck"
                );
                false
            }
            Ok(None) => true,
            Err(error) => {
                self.closed = true;
                warn!(
                    session_id = %self.session_id,
                    "ACP subprocess healthcheck failed: {error}"
                );
                false
            }
        }
    }

    pub async fn prompt(
        &mut self,
        req: &ExecutionRequest,
    ) -> Result<(
        AgentStatus,
        Vec<String>,
        Vec<DiscoveredArtifact>,
        Vec<PrePromptExpectedOutputMetadata>,
        Option<String>,
        Option<UsageSnapshot>,
        Vec<XcodeShimWarningEvent>,
        u64,
        u64,
        u64,
        u64,
        u64,
        bool,
        u64,
        Option<LegacyBroadDiscoverySnapshot>,
    )> {
        self.prompt_with_optional_close_signal(req, None).await
    }

    pub async fn prompt_with_close_signal(
        &mut self,
        req: &ExecutionRequest,
        close_rx: &mut watch::Receiver<bool>,
    ) -> Result<(
        AgentStatus,
        Vec<String>,
        Vec<DiscoveredArtifact>,
        Vec<PrePromptExpectedOutputMetadata>,
        Option<String>,
        Option<UsageSnapshot>,
        Vec<XcodeShimWarningEvent>,
        u64,
        u64,
        u64,
        u64,
        u64,
        bool,
        u64,
        Option<LegacyBroadDiscoverySnapshot>,
    )> {
        self.prompt_with_optional_close_signal(req, Some(close_rx))
            .await
    }

    async fn prompt_with_optional_close_signal(
        &mut self,
        req: &ExecutionRequest,
        mut close_rx: Option<&mut watch::Receiver<bool>>,
    ) -> Result<(
        AgentStatus,
        Vec<String>,
        Vec<DiscoveredArtifact>,
        Vec<PrePromptExpectedOutputMetadata>,
        Option<String>,
        Option<UsageSnapshot>,
        Vec<XcodeShimWarningEvent>,
        u64,
        u64,
        u64,
        u64,
        u64,
        bool,
        u64,
        Option<LegacyBroadDiscoverySnapshot>,
    )> {
        let typed_expected_outputs = !req.expected_outputs.is_empty();
        let expected_baseline_paths: Vec<&str> = if typed_expected_outputs {
            Vec::new()
        } else {
            req.expected_output_paths
                .iter()
                .take(200)
                .map(String::as_str)
                .collect()
        };
        let expected_path_baselines: Vec<ExpectedPathBaseline> = expected_baseline_paths
            .iter()
            .map(|path| {
                let recorder = NoopDiscoveryOperationRecorder;
                self.discovery_filesystem
                    .capture_expected_path_baseline_with_recorder(Path::new(*path), &recorder)
            })
            .collect();
        self.request_counter += 1;
        let prompt_id = self.request_counter;
        let metadata_context = pre_prompt_expected_output_context(req, &self.session_id, prompt_id);
        let pre_prompt_metadata_started = Instant::now();
        let pre_prompt_expected_outputs: Vec<PrePromptExpectedOutputMetadata> =
            capture_pre_prompt_expected_outputs(
                self.discovery_filesystem.as_ref(),
                req,
                &metadata_context,
            );
        let missing_count = pre_prompt_expected_outputs
            .iter()
            .filter(|metadata| metadata.baseline_status == ExpectedPathBaselineStatus::Absent)
            .count();
        let stale_or_digest_count = pre_prompt_expected_outputs
            .iter()
            .filter(|metadata| {
                metadata.baseline_status == ExpectedPathBaselineStatus::RegularContentCaptured
            })
            .count();
        let rejected_baseline_count = pre_prompt_expected_outputs
            .iter()
            .filter(|metadata| {
                matches!(
                    metadata.baseline_status,
                    ExpectedPathBaselineStatus::Oversized
                        | ExpectedPathBaselineStatus::Unreadable
                        | ExpectedPathBaselineStatus::NotRegularFile
                        | ExpectedPathBaselineStatus::SymlinkEscape
                        | ExpectedPathBaselineStatus::UnauthorizedRoot
                        | ExpectedPathBaselineStatus::MetadataTimeout
                        | ExpectedPathBaselineStatus::Uncertain
                )
            })
            .count();
        let pre_prompt_metadata_timeout = pre_prompt_expected_outputs.iter().any(|metadata| {
            metadata.baseline_status == ExpectedPathBaselineStatus::MetadataTimeout
        });
        let pre_prompt_metadata_digest_bytes = pre_prompt_expected_outputs
            .iter()
            .filter_map(|metadata| metadata.content_digest.as_ref().zip(metadata.size_bytes))
            .map(|(_, size_bytes)| size_bytes)
            .sum::<u64>();
        let acp_pre_prompt_metadata_latency_ms =
            pre_prompt_metadata_started.elapsed().as_millis() as u64;
        info!(
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            provider = %req.provider,
            acp_pre_prompt_metadata_latency_ms = acp_pre_prompt_metadata_latency_ms,
            acp_pre_prompt_metadata_timeout = pre_prompt_metadata_timeout,
            acp_pre_prompt_metadata_digest_bytes = pre_prompt_metadata_digest_bytes,
            acp_expected_output_spec_count = req.expected_outputs.len(),
            acp_expected_outputs_missing_count = missing_count,
            acp_expected_outputs_stale_count = stale_or_digest_count,
            acp_expected_outputs_rejected_count = rejected_baseline_count,
            "P053 pre-prompt expected-output metadata measured"
        );
        let legacy_broad_discovery_enabled =
            req.legacy_broad_discovery_policy.allows_broad_discovery();
        let broad_baseline = self.baseline_files.clone();
        let prompt_started_at = SystemTime::now();
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
        let mut last_progress = Instant::now();
        let mut streamed_text = String::new();
        let mut streamed_text_truncated = false;
        let mut latest_usage_snapshot = None;
        let mut xcode_shim_warning_events = Vec::new();
        let mut seen_xcode_warning_keys = HashSet::new();

        'streaming: loop {
            let idle = last_activity.elapsed();
            if idle >= IDLE_TIMEOUT {
                bail!(
                    "ACP session idle timeout: no message for {}s (session={})",
                    IDLE_TIMEOUT.as_secs(),
                    self.session_id
                );
            }
            let progress_idle = last_progress.elapsed();
            if progress_idle >= PROGRESS_TIMEOUT {
                bail!(
                    "ACP session progress timeout: no meaningful progress for {}s (session={})",
                    PROGRESS_TIMEOUT.as_secs(),
                    self.session_id
                );
            }
            let remaining = IDLE_TIMEOUT - idle;

            line.clear();
            let read_outcome = {
                let read_line = timeout(
                    remaining,
                    read_capped_ndjson_line(
                        &mut self.reader,
                        &mut line,
                        ndjson_line_cap_bytes(&req.expected_outputs),
                        "ACP prompt stream read_line",
                    ),
                );
                match close_rx.as_deref_mut() {
                    Some(close_rx) => {
                        if *close_rx.borrow() {
                            AcpPromptReadOutcome::CloseRequested
                        } else {
                            tokio::select! {
                                _ = close_rx.changed() => AcpPromptReadOutcome::CloseRequested,
                                result = read_line => AcpPromptReadOutcome::Read(result),
                            }
                        }
                    }
                    None => AcpPromptReadOutcome::Read(read_line.await),
                }
            };

            let n = match read_outcome {
                AcpPromptReadOutcome::CloseRequested => {
                    let session_id = self.session_id.clone();
                    let _ = self.close().await;
                    bail!("ACP session closed during active prompt (session={session_id})");
                }
                AcpPromptReadOutcome::Read(result) => result
                    .context("ACP session idle timeout — no message received")?
                    .context("ACP prompt stream read_line error")?,
            };

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
                                last_progress = Instant::now();
                            }
                        }
                        continue;
                    }
                    "session/update" => {
                        debug!(session_id = %self.session_id, "ACP: session/update notification");
                        for warning in residual_xcode_path_warnings_from_update(&parsed) {
                            let dedupe_key = format!(
                                "{}\u{1f}{}\u{1f}{}",
                                warning.source_field, warning.matched_substring, warning.excerpt
                            );
                            if seen_xcode_warning_keys.insert(dedupe_key) {
                                xcode_shim_warning_events.push(warning);
                            }
                        }
                        if let Some(chunk) = extract_text_chunk(&parsed) {
                            if !strip_ansi(&chunk).trim().is_empty() {
                                last_progress = Instant::now();
                            }
                            push_streamed_transcript_chunk(
                                &mut streamed_text,
                                &chunk,
                                &mut streamed_text_truncated,
                            );
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
                        return Ok((
                            AgentStatus::Failed,
                            vec![],
                            vec![],
                            pre_prompt_expected_outputs,
                            non_empty_transcript(streamed_text),
                            latest_usage_snapshot,
                            xcode_shim_warning_events,
                            self.acp_pre_initialize_local_latency_ms,
                            self.acp_initialize_latency_ms,
                            self.acp_session_new_latency_ms,
                            0,
                            acp_pre_prompt_metadata_latency_ms,
                            pre_prompt_metadata_timeout,
                            pre_prompt_metadata_digest_bytes,
                            None,
                        ));
                    }
                    if let Some(chunk) = extract_text_chunk(&parsed) {
                        push_streamed_transcript_chunk(
                            &mut streamed_text,
                            &chunk,
                            &mut streamed_text_truncated,
                        );
                    }
                    break 'streaming;
                }
                continue;
            }
        }

        let acp_prompt_duration_ms = SystemTime::now()
            .duration_since(prompt_started_at)
            .unwrap_or_default()
            .as_millis() as u64;
        info!(
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            provider = %req.provider,
            acp_prompt_duration_ms = acp_prompt_duration_ms,
            "P053 ACP prompt duration measured"
        );

        let post_files = legacy_broad_discovery_enabled.then(|| {
            let recorder = NoopDiscoveryOperationRecorder;
            self.discovery_filesystem
                .snapshot_legacy_broad_discovery_with_recorder(
                    Path::new(&self.snapshot_root),
                    &recorder,
                )
        });
        let mut new_files: Vec<String> = match (broad_baseline.as_ref(), post_files.as_ref()) {
            (Some(baseline), Some(post_files)) => {
                let paths: Vec<String> = post_files
                    .files
                    .difference(&baseline.files)
                    .cloned()
                    .collect();
                warn!(
                    run_id = %req.run_id,
                    stage_id = %req.stage_id,
                    root = %self.snapshot_root,
                    discovered_count = paths.len(),
                    baseline_files_visited = baseline.files_visited,
                    post_files_visited = post_files.files_visited,
                    baseline_truncated = baseline.was_truncated(),
                    post_truncated = post_files.was_truncated(),
                    baseline_total_bytes = baseline.total_bytes,
                    post_total_bytes = post_files.total_bytes,
                    "ACP legacy broad discovery is enabled for this prompt"
                );
                paths
            }
            (None, Some(post_files)) => {
                let paths: Vec<String> = post_files
                    .files
                    .iter()
                    .filter(|path| {
                        legacy_broad_file_modified_after_prompt_start(path, prompt_started_at)
                    })
                    .cloned()
                    .collect();
                warn!(
                    run_id = %req.run_id,
                    stage_id = %req.stage_id,
                    root = %self.snapshot_root,
                    discovered_count = paths.len(),
                    post_files_visited = post_files.files_visited,
                    post_truncated = post_files.was_truncated(),
                    post_total_bytes = post_files.total_bytes,
                    "ACP legacy broad discovery is enabled for this prompt without a previous post-prompt baseline"
                );
                paths
            }
            _ => Vec::new(),
        };
        if typed_expected_outputs {
            new_files.retain(|path| {
                let Some(spec) = req
                    .expected_outputs
                    .iter()
                    .find(|spec| spec.target_path == *path)
                else {
                    return true;
                };
                let Some(metadata) = pre_prompt_expected_outputs.iter().find(|metadata| {
                    metadata.output_name == spec.output_name
                        && metadata.target_path == spec.target_path
                }) else {
                    return false;
                };
                let recorder = NoopDiscoveryOperationRecorder;
                self.discovery_filesystem
                    .expected_output_has_current_content_with_recorder(
                        spec,
                        metadata,
                        &metadata_context,
                        &recorder,
                    )
            });
            for spec in &req.expected_outputs {
                if let Some(metadata) = pre_prompt_expected_outputs.iter().find(|metadata| {
                    metadata.output_name == spec.output_name
                        && metadata.target_path == spec.target_path
                }) {
                    let recorder = NoopDiscoveryOperationRecorder;
                    if self
                        .discovery_filesystem
                        .expected_output_has_current_content_with_recorder(
                            spec,
                            metadata,
                            &metadata_context,
                            &recorder,
                        )
                        && !new_files.iter().any(|p| p == &spec.target_path)
                    {
                        new_files.push(spec.target_path.clone());
                    }
                }
            }
        } else {
            new_files.retain(|path| {
                let Some(baseline) = expected_path_baselines
                    .iter()
                    .find(|baseline| baseline.target_path == *path)
                else {
                    return true;
                };
                let recorder = NoopDiscoveryOperationRecorder;
                self.discovery_filesystem
                    .expected_path_has_current_content_with_recorder(baseline, &recorder)
            });
            for baseline in &expected_path_baselines {
                let recorder = NoopDiscoveryOperationRecorder;
                if self
                    .discovery_filesystem
                    .expected_path_has_current_content_with_recorder(baseline, &recorder)
                    && !new_files.iter().any(|p| p == &baseline.target_path)
                {
                    new_files.push(baseline.target_path.clone());
                }
            }
        }
        new_files.sort();
        if let Some(post_files_snapshot) = post_files.clone() {
            self.baseline_files = Some(post_files_snapshot);
        }

        let mut discovered_artifacts =
            extract_output_envelopes(&streamed_text, &req.expected_outputs);
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
            let max_bytes = req
                .expected_outputs
                .iter()
                .find(|spec| spec.target_path == *path)
                .map(|spec| spec.max_bytes)
                .unwrap_or(DEFAULT_PROVIDER_ENVELOPE_MAX_BYTES as u64);
            if let Some(content) = read_file_with_cap(path_obj, max_bytes.saturating_add(1)) {
                discovered_artifacts.push(DiscoveredArtifact {
                    name,
                    content,
                    source_path: Some(path.clone()),
                    source_kind: DiscoveredArtifactSourceKind::ExactPath,
                });
            }
        }

        Ok((
            AgentStatus::Completed,
            new_files,
            discovered_artifacts,
            pre_prompt_expected_outputs,
            non_empty_transcript(streamed_text),
            latest_usage_snapshot,
            xcode_shim_warning_events,
            self.acp_pre_initialize_local_latency_ms,
            self.acp_initialize_latency_ms,
            self.acp_session_new_latency_ms,
            acp_prompt_duration_ms,
            acp_pre_prompt_metadata_latency_ms,
            pre_prompt_metadata_timeout,
            pre_prompt_metadata_digest_bytes,
            post_files,
        ))
    }

    pub async fn close(&mut self) -> Result<Option<AcpCloseDiagnostic>> {
        if self.closed {
            return Ok(None);
        }
        let mut close_diagnostic = None;

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
            Ok(Err(error)) => {
                merge_close_diagnostic(
                    &mut close_diagnostic,
                    Some(format!("ACP subprocess wait failed during close: {error}")),
                    transport_error_code_from_message(&error.to_string()),
                    None,
                );
                false
            }
            Err(_) => {
                debug!(
                    session_id = %self.session_id,
                    "ACP subprocess did not exit within {}s — force-killing",
                    SHUTDOWN_WAIT.as_secs()
                );
                #[cfg(unix)]
                if let Some(pid) = self.child.id() {
                    signal_process_group(pid, libc::SIGTERM);
                }
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
                merge_close_diagnostic(
                    &mut close_diagnostic,
                    Some(format!(
                        "ACP subprocess did not exit within {}s during close",
                        SHUTDOWN_WAIT.as_secs()
                    )),
                    None,
                    None,
                );
                false
            }
        };

        self.closed = true;
        if !exit_success {
            warn!(session_id = %self.session_id, "ACP subprocess exited with non-zero status");
        }
        Ok(close_diagnostic)
    }
}

fn read_file_with_cap(path: &Path, cap_bytes: u64) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = file.take(cap_bytes);
    let mut content = Vec::new();
    reader.read_to_end(&mut content).ok()?;
    Some(content)
}

fn transport_error_code_from_message(message: &str) -> Option<String> {
    let lower = message.to_ascii_lowercase();
    (lower.contains("epipe") || lower.contains("broken pipe")).then(|| "EPIPE".to_string())
}

fn merge_close_diagnostic(
    diagnostic: &mut Option<AcpCloseDiagnostic>,
    message: Option<String>,
    transport_error_code: Option<String>,
    provider_exit_status: Option<i64>,
) {
    match diagnostic {
        Some(existing) => {
            if let Some(message) = message {
                if !existing.message.is_empty() {
                    existing.message.push_str("; ");
                }
                existing.message.push_str(&message);
            }
            if existing.transport_error_code.is_none() {
                existing.transport_error_code = transport_error_code;
            }
            if existing.provider_exit_status.is_none() {
                existing.provider_exit_status = provider_exit_status;
            }
        }
        None => {
            *diagnostic = Some(AcpCloseDiagnostic {
                transport_error_code,
                provider_exit_status,
                message: message.unwrap_or_else(|| "ACP session close diagnostic".to_string()),
            });
        }
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
    let (
        status,
        paths,
        artifacts,
        _pre_prompt_expected_outputs,
        _transcript_text,
        _usage,
        _xcode_shim_warning_events,
        _acp_pre_initialize_local_latency_ms,
        _acp_initialize_latency_ms,
        _acp_session_new_latency_ms,
        _acp_prompt_duration_ms,
        _acp_pre_prompt_metadata_latency_ms,
        _acp_pre_prompt_metadata_timeout,
        _acp_pre_prompt_metadata_digest_bytes,
        _legacy_broad_discovery_snapshot,
    ) = session.prompt(req).await?;
    let _ = session.close().await;
    Ok((status, paths, artifacts))
}

impl Drop for AcpTransportSession {
    fn drop(&mut self) {
        if self.closed {
            return;
        }

        #[cfg(unix)]
        if let Some(pid) = self.child.id() {
            signal_process_group(pid, libc::SIGTERM);
        }
        if let Err(error) = self.child.start_kill() {
            warn!(
                session_id = %self.session_id,
                error = %error,
                "Failed to start-kill unclosed ACP subprocess during drop"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_uses_longer_handshake_timeout() {
        assert_eq!(
            handshake_timeout_for_provider("gemini"),
            Duration::from_secs(120)
        );
        assert_eq!(
            handshake_timeout_for_provider("Gemini"),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn non_gemini_providers_keep_default_handshake_timeout() {
        assert_eq!(
            handshake_timeout_for_provider("claude"),
            Duration::from_secs(90)
        );
        assert_eq!(
            handshake_timeout_for_provider("codex"),
            Duration::from_secs(90)
        );
    }

    #[test]
    fn stderr_epipe_is_diagnostic_warning() {
        assert!(stderr_line_is_diagnostic_warning(
            "ACP write error: Error: write EPIPE"
        ));
        assert!(stderr_line_is_diagnostic_warning("code: 'EPIPE'"));
        assert!(!stderr_line_is_diagnostic_warning("ordinary debug detail"));
    }

    #[test]
    fn proposal_053_acp_prompt_metadata_uses_discovery_filesystem_fake() {
        let output_path = "/tmp/run/proposal_review.json";
        let expected_output = ExpectedOutputSpec {
            output_name: "proposal_review".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: output_path.to_string(),
            companion_of: None,
            display_label: "Proposal review".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 1024,
            aggregate_acceptance_cap_bytes: 4096,
            authorized_roots: vec![domain::discovery::AuthorizedRoot {
                root_class: domain::discovery::OutputRootClass::ChainworksMetaRoot,
                root_path: "/tmp/run".to_string(),
            }],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        };
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: agent_execution_id.to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "prompt-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };
        let mut metadata = PrePromptExpectedOutputMetadata::absent(&expected_output, &context);
        metadata.baseline_status = ExpectedPathBaselineStatus::RegularContentCaptured;
        metadata.existed = true;
        metadata.file_type = "regular".to_string();
        metadata.size_bytes = Some(42);
        let fake = domain::discovery::FakeDiscoveryFilesystem::new()
            .with_pre_prompt_metadata("proposal_review", metadata.clone());
        let req = ExecutionRequest {
            run_id: domain::ids::RunId::new(),
            stage_execution_id: Some("stage-exec-1".to_string()),
            stage_id: "stage-1".to_string(),
            attempt_number: 1,
            agent_execution_id: Some(agent_execution_id),
            agent_id: "agent-1".to_string(),
            provider: "codex".to_string(),
            model: Some("gpt-5.5".to_string()),
            effort: Some("high".to_string()),
            workspace_root: "/tmp/run".to_string(),
            prompt: "write the output".to_string(),
            worktree_root: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: vec![expected_output],
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: Some("session-1".to_string()),
            provider_session_id: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: Some("/tmp/run".to_string()),
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
        };

        let captured = capture_pre_prompt_expected_outputs(&fake, &req, &context);

        assert_eq!(captured, vec![metadata]);
    }

    #[test]
    fn output_envelope_extraction_caps_declared_payload_before_settlement() {
        let output_path = "/tmp/run/proposal_review.json";
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "proposal_review".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: output_path.to_string(),
            companion_of: None,
            display_label: "Proposal review".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 8,
            aggregate_acceptance_cap_bytes: 64,
            authorized_roots: vec![domain::discovery::AuthorizedRoot {
                root_class: domain::discovery::OutputRootClass::ChainworksMetaRoot,
                root_path: "/tmp/run".to_string(),
            }],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let stream =
            format!("{OUTPUT_START_MARKER}proposal_review>>>1234567890abcdef{OUTPUT_END_MARKER}");

        let artifacts = extract_output_envelopes(&stream, &expected_outputs);

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "proposal_review");
        assert_eq!(artifacts[0].content, b"123456789");
        assert_eq!(
            artifacts[0].source_kind,
            DiscoveredArtifactSourceKind::ProviderEnvelope
        );
    }

    #[test]
    fn streamed_transcript_accumulation_is_capped_with_marker() {
        let mut transcript = "a".repeat(MAX_STREAMED_TRANSCRIPT_BYTES - 4);
        let mut truncated = false;

        push_streamed_transcript_chunk(&mut transcript, "bbbbbbbb", &mut truncated);

        assert!(truncated);
        assert!(transcript.len() <= MAX_STREAMED_TRANSCRIPT_BYTES);
        assert!(transcript.ends_with(STREAMED_TRANSCRIPT_TRUNCATION_MARKER));

        let len_after_truncation = transcript.len();
        push_streamed_transcript_chunk(&mut transcript, "ignored", &mut truncated);
        assert_eq!(transcript.len(), len_after_truncation);
    }

    #[test]
    fn json_chainworks_output_extraction_caps_declared_payload_before_settlement() {
        let output_path = "/tmp/run/implementation/progress.md";
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "implementation_progress".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: output_path.to_string(),
            companion_of: None,
            display_label: "Implementation progress".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 4,
            aggregate_acceptance_cap_bytes: 64,
            authorized_roots: vec![domain::discovery::AuthorizedRoot {
                root_class: domain::discovery::OutputRootClass::ChainworksMetaRoot,
                root_path: "/tmp/run".to_string(),
            }],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let stream = serde_json::json!({
            "CHAINWORKS_OUTPUT": {
                output_path: "abcdef",
            }
        })
        .to_string();

        let artifacts = extract_output_envelopes(&stream, &expected_outputs);

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, output_path);
        assert_eq!(artifacts[0].content, b"abcde");
        assert_eq!(
            artifacts[0].source_kind,
            DiscoveredArtifactSourceKind::ChainworksOutput
        );
    }
}
