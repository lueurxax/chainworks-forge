use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use domain::ids::{AgentExecutionId, RunId};
use domain::xcode_runtime::{
    McpBrokerObservation, XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, Notify};
use tracing::{error, warn};
use uuid::Uuid;

use crate::manager::{BrokeredXcodeLeaseAttachment, XcodeBrokerLeaseAttacher};
use crate::xcode_target::{
    probe_local_xcode_host, target_resolver_failure_class, HostProbeContext,
    LocalXcodeHostProbeConfig, XcodeTargetResolver, XcodeTargetSelectionConfidence,
    XcodeTargetSelectionInput, XcodeTargetSnapshot,
};
use crate::{
    AcpMcpServerPayload, BrokeredXcodeMcpIntent, ExecutionRequest, NoopXcodeRuntimeObservationSink,
    ResolvedMcpServerTransport, XcodeRuntimeObservationSink,
};

const XCODE_MCP_ACTION_REQUIRED_AFTER: Duration = Duration::from_secs(5);
const XCODE_MCP_ACTIVE_LEASE_IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
#[cfg(test)]
const XCODE_MCP_BACKEND_SHUTDOWN_WAIT: Duration = Duration::from_millis(250);
#[cfg(not(test))]
const XCODE_MCP_BACKEND_SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct XcodeMcpBridgePoolConfig {
    pub pool_id: String,
    pub base_url: String,
    pub max_active_leases: usize,
    pub max_queued_leases: usize,
    pub queue_timeout: Duration,
    pub spawn_init_timeout: Duration,
    pub first_connect_timeout: Duration,
    pub broker_disabled: bool,
    pub tool_allowlists_by_hash: BTreeMap<String, BTreeSet<String>>,
    pub target_probe_context: Option<HostProbeContext>,
    pub use_local_host_probe: bool,
}

impl Default for XcodeMcpBridgePoolConfig {
    fn default() -> Self {
        Self {
            pool_id: "local-xcode-mcp-pool".to_string(),
            base_url: "http://127.0.0.1:0/xcode-mcp".to_string(),
            max_active_leases: 8,
            max_queued_leases: 16,
            queue_timeout: Duration::from_secs(45),
            spawn_init_timeout: Duration::from_secs(30),
            first_connect_timeout: Duration::from_secs(60),
            broker_disabled: false,
            tool_allowlists_by_hash: BTreeMap::new(),
            target_probe_context: None,
            use_local_host_probe: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XcodeMcpLeaseState {
    Reserved,
    Active,
    Closing,
}

#[derive(Clone, Debug)]
struct LeaseRecord {
    run_id: RunId,
    agent_execution_id: Option<AgentExecutionId>,
    endpoint: String,
    authorization_hash: String,
    state: XcodeMcpLeaseState,
    last_activity_at: Instant,
    first_connect_deadline: Instant,
    mcp_policy: BrokerMcpPolicy,
    target_snapshot: Option<XcodeTargetSnapshot>,
}

#[derive(Default)]
struct XcodeMcpBridgePoolState {
    leases: HashMap<String, LeaseRecord>,
    target_probe_context: Option<HostProbeContext>,
    health_last_state: Option<XcodeBrokerHealthState>,
    health_last_transition_at: String,
    helper_cleanup_reaped_leases_total: u64,
}

pub struct XcodeMcpBridgePool {
    config: XcodeMcpBridgePoolConfig,
    state: Mutex<XcodeMcpBridgePoolState>,
    initialize_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    queued_leases: AtomicUsize,
    queue_notify: Notify,
    observation_sink: Arc<dyn XcodeRuntimeObservationSink>,
    backend: Option<Arc<dyn XcodeMcpBackend>>,
    observation_persistence_failures: AtomicU64,
}

#[derive(Clone, Debug)]
pub struct XcodeMcpProcessBackendConfig {
    pub command: String,
    pub args: Vec<String>,
    pub request_timeout: Duration,
}

impl Default for XcodeMcpProcessBackendConfig {
    fn default() -> Self {
        Self {
            command: "xcrun".to_string(),
            args: vec!["mcpbridge".to_string()],
            request_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Default)]
pub struct XcodeMcpProcessBackend {
    config: XcodeMcpProcessBackendConfig,
    registry: Mutex<XcodeMcpProcessBackendRegistry>,
}

#[derive(Default)]
struct XcodeMcpProcessBackendRegistry {
    sessions: HashMap<String, Arc<Mutex<XcodeMcpProcessBackendSession>>>,
    lease_to_session: HashMap<String, String>,
    session_ref_counts: HashMap<String, usize>,
}

struct XcodeMcpProcessBackendSession {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_backend_id: u64,
    cached_initialize_result: Option<serde_json::Value>,
    initialized_notification_forwarded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XcodeBrokerHealthState {
    Disabled,
    Healthy,
    Degraded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeBrokerHealthSnapshot {
    pub state: XcodeBrokerHealthState,
    pub reason_code: String,
    pub can_acquire_new_xcode_leases: bool,
    pub active_lease_count: usize,
    pub initialize_queue_depth: usize,
    pub last_transition_at: String,
    pub operator_message: String,
    pub pool_id: String,
    pub active_leases: usize,
    pub queued_leases: usize,
    pub max_active_leases: usize,
    pub max_queued_leases: usize,
    pub broker_disabled: bool,
    pub backend_available: bool,
    pub observation_persistence_failures: u64,
    pub stale_lease_count: usize,
    pub backend_session_count: usize,
    pub helper_cleanup_reaped_leases_total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XcodeBrokerHttpRouteState {
    Activated,
    AlreadyActive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XcodeMcpBackendRequestContext {
    pub run_id: RunId,
    pub lease_id: String,
    pub endpoint: String,
    pub agent_execution_id: Option<AgentExecutionId>,
    pub target_snapshot: Option<XcodeTargetSnapshot>,
}

#[async_trait]
pub trait XcodeMcpBackend: Send + Sync {
    async fn forward_json_rpc(
        &self,
        context: XcodeMcpBackendRequestContext,
        request: serde_json::Value,
    ) -> Result<serde_json::Value>;

    async fn backend_process_id(&self, _lease_id: &str) -> Option<i64> {
        None
    }

    async fn release_lease(&self, _lease_id: &str) -> Result<()> {
        Ok(())
    }

    async fn active_backend_session_count(&self) -> Option<usize> {
        None
    }
}

impl XcodeMcpProcessBackend {
    pub fn new(config: XcodeMcpProcessBackendConfig) -> Self {
        Self {
            config,
            registry: Mutex::new(XcodeMcpProcessBackendRegistry::default()),
        }
    }

    async fn session_for(
        &self,
        context: &XcodeMcpBackendRequestContext,
    ) -> Result<Arc<Mutex<XcodeMcpProcessBackendSession>>> {
        let session_key = backend_session_key(context);
        let mut registry = self.registry.lock().await;
        if let Some(session) = registry.sessions.get(&session_key).cloned() {
            if !registry.lease_to_session.contains_key(&context.lease_id) {
                registry
                    .lease_to_session
                    .insert(context.lease_id.clone(), session_key.clone());
                *registry.session_ref_counts.entry(session_key).or_default() += 1;
            }
            return Ok(session.clone());
        }
        let session = Arc::new(Mutex::new(
            XcodeMcpProcessBackendSession::spawn(&self.config, context).await?,
        ));
        registry
            .sessions
            .insert(session_key.clone(), session.clone());
        registry
            .lease_to_session
            .insert(context.lease_id.clone(), session_key.clone());
        registry.session_ref_counts.insert(session_key, 1);
        Ok(session)
    }

    async fn remove_session_after_failure(&self, lease_id: &str) {
        let session = {
            let mut registry = self.registry.lock().await;
            let Some(session_key) = registry.lease_to_session.remove(lease_id) else {
                return;
            };
            registry
                .lease_to_session
                .retain(|_, mapped_key| mapped_key != &session_key);
            registry.session_ref_counts.remove(&session_key);
            registry.sessions.remove(&session_key)
        };
        if let Some(session) = session {
            let _ = session.lock().await.close().await;
        }
    }
}

#[async_trait]
impl XcodeMcpBackend for XcodeMcpProcessBackend {
    async fn forward_json_rpc(
        &self,
        context: XcodeMcpBackendRequestContext,
        request: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let session = self.session_for(&context).await?;
        let result = {
            let mut session = session.lock().await;
            match tokio::time::timeout(self.config.request_timeout, session.forward(request)).await
            {
                Ok(result) => result,
                Err(_) => Err(anyhow::anyhow!(
                    "xcode_mcp_initialize_timeout: timed out after {:?} waiting for backend response for lease '{}'",
                    self.config.request_timeout,
                    context.lease_id
                )),
            }
        };
        if result.is_err() {
            self.remove_session_after_failure(&context.lease_id).await;
        }
        result
    }

    async fn backend_process_id(&self, lease_id: &str) -> Option<i64> {
        let session = {
            let registry = self.registry.lock().await;
            let session_key = registry.lease_to_session.get(lease_id)?;
            registry.sessions.get(session_key).cloned()?
        };
        let session = session.lock().await;
        session.child.id().map(|pid| pid as i64)
    }

    async fn release_lease(&self, lease_id: &str) -> Result<()> {
        let session = {
            let mut registry = self.registry.lock().await;
            let Some(session_key) = registry.lease_to_session.remove(lease_id) else {
                return Ok(());
            };
            let remaining = registry
                .session_ref_counts
                .get(&session_key)
                .copied()
                .unwrap_or(1)
                .saturating_sub(1);
            if remaining > 0 {
                registry.session_ref_counts.insert(session_key, remaining);
                None
            } else {
                registry.session_ref_counts.remove(&session_key);
                registry.sessions.remove(&session_key)
            }
        };
        if let Some(session) = session {
            session.lock().await.close().await?;
        }
        Ok(())
    }

    async fn active_backend_session_count(&self) -> Option<usize> {
        Some(self.registry.lock().await.sessions.len())
    }
}

impl XcodeMcpProcessBackendSession {
    async fn spawn(
        config: &XcodeMcpProcessBackendConfig,
        context: &XcodeMcpBackendRequestContext,
    ) -> Result<Self> {
        let Some(target_snapshot) = context.target_snapshot.as_ref() else {
            bail!(
                "host_env_unavailable: cannot spawn Xcode MCP backend for lease '{}' without a resolved Xcode target snapshot",
                context.lease_id
            );
        };

        let mut command = Command::new(&config.command);
        command
            .env_clear()
            .args(&config.args)
            .env("HOME", &target_snapshot.operator_home)
            .env("TMPDIR", &target_snapshot.darwin_tmpdir)
            .env("DEVELOPER_DIR", &target_snapshot.developer_dir)
            .env(
                "USER",
                operator_account_name(&target_snapshot.operator_home),
            )
            .env(
                "LOGNAME",
                operator_account_name(&target_snapshot.operator_home),
            )
            .env("PATH", xcode_backend_path(&target_snapshot.developer_dir))
            .env("MCP_XCODE_PID", target_snapshot.xcode_pid.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        crate::transport::isolate_process_group(&mut command);

        let mut child = command.spawn().with_context(|| {
            format!(
                "xcode_mcp_backend_spawn_failed: spawn Xcode MCP backend for lease '{}' with command '{}'",
                context.lease_id, config.command
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .context("xcode_mcp_backend_spawn_failed: backend process has no stdin pipe")?;
        let stdout = child
            .stdout
            .take()
            .context("xcode_mcp_backend_spawn_failed: backend process has no stdout pipe")?;

        Ok(Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            next_backend_id: 0,
            cached_initialize_result: None,
            initialized_notification_forwarded: false,
        })
    }

    async fn forward(&mut self, mut request: serde_json::Value) -> Result<serde_json::Value> {
        let method = request
            .get("method")
            .and_then(|method| method.as_str())
            .map(str::to_string);
        let original_id = request.get("id").cloned();
        if original_id.is_none() {
            if method.as_deref() == Some("notifications/initialized")
                && self.initialized_notification_forwarded
            {
                return Ok(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": serde_json::Value::Null,
                    "result": serde_json::Value::Null
                }));
            }
            write_json_line(&mut self.stdin, &request)
                .await
                .context("xcode_mcp_backend_write_failed: send notification to backend process")?;
            if method.as_deref() == Some("notifications/initialized") {
                self.initialized_notification_forwarded = true;
            }
            return Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "result": serde_json::Value::Null
            }));
        }

        let original_id = original_id.unwrap_or(serde_json::Value::Null);
        if method.as_deref() == Some("initialize") {
            if let Some(cached_result) = self.cached_initialize_result.clone() {
                return Ok(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": original_id,
                    "result": cached_result
                }));
            }
        }
        self.next_backend_id += 1;
        let backend_id = self.next_backend_id;
        request["id"] = serde_json::Value::from(backend_id);

        write_json_line(&mut self.stdin, &request)
            .await
            .context("xcode_mcp_backend_write_failed: send request to backend process")?;

        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .await
                .context("xcode_mcp_backend_read_failed: read response from backend process")?;
            if read == 0 {
                bail!("xcode_mcp_backend_crashed: backend process stdout closed");
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut response: serde_json::Value = serde_json::from_str(trimmed)
                .context("xcode_mcp_backend_read_failed: parse backend JSON-RPC response")?;
            if response.get("id").and_then(|id| id.as_u64()) != Some(backend_id) {
                continue;
            }
            if method.as_deref() == Some("initialize") {
                if let Some(result) = response.get("result").cloned() {
                    self.cached_initialize_result = Some(result);
                }
            }
            response["id"] = original_id;
            return Ok(response);
        }
    }

    async fn close(&mut self) -> Result<()> {
        let child_pid = self.child.id();
        let _ = self.stdin.shutdown().await;
        match tokio::time::timeout(XCODE_MCP_BACKEND_SHUTDOWN_WAIT, self.child.wait()).await {
            Ok(Ok(_status)) => Ok(()),
            _ => {
                #[cfg(unix)]
                if let Some(pid) = child_pid {
                    crate::transport::signal_process_group(pid, libc::SIGTERM);
                }
                if tokio::time::timeout(XCODE_MCP_BACKEND_SHUTDOWN_WAIT, self.child.wait())
                    .await
                    .is_ok_and(|result| result.is_ok())
                {
                    return Ok(());
                }
                #[cfg(unix)]
                if let Some(pid) = child_pid {
                    crate::transport::signal_process_group(pid, libc::SIGKILL);
                }
                let _ = self.child.kill().await;
                let _ = self.child.wait().await;
                Ok(())
            }
        }
    }
}

async fn write_json_line(stdin: &mut ChildStdin, msg: &serde_json::Value) -> Result<()> {
    let mut line = serde_json::to_vec(msg)?;
    line.push(b'\n');
    stdin.write_all(&line).await?;
    stdin.flush().await?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrokerMcpPolicyDecision {
    Allow,
    Deny { reason: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrokerMcpPolicy {
    allowlist_hash: Option<String>,
    allowed_tools: Option<BTreeSet<String>>,
}

impl BrokerMcpPolicy {
    pub fn from_intent(
        intent: &crate::BrokeredXcodeMcpIntent,
        allowlists_by_hash: &BTreeMap<String, BTreeSet<String>>,
    ) -> Self {
        let allowlist_hash = intent.resolved_tool_allowlist_hash.clone();
        let allowed_tools = allowlist_hash
            .as_deref()
            .map(|hash| allowlists_by_hash.get(hash).cloned().unwrap_or_default());
        Self {
            allowlist_hash,
            allowed_tools,
        }
    }

    pub fn allow_all() -> Self {
        Self::default()
    }

    pub fn filter_tools_list(&self, mut result: serde_json::Value) -> serde_json::Value {
        let Some(allowed_tools) = &self.allowed_tools else {
            return result;
        };
        let Some(tools) = result
            .get_mut("tools")
            .and_then(|tools| tools.as_array_mut())
        else {
            return result;
        };
        tools.retain(|tool| {
            tool.get("name")
                .and_then(|name| name.as_str())
                .is_some_and(|name| allowed_tools.contains(name))
        });
        result
    }

    pub fn authorize_tools_call(&self, tool_name: &str) -> BrokerMcpPolicyDecision {
        let Some(allowed_tools) = &self.allowed_tools else {
            return BrokerMcpPolicyDecision::Allow;
        };
        if allowed_tools.contains(tool_name) {
            BrokerMcpPolicyDecision::Allow
        } else {
            BrokerMcpPolicyDecision::Deny {
                reason: format!(
                    "tool '{tool_name}' is not in broker allowlist {}",
                    self.allowlist_hash
                        .as_deref()
                        .unwrap_or("<missing-allowlist-hash>")
                ),
            }
        }
    }
}

struct QueueLeasePermit<'a> {
    queued_leases: &'a AtomicUsize,
    lease_count: usize,
}

impl Drop for QueueLeasePermit<'_> {
    fn drop(&mut self) {
        self.queued_leases
            .fetch_sub(self.lease_count, Ordering::AcqRel);
    }
}

impl XcodeMcpBridgePool {
    pub fn new(config: XcodeMcpBridgePoolConfig) -> Self {
        Self::new_with_sink(config, Arc::new(NoopXcodeRuntimeObservationSink))
    }

    pub fn new_with_sink(
        config: XcodeMcpBridgePoolConfig,
        observation_sink: Arc<dyn XcodeRuntimeObservationSink>,
    ) -> Self {
        Self::new_with_optional_backend(config, observation_sink, None)
    }

    pub fn new_with_sink_and_backend(
        config: XcodeMcpBridgePoolConfig,
        observation_sink: Arc<dyn XcodeRuntimeObservationSink>,
        backend: Arc<dyn XcodeMcpBackend>,
    ) -> Self {
        Self::new_with_optional_backend(config, observation_sink, Some(backend))
    }

    pub fn new_with_sink_and_process_backend(
        config: XcodeMcpBridgePoolConfig,
        observation_sink: Arc<dyn XcodeRuntimeObservationSink>,
    ) -> Self {
        Self::new_with_sink_and_backend(
            config,
            observation_sink,
            Arc::new(XcodeMcpProcessBackend::new(
                XcodeMcpProcessBackendConfig::default(),
            )),
        )
    }

    fn new_with_optional_backend(
        config: XcodeMcpBridgePoolConfig,
        observation_sink: Arc<dyn XcodeRuntimeObservationSink>,
        backend: Option<Arc<dyn XcodeMcpBackend>>,
    ) -> Self {
        let target_probe_context = config.target_probe_context.clone();
        Self {
            config,
            state: Mutex::new(XcodeMcpBridgePoolState {
                target_probe_context,
                ..XcodeMcpBridgePoolState::default()
            }),
            initialize_locks: Mutex::new(HashMap::new()),
            queued_leases: AtomicUsize::new(0),
            queue_notify: Notify::new(),
            observation_sink,
            backend,
            observation_persistence_failures: AtomicU64::new(0),
        }
    }

    pub fn has_backend(&self) -> bool {
        self.backend.is_some()
    }

    pub fn has_tool_allowlist_hash(&self, hash: &str) -> bool {
        self.config.tool_allowlists_by_hash.contains_key(hash)
    }

    pub async fn active_lease_count(&self) -> usize {
        self.state.lock().await.leases.len()
    }

    pub fn queued_lease_count(&self) -> usize {
        self.queued_leases.load(Ordering::Acquire)
    }

    pub async fn lease_state(&self, lease_id: &str) -> Option<XcodeMcpLeaseState> {
        self.state
            .lock()
            .await
            .leases
            .get(lease_id)
            .map(|lease| lease.state)
    }

    pub async fn lease_target_snapshot(&self, lease_id: &str) -> Option<XcodeTargetSnapshot> {
        self.state
            .lock()
            .await
            .leases
            .get(lease_id)
            .and_then(|lease| lease.target_snapshot.clone())
    }

    pub async fn replace_target_probe_context(&self, host: Option<HostProbeContext>) {
        self.state.lock().await.target_probe_context = host;
    }

    pub async fn health_snapshot(&self) -> XcodeBrokerHealthSnapshot {
        let backend_session_count = match self.backend.as_ref() {
            Some(backend) => backend.active_backend_session_count().await.unwrap_or(0),
            None => 0,
        };
        let mut state_guard = self.state.lock().await;
        let active_leases = state_guard.leases.len();
        let now = Instant::now();
        let stale_lease_count = state_guard
            .leases
            .values()
            .filter(|lease| {
                lease.state == XcodeMcpLeaseState::Active
                    && now.duration_since(lease.last_activity_at)
                        >= XCODE_MCP_ACTIVE_LEASE_IDLE_TIMEOUT
            })
            .count();
        let queued_leases = self.queued_lease_count();
        let broker_disabled = self.broker_disabled();
        let backend_available = self.has_backend();
        let observation_persistence_failures = self
            .observation_persistence_failures
            .load(Ordering::Acquire);
        let state = if broker_disabled {
            XcodeBrokerHealthState::Disabled
        } else if !backend_available {
            XcodeBrokerHealthState::Failed
        } else if stale_lease_count > 0 {
            XcodeBrokerHealthState::Degraded
        } else if queued_leases > 0 || active_leases >= self.config.max_active_leases {
            XcodeBrokerHealthState::Degraded
        } else if observation_persistence_failures > 0 {
            XcodeBrokerHealthState::Degraded
        } else {
            XcodeBrokerHealthState::Healthy
        };
        if state_guard.health_last_state != Some(state)
            || state_guard.health_last_transition_at.is_empty()
        {
            state_guard.health_last_state = Some(state);
            state_guard.health_last_transition_at = Utc::now().to_rfc3339();
        }
        let last_transition_at = state_guard.health_last_transition_at.clone();
        let (reason_code, operator_message) = xcode_broker_health_reason(
            state,
            broker_disabled,
            backend_available,
            active_leases,
            stale_lease_count,
            queued_leases,
            self.config.max_active_leases,
            observation_persistence_failures,
        );
        let can_acquire_new_xcode_leases = state == XcodeBrokerHealthState::Healthy;

        XcodeBrokerHealthSnapshot {
            state,
            reason_code,
            can_acquire_new_xcode_leases,
            active_lease_count: active_leases,
            initialize_queue_depth: queued_leases,
            last_transition_at,
            operator_message,
            pool_id: self.config.pool_id.clone(),
            active_leases,
            queued_leases,
            max_active_leases: self.config.max_active_leases,
            max_queued_leases: self.config.max_queued_leases,
            broker_disabled,
            backend_available,
            observation_persistence_failures,
            stale_lease_count,
            backend_session_count,
            helper_cleanup_reaped_leases_total: state_guard.helper_cleanup_reaped_leases_total,
        }
    }

    pub async fn authorize_and_mark_lease_active(
        &self,
        lease_id: &str,
        authorization_header: Option<&str>,
    ) -> Result<XcodeBrokerHttpRouteState> {
        if self.broker_disabled() {
            bail!("xcode_mcp_broker_disabled: brokered Xcode MCP route is disabled");
        }
        let Some(actual_hash) = authorization_hash_from_header(authorization_header) else {
            bail!("xcode_mcp_unauthorized: missing or invalid Xcode MCP lease bearer token");
        };

        let (agent_execution_id, observation, route_state) = {
            let mut state = self.state.lock().await;
            let sibling_leases = state.leases.len().saturating_sub(1) as i64;
            let Some(lease) = state.leases.get_mut(lease_id) else {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is not available");
            };
            if lease.authorization_hash != actual_hash {
                bail!("xcode_mcp_unauthorized: bearer token does not match lease '{lease_id}'");
            }
            if lease.state == XcodeMcpLeaseState::Closing {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is closing");
            }
            if lease.state == XcodeMcpLeaseState::Active {
                lease.last_activity_at = Instant::now();
                return Ok(XcodeBrokerHttpRouteState::AlreadyActive);
            }
            lease.state = XcodeMcpLeaseState::Active;
            lease.last_activity_at = Instant::now();
            (
                lease.agent_execution_id,
                self.lease_observation(
                    lease_id.to_string(),
                    lease.endpoint.clone(),
                    lease.agent_execution_id,
                    "lease_active",
                    None,
                    format!(
                        "Activated brokered Xcode MCP lease '{lease_id}' after first HTTP connect"
                    ),
                    Some(sibling_leases),
                ),
                XcodeBrokerHttpRouteState::Activated,
            )
        };

        self.record_observation(agent_execution_id, observation)
            .await;
        Ok(route_state)
    }

    pub async fn authorize_json_rpc_request(
        &self,
        lease_id: &str,
        request: &serde_json::Value,
    ) -> Result<()> {
        if request.get("method").and_then(|value| value.as_str()) != Some("tools/call") {
            return Ok(());
        }
        let tool_name = request
            .get("params")
            .and_then(|params| params.get("name").or_else(|| params.get("toolName")))
            .and_then(|name| name.as_str())
            .unwrap_or("");
        if tool_name.is_empty() {
            bail!("xcode_mcp_tool_denied: tools/call request is missing params.name");
        }

        let (agent_execution_id, endpoint, policy) = {
            let state = self.state.lock().await;
            let Some(lease) = state.leases.get(lease_id) else {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is not available");
            };
            (
                lease.agent_execution_id,
                lease.endpoint.clone(),
                lease.mcp_policy.clone(),
            )
        };

        match policy.authorize_tools_call(tool_name) {
            BrokerMcpPolicyDecision::Allow => Ok(()),
            BrokerMcpPolicyDecision::Deny { reason } => {
                let observation = self.lease_observation(
                    lease_id.to_string(),
                    endpoint,
                    agent_execution_id,
                    "tool_call_denied",
                    None,
                    format!("Denied brokered Xcode MCP tools/call for '{tool_name}': {reason}"),
                    None,
                );
                self.record_observation(agent_execution_id, observation)
                    .await;
                bail!("xcode_mcp_tool_denied: {reason}");
            }
        }
    }

    pub async fn filter_tools_list_result(
        &self,
        lease_id: &str,
        result: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let policy = {
            let state = self.state.lock().await;
            let Some(lease) = state.leases.get(lease_id) else {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is not available");
            };
            lease.mcp_policy.clone()
        };
        Ok(policy.filter_tools_list(result))
    }

    pub async fn forward_json_rpc_request(
        &self,
        lease_id: &str,
        request: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let Some(backend) = self.backend.clone() else {
            bail!("xcode_mcp_backend_unavailable: backend process routing is not yet attached");
        };
        let context = {
            let mut state = self.state.lock().await;
            let Some(lease) = state.leases.get_mut(lease_id) else {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is not available");
            };
            lease.last_activity_at = Instant::now();
            XcodeMcpBackendRequestContext {
                run_id: lease.run_id,
                lease_id: lease_id.to_string(),
                endpoint: lease.endpoint.clone(),
                agent_execution_id: lease.agent_execution_id,
                target_snapshot: lease.target_snapshot.clone(),
            }
        };
        let method = request
            .get("method")
            .and_then(|method| method.as_str())
            .map(str::to_string);
        let response = if method.as_deref() == Some("initialize") {
            self.forward_initialize_json_rpc_request(backend, context, request)
                .await?
        } else {
            self.forward_backend_json_rpc_request(backend, context, request, method.as_deref())
                .await?
        };
        if method.as_deref() == Some("tools/list") {
            self.filter_tools_list_response(lease_id, response).await
        } else {
            Ok(response)
        }
    }

    async fn forward_initialize_json_rpc_request(
        &self,
        backend: Arc<dyn XcodeMcpBackend>,
        context: XcodeMcpBackendRequestContext,
        request: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let lock_key = initialize_lock_key(&context);
        let lock = self.initialize_lock_for(&lock_key).await;
        let wait_started = Instant::now();
        let lock_guard = if self.config.spawn_init_timeout <= XCODE_MCP_ACTION_REQUIRED_AFTER {
            match tokio::time::timeout(self.config.spawn_init_timeout, lock.lock()).await {
                Ok(lock_guard) => lock_guard,
                Err(_) => {
                    let observation = self.initialize_observation(
                        &context,
                        "initialize_lock_timeout",
                        Some(XcodeRuntimeFailureClass::XcodeMcpInitializeTimeout),
                        self.config.spawn_init_timeout.as_millis() as i64,
                        format!("Timed out waiting for Xcode MCP initialize lock '{lock_key}'"),
                    );
                    self.record_observation(context.agent_execution_id, observation)
                        .await;
                    bail!(
                        "xcode_mcp_initialize_timeout: timed out after {:?} waiting for initialize lock '{}'",
                        self.config.spawn_init_timeout,
                        lock_key
                    );
                }
            }
        } else {
            tokio::select! {
                lock_guard = lock.lock() => lock_guard,
                _ = tokio::time::sleep(XCODE_MCP_ACTION_REQUIRED_AFTER) => {
                    let wait_ms = wait_started.elapsed().as_millis() as i64;
                    let observation = self.initialize_observation(
                        &context,
                        "initialize_action_required",
                        Some(XcodeRuntimeFailureClass::XcodeMcpActionRequired),
                        wait_ms,
                        format!(
                            "Action Required: Check Xcode after waiting {} ms for initialize lock '{}'",
                            wait_ms, lock_key
                        ),
                    );
                    self.record_observation(context.agent_execution_id, observation)
                        .await;

                    let remaining = self
                        .config
                        .spawn_init_timeout
                        .saturating_sub(wait_started.elapsed());
                    match tokio::time::timeout(remaining, lock.lock()).await {
                        Ok(lock_guard) => lock_guard,
                        Err(_) => {
                            let observation = self.initialize_observation(
                                &context,
                                "initialize_lock_timeout",
                                Some(XcodeRuntimeFailureClass::XcodeMcpInitializeTimeout),
                                self.config.spawn_init_timeout.as_millis() as i64,
                                format!("Timed out waiting for Xcode MCP initialize lock '{lock_key}'"),
                            );
                            self.record_observation(context.agent_execution_id, observation)
                                .await;
                            bail!(
                                "xcode_mcp_initialize_timeout: timed out after {:?} waiting for initialize lock '{}'",
                                self.config.spawn_init_timeout,
                                lock_key
                            );
                        }
                    }
                }
            }
        };

        let wait_ms = wait_started.elapsed().as_millis() as i64;
        let observation = self.initialize_observation(
            &context,
            "initialize_lock_acquired",
            None,
            wait_ms,
            format!(
                "Forwarding brokered Xcode MCP initialize for lease '{}' after waiting {} ms",
                context.lease_id, wait_ms
            ),
        );
        self.record_observation(context.agent_execution_id, observation)
            .await;

        let response = self
            .forward_backend_json_rpc_request(backend, context, request, Some("initialize"))
            .await;
        drop(lock_guard);
        response
    }

    async fn forward_backend_json_rpc_request(
        &self,
        backend: Arc<dyn XcodeMcpBackend>,
        context: XcodeMcpBackendRequestContext,
        request: serde_json::Value,
        method: Option<&str>,
    ) -> Result<serde_json::Value> {
        let method = method
            .filter(|method| !method.is_empty())
            .unwrap_or("<missing-method>")
            .to_string();
        let started = Instant::now();
        let response = backend.forward_json_rpc(context.clone(), request).await;
        let latency_ms = started.elapsed().as_millis() as i64;
        let backend_process_id = backend.backend_process_id(&context.lease_id).await;
        let observation = match &response {
            Ok(_) => self.backend_request_observation(
                &context,
                "backend_request_completed",
                None,
                latency_ms,
                backend_process_id,
                format!(
                    "Brokered Xcode MCP backend completed method '{}' for lease '{}' in {} ms",
                    method, context.lease_id, latency_ms
                ),
            ),
            Err(error) => self.backend_request_observation(
                &context,
                "backend_request_failed",
                Some(xcode_failure_class_from_backend_error(error)),
                latency_ms,
                backend_process_id,
                format!(
                    "Brokered Xcode MCP backend failed method '{}' for lease '{}' after {} ms: {}",
                    method, context.lease_id, latency_ms, error
                ),
            ),
        };
        self.record_observation(context.agent_execution_id, observation)
            .await;
        response
    }

    async fn initialize_lock_for(&self, lock_key: &str) -> Arc<Mutex<()>> {
        let mut locks = self.initialize_locks.lock().await;
        locks
            .entry(lock_key.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    async fn filter_tools_list_response(
        &self,
        lease_id: &str,
        mut response: serde_json::Value,
    ) -> Result<serde_json::Value> {
        if let Some(result) = response.get_mut("result") {
            let filtered = self
                .filter_tools_list_result(lease_id, std::mem::take(result))
                .await?;
            *result = filtered;
            return Ok(response);
        }
        self.filter_tools_list_result(lease_id, response).await
    }

    pub async fn mark_lease_active(&self, lease_id: &str) -> Result<()> {
        let (agent_execution_id, observation) = {
            let mut state = self.state.lock().await;
            let sibling_leases = state.leases.len().saturating_sub(1) as i64;
            let Some(lease) = state.leases.get_mut(lease_id) else {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is not available");
            };
            if lease.state == XcodeMcpLeaseState::Active {
                lease.last_activity_at = Instant::now();
                return Ok(());
            }
            if lease.state == XcodeMcpLeaseState::Closing {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is closing");
            }
            lease.state = XcodeMcpLeaseState::Active;
            lease.last_activity_at = Instant::now();
            (
                lease.agent_execution_id,
                self.lease_observation(
                    lease_id.to_string(),
                    lease.endpoint.clone(),
                    lease.agent_execution_id,
                    "lease_active",
                    None,
                    format!(
                        "Activated brokered Xcode MCP lease '{lease_id}' after first HTTP connect"
                    ),
                    Some(sibling_leases),
                ),
            )
        };
        self.record_observation(agent_execution_id, observation)
            .await;
        Ok(())
    }

    pub async fn cleanup_first_connect_timeouts(&self) -> Result<Vec<String>> {
        let now = Instant::now();
        let mut observations = Vec::new();
        let mut expired_lease_ids = Vec::new();
        {
            let mut state = self.state.lock().await;
            let expired = state
                .leases
                .iter()
                .filter_map(|(lease_id, lease)| {
                    (lease.state == XcodeMcpLeaseState::Reserved
                        && lease.first_connect_deadline <= now)
                        .then(|| lease_id.clone())
                })
                .collect::<Vec<_>>();

            for lease_id in expired {
                let Some(lease) = state.leases.remove(&lease_id) else {
                    continue;
                };
                observations.push((
                    lease.agent_execution_id,
                    self.lease_observation(
                        lease_id.clone(),
                        lease.endpoint,
                        lease.agent_execution_id,
                        "first_connect_timeout",
                        Some(XcodeRuntimeFailureClass::XcodeMcpFirstConnectTimeout),
                        format!(
                            "Released brokered Xcode MCP lease '{lease_id}' after first-connect timeout"
                        ),
                        Some(state.leases.len() as i64),
                    ),
                ));
                expired_lease_ids.push(lease_id);
            }
        }

        for (agent_execution_id, observation) in observations {
            self.record_observation(agent_execution_id, observation)
                .await;
        }

        if !expired_lease_ids.is_empty() {
            self.queue_notify.notify_waiters();
        }

        Ok(expired_lease_ids)
    }

    pub async fn cleanup_pid_drift(&self) -> Result<Vec<String>> {
        let host = {
            let state = self.state.lock().await;
            state.target_probe_context.clone()
        };
        let Some(host) = host else {
            return Ok(Vec::new());
        };

        let mut observations = Vec::new();
        let mut drifted_lease_ids = Vec::new();
        {
            let mut state = self.state.lock().await;
            let drifted = state
                .leases
                .iter()
                .filter_map(|(lease_id, lease)| {
                    lease
                        .target_snapshot
                        .as_ref()
                        .is_some_and(|snapshot| !target_snapshot_matches_host(snapshot, &host))
                        .then(|| lease_id.clone())
                })
                .collect::<Vec<_>>();

            for lease_id in drifted {
                let Some(lease) = state.leases.remove(&lease_id) else {
                    continue;
                };
                observations.push((
                    lease.agent_execution_id,
                    self.lease_observation_with_target(
                        lease_id.clone(),
                        lease.endpoint,
                        lease.agent_execution_id,
                        "pool_pid_drift",
                        Some(XcodeRuntimeFailureClass::PoolPidDrift),
                        format!(
                            "Closed brokered Xcode MCP lease '{lease_id}' because selected Xcode pid drifted"
                        ),
                        Some(state.leases.len() as i64),
                        lease.target_snapshot.as_ref(),
                    ),
                ));
                drifted_lease_ids.push(lease_id);
            }
        }

        for (agent_execution_id, observation) in observations {
            self.record_observation(agent_execution_id, observation)
                .await;
        }

        if !drifted_lease_ids.is_empty() {
            self.release_backend_leases(&drifted_lease_ids).await;
            self.queue_notify.notify_waiters();
        }

        Ok(drifted_lease_ids)
    }

    pub async fn cleanup_idle_active_leases(&self) -> Result<Vec<String>> {
        self.cleanup_idle_active_leases_older_than(XCODE_MCP_ACTIVE_LEASE_IDLE_TIMEOUT)
            .await
    }

    pub async fn cleanup_idle_active_leases_older_than(
        &self,
        idle_timeout: Duration,
    ) -> Result<Vec<String>> {
        let now = Instant::now();
        let mut observations = Vec::new();
        let mut stale_lease_ids = Vec::new();
        {
            let mut state = self.state.lock().await;
            let stale = state
                .leases
                .iter()
                .filter_map(|(lease_id, lease)| {
                    (lease.state == XcodeMcpLeaseState::Active
                        && now.duration_since(lease.last_activity_at) >= idle_timeout)
                        .then(|| lease_id.clone())
                })
                .collect::<Vec<_>>();

            for lease_id in stale {
                let Some(lease) = state.leases.remove(&lease_id) else {
                    continue;
                };
                let idle_ms = now.duration_since(lease.last_activity_at).as_millis() as i64;
                observations.push((
                    lease.agent_execution_id,
                    self.lease_observation_with_target(
                        lease_id.clone(),
                        lease.endpoint,
                        lease.agent_execution_id,
                        "lease_idle_timeout",
                        Some(XcodeRuntimeFailureClass::BrokerInfrastructure),
                        format!(
                            "Released brokered Xcode MCP lease '{lease_id}' after {idle_ms} ms without provider activity"
                        ),
                        Some(state.leases.len() as i64),
                        lease.target_snapshot.as_ref(),
                    ),
                ));
                stale_lease_ids.push(lease_id);
            }
            state.helper_cleanup_reaped_leases_total += stale_lease_ids.len() as u64;
        }

        for (agent_execution_id, observation) in observations {
            self.record_observation(agent_execution_id, observation)
                .await;
        }

        if !stale_lease_ids.is_empty() {
            self.release_backend_leases(&stale_lease_ids).await;
            self.queue_notify.notify_waiters();
        }

        Ok(stale_lease_ids)
    }

    fn broker_disabled(&self) -> bool {
        self.config.broker_disabled || env_flag_enabled("CHAINWORKS_XCODE_BROKER_DISABLED")
    }

    fn try_acquire_queue_permit(&self, lease_count: usize) -> Option<QueueLeasePermit<'_>> {
        loop {
            let current = self.queued_leases.load(Ordering::Acquire);
            let next = current.checked_add(lease_count)?;
            if next > self.config.max_queued_leases {
                return None;
            }
            if self
                .queued_leases
                .compare_exchange(current, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(QueueLeasePermit {
                    queued_leases: &self.queued_leases,
                    lease_count,
                });
            }
        }
    }

    fn endpoint_for(&self, lease_id: &str) -> String {
        format!(
            "{}/{}",
            self.config.base_url.trim_end_matches('/'),
            lease_id
        )
    }

    async fn record_observation(
        &self,
        agent_execution_id: Option<AgentExecutionId>,
        observation: McpBrokerObservation,
    ) {
        let Some(agent_execution_id) = agent_execution_id else {
            return;
        };
        if let Err(err) = self
            .observation_sink
            .append_xcode_runtime_observation(
                agent_execution_id,
                XcodeRuntimeObservationUpdate::McpBrokerObservation(observation),
            )
            .await
        {
            self.observation_persistence_failures
                .fetch_add(1, Ordering::AcqRel);
            error!(
                agent_execution_id = %agent_execution_id,
                error = %err,
                metric = "xcode_observation_persist_failed_total",
                warning = "observation_persistence_degraded",
                "Failed to persist Xcode broker pool observation"
            );
        }
    }

    async fn release_backend_leases(&self, lease_ids: &[String]) {
        let Some(backend) = self.backend.as_ref() else {
            return;
        };
        for lease_id in lease_ids {
            if let Err(err) = backend.release_lease(lease_id).await {
                warn!(
                    lease_id = %lease_id,
                    error = %err,
                    "Failed to release Xcode MCP backend lease"
                );
            }
        }
    }

    fn lease_observation(
        &self,
        lease_id: String,
        endpoint: String,
        agent_execution_id: Option<AgentExecutionId>,
        disposition: impl Into<String>,
        failure_class: Option<XcodeRuntimeFailureClass>,
        status_update: impl Into<String>,
        sibling_leases_at_spawn: Option<i64>,
    ) -> McpBrokerObservation {
        McpBrokerObservation {
            source: "xcode_mcp_broker".to_string(),
            backend_start_disposition: disposition.into(),
            pool_id: Some(self.config.pool_id.clone()),
            lease_id: Some(lease_id),
            xcode_pid: None,
            backend_process_id: None,
            http_endpoint: Some(endpoint),
            xcode_home_disposition: None,
            xcode_tmpdir_disposition: None,
            simulator_selection: None,
            sibling_leases_at_spawn,
            backend_initialize_wait_ms: None,
            backend_startup_latency_ms: None,
            http_session_startup_latency_ms: None,
            backend_failure_class: failure_class,
            originating_execution_id: agent_execution_id.map(|id| id.to_string()),
            prompt_cycle_index: None,
            status_update: Some(status_update.into()),
        }
    }

    fn lease_observation_with_target(
        &self,
        lease_id: String,
        endpoint: String,
        agent_execution_id: Option<AgentExecutionId>,
        disposition: impl Into<String>,
        failure_class: Option<XcodeRuntimeFailureClass>,
        status_update: impl Into<String>,
        sibling_leases_at_spawn: Option<i64>,
        target_snapshot: Option<&XcodeTargetSnapshot>,
    ) -> McpBrokerObservation {
        let mut observation = self.lease_observation(
            lease_id,
            endpoint,
            agent_execution_id,
            disposition,
            failure_class,
            status_update,
            sibling_leases_at_spawn,
        );
        apply_target_snapshot_to_observation(&mut observation, target_snapshot);
        observation
    }

    fn initialize_observation(
        &self,
        context: &XcodeMcpBackendRequestContext,
        disposition: impl Into<String>,
        failure_class: Option<XcodeRuntimeFailureClass>,
        wait_ms: i64,
        status_update: impl Into<String>,
    ) -> McpBrokerObservation {
        let mut observation = self.lease_observation_with_target(
            context.lease_id.clone(),
            context.endpoint.clone(),
            context.agent_execution_id,
            disposition,
            failure_class,
            status_update,
            None,
            context.target_snapshot.as_ref(),
        );
        observation.backend_initialize_wait_ms = Some(wait_ms);
        observation
    }

    fn capacity_observation(
        &self,
        agent_execution_id: Option<AgentExecutionId>,
        requested: usize,
        active: usize,
    ) -> McpBrokerObservation {
        McpBrokerObservation {
            source: "xcode_mcp_broker".to_string(),
            backend_start_disposition: "capacity_rejected".to_string(),
            pool_id: Some(self.config.pool_id.clone()),
            lease_id: None,
            xcode_pid: None,
            backend_process_id: None,
            http_endpoint: None,
            xcode_home_disposition: None,
            xcode_tmpdir_disposition: None,
            simulator_selection: None,
            sibling_leases_at_spawn: Some(active as i64),
            backend_initialize_wait_ms: None,
            backend_startup_latency_ms: None,
            http_session_startup_latency_ms: None,
            backend_failure_class: Some(XcodeRuntimeFailureClass::XcodeMcpCapacityExhausted),
            originating_execution_id: agent_execution_id.map(|id| id.to_string()),
            prompt_cycle_index: None,
            status_update: Some(format!(
                "Rejected {requested} Xcode MCP lease request(s): active lease capacity {} exhausted",
                self.config.max_active_leases
            )),
        }
    }

    fn disabled_observation(
        &self,
        agent_execution_id: Option<AgentExecutionId>,
        requested: usize,
        active: usize,
    ) -> McpBrokerObservation {
        McpBrokerObservation {
            source: "xcode_mcp_broker".to_string(),
            backend_start_disposition: "broker_disabled".to_string(),
            pool_id: Some(self.config.pool_id.clone()),
            lease_id: None,
            xcode_pid: None,
            backend_process_id: None,
            http_endpoint: None,
            xcode_home_disposition: None,
            xcode_tmpdir_disposition: None,
            simulator_selection: None,
            sibling_leases_at_spawn: Some(active as i64),
            backend_initialize_wait_ms: None,
            backend_startup_latency_ms: None,
            http_session_startup_latency_ms: None,
            backend_failure_class: Some(XcodeRuntimeFailureClass::BrokerInfrastructure),
            originating_execution_id: agent_execution_id.map(|id| id.to_string()),
            prompt_cycle_index: None,
            status_update: Some(format!(
                "Rejected {requested} brokered Xcode MCP lease request(s): CHAINWORKS_XCODE_BROKER_DISABLED is enabled"
            )),
        }
    }

    fn queue_observation(
        &self,
        agent_execution_id: Option<AgentExecutionId>,
        requested: usize,
        active: usize,
        disposition: impl Into<String>,
        failure_class: Option<XcodeRuntimeFailureClass>,
        status_update: impl Into<String>,
    ) -> McpBrokerObservation {
        McpBrokerObservation {
            source: "xcode_mcp_broker".to_string(),
            backend_start_disposition: disposition.into(),
            pool_id: Some(self.config.pool_id.clone()),
            lease_id: None,
            xcode_pid: None,
            backend_process_id: None,
            http_endpoint: None,
            xcode_home_disposition: None,
            xcode_tmpdir_disposition: None,
            simulator_selection: None,
            sibling_leases_at_spawn: Some(active as i64),
            backend_initialize_wait_ms: Some(self.config.spawn_init_timeout.as_millis() as i64),
            backend_startup_latency_ms: None,
            http_session_startup_latency_ms: None,
            backend_failure_class: failure_class,
            originating_execution_id: agent_execution_id.map(|id| id.to_string()),
            prompt_cycle_index: None,
            status_update: Some(format!(
                "{} (requested {} lease(s), max queued {})",
                status_update.into(),
                requested,
                self.config.max_queued_leases
            )),
        }
    }

    fn target_resolution_failed_observation(
        &self,
        agent_execution_id: Option<AgentExecutionId>,
        failure_class: XcodeRuntimeFailureClass,
        error: &anyhow::Error,
    ) -> McpBrokerObservation {
        McpBrokerObservation {
            source: "xcode_mcp_broker".to_string(),
            backend_start_disposition: "target_resolution_failed".to_string(),
            pool_id: Some(self.config.pool_id.clone()),
            lease_id: None,
            xcode_pid: None,
            backend_process_id: None,
            http_endpoint: None,
            xcode_home_disposition: None,
            xcode_tmpdir_disposition: None,
            simulator_selection: None,
            sibling_leases_at_spawn: None,
            backend_initialize_wait_ms: None,
            backend_startup_latency_ms: None,
            http_session_startup_latency_ms: None,
            backend_failure_class: Some(failure_class),
            originating_execution_id: agent_execution_id.map(|id| id.to_string()),
            prompt_cycle_index: None,
            status_update: Some(format!(
                "Failed to resolve Xcode target before reserving broker lease: {error}"
            )),
        }
    }

    fn backend_request_observation(
        &self,
        context: &XcodeMcpBackendRequestContext,
        disposition: impl Into<String>,
        failure_class: Option<XcodeRuntimeFailureClass>,
        latency_ms: i64,
        backend_process_id: Option<i64>,
        status_update: impl Into<String>,
    ) -> McpBrokerObservation {
        let mut observation = self.lease_observation_with_target(
            context.lease_id.clone(),
            context.endpoint.clone(),
            context.agent_execution_id,
            disposition,
            failure_class,
            status_update,
            None,
            context.target_snapshot.as_ref(),
        );
        observation.backend_startup_latency_ms = Some(latency_ms);
        observation.backend_process_id = backend_process_id;
        observation
    }

    async fn resolve_target_snapshots(
        &self,
        req: &ExecutionRequest,
        requested: &[&BrokeredXcodeMcpIntent],
    ) -> Result<Vec<Option<XcodeTargetSnapshot>>> {
        let host = if self.config.use_local_host_probe {
            Some(probe_local_xcode_host(LocalXcodeHostProbeConfig {
                workspace_roots: workspace_roots_for_target_probe(req, requested),
                ..LocalXcodeHostProbeConfig::default()
            }))
        } else {
            let state = self.state.lock().await;
            state.target_probe_context.clone()
        };
        let Some(host) = host else {
            return Ok(vec![None; requested.len()]);
        };
        let broker_contract_hash = broker_contract_hash_for_intents(requested);
        let resolver = XcodeTargetResolver;
        requested
            .iter()
            .map(|intent| {
                let input = XcodeTargetSelectionInput {
                    workspace_root: intent
                        .workspace_root
                        .clone()
                        .unwrap_or_else(|| req.workspace_root.clone()),
                    runtime_profile_id: intent.runtime_profile_id.clone(),
                    xcode_pid_selector: intent.xcode_pid_selector.clone(),
                    permission_profile_id: intent.permission_profile_id.clone(),
                    broker_contract_hash: broker_contract_hash.clone(),
                };
                resolver.resolve(&input, &host).map(Some)
            })
            .collect()
    }
}

#[async_trait]
impl XcodeBrokerLeaseAttacher for XcodeMcpBridgePool {
    async fn attach_brokered_xcode_leases(
        &self,
        req: &ExecutionRequest,
    ) -> Result<BrokeredXcodeLeaseAttachment> {
        let requested = req.brokered_xcode_intents();
        if requested.is_empty() {
            return Ok(BrokeredXcodeLeaseAttachment::new(req.clone()));
        }
        let requested_count = requested.len();
        if self.broker_disabled() {
            let active = self.active_lease_count().await;
            let observation =
                self.disabled_observation(req.agent_execution_id, requested_count, active);
            self.record_observation(req.agent_execution_id, observation)
                .await;
            bail!(
                "xcode_mcp_broker_disabled: brokered Xcode MCP is disabled by CHAINWORKS_XCODE_BROKER_DISABLED=1"
            );
        }

        let target_snapshots = match self.resolve_target_snapshots(req, &requested).await {
            Ok(target_snapshots) => target_snapshots,
            Err(err) => {
                let failure_class = target_resolver_failure_class(&err);
                let observation = self.target_resolution_failed_observation(
                    req.agent_execution_id,
                    failure_class,
                    &err,
                );
                self.record_observation(req.agent_execution_id, observation)
                    .await;
                return Err(err);
            }
        };

        let mut replacements: HashMap<usize, AcpMcpServerPayload> = HashMap::new();
        let mut observations = Vec::new();
        let mut lease_ids = Vec::new();
        let mut queue_permit = None;
        let queue_deadline = tokio::time::Instant::now() + self.config.queue_timeout;

        loop {
            let capacity_available = self.queue_notify.notified();
            let mut state = self.state.lock().await;
            let active = state.leases.len();
            if active + requested_count > self.config.max_active_leases {
                drop(state);

                if requested_count > self.config.max_active_leases {
                    let observation =
                        self.capacity_observation(req.agent_execution_id, requested_count, active);
                    self.record_observation(req.agent_execution_id, observation)
                        .await;
                    bail!(
                        "xcode_mcp_capacity_exhausted: requested {} brokered Xcode MCP lease(s) exceeds capacity {}",
                        requested_count,
                        self.config.max_active_leases
                    );
                }

                if queue_permit.is_none() {
                    let Some(permit) = self.try_acquire_queue_permit(requested_count) else {
                        let observation = self.capacity_observation(
                            req.agent_execution_id,
                            requested_count,
                            active,
                        );
                        self.record_observation(req.agent_execution_id, observation)
                            .await;
                        bail!(
                            "xcode_mcp_capacity_exhausted: requested {} brokered Xcode MCP lease(s) with {} active, {} queued, capacity {}, and queue capacity {}",
                            requested_count,
                            active,
                            self.queued_lease_count(),
                            self.config.max_active_leases,
                            self.config.max_queued_leases
                        );
                    };
                    queue_permit = Some(permit);
                    let observation = self.queue_observation(
                        req.agent_execution_id,
                        requested_count,
                        active,
                        "queue_waiting",
                        None,
                        "Waiting for Xcode MCP bridge lease capacity",
                    );
                    self.record_observation(req.agent_execution_id, observation)
                        .await;
                    continue;
                }

                if tokio::time::timeout_at(queue_deadline, capacity_available)
                    .await
                    .is_err()
                {
                    let observation = self.queue_observation(
                        req.agent_execution_id,
                        requested_count,
                        active,
                        "queue_timeout",
                        Some(XcodeRuntimeFailureClass::XcodeMcpCapacityExhausted),
                        "Timed out waiting for Xcode MCP bridge lease capacity",
                    );
                    self.record_observation(req.agent_execution_id, observation)
                        .await;
                    bail!(
                        "xcode_mcp_capacity_exhausted: timed out after {:?} waiting for {} brokered Xcode MCP lease(s)",
                        self.config.queue_timeout,
                        requested_count
                    );
                }

                continue;
            }

            let mut target_index = 0usize;
            for (index, server) in req.mcp_servers.iter().enumerate() {
                let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } = &server.transport
                else {
                    continue;
                };
                let target_snapshot = target_snapshots.get(target_index).cloned().flatten();
                target_index += 1;
                let lease_id = format!("lease-{}", Uuid::new_v4());
                let endpoint = self.endpoint_for(&lease_id);
                let bearer_token = format!("xcode-lease-{}", Uuid::new_v4());
                let mut headers = BTreeMap::new();
                headers.insert(
                    "Authorization".to_string(),
                    format!("Bearer {bearer_token}"),
                );

                state.leases.insert(
                    lease_id.clone(),
                    LeaseRecord {
                        run_id: req.run_id,
                        agent_execution_id: req.agent_execution_id,
                        endpoint: endpoint.clone(),
                        authorization_hash: hash_secret(&bearer_token),
                        state: XcodeMcpLeaseState::Reserved,
                        last_activity_at: Instant::now(),
                        first_connect_deadline: Instant::now() + self.config.first_connect_timeout,
                        mcp_policy: BrokerMcpPolicy::from_intent(
                            intent,
                            &self.config.tool_allowlists_by_hash,
                        ),
                        target_snapshot: target_snapshot.clone(),
                    },
                );
                let sibling_leases = state.leases.len().saturating_sub(1) as i64;
                observations.push(self.lease_observation_with_target(
                    lease_id.clone(),
                    endpoint.clone(),
                    req.agent_execution_id,
                    "lease_reserved",
                    None,
                    lease_reserved_status(&lease_id, &intent.runtime_id, target_snapshot.as_ref()),
                    Some(sibling_leases),
                    target_snapshot.as_ref(),
                ));
                lease_ids.push(lease_id);
                replacements.insert(
                    index,
                    AcpMcpServerPayload {
                        id: intent.runtime_id.clone(),
                        extension_id: intent.extension_id.clone(),
                        transport: ResolvedMcpServerTransport::Http {
                            url: endpoint,
                            headers,
                        },
                    },
                );
            }
            break;
        }
        drop(queue_permit.take());

        for observation in observations {
            self.record_observation(req.agent_execution_id, observation)
                .await;
        }

        let mut attached = req.clone();
        for (index, replacement) in replacements {
            attached.mcp_servers[index] = replacement;
        }

        Ok(BrokeredXcodeLeaseAttachment {
            request: attached,
            lease_ids,
        })
    }

    async fn release_brokered_xcode_leases(&self, lease_ids: &[String]) -> Result<()> {
        let mut observations = Vec::new();
        let mut released_any = false;
        {
            let mut state = self.state.lock().await;
            for lease_id in lease_ids {
                let sibling_leases = state.leases.len().saturating_sub(1) as i64;
                let Some(lease) = state.leases.get_mut(lease_id) else {
                    continue;
                };
                lease.state = XcodeMcpLeaseState::Closing;
                observations.push((
                    lease.agent_execution_id,
                    self.lease_observation(
                        lease_id.clone(),
                        lease.endpoint.clone(),
                        lease.agent_execution_id,
                        "lease_closing",
                        None,
                        format!("Closing brokered Xcode MCP lease '{lease_id}'"),
                        Some(sibling_leases),
                    ),
                ));
                let Some(lease) = state.leases.remove(lease_id) else {
                    continue;
                };
                released_any = true;
                observations.push((
                    lease.agent_execution_id,
                    self.lease_observation(
                        lease_id.clone(),
                        lease.endpoint,
                        lease.agent_execution_id,
                        "lease_released",
                        None,
                        format!("Released brokered Xcode MCP lease '{lease_id}'"),
                        Some(state.leases.len() as i64),
                    ),
                ));
            }
        }

        for (agent_execution_id, observation) in observations {
            self.record_observation(agent_execution_id, observation)
                .await;
        }
        if released_any {
            self.release_backend_leases(lease_ids).await;
            self.queue_notify.notify_waiters();
        }

        Ok(())
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn authorization_hash_from_header(header: Option<&str>) -> Option<String> {
    let value = header?.trim();
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))?
        .trim();
    (!token.is_empty()).then(|| hash_secret(token))
}

fn broker_contract_hash_for_intents(intents: &[&BrokeredXcodeMcpIntent]) -> String {
    let mut intents = intents
        .iter()
        .map(|intent| (*intent).clone())
        .collect::<Vec<_>>();
    intents.sort_by(|a, b| {
        (
            &a.extension_id,
            &a.runtime_id,
            &a.server_id,
            &a.workspace_root,
            &a.xcode_pid_selector,
            &a.runtime_profile_id,
            &a.permission_profile_id,
            &a.resolved_tool_allowlist_hash,
            a.provider_http_required,
        )
            .cmp(&(
                &b.extension_id,
                &b.runtime_id,
                &b.server_id,
                &b.workspace_root,
                &b.xcode_pid_selector,
                &b.runtime_profile_id,
                &b.permission_profile_id,
                &b.resolved_tool_allowlist_hash,
                b.provider_http_required,
            ))
    });
    let raw = serde_json::to_vec(&serde_json::json!({
        "version": 1,
        "xcode_broker_intents": intents,
    }))
    .expect("Xcode broker contract payload should serialize");
    format!("{:x}", Sha256::digest(raw))
}

fn workspace_roots_for_target_probe(
    req: &ExecutionRequest,
    intents: &[&BrokeredXcodeMcpIntent],
) -> Vec<String> {
    let mut roots = BTreeSet::new();
    if !req.workspace_root.is_empty() {
        roots.insert(req.workspace_root.clone());
    }
    for intent in intents {
        if let Some(workspace_root) = intent
            .workspace_root
            .as_ref()
            .filter(|workspace_root| !workspace_root.is_empty())
        {
            roots.insert(workspace_root.clone());
        }
        if let Some(workspace_identity) = intent
            .xcode_pid_selector
            .as_deref()
            .and_then(|selector| selector.strip_prefix("workspace:"))
            .filter(|workspace_identity| !workspace_identity.is_empty())
        {
            roots.insert(workspace_identity.to_string());
        }
    }
    roots.into_iter().collect()
}

fn lease_reserved_status(
    lease_id: &str,
    runtime_id: &str,
    target_snapshot: Option<&XcodeTargetSnapshot>,
) -> String {
    match target_snapshot {
        Some(snapshot) => format!(
            "Reserved brokered Xcode MCP lease '{lease_id}' for runtime '{runtime_id}' targeting Xcode pid {} workspace '{}'",
            snapshot.xcode_pid, snapshot.workspace_identity
        ),
        None => format!("Reserved brokered Xcode MCP lease '{lease_id}' for runtime '{runtime_id}'"),
    }
}

fn apply_target_snapshot_to_observation(
    observation: &mut McpBrokerObservation,
    target_snapshot: Option<&XcodeTargetSnapshot>,
) {
    let Some(snapshot) = target_snapshot else {
        return;
    };
    observation.xcode_pid = Some(snapshot.xcode_pid.to_string());
    observation.xcode_home_disposition = Some("host_operator_home_available".to_string());
    observation.xcode_tmpdir_disposition = Some("darwin_tmpdir_available".to_string());
}

fn xcode_failure_class_from_backend_error(error: &anyhow::Error) -> XcodeRuntimeFailureClass {
    let message = error.to_string();
    if message.contains("xcode_mcp_initialize_timeout") {
        XcodeRuntimeFailureClass::XcodeMcpInitializeTimeout
    } else if message.contains("xcode_mcp_action_required") {
        XcodeRuntimeFailureClass::XcodeMcpActionRequired
    } else if message.contains("xcode_mcp_first_connect_timeout") {
        XcodeRuntimeFailureClass::XcodeMcpFirstConnectTimeout
    } else if message.contains("pool_pid_drift") {
        XcodeRuntimeFailureClass::PoolPidDrift
    } else if message.contains("host_env_unavailable") {
        XcodeRuntimeFailureClass::HostEnvUnavailable
    } else if message.contains("xcode_mcp_backend")
        || message.contains("backend process")
        || message.contains("backend response")
    {
        XcodeRuntimeFailureClass::PerLeaseBackend
    } else {
        XcodeRuntimeFailureClass::BrokerInfrastructure
    }
}

fn initialize_lock_key(context: &XcodeMcpBackendRequestContext) -> String {
    context
        .target_snapshot
        .as_ref()
        .map(|snapshot| format!("xcode-pid:{}", snapshot.xcode_pid))
        .unwrap_or_else(|| "xcode-pid:unresolved".to_string())
}

fn backend_session_key(context: &XcodeMcpBackendRequestContext) -> String {
    context
        .target_snapshot
        .as_ref()
        .map(|snapshot| {
            format!(
                "run:{}:xcode-pid:{}:developer-dir:{}",
                context.run_id, snapshot.xcode_pid, snapshot.developer_dir
            )
        })
        .unwrap_or_else(|| format!("lease:{}", context.lease_id))
}

fn target_snapshot_matches_host(snapshot: &XcodeTargetSnapshot, host: &HostProbeContext) -> bool {
    host.candidate_xcodes.iter().any(|candidate| {
        candidate.alive
            && candidate.pid == snapshot.xcode_pid
            && host
                .expected_gui_uid
                .is_none_or(|expected_uid| candidate.uid == expected_uid)
            && match snapshot.selection_confidence {
                XcodeTargetSelectionConfidence::ExplicitPid => candidate
                    .workspace_identity
                    .as_deref()
                    .is_none_or(|workspace| workspace == snapshot.workspace_identity),
                XcodeTargetSelectionConfidence::WorkspaceMatch => candidate
                    .workspace_identity
                    .as_deref()
                    .is_some_and(|workspace| workspace == snapshot.workspace_identity),
            }
    })
}

fn operator_account_name(operator_home: &str) -> String {
    Path::new(operator_home)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("operator")
        .to_string()
}

fn xcode_backend_path(developer_dir: &str) -> String {
    let developer_dir = developer_dir.trim_end_matches('/');
    format!("/usr/bin:/bin:/usr/sbin:/sbin:{developer_dir}/usr/bin")
}

fn xcode_broker_health_reason(
    state: XcodeBrokerHealthState,
    broker_disabled: bool,
    backend_available: bool,
    active_leases: usize,
    stale_lease_count: usize,
    queued_leases: usize,
    max_active_leases: usize,
    observation_persistence_failures: u64,
) -> (String, String) {
    if broker_disabled || state == XcodeBrokerHealthState::Disabled {
        return (
            "xcode_mcp_broker_disabled".to_string(),
            "Xcode broker disabled".to_string(),
        );
    }
    if !backend_available || state == XcodeBrokerHealthState::Failed {
        return (
            "xcode_mcp_backend_unavailable".to_string(),
            "Xcode broker failed: backend unavailable".to_string(),
        );
    }
    if stale_lease_count > 0 {
        return (
            "xcode_mcp_stale_leases".to_string(),
            format!(
                "Xcode broker degraded: {stale_lease_count} stale helper lease(s) need cleanup"
            ),
        );
    }
    if observation_persistence_failures > 0 {
        return (
            "xcode_observation_persist_failed".to_string(),
            "Xcode broker degraded: observation persistence failures".to_string(),
        );
    }
    if queued_leases > 0 || active_leases >= max_active_leases {
        return (
            "xcode_mcp_capacity_backpressure".to_string(),
            "Xcode broker degraded: capacity backpressure".to_string(),
        );
    }
    ("healthy".to_string(), "Xcode broker healthy".to_string())
}

fn hash_secret(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xcode_target::XcodeProcessCandidate;

    struct FailingObservationSink;

    #[async_trait]
    impl XcodeRuntimeObservationSink for FailingObservationSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            _update: XcodeRuntimeObservationUpdate,
        ) -> Result<()> {
            bail!("fixture persistence failure")
        }
    }

    struct NoopBackend;

    #[async_trait]
    impl XcodeMcpBackend for NoopBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            _request: serde_json::Value,
        ) -> Result<serde_json::Value> {
            Ok(serde_json::json!({"jsonrpc":"2.0","result":{}}))
        }
    }

    struct RecordingBackend {
        released: Mutex<Vec<String>>,
        session_count: AtomicUsize,
    }

    #[async_trait]
    impl XcodeMcpBackend for RecordingBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            _request: serde_json::Value,
        ) -> Result<serde_json::Value> {
            Ok(serde_json::json!({"jsonrpc":"2.0","result":{}}))
        }

        async fn release_lease(&self, lease_id: &str) -> Result<()> {
            self.released.lock().await.push(lease_id.to_string());
            Ok(())
        }

        async fn active_backend_session_count(&self) -> Option<usize> {
            Some(self.session_count.load(Ordering::Acquire))
        }
    }

    #[tokio::test]
    async fn observation_persistence_failure_degrades_broker_health() {
        let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
            XcodeMcpBridgePoolConfig::default(),
            Arc::new(FailingObservationSink),
            Arc::new(NoopBackend),
        );

        pool.record_observation(
            Some(AgentExecutionId::new()),
            pool.lease_observation(
                "lease-fixture".to_string(),
                "http://127.0.0.1:0/xcode-mcp/lease-fixture".to_string(),
                None,
                "reserved",
                None,
                "reserved",
                Some(0),
            ),
        )
        .await;

        let health = pool.health_snapshot().await;
        assert_eq!(health.state, XcodeBrokerHealthState::Degraded);
        assert_eq!(health.reason_code, "xcode_observation_persist_failed");
        assert!(!health.can_acquire_new_xcode_leases);
        assert_eq!(health.active_lease_count, 0);
        assert_eq!(health.initialize_queue_depth, 0);
        assert!(!health.last_transition_at.is_empty());
        assert!(health.operator_message.contains("observation persistence"));
        assert_eq!(health.observation_persistence_failures, 1);
        assert!(health.backend_available);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_backend_close_reaps_backend_process_group() {
        let temp = tempfile::tempdir().unwrap();
        let child_pid_path = temp.path().join("backend-child.pid");
        let mut snapshot = target_snapshot(XcodeTargetSelectionConfidence::ExplicitPid);
        snapshot.operator_home = temp.path().to_string_lossy().to_string();
        snapshot.darwin_tmpdir = temp.path().to_string_lossy().to_string();
        snapshot.developer_dir = "/usr".to_string();
        let context = XcodeMcpBackendRequestContext {
            run_id: RunId::new(),
            lease_id: "lease-process-group".to_string(),
            endpoint: "http://127.0.0.1:0/xcode-mcp/lease-process-group".to_string(),
            agent_execution_id: None,
            target_snapshot: Some(snapshot),
        };
        let config = XcodeMcpProcessBackendConfig {
            command: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "sleep 60 & echo $! > \"$1\"; wait".to_string(),
                "xcode-broker-test".to_string(),
                child_pid_path.to_string_lossy().to_string(),
            ],
            request_timeout: Duration::from_secs(1),
        };

        let mut session = XcodeMcpProcessBackendSession::spawn(&config, &context)
            .await
            .unwrap();
        let child_pid = wait_for_pid_file(&child_pid_path).await;

        session.close().await.unwrap();

        assert!(
            wait_for_process_exit(child_pid).await,
            "backend close must reap child processes in the backend process group"
        );
    }

    #[tokio::test]
    async fn stale_active_leases_degrade_broker_health_with_helper_metrics() {
        let backend = Arc::new(RecordingBackend {
            released: Mutex::new(Vec::new()),
            session_count: AtomicUsize::new(3),
        });
        let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
            XcodeMcpBridgePoolConfig::default(),
            Arc::new(NoopXcodeRuntimeObservationSink),
            backend,
        );
        let now = Instant::now();
        pool.state.lock().await.leases.insert(
            "lease-stale".to_string(),
            LeaseRecord {
                run_id: RunId::new(),
                agent_execution_id: None,
                endpoint: "http://127.0.0.1:0/xcode-mcp/lease-stale".to_string(),
                authorization_hash: hash_secret("token"),
                state: XcodeMcpLeaseState::Active,
                last_activity_at: now
                    - XCODE_MCP_ACTIVE_LEASE_IDLE_TIMEOUT
                    - Duration::from_secs(1),
                first_connect_deadline: now + Duration::from_secs(60),
                mcp_policy: BrokerMcpPolicy::allow_all(),
                target_snapshot: None,
            },
        );

        let health = pool.health_snapshot().await;

        assert_eq!(health.state, XcodeBrokerHealthState::Degraded);
        assert_eq!(health.reason_code, "xcode_mcp_stale_leases");
        assert_eq!(health.stale_lease_count, 1);
        assert_eq!(health.backend_session_count, 3);
        assert_eq!(health.helper_cleanup_reaped_leases_total, 0);
        assert!(!health.can_acquire_new_xcode_leases);
    }

    #[tokio::test]
    async fn cleanup_idle_active_leases_releases_backend_and_tracks_reaped_total() {
        let backend = Arc::new(RecordingBackend {
            released: Mutex::new(Vec::new()),
            session_count: AtomicUsize::new(1),
        });
        let pool = XcodeMcpBridgePool::new_with_sink_and_backend(
            XcodeMcpBridgePoolConfig::default(),
            Arc::new(NoopXcodeRuntimeObservationSink),
            backend.clone(),
        );
        let now = Instant::now();
        pool.state.lock().await.leases.insert(
            "lease-stale".to_string(),
            LeaseRecord {
                run_id: RunId::new(),
                agent_execution_id: None,
                endpoint: "http://127.0.0.1:0/xcode-mcp/lease-stale".to_string(),
                authorization_hash: hash_secret("token"),
                state: XcodeMcpLeaseState::Active,
                last_activity_at: now - Duration::from_secs(60),
                first_connect_deadline: now + Duration::from_secs(60),
                mcp_policy: BrokerMcpPolicy::allow_all(),
                target_snapshot: None,
            },
        );

        let cleaned = pool
            .cleanup_idle_active_leases_older_than(Duration::from_secs(30))
            .await
            .unwrap();

        assert_eq!(cleaned, vec!["lease-stale".to_string()]);
        assert_eq!(pool.active_lease_count().await, 0);
        assert_eq!(
            backend.released.lock().await.as_slice(),
            ["lease-stale".to_string()]
        );
        let health = pool.health_snapshot().await;
        assert_eq!(health.stale_lease_count, 0);
        assert_eq!(health.helper_cleanup_reaped_leases_total, 1);
    }

    #[test]
    fn explicit_pid_snapshot_stays_valid_when_workspace_identity_is_unavailable() {
        let snapshot = target_snapshot(XcodeTargetSelectionConfidence::ExplicitPid);
        let host = host(vec![candidate(4242, None, 501, true)]);

        assert!(target_snapshot_matches_host(&snapshot, &host));
    }

    #[test]
    fn workspace_match_snapshot_requires_matching_workspace_identity() {
        let snapshot = target_snapshot(XcodeTargetSelectionConfidence::WorkspaceMatch);
        let host = host(vec![candidate(4242, None, 501, true)]);

        assert!(!target_snapshot_matches_host(&snapshot, &host));
    }

    #[test]
    fn target_snapshot_drift_rejects_dead_or_wrong_uid_candidates() {
        let snapshot = target_snapshot(XcodeTargetSelectionConfidence::ExplicitPid);

        assert!(!target_snapshot_matches_host(
            &snapshot,
            &host(vec![candidate(4242, Some("/workspace"), 501, false)])
        ));
        assert!(!target_snapshot_matches_host(
            &snapshot,
            &host(vec![candidate(4242, Some("/workspace"), 502, true)])
        ));
    }

    fn target_snapshot(
        selection_confidence: XcodeTargetSelectionConfidence,
    ) -> XcodeTargetSnapshot {
        XcodeTargetSnapshot {
            xcode_pid: 4242,
            workspace_identity: "/workspace".to_string(),
            developer_dir: "/Applications/Xcode.app/Contents/Developer".to_string(),
            operator_home: "/Users/gui".to_string(),
            darwin_tmpdir: "/var/folders/t/tmp".to_string(),
            selection_confidence,
            runtime_profile_id: Some("runtime-profile".to_string()),
            permission_profile_id: Some("permission-profile".to_string()),
            broker_contract_hash: "contract-hash".to_string(),
        }
    }

    fn host(candidate_xcodes: Vec<XcodeProcessCandidate>) -> HostProbeContext {
        HostProbeContext {
            expected_gui_uid: Some(501),
            operator_home: Some("/Users/gui".to_string()),
            darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
            developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
            candidate_xcodes,
        }
    }

    fn candidate(
        pid: i64,
        workspace_identity: Option<&str>,
        uid: u32,
        alive: bool,
    ) -> XcodeProcessCandidate {
        XcodeProcessCandidate {
            pid,
            uid,
            workspace_identity: workspace_identity.map(str::to_string),
            app_path: Some("/Applications/Xcode.app".to_string()),
            developer_dir: None,
            operator_home: None,
            darwin_tmpdir: None,
            alive,
        }
    }

    #[cfg(unix)]
    async fn wait_for_pid_file(path: &Path) -> libc::pid_t {
        for _ in 0..50 {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(pid) = raw.trim().parse::<libc::pid_t>() {
                    return pid;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("timed out waiting for child pid file at {}", path.display());
    }

    #[cfg(unix)]
    async fn wait_for_process_exit(pid: libc::pid_t) -> bool {
        for _ in 0..100 {
            if !process_exists(pid) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }

    #[cfg(unix)]
    fn process_exists(pid: libc::pid_t) -> bool {
        let rc = unsafe { libc::kill(pid, 0) };
        if rc == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }
}
