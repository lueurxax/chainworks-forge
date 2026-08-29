//! ACP JSON-RPC 2.0 subprocess transport.
//!
//! Implements the Rust control-plane ACP wire protocol:
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
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::watch;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::adapters::claude::claude_sdk_debug_file_path;
use crate::{
    AcpClaudeRuntimeDiagnostics, AcpCloseDiagnostic, AcpCompletionAbsenceReason,
    AcpCompletionCaptureSource, AcpCompletionCaptureStatus, AcpCompletionTextCaptureMetadata,
    AcpMcpServerPayload, AcpPromptProgressKind, AcpPromptProgressSink, AcpPromptProgressUpdate,
    AcpRuntimeReceipt, AcpRuntimeReceiptCounters, AcpRuntimeReceiptEvent,
    AcpRuntimeReceiptHandshake, AcpRuntimeReceiptPermissionRoundtrip, DiscoveredArtifact,
    DiscoveredArtifactSourceKind, ExecutionRequest, McpActualObservation,
    NoopAcpPromptProgressSink, ResolvedMcpServerTransport, UsageSnapshot,
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
    /// Claude: request aliases may include `"opus"`, but current
    /// `claude-agent-acp` advertises Opus as the `"default"` config option.
    /// Required model selection is resolved against `session/new.configOptions`
    /// before `session/set_config_option` is sent.
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

    /// Best-effort session config options to apply after `session/new` via
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

    /// Required session config options. A send or provider rejection fails
    /// session startup instead of silently falling back to provider defaults.
    pub required_config_options: Vec<(String, String)>,

    /// Some ACP providers expose a mode catalog in `session/new` but do not
    /// apply `session/new.params.mode`. For those providers, send
    /// `session/set_mode` with `modeId` immediately after session creation.
    pub set_mode_after_session_new: bool,

    /// Best-effort debounce before auto-granting ACP permission requests.
    /// Some providers emit the request slightly before their permission
    /// registry is ready to accept the JSON-RPC response.
    pub permission_grant_debounce: Duration,
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
            required_config_options: Vec::new(),
            set_mode_after_session_new: false,
            permission_grant_debounce: Duration::ZERO,
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

#[derive(Debug)]
struct SessionConfigOptionValue {
    value: String,
    name: String,
    description: String,
}

fn resolve_session_config_option_value(
    session_new_result: &Value,
    config_id: &str,
    requested: &str,
) -> Option<String> {
    let options = session_new_result
        .get("configOptions")
        .or_else(|| session_new_result.get("config_options"))?
        .as_array()?;
    let option = options.iter().find(|option| {
        option
            .get("id")
            .or_else(|| option.get("configId"))
            .and_then(Value::as_str)
            == Some(config_id)
    })?;

    let mut values = Vec::new();
    collect_session_config_option_values(option, &mut values);
    resolve_session_config_value_from_values(&values, requested)
}

fn collect_session_config_option_values(
    option: &Value,
    values: &mut Vec<SessionConfigOptionValue>,
) {
    if let Some(value) = option.get("value").and_then(Value::as_str) {
        values.push(SessionConfigOptionValue {
            value: value.to_string(),
            name: option
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            description: option
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    if let Some(children) = option.get("options").and_then(Value::as_array) {
        for child in children {
            collect_session_config_option_values(child, values);
        }
    }
}

fn resolve_session_config_value_from_values(
    values: &[SessionConfigOptionValue],
    requested: &str,
) -> Option<String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return None;
    }
    let requested_lower = requested.to_lowercase();
    if let Some(direct) = values.iter().find(|candidate| {
        candidate.value == requested
            || candidate.value.to_lowercase() == requested_lower
            || candidate.name.to_lowercase() == requested_lower
    }) {
        return Some(direct.value.clone());
    }
    if let Some(includes) = values.iter().find(|candidate| {
        let value = candidate.value.to_lowercase();
        let name = candidate.name.to_lowercase();
        value.contains(&requested_lower)
            || name.contains(&requested_lower)
            || requested_lower.contains(&value)
    }) {
        return Some(includes.value.clone());
    }

    let tokens: Vec<String> = requested_lower
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty() && *token != "claude" && *token != "default")
        .map(ToOwned::to_owned)
        .collect();
    if tokens.is_empty() {
        return None;
    }
    values
        .iter()
        .filter_map(|candidate| {
            let haystack = format!(
                "{} {} {}",
                candidate.value, candidate.name, candidate.description
            )
            .to_lowercase();
            let score = tokens
                .iter()
                .filter(|token| haystack.contains(token.as_str()))
                .count();
            (score > 0).then_some((score, candidate))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, candidate)| candidate.value.clone())
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
/// A provider with an observable unresolved local operation gets a bounded
/// extension beyond the ordinary progress deadline. This covers Claude tool
/// work and Codex's own `wait_agent` delegation boundary.
const POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT: Duration = Duration::from_secs(900);
const PROMPT_PROGRESS_REPORT_INTERVAL: Duration = Duration::from_secs(15);
const LOCAL_ACTIVITY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const PROVIDER_PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CLAUDE_LOCAL_ACTIVITY_MAX_READ_BYTES: u64 = 1024 * 1024;
const PROVIDER_SESSION_STORE_LINE_CAP_BYTES: usize = 16 * 1024 * 1024;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
const LATE_RESPONSE_DIAGNOSTIC_WINDOW: Duration = Duration::from_secs(2);
const CLAUDE_WATCHDOG_CANCEL_DRAIN_WINDOW: Duration = Duration::from_secs(5);

enum AcpPromptReadOutcome {
    Read(Result<usize>),
    PollElapsed,
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
const COMPLETION_CAPTURE_RAW_BYTE_LIMIT: usize = 1024 * 1024;
const RUNTIME_RECEIPT_EVENT_SAMPLE_LIMIT: usize = 8;
const CLAUDE_SDK_DIAGNOSTIC_EVENT_LIMIT: usize = 64;

#[derive(Clone, Debug, Default)]
struct ClaudeLocalActivitySummary {
    event_count: u64,
    assistant_messages: u64,
    tool_uses: u64,
    tool_results: u64,
    user_tool_results: u64,
    background_tasks_started: u64,
    background_tasks_finished: u64,
    last_prompts: u64,
    type_counts: HashMap<String, u64>,
    last_event_type: Option<String>,
    last_assistant_message_id: Option<String>,
    last_assistant_stop_reason: Option<String>,
    last_assistant_incomplete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AcpSilenceDeadlineDecision {
    Continue,
    WarnGrace,
    Timeout,
}

fn local_activity_timeout_decision(
    has_observed_local_activity: bool,
    elapsed: Duration,
    warning_recorded: bool,
) -> AcpSilenceDeadlineDecision {
    if !has_observed_local_activity {
        return if elapsed >= PROGRESS_TIMEOUT {
            AcpSilenceDeadlineDecision::Timeout
        } else {
            AcpSilenceDeadlineDecision::Continue
        };
    }
    if elapsed >= POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT {
        AcpSilenceDeadlineDecision::Timeout
    } else if elapsed >= PROGRESS_TIMEOUT && !warning_recorded {
        AcpSilenceDeadlineDecision::WarnGrace
    } else {
        AcpSilenceDeadlineDecision::Continue
    }
}

fn local_activity_progress_timeout_limit(has_open_local_activity: bool) -> Duration {
    if has_open_local_activity {
        POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT
    } else {
        PROGRESS_TIMEOUT
    }
}

#[derive(Clone, Debug, Default)]
struct ClaudeLocalActivityObservation {
    should_extend_watchdog: bool,
    new_event_count: u64,
    has_open_local_activity: bool,
}

#[derive(Debug)]
struct ClaudeLocalActivityMonitor {
    transcript_path: PathBuf,
    offset: u64,
    summary: ClaudeLocalActivitySummary,
    open_tool_use_ids: HashSet<String>,
    open_background_task_ids: HashSet<String>,
}

impl ClaudeLocalActivityMonitor {
    fn for_request(req: &ExecutionRequest, session_id: &str) -> Option<Self> {
        if !req.provider.eq_ignore_ascii_case("claude") {
            return None;
        }
        let projects_root = claude_projects_root()?;
        let cwd = effective_claude_cwd(req);
        let project_key = claude_project_key(cwd);
        let transcript_path = projects_root
            .join(project_key)
            .join(format!("{session_id}.jsonl"));
        Some(Self::new_for_path(transcript_path))
    }

    fn new_for_path(transcript_path: PathBuf) -> Self {
        let offset = std::fs::metadata(&transcript_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        Self {
            transcript_path,
            offset,
            summary: ClaudeLocalActivitySummary::default(),
            open_tool_use_ids: HashSet::new(),
            open_background_task_ids: HashSet::new(),
        }
    }

    fn poll(&mut self, _now: Instant) -> Result<ClaudeLocalActivityObservation> {
        let mut new_event_count = 0;
        let metadata = match std::fs::metadata(&self.transcript_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ClaudeLocalActivityObservation {
                    should_extend_watchdog: self.has_watchdog_extending_local_activity(),
                    new_event_count: 0,
                    has_open_local_activity: self.has_open_local_activity(),
                });
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Claude local activity: stat transcript {}",
                        self.transcript_path.display()
                    )
                });
            }
        };
        let len = metadata.len();
        if len < self.offset {
            self.offset = 0;
        }
        if len > self.offset {
            let bytes_to_read = (len - self.offset).min(CLAUDE_LOCAL_ACTIVITY_MAX_READ_BYTES);
            let mut file = std::fs::File::open(&self.transcript_path).with_context(|| {
                format!(
                    "Claude local activity: open transcript {}",
                    self.transcript_path.display()
                )
            })?;
            file.seek(SeekFrom::Start(self.offset)).with_context(|| {
                format!(
                    "Claude local activity: seek transcript {}",
                    self.transcript_path.display()
                )
            })?;
            let mut reader = file.take(bytes_to_read);
            let mut chunk = String::new();
            reader.read_to_string(&mut chunk).with_context(|| {
                format!(
                    "Claude local activity: read transcript {}",
                    self.transcript_path.display()
                )
            })?;
            self.offset = self.offset.saturating_add(bytes_to_read);
            for line in chunk.lines().map(str::trim).filter(|line| !line.is_empty()) {
                if let Ok(entry) = serde_json::from_str::<Value>(line) {
                    if self.observe_entry(&entry) {
                        new_event_count += 1;
                    }
                }
            }
        }

        Ok(ClaudeLocalActivityObservation {
            should_extend_watchdog: new_event_count > 0
                || self.has_watchdog_extending_local_activity(),
            new_event_count,
            has_open_local_activity: self.has_open_local_activity(),
        })
    }

    fn observe_entry(&mut self, entry: &Value) -> bool {
        let entry_type = entry
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        *self
            .summary
            .type_counts
            .entry(entry_type.to_string())
            .or_insert(0) += 1;

        let mut relevant = false;
        match entry_type {
            "assistant" => {
                self.summary.assistant_messages += 1;
                self.summary.last_event_type = Some("assistant".to_string());
                self.summary.last_assistant_message_id = entry
                    .get("message")
                    .and_then(|message| message.get("id"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        entry
                            .get("id")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    });
                let stop_reason = entry
                    .get("message")
                    .and_then(|message| message.get("stop_reason"))
                    .or_else(|| entry.get("stop_reason"));
                if let Some(stop_reason) = stop_reason {
                    self.summary.last_assistant_stop_reason = Some(match stop_reason {
                        Value::Null => "null".to_string(),
                        Value::String(value) => value.clone(),
                        other => other.to_string(),
                    });
                    self.summary.last_assistant_incomplete = stop_reason.is_null();
                }
                relevant = true;
            }
            "last-prompt" | "last_prompt" => {
                self.summary.last_prompts += 1;
                self.summary.last_event_type = Some("last-prompt".to_string());
                relevant = true;
            }
            _ => {}
        }

        let mut tool_use_ids = Vec::new();
        collect_claude_tool_use_ids(entry, &mut tool_use_ids);
        if !tool_use_ids.is_empty() {
            self.summary.tool_uses += tool_use_ids.len() as u64;
            self.summary.last_event_type = Some("tool_use".to_string());
            relevant = true;
            for id in tool_use_ids {
                self.open_tool_use_ids.insert(id);
            }
        }

        let mut tool_result_ids = Vec::new();
        collect_claude_tool_result_ids(entry, &mut tool_result_ids);
        if !tool_result_ids.is_empty() {
            self.summary.tool_results += tool_result_ids.len() as u64;
            if entry_type == "user" {
                self.summary.user_tool_results += tool_result_ids.len() as u64;
            }
            self.summary.last_event_type = Some("tool_result".to_string());
            relevant = true;
            for id in tool_result_ids {
                self.open_tool_use_ids.remove(&id);
            }
        }

        let mut background_task_observations = Vec::new();
        collect_claude_background_task_observations(entry, &mut background_task_observations);
        if !background_task_observations.is_empty() {
            let mut by_id: HashMap<String, ClaudeBackgroundTaskObservation> = HashMap::new();
            for observation in background_task_observations {
                by_id
                    .entry(observation.id.clone())
                    .and_modify(|existing| {
                        existing.terminal = existing.terminal || observation.terminal;
                    })
                    .or_insert(observation);
            }
            relevant = true;
            self.summary.last_event_type = Some("background_task".to_string());
            for observation in by_id.into_values() {
                if observation.terminal {
                    self.summary.background_tasks_finished += 1;
                    self.open_background_task_ids.remove(&observation.id);
                } else {
                    self.summary.background_tasks_started += 1;
                    self.open_background_task_ids.insert(observation.id);
                }
            }
        }

        if relevant {
            self.summary.event_count += 1;
        }
        relevant
    }

    fn has_open_local_activity(&self) -> bool {
        self.has_open_tool_use() || self.has_open_background_task()
    }

    fn has_watchdog_extending_local_activity(&self) -> bool {
        self.has_open_tool_use()
            || (self.has_open_background_task() && !self.last_assistant_turn_completed())
    }

    fn last_assistant_turn_completed(&self) -> bool {
        self.summary
            .last_assistant_stop_reason
            .as_deref()
            .is_some_and(|stop_reason| stop_reason.eq_ignore_ascii_case("end_turn"))
    }

    fn has_open_tool_use(&self) -> bool {
        !self.open_tool_use_ids.is_empty()
    }

    fn has_open_background_task(&self) -> bool {
        !self.open_background_task_ids.is_empty()
    }

    fn open_tool_use_count(&self) -> usize {
        self.open_tool_use_ids.len()
    }

    fn open_background_task_count(&self) -> usize {
        self.open_background_task_ids.len()
    }

    fn summary(&self) -> &ClaudeLocalActivitySummary {
        &self.summary
    }

    fn summary_for_error(&self) -> String {
        format!(
            "local_event_count={}, assistant_messages={}, tool_uses={}, tool_results={}, background_tasks_started={}, background_tasks_finished={}, open_tool_uses={}, open_background_tasks={}, last_event_type={}, last_assistant_message_id={}, last_assistant_stop_reason={}, last_assistant_incomplete={}",
            self.summary.event_count,
            self.summary.assistant_messages,
            self.summary.tool_uses,
            self.summary.tool_results,
            self.summary.background_tasks_started,
            self.summary.background_tasks_finished,
            self.open_tool_use_count(),
            self.open_background_task_count(),
            self.summary.last_event_type.as_deref().unwrap_or("none"),
            self.summary
                .last_assistant_message_id
                .as_deref()
                .unwrap_or("none"),
            self.summary
                .last_assistant_stop_reason
                .as_deref()
                .unwrap_or("none"),
            self.summary.last_assistant_incomplete
        )
    }

    fn has_observed_activity(&self) -> bool {
        self.summary.event_count > 0
    }

    fn latest_final_response_text(&self) -> Option<String> {
        latest_claude_final_response_text_in_file(&self.transcript_path)
    }
}

#[derive(Clone, Debug)]
struct ClaudeBackgroundTaskObservation {
    id: String,
    terminal: bool,
}

fn claude_projects_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("CHAINWORKS_CLAUDE_PROJECTS_DIR") {
        return Some(PathBuf::from(root));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".claude").join("projects"))
}

fn effective_claude_cwd(req: &ExecutionRequest) -> &str {
    let uses_worktree_cwd = req.worktree_write_enabled
        || matches!(
            req.worktree_strategy.as_deref(),
            Some("dedicated") | Some("shared_implementation_worktree")
        );
    if uses_worktree_cwd {
        req.worktree_root.as_deref().unwrap_or(&req.workspace_root)
    } else {
        &req.workspace_root
    }
}

fn claude_project_key(cwd: &str) -> String {
    cwd.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn collect_claude_tool_use_ids(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("tool_use") {
                if let Some(id) = map.get("id").and_then(Value::as_str) {
                    ids.push(id.to_string());
                }
            }
            for nested in map.values() {
                collect_claude_tool_use_ids(nested, ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_claude_tool_use_ids(item, ids);
            }
        }
        _ => {}
    }
}

fn collect_claude_tool_result_ids(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some("tool_result") {
                if let Some(id) = map.get("tool_use_id").and_then(Value::as_str) {
                    ids.push(id.to_string());
                }
            }
            for nested in map.values() {
                collect_claude_tool_result_ids(nested, ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_claude_tool_result_ids(item, ids);
            }
        }
        _ => {}
    }
}

fn collect_claude_background_task_observations(
    value: &Value,
    observations: &mut Vec<ClaudeBackgroundTaskObservation>,
) {
    match value {
        Value::Object(map) => {
            if let Some(id) = extract_background_task_id_from_object(map) {
                observations.push(ClaudeBackgroundTaskObservation {
                    id,
                    terminal: object_has_terminal_background_task_status(map),
                });
            }
            for nested in map.values() {
                collect_claude_background_task_observations(nested, observations);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_claude_background_task_observations(item, observations);
            }
        }
        Value::String(text) => {
            if let Some(id) = background_task_start_id_from_text(text) {
                observations.push(ClaudeBackgroundTaskObservation {
                    id,
                    terminal: false,
                });
            }
            if let Some(id) = terminal_background_task_id_from_text(text) {
                observations.push(ClaudeBackgroundTaskObservation { id, terminal: true });
            }
        }
        _ => {}
    }
}

fn extract_background_task_id_from_object(map: &serde_json::Map<String, Value>) -> Option<String> {
    const KEYS: &[&str] = &[
        "backgroundTaskId",
        "background_task_id",
        "taskId",
        "task_id",
    ];
    KEYS.iter().find_map(|key| {
        map.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn object_has_terminal_background_task_status(map: &serde_json::Map<String, Value>) -> bool {
    const STATUS_KEYS: &[&str] = &["status", "state", "outcome", "result"];
    STATUS_KEYS.iter().any(|key| {
        map.get(*key)
            .and_then(Value::as_str)
            .map(background_task_status_is_terminal)
            .unwrap_or(false)
    })
}

fn background_task_status_is_terminal(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "completed"
            | "complete"
            | "done"
            | "success"
            | "succeeded"
            | "failed"
            | "failure"
            | "error"
            | "errored"
            | "killed"
            | "cancelled"
            | "canceled"
            | "stopped"
            | "terminated"
    )
}

fn background_task_start_id_from_text(text: &str) -> Option<String> {
    const PREFIX: &str = "Command running in background with ID:";
    let (_, rest) = text.split_once(PREFIX)?;
    let id = rest
        .trim_start()
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .next()
        .unwrap_or_default()
        .trim();
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

fn terminal_background_task_id_from_text(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let terminal_word = [
        " completed",
        " failed",
        " killed",
        " cancelled",
        " canceled",
        " stopped",
        " terminated",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if !terminal_word
        || !(lower.contains("taskoutput")
            || lower.contains("task output")
            || lower.contains("background task"))
    {
        return None;
    }
    task_id_token_from_text(text)
}

fn task_id_token_from_text(text: &str) -> Option<String> {
    const IGNORED: &[&str] = &[
        "taskoutput",
        "task",
        "output",
        "background",
        "with",
        "id",
        "completed",
        "complete",
        "failed",
        "failure",
        "killed",
        "cancelled",
        "canceled",
        "stopped",
        "terminated",
    ];
    text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'))
        .map(str::trim)
        .filter(|token| token.len() >= 3)
        .find(|token| {
            let lower = token.to_ascii_lowercase();
            !IGNORED.contains(&lower.as_str())
        })
        .map(ToOwned::to_owned)
}

#[derive(Clone, Debug, Default)]
struct CodexLocalActivitySummary {
    event_count: u64,
    function_calls: u64,
    function_outputs: u64,
    running_processes_started: u64,
    running_process_outputs: u64,
    running_processes_finished: u64,
    background_agent_waits_started: u64,
    background_agent_waits_finished: u64,
    turn_aborted: bool,
    turn_completed: bool,
    stdin_closed_control_failures: u64,
    unbounded_tool_outputs: u64,
    max_original_token_count: Option<u64>,
    max_total_output_lines: Option<u64>,
    turn_aborted_after_open_process: bool,
    last_pathology: Option<String>,
    last_event_type: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct CodexLocalActivityObservation {
    should_extend_watchdog: bool,
    new_event_count: u64,
    has_open_local_activity: bool,
}

#[derive(Debug)]
struct CodexLocalActivityMonitor {
    session_id: Option<String>,
    candidate_roots: Vec<PathBuf>,
    session_path: Option<PathBuf>,
    offset: u64,
    summary: CodexLocalActivitySummary,
    active_call_process_ids: HashMap<String, String>,
    open_process_ids: HashSet<String>,
    open_background_agent_wait_call_ids: HashSet<String>,
}

impl CodexLocalActivityMonitor {
    fn for_request(req: &ExecutionRequest, session_id: &str) -> Option<Self> {
        if !req.provider.eq_ignore_ascii_case("codex") {
            return None;
        }
        let mut candidate_roots = Vec::new();
        if let Some(runtime_home) = req.provider_runtime_home.as_deref() {
            push_codex_session_store_root_candidate(&mut candidate_roots, Path::new(runtime_home));
        }
        push_codex_runtime_root_candidate(&mut candidate_roots, Path::new(&req.workspace_root));
        if let Some(worktree_root) = req.worktree_root.as_deref() {
            push_codex_runtime_root_candidate(&mut candidate_roots, Path::new(worktree_root));
        }
        if candidate_roots.is_empty() {
            return None;
        }
        Some(Self {
            session_id: Some(session_id.to_string()),
            candidate_roots,
            session_path: None,
            offset: 0,
            summary: CodexLocalActivitySummary::default(),
            active_call_process_ids: HashMap::new(),
            open_process_ids: HashSet::new(),
            open_background_agent_wait_call_ids: HashSet::new(),
        })
    }

    #[cfg(test)]
    fn new_for_path(session_path: PathBuf) -> Self {
        let offset = std::fs::metadata(&session_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        Self {
            session_id: None,
            candidate_roots: Vec::new(),
            session_path: Some(session_path),
            offset,
            summary: CodexLocalActivitySummary::default(),
            active_call_process_ids: HashMap::new(),
            open_process_ids: HashSet::new(),
            open_background_agent_wait_call_ids: HashSet::new(),
        }
    }

    fn poll(&mut self, _now: Instant) -> Result<CodexLocalActivityObservation> {
        self.ensure_session_path();
        let Some(session_path) = self.session_path.as_ref() else {
            return Ok(CodexLocalActivityObservation {
                should_extend_watchdog: self.has_open_local_activity(),
                new_event_count: 0,
                has_open_local_activity: self.has_open_local_activity(),
            });
        };

        let mut new_event_count = 0;
        let metadata = match std::fs::metadata(session_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CodexLocalActivityObservation {
                    should_extend_watchdog: self.has_open_local_activity(),
                    new_event_count: 0,
                    has_open_local_activity: self.has_open_local_activity(),
                });
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Codex local activity: stat session {}",
                        session_path.display()
                    )
                });
            }
        };
        let len = metadata.len();
        if len < self.offset {
            self.offset = 0;
        }
        if len > self.offset {
            let bytes_to_read = (len - self.offset).min(CLAUDE_LOCAL_ACTIVITY_MAX_READ_BYTES);
            let mut file = std::fs::File::open(session_path).with_context(|| {
                format!(
                    "Codex local activity: open session {}",
                    session_path.display()
                )
            })?;
            file.seek(SeekFrom::Start(self.offset)).with_context(|| {
                format!(
                    "Codex local activity: seek session {}",
                    session_path.display()
                )
            })?;
            let mut reader = file.take(bytes_to_read);
            let mut chunk = String::new();
            reader.read_to_string(&mut chunk).with_context(|| {
                format!(
                    "Codex local activity: read session {}",
                    session_path.display()
                )
            })?;
            self.offset = self.offset.saturating_add(bytes_to_read);
            for line in chunk.lines().map(str::trim).filter(|line| !line.is_empty()) {
                if let Ok(entry) = serde_json::from_str::<Value>(line) {
                    if self.observe_entry(&entry) {
                        new_event_count += 1;
                    }
                }
            }
        }

        Ok(CodexLocalActivityObservation {
            should_extend_watchdog: new_event_count > 0 || self.has_open_local_activity(),
            new_event_count,
            has_open_local_activity: self.has_open_local_activity(),
        })
    }

    fn ensure_session_path(&mut self) {
        if self.session_path.is_none() {
            self.session_path = self
                .session_id
                .as_deref()
                .and_then(|session_id| find_codex_session_store(&self.candidate_roots, session_id));
        }
    }

    fn quota_failure_event_from_session_store(&mut self) -> Option<ProviderFailureEvent> {
        self.ensure_session_path();
        self.session_path
            .as_deref()
            .and_then(codex_session_store_credits_exhausted_failure)
    }

    fn provider_failure_event_from_local_activity(&self) -> Option<ProviderFailureEvent> {
        let pathology = self.summary.last_pathology.as_deref()?;
        let detail = format!(
            "{pathology}; {}; max_original_token_count={}; max_total_output_lines={}",
            self.summary_for_error(),
            self.summary
                .max_original_token_count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.summary
                .max_total_output_lines
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        );
        Some(ProviderFailureEvent {
            failure_phase: "codex_tool_session_control_failure",
            message: format!(
                "Codex tool/session control failure: {pathology}; {}",
                self.summary_for_error()
            ),
            detail,
        })
    }

    fn observe_entry(&mut self, entry: &Value) -> bool {
        let entry_type = entry
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if entry_type == "event_msg" {
            match entry.pointer("/payload/type").and_then(Value::as_str) {
                Some("turn_aborted") => {
                    self.summary.event_count += 1;
                    self.summary.turn_aborted = true;
                    if !self.open_process_ids.is_empty() {
                        self.summary.turn_aborted_after_open_process = true;
                        self.summary.last_pathology =
                            Some("codex_turn_aborted_after_open_process".to_string());
                    }
                    self.summary.last_event_type = Some("turn_aborted".to_string());
                    self.active_call_process_ids.clear();
                    self.open_process_ids.clear();
                    self.open_background_agent_wait_call_ids.clear();
                    return true;
                }
                Some("task_complete") => {
                    self.summary.event_count += 1;
                    self.summary.turn_completed = true;
                    self.summary.last_event_type = Some("task_complete".to_string());
                    if !self.open_process_ids.is_empty() {
                        self.summary.running_processes_finished +=
                            self.open_process_ids.len() as u64;
                    }
                    self.active_call_process_ids.clear();
                    self.open_process_ids.clear();
                    self.open_background_agent_wait_call_ids.clear();
                    return true;
                }
                _ => {}
            }
        }

        if entry_type != "response_item" {
            return false;
        }
        let Some(payload) = entry.get("payload") else {
            return false;
        };
        match payload.get("type").and_then(Value::as_str) {
            Some("function_call") => self.observe_function_call(payload),
            Some("function_call_output") => self.observe_function_call_output(payload),
            _ => false,
        }
    }

    fn observe_function_call(&mut self, payload: &Value) -> bool {
        let Some(name) = payload.get("name").and_then(Value::as_str) else {
            return false;
        };
        let Some(call_id) = payload.get("call_id").and_then(Value::as_str) else {
            return false;
        };
        if !matches!(name, "exec_command" | "write_stdin" | "wait_agent") {
            return false;
        }
        self.summary.event_count += 1;
        self.summary.function_calls += 1;
        self.summary.last_event_type = Some(name.to_string());
        if name == "write_stdin" {
            if let Some(process_id) = codex_function_call_session_id(payload) {
                self.active_call_process_ids
                    .insert(call_id.to_string(), process_id.clone());
                self.open_process_ids.insert(process_id);
            }
        }
        if name == "wait_agent"
            && self
                .open_background_agent_wait_call_ids
                .insert(call_id.to_string())
        {
            self.summary.background_agent_waits_started += 1;
        }
        true
    }

    fn observe_function_call_output(&mut self, payload: &Value) -> bool {
        let Some(call_id) = payload.get("call_id").and_then(Value::as_str) else {
            return false;
        };
        let Some(output) = payload.get("output").and_then(Value::as_str) else {
            return false;
        };
        self.summary.event_count += 1;
        self.summary.function_outputs += 1;
        self.summary.last_event_type = Some("function_call_output".to_string());
        self.observe_function_output_pathology(output);
        if output.contains("aborted by user") {
            self.summary.turn_aborted = true;
            self.active_call_process_ids.clear();
            self.open_process_ids.clear();
            self.open_background_agent_wait_call_ids.clear();
            return true;
        }
        if self.open_background_agent_wait_call_ids.remove(call_id) {
            self.summary.background_agent_waits_finished += 1;
            return true;
        }
        if let Some(process_id) = codex_running_process_id_from_output(output) {
            self.active_call_process_ids
                .insert(call_id.to_string(), process_id.clone());
            if self.open_process_ids.insert(process_id) {
                self.summary.running_processes_started += 1;
            }
            self.summary.running_process_outputs += 1;
            return true;
        }
        if codex_output_is_terminal_process_result(output) {
            if let Some(process_id) = self.active_call_process_ids.remove(call_id) {
                if self.open_process_ids.remove(&process_id) {
                    self.summary.running_processes_finished += 1;
                }
            } else if !self.open_process_ids.is_empty() {
                self.summary.running_processes_finished += self.open_process_ids.len() as u64;
                self.open_process_ids.clear();
            }
            return true;
        }
        true
    }

    fn observe_function_output_pathology(&mut self, output: &str) {
        if output.contains("write_stdin failed: stdin is closed") {
            self.summary.stdin_closed_control_failures += 1;
            self.summary.last_pathology = Some("codex_tool_stdin_closed".to_string());
        }
        if let Some(count) = codex_metric_from_output(output, "Original token count:") {
            self.summary.max_original_token_count = Some(
                self.summary
                    .max_original_token_count
                    .map(|current| current.max(count))
                    .unwrap_or(count),
            );
            if count > CODEX_UNBOUNDED_TOOL_OUTPUT_TOKEN_CAP {
                self.summary.unbounded_tool_outputs += 1;
                self.summary.last_pathology = Some("codex_unbounded_tool_output".to_string());
            }
        }
        if let Some(count) = codex_metric_from_output(output, "Total output lines:") {
            self.summary.max_total_output_lines = Some(
                self.summary
                    .max_total_output_lines
                    .map(|current| current.max(count))
                    .unwrap_or(count),
            );
            if count > CODEX_UNBOUNDED_TOOL_OUTPUT_LINE_CAP {
                self.summary.unbounded_tool_outputs += 1;
                self.summary.last_pathology = Some("codex_unbounded_tool_output".to_string());
            }
        }
    }

    fn has_open_local_activity(&self) -> bool {
        !self.open_process_ids.is_empty() || !self.open_background_agent_wait_call_ids.is_empty()
    }

    fn open_process_count(&self) -> usize {
        self.open_process_ids.len()
    }

    fn summary(&self) -> &CodexLocalActivitySummary {
        &self.summary
    }

    fn summary_for_error(&self) -> String {
        format!(
            "local_event_count={}, function_calls={}, function_outputs={}, running_processes_started={}, running_process_outputs={}, running_processes_finished={}, background_agent_waits_started={}, background_agent_waits_finished={}, open_processes={}, open_background_agent_waits={}, turn_aborted={}, turn_completed={}, stdin_closed_control_failures={}, unbounded_tool_outputs={}, max_original_token_count={}, max_total_output_lines={}, turn_aborted_after_open_process={}, last_pathology={}, last_event_type={}",
            self.summary.event_count,
            self.summary.function_calls,
            self.summary.function_outputs,
            self.summary.running_processes_started,
            self.summary.running_process_outputs,
            self.summary.running_processes_finished,
            self.summary.background_agent_waits_started,
            self.summary.background_agent_waits_finished,
            self.open_process_count(),
            self.open_background_agent_wait_call_ids.len(),
            self.summary.turn_aborted,
            self.summary.turn_completed,
            self.summary.stdin_closed_control_failures,
            self.summary.unbounded_tool_outputs,
            self.summary
                .max_original_token_count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.summary
                .max_total_output_lines
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string()),
            self.summary.turn_aborted_after_open_process,
            self.summary.last_pathology.as_deref().unwrap_or("none"),
            self.summary.last_event_type.as_deref().unwrap_or("none"),
        )
    }

    fn has_observed_activity(&self) -> bool {
        self.summary.event_count > 0
    }
}

fn push_codex_runtime_root_candidate(candidates: &mut Vec<PathBuf>, base: &Path) {
    if base.as_os_str().is_empty() {
        return;
    }
    let candidate = base.join(".forge-codex-acp");
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn push_codex_session_store_root_candidate(candidates: &mut Vec<PathBuf>, candidate: &Path) {
    if candidate.as_os_str().is_empty() {
        return;
    }
    let candidate = candidate.to_path_buf();
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn find_codex_session_store(candidate_roots: &[PathBuf], session_id: &str) -> Option<PathBuf> {
    const MAX_DIRS: usize = 512;
    const MAX_FILES: usize = 2048;
    for root in candidate_roots {
        if !root.exists() {
            continue;
        }
        let mut stack = vec![root.clone()];
        let mut visited_dirs = 0usize;
        let mut visited_files = 0usize;
        while let Some(dir) = stack.pop() {
            visited_dirs += 1;
            if visited_dirs > MAX_DIRS || visited_files > MAX_FILES {
                break;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if file_type.is_dir() {
                    stack.push(path);
                } else if file_type.is_file() {
                    visited_files += 1;
                    let file_name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    if file_name.ends_with(".jsonl") && file_name.contains(session_id) {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn codex_session_store_credits_exhausted_failure(path: &Path) -> Option<ProviderFailureEvent> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    for line in reader.lines().map_while(Result::ok) {
        if line.len() > PROVIDER_SESSION_STORE_LINE_CAP_BYTES {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let credits = value.pointer("/payload/rate_limits/credits")?;
        if credits
            .get("has_credits")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            continue;
        }
        let limit_id = value
            .pointer("/payload/rate_limits/limit_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let balance = credits
            .get("balance")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let unlimited = credits
            .get("unlimited")
            .and_then(Value::as_bool)
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let detail = format!(
            "codex_credits_exhausted;limit_id={limit_id};balance={balance};unlimited={unlimited}"
        );
        return Some(ProviderFailureEvent {
            failure_phase: "provider_quota",
            message: format!(
                "Codex credits exhausted: limit_id={limit_id}, balance={balance}, unlimited={unlimited}"
            ),
            detail,
        });
    }
    None
}

fn codex_function_call_session_id(payload: &Value) -> Option<String> {
    let arguments = payload.get("arguments").and_then(Value::as_str)?;
    let parsed = serde_json::from_str::<Value>(arguments).ok()?;
    let session_id = parsed.get("session_id")?;
    match session_id {
        Value::Number(number) => Some(number.to_string()),
        Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_string()),
        _ => None,
    }
}

const CODEX_UNBOUNDED_TOOL_OUTPUT_TOKEN_CAP: u64 = 250_000;
const CODEX_UNBOUNDED_TOOL_OUTPUT_LINE_CAP: u64 = 2_000;

fn codex_metric_from_output(output: &str, marker: &str) -> Option<u64> {
    let (_, after_marker) = output.split_once(marker)?;
    let digits = after_marker
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

fn codex_running_process_id_from_output(output: &str) -> Option<String> {
    let marker = "Process running with session ID ";
    let (_, after_marker) = output.split_once(marker)?;
    let id = after_marker
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect::<String>();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

fn codex_output_is_terminal_process_result(output: &str) -> bool {
    let lowered = output.to_ascii_lowercase();
    lowered.contains("process exited with code")
        || lowered.contains("process completed")
        || lowered.contains("process failed")
        || lowered.contains("command failed")
}

fn latest_claude_final_response_text_in_file(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let mut latest = None;
    for line in reader.lines().map_while(Result::ok) {
        if line.len() > PROVIDER_SESSION_STORE_LINE_CAP_BYTES || !line.contains("CHAINWORKS_OUTPUT")
        {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(text) = claude_session_line_assistant_final_text(&value) {
            latest = Some(text);
        }
    }
    latest
}

fn claude_session_line_assistant_final_text(value: &Value) -> Option<String> {
    if value.get("type").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let message = value.get("message");
    let stop_reason = message
        .and_then(|message| message.get("stop_reason"))
        .or_else(|| value.get("stop_reason"));
    if !stop_reason
        .and_then(Value::as_str)
        .is_some_and(|reason| reason.eq_ignore_ascii_case("end_turn"))
    {
        return None;
    }
    let content = message
        .and_then(|message| message.get("content"))
        .or_else(|| value.get("content"))
        .and_then(Value::as_array)?;
    for block in content {
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
    }
    None
}

fn max_instant_option(base: Instant, candidate: Option<Instant>) -> Instant {
    candidate.filter(|instant| *instant > base).unwrap_or(base)
}

pub(crate) fn handshake_timeout_for_provider(provider: &str) -> Duration {
    let provider = provider.strip_suffix("_acp").unwrap_or(provider);
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

#[cfg(test)]
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
            for key in ["text", "content", "message", "delta", "parts", "output"] {
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

fn extract_agent_message_chunk(parsed: &Value) -> Option<String> {
    if parsed.get("method").and_then(Value::as_str) != Some("session/update") {
        return None;
    }
    let update = parsed.pointer("/params/update")?;
    let Some(session_update) = update.get("sessionUpdate").and_then(Value::as_str) else {
        return None;
    };
    if !matches!(session_update, "agent_message_chunk" | "text_chunk") {
        return None;
    }
    update
        .get("content")
        .or_else(|| update.pointer("/message/content"))
        .and_then(extract_text_from_value)
        .filter(|text| !strip_ansi(text).trim().is_empty())
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

fn transcript_with_prompt_error(mut streamed_text: String, err_msg: &str) -> Option<String> {
    let diagnostic = format!("ACP session/prompt error: {err_msg}");
    if streamed_text.trim().is_empty() {
        Some(diagnostic)
    } else {
        streamed_text.push_str("\n\n");
        streamed_text.push_str(&diagnostic);
        Some(streamed_text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ProviderFailureEvent {
    failure_phase: &'static str,
    message: String,
    detail: String,
}

fn classify_provider_failure_event(parsed: &Value, provider: &str) -> Option<ProviderFailureEvent> {
    if !provider.eq_ignore_ascii_case("junie") {
        return None;
    }
    find_junie_provider_failure_event(parsed)
}

fn classify_prompt_error_response(
    provider: &str,
    jsonrpc_error_code: Option<i64>,
    err_msg: &str,
) -> ProviderFailureEvent {
    if is_provider_quota_or_capacity_failure(provider, err_msg) {
        return ProviderFailureEvent {
            failure_phase: "provider_quota",
            message: format!("provider quota/capacity failure: {provider}: {err_msg}"),
            detail: format!(
                "jsonrpc_error_code={};message={}",
                jsonrpc_error_code
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                truncate_runtime_receipt_detail(err_msg)
            ),
        };
    }
    ProviderFailureEvent {
        failure_phase: "prompt_error_response",
        message: format!("ACP session/prompt returned error: {err_msg}"),
        detail: format!(
            "jsonrpc_error_code={};message={}",
            jsonrpc_error_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            truncate_runtime_receipt_detail(err_msg)
        ),
    }
}

fn is_provider_quota_or_capacity_failure(provider: &str, message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    let provider = provider.to_ascii_lowercase();
    lower.contains("quota")
        || lower.contains("rate limit")
        || lower.contains("hit your limit")
        || lower.contains("session limit")
        || lower.contains("exhausted your capacity")
        || lower.contains("capacity on this model")
        || (provider == "gemini" && lower.contains("capacity"))
}

fn find_junie_provider_failure_event(value: &Value) -> Option<ProviderFailureEvent> {
    match value {
        Value::Object(map) => {
            if map.get("kind").and_then(Value::as_str) == Some("AgentFailureEvent") {
                let error_code = map
                    .get("errorCode")
                    .or_else(|| map.get("error_code"))
                    .and_then(Value::as_str);
                let provider_message = map.get("message").and_then(Value::as_str).unwrap_or("");
                if is_junie_quota_failure(error_code, provider_message) {
                    let detail = format!(
                        "kind=AgentFailureEvent;errorCode={};message={}",
                        error_code.unwrap_or("unknown"),
                        provider_message
                    );
                    return Some(ProviderFailureEvent {
                        failure_phase: "provider_quota",
                        message: format!(
                            "provider quota/capacity failure: Junie AgentFailureEvent errorCode={}; message={}",
                            error_code.unwrap_or("unknown"),
                            provider_message
                        ),
                        detail,
                    });
                }
            }
            map.values().find_map(find_junie_provider_failure_event)
        }
        Value::Array(items) => items.iter().find_map(find_junie_provider_failure_event),
        _ => None,
    }
}

fn is_junie_quota_failure(error_code: Option<&str>, provider_message: &str) -> bool {
    if matches!(error_code, Some("ExitPaymentRequired")) {
        return true;
    }
    let lower = provider_message.to_ascii_lowercase();
    lower.contains("insufficient account balance")
        || (lower.contains("tokens") && lower.contains("balance") && lower.contains("spent"))
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

#[derive(Clone, Debug)]
struct CompletionTextCapture {
    terminal_final_response_seen: bool,
    terminal_final_response: Option<CapturedCompletionText>,
    provider_session_store_final_response: Option<CapturedCompletionText>,
    streamed_update_tail: Option<CapturedCompletionText>,
}

#[derive(Clone, Debug)]
struct CapturedCompletionText {
    text: String,
    truncated: bool,
}

#[derive(Clone, Debug)]
struct SelectedCompletionTextCapture {
    text: Option<String>,
    metadata: AcpCompletionTextCaptureMetadata,
}

impl Default for CompletionTextCapture {
    fn default() -> Self {
        Self {
            terminal_final_response_seen: false,
            terminal_final_response: None,
            provider_session_store_final_response: None,
            streamed_update_tail: None,
        }
    }
}

impl CompletionTextCapture {
    fn push_streamed_update(&mut self, chunk: &str) {
        let sanitized = strip_ansi(chunk);
        if sanitized.trim().is_empty() {
            return;
        }
        push_completion_tail_chunk(&mut self.streamed_update_tail, &sanitized);
    }

    fn set_terminal_final_response(&mut self, text: &str) {
        let sanitized = strip_ansi(text);
        self.terminal_final_response_seen = true;
        self.terminal_final_response = bounded_completion_text(sanitized);
    }

    fn set_provider_session_store_final_response(&mut self, text: &str) {
        let sanitized = strip_ansi(text);
        self.provider_session_store_final_response = bounded_completion_text(sanitized);
    }

    #[cfg(test)]
    fn select_extraction_input(&self) -> SelectedCompletionTextCapture {
        self.select_extraction_input_with_capped_stream(None, false)
    }

    fn select_extraction_input_with_capped_stream(
        &self,
        capped_stream: Option<&str>,
        capped_stream_truncated: bool,
    ) -> SelectedCompletionTextCapture {
        if let Some(capture) = non_empty_capture(self.terminal_final_response.as_ref()) {
            return selected_completion_text(
                capture,
                AcpCompletionCaptureSource::TerminalFinalResponse,
            );
        }

        if let Some(capture) =
            non_empty_capture(self.provider_session_store_final_response.as_ref())
        {
            return selected_completion_text(
                capture,
                AcpCompletionCaptureSource::ProviderSessionStoreFinalResponse,
            );
        }

        if capped_stream_truncated {
            if let Some(capture) = non_empty_capture(self.streamed_update_tail.as_ref()) {
                return selected_completion_text(
                    capture,
                    AcpCompletionCaptureSource::StreamedUpdateTail,
                );
            }
        } else if let Some(stream) = capped_stream.filter(|stream| !stream.trim().is_empty()) {
            let capture = CapturedCompletionText {
                text: stream.to_string(),
                truncated: false,
            };
            return selected_completion_text(&capture, AcpCompletionCaptureSource::CappedStream);
        } else if let Some(capture) = non_empty_capture(self.streamed_update_tail.as_ref()) {
            return selected_completion_text(
                capture,
                AcpCompletionCaptureSource::StreamedUpdateTail,
            );
        }

        let absence_reason = if self.terminal_final_response_seen {
            AcpCompletionAbsenceReason::TerminalResponseWithoutText
        } else {
            AcpCompletionAbsenceReason::NoTerminalOrStreamText
        };

        SelectedCompletionTextCapture {
            text: None,
            metadata: AcpCompletionTextCaptureMetadata {
                capture_status: AcpCompletionCaptureStatus::Absent,
                capture_source: None,
                captured_text: None,
                raw_byte_limit: COMPLETION_CAPTURE_RAW_BYTE_LIMIT as u64,
                captured_byte_count: 0,
                completion_text_truncated: false,
                extraction_input_truncated: false,
                extraction_input_sha256: None,
                absence_reason: Some(absence_reason),
            },
        }
    }
}

fn non_empty_capture(capture: Option<&CapturedCompletionText>) -> Option<&CapturedCompletionText> {
    capture.filter(|capture| !capture.text.trim().is_empty())
}

fn bounded_completion_text(text: String) -> Option<CapturedCompletionText> {
    if text.trim().is_empty() {
        return None;
    }
    let mut text = text;
    let truncated = text.len() > COMPLETION_CAPTURE_RAW_BYTE_LIMIT;
    if truncated {
        truncate_string_to_byte_len(&mut text, COMPLETION_CAPTURE_RAW_BYTE_LIMIT);
    }
    Some(CapturedCompletionText { text, truncated })
}

fn push_completion_tail_chunk(capture: &mut Option<CapturedCompletionText>, chunk: &str) {
    let capture = capture.get_or_insert_with(|| CapturedCompletionText {
        text: String::new(),
        truncated: false,
    });
    capture.text.push_str(chunk);
    if capture.text.len() > COMPLETION_CAPTURE_RAW_BYTE_LIMIT {
        capture.truncated = true;
        let remove_len = capture.text.len() - COMPLETION_CAPTURE_RAW_BYTE_LIMIT;
        let mut start = remove_len;
        while start < capture.text.len() && !capture.text.is_char_boundary(start) {
            start += 1;
        }
        capture.text.drain(..start);
    }
}

fn selected_completion_text(
    capture: &CapturedCompletionText,
    source: AcpCompletionCaptureSource,
) -> SelectedCompletionTextCapture {
    SelectedCompletionTextCapture {
        text: Some(capture.text.clone()),
        metadata: AcpCompletionTextCaptureMetadata {
            capture_status: AcpCompletionCaptureStatus::Captured,
            capture_source: Some(source),
            captured_text: Some(capture.text.clone()),
            raw_byte_limit: COMPLETION_CAPTURE_RAW_BYTE_LIMIT as u64,
            captured_byte_count: capture.text.len() as u64,
            completion_text_truncated: capture.truncated,
            extraction_input_truncated: capture.truncated,
            extraction_input_sha256: Some(sha256_hex(capture.text.as_bytes())),
            absence_reason: None,
        },
    }
}

pub(crate) fn recovered_completion_text_capture_metadata(
    text: &str,
    source: AcpCompletionCaptureSource,
) -> AcpCompletionTextCaptureMetadata {
    let sanitized = strip_ansi(text);
    let Some(capture) = bounded_completion_text(sanitized) else {
        return AcpCompletionTextCaptureMetadata {
            capture_status: AcpCompletionCaptureStatus::Absent,
            capture_source: None,
            captured_text: None,
            raw_byte_limit: COMPLETION_CAPTURE_RAW_BYTE_LIMIT as u64,
            captured_byte_count: 0,
            completion_text_truncated: false,
            extraction_input_truncated: false,
            extraction_input_sha256: None,
            absence_reason: Some(AcpCompletionAbsenceReason::NoTerminalOrStreamText),
        };
    };
    selected_completion_text(&capture, source).metadata
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

pub fn extract_output_envelopes(
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
    for artifact in extract_labeled_expected_output_json_blocks(stream_text, expected_outputs) {
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
    append_json_value_output_envelopes_from_text(stream_text, expected_outputs, &mut artifacts, 0);
    let mut seen: HashSet<String> = artifacts
        .iter()
        .map(|artifact| artifact.name.clone())
        .collect();
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
        for artifact in chainworks_output_artifacts_from_value(&value, expected_outputs) {
            if seen.insert(artifact.name.clone()) {
                artifacts.push(artifact);
            }
        }
        cursor = found + "\"CHAINWORKS_OUTPUT\"".len();
    }
    artifacts
}

fn append_json_value_output_envelopes_from_text(
    text: &str,
    expected_outputs: &[ExpectedOutputSpec],
    artifacts: &mut Vec<DiscoveredArtifact>,
    depth: usize,
) {
    if depth > 2 || !text.contains("CHAINWORKS_OUTPUT") {
        return;
    }
    let Ok(value) = serde_json::from_str::<Value>(text.trim()) else {
        return;
    };
    match value {
        Value::String(inner) => {
            append_json_value_output_envelopes_from_text(
                &inner,
                expected_outputs,
                artifacts,
                depth + 1,
            );
        }
        other => {
            artifacts.extend(chainworks_output_artifacts_from_value(
                &other,
                expected_outputs,
            ));
        }
    }
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

fn extract_labeled_expected_output_json_blocks(
    stream_text: &str,
    expected_outputs: &[ExpectedOutputSpec],
) -> Vec<DiscoveredArtifact> {
    let ascii_lower = stream_text.to_ascii_lowercase();
    expected_outputs
        .iter()
        .filter_map(|spec| {
            let labels = labeled_output_candidates(spec);
            let payload = labels
                .iter()
                .find_map(|label| labeled_json_payload_after(stream_text, &ascii_lower, label))?;
            let bytes = bounded_envelope_payload_bytes(
                &spec.output_name,
                payload.as_bytes(),
                expected_outputs,
            );
            Some(DiscoveredArtifact {
                name: spec.output_name.clone(),
                content: bytes,
                source_path: None,
                source_kind: DiscoveredArtifactSourceKind::ChainworksOutput,
            })
        })
        .collect()
}

fn labeled_output_candidates(spec: &ExpectedOutputSpec) -> Vec<String> {
    let mut labels = Vec::new();
    push_label_candidate(&mut labels, &spec.output_name);
    push_label_candidate(&mut labels, &spec.output_name.replace('_', " "));
    push_label_candidate(&mut labels, &spec.display_label);
    if let Some(contract_id) = spec.contract_id.as_deref() {
        push_label_candidate(&mut labels, contract_id);
    }
    labels
}

fn push_label_candidate(labels: &mut Vec<String>, candidate: &str) {
    let normalized = candidate.trim().to_ascii_lowercase();
    if !normalized.is_empty() && !labels.iter().any(|existing| existing == &normalized) {
        labels.push(normalized);
    }
}

fn labeled_json_payload_after<'a>(
    stream_text: &'a str,
    ascii_lower: &str,
    label: &str,
) -> Option<&'a str> {
    let mut cursor = 0usize;
    while let Some(found_rel) = ascii_lower[cursor..].find(label) {
        let label_start = cursor + found_rel;
        let label_end = label_start + label.len();
        cursor = label_end;
        if !valid_label_start_boundary(stream_text.as_bytes(), label_start) {
            continue;
        }
        let mut payload_start = skip_horizontal_whitespace(stream_text, label_end);
        let Some(after_separator) = consume_optional_label_separator(stream_text, payload_start)
        else {
            continue;
        };
        payload_start = after_separator;
        payload_start = skip_ascii_whitespace(stream_text, payload_start);
        if let Some(payload) = fenced_json_payload_at(stream_text, payload_start) {
            return Some(payload);
        }
        if let Some(payload) = inline_json_payload_at(stream_text, payload_start) {
            return Some(payload);
        }
    }
    None
}

fn valid_label_start_boundary(bytes: &[u8], start: usize) -> bool {
    start == 0
        || matches!(
            bytes[start.saturating_sub(1)],
            b'\n' | b'\r' | b'\t' | b' ' | b'#' | b'*' | b'-' | b'`'
        )
}

fn skip_horizontal_whitespace(text: &str, mut index: usize) -> usize {
    while matches!(text.as_bytes().get(index), Some(b' ' | b'\t')) {
        index += 1;
    }
    index
}

fn skip_ascii_whitespace(text: &str, mut index: usize) -> usize {
    while text
        .as_bytes()
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    index
}

fn consume_optional_label_separator(text: &str, index: usize) -> Option<usize> {
    match text.as_bytes().get(index) {
        Some(b':') => Some(index + 1),
        Some(b'\n' | b'\r') => Some(index),
        Some(b'-') if text.as_bytes().get(index + 1) == Some(&b' ') => Some(index + 1),
        Some(b'`') | Some(b'{') => Some(index),
        _ => None,
    }
}

fn fenced_json_payload_at(text: &str, start: usize) -> Option<&str> {
    let rest = text.get(start..)?;
    let after_ticks = rest.strip_prefix("```")?;
    let content_start_rel = after_ticks.find('\n')? + 1;
    let content_start = start + 3 + content_start_rel;
    let content_rest = text.get(content_start..)?;
    let end_rel = content_rest
        .find("\n```")
        .or_else(|| content_rest.find("```"))?;
    let payload = content_rest[..end_rel].trim();
    serde_json::from_str::<Value>(payload).ok()?;
    Some(payload)
}

fn inline_json_payload_at(text: &str, start: usize) -> Option<&str> {
    let rest = text.get(start..)?;
    let end = json_value_prefix_len(rest)?;
    let payload = rest[..end].trim();
    serde_json::from_str::<Value>(payload).ok()?;
    Some(payload)
}

fn json_value_prefix_len(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let (_, first) = chars.next()?;
    let expected_close = match first {
        '{' => '}',
        '[' => ']',
        _ => return None,
    };
    let mut stack = vec![expected_close];
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in chars {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => stack.push('}'),
            '[' => stack.push(']'),
            '}' | ']' => {
                if stack.pop()? != ch {
                    return None;
                }
                if stack.is_empty() {
                    return Some(idx + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
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
    // SEC-ACP-001: log sanitized summary only — params can carry bearer tokens, prompts, env vars.
    debug!(msg = %sanitize_outbound_acp_debug(msg), "ACP → subprocess");
    Ok(())
}

// ---------------------------------------------------------------------------
// Handshake response reader — blocks until a response with `expected_id` arrives.
// Notifications (no `id` field) are silently skipped.
// ---------------------------------------------------------------------------

pub(crate) async fn await_response(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    child: &mut Child,
    expected_id: &str,
    time_limit: Duration,
    phase: &str,
) -> Result<Value> {
    let start = Instant::now();
    let mut line = String::new();

    loop {
        ensure_provider_process_alive(child, phase)?;
        let elapsed = start.elapsed();
        if elapsed >= time_limit {
            bail!("ACP handshake timed out waiting for response id={expected_id}");
        }
        let remaining = time_limit - elapsed;
        let read_wait = remaining.min(PROVIDER_PROCESS_POLL_INTERVAL);

        line.clear();
        let n = match timeout(
            read_wait,
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
                ensure_provider_process_alive(child, phase)?;
                if read_wait < remaining {
                    continue;
                }
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
        // SEC-ACP-001: log summary only — handshake responses can carry session tokens.
        let handshake_summary = summarize_runtime_receipt_message(&parsed).unwrap_or_else(|| {
            format!(
                "id={}",
                parsed
                    .get("id")
                    .and_then(normalize_jsonrpc_id)
                    .unwrap_or_default()
            )
        });
        debug!(msg = %handshake_summary, "ACP ← subprocess (handshake)");

        // Extract response id — ACP may encode it as integer or string
        let msg_id = parsed.get("id").and_then(normalize_jsonrpc_id);

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

fn ensure_provider_process_alive(child: &mut Child, phase: &str) -> Result<()> {
    match child.try_wait() {
        Ok(Some(status)) => {
            bail!("ACP provider subprocess exited during {phase}: {status}");
        }
        Ok(None) => Ok(()),
        Err(error) => Err(error).context(format!(
            "ACP provider subprocess liveness check during {phase}"
        )),
    }
}

async fn diagnose_late_handshake_response(
    reader: &mut BufReader<tokio::process::ChildStdout>,
    expected_id: &str,
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
            let late_id = parsed.get("id").and_then(normalize_jsonrpc_id);
            if late_id.as_deref() != Some(expected_id) {
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

    let request_id = format_client_request_id("probe-initialize", 1);
    send_ndjson(
        &mut stdin,
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
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

    let result = await_response(
        &mut reader,
        &mut child,
        &request_id,
        handshake_timeout,
        "capability probe initialize handshake",
    )
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

fn build_permission_grant(request_id: &Value, params: &Value) -> Option<Value> {
    let options = permission_options(params);
    let option_id = permission_preferred_auto_grant_option(&options)?;

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

/// SEC-P079-001: P079 repair-specific permission grant builder.
/// Only selects a single-use allow_once option for the canonical fs.write request.
/// Fails closed: if no allow_once option exists, returns None so the caller can
/// settle as failed rather than granting any broader allow_always permission.
/// This must be used instead of build_permission_grant whenever p079_repair_canonical_paths is set.
fn build_p079_repair_permission_grant(request_id: &Value, params: &Value) -> Option<Value> {
    let options = permission_options(params);
    let option_id = allow_once_option(&options)?;

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

/// Explicit allowlist of tool names that represent a filesystem write operation.
/// Any tool name NOT in this list is denied during the P079 repair posture.
/// These are exact matches; substring matching is intentionally prohibited (SEC-HIGH-001).
const P079_WRITE_TOOL_ALLOWLIST: &[&str] = &[
    "write_file",         // Claude Code, Codex
    "create_file",        // Claude Code variant
    "overwrite_file",     // some provider variants
    "str_replace_editor", // Claude Code multi-purpose editor (requires command=write/create check)
    "edit_file",          // Claude Code edit tool (requires path to be checked)
];

/// Tool name prefixes that must always be denied regardless of any input field content.
/// These represent shell, network, and other non-filesystem-write operations.
const P079_ALWAYS_DENIED_PREFIXES: &[&str] = &[
    "bash",
    "shell",
    "execute",
    "run_command",
    "run_",
    "terminal",
    "computer",
    "http_request",
    "curl",
    "fetch",
    "web_search",
    "browser",
    "network",
    "mcp_",
    "list_directory",
    "read_file",
    "glob",
    "grep",
    "find_",
    "search_",
];

/// P079-SEC-HIGH-001: check whether a permission request should be denied under the
/// P079 repair permission posture. Returns true when the request must be denied.
///
/// The posture allows ONLY `fs.write` requests whose resolved target byte-matches a
/// frozen canonical output path. Everything else is denied:
/// - tools not in the explicit write allowlist
/// - tools with always-denied name prefixes (shell, network, custom, etc.)
/// - write tools where no structured path field is present (no title fallback)
/// - write tools where the structured path does not byte-match a frozen canonical path
///
/// Title/name heuristics (substring matching, title-token path extraction) are
/// intentionally removed. A provider-controlled title cannot authorize any operation.
pub fn p079_posture_denied(params: &Value, canonical_paths: &[String]) -> bool {
    let tool_name = params["toolCall"]["name"].as_str().unwrap_or("");

    // Deny tools with always-denied name prefixes first, regardless of any other field.
    let tool_lower = tool_name.to_lowercase();
    if P079_ALWAYS_DENIED_PREFIXES
        .iter()
        .any(|prefix| tool_lower.starts_with(prefix))
    {
        return true;
    }

    // Must be an exact match in the explicit write tool allowlist.
    if !P079_WRITE_TOOL_ALLOWLIST.iter().any(|&t| t == tool_name) {
        return true;
    }

    // For str_replace_editor: require command=write or command=create in structured input.
    if tool_name == "str_replace_editor" {
        let command = params["toolCall"]["input"]["command"]
            .as_str()
            .unwrap_or("");
        if command != "write" && command != "create" {
            return true;
        }
    }

    // Extract all non-empty path field values from known structured input fields.
    // If multiple distinct paths are present the provider runtime might use one
    // that differs from the canonical first field — fail closed in that case so
    // this boundary remains enforceable.
    // Title-token fallback is intentionally absent: title is provider-controlled.
    let input = &params["toolCall"]["input"];
    let raw_paths: [Option<&str>; 4] = [
        input["file_path"].as_str(),
        input["path"].as_str(),
        input["filePath"].as_str(),
        input["new_file_path"].as_str(),
    ];
    let non_empty: Vec<&str> = raw_paths
        .iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .copied()
        .collect();

    // No structured path found — deny (fail-closed; no implicit path allowed).
    let Some(&first_path) = non_empty.first() else {
        return true;
    };

    // Multiple distinct paths present — ambiguous which one the provider runtime
    // will use; deny to prevent a canonical first field masking a non-canonical one.
    if non_empty.iter().any(|&p| p != first_path) {
        return true;
    }

    // Byte-exact comparison against frozen canonical output paths.
    !canonical_paths.iter().any(|p| p == first_path)
}

/// Extract the tool name and normalized path from a permission request params,
/// for use in structured P079 permission decision evidence.
/// Returns (tool_name, normalized_path). Path is empty string when absent.
pub fn p079_extract_decision_fields(params: &Value) -> (String, String) {
    // SEC-P079-002: sanitize the provider-controlled tool name before it reaches
    // permission_decisions.method storage. Strip control characters and cap length
    // so providers cannot inject newlines, tokens, or large strings into readback.
    let raw_tool_name = params["toolCall"]["name"].as_str().unwrap_or("");
    let tool_name = p079_sanitize_method_name(raw_tool_name);
    let path = params["toolCall"]["input"]["file_path"]
        .as_str()
        .or_else(|| params["toolCall"]["input"]["path"].as_str())
        .or_else(|| params["toolCall"]["input"]["filePath"].as_str())
        .or_else(|| params["toolCall"]["input"]["new_file_path"].as_str())
        .unwrap_or("")
        .to_string();
    (tool_name, path)
}

/// Sanitize a provider-supplied tool name for safe storage in permission_decisions.method.
/// Strips ASCII control characters (including newlines and tabs) and caps to 128 bytes.
/// SEC-P079-002: prevents injection of log-injection payloads or bearer tokens via tool names.
fn p079_sanitize_method_name(raw: &str) -> String {
    const MAX_METHOD_BYTES: usize = 128;
    // Strip ASCII control characters (covers newlines, tabs, carriage returns).
    let sanitized: String = raw.chars().filter(|c| !c.is_ascii_control()).collect();
    // SEC-P079-002: truncate at a valid UTF-8 character boundary to prevent panic
    // on multibyte characters that straddle the byte limit.
    let truncated = if sanitized.len() > MAX_METHOD_BYTES {
        let mut end = MAX_METHOD_BYTES;
        while !sanitized.is_char_boundary(end) {
            end -= 1;
        }
        &sanitized[..end]
    } else {
        &sanitized
    };
    // SEC-P079-002: redact patterns that could leak credentials or absolute paths.
    // Only redact if the name contains a suspicious pattern; normal tool names
    // (write_file, bash, str_replace_editor) are preserved unchanged.
    p079_redact_method_name_credential_patterns(truncated)
}

/// Redacts credential-like substrings from a sanitized, already-truncated tool name.
/// SEC-P079-MED-004: covers leading and embedded absolute filesystem paths, bearer
/// tokens (case-insensitive), and common API token prefixes (sk-, ghp_, xoxb-, etc).
fn p079_redact_method_name_credential_patterns(name: &str) -> String {
    // Embedded absolute path: tool names like "write_file_/Users/user/.ssh/id_rsa".
    // Check for both leading slash and common absolute path components anywhere.
    // Aligns with p079_redact_transport_error path roots (same sanitizer family).
    if name.starts_with('/')
        || name.contains("/Users/")
        || name.contains("/home/")
        || name.contains("/tmp/")
        || name.contains("/var/")
        || name.contains("/etc/")
        || name.contains("/root/")
        || name.contains("/private/")
        || name.contains("/Volumes/")
        || name.contains("/proc/")
        || name.contains("/sys/")
        || name.contains("/run/")
    {
        return "[REDACTED_PATH]".to_string();
    }
    let lower = name.to_lowercase();
    // Bearer token or known sensitive keyword prefixes embedded in tool names.
    if lower.contains("bearer ")
        || lower.contains("password")
        || lower.contains("secret")
        || lower.contains("api_key")
        || lower.contains("apikey")
    {
        return "[REDACTED_CREDENTIAL]".to_string();
    }
    // Common API token prefixes: sk-, ghp_, xoxb-, xoxp-, AKIA, github_pat_, etc.
    // These are the same prefixes used in plan-evidence credential redaction.
    for prefix in &[
        "sk-ant-",
        "sk-",
        "AIza",
        "anth-",
        "ghp_",
        "ghs_",
        "gho_",
        "github_pat_",
        "AKIA",
        "ASIA",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
    ] {
        if name.contains(prefix) {
            return "[REDACTED_CREDENTIAL]".to_string();
        }
    }
    name.to_string()
}

/// Classify a denied tool name into the closed P079 resource_kind enum.
/// Used for evidence when the posture denies a request.
pub fn p079_classify_resource_kind_from_tool(tool_name: &str) -> &'static str {
    let lower = tool_name.to_lowercase();
    if lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("exec")
        || lower.contains("run_command")
        || lower.contains("terminal")
        || lower.contains("computer")
    {
        return "shell";
    }
    if lower.contains("http")
        || lower.contains("curl")
        || lower.contains("fetch")
        || lower.contains("web_search")
        || lower.contains("browser")
        || lower.contains("network")
    {
        return "network";
    }
    if lower.starts_with("mcp_") {
        return "tool_mcp";
    }
    if lower.contains("read_file") || lower.contains("list_directory") || lower.contains("glob") {
        return "fs_read";
    }
    // Unknown write-like tools that aren't in the canonical allowlist are custom.
    "tool_custom"
}

fn build_permission_denial(request_id: &Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": request_id,
        "error": {
            "code": -32001,
            "message": "Permission denied by P079 repair posture: request outside frozen canonical output paths (unsafe_continuation)"
        }
    })
}

/// P079-SEC-HIGH-001/003: verify that no component of the given path (parents AND the final
/// file component) is a symlink. Returns true when the path is safe.
/// Fail-closed: any lstat failure or symlink in any path component returns false.
///
/// This is called at permission-grant time for P079 canonical output writes. Even when the
/// requested path byte-matches a frozen canonical output path, a swap of any component to a
/// symlink after the canonical path was computed can redirect the write outside the run
/// meta-root. Checking the final file component (SEC-HIGH-001) is required because a provider
/// can pre-create the declared output file as a symlink before requesting write permission.
async fn p079_path_parents_have_no_symlinks(path: &str) -> bool {
    use std::path::PathBuf;
    let pb = PathBuf::from(path);
    let parent = match pb.parent() {
        Some(p) => p.to_path_buf(),
        None => return true,
    };
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component);
        match tokio::fs::symlink_metadata(&current).await {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return false;
                }
            }
            Err(_) => {
                // Fail-closed: if we cannot stat a parent component, deny.
                return false;
            }
        }
    }
    // SEC-P079-HIGH-001: also check the final file component. The parent walk above only
    // covers parent directories; a provider can pre-create the declared output path as a
    // symlink to an outside file and request write permission. When the file does not yet
    // exist (ENOENT) there is no symlink to redirect through — allow. Unknown stat errors
    // on the final component are fail-closed.
    match tokio::fs::symlink_metadata(&pb).await {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return false;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File does not exist yet — no symlink present, allow.
        }
        Err(_) => {
            // Unknown stat failure on final component — fail closed.
            return false;
        }
    }
    true
}

fn permission_options(params: &Value) -> Vec<&Value> {
    params["options"]
        .as_array()
        .map(|a| a.iter().collect())
        .or_else(|| {
            params["toolCall"]["options"]
                .as_array()
                .map(|a| a.iter().collect())
        })
        .unwrap_or_default()
}

fn permission_preferred_auto_grant_option<'a>(options: &'a [&'a Value]) -> Option<&'a str> {
    read_only_allow_always_option(options)
        .or_else(|| allow_once_option(options))
        .or_else(|| approved_option(options))
}

fn read_only_allow_always_option<'a>(options: &'a [&'a Value]) -> Option<&'a str> {
    options
        .iter()
        .find(|option| {
            option["kind"].as_str() == Some("allow_always")
                && permission_option_text(option).contains("read-only")
        })
        .and_then(|option| option["optionId"].as_str())
}

fn allow_once_option<'a>(options: &'a [&'a Value]) -> Option<&'a str> {
    options
        .iter()
        .find(|option| option["kind"].as_str() == Some("allow_once"))
        .and_then(|option| option["optionId"].as_str())
}

fn approved_option<'a>(options: &'a [&'a Value]) -> Option<&'a str> {
    options
        .iter()
        .find(|option| option["optionId"].as_str() == Some("approved"))
        .and_then(|option| option["optionId"].as_str())
}

fn permission_option_text(option: &Value) -> String {
    let name = option["name"].as_str().unwrap_or_default();
    let option_id = option["optionId"].as_str().unwrap_or_default();
    format!("{name} {option_id}").to_lowercase()
}

fn permission_option_ids(params: &Value) -> Vec<String> {
    params["options"]
        .as_array()
        .map(|a| a.iter().collect::<Vec<_>>())
        .or_else(|| {
            params["toolCall"]["options"]
                .as_array()
                .map(|a| a.iter().collect::<Vec<_>>())
        })
        .unwrap_or_default()
        .into_iter()
        .filter_map(|option| option["optionId"].as_str().map(ToOwned::to_owned))
        .collect()
}

fn summarize_permission_request(request_id: &Value, params: &Value) -> String {
    let request_id = normalize_jsonrpc_id(request_id).unwrap_or_else(|| request_id.to_string());
    let tool_title = params["toolCall"]["title"].as_str().unwrap_or("unknown");
    let option_ids = permission_option_ids(params);
    format!(
        "id={request_id};title={tool_title};options={}",
        option_ids.join("|")
    )
}

/// SEC-MED-001: Returns a sanitized event label for P079 unsafe continuation events.
/// Only includes the request ID and resource kind (both server-derived), never provider-controlled
/// title text, option IDs, or path content that may carry credentials or tokens.
fn p079_sanitized_event_label(normalized_req_id: &str, resource_kind: &str) -> String {
    format!("id={normalized_req_id};resource_kind={resource_kind}")
}

fn summarize_permission_grant(grant: &Value) -> String {
    let request_id = grant
        .get("id")
        .and_then(normalize_jsonrpc_id)
        .unwrap_or_else(|| Value::Null.to_string());
    let option_id = grant["result"]["outcome"]["optionId"]
        .as_str()
        .unwrap_or("unknown");
    format!("id={request_id};selected={option_id}")
}

fn summarize_runtime_receipt_message(parsed: &Value) -> Option<String> {
    if let Some(method) = parsed.get("method").and_then(Value::as_str) {
        return Some(format!("method={method}"));
    }
    let msg_id = parsed
        .get("id")
        .and_then(normalize_jsonrpc_id)
        // SEC-ACP-002: cap provider-controlled response IDs before they reach logs/receipts.
        .map(|raw| cap_provider_request_id(&raw));
    let is_error = parsed.get("error").is_some();
    let has_result = parsed.get("result").is_some();
    match (msg_id, is_error, has_result) {
        (Some(msg_id), true, _) => Some(format!("response_error id={msg_id}")),
        (Some(msg_id), _, true) => Some(format!("response_result id={msg_id}")),
        (Some(msg_id), _, _) => Some(format!("response id={msg_id}")),
        _ => None,
    }
}

fn json_for_runtime_receipt(value: &Value) -> Option<String> {
    serde_json::to_string(value)
        .ok()
        .map(|json| truncate_runtime_receipt_payload(&json))
}

/// SEC-ACP-001: produce a redacted summary of an outbound ACP message safe for debug logging.
/// Logs method, id, and a field-count summary for params — never param values, which can
/// carry bearer tokens, prompts, MCP environment vars, and other secrets.
fn sanitize_outbound_acp_debug(msg: &Value) -> String {
    let method = msg
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("<response>");
    let id = msg
        .get("id")
        .and_then(normalize_jsonrpc_id)
        .unwrap_or_else(|| "<none>".to_string());
    match msg
        .get("params")
        .and_then(Value::as_object)
        .map(|o| o.len())
    {
        Some(n) if n > 0 => format!("method={method} id={id} params=[{n} fields redacted]"),
        _ => format!("method={method} id={id}"),
    }
}

/// SEC-ACP-002: hash a provider-supplied JSON-RPC request ID before storing in runtime receipts
/// or readback surfaces. All provider-controlled IDs are replaced unconditionally with a stable
/// short hash — short alphanumeric IDs can still carry token-shaped secrets (e.g. `sk-abc12`,
/// `ghp_1234567`, `AKIA…`) that fit the old allowlist. Hashing everything prevents any
/// provider-controlled string from reaching non-operator readback.
fn cap_provider_request_id(raw: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    raw.hash(&mut h);
    format!("pid-{:016x}", h.finish())
}

fn format_client_request_id(purpose: &str, sequence: u64) -> String {
    format!("chainworks-{purpose}-{sequence}")
}

fn normalize_jsonrpc_id(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text.clone()),
        _ => None,
    }
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
    provider: String,
    model: Option<String>,
    permission_grant_debounce: Duration,
    xcode_shim_injected: bool,
    requires_xcode_host_execution: bool,
    claude_sdk_debug_file_path: Option<String>,
    last_runtime_receipt: Option<AcpRuntimeReceipt>,
}

#[derive(Clone, Debug)]
struct RuntimeReceiptTracker {
    started_at_wall: chrono::DateTime<chrono::Utc>,
    started_at_mono: Instant,
    handshake: AcpRuntimeReceiptHandshake,
    counters: AcpRuntimeReceiptCounters,
    permission_roundtrips: Vec<RuntimeReceiptPermissionRoundtrip>,
    first_events: Vec<AcpRuntimeReceiptEvent>,
    last_events: Vec<AcpRuntimeReceiptEvent>,
    last_event_kind: Option<String>,
    last_event_at_ms: Option<u64>,
    claude_diagnostics: Option<AcpClaudeRuntimeDiagnostics>,
    /// P079-SEC-HIGH-001: true when the repair turn was terminated by a posture
    /// denial. Propagated to AcpRuntimeReceipt.p079_unsafe_continuation.
    p079_unsafe_continuation: bool,
}

#[derive(Clone, Debug)]
struct RuntimeReceiptPermissionRoundtrip {
    request_id: String,
    requested_at_ms: u64,
    request_summary: Option<String>,
    request_payload: Option<String>,
    grant_sent_at_ms: Option<u64>,
    grant_summary: Option<String>,
    grant_payload: Option<String>,
    first_post_grant_event_at_ms: Option<u64>,
    first_post_grant_event_kind: Option<String>,
    first_post_grant_event_detail: Option<String>,
    outcome: Option<String>,
    /// P079-SEC-MED-001: structured decision fields recorded at evaluation time,
    /// not derived post-hoc from grant_sent_at_ms.
    p079_tool_name: Option<String>,
    p079_normalized_path: Option<String>,
    p079_matched_canonical_path: Option<String>,
    p079_decision_reason: Option<String>,
    p079_resource_kind: Option<String>,
}

impl RuntimeReceiptTracker {
    fn new(started_at_wall: chrono::DateTime<chrono::Utc>, started_at_mono: Instant) -> Self {
        Self {
            started_at_wall,
            started_at_mono,
            handshake: AcpRuntimeReceiptHandshake::default(),
            counters: AcpRuntimeReceiptCounters::default(),
            permission_roundtrips: Vec::new(),
            first_events: Vec::new(),
            last_events: Vec::new(),
            last_event_kind: None,
            last_event_at_ms: None,
            claude_diagnostics: None,
            p079_unsafe_continuation: false,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        self.started_at_mono.elapsed().as_millis() as u64
    }

    fn push_event(&mut self, kind: impl Into<String>, detail: Option<String>) {
        let at_ms = self.elapsed_ms();
        let event = AcpRuntimeReceiptEvent {
            at_ms,
            kind: kind.into(),
            detail: detail.map(|detail| truncate_runtime_receipt_detail(&detail)),
        };
        if self.first_events.len() < RUNTIME_RECEIPT_EVENT_SAMPLE_LIMIT {
            self.first_events.push(event.clone());
        }
        self.last_events.push(event.clone());
        if self.last_events.len() > RUNTIME_RECEIPT_EVENT_SAMPLE_LIMIT {
            self.last_events.remove(0);
        }
        self.last_event_kind = Some(event.kind.clone());
        self.last_event_at_ms = Some(at_ms);
    }

    fn note_initialize_sent(&mut self, request_id: &str) {
        self.handshake.initialize_sent_at_ms = Some(self.elapsed_ms());
        self.push_event("initialize_sent", Some(format!("id={request_id}")));
    }

    fn note_initialize_received(&mut self, request_id: &str) {
        self.handshake.initialize_received_at_ms = Some(self.elapsed_ms());
        self.push_event("initialize_received", Some(format!("id={request_id}")));
    }

    fn note_session_new_sent(&mut self, request_id: &str) {
        self.handshake.session_new_sent_at_ms = Some(self.elapsed_ms());
        self.push_event("session_new_sent", Some(format!("id={request_id}")));
    }

    fn note_session_new_received(&mut self, session_id: &str) {
        self.handshake.session_new_received_at_ms = Some(self.elapsed_ms());
        self.push_event(
            "session_new_received",
            Some(format!("session_id={session_id}")),
        );
    }

    fn note_prompt_sent(&mut self, request_id: &str) {
        self.handshake.prompt_sent_at_ms = Some(self.elapsed_ms());
        self.push_event("prompt_sent", Some(format!("id={request_id}")));
    }

    fn note_terminal_response(&mut self, status: &str) {
        self.handshake.terminal_response_at_ms = Some(self.elapsed_ms());
        self.push_event("terminal_response", Some(format!("status={status}")));
    }

    fn configure_claude_diagnostics(&mut self, debug_file_path: Option<String>) {
        self.claude_diagnostics = Some(AcpClaudeRuntimeDiagnostics {
            debug_file_path,
            ..AcpClaudeRuntimeDiagnostics::default()
        });
    }

    fn note_claude_sdk_message(&mut self, parsed: &Value) -> bool {
        let Some(observation) = claude_sdk_message_observation(parsed) else {
            return false;
        };
        let at_ms = self.elapsed_ms();
        let diagnostics = self
            .claude_diagnostics
            .get_or_insert_with(AcpClaudeRuntimeDiagnostics::default);
        diagnostics.raw_sdk_message_count += 1;
        diagnostics.last_sdk_message_type = Some(observation.message_type.clone());
        diagnostics.last_sdk_message_subtype = observation.subtype.clone();
        match observation.message_type.as_str() {
            "stream_event" => {
                diagnostics.stream_event_count += 1;
                diagnostics.last_stream_event_type = observation.stream_event_type.clone();
            }
            "assistant" => diagnostics.assistant_count += 1,
            "result" => {
                diagnostics.result_count += 1;
                diagnostics.result_seen = true;
            }
            "system" if observation.subtype.as_deref() == Some("session_state_changed") => {
                diagnostics.session_state_changed_count += 1;
                if observation.session_state.as_deref() == Some("idle") {
                    diagnostics.idle_seen = true;
                }
            }
            _ => {}
        }
        diagnostics.sanitized_events.push(AcpRuntimeReceiptEvent {
            at_ms,
            kind: format!("claude_sdk:{}", observation.message_type),
            detail: Some(observation.detail),
        });
        if diagnostics.sanitized_events.len() > CLAUDE_SDK_DIAGNOSTIC_EVENT_LIMIT {
            diagnostics.sanitized_events.remove(0);
            diagnostics.sanitized_events_truncated = true;
        }
        true
    }

    fn note_claude_watchdog_cancel_sent(&mut self, send_succeeded: bool) {
        let diagnostics = self
            .claude_diagnostics
            .get_or_insert_with(AcpClaudeRuntimeDiagnostics::default);
        diagnostics.cancel_sent_on_watchdog = true;
        diagnostics.cancel_send_succeeded = send_succeeded;
        diagnostics.result_seen_before_cancel = diagnostics.result_seen;
        diagnostics.idle_seen_before_cancel = diagnostics.idle_seen;
        self.push_event(
            "claude_watchdog_cancel_sent",
            Some(format!("send_succeeded={send_succeeded}")),
        );
    }

    fn note_claude_cancel_drain_message(&mut self) {
        let diagnostics = self
            .claude_diagnostics
            .get_or_insert_with(AcpClaudeRuntimeDiagnostics::default);
        diagnostics.cancel_drain_message_count += 1;
    }

    fn note_claude_cancel_flush_observed(&mut self, status: &str) {
        let diagnostics = self
            .claude_diagnostics
            .get_or_insert_with(AcpClaudeRuntimeDiagnostics::default);
        diagnostics.cancel_flush_observed = true;
        diagnostics.cancel_terminal_status = Some(status.to_string());
        self.push_event(
            "claude_watchdog_cancel_flush_observed",
            Some(format!("status={status}")),
        );
    }

    fn note_incoming_message(&mut self, detail: Option<String>) {
        self.counters.total_messages += 1;
        self.push_event("incoming_message", detail);
    }

    fn note_permission_request(
        &mut self,
        request_id: &str,
        detail: Option<String>,
        payload: Option<String>,
    ) {
        self.counters.permission_request_count += 1;
        self.permission_roundtrips
            .push(RuntimeReceiptPermissionRoundtrip {
                request_id: request_id.to_string(),
                requested_at_ms: self.elapsed_ms(),
                request_summary: detail.clone(),
                request_payload: payload,
                grant_sent_at_ms: None,
                grant_summary: None,
                grant_payload: None,
                first_post_grant_event_at_ms: None,
                first_post_grant_event_kind: None,
                first_post_grant_event_detail: None,
                outcome: Some("awaiting_grant".to_string()),
                p079_tool_name: None,
                p079_normalized_path: None,
                p079_matched_canonical_path: None,
                p079_decision_reason: None,
                p079_resource_kind: None,
            });
        self.push_event("permission_request", detail);
    }

    /// P079-SEC-MED-001: record the structured P079 posture evaluation decision on the
    /// most-recent permission roundtrip for this request_id. This stores the actual
    /// evaluator decision (tool name, normalized path, matched canonical path, reason,
    /// resource kind) so that evidence serialization does not need to re-derive it from
    /// grant_sent_at_ms.
    fn note_p079_posture_decision(
        &mut self,
        request_id: &str,
        tool_name: &str,
        normalized_path: &str,
        matched_canonical_path: Option<&str>,
        decision_reason: &str,
        resource_kind: &str,
    ) {
        if let Some(roundtrip) = self
            .permission_roundtrips
            .iter_mut()
            .rev()
            .find(|rt| rt.request_id == request_id)
        {
            roundtrip.p079_tool_name = Some(tool_name.to_string());
            roundtrip.p079_normalized_path = Some(normalized_path.to_string());
            roundtrip.p079_matched_canonical_path = matched_canonical_path.map(|s| s.to_string());
            roundtrip.p079_decision_reason = Some(decision_reason.to_string());
            roundtrip.p079_resource_kind = Some(resource_kind.to_string());
        }
    }

    fn note_permission_grant_sent(
        &mut self,
        request_id: &str,
        detail: Option<String>,
        payload: Option<String>,
    ) {
        self.counters.permission_grant_sent_count += 1;
        self.counters.meaningful_progress_count += 1;
        let at_ms = self.elapsed_ms();
        if let Some(roundtrip) = self
            .permission_roundtrips
            .iter_mut()
            .rev()
            .find(|roundtrip| {
                roundtrip.request_id == request_id && roundtrip.grant_sent_at_ms.is_none()
            })
        {
            roundtrip.grant_sent_at_ms = Some(at_ms);
            roundtrip.grant_summary = detail.clone();
            roundtrip.grant_payload = payload;
            roundtrip.outcome = Some("awaiting_post_grant_event".to_string());
        }
        self.push_event("permission_grant_sent", detail);
    }

    fn note_permission_grant_failed(&mut self, request_id: &str, detail: Option<String>) {
        self.counters.permission_grant_failed_count += 1;
        if let Some(roundtrip) = self
            .permission_roundtrips
            .iter_mut()
            .rev()
            .find(|roundtrip| {
                roundtrip.request_id == request_id && roundtrip.grant_sent_at_ms.is_none()
            })
        {
            roundtrip.outcome = Some("grant_send_failed".to_string());
        }
        self.push_event("permission_grant_failed", detail);
    }

    fn note_post_grant_event(&mut self, kind: impl Into<String>, detail: Option<String>) {
        let kind = kind.into();
        let at_ms = self.elapsed_ms();
        if let Some(roundtrip) = self
            .permission_roundtrips
            .iter_mut()
            .rev()
            .find(|roundtrip| {
                roundtrip.grant_sent_at_ms.is_some()
                    && roundtrip.first_post_grant_event_at_ms.is_none()
            })
        {
            roundtrip.first_post_grant_event_at_ms = Some(at_ms);
            roundtrip.first_post_grant_event_kind = Some(kind);
            roundtrip.first_post_grant_event_detail =
                detail.map(|detail| truncate_runtime_receipt_detail(&detail));
            roundtrip.outcome = Some("post_grant_activity_observed".to_string());
        }
    }

    fn note_session_update(
        &mut self,
        kind: &str,
        meaningful_progress: bool,
        detail: Option<String>,
    ) {
        self.counters.session_update_count += 1;
        match kind {
            "agent_message_chunk" => self.counters.agent_message_chunk_count += 1,
            "agent_thought_chunk" => self.counters.agent_thought_chunk_count += 1,
            "tool_call" => self.counters.tool_call_count += 1,
            "tool_call_update" => self.counters.tool_call_update_count += 1,
            "plan" => self.counters.plan_update_count += 1,
            _ => self.counters.unknown_notification_count += 1,
        }
        if meaningful_progress {
            self.counters.meaningful_progress_count += 1;
        }
        self.push_event(format!("session_update:{kind}"), detail);
    }

    fn build(
        &self,
        provider: &str,
        model: Option<&String>,
        provider_session_id: &str,
        session_generation_id: Option<&String>,
        xcode_shim_injected: bool,
        requires_xcode_host_execution: bool,
        status: &str,
        failure_phase: Option<String>,
    ) -> AcpRuntimeReceipt {
        AcpRuntimeReceipt {
            schema_version: 1,
            transport_family: "acp_stdio".to_string(),
            provider: provider.to_string(),
            model: model.cloned(),
            provider_session_id: Some(provider_session_id.to_string()),
            session_generation_id: session_generation_id.cloned(),
            status: status.to_string(),
            failure_phase,
            jsonrpc_error_code: None,
            provider_error_message_redacted: None,
            started_at: self.started_at_wall.to_rfc3339(),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
            xcode_shim_injected,
            requires_xcode_host_execution,
            handshake: self.handshake.clone(),
            counters: self.counters.clone(),
            permission_roundtrips: self
                .permission_roundtrips
                .iter()
                .cloned()
                .map(|mut roundtrip| {
                    roundtrip.outcome = Some(
                        match (
                            roundtrip.grant_sent_at_ms.is_some(),
                            roundtrip.first_post_grant_event_at_ms.is_some(),
                            status,
                        ) {
                            (false, _, "failed") => "permission_request_unresolved",
                            (false, _, _) => "awaiting_grant",
                            (true, false, "failed") => "timed_out_without_post_grant_event",
                            (true, false, "completed") => "completed_without_post_grant_event",
                            (true, false, _) => "awaiting_post_grant_event",
                            (true, true, _) => "post_grant_activity_observed",
                        }
                        .to_string(),
                    );
                    AcpRuntimeReceiptPermissionRoundtrip {
                        request_id: roundtrip.request_id,
                        requested_at_ms: roundtrip.requested_at_ms,
                        request_summary: roundtrip.request_summary,
                        request_payload: roundtrip.request_payload,
                        grant_sent_at_ms: roundtrip.grant_sent_at_ms,
                        grant_summary: roundtrip.grant_summary,
                        grant_payload: roundtrip.grant_payload,
                        first_post_grant_event_at_ms: roundtrip.first_post_grant_event_at_ms,
                        first_post_grant_event_kind: roundtrip.first_post_grant_event_kind,
                        first_post_grant_event_detail: roundtrip.first_post_grant_event_detail,
                        outcome: roundtrip.outcome,
                        p079_tool_name: roundtrip.p079_tool_name,
                        p079_normalized_path: roundtrip.p079_normalized_path,
                        p079_matched_canonical_path: roundtrip.p079_matched_canonical_path,
                        p079_decision_reason: roundtrip.p079_decision_reason,
                        p079_resource_kind: roundtrip.p079_resource_kind,
                    }
                })
                .collect(),
            first_events: self.first_events.clone(),
            last_events: self.last_events.clone(),
            claude_diagnostics: self.claude_diagnostics.clone(),
            p079_unsafe_continuation: self.p079_unsafe_continuation,
        }
    }
}

#[derive(Clone, Debug)]
struct ClaudeSdkMessageObservation {
    message_type: String,
    subtype: Option<String>,
    stream_event_type: Option<String>,
    session_state: Option<String>,
    detail: String,
}

fn bounded_claude_diagnostic_scalar(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|text| text.chars().take(96).collect::<String>())
        .filter(|text| !text.is_empty())
}

fn claude_sdk_message_observation(parsed: &Value) -> Option<ClaudeSdkMessageObservation> {
    if parsed.get("method").and_then(Value::as_str) != Some("_claude/sdkMessage") {
        return None;
    }
    let message = parsed.pointer("/params/message")?;
    let message_type = bounded_claude_diagnostic_scalar(message.get("type")?)?;
    let subtype = message
        .get("subtype")
        .and_then(bounded_claude_diagnostic_scalar);
    let accepted = matches!(
        message_type.as_str(),
        "stream_event" | "assistant" | "result"
    ) || (message_type == "system"
        && subtype.as_deref() == Some("session_state_changed"));
    if !accepted {
        return None;
    }

    let stream_event_type = message
        .pointer("/event/type")
        .and_then(bounded_claude_diagnostic_scalar);
    let session_state = message
        .get("state")
        .and_then(bounded_claude_diagnostic_scalar);
    let mut detail = vec![format!("type={message_type}")];
    if let Some(subtype) = &subtype {
        detail.push(format!("subtype={subtype}"));
    }
    if let Some(uuid) = message
        .get("uuid")
        .and_then(bounded_claude_diagnostic_scalar)
    {
        detail.push(format!("uuid={uuid}"));
    }
    if let Some(message_id) = message
        .pointer("/message/id")
        .and_then(bounded_claude_diagnostic_scalar)
    {
        detail.push(format!("message_id={message_id}"));
    }
    if let Some(stop_reason) = message
        .pointer("/message/stop_reason")
        .and_then(bounded_claude_diagnostic_scalar)
    {
        detail.push(format!("stop_reason={stop_reason}"));
    }
    if let Some(event_type) = &stream_event_type {
        detail.push(format!("stream_event={event_type}"));
    }
    if let Some(index) = message.pointer("/event/index").and_then(Value::as_u64) {
        detail.push(format!("index={index}"));
    }
    if let Some(state) = &session_state {
        detail.push(format!("state={state}"));
    }

    let content_blocks: Vec<&Value> = message
        .pointer("/message/content")
        .and_then(Value::as_array)
        .map(|blocks| blocks.iter().collect())
        .or_else(|| {
            message
                .pointer("/event/content_block")
                .map(|block| vec![block])
        })
        .unwrap_or_default();
    let content_types: Vec<String> = content_blocks
        .iter()
        .filter_map(|block| block.get("type").and_then(bounded_claude_diagnostic_scalar))
        .collect();
    if !content_types.is_empty() {
        detail.push(format!("content_types={}", content_types.join(",")));
    }
    for block in content_blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
    {
        if let Some(name) = block.get("name").and_then(bounded_claude_diagnostic_scalar) {
            detail.push(format!("tool_name={name}"));
        }
        if let Some(tool_use_id) = block.get("id").and_then(bounded_claude_diagnostic_scalar) {
            detail.push(format!("tool_use_id={tool_use_id}"));
        }
    }

    Some(ClaudeSdkMessageObservation {
        message_type,
        subtype,
        stream_event_type,
        session_state,
        detail: detail.join(";"),
    })
}

fn claude_sdk_message_extends_watchdog(parsed: &Value) -> bool {
    let Some(observation) = claude_sdk_message_observation(parsed) else {
        return false;
    };

    matches!(
        (
            observation.message_type.as_str(),
            observation.stream_event_type.as_deref()
        ),
        ("assistant", _)
            | (
                "stream_event",
                Some("content_block_start" | "content_block_delta" | "content_block_stop")
            )
    )
}

fn session_cancel_notification(session_id: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "session/cancel",
        "params": {"sessionId": session_id}
    })
}

fn is_claude_provider(provider: &str) -> bool {
    provider
        .strip_suffix("_acp")
        .unwrap_or(provider)
        .eq_ignore_ascii_case("claude")
}

async fn cancel_and_drain_claude_watchdog<R, W>(
    reader: &mut R,
    writer: &mut W,
    session_id: &str,
    prompt_id: &str,
    max_line_bytes: usize,
    drain_window: Duration,
    runtime_receipt: &mut RuntimeReceiptTracker,
) -> Result<()>
where
    R: tokio::io::AsyncBufRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let cancel = session_cancel_notification(session_id);
    let mut payload = serde_json::to_vec(&cancel).context("serialize ACP session/cancel")?;
    payload.push(b'\n');
    if let Err(error) = writer.write_all(&payload).await {
        runtime_receipt.note_claude_watchdog_cancel_sent(false);
        return Err(error).context("write ACP session/cancel");
    }
    writer.flush().await.context("flush ACP session/cancel")?;
    runtime_receipt.note_claude_watchdog_cancel_sent(true);

    let drain_started = Instant::now();
    let mut line = String::new();
    loop {
        let remaining = drain_window.saturating_sub(drain_started.elapsed());
        if remaining.is_zero() {
            runtime_receipt.push_event(
                "claude_watchdog_cancel_drain_finished",
                Some("reason=timeout".to_string()),
            );
            break;
        }
        let n = match timeout(
            remaining,
            read_capped_ndjson_line(
                reader,
                &mut line,
                max_line_bytes,
                "Claude watchdog cancel drain",
            ),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                runtime_receipt.push_event(
                    "claude_watchdog_cancel_drain_finished",
                    Some("reason=timeout".to_string()),
                );
                break;
            }
        };
        if n == 0 {
            runtime_receipt.push_event(
                "claude_watchdog_cancel_drain_finished",
                Some("reason=stdout_closed".to_string()),
            );
            break;
        }
        let Ok(parsed) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        runtime_receipt.note_claude_cancel_drain_message();
        runtime_receipt.note_incoming_message(summarize_runtime_receipt_message(&parsed));
        runtime_receipt.note_claude_sdk_message(&parsed);

        if parsed.get("id").and_then(normalize_jsonrpc_id).as_deref() == Some(prompt_id) {
            let status = if parsed.get("error").is_some() {
                "failed".to_string()
            } else {
                parsed
                    .pointer("/result/stopReason")
                    .or_else(|| parsed.pointer("/result/stop_reason"))
                    .and_then(bounded_claude_diagnostic_scalar)
                    .unwrap_or_else(|| "terminal".to_string())
            };
            runtime_receipt.note_claude_cancel_flush_observed(&status);
            runtime_receipt.note_terminal_response(&status);
            break;
        }
    }
    Ok(())
}

fn truncate_runtime_receipt_detail(detail: &str) -> String {
    const LIMIT: usize = 240;
    if detail.len() <= LIMIT {
        detail.to_string()
    } else {
        format!("{}…", &detail[..LIMIT])
    }
}

fn truncate_runtime_receipt_payload(payload: &str) -> String {
    const LIMIT: usize = 1200;
    if payload.len() <= LIMIT {
        payload.to_string()
    } else {
        format!("{}…", &payload[..LIMIT])
    }
}

fn session_update_observation(parsed: &Value) -> (&'static str, bool, Option<String>) {
    let mut type_markers = Vec::new();
    collect_nested_type_markers(parsed, &mut type_markers);
    let has_text_progress = extract_text_chunk(parsed)
        .as_deref()
        .map(str::trim)
        .is_some_and(|chunk| !chunk.is_empty());
    let provider_activity_marker = provider_activity_type_marker(&type_markers);
    let kind = if type_markers
        .iter()
        .any(|marker| marker == "tool_call_update")
    {
        "tool_call_update"
    } else if type_markers.iter().any(|marker| marker == "tool_call") {
        "tool_call"
    } else if type_markers
        .iter()
        .any(|marker| marker == "agent_message_chunk")
    {
        "agent_message_chunk"
    } else if type_markers
        .iter()
        .any(|marker| marker == "agent_thought_chunk")
    {
        "agent_thought_chunk"
    } else if type_markers.iter().any(|marker| marker == "plan") {
        "plan"
    } else if has_text_progress {
        "text_chunk"
    } else if provider_activity_marker.is_some() {
        "provider_activity"
    } else {
        "other"
    };
    let meaningful_progress = matches!(
        kind,
        "tool_call_update"
            | "tool_call"
            | "agent_message_chunk"
            | "agent_thought_chunk"
            | "plan"
            | "provider_activity"
            | "text_chunk"
    );
    let detail = provider_activity_marker
        .map(str::to_string)
        .or_else(|| (!type_markers.is_empty()).then(|| type_markers.join(",")))
        .or_else(|| has_text_progress.then(|| "text_progress".to_string()));
    (kind, meaningful_progress, detail)
}

fn session_update_refreshes_progress_deadline(
    update_kind: &str,
    meaningful_progress: bool,
) -> bool {
    meaningful_progress
        && matches!(
            update_kind,
            "tool_call_update"
                | "tool_call"
                | "agent_message_chunk"
                | "agent_thought_chunk"
                | "plan"
                | "provider_activity"
                | "text_chunk"
        )
}

fn timeline_title_for_update(update_kind: &str) -> &'static str {
    match update_kind {
        "tool_call" => "Tool call",
        "tool_call_update" => "Tool update",
        "agent_message_chunk" | "text_chunk" => "Agent response",
        "agent_thought_chunk" => "Agent thought",
        "plan" => "Plan update",
        "provider_activity" => "Provider activity",
        _ => "Runtime update",
    }
}

fn timeline_detail_for_update(
    update_kind: &str,
    parsed: &Value,
    fallback: Option<&str>,
) -> Option<String> {
    if matches!(
        update_kind,
        "agent_message_chunk" | "text_chunk" | "agent_thought_chunk"
    ) {
        if let Some(chunk) =
            extract_text_chunk(parsed).and_then(|chunk| bounded_timeline_detail(&chunk))
        {
            return Some(chunk);
        }
    }

    let update = parsed
        .pointer("/params/update")
        .or_else(|| parsed.pointer("/params"))
        .unwrap_or(parsed);
    let mut parts = Vec::new();
    collect_first_string_for_keys(
        update,
        &["title", "name", "command", "cmd", "path", "status", "kind"],
        &mut parts,
        4,
    );
    if !parts.is_empty() {
        return bounded_timeline_detail(&parts.join(" · "));
    }
    fallback.and_then(bounded_timeline_detail)
}

fn collect_first_string_for_keys(
    value: &Value,
    keys: &[&str],
    parts: &mut Vec<String>,
    limit: usize,
) {
    if parts.len() >= limit {
        return;
    }
    match value {
        Value::Object(map) => {
            for key in keys {
                if parts.len() >= limit {
                    return;
                }
                if let Some(text) = map.get(*key).and_then(Value::as_str) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() && !parts.iter().any(|part| part == trimmed) {
                        parts.push(trimmed.to_string());
                    }
                }
            }
            for nested in map.values() {
                collect_first_string_for_keys(nested, keys, parts, limit);
                if parts.len() >= limit {
                    return;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_first_string_for_keys(item, keys, parts, limit);
                if parts.len() >= limit {
                    return;
                }
            }
        }
        _ => {}
    }
}

fn provider_activity_type_marker(type_markers: &[String]) -> Option<&'static str> {
    if type_markers.iter().any(|marker| marker == "read") {
        Some("read")
    } else if type_markers.iter().any(|marker| marker == "search") {
        Some("search")
    } else {
        None
    }
}

fn collect_nested_type_markers(value: &Value, markers: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for key in ["sessionUpdate", "session_update", "type"] {
                if let Some(marker) = map.get(key).and_then(Value::as_str) {
                    let marker = marker.to_string();
                    if !markers.iter().any(|existing| existing == &marker) {
                        markers.push(marker);
                    }
                }
            }
            for nested in map.values() {
                collect_nested_type_markers(nested, markers);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_nested_type_markers(item, markers);
            }
        }
        _ => {}
    }
}

async fn record_prompt_progress_for_session(
    req: &ExecutionRequest,
    progress_sink: &Arc<dyn AcpPromptProgressSink>,
    provider_session_id: &str,
    kind: AcpPromptProgressKind,
) {
    record_prompt_progress_detail_for_session(
        req,
        progress_sink,
        provider_session_id,
        kind,
        None,
        None,
        None,
    )
    .await;
}

async fn record_prompt_progress_detail_for_session(
    req: &ExecutionRequest,
    progress_sink: &Arc<dyn AcpPromptProgressSink>,
    provider_session_id: &str,
    kind: AcpPromptProgressKind,
    title: Option<String>,
    detail: Option<String>,
    surface_label: Option<String>,
) {
    let update = AcpPromptProgressUpdate {
        run_id: req.run_id,
        agent_execution_id: req.agent_execution_id,
        stage_execution_id: req.stage_execution_id.clone(),
        stage_id: req.stage_id.clone(),
        agent_id: req.agent_id.clone(),
        provider: req.provider.clone(),
        session_generation_id: req.session_generation_id.clone(),
        provider_session_id: provider_session_id.to_string(),
        kind,
        title,
        detail,
        surface_label,
    };
    if let Err(error) = progress_sink.record_acp_prompt_progress(update).await {
        warn!(
            session_id = %provider_session_id,
            error = %error,
            "ACP: prompt progress sink failed"
        );
    }
}

fn bounded_timeline_detail(text: &str) -> Option<String> {
    let mut normalized = normalized_timeline_detail(text)?;
    if normalized.is_empty() {
        return None;
    }
    const LIMIT: usize = 1400;
    if normalized.len() > LIMIT {
        truncate_string_to_byte_len(&mut normalized, LIMIT);
        normalized.push_str("…");
    }
    Some(normalized)
}

fn normalized_timeline_detail(text: &str) -> Option<String> {
    let normalized = strip_ansi(text)
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

async fn poll_claude_local_activity_watchdog(
    monitor: Option<&mut ClaudeLocalActivityMonitor>,
    req: &ExecutionRequest,
    progress_sink: &Arc<dyn AcpPromptProgressSink>,
    provider_session_id: &str,
    last_provider_local_activity: &mut Option<Instant>,
    last_provider_local_progress: &mut Option<Instant>,
    last_prompt_progress_reported: &mut Option<Instant>,
) {
    let Some(monitor) = monitor else {
        return;
    };
    match monitor.poll(Instant::now()) {
        Ok(observation) if observation.should_extend_watchdog => {
            let now = Instant::now();
            *last_provider_local_activity = Some(now);
            if observation.new_event_count > 0 {
                *last_provider_local_progress = Some(now);
            }
            if observation.new_event_count > 0 {
                debug!(
                    session_id = %provider_session_id,
                    local_event_count = monitor.summary().event_count,
                    tool_uses = monitor.summary().tool_uses,
                    tool_results = monitor.summary().tool_results,
                    open_tool_uses = monitor.open_tool_use_count(),
                    open_background_tasks = monitor.open_background_task_count(),
                    "Claude local activity observed while ACP stream is quiet"
                );
            }
            if (observation.new_event_count > 0 || observation.has_open_local_activity)
                && last_prompt_progress_reported
                    .map(|reported_at| reported_at.elapsed() >= PROMPT_PROGRESS_REPORT_INTERVAL)
                    .unwrap_or(true)
            {
                record_prompt_progress_for_session(
                    req,
                    progress_sink,
                    provider_session_id,
                    AcpPromptProgressKind::ProviderLocalActivity,
                )
                .await;
                *last_prompt_progress_reported = Some(Instant::now());
            }
        }
        Ok(_) => {}
        Err(error) => {
            warn!(
                session_id = %provider_session_id,
                error = %error,
                "Claude local activity monitor failed; falling back to ACP stream activity"
            );
        }
    }
}

fn note_claude_local_activity_receipt_event(
    runtime_receipt: &mut RuntimeReceiptTracker,
    monitor: Option<&ClaudeLocalActivityMonitor>,
) -> Option<String> {
    let summary = monitor.map(ClaudeLocalActivityMonitor::summary_for_error)?;
    runtime_receipt.push_event("provider_local_activity_summary", Some(summary.clone()));
    Some(summary)
}

async fn poll_codex_local_activity_watchdog(
    monitor: Option<&mut CodexLocalActivityMonitor>,
    req: &ExecutionRequest,
    progress_sink: &Arc<dyn AcpPromptProgressSink>,
    provider_session_id: &str,
    last_provider_local_activity: &mut Option<Instant>,
    last_provider_local_progress: &mut Option<Instant>,
    last_prompt_progress_reported: &mut Option<Instant>,
) {
    let Some(monitor) = monitor else {
        return;
    };
    match monitor.poll(Instant::now()) {
        Ok(observation) if observation.should_extend_watchdog => {
            let now = Instant::now();
            *last_provider_local_activity = Some(now);
            if observation.new_event_count > 0 {
                *last_provider_local_progress = Some(now);
            }
            if observation.new_event_count > 0 {
                debug!(
                    session_id = %provider_session_id,
                    local_event_count = monitor.summary().event_count,
                    function_calls = monitor.summary().function_calls,
                    function_outputs = monitor.summary().function_outputs,
                    open_processes = monitor.open_process_count(),
                    "Codex local activity observed while ACP stream is quiet"
                );
            }
            if (observation.new_event_count > 0 || observation.has_open_local_activity)
                && last_prompt_progress_reported
                    .map(|reported_at| reported_at.elapsed() >= PROMPT_PROGRESS_REPORT_INTERVAL)
                    .unwrap_or(true)
            {
                record_prompt_progress_for_session(
                    req,
                    progress_sink,
                    provider_session_id,
                    AcpPromptProgressKind::ProviderLocalActivity,
                )
                .await;
                *last_prompt_progress_reported = Some(Instant::now());
            }
        }
        Ok(_) => {}
        Err(error) => {
            warn!(
                session_id = %provider_session_id,
                error = %error,
                "Codex local activity monitor failed; falling back to ACP stream activity"
            );
        }
    }
}

fn note_codex_local_activity_receipt_event(
    runtime_receipt: &mut RuntimeReceiptTracker,
    monitor: Option<&CodexLocalActivityMonitor>,
) -> Option<String> {
    let summary = monitor.map(CodexLocalActivityMonitor::summary_for_error)?;
    runtime_receipt.push_event("provider_local_activity_summary", Some(summary.clone()));
    Some(summary)
}

fn provider_local_activity_summary(
    runtime_receipt: &mut RuntimeReceiptTracker,
    claude_monitor: Option<&ClaudeLocalActivityMonitor>,
    codex_monitor: Option<&CodexLocalActivityMonitor>,
) -> String {
    note_claude_local_activity_receipt_event(runtime_receipt, claude_monitor)
        .or_else(|| note_codex_local_activity_receipt_event(runtime_receipt, codex_monitor))
        .unwrap_or_else(|| "provider_local_activity=unavailable".to_string())
}

fn provider_stream_silence_classification(
    claude_monitor: Option<&ClaudeLocalActivityMonitor>,
    codex_monitor: Option<&CodexLocalActivityMonitor>,
) -> &'static str {
    if claude_monitor.is_some_and(ClaudeLocalActivityMonitor::has_observed_activity)
        || codex_monitor.is_some_and(CodexLocalActivityMonitor::has_observed_activity)
    {
        "provider_stream_silent_with_local_activity"
    } else {
        "provider_stream_silent_no_local_activity"
    }
}

fn note_provider_silence_grace_receipt_event(
    runtime_receipt: &mut RuntimeReceiptTracker,
    claude_monitor: Option<&ClaudeLocalActivityMonitor>,
    codex_monitor: Option<&CodexLocalActivityMonitor>,
    phase: &str,
    elapsed: Duration,
) -> Option<String> {
    let summary = provider_local_activity_summary(runtime_receipt, claude_monitor, codex_monitor);
    runtime_receipt.push_event(
        "provider_local_activity_silence_grace_started",
        Some(format!(
            "phase={}, elapsed_s={}, grace_timeout_s={}, {}",
            phase,
            elapsed.as_secs(),
            POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT.as_secs(),
            summary
        )),
    );
    Some(summary)
}

fn recover_claude_session_store_final_response(
    completion_capture: &mut CompletionTextCapture,
    runtime_receipt: &mut RuntimeReceiptTracker,
    monitor: Option<&ClaudeLocalActivityMonitor>,
) -> bool {
    let Some(final_response) =
        monitor.and_then(ClaudeLocalActivityMonitor::latest_final_response_text)
    else {
        return false;
    };
    completion_capture.set_provider_session_store_final_response(&final_response);
    runtime_receipt.push_event(
        "provider_session_store_final_response_recovered",
        Some(format!(
            "captured_byte_count={}",
            final_response.len().min(COMPLETION_CAPTURE_RAW_BYTE_LIMIT)
        )),
    );
    true
}

impl AcpTransportSession {
    async fn record_prompt_progress(
        &self,
        req: &ExecutionRequest,
        progress_sink: &Arc<dyn AcpPromptProgressSink>,
        kind: AcpPromptProgressKind,
    ) {
        record_prompt_progress_for_session(req, progress_sink, &self.session_id, kind).await;
    }

    fn provider_failure_receipt(
        &mut self,
        runtime_receipt: &mut RuntimeReceiptTracker,
        req: &ExecutionRequest,
        provider_failure: &ProviderFailureEvent,
        terminal_response_status: Option<&str>,
        jsonrpc_error_code: Option<i64>,
        provider_error_message: Option<&str>,
    ) -> AcpRuntimeReceipt {
        runtime_receipt.push_event("provider_failure", Some(provider_failure.detail.clone()));
        if let Some(status) = terminal_response_status {
            runtime_receipt.note_terminal_response(status);
        }
        let mut receipt = runtime_receipt.build(
            &self.provider,
            self.model.as_ref(),
            &self.session_id,
            req.session_generation_id.as_ref(),
            self.xcode_shim_injected,
            self.requires_xcode_host_execution,
            "failed",
            Some(provider_failure.failure_phase.to_string()),
        );
        receipt.jsonrpc_error_code = jsonrpc_error_code;
        receipt.provider_error_message_redacted = Some(truncate_runtime_receipt_detail(
            provider_error_message.unwrap_or(&provider_failure.message),
        ));
        self.last_runtime_receipt = Some(receipt.clone());
        receipt
    }

    async fn diagnose_claude_watchdog_timeout(
        &mut self,
        req: &ExecutionRequest,
        prompt_id: &str,
        phase: &str,
        runtime_receipt: &mut RuntimeReceiptTracker,
    ) {
        if !is_claude_provider(&self.provider) {
            return;
        }
        runtime_receipt.push_event(
            "claude_watchdog_cancel_requested",
            Some(format!("phase={phase}")),
        );
        if let Err(error) = cancel_and_drain_claude_watchdog(
            &mut self.reader,
            &mut self.stdin,
            &self.session_id,
            prompt_id,
            ndjson_line_cap_bytes(&req.expected_outputs),
            CLAUDE_WATCHDOG_CANCEL_DRAIN_WINDOW,
            runtime_receipt,
        )
        .await
        {
            runtime_receipt.push_event(
                "claude_watchdog_cancel_drain_failed",
                Some(error.to_string()),
            );
            warn!(
                session_id = %self.session_id,
                phase = phase,
                error = %error,
                "Claude watchdog cancel-drain failed"
            );
        }
    }

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
        let startup_wall_started = chrono::Utc::now();
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
        let mut next_id = |purpose: &str| {
            req_counter += 1;
            format_client_request_id(purpose, req_counter)
        };

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
        let init_id = next_id("initialize");
        let initialize_started = Instant::now();
        let mut startup_receipt = RuntimeReceiptTracker::new(startup_wall_started, startup_started);
        startup_receipt.note_initialize_sent(&init_id);
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

        await_response(
            &mut reader,
            &mut child,
            &init_id,
            handshake_timeout_for_provider(&req.provider),
            "initialize handshake",
        )
        .await
        .context("ACP: initialize handshake")?;
        startup_receipt.note_initialize_received(&init_id);
        let acp_initialize_latency_ms = initialize_started.elapsed().as_millis() as u64;
        info!(
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            provider = %req.provider,
            acp_initialize_latency_ms = acp_initialize_latency_ms,
            "P053 ACP initialize latency measured"
        );

        let sn_id = next_id("session-new");
        let sn_params =
            build_session_new_params(req, config).context("ACP: build session/new params")?;
        let session_new_started = Instant::now();
        startup_receipt.note_session_new_sent(&sn_id);
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

        let sn_result = await_response(
            &mut reader,
            &mut child,
            &sn_id,
            HANDSHAKE_TIMEOUT,
            "session/new handshake",
        )
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
        startup_receipt.note_session_new_received(&session_id);
        let mcp_observation = observe_mcp_actuals(&sn_result, req, &session_id);
        let mcp_session_startup_latency_ms = mcp_observation
            .as_ref()
            .map(|_| startup_started.elapsed().as_millis() as i64);

        if config.set_mode_after_session_new {
            let set_mode_id = next_id("session-set-mode");
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
            await_response(
                &mut reader,
                &mut child,
                &set_mode_id,
                HANDSHAKE_TIMEOUT,
                "session/set_mode handshake",
            )
            .await
            .context("ACP: session/set_mode handshake")?;
        }

        for (config_id, value) in &config.config_options {
            let resolved_value = resolve_session_config_option_value(&sn_result, config_id, value)
                .unwrap_or_else(|| value.to_string());
            let sco_id = next_id("session-set-config-option");
            if let Err(e) = send_ndjson(
                &mut stdin,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": sco_id,
                    "method": "session/set_config_option",
                    "params": {
                        "sessionId": session_id,
                        "configId": config_id,
                        "value": resolved_value,
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

            match await_response(
                &mut reader,
                &mut child,
                &sco_id,
                HANDSHAKE_TIMEOUT,
                "session/set_config_option handshake",
            )
            .await
            {
                Ok(_) => {
                    // SEC-ACP-001: omit value — config option values can carry model keys or tokens.
                    debug!(
                        session_id = %session_id,
                        config_id = %config_id,
                        "ACP: session/set_config_option applied"
                    );
                }
                Err(e) => {
                    warn!(
                        session_id = %session_id,
                        config_id = %config_id,
                        "ACP: session/set_config_option rejected: {e}"
                    );
                }
            }
        }

        for (config_id, value) in &config.required_config_options {
            let resolved_value = resolve_session_config_option_value(&sn_result, config_id, value)
                .unwrap_or_else(|| value.to_string());
            let sco_id = next_id("session-set-required-config-option");
            send_ndjson(
                &mut stdin,
                &serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": sco_id,
                    "method": "session/set_config_option",
                    "params": {
                        "sessionId": session_id,
                        "configId": config_id,
                        "value": resolved_value,
                    }
                }),
            )
            .await
            .with_context(|| format!("ACP: send required session/set_config_option {config_id}"))?;

            await_response(
                &mut reader,
                &mut child,
                &sco_id,
                HANDSHAKE_TIMEOUT,
                "required session/set_config_option handshake",
            )
            .await
            .with_context(|| {
                format!("ACP: required session/set_config_option rejected for {config_id}")
            })?;
            // SEC-ACP-001: omit value — config option values can carry model keys or tokens.
            debug!(
                session_id = %session_id,
                config_id = %config_id,
                "ACP: required session/set_config_option applied"
            );
        }

        let last_runtime_receipt = Some(startup_receipt.build(
            &req.provider,
            req.model.as_ref(),
            &session_id,
            req.session_generation_id.as_ref(),
            req.xcode_shim_injection_signal,
            req.requires_xcode_host_execution,
            "session_ready",
            None,
        ));

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
            provider: req.provider.clone(),
            model: req.model.clone(),
            permission_grant_debounce: config.permission_grant_debounce,
            xcode_shim_injected: req.xcode_shim_injection_signal,
            requires_xcode_host_execution: req.requires_xcode_host_execution,
            claude_sdk_debug_file_path: claude_sdk_debug_file_path(req)
                .map(|path| path.to_string_lossy().into_owned()),
            last_runtime_receipt,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn child_pid(&self) -> Option<u32> {
        self.child.id()
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

    pub fn runtime_receipt(&self) -> Option<&AcpRuntimeReceipt> {
        self.last_runtime_receipt.as_ref()
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
        AcpCompletionTextCaptureMetadata,
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
        self.prompt_with_optional_close_signal(req, None, Arc::new(NoopAcpPromptProgressSink))
            .await
    }

    pub async fn prompt_with_progress_sink(
        &mut self,
        req: &ExecutionRequest,
        progress_sink: Arc<dyn AcpPromptProgressSink>,
    ) -> Result<(
        AgentStatus,
        Vec<String>,
        Vec<DiscoveredArtifact>,
        Vec<PrePromptExpectedOutputMetadata>,
        Option<String>,
        AcpCompletionTextCaptureMetadata,
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
        self.prompt_with_optional_close_signal(req, None, progress_sink)
            .await
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
        AcpCompletionTextCaptureMetadata,
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
        self.prompt_with_optional_close_signal(
            req,
            Some(close_rx),
            Arc::new(NoopAcpPromptProgressSink),
        )
        .await
    }

    pub async fn prompt_with_close_signal_and_progress_sink(
        &mut self,
        req: &ExecutionRequest,
        close_rx: &mut watch::Receiver<bool>,
        progress_sink: Arc<dyn AcpPromptProgressSink>,
    ) -> Result<(
        AgentStatus,
        Vec<String>,
        Vec<DiscoveredArtifact>,
        Vec<PrePromptExpectedOutputMetadata>,
        Option<String>,
        AcpCompletionTextCaptureMetadata,
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
        self.prompt_with_optional_close_signal(req, Some(close_rx), progress_sink)
            .await
    }

    async fn prompt_with_optional_close_signal(
        &mut self,
        req: &ExecutionRequest,
        mut close_rx: Option<&mut watch::Receiver<bool>>,
        progress_sink: Arc<dyn AcpPromptProgressSink>,
    ) -> Result<(
        AgentStatus,
        Vec<String>,
        Vec<DiscoveredArtifact>,
        Vec<PrePromptExpectedOutputMetadata>,
        Option<String>,
        AcpCompletionTextCaptureMetadata,
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
        let startup_offset_ms = if req.reuse_existing_session {
            0
        } else {
            self.acp_pre_initialize_local_latency_ms
                + self.acp_initialize_latency_ms
                + self.acp_session_new_latency_ms
        };
        let runtime_started_mono = Instant::now() - Duration::from_millis(startup_offset_ms);
        let runtime_started_wall =
            chrono::Utc::now() - chrono::Duration::milliseconds(startup_offset_ms as i64);
        let mut runtime_receipt =
            RuntimeReceiptTracker::new(runtime_started_wall, runtime_started_mono);
        if is_claude_provider(&self.provider) {
            runtime_receipt.configure_claude_diagnostics(self.claude_sdk_debug_file_path.clone());
        }
        if req.reuse_existing_session {
            runtime_receipt.push_event(
                "session_reused",
                req.session_generation_id
                    .as_ref()
                    .map(|generation_id| format!("generation_id={generation_id}")),
            );
        } else {
            runtime_receipt.handshake.initialize_sent_at_ms =
                Some(self.acp_pre_initialize_local_latency_ms);
            runtime_receipt.handshake.initialize_received_at_ms =
                Some(self.acp_pre_initialize_local_latency_ms + self.acp_initialize_latency_ms);
            runtime_receipt.handshake.session_new_sent_at_ms =
                runtime_receipt.handshake.initialize_received_at_ms;
            runtime_receipt.handshake.session_new_received_at_ms = Some(
                self.acp_pre_initialize_local_latency_ms
                    + self.acp_initialize_latency_ms
                    + self.acp_session_new_latency_ms,
            );
        }
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
        let prompt_sequence = self.request_counter;
        let prompt_id = format_client_request_id("session-prompt", prompt_sequence);
        let metadata_context =
            pre_prompt_expected_output_context(req, &self.session_id, prompt_sequence);
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
        let mut claude_local_activity =
            ClaudeLocalActivityMonitor::for_request(req, &self.session_id);
        let mut codex_local_activity =
            CodexLocalActivityMonitor::for_request(req, &self.session_id);
        if let Err(error) = send_ndjson(
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
        .context("ACP: send session/prompt")
        {
            self.last_runtime_receipt = Some(runtime_receipt.build(
                &self.provider,
                self.model.as_ref(),
                &self.session_id,
                req.session_generation_id.as_ref(),
                self.xcode_shim_injected,
                self.requires_xcode_host_execution,
                "failed",
                Some("prompt_send_failed".to_string()),
            ));
            return Err(error);
        }
        runtime_receipt.note_prompt_sent(&prompt_id);
        record_prompt_progress_detail_for_session(
            req,
            &progress_sink,
            &self.session_id,
            AcpPromptProgressKind::PromptSent,
            Some("Prompt sent".to_string()),
            normalized_timeline_detail(&req.prompt),
            Some("operator_prompt".to_string()),
        )
        .await;

        let mut line = String::new();
        let mut last_acp_activity = Instant::now();
        let mut last_acp_progress = Instant::now();
        let mut last_provider_local_activity: Option<Instant> = None;
        let mut last_provider_local_progress: Option<Instant> = None;
        let mut last_prompt_progress_reported = Some(Instant::now());
        let mut streamed_text = String::new();
        let mut streamed_text_truncated = false;
        let mut completion_streamed_text = String::new();
        let mut completion_streamed_text_truncated = false;
        let mut completion_capture = CompletionTextCapture::default();
        let mut latest_usage_snapshot = None;
        let mut xcode_shim_warning_events = Vec::new();
        let mut seen_xcode_warning_keys = HashSet::new();
        let mut failure_phase: Option<String> = None;
        let mut claude_local_activity_silence_warning_recorded = false;
        let mut claude_local_activity_progress_warning_recorded = false;

        'streaming: loop {
            poll_claude_local_activity_watchdog(
                claude_local_activity.as_mut(),
                req,
                &progress_sink,
                &self.session_id,
                &mut last_provider_local_activity,
                &mut last_provider_local_progress,
                &mut last_prompt_progress_reported,
            )
            .await;
            poll_codex_local_activity_watchdog(
                codex_local_activity.as_mut(),
                req,
                &progress_sink,
                &self.session_id,
                &mut last_provider_local_activity,
                &mut last_provider_local_progress,
                &mut last_prompt_progress_reported,
            )
            .await;

            let effective_last_activity =
                max_instant_option(last_acp_activity, last_provider_local_activity);
            let idle = effective_last_activity.elapsed();
            if idle >= IDLE_TIMEOUT {
                let has_observed_local_activity = claude_local_activity
                    .as_ref()
                    .is_some_and(|monitor| monitor.has_observed_activity())
                    || codex_local_activity
                        .as_ref()
                        .is_some_and(|monitor| monitor.has_observed_activity());
                match local_activity_timeout_decision(
                    has_observed_local_activity,
                    idle,
                    claude_local_activity_silence_warning_recorded,
                ) {
                    AcpSilenceDeadlineDecision::WarnGrace => {
                        claude_local_activity_silence_warning_recorded = true;
                        let local_summary = note_provider_silence_grace_receipt_event(
                            &mut runtime_receipt,
                            claude_local_activity.as_ref(),
                            codex_local_activity.as_ref(),
                            "idle_timeout",
                            idle,
                        )
                        .unwrap_or_else(|| "provider_local_activity=unavailable".to_string());
                        warn!(
                            session_id = %self.session_id,
                            elapsed_s = idle.as_secs(),
                            grace_timeout_s = POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT.as_secs(),
                            local_summary = %local_summary,
                            "ACP stream silent after local activity; entering grace window"
                        );
                    }
                    AcpSilenceDeadlineDecision::Continue => {}
                    AcpSilenceDeadlineDecision::Timeout => {
                        if recover_claude_session_store_final_response(
                            &mut completion_capture,
                            &mut runtime_receipt,
                            claude_local_activity.as_ref(),
                        ) {
                            runtime_receipt.push_event(
                                "terminal_response_missing_session_store_final_available",
                                Some("phase=idle_timeout".to_string()),
                            );
                            break 'streaming;
                        }
                        if let Some(provider_failure) =
                            codex_local_activity.as_ref().and_then(|monitor| {
                                monitor.provider_failure_event_from_local_activity()
                            })
                        {
                            let receipt = self.provider_failure_receipt(
                                &mut runtime_receipt,
                                req,
                                &provider_failure,
                                None,
                                None,
                                None,
                            );
                            return Err(anyhow::Error::new(crate::AcpExecutionError::new(
                                provider_failure.message,
                                Some(receipt),
                            )));
                        }
                        let classification = provider_stream_silence_classification(
                            claude_local_activity.as_ref(),
                            codex_local_activity.as_ref(),
                        );
                        failure_phase = Some("idle_timeout".to_string());
                        let local_summary = provider_local_activity_summary(
                            &mut runtime_receipt,
                            claude_local_activity.as_ref(),
                            codex_local_activity.as_ref(),
                        );
                        self.diagnose_claude_watchdog_timeout(
                            req,
                            &prompt_id,
                            "idle_timeout",
                            &mut runtime_receipt,
                        )
                        .await;
                        self.last_runtime_receipt = Some(runtime_receipt.build(
                            &self.provider,
                            self.model.as_ref(),
                            &self.session_id,
                            req.session_generation_id.as_ref(),
                            self.xcode_shim_injected,
                            self.requires_xcode_host_execution,
                            "failed",
                            failure_phase.clone(),
                        ));
                        return Err(anyhow::anyhow!(
                            "ACP session idle timeout: {classification}; no message for {}s (session={}, last_acp_activity_age_s={}, last_provider_local_activity_age_s={}, {local_summary})",
                            IDLE_TIMEOUT.as_secs(),
                            self.session_id,
                            last_acp_activity.elapsed().as_secs(),
                            last_provider_local_activity
                                .map(|instant| instant.elapsed().as_secs().to_string())
                                .unwrap_or_else(|| "none".to_string())
                        ));
                    }
                }
            }
            let effective_last_progress =
                max_instant_option(last_acp_progress, last_provider_local_progress);
            let progress_idle = effective_last_progress.elapsed();
            let has_open_local_activity = claude_local_activity
                .as_ref()
                .is_some_and(|monitor| monitor.has_open_local_activity())
                || codex_local_activity
                    .as_ref()
                    .is_some_and(|monitor| monitor.has_open_local_activity());
            match local_activity_timeout_decision(
                has_open_local_activity,
                progress_idle,
                claude_local_activity_progress_warning_recorded,
            ) {
                AcpSilenceDeadlineDecision::WarnGrace => {
                    claude_local_activity_progress_warning_recorded = true;
                    let local_summary = note_provider_silence_grace_receipt_event(
                        &mut runtime_receipt,
                        claude_local_activity.as_ref(),
                        codex_local_activity.as_ref(),
                        "progress_timeout",
                        progress_idle,
                    )
                    .unwrap_or_else(|| "provider_local_activity=unavailable".to_string());
                    warn!(
                        session_id = %self.session_id,
                        elapsed_s = progress_idle.as_secs(),
                        grace_timeout_s = POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT.as_secs(),
                        local_summary = %local_summary,
                        "ACP progress stalled with open local work; entering grace window"
                    );
                }
                AcpSilenceDeadlineDecision::Continue => {}
                AcpSilenceDeadlineDecision::Timeout => {
                    if recover_claude_session_store_final_response(
                        &mut completion_capture,
                        &mut runtime_receipt,
                        claude_local_activity.as_ref(),
                    ) {
                        runtime_receipt.push_event(
                            "terminal_response_missing_session_store_final_available",
                            Some("phase=progress_timeout".to_string()),
                        );
                        break 'streaming;
                    }
                    if let Some(provider_failure) = codex_local_activity
                        .as_ref()
                        .and_then(|monitor| monitor.provider_failure_event_from_local_activity())
                    {
                        let receipt = self.provider_failure_receipt(
                            &mut runtime_receipt,
                            req,
                            &provider_failure,
                            None,
                            None,
                            None,
                        );
                        return Err(anyhow::Error::new(crate::AcpExecutionError::new(
                            provider_failure.message,
                            Some(receipt),
                        )));
                    }
                    let classification = provider_stream_silence_classification(
                        claude_local_activity.as_ref(),
                        codex_local_activity.as_ref(),
                    );
                    failure_phase = Some("progress_timeout".to_string());
                    let local_summary = provider_local_activity_summary(
                        &mut runtime_receipt,
                        claude_local_activity.as_ref(),
                        codex_local_activity.as_ref(),
                    );
                    self.diagnose_claude_watchdog_timeout(
                        req,
                        &prompt_id,
                        "progress_timeout",
                        &mut runtime_receipt,
                    )
                    .await;
                    self.last_runtime_receipt = Some(runtime_receipt.build(
                        &self.provider,
                        self.model.as_ref(),
                        &self.session_id,
                        req.session_generation_id.as_ref(),
                        self.xcode_shim_injected,
                        self.requires_xcode_host_execution,
                        "failed",
                        failure_phase.clone(),
                    ));
                    return Err(anyhow::anyhow!(
                    "ACP session progress timeout: {classification}; no meaningful progress for {}s (session={}, {local_summary})",
                    PROGRESS_TIMEOUT.as_secs(),
                    self.session_id
                ));
                }
            }
            let read_wait = if claude_local_activity.is_some() || codex_local_activity.is_some() {
                let remaining_idle = IDLE_TIMEOUT.saturating_sub(idle);
                let remaining_progress =
                    local_activity_progress_timeout_limit(has_open_local_activity)
                        .saturating_sub(progress_idle);
                remaining_idle
                    .min(remaining_progress)
                    .min(LOCAL_ACTIVITY_POLL_INTERVAL)
            } else {
                let remaining_idle = IDLE_TIMEOUT.saturating_sub(idle);
                let remaining_progress =
                    local_activity_progress_timeout_limit(has_open_local_activity)
                        .saturating_sub(progress_idle);
                remaining_idle.min(remaining_progress)
            }
            .min(PROVIDER_PROCESS_POLL_INTERVAL);

            let mut close_requested = false;
            let mut deferred_provider_failure: Option<ProviderFailureEvent> = None;
            let mut deferred_claude_final_response: Option<(String, &'static str)> = None;
            let n_result: Result<usize> = {
                line.clear();
                let session_id = self.session_id.clone();
                let read_line = read_capped_ndjson_line(
                    &mut self.reader,
                    &mut line,
                    ndjson_line_cap_bytes(&req.expected_outputs),
                    "ACP prompt stream read_line",
                );
                tokio::pin!(read_line);
                loop {
                    let sleep = tokio::time::sleep(read_wait);
                    tokio::pin!(sleep);
                    let read_outcome = match close_rx.as_deref_mut() {
                        Some(close_rx) => {
                            if *close_rx.borrow() {
                                AcpPromptReadOutcome::CloseRequested
                            } else {
                                tokio::select! {
                                    _ = close_rx.changed() => AcpPromptReadOutcome::CloseRequested,
                                    result = &mut read_line => AcpPromptReadOutcome::Read(result),
                                    _ = &mut sleep => AcpPromptReadOutcome::PollElapsed,
                                }
                            }
                        }
                        None => {
                            tokio::select! {
                                result = &mut read_line => AcpPromptReadOutcome::Read(result),
                                _ = &mut sleep => AcpPromptReadOutcome::PollElapsed,
                            }
                        }
                    };

                    if matches!(&read_outcome, AcpPromptReadOutcome::PollElapsed) {
                        match self.child.try_wait() {
                            Ok(Some(status)) => {
                                failure_phase = Some("provider_subprocess_exit".to_string());
                                break Err(anyhow::anyhow!(
                                    "ACP provider subprocess exited during active prompt: {status} (session={session_id})"
                                ));
                            }
                            Ok(None) => {}
                            Err(error) => {
                                failure_phase =
                                    Some("provider_subprocess_liveness_check_failed".to_string());
                                break Err(error).context(
                                    "ACP provider subprocess liveness check during active prompt",
                                );
                            }
                        }
                    }

                    match read_outcome {
                        AcpPromptReadOutcome::CloseRequested => {
                            close_requested = true;
                            failure_phase = Some("prompt_closed_during_stream".to_string());
                            break Err(anyhow::anyhow!(
                                "ACP session closed during active prompt (session={session_id})"
                            ));
                        }
                        AcpPromptReadOutcome::Read(Ok(read_result)) => break Ok(read_result),
                        AcpPromptReadOutcome::Read(Err(error)) => {
                            break Err(error).context("ACP prompt stream read_line error");
                        }
                        AcpPromptReadOutcome::PollElapsed
                            if claude_local_activity.is_some()
                                || codex_local_activity.is_some() =>
                        {
                            debug!(
                                session_id = %session_id,
                                "ACP prompt stream read poll elapsed; checking provider local activity"
                            );
                            poll_claude_local_activity_watchdog(
                                claude_local_activity.as_mut(),
                                req,
                                &progress_sink,
                                &session_id,
                                &mut last_provider_local_activity,
                                &mut last_provider_local_progress,
                                &mut last_prompt_progress_reported,
                            )
                            .await;
                            poll_codex_local_activity_watchdog(
                                codex_local_activity.as_mut(),
                                req,
                                &progress_sink,
                                &session_id,
                                &mut last_provider_local_activity,
                                &mut last_provider_local_progress,
                                &mut last_prompt_progress_reported,
                            )
                            .await;
                            let effective_last_activity =
                                max_instant_option(last_acp_activity, last_provider_local_activity);
                            let idle = effective_last_activity.elapsed();
                            if idle >= IDLE_TIMEOUT {
                                let has_observed_local_activity = claude_local_activity
                                    .as_ref()
                                    .is_some_and(|monitor| monitor.has_observed_activity())
                                    || codex_local_activity
                                        .as_ref()
                                        .is_some_and(|monitor| monitor.has_observed_activity());
                                match local_activity_timeout_decision(
                                    has_observed_local_activity,
                                    idle,
                                    claude_local_activity_silence_warning_recorded,
                                ) {
                                    AcpSilenceDeadlineDecision::WarnGrace => {
                                        claude_local_activity_silence_warning_recorded = true;
                                        let local_summary =
                                            note_provider_silence_grace_receipt_event(
                                                &mut runtime_receipt,
                                                claude_local_activity.as_ref(),
                                                codex_local_activity.as_ref(),
                                                "idle_timeout_inner",
                                                idle,
                                            )
                                            .unwrap_or_else(|| {
                                                "provider_local_activity=unavailable".to_string()
                                            });
                                        warn!(
                                            session_id = %session_id,
                                            elapsed_s = idle.as_secs(),
                                            grace_timeout_s = POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT.as_secs(),
                                            local_summary = %local_summary,
                                            "ACP stream silent after local activity; entering grace window (inner)"
                                        );
                                    }
                                    AcpSilenceDeadlineDecision::Continue => {}
                                    AcpSilenceDeadlineDecision::Timeout => {
                                        if let Some(final_response) =
                                            claude_local_activity.as_ref().and_then(|monitor| {
                                                monitor.latest_final_response_text()
                                            })
                                        {
                                            deferred_claude_final_response =
                                                Some((final_response, "idle_timeout_inner"));
                                            break Err(anyhow::anyhow!(
                                                "ACP terminal response missing but Claude session store final response is available (session={session_id})"
                                            ));
                                        }
                                        if let Some(provider_failure) =
                                            codex_local_activity.as_ref().and_then(|monitor| {
                                                monitor.provider_failure_event_from_local_activity()
                                            })
                                        {
                                            failure_phase =
                                                Some(provider_failure.failure_phase.to_string());
                                            deferred_provider_failure =
                                                Some(provider_failure.clone());
                                            break Err(anyhow::Error::new(
                                                crate::AcpExecutionError::new(
                                                    provider_failure.message,
                                                    None,
                                                ),
                                            ));
                                        }
                                        let classification = provider_stream_silence_classification(
                                            claude_local_activity.as_ref(),
                                            codex_local_activity.as_ref(),
                                        );
                                        failure_phase = Some("idle_timeout".to_string());
                                        let local_summary = provider_local_activity_summary(
                                            &mut runtime_receipt,
                                            claude_local_activity.as_ref(),
                                            codex_local_activity.as_ref(),
                                        );
                                        break Err(anyhow::anyhow!(
                                            "ACP session idle timeout: {classification}; no message for {}s (session={}, last_acp_activity_age_s={}, last_provider_local_activity_age_s={}, {local_summary})",
                                            IDLE_TIMEOUT.as_secs(),
                                            session_id,
                                            last_acp_activity.elapsed().as_secs(),
                                            last_provider_local_activity
                                                .map(|instant| instant
                                                    .elapsed()
                                                    .as_secs()
                                                    .to_string())
                                                .unwrap_or_else(|| "none".to_string())
                                        ));
                                    }
                                }
                            }
                            let effective_last_progress =
                                max_instant_option(last_acp_progress, last_provider_local_progress);
                            let progress_idle = effective_last_progress.elapsed();
                            let progress_timeout_limit = local_activity_progress_timeout_limit(
                                claude_local_activity
                                    .as_ref()
                                    .is_some_and(|monitor| monitor.has_open_local_activity())
                                    || codex_local_activity
                                        .as_ref()
                                        .is_some_and(|monitor| monitor.has_open_local_activity()),
                            );
                            if progress_idle >= progress_timeout_limit {
                                if let Some(final_response) = claude_local_activity
                                    .as_ref()
                                    .and_then(|monitor| monitor.latest_final_response_text())
                                {
                                    deferred_claude_final_response =
                                        Some((final_response, "progress_timeout_inner"));
                                    break Err(anyhow::anyhow!(
                                        "ACP terminal response missing but Claude session store final response is available (session={session_id})"
                                    ));
                                }
                                if let Some(provider_failure) =
                                    codex_local_activity.as_ref().and_then(|monitor| {
                                        monitor.provider_failure_event_from_local_activity()
                                    })
                                {
                                    failure_phase =
                                        Some(provider_failure.failure_phase.to_string());
                                    deferred_provider_failure = Some(provider_failure.clone());
                                    break Err(anyhow::Error::new(crate::AcpExecutionError::new(
                                        provider_failure.message,
                                        None,
                                    )));
                                }
                                let classification = provider_stream_silence_classification(
                                    claude_local_activity.as_ref(),
                                    codex_local_activity.as_ref(),
                                );
                                failure_phase = Some("progress_timeout".to_string());
                                let local_summary = provider_local_activity_summary(
                                    &mut runtime_receipt,
                                    claude_local_activity.as_ref(),
                                    codex_local_activity.as_ref(),
                                );
                                break Err(anyhow::anyhow!(
                                    "ACP session progress timeout: {classification}; no meaningful progress for {}s (session={}, {local_summary})",
                                    PROGRESS_TIMEOUT.as_secs(),
                                    session_id
                                ));
                            }
                            continue;
                        }
                        AcpPromptReadOutcome::PollElapsed => {
                            continue;
                        }
                    }
                }
            };
            if close_requested {
                let _ = self.close().await;
            }
            let n = match n_result {
                Ok(n) => n,
                Err(error) => {
                    if let Some((final_response, phase)) = deferred_claude_final_response {
                        completion_capture
                            .set_provider_session_store_final_response(&final_response);
                        runtime_receipt.push_event(
                            "provider_session_store_final_response_recovered",
                            Some(format!(
                                "captured_byte_count={}",
                                final_response.len().min(COMPLETION_CAPTURE_RAW_BYTE_LIMIT)
                            )),
                        );
                        runtime_receipt.push_event(
                            "terminal_response_missing_session_store_final_available",
                            Some(format!("phase={phase}")),
                        );
                        break 'streaming;
                    } else if let Some(provider_failure) = deferred_provider_failure {
                        let receipt = self.provider_failure_receipt(
                            &mut runtime_receipt,
                            req,
                            &provider_failure,
                            None,
                            None,
                            None,
                        );
                        return Err(anyhow::Error::new(crate::AcpExecutionError::new(
                            provider_failure.message,
                            Some(receipt),
                        )));
                    } else {
                        if matches!(
                            failure_phase.as_deref(),
                            Some("idle_timeout" | "progress_timeout")
                        ) {
                            self.diagnose_claude_watchdog_timeout(
                                req,
                                &prompt_id,
                                failure_phase.as_deref().unwrap_or("prompt_stream_failed"),
                                &mut runtime_receipt,
                            )
                            .await;
                        }
                        self.last_runtime_receipt = Some(
                            runtime_receipt.build(
                                &self.provider,
                                self.model.as_ref(),
                                &self.session_id,
                                req.session_generation_id.as_ref(),
                                self.xcode_shim_injected,
                                self.requires_xcode_host_execution,
                                "failed",
                                failure_phase
                                    .clone()
                                    .or_else(|| Some("prompt_stream_failed".to_string())),
                            ),
                        );
                    }
                    return Err(error);
                }
            };

            if n == 0 {
                self.last_runtime_receipt = Some(runtime_receipt.build(
                    &self.provider,
                    self.model.as_ref(),
                    &self.session_id,
                    req.session_generation_id.as_ref(),
                    self.xcode_shim_injected,
                    self.requires_xcode_host_execution,
                    "failed",
                    Some("stdout_closed_before_terminal_response".to_string()),
                ));
                return Err(anyhow::anyhow!(
                    "ACP stdout closed before terminal response (session={})",
                    self.session_id
                ));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let parsed: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => continue,
            };
            last_acp_activity = Instant::now();
            let receipt_message_summary = summarize_runtime_receipt_message(&parsed);
            runtime_receipt.note_incoming_message(receipt_message_summary.clone());
            runtime_receipt.note_post_grant_event(
                parsed
                    .get("method")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| "response".to_string()),
                receipt_message_summary.clone(),
            );
            // SEC-ACP-001: log summary only — raw payload can carry provider credentials/outputs.
            debug!(msg = %receipt_message_summary.as_deref().unwrap_or("<unknown>"), "ACP ← subprocess (stream)");
            if last_prompt_progress_reported
                .map(|reported_at| reported_at.elapsed() >= PROMPT_PROGRESS_REPORT_INTERVAL)
                .unwrap_or(true)
            {
                self.record_prompt_progress(
                    req,
                    &progress_sink,
                    AcpPromptProgressKind::MessageReceived,
                )
                .await;
                last_prompt_progress_reported = Some(Instant::now());
            }
            if let Some(snapshot) = extract_usage_snapshot(&parsed) {
                merge_usage_snapshot(&mut latest_usage_snapshot, snapshot);
            }

            let msg_id = parsed.get("id").and_then(normalize_jsonrpc_id);

            if let Some(method) = parsed["method"].as_str() {
                match method {
                    "_claude/sdkMessage" => {
                        // Claude delivers model output on this raw SDK channel rather than as a
                        // session/update. Count only content-bearing events as progress so a
                        // live stream cannot be cancelled by the ACP watchdog.
                        let extends_watchdog = claude_sdk_message_extends_watchdog(&parsed);
                        runtime_receipt.note_claude_sdk_message(&parsed);
                        if extends_watchdog {
                            last_acp_progress = Instant::now();
                            if last_prompt_progress_reported
                                .map(|reported_at| {
                                    reported_at.elapsed() >= PROMPT_PROGRESS_REPORT_INTERVAL
                                })
                                .unwrap_or(true)
                            {
                                record_prompt_progress_detail_for_session(
                                    req,
                                    &progress_sink,
                                    &self.session_id,
                                    AcpPromptProgressKind::MeaningfulProgress,
                                    Some("Claude SDK content streamed".to_string()),
                                    Some("source=claude_sdk".to_string()),
                                    Some("claude_sdk_stream".to_string()),
                                )
                                .await;
                                last_prompt_progress_reported = Some(Instant::now());
                            }
                        }
                        continue;
                    }
                    "session/request_permission" => {
                        if let Some(req_id) = parsed.get("id") {
                            let params = parsed.get("params").cloned().unwrap_or(Value::Null);
                            let normalized_req_id =
                                normalize_jsonrpc_id(req_id).unwrap_or_else(|| req_id.to_string());
                            // SEC-ACP-002: cap provider-controlled ID before it enters receipts/readback.
                            let capped_req_id = cap_provider_request_id(&normalized_req_id);
                            let request_summary = summarize_permission_request(req_id, &params);
                            // SEC-P079-002: do not emit raw provider-controlled title/optionIds
                            // to the progress timeline when in P079 repair mode; they may carry
                            // credentials, absolute paths, or bearer-like strings.
                            let progress_request_detail =
                                if req.p079_repair_canonical_paths.is_some() {
                                    Some(format!("p079_repair_mode:id={capped_req_id}"))
                                } else {
                                    Some(request_summary.clone())
                                };
                            record_prompt_progress_detail_for_session(
                                req,
                                &progress_sink,
                                &self.session_id,
                                AcpPromptProgressKind::MeaningfulProgress,
                                Some("Permission requested".to_string()),
                                progress_request_detail,
                                Some("permission_request".to_string()),
                            )
                            .await;
                            // P079-SEC-HIGH-001: apply repair permission posture when set.
                            // Evaluate posture and capture structured decision fields for
                            // SEC-MED-001 evidence before deciding whether to deny.
                            let p079_canonical_paths = req.p079_repair_canonical_paths.as_deref();
                            let p079_posture_violation = p079_canonical_paths
                                .map(|paths| p079_posture_denied(&params, paths))
                                .unwrap_or(false);
                            // SEC-HIGH-002: lift decision fields before recording the receipt so
                            // the path is available for both the posture decision record and the
                            // symlink check at grant time, without re-parsing params twice.
                            let (p079_tool_name, p079_norm_path) = if p079_canonical_paths.is_some()
                            {
                                p079_extract_decision_fields(&params)
                            } else {
                                (String::new(), String::new())
                            };
                            // SEC-HIGH-002: in P079 repair posture, do not persist raw permission
                            // request params in the runtime receipt; they may contain sensitive
                            // paths or credentials. Structured decision evidence is captured via
                            // note_p079_posture_decision below.
                            let p079_receipt_payload = if p079_canonical_paths.is_some() {
                                None
                            } else {
                                json_for_runtime_receipt(&params)
                            };
                            runtime_receipt.note_permission_request(
                                &capped_req_id,
                                // SEC-P079-002: in P079 repair mode use a sanitized label so
                                // provider-controlled title/optionId strings do not reach receipts.
                                if p079_canonical_paths.is_some() {
                                    Some(format!("p079_repair_mode:id={capped_req_id}"))
                                } else {
                                    Some(request_summary.clone())
                                },
                                p079_receipt_payload,
                            );
                            // SEC-MED-001: record structured decision on the roundtrip
                            // regardless of allow/deny outcome.
                            if p079_canonical_paths.is_some() {
                                let matched = p079_canonical_paths
                                    .unwrap_or(&[])
                                    .iter()
                                    .find(|p| p.as_str() == p079_norm_path.as_str())
                                    .map(|s| s.as_str());
                                let (reason, rk) = if p079_posture_violation {
                                    (
                                        "p079_posture_denied_unsafe_continuation",
                                        p079_classify_resource_kind_from_tool(&p079_tool_name),
                                    )
                                } else {
                                    ("canonical_path_allowed", "fs_write_canonical_output_path")
                                };
                                // SEC-HIGH-002: for denied requests, do not persist the raw
                                // requested path; store empty string so the field is present but
                                // contains no sensitive filesystem location.
                                let p079_path_for_evidence = if p079_posture_violation {
                                    ""
                                } else {
                                    p079_norm_path.as_str()
                                };
                                runtime_receipt.note_p079_posture_decision(
                                    &capped_req_id,
                                    &p079_tool_name,
                                    p079_path_for_evidence,
                                    matched,
                                    reason,
                                    rk,
                                );
                            }
                            // SEC-MED-001: pre-compute a sanitized label for P079 unsafe continuation
                            // events and error messages. This label excludes provider-controlled
                            // title text and option IDs that may carry credentials, tokens, or absolute
                            // paths. Only the server-derived request ID and tool-name-based resource_kind
                            // are included. Use this label (not request_summary) in events and errors.
                            let p079_safe_label = p079_sanitized_event_label(
                                &capped_req_id,
                                p079_classify_resource_kind_from_tool(&p079_tool_name),
                            );
                            if p079_posture_violation {
                                warn!(
                                    session_id = %self.session_id,
                                    req_id = %capped_req_id,
                                    resource_kind = %p079_classify_resource_kind_from_tool(&p079_tool_name),
                                    "P079 repair posture: permission denied (unsafe_continuation); terminating repair turn"
                                );
                                runtime_receipt.note_permission_grant_failed(
                                    &capped_req_id,
                                    Some("p079_posture_denied:unsafe_continuation".to_string()),
                                );
                                // Send denial before terminating.
                                let denial = build_permission_denial(req_id);
                                if let Err(e) = send_ndjson(&mut self.stdin, &denial).await {
                                    warn!(
                                        session_id = %self.session_id,
                                        "P079 posture: failed to send denial: {e}"
                                    );
                                }
                                // P079-SEC-HIGH-001: mark the receipt and return immediately so
                                // the executor can settle as rejected_invalid+unsafe_continuation
                                // without materialising any outputs from this turn.
                                runtime_receipt.p079_unsafe_continuation = true;
                                runtime_receipt.push_event(
                                    "p079_unsafe_continuation",
                                    // SEC-MED-001: use sanitized label, not provider-controlled request_summary.
                                    Some(format!("denied:{p079_safe_label}")),
                                );
                                let receipt = runtime_receipt.build(
                                    &self.provider,
                                    self.model.as_ref(),
                                    &self.session_id,
                                    req.session_generation_id.as_ref(),
                                    self.xcode_shim_injected,
                                    self.requires_xcode_host_execution,
                                    "failed",
                                    Some("p079_unsafe_continuation".to_string()),
                                );
                                self.last_runtime_receipt = Some(receipt.clone());
                                return Err(anyhow::Error::new(crate::AcpExecutionError::new(
                                    // SEC-MED-001: use sanitized label, not provider-controlled request_summary.
                                    format!(
                                        "p079_unsafe_continuation:repair_turn_terminated_by_posture:{p079_safe_label}"
                                    ),
                                    Some(receipt),
                                )));
                            } else {
                                // P079-SEC-HIGH-003: before granting, verify no parent of the
                                // canonical output path is a symlink. A symlink swap after the
                                // canonical path was frozen can redirect the provider write outside
                                // the run meta-root even when the requested path bytes match exactly.
                                // Fail-closed: treat symlink detection or stat failure as a posture
                                // violation and terminate the repair turn.
                                if !p079_norm_path.is_empty()
                                    && !p079_path_parents_have_no_symlinks(&p079_norm_path).await
                                {
                                    warn!(
                                        session_id = %self.session_id,
                                        req_id = %capped_req_id,
                                        resource_kind = %p079_classify_resource_kind_from_tool(&p079_tool_name),
                                        "P079 repair posture: symlink detected in canonical output path parents; terminating repair turn (unsafe_continuation)"
                                    );
                                    runtime_receipt.note_permission_grant_failed(
                                        &capped_req_id,
                                        Some("p079_posture_denied:symlink_escape".to_string()),
                                    );
                                    let denial = build_permission_denial(req_id);
                                    if let Err(e) = send_ndjson(&mut self.stdin, &denial).await {
                                        warn!(
                                            session_id = %self.session_id,
                                            "P079 posture symlink check: failed to send denial: {e}"
                                        );
                                    }
                                    runtime_receipt.p079_unsafe_continuation = true;
                                    runtime_receipt.push_event(
                                        "p079_unsafe_continuation",
                                        // SEC-MED-001: use sanitized label, not provider-controlled request_summary.
                                        Some(format!("symlink_escape:{p079_safe_label}")),
                                    );
                                    let receipt = runtime_receipt.build(
                                        &self.provider,
                                        self.model.as_ref(),
                                        &self.session_id,
                                        req.session_generation_id.as_ref(),
                                        self.xcode_shim_injected,
                                        self.requires_xcode_host_execution,
                                        "failed",
                                        Some("p079_unsafe_continuation".to_string()),
                                    );
                                    self.last_runtime_receipt = Some(receipt.clone());
                                    return Err(anyhow::Error::new(crate::AcpExecutionError::new(
                                        // SEC-MED-001: use sanitized label, not provider-controlled request_summary.
                                        format!(
                                            "p079_unsafe_continuation:symlink_escape_in_canonical_path_parent:{p079_safe_label}"
                                        ),
                                        Some(receipt),
                                    )));
                                }
                                // SEC-P079-002: in P079 mode log only sanitized fields.
                                if p079_canonical_paths.is_some() {
                                    debug!(
                                        session_id = %self.session_id,
                                        req_id = %capped_req_id,
                                        "ACP: auto-granting canonical-write permission in P079 repair mode"
                                    );
                                } else {
                                    debug!(
                                        session_id = %self.session_id,
                                        request = %request_summary,
                                        "ACP: auto-granting permission request"
                                    );
                                }
                                if !self.permission_grant_debounce.is_zero() {
                                    tokio::time::sleep(self.permission_grant_debounce).await;
                                }
                                // SEC-P079-001: in P079 repair mode only grant allow_once; never allow_always.
                                let grant_option = if p079_canonical_paths.is_some() {
                                    build_p079_repair_permission_grant(req_id, &params)
                                } else {
                                    build_permission_grant(req_id, &params)
                                };
                                if let Some(grant) = grant_option {
                                    // SEC-P079-001: in P079 repair mode, use a sanitized grant summary
                                    // that excludes the provider-controlled optionId to prevent
                                    // credential/token leakage through grant_summary, progress, or receipts.
                                    let grant_summary = if p079_canonical_paths.is_some() {
                                        p079_sanitized_event_label(
                                            &capped_req_id,
                                            "fs_write_canonical_output_path",
                                        )
                                    } else {
                                        summarize_permission_grant(&grant)
                                    };
                                    if let Err(e) = send_ndjson(&mut self.stdin, &grant).await {
                                        runtime_receipt.note_permission_grant_failed(
                                            &capped_req_id,
                                            Some(e.to_string()),
                                        );
                                        warn!(
                                            session_id = %self.session_id,
                                            "ACP: failed to send permission grant: {e}"
                                        );
                                    } else {
                                        // SEC-P079-001: in P079 repair mode suppress the raw grant
                                        // JSON payload to prevent provider-controlled optionId from
                                        // leaking into the runtime receipt alongside grant_summary.
                                        let grant_payload = if p079_canonical_paths.is_some() {
                                            None
                                        } else {
                                            json_for_runtime_receipt(&grant)
                                        };
                                        runtime_receipt.note_permission_grant_sent(
                                            &capped_req_id,
                                            Some(grant_summary.clone()),
                                            grant_payload,
                                        );
                                        record_prompt_progress_detail_for_session(
                                            req,
                                            &progress_sink,
                                            &self.session_id,
                                            AcpPromptProgressKind::MeaningfulProgress,
                                            Some("Permission granted".to_string()),
                                            Some(grant_summary.clone()),
                                            Some("permission_grant".to_string()),
                                        )
                                        .await;
                                        debug!(
                                            session_id = %self.session_id,
                                            grant = %grant_summary,
                                            "ACP: permission grant sent"
                                        );
                                    }
                                    last_acp_progress = Instant::now();
                                    if last_prompt_progress_reported
                                        .map(|reported_at| {
                                            reported_at.elapsed() >= PROMPT_PROGRESS_REPORT_INTERVAL
                                        })
                                        .unwrap_or(true)
                                    {
                                        self.record_prompt_progress(
                                            req,
                                            &progress_sink,
                                            AcpPromptProgressKind::MeaningfulProgress,
                                        )
                                        .await;
                                        last_prompt_progress_reported = Some(Instant::now());
                                    }
                                } else {
                                    runtime_receipt.note_permission_grant_failed(
                                        &capped_req_id,
                                        Some(format!(
                                            "id={capped_req_id};reason=no_supported_option"
                                        )),
                                    );
                                    if p079_canonical_paths.is_some() {
                                        // SEC-P079-LOW-002: send explicit denial and terminate repair
                                        // turn immediately — no allow_once means no safe grant path.
                                        warn!(
                                            session_id = %self.session_id,
                                            req_id = %capped_req_id,
                                            "ACP: P079 repair mode: no allow_once option; terminating repair turn (unsafe_continuation)"
                                        );
                                        let denial = build_permission_denial(req_id);
                                        if let Err(e) = send_ndjson(&mut self.stdin, &denial).await
                                        {
                                            warn!(
                                                session_id = %self.session_id,
                                                "P079 no_allow_once: failed to send denial: {e}"
                                            );
                                        }
                                        runtime_receipt.p079_unsafe_continuation = true;
                                        runtime_receipt.push_event(
                                            "p079_unsafe_continuation",
                                            Some(format!("no_allow_once:{p079_safe_label}")),
                                        );
                                        let receipt = runtime_receipt.build(
                                            &self.provider,
                                            self.model.as_ref(),
                                            &self.session_id,
                                            req.session_generation_id.as_ref(),
                                            self.xcode_shim_injected,
                                            self.requires_xcode_host_execution,
                                            "failed",
                                            Some("p079_unsafe_continuation".to_string()),
                                        );
                                        self.last_runtime_receipt = Some(receipt.clone());
                                        return Err(anyhow::Error::new(
                                            crate::AcpExecutionError::new(
                                                format!(
                                                    "p079_unsafe_continuation:no_allow_once_option:{p079_safe_label}"
                                                ),
                                                Some(receipt),
                                            ),
                                        ));
                                    } else {
                                        warn!(
                                            session_id = %self.session_id,
                                            request = %request_summary,
                                            "ACP: permission request had no supported auto-grant option"
                                        );
                                    }
                                }
                            } // P079-SEC-HIGH-001: close non-posture-violation else branch
                        }
                        continue;
                    }
                    "session/update" => {
                        debug!(session_id = %self.session_id, "ACP: session/update notification");
                        let (update_kind, meaningful_progress, detail) =
                            session_update_observation(&parsed);
                        if meaningful_progress {
                            if session_update_refreshes_progress_deadline(
                                update_kind,
                                meaningful_progress,
                            ) {
                                last_acp_progress = Instant::now();
                            }
                            record_prompt_progress_detail_for_session(
                                req,
                                &progress_sink,
                                &self.session_id,
                                AcpPromptProgressKind::MeaningfulProgress,
                                Some(timeline_title_for_update(update_kind).to_string()),
                                timeline_detail_for_update(update_kind, &parsed, detail.as_deref()),
                                Some(update_kind.to_string()),
                            )
                            .await;
                        }
                        runtime_receipt.note_session_update(
                            update_kind,
                            meaningful_progress,
                            detail,
                        );
                        if let Some(provider_failure) =
                            classify_provider_failure_event(&parsed, &self.provider)
                        {
                            runtime_receipt.push_event(
                                "provider_failure",
                                Some(provider_failure.detail.clone()),
                            );
                            runtime_receipt.note_terminal_response("failed");
                            let receipt = runtime_receipt.build(
                                &self.provider,
                                self.model.as_ref(),
                                &self.session_id,
                                req.session_generation_id.as_ref(),
                                self.xcode_shim_injected,
                                self.requires_xcode_host_execution,
                                "failed",
                                Some(provider_failure.failure_phase.to_string()),
                            );
                            self.last_runtime_receipt = Some(receipt.clone());
                            warn!(
                                session_id = %self.session_id,
                                provider = %self.provider,
                                error = %provider_failure.message,
                                "ACP provider reported terminal failure"
                            );
                            return Err(anyhow::Error::new(crate::AcpExecutionError::new(
                                provider_failure.message,
                                Some(receipt),
                            )));
                        }
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
                                last_acp_progress = Instant::now();
                                if last_prompt_progress_reported
                                    .map(|reported_at| {
                                        reported_at.elapsed() >= PROMPT_PROGRESS_REPORT_INTERVAL
                                    })
                                    .unwrap_or(true)
                                {
                                    self.record_prompt_progress(
                                        req,
                                        &progress_sink,
                                        AcpPromptProgressKind::MeaningfulProgress,
                                    )
                                    .await;
                                    last_prompt_progress_reported = Some(Instant::now());
                                }
                            }
                            push_streamed_transcript_chunk(
                                &mut streamed_text,
                                &chunk,
                                &mut streamed_text_truncated,
                            );
                        }
                        if let Some(chunk) = extract_agent_message_chunk(&parsed) {
                            push_streamed_transcript_chunk(
                                &mut completion_streamed_text,
                                &chunk,
                                &mut completion_streamed_text_truncated,
                            );
                            completion_capture.push_streamed_update(&chunk);
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
                        let jsonrpc_error_code = parsed["error"]["code"].as_i64();
                        let provider_failure = codex_local_activity
                            .as_mut()
                            .and_then(|monitor| monitor.quota_failure_event_from_session_store())
                            .unwrap_or_else(|| {
                                classify_prompt_error_response(
                                    &self.provider,
                                    jsonrpc_error_code,
                                    err_msg,
                                )
                            });
                        let _receipt = self.provider_failure_receipt(
                            &mut runtime_receipt,
                            req,
                            &provider_failure,
                            Some("failed"),
                            jsonrpc_error_code,
                            Some(if provider_failure.failure_phase == "provider_quota" {
                                provider_failure.message.as_str()
                            } else {
                                err_msg
                            }),
                        );
                        warn!(
                            session_id = %self.session_id,
                            failure_phase = provider_failure.failure_phase,
                            "ACP session/prompt returned error: {err_msg}"
                        );
                        let transcript_error = if provider_failure.failure_phase == "provider_quota"
                        {
                            provider_failure.message.as_str()
                        } else {
                            err_msg
                        };
                        return Ok((
                            AgentStatus::Failed,
                            vec![],
                            vec![],
                            pre_prompt_expected_outputs,
                            transcript_with_prompt_error(streamed_text, transcript_error),
                            completion_capture
                                .select_extraction_input_with_capped_stream(None, true)
                                .metadata,
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
                        completion_capture.set_terminal_final_response(&chunk);
                        record_prompt_progress_detail_for_session(
                            req,
                            &progress_sink,
                            &self.session_id,
                            AcpPromptProgressKind::MeaningfulProgress,
                            Some("Final response".to_string()),
                            bounded_timeline_detail(&chunk),
                            Some("final_response".to_string()),
                        )
                        .await;
                        push_streamed_transcript_chunk(
                            &mut streamed_text,
                            &chunk,
                            &mut streamed_text_truncated,
                        );
                    }
                    runtime_receipt.note_terminal_response("completed");
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
        self.last_runtime_receipt = Some(runtime_receipt.build(
            &self.provider,
            self.model.as_ref(),
            &self.session_id,
            req.session_generation_id.as_ref(),
            self.xcode_shim_injected,
            self.requires_xcode_host_execution,
            "completed",
            None,
        ));

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

        let completion_selection = completion_capture.select_extraction_input_with_capped_stream(
            non_empty_transcript(completion_streamed_text.clone()).as_deref(),
            completion_streamed_text_truncated,
        );
        let mut discovered_artifacts = completion_selection
            .text
            .as_deref()
            .map(|text| extract_output_envelopes(text, &req.expected_outputs))
            .unwrap_or_default();
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
            completion_selection.metadata,
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
        let close_id = format_client_request_id("session-close", self.request_counter);
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
        _completion_text_capture,
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
    use std::io::Write;

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
        assert_eq!(
            handshake_timeout_for_provider("gemini_acp"),
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
    fn client_request_ids_are_namespaced_strings() {
        let initialize_id = format_client_request_id("initialize", 1);
        let prompt_id = format_client_request_id("session-prompt", 3);

        assert_eq!(initialize_id, "chainworks-initialize-1");
        assert_eq!(prompt_id, "chainworks-session-prompt-3");
        assert!(
            initialize_id.parse::<u64>().is_err(),
            "client request ids must not reuse the small numeric namespace that providers may also emit"
        );
    }

    #[test]
    fn normalize_jsonrpc_id_accepts_numbers_and_strings() {
        assert_eq!(
            normalize_jsonrpc_id(&serde_json::json!(2)),
            Some("2".to_string())
        );
        assert_eq!(
            normalize_jsonrpc_id(&serde_json::json!("chainworks-session-new-2")),
            Some("chainworks-session-new-2".to_string())
        );
        assert_eq!(normalize_jsonrpc_id(&serde_json::json!(null)), None);
    }

    #[test]
    fn permission_summaries_normalize_jsonrpc_ids() {
        let params = serde_json::json!({
            "toolCall": {
                "title": "Open file"
            },
            "options": [
                {"kind": "allow_once", "optionId": "allow_once"}
            ]
        });
        let grant = build_permission_grant(&serde_json::json!(2), &params).expect("grant");

        assert_eq!(
            summarize_permission_request(&serde_json::json!(2), &params),
            "id=2;title=Open file;options=allow_once"
        );
        assert_eq!(
            summarize_permission_grant(&grant),
            "id=2;selected=allow_once"
        );
    }

    #[test]
    fn permission_grant_preserves_provider_request_id_shape() {
        let params = serde_json::json!({
            "options": [
                {"kind": "allow_once", "optionId": "allow_once"}
            ]
        });

        let numeric_grant =
            build_permission_grant(&serde_json::json!(2), &params).expect("grant should exist");
        assert_eq!(numeric_grant["id"], serde_json::json!(2));

        let string_grant =
            build_permission_grant(&serde_json::json!("provider-request-2"), &params)
                .expect("grant should exist");
        assert_eq!(string_grant["id"], serde_json::json!("provider-request-2"));
    }

    #[test]
    fn permission_grant_keeps_allow_once_before_non_read_only_allow_always() {
        let params = serde_json::json!({
            "options": [
                {"kind": "allow_once", "name": "Yes", "optionId": "Yes"},
                {"kind": "reject_once", "name": "No", "optionId": "No"},
                {
                    "kind": "allow_always",
                    "name": "Always allow (\"/workspace/implementation/plan.md\")",
                    "optionId": "Always allow (\"/workspace/implementation/plan.md\")"
                },
                {
                    "kind": "allow_always",
                    "name": "Always allow (\"/workspace/implementation/*\")",
                    "optionId": "Always allow (\"/workspace/implementation/*\")"
                }
            ]
        });

        let grant =
            build_permission_grant(&serde_json::json!(2), &params).expect("grant should exist");

        assert_eq!(grant["id"], serde_json::json!(2));
        assert_eq!(
            grant["result"]["outcome"]["optionId"],
            serde_json::json!("Yes")
        );
    }

    #[test]
    fn permission_grant_prefers_read_only_allow_always_to_avoid_repeated_junie_roundtrips() {
        let params = serde_json::json!({
            "options": [
                {"kind": "allow_once", "name": "Yes", "optionId": "Yes"},
                {"kind": "reject_once", "name": "No", "optionId": "No"},
                {
                    "kind": "allow_always",
                    "name": "Always allow all read-only commands (ls, cat, grep, etc.)",
                    "optionId": "Always allow all read-only commands (ls, cat, grep, etc.)"
                }
            ]
        });

        let grant =
            build_permission_grant(&serde_json::json!(2), &params).expect("grant should exist");

        assert_eq!(grant["id"], serde_json::json!(2));
        assert_eq!(
            grant["result"]["outcome"]["optionId"],
            serde_json::json!("Always allow all read-only commands (ls, cat, grep, etc.)")
        );
    }

    #[test]
    fn runtime_receipt_captures_permission_payload_and_post_grant_outcome() {
        let started_wall = chrono::Utc::now();
        let started_mono = Instant::now();
        let mut tracker = RuntimeReceiptTracker::new(started_wall, started_mono);
        let params = serde_json::json!({
            "toolCall": {
                "title": "Open file",
                "kind": "terminal"
            },
            "options": [
                {"kind": "allow_once", "optionId": "allow_once"}
            ]
        });
        let grant = build_permission_grant(&serde_json::json!(2), &params).expect("grant");
        let request_id = "2";

        tracker.note_permission_request(
            request_id,
            Some(summarize_permission_request(&serde_json::json!(2), &params)),
            json_for_runtime_receipt(&params),
        );
        tracker.note_permission_grant_sent(
            request_id,
            Some(summarize_permission_grant(&grant)),
            json_for_runtime_receipt(&grant),
        );
        tracker.note_post_grant_event(
            "session/update:tool_call",
            Some("method=session/update".to_string()),
        );

        let receipt = tracker.build(
            "junie",
            None,
            "provider-session-1",
            None,
            false,
            false,
            "failed",
            Some("progress_timeout".to_string()),
        );

        let roundtrip = receipt
            .permission_roundtrips
            .into_iter()
            .next()
            .expect("permission roundtrip");
        assert_eq!(roundtrip.request_id, "2");
        assert_eq!(
            roundtrip.request_summary.as_deref(),
            Some("id=2;title=Open file;options=allow_once")
        );
        assert!(roundtrip
            .request_payload
            .as_deref()
            .is_some_and(|payload| payload.contains("\"title\":\"Open file\"")));
        assert!(roundtrip
            .grant_payload
            .as_deref()
            .is_some_and(|payload| payload.contains("\"optionId\":\"allow_once\"")));
        assert_eq!(
            roundtrip.first_post_grant_event_kind.as_deref(),
            Some("session/update:tool_call")
        );
        assert_eq!(
            roundtrip.first_post_grant_event_detail.as_deref(),
            Some("method=session/update")
        );
        assert_eq!(
            roundtrip.outcome.as_deref(),
            Some("post_grant_activity_observed")
        );
    }

    #[test]
    fn runtime_receipt_records_claude_local_activity_summary() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let transcript_path = tempdir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&transcript_path).expect("transcript");
        let mut monitor = ClaudeLocalActivityMonitor::new_for_path(transcript_path);
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"Command running in background with ID: bttluot5u. Output is being written to: /tmp/tasks/bttluot5u.output","is_error":false}}]}},"toolUseResult":{{"backgroundTaskId":"bttluot5u"}}}}"#
        )
        .expect("write background task result");
        file.flush().expect("flush background task");
        monitor.poll(Instant::now()).expect("poll");

        let started_wall = chrono::Utc::now();
        let started_mono = Instant::now();
        let mut tracker = RuntimeReceiptTracker::new(started_wall, started_mono);
        note_claude_local_activity_receipt_event(&mut tracker, Some(&monitor));

        let receipt = tracker.build(
            "claude",
            None,
            "provider-session-1",
            None,
            false,
            false,
            "failed",
            Some("idle_timeout".to_string()),
        );

        assert!(receipt.last_events.iter().any(|event| {
            event.kind == "provider_local_activity_summary"
                && event
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("open_background_tasks=1"))
        }));
    }

    #[test]
    fn runtime_receipt_persists_sanitized_claude_sdk_boundary_events() {
        let started_wall = chrono::Utc::now();
        let started_mono = Instant::now();
        let mut tracker = RuntimeReceiptTracker::new(started_wall, started_mono);
        tracker
            .configure_claude_diagnostics(Some("/tmp/claude-sdk-debug/agent-exec.log".to_string()));

        for message in [
            serde_json::json!({
                "method": "_claude/sdkMessage",
                "params": {"message": {
                    "type": "stream_event",
                    "event": {
                        "type": "content_block_start",
                        "index": 3,
                        "content_block": {
                            "type": "tool_use",
                            "id": "toolu_14",
                            "name": "Write",
                            "input": {"file_path": "/secret/path", "content": "secret-body"}
                        }
                    }
                }}
            }),
            serde_json::json!({
                "method": "_claude/sdkMessage",
                "params": {"message": {
                    "type": "assistant",
                    "uuid": "assistant-uuid",
                    "message": {
                        "id": "msg_14",
                        "stop_reason": "tool_use",
                        "content": [{
                            "type": "tool_use",
                            "id": "toolu_14",
                            "name": "Write",
                            "input": {"content": "secret-body"}
                        }]
                    }
                }}
            }),
            serde_json::json!({
                "method": "_claude/sdkMessage",
                "params": {"message": {"type": "result", "subtype": "success"}}
            }),
            serde_json::json!({
                "method": "_claude/sdkMessage",
                "params": {"message": {
                    "type": "system",
                    "subtype": "session_state_changed",
                    "state": "idle"
                }}
            }),
        ] {
            assert!(tracker.note_claude_sdk_message(&message));
        }

        let receipt = tracker.build(
            "claude",
            None,
            "provider-session-1",
            None,
            false,
            false,
            "failed",
            Some("progress_timeout".to_string()),
        );
        let diagnostics = receipt.claude_diagnostics.expect("Claude diagnostics");

        assert_eq!(diagnostics.raw_sdk_message_count, 4);
        assert_eq!(diagnostics.stream_event_count, 1);
        assert_eq!(diagnostics.assistant_count, 1);
        assert_eq!(diagnostics.result_count, 1);
        assert_eq!(diagnostics.session_state_changed_count, 1);
        assert!(diagnostics.result_seen);
        assert!(diagnostics.idle_seen);
        assert_eq!(
            diagnostics.last_stream_event_type.as_deref(),
            Some("content_block_start")
        );
        assert_eq!(
            diagnostics.debug_file_path.as_deref(),
            Some("/tmp/claude-sdk-debug/agent-exec.log")
        );
        let serialized = serde_json::to_string(&diagnostics).expect("serialize diagnostics");
        assert!(serialized.contains("tool_name=Write"));
        assert!(serialized.contains("tool_use_id=toolu_14"));
        assert!(!serialized.contains("secret-body"));
        assert!(!serialized.contains("/secret/path"));
    }

    #[test]
    fn claude_sdk_content_stream_extends_watchdog_but_usage_does_not() {
        let content_delta = serde_json::json!({
            "method": "_claude/sdkMessage",
            "params": {"message": {
                "type": "stream_event",
                "event": {
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "text_delta", "text": "private output"}
                }
            }}
        });
        let usage_update = serde_json::json!({
            "method": "_claude/sdkMessage",
            "params": {"message": {
                "type": "stream_event",
                "event": {"type": "message_delta", "usage": {"output_tokens": 1}}
            }}
        });
        let assistant_message = serde_json::json!({
            "method": "_claude/sdkMessage",
            "params": {"message": {
                "type": "assistant",
                "message": {"content": [{"type": "text", "text": "private output"}]}
            }}
        });

        assert!(claude_sdk_message_extends_watchdog(&content_delta));
        assert!(claude_sdk_message_extends_watchdog(&assistant_message));
        assert!(!claude_sdk_message_extends_watchdog(&usage_update));
    }

    #[test]
    fn claude_watchdog_cancel_receipt_distinguishes_terminal_flush() {
        assert_eq!(
            session_cancel_notification("provider-session-1"),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "session/cancel",
                "params": {"sessionId": "provider-session-1"}
            })
        );

        let started_wall = chrono::Utc::now();
        let started_mono = Instant::now();
        let mut tracker = RuntimeReceiptTracker::new(started_wall, started_mono);
        tracker.configure_claude_diagnostics(None);
        tracker.note_claude_watchdog_cancel_sent(true);
        tracker.note_claude_cancel_drain_message();
        tracker.note_claude_cancel_flush_observed("cancelled");

        let receipt = tracker.build(
            "claude",
            None,
            "provider-session-1",
            None,
            false,
            false,
            "failed",
            Some("progress_timeout".to_string()),
        );
        let diagnostics = receipt.claude_diagnostics.expect("Claude diagnostics");
        assert!(diagnostics.cancel_sent_on_watchdog);
        assert!(diagnostics.cancel_send_succeeded);
        assert!(diagnostics.cancel_flush_observed);
        assert_eq!(diagnostics.cancel_drain_message_count, 1);
        assert_eq!(
            diagnostics.cancel_terminal_status.as_deref(),
            Some("cancelled")
        );
    }

    #[tokio::test]
    async fn claude_watchdog_cancel_drains_raw_boundary_and_terminal_response() {
        let (client, server) = tokio::io::duplex(16 * 1024);
        let (client_read, mut client_write) = tokio::io::split(client);
        let (server_read, mut server_write) = tokio::io::split(server);
        let mut reader = BufReader::new(client_read);
        let mut server_reader = BufReader::new(server_read);

        let server_task = tokio::spawn(async move {
            let mut cancel_line = String::new();
            server_reader
                .read_line(&mut cancel_line)
                .await
                .expect("read cancel");
            assert_eq!(
                serde_json::from_str::<Value>(cancel_line.trim()).expect("cancel json"),
                session_cancel_notification("provider-session-1")
            );
            for message in [
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "_claude/sdkMessage",
                    "params": {"message": {"type": "result", "subtype": "success"}}
                }),
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "_claude/sdkMessage",
                    "params": {"message": {
                        "type": "system",
                        "subtype": "session_state_changed",
                        "state": "idle"
                    }}
                }),
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": "chainworks-session-prompt-7",
                    "result": {"stopReason": "cancelled"}
                }),
            ] {
                server_write
                    .write_all(format!("{}\n", message).as_bytes())
                    .await
                    .expect("write diagnostic response");
            }
        });

        let mut tracker = RuntimeReceiptTracker::new(chrono::Utc::now(), Instant::now());
        tracker.configure_claude_diagnostics(None);
        cancel_and_drain_claude_watchdog(
            &mut reader,
            &mut client_write,
            "provider-session-1",
            "chainworks-session-prompt-7",
            16 * 1024,
            Duration::from_secs(1),
            &mut tracker,
        )
        .await
        .expect("cancel drain");
        server_task.await.expect("server task");

        let diagnostics = tracker
            .claude_diagnostics
            .expect("Claude diagnostics after cancel drain");
        assert!(diagnostics.cancel_send_succeeded);
        assert!(diagnostics.result_seen);
        assert!(diagnostics.idle_seen);
        assert!(diagnostics.cancel_flush_observed);
        assert_eq!(diagnostics.cancel_drain_message_count, 3);
        assert_eq!(
            diagnostics.cancel_terminal_status.as_deref(),
            Some("cancelled")
        );
    }

    #[test]
    fn runtime_receipt_marks_permission_timeout_without_post_grant_event() {
        let started_wall = chrono::Utc::now();
        let started_mono = Instant::now();
        let mut tracker = RuntimeReceiptTracker::new(started_wall, started_mono);
        let params = serde_json::json!({
            "toolCall": {
                "title": "Open file"
            },
            "options": [
                {"kind": "allow_once", "optionId": "allow_once"}
            ]
        });
        let grant = build_permission_grant(&serde_json::json!(2), &params).expect("grant");
        tracker.note_permission_request(
            "2",
            Some(summarize_permission_request(&serde_json::json!(2), &params)),
            json_for_runtime_receipt(&params),
        );
        tracker.note_permission_grant_sent(
            "2",
            Some(summarize_permission_grant(&grant)),
            json_for_runtime_receipt(&grant),
        );

        let receipt = tracker.build(
            "junie",
            None,
            "provider-session-1",
            None,
            false,
            false,
            "failed",
            Some("idle_timeout".to_string()),
        );

        let roundtrip = receipt
            .permission_roundtrips
            .into_iter()
            .next()
            .expect("permission roundtrip");
        assert_eq!(
            roundtrip.outcome.as_deref(),
            Some("timed_out_without_post_grant_event")
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
    fn claude_local_activity_detects_open_tool_use_as_progress() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let transcript_path = tempdir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&transcript_path).expect("transcript");
        let mut monitor = ClaudeLocalActivityMonitor::new_for_path(transcript_path);
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","id":"toolu_1","name":"Bash","input":{{"command":"./scripts/test-gate.sh proposal-066"}}}}]}}}}"#
        )
        .expect("write transcript");
        file.flush().expect("flush transcript");

        let observation = monitor.poll(Instant::now()).expect("poll");

        assert!(observation.should_extend_watchdog);
        assert_eq!(monitor.summary().event_count, 1);
        assert_eq!(monitor.summary().tool_uses, 1);
        assert_eq!(monitor.open_tool_use_count(), 1);

        let observation = monitor.poll(Instant::now()).expect("poll");
        assert!(
            observation.should_extend_watchdog,
            "an unresolved Claude tool_use keeps the provider locally active while ACP is quiet"
        );
    }

    #[test]
    fn claude_local_activity_clears_open_tool_use_on_tool_result() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let transcript_path = tempdir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&transcript_path).expect("transcript");
        let mut monitor = ClaudeLocalActivityMonitor::new_for_path(transcript_path.clone());
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","id":"toolu_1","name":"Bash","input":{{"command":"./scripts/test-gate.sh proposal-066"}}}}]}}}}"#
        )
        .expect("write tool_use");
        file.flush().expect("flush tool_use");

        assert!(
            monitor
                .poll(Instant::now())
                .expect("poll")
                .should_extend_watchdog
        );

        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"done"}}]}}}}"#
        )
        .expect("write tool_result");
        file.flush().expect("flush transcript");

        let observation = monitor.poll(Instant::now()).expect("poll");

        assert!(observation.should_extend_watchdog);
        assert_eq!(monitor.summary().tool_results, 1);
        assert_eq!(monitor.open_tool_use_count(), 0);

        let observation = monitor.poll(Instant::now()).expect("poll");
        assert!(
            !observation.should_extend_watchdog,
            "after tool_result and no new JSONL entries, Claude local activity no longer masks ACP silence"
        );
    }

    #[test]
    fn claude_local_activity_tracks_background_task_after_tool_result() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let transcript_path = tempdir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&transcript_path).expect("transcript");
        let mut monitor = ClaudeLocalActivityMonitor::new_for_path(transcript_path.clone());
        writeln!(
            file,
            r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","id":"toolu_1","name":"Bash","input":{{"command":"./scripts/test-gate.sh proposal-086"}}}}]}}}}"#
        )
        .expect("write tool_use");
        file.flush().expect("flush tool_use");

        assert!(
            monitor
                .poll(Instant::now())
                .expect("poll")
                .should_extend_watchdog
        );

        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"Command running in background with ID: bttluot5u. Output is being written to: /tmp/tasks/bttluot5u.output","is_error":false}}]}},"toolUseResult":{{"stdout":"","stderr":"","interrupted":false,"backgroundTaskId":"bttluot5u","assistantAutoBackgrounded":false}}}}"#
        )
        .expect("write background task result");
        file.flush().expect("flush transcript");

        let observation = monitor.poll(Instant::now()).expect("poll");

        assert!(observation.should_extend_watchdog);
        assert_eq!(monitor.open_tool_use_count(), 0);
        assert!(
            monitor
                .summary_for_error()
                .contains("open_background_tasks=1"),
            "summary should expose the open Claude background task"
        );

        let observation = monitor.poll(Instant::now()).expect("poll");
        assert!(
            observation.should_extend_watchdog,
            "an unresolved Claude background task keeps the provider locally active while ACP is quiet"
        );
    }

    #[test]
    fn claude_local_activity_clears_background_task_on_terminal_status() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let transcript_path = tempdir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&transcript_path).expect("transcript");
        let mut monitor = ClaudeLocalActivityMonitor::new_for_path(transcript_path.clone());
        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"toolu_1","content":"Command running in background with ID: bttluot5u. Output is being written to: /tmp/tasks/bttluot5u.output","is_error":false}}]}},"toolUseResult":{{"backgroundTaskId":"bttluot5u"}}}}"#
        )
        .expect("write background task result");
        file.flush().expect("flush background task");

        assert!(
            monitor
                .poll(Instant::now())
                .expect("poll")
                .should_extend_watchdog
        );
        assert!(monitor
            .summary_for_error()
            .contains("open_background_tasks=1"));

        writeln!(
            file,
            r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","content":"TaskOutput bttluot5u completed","is_error":false}}]}},"toolUseResult":{{"backgroundTaskId":"bttluot5u","status":"completed"}}}}"#
        )
        .expect("write terminal background task result");
        file.flush().expect("flush terminal task");

        let observation = monitor.poll(Instant::now()).expect("poll");

        assert!(observation.should_extend_watchdog);
        assert!(monitor
            .summary_for_error()
            .contains("open_background_tasks=0"));

        let observation = monitor.poll(Instant::now()).expect("poll");
        assert!(
            !observation.should_extend_watchdog,
            "after terminal TaskOutput and no new JSONL entries, Claude local activity no longer masks ACP silence"
        );
    }

    #[test]
    fn claude_local_activity_missing_or_still_jsonl_does_not_extend_watchdog() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let transcript_path = tempdir.path().join("missing-session.jsonl");
        let mut monitor = ClaudeLocalActivityMonitor::new_for_path(transcript_path);

        let observation = monitor.poll(Instant::now()).expect("poll");

        assert!(!observation.should_extend_watchdog);
        assert_eq!(monitor.summary().event_count, 0);
        assert_eq!(monitor.open_tool_use_count(), 0);
    }

    #[test]
    fn open_provider_local_work_uses_bounded_progress_grace() {
        assert_eq!(
            local_activity_progress_timeout_limit(false),
            PROGRESS_TIMEOUT,
            "ordinary provider progress still uses the five-minute deadline"
        );
        assert_eq!(
            local_activity_progress_timeout_limit(true),
            POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT,
            "unresolved provider-local work receives the bounded grace window"
        );
        assert_eq!(
            local_activity_timeout_decision(true, PROGRESS_TIMEOUT, false,),
            AcpSilenceDeadlineDecision::WarnGrace
        );
        assert_eq!(
            local_activity_timeout_decision(true, POST_LOCAL_ACTIVITY_SILENCE_GRACE_TIMEOUT, true,),
            AcpSilenceDeadlineDecision::Timeout
        );
    }

    #[test]
    fn codex_session_store_credits_exhausted_becomes_provider_quota() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let session_path = tempdir.path().join("codex-session.jsonl");
        std::fs::write(
            &session_path,
            serde_json::json!({
                "type": "event_msg",
                "payload": {
                    "type": "token_count",
                    "rate_limits": {
                        "limit_id": "gpt-5.6-usage",
                        "credits": {
                            "has_credits": false,
                            "balance": "0",
                            "unlimited": false
                        }
                    }
                }
            })
            .to_string()
                + "\n",
        )
        .expect("write codex session store");

        let failure =
            codex_session_store_credits_exhausted_failure(&session_path).expect("quota failure");

        assert_eq!(failure.failure_phase, "provider_quota");
        assert!(failure.message.contains("Codex credits exhausted"));
        assert!(failure.detail.contains("codex_credits_exhausted"));
        assert!(failure.detail.contains("limit_id=gpt-5.6-usage"));
    }

    #[test]
    fn codex_monitor_uses_explicit_runtime_home_before_bounded_workspace_scan() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let workspace_root = tempdir.path().join("workspace");
        let historical_root = workspace_root.join(".forge-codex-acp");
        for index in 0..513 {
            std::fs::create_dir_all(historical_root.join(format!("historical-{index}")))
                .expect("create historical runtime home");
        }

        let runtime_home = tempdir.path().join("runtime-home");
        let session_path = runtime_home.join("sessions/codex-session-1.jsonl");
        std::fs::create_dir_all(session_path.parent().expect("session parent"))
            .expect("create exact runtime home");
        std::fs::write(&session_path, "{}\n").expect("write exact session store");

        let req = ExecutionRequest {
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage-1".to_string(),
            attempt_number: 1,
            agent_execution_id: None,
            agent_id: "agent-1".to_string(),
            provider: "codex".to_string(),
            model: None,
            effort: None,
            workspace_root: workspace_root.to_string_lossy().into_owned(),
            prompt: "test".to_string(),
            worktree_root: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            provider_runtime_home: Some(runtime_home.to_string_lossy().into_owned()),
            p079_repair_canonical_paths: None,
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

        let mut monitor =
            CodexLocalActivityMonitor::for_request(&req, "session-1").expect("Codex monitor");
        assert_eq!(monitor.candidate_roots.first(), Some(&runtime_home));

        monitor.ensure_session_path();
        assert_eq!(
            monitor.session_path.as_deref(),
            Some(session_path.as_path())
        );
    }

    #[test]
    fn codex_local_activity_unbounded_output_becomes_tool_session_failure() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let session_path = tempdir.path().join("codex-session.jsonl");
        let mut monitor = CodexLocalActivityMonitor::new_for_path(session_path.clone());
        let output = "Original token count: 2479089\nTotal output lines: 5097";
        std::fs::write(
            &session_path,
            serde_json::json!({
                "type": "response_item",
                "payload": {
                    "type": "function_call_output",
                    "call_id": "call-1",
                    "output": output
                }
            })
            .to_string()
                + "\n",
        )
        .expect("write codex session store");

        monitor.poll(Instant::now()).expect("poll codex store");
        let failure = monitor
            .provider_failure_event_from_local_activity()
            .expect("tool-output pathology failure");

        assert_eq!(failure.failure_phase, "codex_tool_session_control_failure");
        assert!(failure.message.contains("codex_unbounded_tool_output"));
        assert!(failure.detail.contains("max_original_token_count=2479089"));
        assert!(failure.detail.contains("max_total_output_lines=5097"));
    }

    #[test]
    fn codex_wait_agent_keeps_watchdog_open_until_its_result_arrives() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let session_path = tempdir.path().join("codex-session.jsonl");
        let mut monitor = CodexLocalActivityMonitor::new_for_path(session_path.clone());
        let wait_call = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "function_call",
                "name": "wait_agent",
                "call_id": "wait-1"
            }
        });
        std::fs::write(&session_path, format!("{wait_call}\n"))
            .expect("write Codex wait-agent call");

        let observation = monitor.poll(Instant::now()).expect("poll wait-agent call");
        assert!(observation.should_extend_watchdog);
        assert!(observation.has_open_local_activity);
        assert!(monitor
            .summary_for_error()
            .contains("open_background_agent_waits=1"));

        let wait_result = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "function_call_output",
                "call_id": "wait-1",
                "output": "reviewer completed"
            }
        });
        std::fs::write(&session_path, format!("{wait_call}\n{wait_result}\n"))
            .expect("write Codex wait-agent result");

        let observation = monitor
            .poll(Instant::now())
            .expect("poll wait-agent result");
        assert!(observation.should_extend_watchdog);
        assert!(!observation.has_open_local_activity);
        assert!(monitor
            .summary_for_error()
            .contains("background_agent_waits_finished=1"));

        let observation = monitor
            .poll(Instant::now())
            .expect("poll settled wait-agent result");
        assert!(!observation.should_extend_watchdog);
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
            model: Some("gpt-5.6".to_string()),
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
            provider_runtime_home: None,
            p079_repair_canonical_paths: None,
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
            toolchain_home: None,
            toolchain_go_scope_enabled: false,
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
    fn proposal_090_provider_authored_failure_envelope_is_not_extracted_as_output() {
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "implementation_progress".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: "/tmp/run/implementation/progress.md".to_string(),
            companion_of: None,
            display_label: "Implementation progress".to_string(),
            contract_id: Some("implementation_progress".to_string()),
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 1024,
            aggregate_acceptance_cap_bytes: 4096,
            authorized_roots: vec![domain::discovery::AuthorizedRoot {
                root_class: domain::discovery::OutputRootClass::ChainworksMetaRoot,
                root_path: "/tmp/run".to_string(),
            }],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let provider_claim = r#"{
          "code_writer_engine_failure_v1": {
            "provider": "junie",
            "completion_boundary_subtype": "junie_progress_without_terminal_handoff",
            "public_message": "provider-authored claim"
          }
        }"#;
        let unknown_schema = r#"{
          "CHAINWORKS_OUTPUT_V2": {
            "implementation_progress": "not authoritative"
          }
        }"#;

        assert!(extract_output_envelopes(provider_claim, &expected_outputs).is_empty());
        assert!(extract_output_envelopes(unknown_schema, &expected_outputs).is_empty());
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
    fn prompt_error_is_returned_as_failure_transcript() {
        let transcript = transcript_with_prompt_error(
            String::new(),
            "Internal error: You've hit your limit · resets 11:40pm (Asia/Nicosia)",
        )
        .expect("prompt error should be preserved");

        assert!(transcript.contains("ACP session/prompt error"));
        assert!(transcript.contains("hit your limit"));
    }

    #[test]
    fn prompt_error_appends_to_existing_streamed_transcript() {
        let transcript = transcript_with_prompt_error("partial progress".into(), "quota exceeded")
            .expect("prompt error should be preserved");

        assert!(transcript.starts_with("partial progress\n\nACP session/prompt error"));
        assert!(transcript.ends_with("quota exceeded"));
    }

    #[test]
    fn final_prompt_response_output_field_is_captured_as_text() {
        let parsed = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": {
                "stopReason": "end_turn",
                "sessionId": "codex-session",
                "output": [
                    {
                        "type": "text",
                        "text": "<<<CHAINWORKS_OUTPUT:implementation_progress>>>{\"status\":\"done\"}<<<END_CHAINWORKS_OUTPUT>>>"
                    }
                ]
            }
        });

        let chunk = extract_text_chunk(&parsed).expect("final output text should be captured");

        assert!(chunk.contains("<<<CHAINWORKS_OUTPUT:implementation_progress>>>"));
    }

    #[test]
    fn codex_read_and_search_updates_count_as_meaningful_provider_activity() {
        let read_update = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "codex-session",
                "update": {
                    "type": "read",
                    "path": "Chainworks Forge/Support/PreviewSupport.swift"
                }
            }
        });
        let search_update = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "codex-session",
                "update": {
                    "type": "search",
                    "query": "proposal_implementation_auditor"
                }
            }
        });
        let unknown_update = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "codex-session",
                "update": {
                    "type": "unknown"
                }
            }
        });

        assert_eq!(
            session_update_observation(&read_update),
            ("provider_activity", true, Some("read".to_string()))
        );
        assert_eq!(
            session_update_observation(&search_update),
            ("provider_activity", true, Some("search".to_string()))
        );
        assert_eq!(
            session_update_observation(&unknown_update),
            ("other", false, Some("unknown".to_string()))
        );
    }

    #[test]
    fn codex_acp_thought_chunk_preserves_declared_session_update_kind() {
        let thought_update = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "codex-session",
                "update": {
                    "sessionUpdate": "agent_thought_chunk",
                    "content": {
                        "type": "text",
                        "text": "**Planning manual baseline comparison**"
                    }
                }
            }
        });

        assert_eq!(
            session_update_observation(&thought_update),
            (
                "agent_thought_chunk",
                true,
                Some("agent_thought_chunk,text".to_string())
            )
        );
        assert_eq!(
            timeline_title_for_update("agent_thought_chunk"),
            "Agent thought"
        );
    }

    #[test]
    fn meaningful_session_updates_refresh_progress_deadline() {
        assert!(session_update_refreshes_progress_deadline(
            "provider_activity",
            true
        ));
        assert!(session_update_refreshes_progress_deadline(
            "tool_call",
            true
        ));
        assert!(session_update_refreshes_progress_deadline(
            "tool_call_update",
            true
        ));
        assert!(session_update_refreshes_progress_deadline(
            "text_chunk",
            true
        ));
        assert!(!session_update_refreshes_progress_deadline("other", false));
    }

    #[test]
    fn junie_exit_payment_required_agent_failure_is_provider_quota() {
        let update = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "session-260516-192707-1rwl",
                "event": {
                    "state": "FAILED",
                    "agentEvent": {
                        "kind": "AgentFailureEvent",
                        "message": "Junie: Insufficient Account Balance. All tokens on your balance are spent.",
                        "errorCode": "ExitPaymentRequired"
                    }
                }
            }
        });

        let failure = classify_provider_failure_event(&update, "junie").expect("provider failure");

        assert_eq!(failure.failure_phase, "provider_quota");
        assert!(failure.message.contains("provider quota/capacity failure"));
        assert!(failure.message.contains("ExitPaymentRequired"));
        assert!(failure.detail.contains("Insufficient Account Balance"));
    }

    #[test]
    fn junie_provider_quota_detection_tolerates_wrapped_native_events() {
        let update = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "session-260516-192707-1rwl",
                "update": {
                    "type": "com.agentclientprotocol.rpc.JsonRpcNotification",
                    "payload": {
                        "event": {
                            "agentEvent": {
                                "kind": "AgentFailureEvent",
                                "message": "Junie: Insufficient Account Balance. All tokens on your balance are spent.",
                                "errorCode": "ExitPaymentRequired"
                            }
                        }
                    }
                }
            }
        });

        assert_eq!(
            classify_provider_failure_event(&update, "junie")
                .expect("wrapped provider failure")
                .failure_phase,
            "provider_quota"
        );
    }

    #[test]
    fn junie_provider_quota_detection_is_provider_scoped() {
        let update = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "event": {
                    "agentEvent": {
                        "kind": "AgentFailureEvent",
                        "message": "Junie: Insufficient Account Balance. All tokens on your balance are spent.",
                        "errorCode": "ExitPaymentRequired"
                    }
                }
            }
        });

        assert!(classify_provider_failure_event(&update, "codex").is_none());
    }

    #[test]
    fn gemini_capacity_prompt_error_is_provider_quota() {
        let failure = classify_prompt_error_response(
            "gemini",
            Some(500),
            "You have exhausted your capacity on this model.",
        );

        assert_eq!(failure.failure_phase, "provider_quota");
        assert!(failure.message.contains("provider quota/capacity failure"));
        assert!(failure.detail.contains("jsonrpc_error_code=500"));
    }

    #[test]
    fn claude_session_limit_prompt_error_is_provider_quota() {
        let failure = classify_prompt_error_response(
            "claude",
            Some(-32603),
            "Internal error: You've hit your session limit · resets 8pm (Asia/Nicosia)",
        );

        assert_eq!(failure.failure_phase, "provider_quota");
        assert!(failure.message.contains("provider quota/capacity failure"));
        assert!(failure.detail.contains("jsonrpc_error_code=-32603"));
    }

    #[test]
    fn proposal_089_completion_capture_uses_assistant_text_chunks_not_tool_or_thought_chunks() {
        let thought = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "junie-session",
                "update": {
                    "sessionUpdate": "agent_thought_chunk",
                    "content": {"type": "text", "text": "Task: Return JSON object"}
                }
            }
        });
        let tool = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "junie-session",
                "update": {
                    "sessionUpdate": "tool_call",
                    "title": "Open CHAINWORKS_OUTPUT",
                    "content": [{"type": "content", "content": {"type": "text", "text": "CHAINWORKS_OUTPUT"}}]
                }
            }
        });
        let message = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "junie-session",
                "update": {
                    "sessionUpdate": "agent_message_chunk",
                    "content": {
                        "type": "text",
                        "text": "{\"CHAINWORKS_OUTPUT\":{\"tests_result\":{\"status\":\"not_run\",\"commands\":[]}}}"
                    }
                }
            }
        });
        let claude_text_chunk = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session/update",
            "params": {
                "sessionId": "claude-session",
                "update": {
                    "sessionUpdate": "text_chunk",
                    "content": {
                        "type": "text",
                        "text": "{\"CHAINWORKS_OUTPUT\":{\"proposal_review_ui\":{\"decision\":\"approve\"}}}"
                    }
                }
            }
        });

        assert_eq!(extract_agent_message_chunk(&thought), None);
        assert_eq!(extract_agent_message_chunk(&tool), None);
        assert_eq!(
            extract_agent_message_chunk(&message).as_deref(),
            Some(
                "{\"CHAINWORKS_OUTPUT\":{\"tests_result\":{\"status\":\"not_run\",\"commands\":[]}}}"
            )
        );
        assert_eq!(
            extract_agent_message_chunk(&claude_text_chunk).as_deref(),
            Some("{\"CHAINWORKS_OUTPUT\":{\"proposal_review_ui\":{\"decision\":\"approve\"}}}")
        );
    }

    #[test]
    fn proposal_088_completion_capture_prefers_terminal_final_response_after_large_streamed_prelude(
    ) {
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "implementation_progress".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: "/tmp/run/implementation/progress.json".to_string(),
            companion_of: None,
            display_label: "Implementation progress".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 1024,
            aggregate_acceptance_cap_bytes: 4096,
            authorized_roots: vec![],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let mut capture = CompletionTextCapture::default();
        capture.push_streamed_update(&"p".repeat(COMPLETION_CAPTURE_RAW_BYTE_LIMIT + 512));
        capture.set_terminal_final_response(
            "done\n<<<CHAINWORKS_OUTPUT:implementation_progress>>>{\"status\":\"done\"}<<<END_CHAINWORKS_OUTPUT>>>",
        );

        let selected = capture.select_extraction_input();
        let artifacts =
            extract_output_envelopes(selected.text.as_deref().unwrap(), &expected_outputs);

        assert_eq!(
            selected.metadata.capture_source,
            Some(crate::AcpCompletionCaptureSource::TerminalFinalResponse)
        );
        assert_eq!(selected.metadata.completion_text_truncated, false);
        assert_eq!(selected.metadata.extraction_input_truncated, false);
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "implementation_progress");
    }

    #[test]
    fn provider_session_store_final_response_is_used_before_streamed_tail() {
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "tests_result".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: "/tmp/run/implementation/tests-result.json".to_string(),
            companion_of: None,
            display_label: "Tests result".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 1024,
            aggregate_acceptance_cap_bytes: 4096,
            authorized_roots: vec![],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let mut capture = CompletionTextCapture::default();
        capture.push_streamed_update(
            "\n<<<CHAINWORKS_OUTPUT:tests_result>>>{\"status\":\"stale\",\"commands\":[]}<<<END_CHAINWORKS_OUTPUT>>>",
        );
        capture.set_provider_session_store_final_response(
            "\n<<<CHAINWORKS_OUTPUT:tests_result>>>{\"status\":\"passed\",\"commands\":[]}<<<END_CHAINWORKS_OUTPUT>>>",
        );

        let selected = capture.select_extraction_input();
        let artifacts =
            extract_output_envelopes(selected.text.as_deref().unwrap(), &expected_outputs);

        assert_eq!(
            selected.metadata.capture_source,
            Some(crate::AcpCompletionCaptureSource::ProviderSessionStoreFinalResponse)
        );
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "tests_result");
        let content =
            std::str::from_utf8(&artifacts[0].content).expect("artifact content is utf-8");
        assert!(content.contains("\"passed\""));
    }

    #[test]
    fn proposal_088_completion_capture_streamed_tail_finds_output_after_large_prelude_without_terminal_text(
    ) {
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "tests_result".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: "/tmp/run/implementation/tests-result.json".to_string(),
            companion_of: None,
            display_label: "Tests result".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 1024,
            aggregate_acceptance_cap_bytes: 4096,
            authorized_roots: vec![],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let mut capture = CompletionTextCapture::default();
        capture.push_streamed_update(&"p".repeat(COMPLETION_CAPTURE_RAW_BYTE_LIMIT + 512));
        capture.push_streamed_update(
            "\n<<<CHAINWORKS_OUTPUT:tests_result>>>{\"status\":\"passed\",\"commands\":[]}<<<END_CHAINWORKS_OUTPUT>>>",
        );

        let selected = capture.select_extraction_input_with_capped_stream(
            Some(&"p".repeat(MAX_STREAMED_TRANSCRIPT_BYTES)),
            true,
        );
        let artifacts =
            extract_output_envelopes(selected.text.as_deref().unwrap(), &expected_outputs);

        assert_eq!(
            selected.metadata.capture_source,
            Some(crate::AcpCompletionCaptureSource::StreamedUpdateTail)
        );
        assert!(selected.metadata.completion_text_truncated);
        assert!(selected.metadata.extraction_input_truncated);
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "tests_result");
    }

    #[test]
    fn proposal_088_completion_capture_reports_truncated_extraction_input_metadata() {
        let mut capture = CompletionTextCapture::default();
        capture.set_terminal_final_response(&format!(
            "{}tail",
            "x".repeat(COMPLETION_CAPTURE_RAW_BYTE_LIMIT + 128)
        ));

        let selected = capture.select_extraction_input();
        let text = selected.text.expect("truncated capture text");

        assert_eq!(
            selected.metadata.capture_status,
            crate::AcpCompletionCaptureStatus::Captured
        );
        assert_eq!(
            selected.metadata.capture_source,
            Some(crate::AcpCompletionCaptureSource::TerminalFinalResponse)
        );
        assert_eq!(
            selected.metadata.raw_byte_limit,
            COMPLETION_CAPTURE_RAW_BYTE_LIMIT as u64
        );
        assert_eq!(
            selected.metadata.captured_byte_count,
            COMPLETION_CAPTURE_RAW_BYTE_LIMIT as u64
        );
        assert!(selected.metadata.completion_text_truncated);
        assert!(selected.metadata.extraction_input_truncated);
        assert_eq!(text.len(), COMPLETION_CAPTURE_RAW_BYTE_LIMIT);
        let expected_sha256 = sha256_hex(text.as_bytes());
        assert_eq!(
            selected.metadata.extraction_input_sha256.as_deref(),
            Some(expected_sha256.as_str())
        );
    }

    #[test]
    fn proposal_088_completion_capture_classifies_empty_terminal_final_response() {
        let mut capture = CompletionTextCapture::default();
        capture.set_terminal_final_response("   \n\t");

        let selected = capture.select_extraction_input();

        assert_eq!(selected.text, None);
        assert_eq!(
            selected.metadata.capture_status,
            crate::AcpCompletionCaptureStatus::Absent
        );
        assert_eq!(
            selected.metadata.absence_reason,
            Some(crate::AcpCompletionAbsenceReason::TerminalResponseWithoutText)
        );
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

    #[test]
    fn json_chainworks_output_is_extracted_from_fenced_final_text_with_trailing_prose() {
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "tests_result".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: "/tmp/run/implementation/tests-result.json".to_string(),
            companion_of: None,
            display_label: "tests result".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 1024,
            aggregate_acceptance_cap_bytes: 4096,
            authorized_roots: vec![],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let stream = "done\n```json\n{\"CHAINWORKS_OUTPUT\":{\"tests_result\":{\"status\":\"passed\",\"commands\":[]}}}\n```\nthanks";

        let artifacts = extract_output_envelopes(stream, &expected_outputs);

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "tests_result");
        assert_eq!(
            serde_json::from_slice::<Value>(&artifacts[0].content).unwrap(),
            serde_json::json!({"status": "passed", "commands": []})
        );
        assert_eq!(
            artifacts[0].source_kind,
            DiscoveredArtifactSourceKind::ChainworksOutput
        );
    }

    #[test]
    fn stringified_json_chainworks_output_is_extracted_from_final_text() {
        let expected_outputs = vec![ExpectedOutputSpec {
            output_name: "implementation_self_assessment".to_string(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: "/tmp/run/implementation/self-assessment.json".to_string(),
            companion_of: None,
            display_label: "implementation self assessment".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 1024,
            aggregate_acceptance_cap_bytes: 4096,
            authorized_roots: vec![],
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }];
        let stream = serde_json::json!(
            "{\"CHAINWORKS_OUTPUT\":{\"implementation_self_assessment\":{\"status\":\"needs_code_fixes\",\"remaining_code_tasks\":[]}}}"
        )
        .to_string();

        let artifacts = extract_output_envelopes(&stream, &expected_outputs);

        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "implementation_self_assessment");
        assert_eq!(
            serde_json::from_slice::<Value>(&artifacts[0].content).unwrap(),
            serde_json::json!({"status": "needs_code_fixes", "remaining_code_tasks": []})
        );
        assert_eq!(
            artifacts[0].source_kind,
            DiscoveredArtifactSourceKind::ChainworksOutput
        );
    }

    #[test]
    fn labeled_expected_output_json_fences_are_extracted_without_chainworks_envelope() {
        let expected_outputs = vec![
            ExpectedOutputSpec {
                output_name: "implementation_progress".to_string(),
                output_role: domain::discovery::ExpectedOutputRole::Machine,
                target_path: "/tmp/run/implementation/progress.json".to_string(),
                companion_of: None,
                display_label: "implementation progress".to_string(),
                contract_id: Some("implementation_progress".to_string()),
                required: true,
                reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
                max_bytes: 1024,
                aggregate_acceptance_cap_bytes: 4096,
                authorized_roots: vec![],
                source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
            },
            ExpectedOutputSpec {
                output_name: "implementation_self_assessment".to_string(),
                output_role: domain::discovery::ExpectedOutputRole::Machine,
                target_path: "/tmp/run/implementation/self-assessment.json".to_string(),
                companion_of: None,
                display_label: "implementation self assessment".to_string(),
                contract_id: Some("implementation_self_assessment_v2".to_string()),
                required: true,
                reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
                max_bytes: 1024,
                aggregate_acceptance_cap_bytes: 4096,
                authorized_roots: vec![],
                source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
            },
            ExpectedOutputSpec {
                output_name: "tests_result".to_string(),
                output_role: domain::discovery::ExpectedOutputRole::Machine,
                target_path: "/tmp/run/implementation/tests-result.json".to_string(),
                companion_of: None,
                display_label: "tests result".to_string(),
                contract_id: Some("tests_result".to_string()),
                required: true,
                reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
                max_bytes: 1024,
                aggregate_acceptance_cap_bytes: 4096,
                authorized_roots: vec![],
                source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
            },
        ];
        let stream = r#"
Implementation progress:
```json
{"status":"in_progress","summary":"continued P084","completed_tasks":[],"remaining_tasks":["swift readback"]}
```

implementation_self_assessment
```json
{"status":"needs_code_fixes","implementation_complete":false,"verification_green":true,"remaining_code_tasks":[],"handoff_tasks":[],"known_risks":[],"tests_run":[],"docs_impacted":[]}
```

Tests result:
```json
{"status":"passed","commands":["./scripts/test-gate.sh proposal-084"],"failures":[]}
```
"#;

        let artifacts = extract_output_envelopes(stream, &expected_outputs);

        for output_name in [
            "implementation_progress",
            "implementation_self_assessment",
            "tests_result",
        ] {
            assert!(
                artifacts
                    .iter()
                    .any(|artifact| artifact.name == output_name),
                "missing labeled output {output_name}: {artifacts:?}"
            );
        }
    }

    // SEC-P079-HIGH-001: p079_path_parents_have_no_symlinks must also reject paths where
    // the final file component is a symlink (not only parent directories).
    // Uses canonicalized tempdir path to avoid macOS /var → /private/var parent symlink.
    #[tokio::test]
    async fn sec_high_001_path_check_rejects_symlinked_final_component() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        // Canonicalize the tempdir path to resolve platform-level symlinks (e.g. macOS /var→/private/var)
        // so that parent-component walks do not spuriously fail on platform symlinks.
        let canon_dir = dir.path().canonicalize().unwrap();
        let real_file = canon_dir.join("real_output.json");
        std::fs::write(&real_file, b"original").unwrap();
        // Create a symlink that points at the real file (same directory — no parent symlink).
        let symlink_path = canon_dir.join("declared_output.json");
        symlink(&real_file, &symlink_path).unwrap();
        // The final component is a symlink; p079_path_parents_have_no_symlinks must return false.
        let result = p079_path_parents_have_no_symlinks(symlink_path.to_str().unwrap()).await;
        assert!(
            !result,
            "p079_path_parents_have_no_symlinks must return false when final component is a symlink"
        );
        // A non-existent file on a canon path must return true (no symlink present on final component).
        let new_output = canon_dir.join("new_output.json");
        let result_new = p079_path_parents_have_no_symlinks(new_output.to_str().unwrap()).await;
        assert!(
            result_new,
            "p079_path_parents_have_no_symlinks must return true for a non-existent file (no symlink) on canon path"
        );
    }

    #[test]
    fn sec_med_001_sanitized_event_label_excludes_provider_controlled_content() {
        // SEC-MED-001: p079_sanitized_event_label must not include provider-controlled title
        // or option IDs that could carry credentials, tokens, or absolute paths.
        let label = p079_sanitized_event_label("42", "fs_write_canonical_output_path");
        assert!(label.contains("id=42"), "label must include request id");
        assert!(
            label.contains("resource_kind="),
            "label must include resource_kind"
        );
        // Must NOT include "title=" which would include provider-controlled text.
        assert!(
            !label.contains("title="),
            "sanitized label must not include provider-controlled title"
        );
        // Must NOT include "options=" which would include provider-controlled option IDs.
        assert!(
            !label.contains("options="),
            "sanitized label must not include provider-controlled options"
        );
    }

    #[test]
    fn sec_med_001_sanitized_label_with_credential_like_resource_kind_is_safe() {
        // SEC-MED-001: even if a resource_kind string were adversarial (which it can't be
        // since it comes from p079_classify_resource_kind_from_tool, a server-side function),
        // the label format does not embed arbitrary provider strings.
        let label = p079_sanitized_event_label("req-99", "fs_write_canonical_output_path");
        // The label is bounded and does not include raw permission params.
        assert!(!label.is_empty());
        // Key invariant: label never contains path separators from provider data.
        assert!(
            !label.contains("/Users/"),
            "label must not contain filesystem paths"
        );
        assert!(
            !label.contains("Bearer "),
            "label must not contain bearer tokens"
        );
    }

    // SEC-P079-001 regression tests for build_p079_repair_permission_grant.
    // These verify that during a P079 repair turn, the transport ONLY selects allow_once
    // options and never grants allow_always, even when allow_always is offered first.
    // This enforces the single-use posture required by the approved proposal.

    #[test]
    fn sec_p079_001_repair_grant_selects_allow_once_when_both_options_present() {
        // SEC-P079-001: even if allow_always is listed first, build_p079_repair_permission_grant
        // must select allow_once. Never grant persistent access during a P079 repair turn.
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file",
                "input": {"file_path": "/tmp/.chainworks/proposals/current/proposal.md"}
            },
            "options": [
                {
                    "kind": "allow_always",
                    "optionId": "Always allow write_file",
                    "name": "Always allow write_file"
                },
                {
                    "kind": "allow_once",
                    "optionId": "allow_once",
                    "name": "Allow once"
                }
            ]
        });

        let grant = build_p079_repair_permission_grant(&serde_json::json!(5), &params)
            .expect("grant must be Some when allow_once is available");

        // Must select allow_once, never allow_always.
        assert_eq!(grant["id"], serde_json::json!(5));
        assert_eq!(
            grant["result"]["outcome"]["optionId"],
            serde_json::json!("allow_once"),
            "SEC-P079-001: P079 repair grant must select allow_once, not allow_always"
        );
    }

    #[test]
    fn sec_p079_001_repair_grant_returns_none_when_only_allow_always_offered() {
        // SEC-P079-001: if the provider offers only allow_always (no allow_once option),
        // build_p079_repair_permission_grant must return None so the caller can fail closed.
        // Granting allow_always during a repair turn violates the P079 posture.
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file",
                "input": {"file_path": "/tmp/.chainworks/proposals/current/proposal.md"}
            },
            "options": [
                {
                    "kind": "allow_always",
                    "optionId": "Always allow write_file",
                    "name": "Always allow write_file"
                }
            ]
        });

        let grant = build_p079_repair_permission_grant(&serde_json::json!(6), &params);
        assert!(
            grant.is_none(),
            "SEC-P079-001: P079 repair grant must return None (fail closed) when no allow_once option exists"
        );
    }

    #[test]
    fn sec_p079_001_repair_grant_returns_none_when_no_options() {
        // SEC-P079-001: empty options list must return None (fail closed).
        let params = serde_json::json!({
            "toolCall": {"name": "write_file"},
            "options": []
        });
        let grant = build_p079_repair_permission_grant(&serde_json::json!(7), &params);
        assert!(
            grant.is_none(),
            "SEC-P079-001: P079 repair grant must return None when options list is empty"
        );
    }

    #[test]
    fn sec_p079_001_repair_grant_selects_correct_allow_once_option_id() {
        // SEC-P079-001: the optionId returned must exactly match the allow_once option's ID,
        // not a hardcoded string. Providers may use custom allow_once optionIds.
        let params = serde_json::json!({
            "toolCall": {"name": "write_file"},
            "options": [
                {"kind": "reject_once", "optionId": "No", "name": "No"},
                {"kind": "allow_once", "optionId": "Yes", "name": "Yes, once"}
            ]
        });
        let grant = build_p079_repair_permission_grant(&serde_json::json!(8), &params)
            .expect("grant must be Some when allow_once is available");
        assert_eq!(
            grant["result"]["outcome"]["optionId"],
            serde_json::json!("Yes"),
            "SEC-P079-001: optionId must match the provider's allow_once option, not a hardcoded string"
        );
    }

    // SEC-P079-002 regression tests for p079_sanitize_method_name / p079_extract_decision_fields.
    // These verify that provider-controlled tool names are sanitized before storage in
    // permission_decisions.method, preventing log injection, newline injection, or token leakage.

    #[test]
    fn sec_p079_002_extract_decision_fields_strips_newlines_from_tool_name() {
        // SEC-P079-002: a provider sending a tool name with embedded newlines (log injection)
        // must have them stripped before the name reaches permission_decisions.method storage.
        // The newline is removed, preventing a single field from spanning log lines.
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file\nX-Injected-Header: evil",
                "input": {"file_path": "/tmp/output.json"}
            }
        });
        let (tool_name, _path) = p079_extract_decision_fields(&params);
        assert!(
            !tool_name.contains('\n'),
            "SEC-P079-002: newlines must be stripped from provider-controlled tool name"
        );
        assert!(
            !tool_name.contains('\r'),
            "SEC-P079-002: carriage returns must be stripped from provider-controlled tool name"
        );
        // After stripping the newline the value is a single concatenated string —
        // no log line break is possible.
        assert!(
            tool_name.starts_with("write_file"),
            "legitimate tool name prefix must be preserved after sanitization"
        );
    }

    #[test]
    fn sec_p079_002_extract_decision_fields_strips_control_chars_from_tool_name() {
        // SEC-P079-002: carriage returns, tabs, and other ASCII control characters
        // must be stripped from the tool name.
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file\r\n\t\x07crafted",
                "input": {"file_path": "/tmp/output.json"}
            }
        });
        let (tool_name, _path) = p079_extract_decision_fields(&params);
        assert!(
            !tool_name.chars().any(|c| c.is_ascii_control()),
            "SEC-P079-002: all ASCII control characters must be stripped from tool name"
        );
    }

    #[test]
    fn sec_p079_002_extract_decision_fields_caps_tool_name_length() {
        // SEC-P079-002: very long tool names (potential amplification attack) must be
        // capped at MAX_METHOD_BYTES to prevent resource bloat in readback.
        let long_name = "x".repeat(512);
        let params = serde_json::json!({
            "toolCall": {
                "name": long_name,
                "input": {"file_path": "/tmp/output.json"}
            }
        });
        let (tool_name, _path) = p079_extract_decision_fields(&params);
        assert!(
            tool_name.len() <= 128,
            "SEC-P079-002: tool name must be capped at 128 bytes; got {} bytes",
            tool_name.len()
        );
    }

    #[test]
    fn sec_p079_002_sanitize_method_name_preserves_legitimate_tool_names() {
        // SEC-P079-002: sanitization must not corrupt normal, clean tool names.
        assert_eq!(p079_sanitize_method_name("write_file"), "write_file");
        assert_eq!(
            p079_sanitize_method_name("str_replace_editor"),
            "str_replace_editor"
        );
        assert_eq!(p079_sanitize_method_name("create_file"), "create_file");
        assert_eq!(p079_sanitize_method_name(""), "");
    }

    #[test]
    fn sec_p079_002_sanitize_method_name_utf8_safe_truncation_no_panic() {
        // SEC-P079-002: truncation at 128 bytes must not panic on multibyte UTF-8 characters.
        // Build a string with 4-byte UTF-8 emoji (🔥 = \u{1F525}) that crosses the 128-byte boundary.
        // 32 * 4 = 128 bytes exactly, then add one more emoji so len() > 128.
        let emoji = "🔥";
        let s: String = emoji.repeat(33); // 33 * 4 = 132 bytes
        let result = p079_sanitize_method_name(&s);
        // Must not panic and must be <= 128 bytes.
        assert!(
            result.len() <= 128,
            "Truncation exceeded 128 bytes: {}",
            result.len()
        );
        // Must be valid UTF-8 (String guarantees this, but verify round-trip).
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn sec_p079_002_sanitize_method_name_redacts_absolute_path() {
        // SEC-P079-002: absolute paths embedded as tool names must be redacted.
        let result = p079_sanitize_method_name("/Users/user/.ssh/id_rsa");
        assert_eq!(result, "[REDACTED_PATH]");
    }

    #[test]
    fn sec_p079_002_sanitize_method_name_redacts_bearer_credential() {
        // SEC-P079-002: bearer token patterns in tool names must be redacted.
        let result = p079_sanitize_method_name("Bearer eyJhbGciOiJIUzI1NiJ9");
        assert_eq!(result, "[REDACTED_CREDENTIAL]");
    }

    // SEC-P079-MED-004: embedded absolute paths (not just leading slash) must be redacted.
    #[test]
    fn sec_p079_med_004_sanitize_method_name_redacts_embedded_path() {
        // Tool name like "write_file_/Users/user/.ssh/id_rsa" was previously not redacted
        // because starts_with('/') is false. Now caught by embedded /Users/ check.
        let result = p079_sanitize_method_name("write_file_/Users/user/.ssh/id_rsa");
        assert_eq!(
            result, "[REDACTED_PATH]",
            "embedded /Users/ path must be redacted"
        );
        let result2 = p079_sanitize_method_name("read_/home/admin/credentials.json");
        assert_eq!(
            result2, "[REDACTED_PATH]",
            "embedded /home/ path must be redacted"
        );
        let result3 = p079_sanitize_method_name("cat_/etc/passwd");
        assert_eq!(
            result3, "[REDACTED_PATH]",
            "embedded /etc/ path must be redacted"
        );
    }

    // SEC-P079-MED-004: common API token prefixes in tool names must be redacted.
    #[test]
    fn sec_p079_med_004_sanitize_method_name_redacts_common_token_prefixes() {
        let cases = [
            ("sk-ant-api123456789abcdef", "[REDACTED_CREDENTIAL]"),
            ("sk-proj-secretvalue12345", "[REDACTED_CREDENTIAL]"),
            ("ghp_abcdef1234567890ghij", "[REDACTED_CREDENTIAL]"),
            ("xoxb-123456789-abcdef", "[REDACTED_CREDENTIAL]"),
            ("AKIA1234567890ABCDEF", "[REDACTED_CREDENTIAL]"),
            ("github_pat_secrettoken", "[REDACTED_CREDENTIAL]"),
        ];
        for (input, expected) in &cases {
            let result = p079_sanitize_method_name(input);
            assert_eq!(
                result, *expected,
                "token prefix in '{input}' must be redacted to '{expected}'"
            );
        }
    }

    // SEC-P079-POSTURE: p079_posture_denied must deny ambiguous multi-path requests.
    #[test]
    fn sec_p079_posture_denied_rejects_ambiguous_multi_path_request() {
        let canonical = vec!["/canonical/output.md".to_string()];
        // Two different path fields with different values — ambiguous; must deny.
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file",
                "input": {
                    "file_path": "/canonical/output.md",
                    "path": "/other/non_canonical.md"
                }
            }
        });
        assert!(
            p079_posture_denied(&params, &canonical),
            "ambiguous multi-path must be denied"
        );
    }

    #[test]
    fn sec_p079_posture_denied_allows_single_canonical_path() {
        let canonical = vec!["/canonical/output.md".to_string()];
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file",
                "input": { "file_path": "/canonical/output.md" }
            }
        });
        assert!(
            !p079_posture_denied(&params, &canonical),
            "single canonical path must be allowed"
        );
    }

    #[test]
    fn sec_p079_posture_denied_allows_duplicate_path_fields_with_same_value() {
        let canonical = vec!["/canonical/output.md".to_string()];
        // Multiple path fields but all the same value — not ambiguous.
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file",
                "input": {
                    "file_path": "/canonical/output.md",
                    "path": "/canonical/output.md"
                }
            }
        });
        assert!(
            !p079_posture_denied(&params, &canonical),
            "same value in multiple fields must be allowed"
        );
    }

    #[test]
    fn sec_p079_posture_denied_denies_canonical_first_with_non_canonical_second() {
        let canonical = vec!["/canonical/output.md".to_string()];
        // Canonical path first but a non-canonical path in a secondary field.
        let params = serde_json::json!({
            "toolCall": {
                "name": "write_file",
                "input": {
                    "file_path": "/canonical/output.md",
                    "new_file_path": "/etc/passwd"
                }
            }
        });
        assert!(
            p079_posture_denied(&params, &canonical),
            "canonical first + non-canonical second must be denied"
        );
    }

    // SEC-ACP-002: all provider-supplied request IDs are hashed unconditionally so
    // token-shaped values never reach runtime receipts or non-operator readback surfaces.
    #[test]
    fn sec_acp_002_provider_request_id_always_hashed() {
        let token_shaped = [
            "sk-abc12",
            "ghp_1234567",
            "github_pat_abcde",
            "xoxb-short",
            "AKIA1234",
            "short",
            "a",
            "abc-123",
        ];
        for raw in &token_shaped {
            let result = cap_provider_request_id(raw);
            // Every provider-supplied ID, including short alphanumeric ones, must be hashed.
            assert!(
                result.starts_with("pid-"),
                "provider id '{raw}' must be hashed to pid-<hash>, got '{result}'"
            );
            // The output must not be the raw ID itself.
            assert_ne!(
                result, *raw,
                "raw provider id '{raw}' must not pass through unchanged"
            );
        }
        // Same input must produce the same hash (deterministic).
        assert_eq!(
            cap_provider_request_id("sk-abc12"),
            cap_provider_request_id("sk-abc12"),
        );
        // Different inputs must produce different hashes.
        assert_ne!(
            cap_provider_request_id("sk-abc12"),
            cap_provider_request_id("ghp_1234567"),
        );
    }

    // SEC-P079-MED-004: legitimate tool names must still pass through unchanged.
    #[test]
    fn sec_p079_med_004_sanitize_method_name_preserves_safe_tool_names() {
        let safe_names = [
            "write_file",
            "str_replace_editor",
            "bash",
            "read_file",
            "list_directory",
            "create_file_editor",
        ];
        for name in &safe_names {
            let result = p079_sanitize_method_name(name);
            assert_eq!(
                result, *name,
                "safe tool name '{name}' must pass through unchanged"
            );
        }
    }
}
