use anyhow::Context;
use chrono::Utc;
use domain::ids::AgentExecutionId;
use domain::xcode_runtime::{
    XcodeHostExecutorEvent, XcodeRuntimeObservationUpdate, XcodeShimEvent, XcodeShimInvocationEvent,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum XcodeShimRouteDecision {
    HostExecutor,
    XcrunPassthrough,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeShimCommandPolicy {
    pub invoked_tool: String,
    pub args: Vec<String>,
    pub decision: XcodeShimRouteDecision,
    pub reason_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeShimProcessBinding {
    pub pid: u32,
    pub uid: u32,
    #[serde(default)]
    pub parent_pid: Option<u32>,
    #[serde(default)]
    pub ancestor_pids: Vec<u32>,
    #[serde(default)]
    pub start_time_fingerprint: Option<String>,
    #[serde(default)]
    pub executable_fingerprint: Option<String>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XcodeShimPeerCredentials {
    pub pid: u32,
    pub uid: u32,
}

#[cfg(unix)]
pub trait XcodeShimProcessInspector: Send + Sync {
    fn inspect_peer(
        &self,
        credentials: XcodeShimPeerCredentials,
    ) -> anyhow::Result<XcodeShimProcessBinding>;
}

#[cfg(unix)]
#[async_trait::async_trait]
pub trait XcodeShimGrantResolver: Send + Sync {
    async fn resolve_grant(&self, token_id: &str) -> anyhow::Result<XcodeShimResolvedDispatch>;
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultXcodeShimProcessInspector;

#[cfg(unix)]
impl XcodeShimProcessInspector for DefaultXcodeShimProcessInspector {
    fn inspect_peer(
        &self,
        credentials: XcodeShimPeerCredentials,
    ) -> anyhow::Result<XcodeShimProcessBinding> {
        let parent_pid = lookup_parent_pid(credentials.pid)?;
        let ancestor_pids = process_ancestor_pids(credentials.pid)?;
        Ok(XcodeShimProcessBinding {
            pid: credentials.pid,
            uid: credentials.uid,
            parent_pid,
            ancestor_pids,
            start_time_fingerprint: None,
            executable_fingerprint: None,
        })
    }
}

#[cfg(unix)]
#[derive(Clone, Debug)]
pub struct XcodeShimResolvedDispatch {
    pub grant: XcodeShimDispatchGrant,
    pub active_prompt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeShimDispatchGrant {
    pub token_id: String,
    pub token_sha256: String,
    pub lease_id: String,
    pub provider_process: XcodeShimProcessBinding,
    pub issued_at_epoch_ms: i64,
    pub expires_at_epoch_ms: i64,
    #[serde(default = "default_active_prompt_required")]
    pub active_prompt_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XcodeShimGrantRecord {
    pub grant: XcodeShimDispatchGrant,
    pub active_prompt: bool,
}

pub trait XcodeShimGrantStore: Send + Sync {
    fn insert_xcode_shim_grant(&self, record: XcodeShimGrantRecord);
    fn set_xcode_shim_grant_active_prompt(&self, token_id: &str, active_prompt: bool) -> bool;
    fn remove_xcode_shim_grant(&self, token_id: &str) -> Option<XcodeShimGrantRecord>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeShimDispatchAttempt {
    pub token_id: String,
    pub token_secret: String,
    pub peer_process: XcodeShimProcessBinding,
    pub now_epoch_ms: i64,
    #[serde(default)]
    pub active_prompt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeShimDispatchAuthorization {
    pub allowed: bool,
    pub reason_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeShimDispatchRequest {
    pub agent_execution_id: Option<AgentExecutionId>,
    pub grant: XcodeShimDispatchGrant,
    pub attempt: XcodeShimDispatchAttempt,
    pub plan_input: XcodeHostExecutorPlanInput,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeShimDispatchOutcome {
    pub authorization: XcodeShimDispatchAuthorization,
    pub policy: XcodeShimCommandPolicy,
    pub plan: Option<XcodeHostExecutorPlan>,
    pub process_output: Option<XcodeHostExecutorProcessOutput>,
    pub exit_status: i64,
    pub reason_code: Option<String>,
}

impl XcodeShimDispatchOutcome {
    fn redacted_for_socket_response(mut self) -> Self {
        self.policy = self.policy.redacted_for_socket_response();
        self.plan = self
            .plan
            .map(XcodeHostExecutorPlan::redacted_for_socket_response);
        self.process_output = self
            .process_output
            .map(XcodeHostExecutorProcessOutput::redacted_for_socket_response);
        self.reason_code = self.reason_code.map(|value| redact_sensitive_text(&value));
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeShimSocketDispatchRequest {
    #[serde(default)]
    pub agent_execution_id: Option<AgentExecutionId>,
    pub token_id: String,
    pub token_secret: String,
    pub now_epoch_ms: i64,
    #[serde(default)]
    pub active_prompt: bool,
    pub plan_input: XcodeHostExecutorPlanInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeHostExecutorSimulatorCandidate {
    pub name: String,
    pub udid: String,
    #[serde(default)]
    pub runtime: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeHostExecutorPlanInput {
    pub invoked_tool: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: String,
    pub workspace_root: String,
    #[serde(default)]
    pub provider_env: BTreeMap<String, String>,
    #[serde(default)]
    pub simulator_candidates: Vec<XcodeHostExecutorSimulatorCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeHostExecutorPlan {
    pub tool: String,
    pub argv: Vec<String>,
    pub cwd: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub env_allowlist_applied: Vec<String>,
    #[serde(default)]
    pub env_dropped_from_provider: Vec<String>,
    pub selected_simulator_id: Option<String>,
}

impl XcodeHostExecutorPlan {
    fn redacted_for_socket_response(mut self) -> Self {
        self.argv = self
            .argv
            .into_iter()
            .map(|value| redact_sensitive_text(&value))
            .collect();
        self.cwd = redact_sensitive_text(&self.cwd);
        self.env = self
            .env
            .into_iter()
            .map(|(key, value)| (key, redact_sensitive_text(&value)))
            .collect();
        self.selected_simulator_id = self
            .selected_simulator_id
            .map(|value| redact_sensitive_text(&value));
        self
    }
}

#[derive(Clone, Debug)]
pub struct XcodeHostExecutorProcessConfig {
    pub tool_paths: BTreeMap<String, String>,
    pub timeout: Duration,
}

impl Default for XcodeHostExecutorProcessConfig {
    fn default() -> Self {
        Self {
            tool_paths: BTreeMap::new(),
            timeout: Duration::from_secs(300),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct XcodeHostExecutorProcessOutput {
    pub event: XcodeHostExecutorEvent,
    pub stdout: String,
    pub stderr: String,
}

impl XcodeHostExecutorProcessOutput {
    fn redacted_for_socket_response(mut self) -> Self {
        self.event.argv = self
            .event
            .argv
            .into_iter()
            .map(|value| redact_sensitive_text(&value))
            .collect();
        self.event.cwd = redact_sensitive_text(&self.event.cwd);
        self.event.selected_simulator_id = self
            .event
            .selected_simulator_id
            .map(|value| redact_sensitive_text(&value));
        self.stdout = redact_sensitive_text(&self.stdout);
        self.stderr = redact_sensitive_text(&self.stderr);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeHostExecutorPlanError {
    pub reason_code: String,
    #[serde(default)]
    pub candidate_simulator_ids: Vec<String>,
}

impl XcodeShimDispatchGrant {
    pub fn new(
        token_id: impl Into<String>,
        token_secret: impl AsRef<str>,
        lease_id: impl Into<String>,
        provider_process: XcodeShimProcessBinding,
        issued_at_epoch_ms: i64,
        expires_at_epoch_ms: i64,
    ) -> Self {
        Self {
            token_id: token_id.into(),
            token_sha256: sha256_hex(token_secret.as_ref()),
            lease_id: lease_id.into(),
            provider_process,
            issued_at_epoch_ms,
            expires_at_epoch_ms,
            active_prompt_required: true,
        }
    }

    pub fn authorize(&self, attempt: &XcodeShimDispatchAttempt) -> XcodeShimDispatchAuthorization {
        if self.token_id != attempt.token_id {
            return reject("p051_shim_token_id_mismatch");
        }
        if self.token_sha256 != sha256_hex(&attempt.token_secret) {
            return reject("p051_shim_token_mismatch");
        }
        if attempt.now_epoch_ms < self.issued_at_epoch_ms {
            return reject("p051_shim_token_not_yet_valid");
        }
        if attempt.now_epoch_ms > self.expires_at_epoch_ms {
            return reject("p051_shim_token_stale");
        }
        if self.active_prompt_required && !attempt.active_prompt {
            return reject("p051_shim_no_active_prompt");
        }
        if self.provider_process.uid != attempt.peer_process.uid {
            return reject("p051_shim_peer_uid_mismatch");
        }
        if !self.peer_process_matches_bound_provider(&attempt.peer_process) {
            return reject("p051_shim_peer_pid_mismatch");
        }
        if self.provider_process.pid == attempt.peer_process.pid {
            if self.provider_process.parent_pid != attempt.peer_process.parent_pid {
                return reject("p051_shim_process_tree_mismatch");
            }
            if self.provider_process.start_time_fingerprint
                != attempt.peer_process.start_time_fingerprint
            {
                return reject("p051_shim_process_start_mismatch");
            }
            if self.provider_process.executable_fingerprint
                != attempt.peer_process.executable_fingerprint
            {
                return reject("p051_shim_process_fingerprint_mismatch");
            }
        }

        XcodeShimDispatchAuthorization {
            allowed: true,
            reason_code: None,
        }
    }

    fn peer_process_matches_bound_provider(&self, peer_process: &XcodeShimProcessBinding) -> bool {
        self.provider_process.pid == peer_process.pid
            || peer_process.parent_pid == Some(self.provider_process.pid)
            || peer_process
                .ancestor_pids
                .iter()
                .any(|ancestor_pid| *ancestor_pid == self.provider_process.pid)
    }
}

#[cfg(unix)]
pub fn current_process_uid() -> u32 {
    unsafe { libc::getuid() }
}

#[cfg(unix)]
fn process_ancestor_pids(pid: u32) -> anyhow::Result<Vec<u32>> {
    let mut ancestors = Vec::new();
    let mut current = pid;
    for _ in 0..16 {
        let Some(parent) = lookup_parent_pid(current)? else {
            break;
        };
        if parent == 0 || parent == current || ancestors.contains(&parent) {
            break;
        }
        ancestors.push(parent);
        if parent == 1 {
            break;
        }
        current = parent;
    }
    Ok(ancestors)
}

#[cfg(unix)]
fn lookup_parent_pid(pid: u32) -> anyhow::Result<Option<u32>> {
    let output = std::process::Command::new("/bin/ps")
        .args(["-o", "ppid=", "-p", &pid.to_string()])
        .output()
        .context("xcode_shim_parent_pid_lookup_failed")?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let parent_pid = trimmed
        .parse::<u32>()
        .context("xcode_shim_parent_pid_parse_failed")?;
    Ok(Some(parent_pid))
}

impl XcodeShimCommandPolicy {
    pub fn evaluate(invoked_tool: impl AsRef<str>, args: &[String]) -> Self {
        let invoked_tool = basename(invoked_tool.as_ref());
        let (decision, reason_code) = match invoked_tool.as_str() {
            "xcodebuild" | "simctl" => (XcodeShimRouteDecision::HostExecutor, None),
            "mcpbridge" => (
                XcodeShimRouteDecision::Reject,
                Some("p051_shim_mcpbridge_broker_only".to_string()),
            ),
            "xcrun" => evaluate_xcrun(args),
            _ => (
                XcodeShimRouteDecision::Reject,
                Some("p051_shim_unknown_tool".to_string()),
            ),
        };

        Self {
            invoked_tool,
            args: args.to_vec(),
            decision,
            reason_code,
        }
    }

    pub fn is_allowed(&self) -> bool {
        self.decision != XcodeShimRouteDecision::Reject
    }

    fn redacted_for_socket_response(mut self) -> Self {
        self.args = self
            .args
            .into_iter()
            .map(|value| redact_sensitive_text(&value))
            .collect();
        self.reason_code = self.reason_code.map(|value| redact_sensitive_text(&value));
        self
    }
}

impl XcodeHostExecutorPlan {
    pub fn build(input: XcodeHostExecutorPlanInput) -> Result<Self, XcodeHostExecutorPlanError> {
        let policy = XcodeShimCommandPolicy::evaluate(&input.invoked_tool, &input.args);
        if policy.decision != XcodeShimRouteDecision::HostExecutor {
            return Err(plan_error(
                policy
                    .reason_code
                    .unwrap_or_else(|| "p051_host_executor_not_allowed".to_string()),
            ));
        }

        let (tool, host_args) = host_executor_invocation(&input.invoked_tool, &input.args)?;
        let cwd = resolve_cwd_inside_workspace(&input.cwd, &input.workspace_root)?;
        let (argv, selected_simulator_id) =
            rewrite_destination_to_simulator_id(&host_args, &input.simulator_candidates)?;
        let (env, env_allowlist_applied, env_dropped_from_provider) =
            apply_host_executor_env_allowlist(input.provider_env);

        Ok(Self {
            tool,
            argv,
            cwd,
            env,
            env_allowlist_applied,
            env_dropped_from_provider,
            selected_simulator_id,
        })
    }

    pub async fn execute_process(
        &self,
        config: &XcodeHostExecutorProcessConfig,
    ) -> Result<XcodeHostExecutorProcessOutput, XcodeHostExecutorPlanError> {
        let executable = config
            .tool_paths
            .get(&self.tool)
            .cloned()
            .unwrap_or_else(|| self.tool.clone());
        let started_at = Utc::now();
        let timer = Instant::now();
        let mut command = Command::new(executable);
        command
            .args(&self.argv)
            .current_dir(&self.cwd)
            .envs(&self.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = match tokio::time::timeout(config.timeout, command.output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(_)) => return Err(plan_error("p051_host_executor_spawn_failed")),
            Err(_) => return Err(plan_error("p051_host_executor_timeout")),
        };
        let duration_ms = timer.elapsed().as_millis().min(i64::MAX as u128) as i64;
        let exit_status = output.status.code().unwrap_or(-1) as i64;

        Ok(XcodeHostExecutorProcessOutput {
            event: XcodeHostExecutorEvent {
                ts: started_at,
                tool: self.tool.clone(),
                argv: self.argv.clone(),
                cwd: self.cwd.clone(),
                host_env_disposition: "allowlist_applied".to_string(),
                env_allowlist_applied: self.env_allowlist_applied.clone(),
                env_dropped_from_provider: self.env_dropped_from_provider.clone(),
                selected_simulator_id: self.selected_simulator_id.clone(),
                exit_status,
                duration_ms,
            },
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub async fn dispatch_xcode_shim_request(
    request: XcodeShimDispatchRequest,
    config: &XcodeHostExecutorProcessConfig,
    observation_sink: &dyn crate::XcodeRuntimeObservationSink,
) -> XcodeShimDispatchOutcome {
    let policy = XcodeShimCommandPolicy::evaluate(
        &request.plan_input.invoked_tool,
        &request.plan_input.args,
    );
    let authorization = request.grant.authorize(&request.attempt);

    if !authorization.allowed {
        let reason_code = authorization.reason_code.clone();
        record_shim_invocation(
            request.agent_execution_id,
            &request.grant,
            &request.attempt,
            &request.plan_input,
            &policy,
            reason_code
                .as_deref()
                .unwrap_or("p051_shim_authorization_rejected"),
            126,
            observation_sink,
        )
        .await;
        return XcodeShimDispatchOutcome {
            authorization,
            policy,
            plan: None,
            process_output: None,
            exit_status: 126,
            reason_code,
        };
    }

    if policy.decision != XcodeShimRouteDecision::HostExecutor {
        let reason_code = policy
            .reason_code
            .clone()
            .unwrap_or_else(|| "p051_host_executor_not_allowed".to_string());
        record_shim_invocation(
            request.agent_execution_id,
            &request.grant,
            &request.attempt,
            &request.plan_input,
            &policy,
            &reason_code,
            126,
            observation_sink,
        )
        .await;
        return XcodeShimDispatchOutcome {
            authorization,
            policy,
            plan: None,
            process_output: None,
            exit_status: 126,
            reason_code: Some(reason_code),
        };
    }

    let plan = match XcodeHostExecutorPlan::build(request.plan_input.clone()) {
        Ok(plan) => plan,
        Err(error) => {
            record_shim_invocation(
                request.agent_execution_id,
                &request.grant,
                &request.attempt,
                &request.plan_input,
                &policy,
                &error.reason_code,
                126,
                observation_sink,
            )
            .await;
            return XcodeShimDispatchOutcome {
                authorization,
                policy,
                plan: None,
                process_output: None,
                exit_status: 126,
                reason_code: Some(error.reason_code),
            };
        }
    };

    match plan.execute_process(config).await {
        Ok(process_output) => {
            let exit_status = process_output.event.exit_status;
            record_shim_invocation(
                request.agent_execution_id,
                &request.grant,
                &request.attempt,
                &request.plan_input,
                &policy,
                "host_executor",
                exit_status,
                observation_sink,
            )
            .await;
            record_host_executor_event(
                request.agent_execution_id,
                process_output.event.clone(),
                observation_sink,
            )
            .await;
            XcodeShimDispatchOutcome {
                authorization,
                policy,
                plan: Some(plan),
                process_output: Some(process_output),
                exit_status,
                reason_code: None,
            }
        }
        Err(error) => {
            record_shim_invocation(
                request.agent_execution_id,
                &request.grant,
                &request.attempt,
                &request.plan_input,
                &policy,
                &error.reason_code,
                126,
                observation_sink,
            )
            .await;
            XcodeShimDispatchOutcome {
                authorization,
                policy,
                plan: Some(plan),
                process_output: None,
                exit_status: 126,
                reason_code: Some(error.reason_code),
            }
        }
    }
}

pub async fn dispatch_xcode_shim_socket_request(
    request: XcodeShimSocketDispatchRequest,
    grant: XcodeShimDispatchGrant,
    peer_process: XcodeShimProcessBinding,
    config: &XcodeHostExecutorProcessConfig,
    observation_sink: &dyn crate::XcodeRuntimeObservationSink,
) -> XcodeShimDispatchOutcome {
    let dispatch_request = XcodeShimDispatchRequest {
        agent_execution_id: request.agent_execution_id,
        grant,
        attempt: XcodeShimDispatchAttempt {
            token_id: request.token_id,
            token_secret: request.token_secret,
            peer_process,
            now_epoch_ms: request.now_epoch_ms,
            active_prompt: request.active_prompt,
        },
        plan_input: request.plan_input,
    };

    dispatch_xcode_shim_request(dispatch_request, config, observation_sink).await
}

#[cfg(unix)]
pub async fn handle_xcode_shim_unix_stream(
    stream: UnixStream,
    grant: XcodeShimDispatchGrant,
    peer_process: XcodeShimProcessBinding,
    config: &XcodeHostExecutorProcessConfig,
    observation_sink: &dyn crate::XcodeRuntimeObservationSink,
) -> anyhow::Result<XcodeShimDispatchOutcome> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await?;
    if bytes == 0 {
        anyhow::bail!("xcode_shim_socket_empty_request");
    }

    let request: XcodeShimSocketDispatchRequest = serde_json::from_str(&line)?;
    let outcome =
        dispatch_xcode_shim_socket_request(request, grant, peer_process, config, observation_sink)
            .await;
    let response_outcome = outcome.redacted_for_socket_response();
    writer
        .write_all(&serde_json::to_vec(&response_outcome)?)
        .await?;
    writer.write_all(b"\n").await?;
    Ok(response_outcome)
}

#[cfg(unix)]
pub async fn handle_xcode_shim_unix_stream_with_peer_credentials(
    stream: UnixStream,
    grant: XcodeShimDispatchGrant,
    config: &XcodeHostExecutorProcessConfig,
    observation_sink: &dyn crate::XcodeRuntimeObservationSink,
    process_inspector: &dyn XcodeShimProcessInspector,
) -> anyhow::Result<XcodeShimDispatchOutcome> {
    let credentials = xcode_shim_peer_credentials(&stream)?;
    let peer_process = process_inspector.inspect_peer(credentials)?;
    handle_xcode_shim_unix_stream(stream, grant, peer_process, config, observation_sink).await
}

#[cfg(unix)]
pub async fn handle_xcode_shim_unix_stream_with_grant_resolver(
    stream: UnixStream,
    config: &XcodeHostExecutorProcessConfig,
    observation_sink: &dyn crate::XcodeRuntimeObservationSink,
    process_inspector: &dyn XcodeShimProcessInspector,
    grant_resolver: &dyn XcodeShimGrantResolver,
) -> anyhow::Result<XcodeShimDispatchOutcome> {
    let credentials = xcode_shim_peer_credentials(&stream)?;
    let peer_process = process_inspector.inspect_peer(credentials)?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await?;
    if bytes == 0 {
        anyhow::bail!("xcode_shim_socket_empty_request");
    }

    let mut request: XcodeShimSocketDispatchRequest = serde_json::from_str(&line)?;
    let resolved = grant_resolver.resolve_grant(&request.token_id).await?;
    request.active_prompt = resolved.active_prompt;
    let outcome = dispatch_xcode_shim_socket_request(
        request,
        resolved.grant,
        peer_process,
        config,
        observation_sink,
    )
    .await;
    let response_outcome = outcome.redacted_for_socket_response();
    writer
        .write_all(&serde_json::to_vec(&response_outcome)?)
        .await?;
    writer.write_all(b"\n").await?;
    Ok(response_outcome)
}

#[cfg(unix)]
pub fn xcode_shim_peer_credentials(
    stream: &UnixStream,
) -> anyhow::Result<XcodeShimPeerCredentials> {
    peer_credentials_from_fd(stream.as_raw_fd())
}

#[cfg(all(unix, target_os = "linux"))]
fn peer_credentials_from_fd(fd: std::os::fd::RawFd) -> anyhow::Result<XcodeShimPeerCredentials> {
    let mut credentials = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            credentials.as_mut_ptr().cast(),
            &mut length,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("xcode_shim_peer_credentials_failed");
    }
    let credentials = unsafe { credentials.assume_init() };
    Ok(XcodeShimPeerCredentials {
        pid: u32::try_from(credentials.pid).context("xcode_shim_peer_pid_out_of_range")?,
        uid: credentials.uid,
    })
}

#[cfg(all(unix, target_os = "macos"))]
fn peer_credentials_from_fd(fd: std::os::fd::RawFd) -> anyhow::Result<XcodeShimPeerCredentials> {
    let mut uid = 0 as libc::uid_t;
    let mut gid = 0 as libc::gid_t;
    let rc = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("xcode_shim_peer_uid_lookup_failed");
    }

    const SOL_LOCAL: libc::c_int = 0;
    const LOCAL_PEERPID: libc::c_int = 2;
    let mut pid = 0 as libc::pid_t;
    let mut length = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            fd,
            SOL_LOCAL,
            LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast(),
            &mut length,
        )
    };
    if rc != 0 {
        return Err(std::io::Error::last_os_error()).context("xcode_shim_peer_pid_lookup_failed");
    }

    Ok(XcodeShimPeerCredentials {
        pid: u32::try_from(pid).context("xcode_shim_peer_pid_out_of_range")?,
        uid,
    })
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn peer_credentials_from_fd(_fd: std::os::fd::RawFd) -> anyhow::Result<XcodeShimPeerCredentials> {
    anyhow::bail!("xcode_shim_peer_credentials_unsupported_platform")
}

async fn record_shim_invocation(
    agent_execution_id: Option<AgentExecutionId>,
    grant: &XcodeShimDispatchGrant,
    attempt: &XcodeShimDispatchAttempt,
    input: &XcodeHostExecutorPlanInput,
    policy: &XcodeShimCommandPolicy,
    policy_reason: &str,
    exit_status: i64,
    observation_sink: &dyn crate::XcodeRuntimeObservationSink,
) {
    let Some(agent_execution_id) = agent_execution_id else {
        return;
    };
    let event = XcodeShimInvocationEvent {
        ts: Utc::now(),
        tool: policy.invoked_tool.clone(),
        via_xcrun: policy.invoked_tool == "xcrun",
        argv: input.args.clone(),
        cwd: input.cwd.clone(),
        policy_decision: route_decision_label(policy.decision).to_string(),
        policy_reason: policy_reason.to_string(),
        derived_peer_pid: attempt.peer_process.pid as i64,
        derived_peer_uid: attempt.peer_process.uid as i64,
        claimed_provider_pid: grant.provider_process.pid as i64,
        peer_pid_mismatch: !grant.peer_process_matches_bound_provider(&attempt.peer_process),
        exit_status,
    };
    let _ = observation_sink
        .append_xcode_runtime_observation(
            agent_execution_id,
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimInvocation(event)),
        )
        .await;
}

async fn record_host_executor_event(
    agent_execution_id: Option<AgentExecutionId>,
    event: XcodeHostExecutorEvent,
    observation_sink: &dyn crate::XcodeRuntimeObservationSink,
) {
    let Some(agent_execution_id) = agent_execution_id else {
        return;
    };
    let _ = observation_sink
        .append_xcode_runtime_observation(
            agent_execution_id,
            XcodeRuntimeObservationUpdate::XcodeHostExecutorEvent(event),
        )
        .await;
}

fn route_decision_label(decision: XcodeShimRouteDecision) -> &'static str {
    match decision {
        XcodeShimRouteDecision::HostExecutor => "host_executor",
        XcodeShimRouteDecision::XcrunPassthrough => "xcrun_passthrough",
        XcodeShimRouteDecision::Reject => "reject",
    }
}

fn host_executor_invocation(
    invoked_tool: &str,
    args: &[String],
) -> Result<(String, Vec<String>), XcodeHostExecutorPlanError> {
    let invoked_tool = basename(invoked_tool);
    if is_host_executor_tool(&invoked_tool) {
        return Ok((invoked_tool, args.to_vec()));
    }
    if invoked_tool != "xcrun" {
        return Err(plan_error("p051_host_executor_not_allowed"));
    }

    let mut idx = 0usize;
    while idx < args.len() {
        let token = args[idx].as_str();
        if token == "--" {
            return host_executor_xcrun_tool(args, idx + 1, idx + 2);
        }
        if matches!(token, "--sdk" | "-sdk" | "--toolchain" | "-toolchain") {
            if args.get(idx + 1).is_none() {
                return Err(plan_error("p051_shim_xcrun_missing_flag_value"));
            }
            idx += 2;
            continue;
        }
        if token.starts_with("--sdk=") || token.starts_with("--toolchain=") {
            idx += 1;
            continue;
        }
        if matches!(token, "--run" | "-r") {
            return host_executor_xcrun_tool(args, idx + 1, idx + 2);
        }
        if token.starts_with('-') {
            return Err(plan_error("p051_shim_xcrun_unknown_flag"));
        }

        return host_executor_xcrun_tool(args, idx, idx + 1);
    }

    Err(plan_error("p051_shim_xcrun_missing_mode"))
}

fn host_executor_xcrun_tool(
    args: &[String],
    tool_index: usize,
    argv_start: usize,
) -> Result<(String, Vec<String>), XcodeHostExecutorPlanError> {
    let Some(tool) = args.get(tool_index).map(|tool| basename(tool)) else {
        return Err(plan_error("p051_shim_xcrun_missing_mode"));
    };
    if !is_host_executor_tool(&tool) {
        return Err(plan_error("p051_host_executor_not_allowed"));
    }

    Ok((tool, args[argv_start..].to_vec()))
}

fn resolve_cwd_inside_workspace(
    cwd: &str,
    workspace_root: &str,
) -> Result<String, XcodeHostExecutorPlanError> {
    let workspace = normalize_path(Path::new(workspace_root));
    if workspace.as_os_str().is_empty() || !workspace.is_absolute() {
        return Err(plan_error("p051_host_executor_invalid_workspace_root"));
    }

    let requested = Path::new(cwd);
    let resolved = if requested.is_absolute() {
        normalize_path(requested)
    } else {
        normalize_path(&workspace.join(requested))
    };

    if resolved == workspace || resolved.starts_with(&workspace) {
        Ok(resolved.to_string_lossy().into_owned())
    } else {
        Err(plan_error("p051_host_executor_cwd_outside_workspace"))
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    normalized
}

fn apply_host_executor_env_allowlist(
    provider_env: BTreeMap<String, String>,
) -> (BTreeMap<String, String>, Vec<String>, Vec<String>) {
    let allowed = host_executor_env_allowlist();
    let mut env = BTreeMap::new();
    let mut env_allowlist_applied = Vec::new();
    let mut env_dropped_from_provider = Vec::new();

    for (key, value) in provider_env {
        if allowed.contains(key.as_str()) {
            env_allowlist_applied.push(key.clone());
            env.insert(key, value);
        } else {
            env_dropped_from_provider.push(key);
        }
    }

    (env, env_allowlist_applied, env_dropped_from_provider)
}

fn host_executor_env_allowlist() -> BTreeSet<&'static str> {
    [
        "ACTION",
        "ARCHS",
        "CONFIGURATION",
        "DEVELOPER_DIR",
        "DESTINATION",
        "ONLY_ACTIVE_ARCH",
        "PLATFORM_NAME",
        "SDKROOT",
        "SCHEME",
        "XCODE_XCCONFIG_FILE",
    ]
    .into_iter()
    .collect()
}

fn rewrite_destination_to_simulator_id(
    args: &[String],
    simulator_candidates: &[XcodeHostExecutorSimulatorCandidate],
) -> Result<(Vec<String>, Option<String>), XcodeHostExecutorPlanError> {
    let Some(destination_index) = args.iter().position(|arg| arg == "-destination") else {
        return Ok((args.to_vec(), None));
    };
    let Some(destination) = args.get(destination_index + 1) else {
        return Err(plan_error("p051_host_executor_missing_destination"));
    };
    if !is_simulator_destination(destination) || destination_contains_id(destination) {
        return Ok((args.to_vec(), destination_id(destination)));
    }

    let Some(name) = destination_value(destination, "name") else {
        return Ok((args.to_vec(), None));
    };

    let matches = simulator_candidates
        .iter()
        .filter(|candidate| candidate.name == name)
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [] => Err(plan_error("p051_simulator_destination_not_found")),
        [candidate] => {
            let mut rewritten = args.to_vec();
            rewritten[destination_index + 1] =
                replace_or_append_destination_value(destination, "id", &candidate.udid);
            Ok((rewritten, Some(candidate.udid.clone())))
        }
        many => Err(XcodeHostExecutorPlanError {
            reason_code: "p051_simulator_destination_ambiguous".to_string(),
            candidate_simulator_ids: many
                .iter()
                .map(|candidate| candidate.udid.clone())
                .collect(),
        }),
    }
}

fn is_simulator_destination(destination: &str) -> bool {
    destination_value(destination, "platform")
        .is_some_and(|platform| platform.contains("Simulator"))
}

fn destination_contains_id(destination: &str) -> bool {
    destination_value(destination, "id").is_some()
}

fn destination_id(destination: &str) -> Option<String> {
    destination_value(destination, "id")
}

fn destination_value(destination: &str, key: &str) -> Option<String> {
    destination.split(',').find_map(|part| {
        let (candidate_key, value) = part.split_once('=')?;
        (candidate_key.trim() == key).then(|| value.trim().to_string())
    })
}

fn replace_or_append_destination_value(destination: &str, key: &str, value: &str) -> String {
    let mut replaced = false;
    let mut parts = destination
        .split(',')
        .map(|part| {
            let Some((candidate_key, _)) = part.split_once('=') else {
                return part.to_string();
            };
            if candidate_key.trim() == key {
                replaced = true;
                format!("{candidate_key}={value}")
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>();

    if !replaced {
        parts.push(format!("{key}={value}"));
    }

    parts.join(",")
}

fn evaluate_xcrun(args: &[String]) -> (XcodeShimRouteDecision, Option<String>) {
    let mut idx = 0usize;
    while idx < args.len() {
        let token = args[idx].as_str();
        if token == "--" {
            return match args.get(idx + 1).map(|tool| basename(tool)) {
                Some(tool) if is_host_executor_tool(&tool) => {
                    (XcodeShimRouteDecision::HostExecutor, None)
                }
                Some(tool) if tool == "mcpbridge" => (
                    XcodeShimRouteDecision::Reject,
                    Some("p051_shim_mcpbridge_broker_only".to_string()),
                ),
                Some(_) => (XcodeShimRouteDecision::XcrunPassthrough, None),
                None => (
                    XcodeShimRouteDecision::Reject,
                    Some("p051_shim_xcrun_missing_mode".to_string()),
                ),
            };
        }
        if matches!(token, "--sdk" | "-sdk" | "--toolchain" | "-toolchain") {
            if args.get(idx + 1).is_none() {
                return (
                    XcodeShimRouteDecision::Reject,
                    Some("p051_shim_xcrun_missing_flag_value".to_string()),
                );
            }
            idx += 2;
            continue;
        }
        if token.starts_with("--sdk=") || token.starts_with("--toolchain=") {
            idx += 1;
            continue;
        }
        if matches!(
            token,
            "--show-sdk-path" | "--show-sdk-version" | "--show-sdk-build-version"
        ) {
            return (XcodeShimRouteDecision::XcrunPassthrough, None);
        }
        if matches!(token, "--find" | "-f") {
            let Some(tool) = args.get(idx + 1).map(|tool| basename(tool)) else {
                return (
                    XcodeShimRouteDecision::Reject,
                    Some("p051_shim_xcrun_missing_find_tool".to_string()),
                );
            };
            if is_guarded_tool(&tool) {
                return (
                    XcodeShimRouteDecision::Reject,
                    Some("p051_shim_xcrun_find_guarded_tool".to_string()),
                );
            }
            return (XcodeShimRouteDecision::XcrunPassthrough, None);
        }
        if matches!(token, "--run" | "-r") {
            let Some(tool) = args.get(idx + 1).map(|tool| basename(tool)) else {
                return (
                    XcodeShimRouteDecision::Reject,
                    Some("p051_shim_xcrun_missing_run_tool".to_string()),
                );
            };
            if tool == "mcpbridge" {
                return (
                    XcodeShimRouteDecision::Reject,
                    Some("p051_shim_mcpbridge_broker_only".to_string()),
                );
            }
            if is_host_executor_tool(&tool) {
                return (XcodeShimRouteDecision::HostExecutor, None);
            }
            return (XcodeShimRouteDecision::XcrunPassthrough, None);
        }
        if token.starts_with('-') {
            return (
                XcodeShimRouteDecision::Reject,
                Some("p051_shim_xcrun_unknown_flag".to_string()),
            );
        }

        let tool = basename(token);
        if tool == "mcpbridge" {
            return (
                XcodeShimRouteDecision::Reject,
                Some("p051_shim_mcpbridge_broker_only".to_string()),
            );
        }
        if is_host_executor_tool(&tool) {
            return (XcodeShimRouteDecision::HostExecutor, None);
        }
        return (XcodeShimRouteDecision::XcrunPassthrough, None);
    }

    (
        XcodeShimRouteDecision::Reject,
        Some("p051_shim_xcrun_missing_mode".to_string()),
    )
}

fn is_guarded_tool(tool: &str) -> bool {
    matches!(tool, "xcodebuild" | "simctl" | "mcpbridge" | "xcrun")
}

fn is_host_executor_tool(tool: &str) -> bool {
    matches!(tool, "xcodebuild" | "simctl")
}

fn basename(token: &str) -> String {
    token
        .trim_matches('"')
        .trim_matches('\'')
        .rsplit('/')
        .next()
        .unwrap_or(token)
        .to_string()
}

fn default_active_prompt_required() -> bool {
    true
}

fn reject(reason_code: &str) -> XcodeShimDispatchAuthorization {
    XcodeShimDispatchAuthorization {
        allowed: false,
        reason_code: Some(reason_code.to_string()),
    }
}

fn plan_error(reason_code: impl Into<String>) -> XcodeHostExecutorPlanError {
    XcodeHostExecutorPlanError {
        reason_code: reason_code.into(),
        candidate_simulator_ids: Vec::new(),
    }
}

fn sha256_hex(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn redact_sensitive_text(input: &str) -> String {
    let redacted_bearer = redact_after_markers(input, &["Bearer "], "<redacted>");
    let redacted_lease = redact_after_markers(&redacted_bearer, &["xcode-lease-"], "<redacted>");
    redact_after_markers(
        &redacted_lease,
        &["token=", "access_token=", "bearer_token=", "authorization="],
        "<redacted>",
    )
}

fn redact_after_markers(input: &str, markers: &[&str], replacement: &str) -> String {
    let mut output = input.to_string();
    for marker in markers {
        output = redact_after_marker(&output, marker, replacement);
    }
    output
}

fn redact_after_marker(input: &str, marker: &str, replacement: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative) = input[cursor..].find(marker) {
        let marker_start = cursor + relative;
        let secret_start = marker_start + marker.len();
        output.push_str(&input[cursor..secret_start]);

        let secret_end = input[secret_start..]
            .char_indices()
            .find_map(|(offset, ch)| is_secret_delimiter(ch).then_some(secret_start + offset))
            .unwrap_or(input.len());
        if secret_end > secret_start {
            output.push_str(replacement);
        }
        cursor = secret_end;
    }
    output.push_str(&input[cursor..]);
    output
}

fn is_secret_delimiter(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '&' | ',' | '"' | '\'' | ']' | '}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::{XcodeRuntimeObservationUpdate, XcodeShimEvent};
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[derive(Default)]
    struct CapturingObservationSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    impl CapturingObservationSink {
        fn updates(&self) -> Vec<XcodeRuntimeObservationUpdate> {
            self.updates.lock().expect("updates poisoned").clone()
        }
    }

    #[async_trait]
    impl crate::XcodeRuntimeObservationSink for CapturingObservationSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().expect("updates poisoned").push(update);
            Ok(())
        }
    }

    fn provider_process() -> XcodeShimProcessBinding {
        XcodeShimProcessBinding {
            pid: 42,
            uid: 501,
            parent_pid: Some(7),
            ancestor_pids: Vec::new(),
            start_time_fingerprint: Some("started-at-123".to_string()),
            executable_fingerprint: Some("provider-sha256".to_string()),
        }
    }

    fn grant() -> XcodeShimDispatchGrant {
        XcodeShimDispatchGrant::new(
            "token-a",
            "secret-a",
            "lease-a",
            provider_process(),
            1_000,
            2_000,
        )
    }

    fn attempt() -> XcodeShimDispatchAttempt {
        XcodeShimDispatchAttempt {
            token_id: "token-a".to_string(),
            token_secret: "secret-a".to_string(),
            peer_process: provider_process(),
            now_epoch_ms: 1_500,
            active_prompt: true,
        }
    }

    fn host_input() -> XcodeHostExecutorPlanInput {
        XcodeHostExecutorPlanInput {
            invoked_tool: "xcodebuild".to_string(),
            args: args(&[
                "-scheme",
                "Chainworks Forge",
                "-destination",
                "platform=iOS Simulator,name=iPhone 15",
                "test",
            ]),
            cwd: "Chainworks Forge".to_string(),
            workspace_root: "/workspace/project".to_string(),
            provider_env: BTreeMap::from([
                ("SCHEME".to_string(), "Chainworks Forge".to_string()),
                ("CONFIGURATION".to_string(), "Debug".to_string()),
                ("HOME".to_string(), "/tmp/provider-home".to_string()),
                (
                    "XCODE_XCCONFIG_FILE".to_string(),
                    "/tmp/config.xcconfig".to_string(),
                ),
            ]),
            simulator_candidates: vec![XcodeHostExecutorSimulatorCandidate {
                name: "iPhone 15".to_string(),
                udid: "SIM-UUID-1".to_string(),
                runtime: Some("iOS 17.5".to_string()),
            }],
        }
    }

    fn process_config(tool_path: &str) -> XcodeHostExecutorProcessConfig {
        XcodeHostExecutorProcessConfig {
            tool_paths: BTreeMap::from([("xcodebuild".to_string(), tool_path.to_string())]),
            timeout: Duration::from_secs(5),
        }
    }

    #[cfg(unix)]
    struct StaticPeerInspector {
        expected_credentials: XcodeShimPeerCredentials,
        peer_process: XcodeShimProcessBinding,
    }

    #[cfg(unix)]
    impl XcodeShimProcessInspector for StaticPeerInspector {
        fn inspect_peer(
            &self,
            credentials: XcodeShimPeerCredentials,
        ) -> anyhow::Result<XcodeShimProcessBinding> {
            assert_eq!(credentials, self.expected_credentials);
            Ok(self.peer_process.clone())
        }
    }

    #[cfg(unix)]
    struct StaticGrantResolver {
        expected_token_id: String,
        resolved: XcodeShimResolvedDispatch,
    }

    #[cfg(unix)]
    #[async_trait]
    impl XcodeShimGrantResolver for StaticGrantResolver {
        async fn resolve_grant(&self, token_id: &str) -> anyhow::Result<XcodeShimResolvedDispatch> {
            assert_eq!(token_id, self.expected_token_id);
            Ok(self.resolved.clone())
        }
    }

    #[test]
    fn routes_xcodebuild_and_simctl_to_host_executor() {
        for tool in ["xcodebuild", "/usr/bin/simctl"] {
            let policy = XcodeShimCommandPolicy::evaluate(tool, &args(&["list", "devices"]));

            assert_eq!(policy.decision, XcodeShimRouteDecision::HostExecutor);
            assert!(policy.reason_code.is_none());
            assert!(policy.is_allowed());
        }
    }

    #[test]
    fn rejects_mcpbridge_as_broker_only() {
        for (tool, args) in [
            ("mcpbridge", vec![]),
            ("xcrun", args(&["mcpbridge"])),
            ("xcrun", args(&["--run", "mcpbridge"])),
            ("xcrun", args(&["--", "mcpbridge"])),
        ] {
            let policy = XcodeShimCommandPolicy::evaluate(tool, &args);

            assert_eq!(policy.decision, XcodeShimRouteDecision::Reject);
            assert_eq!(
                policy.reason_code.as_deref(),
                Some("p051_shim_mcpbridge_broker_only")
            );
            assert!(!policy.is_allowed());
        }
    }

    #[test]
    fn rejects_xcrun_find_for_guarded_tools() {
        for args in [
            args(&["--find", "xcodebuild"]),
            args(&["-f", "/usr/bin/simctl"]),
            args(&["--find", "mcpbridge"]),
        ] {
            let policy = XcodeShimCommandPolicy::evaluate("xcrun", &args);

            assert_eq!(policy.decision, XcodeShimRouteDecision::Reject);
            assert_eq!(
                policy.reason_code.as_deref(),
                Some("p051_shim_xcrun_find_guarded_tool")
            );
        }
    }

    #[test]
    fn rejects_unknown_xcrun_flags() {
        for args in [
            args(&["--diagnose", "simctl", "list"]),
            args(&["--sdk", "iphonesimulator", "--unknown", "simctl"]),
            args(&["-unknown", "xcodebuild"]),
        ] {
            let policy = XcodeShimCommandPolicy::evaluate("xcrun", &args);

            assert_eq!(policy.decision, XcodeShimRouteDecision::Reject);
            assert_eq!(
                policy.reason_code.as_deref(),
                Some("p051_shim_xcrun_unknown_flag")
            );
        }
    }

    #[test]
    fn allows_known_xcrun_modes() {
        for (args, decision) in [
            (
                args(&["--sdk", "iphonesimulator", "simctl", "list", "devices"]),
                XcodeShimRouteDecision::HostExecutor,
            ),
            (
                args(&["--toolchain=default", "--run", "swift", "--version"]),
                XcodeShimRouteDecision::XcrunPassthrough,
            ),
            (
                args(&["--show-sdk-path", "--sdk", "iphoneos"]),
                XcodeShimRouteDecision::XcrunPassthrough,
            ),
            (
                args(&["--find", "swift"]),
                XcodeShimRouteDecision::XcrunPassthrough,
            ),
            (
                args(&["--", "swift", "--version"]),
                XcodeShimRouteDecision::XcrunPassthrough,
            ),
        ] {
            let policy = XcodeShimCommandPolicy::evaluate("xcrun", &args);

            assert_eq!(policy.decision, decision);
            assert!(policy.reason_code.is_none());
        }
    }

    #[test]
    fn host_executor_plan_enforces_cwd_env_and_simulator_uuid_rewrite() {
        let plan = XcodeHostExecutorPlan::build(host_input()).expect("host plan");

        assert_eq!(plan.tool, "xcodebuild");
        assert_eq!(plan.cwd, "/workspace/project/Chainworks Forge");
        assert_eq!(
            plan.argv,
            args(&[
                "-scheme",
                "Chainworks Forge",
                "-destination",
                "platform=iOS Simulator,name=iPhone 15,id=SIM-UUID-1",
                "test",
            ])
        );
        assert_eq!(plan.selected_simulator_id.as_deref(), Some("SIM-UUID-1"));
        assert_eq!(
            plan.env_allowlist_applied,
            args(&["CONFIGURATION", "SCHEME", "XCODE_XCCONFIG_FILE"])
        );
        assert_eq!(plan.env_dropped_from_provider, args(&["HOME"]));
        assert_eq!(
            plan.env.get("SCHEME").map(String::as_str),
            Some("Chainworks Forge")
        );
        assert!(!plan.env.contains_key("HOME"));
    }

    #[test]
    fn host_executor_plan_rewrites_guarded_xcrun_invocations_to_host_tool() {
        for (xcrun_args, expected_tool, expected_argv) in [
            (
                args(&["--sdk", "iphonesimulator", "simctl", "list", "devices"]),
                "simctl",
                args(&["list", "devices"]),
            ),
            (
                args(&["--run", "xcodebuild", "-version"]),
                "xcodebuild",
                args(&["-version"]),
            ),
            (
                args(&["--", "/usr/bin/xcodebuild", "-showsdks"]),
                "xcodebuild",
                args(&["-showsdks"]),
            ),
        ] {
            let mut input = host_input();
            input.invoked_tool = "xcrun".to_string();
            input.args = xcrun_args;

            let plan = XcodeHostExecutorPlan::build(input).expect("host plan");

            assert_eq!(plan.tool, expected_tool);
            assert_eq!(plan.argv, expected_argv);
        }
    }

    #[test]
    fn host_executor_plan_rejects_cwd_outside_workspace() {
        for cwd in ["../outside", "/tmp/outside"] {
            let mut input = host_input();
            input.cwd = cwd.to_string();

            let error = XcodeHostExecutorPlan::build(input).expect_err("outside workspace");

            assert_eq!(
                error.reason_code,
                "p051_host_executor_cwd_outside_workspace"
            );
        }
    }

    #[test]
    fn host_executor_plan_rejects_non_host_executor_policy() {
        let mut input = host_input();
        input.invoked_tool = "xcrun".to_string();
        input.args = args(&["--find", "swift"]);

        let error = XcodeHostExecutorPlan::build(input).expect_err("non host executor route");

        assert_eq!(error.reason_code, "p051_host_executor_not_allowed");
    }

    #[test]
    fn host_executor_plan_reports_ambiguous_simulator_destinations() {
        let mut input = host_input();
        input
            .simulator_candidates
            .push(XcodeHostExecutorSimulatorCandidate {
                name: "iPhone 15".to_string(),
                udid: "SIM-UUID-2".to_string(),
                runtime: Some("iOS 18.0".to_string()),
            });

        let error = XcodeHostExecutorPlan::build(input).expect_err("ambiguous simulator");

        assert_eq!(error.reason_code, "p051_simulator_destination_ambiguous");
        assert_eq!(
            error.candidate_simulator_ids,
            args(&["SIM-UUID-1", "SIM-UUID-2"])
        );
    }

    #[tokio::test]
    async fn host_executor_process_captures_output_status_and_event() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&["-c", "printf host-stdout; printf host-stderr >&2; exit 7"]);
        input.provider_env = BTreeMap::from([
            ("SCHEME".to_string(), "Chainworks Forge".to_string()),
            ("HOME".to_string(), "/tmp/provider-home".to_string()),
        ]);
        let plan = XcodeHostExecutorPlan::build(input).expect("host plan");

        let output = plan
            .execute_process(&process_config("/bin/sh"))
            .await
            .expect("process output");

        assert_eq!(output.stdout, "host-stdout");
        assert_eq!(output.stderr, "host-stderr");
        assert_eq!(output.event.tool, "xcodebuild");
        assert_eq!(output.event.exit_status, 7);
        assert_eq!(
            output.event.cwd,
            workspace.path().to_string_lossy().into_owned()
        );
        assert_eq!(output.event.env_allowlist_applied, args(&["SCHEME"]));
        assert_eq!(output.event.env_dropped_from_provider, args(&["HOME"]));
        assert!(output.event.duration_ms >= 0);
    }

    #[tokio::test]
    async fn host_executor_process_reports_spawn_failure() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&["-version"]);
        let plan = XcodeHostExecutorPlan::build(input).expect("host plan");

        let error = plan
            .execute_process(&process_config("/definitely/missing/xcodebuild"))
            .await
            .expect_err("spawn failure");

        assert_eq!(error.reason_code, "p051_host_executor_spawn_failed");
    }

    #[tokio::test]
    async fn host_executor_process_reports_timeout() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&["-c", "sleep 5"]);
        let plan = XcodeHostExecutorPlan::build(input).expect("host plan");
        let mut config = process_config("/bin/sh");
        config.timeout = Duration::from_millis(10);

        let error = plan.execute_process(&config).await.expect_err("timeout");

        assert_eq!(error.reason_code, "p051_host_executor_timeout");
    }

    #[tokio::test]
    async fn dispatch_executes_host_plan_and_persists_runtime_events() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&["-c", "printf dispatch-stdout; exit 3"]);
        let request = XcodeShimDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            grant: grant(),
            attempt: attempt(),
            plan_input: input,
        };
        let sink = CapturingObservationSink::default();

        let outcome = dispatch_xcode_shim_request(request, &process_config("/bin/sh"), &sink).await;

        assert!(outcome.authorization.allowed);
        assert_eq!(outcome.exit_status, 3);
        assert_eq!(
            outcome
                .process_output
                .as_ref()
                .map(|output| output.stdout.as_str()),
            Some("dispatch-stdout")
        );
        let updates = sink.updates();
        assert_eq!(updates.len(), 2);
        match &updates[0] {
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimInvocation(
                event,
            )) => {
                assert_eq!(event.tool, "xcodebuild");
                assert_eq!(event.policy_decision, "host_executor");
                assert_eq!(event.policy_reason, "host_executor");
                assert_eq!(event.derived_peer_pid, 42);
                assert_eq!(event.claimed_provider_pid, 42);
                assert!(!event.peer_pid_mismatch);
                assert_eq!(event.exit_status, 3);
            }
            other => panic!("expected shim invocation event, got {other:?}"),
        }
        match &updates[1] {
            XcodeRuntimeObservationUpdate::XcodeHostExecutorEvent(event) => {
                assert_eq!(event.tool, "xcodebuild");
                assert_eq!(event.exit_status, 3);
                assert_eq!(event.cwd, workspace.path().to_string_lossy().into_owned());
            }
            other => panic!("expected host executor event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn dispatch_rejects_bad_token_before_host_process() {
        let mut attempt = attempt();
        attempt.token_secret = "wrong".to_string();
        let request = XcodeShimDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            grant: grant(),
            attempt,
            plan_input: host_input(),
        };
        let sink = CapturingObservationSink::default();

        let outcome = dispatch_xcode_shim_request(request, &process_config("/bin/sh"), &sink).await;

        assert!(!outcome.authorization.allowed);
        assert_eq!(
            outcome.reason_code.as_deref(),
            Some("p051_shim_token_mismatch")
        );
        assert!(outcome.plan.is_none());
        assert!(outcome.process_output.is_none());
        let updates = sink.updates();
        assert_eq!(updates.len(), 1);
        match &updates[0] {
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimInvocation(
                event,
            )) => {
                assert_eq!(event.policy_decision, "host_executor");
                assert_eq!(event.policy_reason, "p051_shim_token_mismatch");
                assert_eq!(event.exit_status, 126);
            }
            other => panic!("expected shim invocation event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn socket_dispatch_uses_server_derived_peer_process() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&["-c", "printf socket-stdout; exit 4"]);
        let request = XcodeShimSocketDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            token_id: "token-a".to_string(),
            token_secret: "secret-a".to_string(),
            now_epoch_ms: 1_500,
            active_prompt: true,
            plan_input: input,
        };
        let sink = Arc::new(CapturingObservationSink::default());

        #[cfg(unix)]
        {
            let (client, server) = UnixStream::pair().expect("unix stream pair");
            let grant = grant();
            let config = process_config("/bin/sh");
            let handler_sink = sink.clone();
            let handler = tokio::spawn(async move {
                handle_xcode_shim_unix_stream(
                    server,
                    grant,
                    provider_process(),
                    &config,
                    &*handler_sink,
                )
                .await
            });

            let (client_reader, mut client_writer) = client.into_split();
            let mut payload = serde_json::to_value(&request).expect("request json");
            payload["peer_process"] = serde_json::json!({
                "pid": 999,
                "uid": 501,
                "parent_pid": 999,
                "start_time_fingerprint": "forged",
                "executable_fingerprint": "forged"
            });
            client_writer
                .write_all(serde_json::to_string(&payload).expect("payload").as_bytes())
                .await
                .expect("write payload");
            client_writer.write_all(b"\n").await.expect("write newline");
            client_writer
                .shutdown()
                .await
                .expect("shutdown client write");

            let mut response_line = String::new();
            let mut reader = BufReader::new(client_reader);
            reader
                .read_line(&mut response_line)
                .await
                .expect("read response");
            let response: XcodeShimDispatchOutcome =
                serde_json::from_str(&response_line).expect("response json");
            let handler_outcome = handler
                .await
                .expect("handler task")
                .expect("handler result");

            assert!(response.authorization.allowed);
            assert_eq!(response.exit_status, 4);
            assert_eq!(handler_outcome.exit_status, 4);
            assert_eq!(
                response
                    .process_output
                    .as_ref()
                    .map(|output| output.stdout.as_str()),
                Some("socket-stdout")
            );
        }

        let updates = sink.updates();
        assert_eq!(updates.len(), 2);
        match &updates[0] {
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimInvocation(
                event,
            )) => {
                assert_eq!(event.derived_peer_pid, 42);
                assert_eq!(event.claimed_provider_pid, 42);
                assert!(!event.peer_pid_mismatch);
                assert_eq!(event.exit_status, 4);
            }
            other => panic!("expected shim invocation event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn socket_dispatch_can_derive_peer_process_from_unix_credentials() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&["-c", "printf credential-stdout; exit 5"]);
        let request = XcodeShimSocketDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            token_id: "token-live".to_string(),
            token_secret: "secret-live".to_string(),
            now_epoch_ms: 1_500,
            active_prompt: true,
            plan_input: input,
        };
        let sink = Arc::new(CapturingObservationSink::default());

        #[cfg(unix)]
        {
            let (client, server) = UnixStream::pair().expect("unix stream pair");
            let credentials = xcode_shim_peer_credentials(&server).expect("peer credentials");
            assert!(credentials.pid > 0);
            assert_eq!(credentials.uid, unsafe { libc::getuid() });

            let peer_process = XcodeShimProcessBinding {
                pid: credentials.pid,
                uid: credentials.uid,
                parent_pid: Some(77),
                ancestor_pids: Vec::new(),
                start_time_fingerprint: Some("live-start".to_string()),
                executable_fingerprint: Some("live-executable".to_string()),
            };
            let grant = XcodeShimDispatchGrant::new(
                "token-live",
                "secret-live",
                "lease-live",
                peer_process.clone(),
                1_000,
                2_000,
            );
            let config = process_config("/bin/sh");
            let handler_sink = sink.clone();
            let handler = tokio::spawn(async move {
                let inspector = StaticPeerInspector {
                    expected_credentials: credentials,
                    peer_process,
                };
                handle_xcode_shim_unix_stream_with_peer_credentials(
                    server,
                    grant,
                    &config,
                    &*handler_sink,
                    &inspector,
                )
                .await
            });

            let (client_reader, mut client_writer) = client.into_split();
            let mut payload = serde_json::to_value(&request).expect("request json");
            payload["peer_process"] = serde_json::json!({
                "pid": 999,
                "uid": 999,
                "parent_pid": 999,
                "start_time_fingerprint": "forged",
                "executable_fingerprint": "forged"
            });
            client_writer
                .write_all(serde_json::to_string(&payload).expect("payload").as_bytes())
                .await
                .expect("write payload");
            client_writer.write_all(b"\n").await.expect("write newline");
            client_writer
                .shutdown()
                .await
                .expect("shutdown client write");

            let mut response_line = String::new();
            let mut reader = BufReader::new(client_reader);
            reader
                .read_line(&mut response_line)
                .await
                .expect("read response");
            let response: XcodeShimDispatchOutcome =
                serde_json::from_str(&response_line).expect("response json");
            let handler_outcome = handler
                .await
                .expect("handler task")
                .expect("handler result");

            assert!(response.authorization.allowed);
            assert_eq!(response.exit_status, 5);
            assert_eq!(handler_outcome.exit_status, 5);
            assert_eq!(
                response
                    .process_output
                    .as_ref()
                    .map(|output| output.stdout.as_str()),
                Some("credential-stdout")
            );
        }

        let updates = sink.updates();
        assert_eq!(updates.len(), 2);
        match &updates[0] {
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimInvocation(
                event,
            )) => {
                assert_eq!(event.policy_reason, "host_executor");
                assert_eq!(event.exit_status, 5);
                assert!(!event.peer_pid_mismatch);
            }
            other => panic!("expected shim invocation event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn socket_dispatch_with_grant_resolver_overrides_client_active_prompt() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&["-c", "printf resolver-stdout; exit 6"]);
        let request = XcodeShimSocketDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            token_id: "token-live".to_string(),
            token_secret: "secret-live".to_string(),
            now_epoch_ms: 1_500,
            active_prompt: false,
            plan_input: input,
        };
        let sink = Arc::new(CapturingObservationSink::default());

        #[cfg(unix)]
        {
            let (client, server) = UnixStream::pair().expect("unix stream pair");
            let credentials = xcode_shim_peer_credentials(&server).expect("peer credentials");
            let peer_process = XcodeShimProcessBinding {
                pid: credentials.pid,
                uid: credentials.uid,
                parent_pid: Some(77),
                ancestor_pids: Vec::new(),
                start_time_fingerprint: Some("live-start".to_string()),
                executable_fingerprint: Some("live-executable".to_string()),
            };
            let config = process_config("/bin/sh");
            let handler_sink = sink.clone();
            let handler = tokio::spawn(async move {
                let inspector = StaticPeerInspector {
                    expected_credentials: credentials,
                    peer_process: peer_process.clone(),
                };
                let resolver = StaticGrantResolver {
                    expected_token_id: "token-live".to_string(),
                    resolved: XcodeShimResolvedDispatch {
                        grant: XcodeShimDispatchGrant::new(
                            "token-live",
                            "secret-live",
                            "lease-live",
                            peer_process,
                            1_000,
                            2_000,
                        ),
                        active_prompt: true,
                    },
                };
                handle_xcode_shim_unix_stream_with_grant_resolver(
                    server,
                    &config,
                    &*handler_sink,
                    &inspector,
                    &resolver,
                )
                .await
            });

            let (client_reader, mut client_writer) = client.into_split();
            client_writer
                .write_all(serde_json::to_string(&request).expect("payload").as_bytes())
                .await
                .expect("write payload");
            client_writer.write_all(b"\n").await.expect("write newline");
            client_writer
                .shutdown()
                .await
                .expect("shutdown client write");

            let mut response_line = String::new();
            let mut reader = BufReader::new(client_reader);
            reader
                .read_line(&mut response_line)
                .await
                .expect("read response");
            let response: XcodeShimDispatchOutcome =
                serde_json::from_str(&response_line).expect("response json");
            let handler_outcome = handler
                .await
                .expect("handler task")
                .expect("handler result");

            assert!(response.authorization.allowed);
            assert_eq!(response.exit_status, 6);
            assert_eq!(handler_outcome.exit_status, 6);
        }

        let updates = sink.updates();
        assert_eq!(updates.len(), 2);
        match &updates[0] {
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimInvocation(
                event,
            )) => {
                assert_eq!(event.policy_reason, "host_executor");
                assert_eq!(event.exit_status, 6);
            }
            other => panic!("expected shim invocation event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn socket_dispatch_with_grant_resolver_rejects_unknown_token() {
        let request = XcodeShimSocketDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            token_id: "missing-token".to_string(),
            token_secret: "secret-live".to_string(),
            now_epoch_ms: 1_500,
            active_prompt: true,
            plan_input: host_input(),
        };
        let sink = Arc::new(CapturingObservationSink::default());

        #[cfg(unix)]
        {
            let (client, server) = UnixStream::pair().expect("unix stream pair");
            let credentials = xcode_shim_peer_credentials(&server).expect("peer credentials");
            let handler_sink = sink.clone();
            let handler = tokio::spawn(async move {
                let inspector = StaticPeerInspector {
                    expected_credentials: credentials,
                    peer_process: provider_process(),
                };
                struct MissingGrantResolver;
                #[async_trait]
                impl XcodeShimGrantResolver for MissingGrantResolver {
                    async fn resolve_grant(
                        &self,
                        _token_id: &str,
                    ) -> anyhow::Result<XcodeShimResolvedDispatch> {
                        anyhow::bail!("xcode_shim_unknown_token_id");
                    }
                }
                let resolver = MissingGrantResolver;
                handle_xcode_shim_unix_stream_with_grant_resolver(
                    server,
                    &process_config("/bin/sh"),
                    &*handler_sink,
                    &inspector,
                    &resolver,
                )
                .await
            });

            let (client_reader, mut client_writer) = client.into_split();
            client_writer
                .write_all(serde_json::to_string(&request).expect("payload").as_bytes())
                .await
                .expect("write payload");
            client_writer.write_all(b"\n").await.expect("write newline");
            client_writer
                .shutdown()
                .await
                .expect("shutdown client write");
            drop(client_reader);

            let error = handler
                .await
                .expect("handler task")
                .expect_err("unknown token should fail");
            assert!(error.to_string().contains("xcode_shim_unknown_token_id"));
        }

        assert!(sink.updates().is_empty());
    }

    #[tokio::test]
    async fn socket_dispatch_response_redacts_token_bearing_surfaces() {
        let workspace = tempfile::tempdir().expect("workspace");
        let mut input = host_input();
        input.cwd = ".".to_string();
        input.workspace_root = workspace.path().to_string_lossy().into_owned();
        input.args = args(&[
            "-c",
            "printf 'Bearer raw-stdout-token token=raw-stdout-query'; printf 'xcode-lease-raw-stderr-token authorization=raw-stderr-auth' >&2",
            "token=raw-argv-token",
        ]);
        input
            .provider_env
            .insert("SCHEME".to_string(), "token=raw-env-token".to_string());
        let request = XcodeShimSocketDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            token_id: "token-a".to_string(),
            token_secret: "secret-a".to_string(),
            now_epoch_ms: 1_500,
            active_prompt: true,
            plan_input: input,
        };
        let sink = Arc::new(CapturingObservationSink::default());

        #[cfg(unix)]
        {
            let (client, server) = UnixStream::pair().expect("unix stream pair");
            let grant = grant();
            let config = process_config("/bin/sh");
            let handler_sink = sink.clone();
            let handler = tokio::spawn(async move {
                handle_xcode_shim_unix_stream(
                    server,
                    grant,
                    provider_process(),
                    &config,
                    &*handler_sink,
                )
                .await
            });

            let (client_reader, mut client_writer) = client.into_split();
            client_writer
                .write_all(serde_json::to_string(&request).expect("payload").as_bytes())
                .await
                .expect("write payload");
            client_writer.write_all(b"\n").await.expect("write newline");
            client_writer
                .shutdown()
                .await
                .expect("shutdown client write");

            let mut response_line = String::new();
            let mut reader = BufReader::new(client_reader);
            reader
                .read_line(&mut response_line)
                .await
                .expect("read response");
            let response: XcodeShimDispatchOutcome =
                serde_json::from_str(&response_line).expect("response json");
            let handler_outcome = handler
                .await
                .expect("handler task")
                .expect("handler result");

            let serialized = serde_json::to_string(&response).expect("response string");
            assert_eq!(response, handler_outcome);
            assert!(!serialized.contains("raw-stdout-token"));
            assert!(!serialized.contains("raw-stdout-query"));
            assert!(!serialized.contains("raw-stderr-token"));
            assert!(!serialized.contains("raw-stderr-auth"));
            assert!(!serialized.contains("raw-argv-token"));
            assert!(!serialized.contains("raw-env-token"));
            assert!(serialized.contains("Bearer <redacted>"));
            assert!(serialized.contains("token=<redacted>"));
            assert!(serialized.contains("xcode-lease-<redacted>"));
            assert!(serialized.contains("authorization=<redacted>"));
        }

        assert_eq!(sink.updates().len(), 2);
    }

    #[tokio::test]
    async fn socket_dispatch_rejects_server_derived_peer_mismatch() {
        let mut peer_process = provider_process();
        peer_process.pid = 43;
        let request = XcodeShimSocketDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            token_id: "token-a".to_string(),
            token_secret: "secret-a".to_string(),
            now_epoch_ms: 1_500,
            active_prompt: true,
            plan_input: host_input(),
        };
        let sink = CapturingObservationSink::default();

        let outcome = dispatch_xcode_shim_socket_request(
            request,
            grant(),
            peer_process,
            &process_config("/bin/sh"),
            &sink,
        )
        .await;

        assert!(!outcome.authorization.allowed);
        assert_eq!(
            outcome.reason_code.as_deref(),
            Some("p051_shim_peer_pid_mismatch")
        );
        assert!(outcome.process_output.is_none());
        let updates = sink.updates();
        assert_eq!(updates.len(), 1);
        match &updates[0] {
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimInvocation(
                event,
            )) => {
                assert_eq!(event.derived_peer_pid, 43);
                assert_eq!(event.claimed_provider_pid, 42);
                assert!(event.peer_pid_mismatch);
                assert_eq!(event.policy_reason, "p051_shim_peer_pid_mismatch");
            }
            other => panic!("expected shim invocation event, got {other:?}"),
        }
    }

    #[test]
    fn authorizes_bound_provider_process_during_active_prompt() {
        let authorization = grant().authorize(&attempt());

        assert!(authorization.allowed);
        assert!(authorization.reason_code.is_none());
        assert_eq!(grant().token_sha256.len(), 64);
        assert_ne!(grant().token_sha256, "secret-a");
    }

    #[test]
    fn authorizes_descendant_shim_process_during_active_prompt() {
        let mut attempt = attempt();
        attempt.peer_process.pid = 99;
        attempt.peer_process.parent_pid = Some(42);
        attempt.peer_process.ancestor_pids = vec![42, 7];
        attempt.peer_process.start_time_fingerprint = None;
        attempt.peer_process.executable_fingerprint = None;

        let authorization = grant().authorize(&attempt);

        assert!(authorization.allowed);
        assert!(authorization.reason_code.is_none());
    }

    #[test]
    fn rejects_stale_and_mismatched_shim_tokens() {
        for (attempt, reason) in [
            {
                let mut attempt = attempt();
                attempt.now_epoch_ms = 2_001;
                (attempt, "p051_shim_token_stale")
            },
            {
                let mut attempt = attempt();
                attempt.token_secret = "wrong".to_string();
                (attempt, "p051_shim_token_mismatch")
            },
            {
                let mut attempt = attempt();
                attempt.token_id = "other".to_string();
                (attempt, "p051_shim_token_id_mismatch")
            },
        ] {
            let authorization = grant().authorize(&attempt);

            assert!(!authorization.allowed);
            assert_eq!(authorization.reason_code.as_deref(), Some(reason));
        }
    }

    #[test]
    fn rejects_same_uid_replay_from_different_process() {
        let mut attempt = attempt();
        attempt.peer_process.pid = 43;

        let authorization = grant().authorize(&attempt);

        assert!(!authorization.allowed);
        assert_eq!(
            authorization.reason_code.as_deref(),
            Some("p051_shim_peer_pid_mismatch")
        );
    }

    #[test]
    fn rejects_forged_or_reused_process_identity() {
        for (attempt, reason) in [
            {
                let mut attempt = attempt();
                attempt.peer_process.parent_pid = Some(8);
                (attempt, "p051_shim_process_tree_mismatch")
            },
            {
                let mut attempt = attempt();
                attempt.peer_process.start_time_fingerprint = Some("reused-pid".to_string());
                (attempt, "p051_shim_process_start_mismatch")
            },
            {
                let mut attempt = attempt();
                attempt.peer_process.executable_fingerprint = Some("other-binary".to_string());
                (attempt, "p051_shim_process_fingerprint_mismatch")
            },
        ] {
            let authorization = grant().authorize(&attempt);

            assert!(!authorization.allowed);
            assert_eq!(authorization.reason_code.as_deref(), Some(reason));
        }
    }

    #[test]
    fn rejects_dispatch_outside_active_prompt_window() {
        let mut attempt = attempt();
        attempt.active_prompt = false;

        let authorization = grant().authorize(&attempt);

        assert!(!authorization.allowed);
        assert_eq!(
            authorization.reason_code.as_deref(),
            Some("p051_shim_no_active_prompt")
        );
    }
}
