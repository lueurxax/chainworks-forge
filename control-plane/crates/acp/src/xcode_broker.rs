use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use domain::ids::AgentExecutionId;
use domain::xcode_runtime::{
    McpBrokerObservation, XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, Notify};
use tracing::warn;
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
    agent_execution_id: Option<AgentExecutionId>,
    endpoint: String,
    authorization_hash: String,
    state: XcodeMcpLeaseState,
    first_connect_deadline: Instant,
    mcp_policy: BrokerMcpPolicy,
    target_snapshot: Option<XcodeTargetSnapshot>,
}

#[derive(Default)]
struct XcodeMcpBridgePoolState {
    leases: HashMap<String, LeaseRecord>,
    target_probe_context: Option<HostProbeContext>,
}

pub struct XcodeMcpBridgePool {
    config: XcodeMcpBridgePoolConfig,
    state: Mutex<XcodeMcpBridgePoolState>,
    initialize_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    queued_leases: AtomicUsize,
    queue_notify: Notify,
    observation_sink: Arc<dyn XcodeRuntimeObservationSink>,
    backend: Option<Arc<dyn XcodeMcpBackend>>,
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
    sessions: Mutex<HashMap<String, Arc<Mutex<XcodeMcpProcessBackendSession>>>>,
}

struct XcodeMcpProcessBackendSession {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
    next_backend_id: u64,
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
    pub pool_id: String,
    pub active_leases: usize,
    pub queued_leases: usize,
    pub max_active_leases: usize,
    pub max_queued_leases: usize,
    pub broker_disabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XcodeBrokerHttpRouteState {
    Activated,
    AlreadyActive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XcodeMcpBackendRequestContext {
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
}

impl XcodeMcpProcessBackend {
    pub fn new(config: XcodeMcpProcessBackendConfig) -> Self {
        Self {
            config,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    async fn session_for(
        &self,
        context: &XcodeMcpBackendRequestContext,
    ) -> Result<Arc<Mutex<XcodeMcpProcessBackendSession>>> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&context.lease_id) {
            return Ok(session.clone());
        }
        let session = Arc::new(Mutex::new(
            XcodeMcpProcessBackendSession::spawn(&self.config, context).await?,
        ));
        sessions.insert(context.lease_id.clone(), session.clone());
        Ok(session)
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
        let mut session = session.lock().await;
        match tokio::time::timeout(self.config.request_timeout, session.forward(request)).await {
            Ok(result) => result,
            Err(_) => {
                bail!(
                    "xcode_mcp_initialize_timeout: timed out after {:?} waiting for backend response for lease '{}'",
                    self.config.request_timeout,
                    context.lease_id
                )
            }
        }
    }

    async fn backend_process_id(&self, lease_id: &str) -> Option<i64> {
        let session = self.sessions.lock().await.get(lease_id).cloned()?;
        let session = session.lock().await;
        session.child.id().map(|pid| pid as i64)
    }

    async fn release_lease(&self, lease_id: &str) -> Result<()> {
        let session = self.sessions.lock().await.remove(lease_id);
        if let Some(session) = session {
            session.lock().await.close().await?;
        }
        Ok(())
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
            .args(&config.args)
            .env("HOME", &target_snapshot.operator_home)
            .env("TMPDIR", &target_snapshot.darwin_tmpdir)
            .env("DEVELOPER_DIR", &target_snapshot.developer_dir)
            .env(
                "CHAINWORKS_XCODE_PID",
                target_snapshot.xcode_pid.to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);

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
        })
    }

    async fn forward(&mut self, mut request: serde_json::Value) -> Result<serde_json::Value> {
        let original_id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
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
            response["id"] = original_id;
            return Ok(response);
        }
    }

    async fn close(&mut self) -> Result<()> {
        let _ = self.stdin.shutdown().await;
        match tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(_status)) => Ok(()),
            _ => {
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
        }
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
        let active_leases = self.active_lease_count().await;
        let queued_leases = self.queued_lease_count();
        let broker_disabled = self.broker_disabled();
        let state = if broker_disabled {
            XcodeBrokerHealthState::Disabled
        } else if queued_leases > 0 || active_leases >= self.config.max_active_leases {
            XcodeBrokerHealthState::Degraded
        } else {
            XcodeBrokerHealthState::Healthy
        };

        XcodeBrokerHealthSnapshot {
            state,
            pool_id: self.config.pool_id.clone(),
            active_leases,
            queued_leases,
            max_active_leases: self.config.max_active_leases,
            max_queued_leases: self.config.max_queued_leases,
            broker_disabled,
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
                return Ok(XcodeBrokerHttpRouteState::AlreadyActive);
            }
            lease.state = XcodeMcpLeaseState::Active;
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
            let state = self.state.lock().await;
            let Some(lease) = state.leases.get(lease_id) else {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is not available");
            };
            XcodeMcpBackendRequestContext {
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
        let lock_guard = match tokio::time::timeout(self.config.spawn_init_timeout, lock.lock())
            .await
        {
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
                return Ok(());
            }
            if lease.state == XcodeMcpLeaseState::Closing {
                bail!("xcode_mcp_first_connect_timeout: lease '{lease_id}' is closing");
            }
            lease.state = XcodeMcpLeaseState::Active;
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
            warn!(
                agent_execution_id = %agent_execution_id,
                error = %err,
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
                        agent_execution_id: req.agent_execution_id,
                        endpoint: endpoint.clone(),
                        authorization_hash: hash_secret(&bearer_token),
                        state: XcodeMcpLeaseState::Reserved,
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

fn hash_secret(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xcode_target::XcodeProcessCandidate;

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
}
