pub mod auggie;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod junie;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::session::AcpSessionHandle;
use crate::transport::AcpSessionConfig;
#[cfg(unix)]
use crate::{current_process_uid, inspect_xcode_shim_process_binding};
use crate::{ExecutionRequest, ExecutionResult, XcodeShimGrantRecord, XcodeShimGrantStore};

/// Process launch details prepared independently from `session/new` params.
#[derive(Clone)]
pub struct AcpLaunchSpec {
    pub binary_path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cleanup_paths: Vec<PathBuf>,
    pub xcode_shim_runtime: Option<XcodeShimLaunchRuntime>,
    expected_capability_fingerprint: Option<CapabilitySliceFingerprint>,
}

impl std::fmt::Debug for AcpLaunchSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcpLaunchSpec")
            .field("binary_path", &self.binary_path)
            .field("args", &self.args)
            .field("env", &self.env)
            .field("cleanup_paths", &self.cleanup_paths)
            .field("xcode_shim_runtime", &self.xcode_shim_runtime)
            .field(
                "expected_capability_fingerprint",
                &self.expected_capability_fingerprint,
            )
            .finish()
    }
}

#[derive(Clone)]
pub struct XcodeShimLaunchRuntime {
    pub token_id: String,
    pub token_secret: String,
    pub lease_id: String,
    pub socket_path: String,
    pub shim_dir: String,
    pub workspace_root: String,
    pub agent_execution_id: Option<domain::ids::AgentExecutionId>,
    pub store: std::sync::Arc<dyn XcodeShimGrantStore>,
}

impl std::fmt::Debug for XcodeShimLaunchRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("XcodeShimLaunchRuntime")
            .field("token_id", &self.token_id)
            .field("lease_id", &self.lease_id)
            .field("socket_path", &self.socket_path)
            .field("shim_dir", &self.shim_dir)
            .field("workspace_root", &self.workspace_root)
            .field("agent_execution_id", &self.agent_execution_id)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct XcodeShimGrantCleanup {
    token_id: String,
    store: std::sync::Arc<dyn XcodeShimGrantStore>,
}

impl XcodeShimGrantCleanup {
    pub fn set_active_prompt(&self, active_prompt: bool) {
        let _ = self
            .store
            .set_xcode_shim_grant_active_prompt(&self.token_id, active_prompt);
    }

    pub fn remove(&self) {
        let _ = self.store.remove_xcode_shim_grant(&self.token_id);
    }
}

impl AcpLaunchSpec {
    pub fn new(binary_path: impl Into<String>) -> Self {
        Self {
            binary_path: binary_path.into(),
            args: Vec::new(),
            env: Vec::new(),
            cleanup_paths: Vec::new(),
            xcode_shim_runtime: None,
            expected_capability_fingerprint: None,
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn with_envs(mut self, env: Vec<(String, String)>) -> Self {
        self.env = env;
        self
    }

    pub fn capability_fingerprint(
        &self,
        runtime_profile_id: Option<&str>,
        adapter_settings_fingerprint: Option<&str>,
    ) -> CapabilitySliceFingerprint {
        CapabilitySliceFingerprint::from_launch_spec(
            self,
            runtime_profile_id,
            adapter_settings_fingerprint,
        )
    }

    pub fn record_capability_fingerprint(
        &mut self,
        runtime_profile_id: Option<&str>,
        adapter_settings_fingerprint: Option<&str>,
    ) -> CapabilitySliceFingerprint {
        let fingerprint =
            self.capability_fingerprint(runtime_profile_id, adapter_settings_fingerprint);
        self.expected_capability_fingerprint = Some(fingerprint.clone());
        fingerprint
    }

    pub fn apply_chainworks_meta_root_env(&mut self, req: &ExecutionRequest) {
        if let Some(meta_root) = chainworks_meta_root_env_value(req) {
            self.env.retain(|(name, _)| name != "CHAINWORKS_META_ROOT");
            self.env
                .push(("CHAINWORKS_META_ROOT".to_string(), meta_root));
        }
    }

    fn verify_capability_fingerprint(&self) -> Result<()> {
        let Some(expected) = &self.expected_capability_fingerprint else {
            return Ok(());
        };
        let actual = self.capability_fingerprint(
            expected.runtime_profile_id.as_deref(),
            Some(&expected.adapter_settings_sha256),
        );
        if &actual != expected {
            bail!(
                "provider_launch_spec_capability_drift: launch spec changed after capability preflight"
            );
        }
        Ok(())
    }

    pub fn attach_xcode_shim_runtime(&mut self, runtime: XcodeShimLaunchRuntime) {
        self.env.retain(|(name, _)| {
            !matches!(
                name.as_str(),
                "CHAINWORKS_XCODE_SHIM_TOKEN_ID"
                    | "CHAINWORKS_XCODE_SHIM_TOKEN"
                    | "CHAINWORKS_XCODE_SHIM_SOCKET"
                    | "CHAINWORKS_XCODE_SHIM_AGENT_EXECUTION_ID"
                    | "CHAINWORKS_XCODE_SHIM_WORKSPACE_ROOT"
                    | "CHAINWORKS_XCODE_SHIM_DIR"
            )
        });
        self.env.push((
            "CHAINWORKS_XCODE_SHIM_TOKEN_ID".into(),
            runtime.token_id.clone(),
        ));
        self.env.push((
            "CHAINWORKS_XCODE_SHIM_TOKEN".into(),
            runtime.token_secret.clone(),
        ));
        self.env.push((
            "CHAINWORKS_XCODE_SHIM_SOCKET".into(),
            runtime.socket_path.clone(),
        ));
        self.env.push((
            "CHAINWORKS_XCODE_SHIM_WORKSPACE_ROOT".into(),
            runtime.workspace_root.clone(),
        ));
        self.env
            .push(("CHAINWORKS_XCODE_SHIM_DIR".into(), runtime.shim_dir.clone()));
        if let Some(agent_execution_id) = runtime.agent_execution_id {
            self.env.push((
                "CHAINWORKS_XCODE_SHIM_AGENT_EXECUTION_ID".into(),
                agent_execution_id.to_string(),
            ));
        }
        prepend_path_env(&mut self.env, &runtime.shim_dir);
        self.xcode_shim_runtime = Some(runtime);
    }

    pub fn register_xcode_shim_grant_for_child(
        &self,
        child: &tokio::process::Child,
    ) -> Result<Option<XcodeShimGrantCleanup>> {
        let Some(runtime) = &self.xcode_shim_runtime else {
            return Ok(None);
        };
        let pid = child
            .id()
            .ok_or_else(|| anyhow::anyhow!("p051_xcode_shim_provider_pid_unavailable"))?;
        let now_epoch_ms = chrono::Utc::now().timestamp_millis();
        #[cfg(unix)]
        let provider_process = inspect_xcode_shim_process_binding(pid, current_process_uid())
            .context("p051_xcode_shim_provider_process_inspection_failed")?;
        #[cfg(not(unix))]
        let provider_process = crate::XcodeShimProcessBinding {
            pid,
            uid: 0,
            parent_pid: None,
            ancestor_pids: Vec::new(),
            start_time_fingerprint: None,
            executable_fingerprint: None,
        };
        let grant = crate::XcodeShimDispatchGrant::new(
            runtime.token_id.clone(),
            &runtime.token_secret,
            runtime.lease_id.clone(),
            provider_process,
            now_epoch_ms,
            now_epoch_ms + 6 * 60 * 60 * 1000,
        );
        runtime.store.insert_xcode_shim_grant(XcodeShimGrantRecord {
            grant,
            active_prompt: false,
        });
        Ok(Some(XcodeShimGrantCleanup {
            token_id: runtime.token_id.clone(),
            store: runtime.store.clone(),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BinaryFingerprint {
    pub canonical_path: String,
    pub size_bytes: Option<u64>,
    pub modified_unix_millis: Option<u128>,
    pub sha256: Option<String>,
    pub hash_unavailable: bool,
}

impl BinaryFingerprint {
    pub fn from_binary_path(binary_path: &str) -> Self {
        let resolved_path = resolve_binary_path(binary_path);
        let canonical_path = resolved_path
            .as_ref()
            .and_then(|path| path.canonicalize().ok())
            .unwrap_or_else(|| resolved_path.unwrap_or_else(|| PathBuf::from(binary_path)));

        let metadata = std::fs::metadata(&canonical_path).ok();
        let size_bytes = metadata.as_ref().map(|metadata| metadata.len());
        let modified_unix_millis = metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis());

        let sha256 = std::fs::read(&canonical_path)
            .ok()
            .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
        let hash_unavailable = sha256.is_none();

        Self {
            canonical_path: canonical_path.to_string_lossy().into_owned(),
            size_bytes,
            modified_unix_millis,
            sha256,
            hash_unavailable,
        }
    }
}

fn resolve_binary_path(binary_path: &str) -> Option<PathBuf> {
    let path = Path::new(binary_path);
    if path.components().count() > 1 || path.is_absolute() {
        return Some(path.to_path_buf());
    }

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary_path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilitySliceFingerprint {
    pub runtime_profile_id: Option<String>,
    pub binary_fingerprint: BinaryFingerprint,
    pub ordered_launch_args_sha256: String,
    pub capability_env_sha256: String,
    pub adapter_settings_sha256: String,
    pub fingerprint_sha256: String,
}

impl CapabilitySliceFingerprint {
    pub fn from_launch_spec(
        launch_spec: &AcpLaunchSpec,
        runtime_profile_id: Option<&str>,
        adapter_settings_fingerprint: Option<&str>,
    ) -> Self {
        let binary_fingerprint = BinaryFingerprint::from_binary_path(&launch_spec.binary_path);
        let ordered_launch_args_sha256 = sha256_json(&launch_spec.args);
        let mut env = capability_env(&launch_spec.env);
        env.sort();
        let capability_env_sha256 = sha256_json(&env);
        let adapter_settings_sha256 = adapter_settings_fingerprint
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| sha256_json(&serde_json::json!({})));
        let runtime_profile_id = runtime_profile_id.map(ToOwned::to_owned);
        let fingerprint_sha256 = sha256_json(&serde_json::json!({
            "runtime_profile_id": runtime_profile_id,
            "binary_fingerprint": binary_fingerprint,
            "ordered_launch_args_sha256": ordered_launch_args_sha256,
            "capability_env_sha256": capability_env_sha256,
            "adapter_settings_sha256": adapter_settings_sha256,
        }));

        Self {
            runtime_profile_id,
            binary_fingerprint,
            ordered_launch_args_sha256,
            capability_env_sha256,
            adapter_settings_sha256,
            fingerprint_sha256,
        }
    }
}

fn sha256_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("capability fingerprint value should serialize");
    format!("{:x}", Sha256::digest(bytes))
}

fn capability_env(env: &[(String, String)]) -> Vec<(String, String)> {
    let shim_dir = env
        .iter()
        .find(|(name, _)| name == "CHAINWORKS_XCODE_SHIM_DIR")
        .map(|(_, value)| value.clone());
    env.iter()
        .filter_map(|(name, value)| {
            if name.starts_with("CHAINWORKS_XCODE_SHIM_") {
                return None;
            }
            if name == "PATH" {
                let value = strip_path_prefix(value, shim_dir.as_deref());
                return Some((name.clone(), value));
            }
            Some((name.clone(), value.clone()))
        })
        .collect()
}

fn strip_path_prefix(path_value: &str, prefix: Option<&str>) -> String {
    let Some(prefix) = prefix.filter(|prefix| !prefix.is_empty()) else {
        return path_value.to_string();
    };
    if path_value == prefix {
        return String::new();
    }
    path_value
        .strip_prefix(&format!("{prefix}:"))
        .unwrap_or(path_value)
        .to_string()
}

fn prepend_path_env(env: &mut Vec<(String, String)>, prefix: &str) {
    let current = env
        .iter()
        .find(|(name, _)| name == "PATH")
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var("PATH").ok())
        .unwrap_or_default();
    env.retain(|(name, _)| name != "PATH");
    let next = if current.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}:{current}")
    };
    env.push(("PATH".to_string(), next));
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProbeKey {
    pub adapter_family: String,
    pub runtime_profile_id: Option<String>,
    pub binary_fingerprint: BinaryFingerprint,
    pub ordered_launch_args_sha256: String,
    pub capability_env_sha256: String,
    pub adapter_settings_sha256: String,
}

impl ProbeKey {
    pub fn from_launch_spec(
        adapter_family: impl Into<String>,
        runtime_profile_id: Option<&str>,
        launch_spec: &AcpLaunchSpec,
        adapter_settings_fingerprint: Option<&str>,
    ) -> Self {
        let fingerprint =
            launch_spec.capability_fingerprint(runtime_profile_id, adapter_settings_fingerprint);
        Self {
            adapter_family: adapter_family.into(),
            runtime_profile_id: fingerprint.runtime_profile_id,
            binary_fingerprint: fingerprint.binary_fingerprint,
            ordered_launch_args_sha256: fingerprint.ordered_launch_args_sha256,
            capability_env_sha256: fingerprint.capability_env_sha256,
            adapter_settings_sha256: fingerprint.adapter_settings_sha256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub http_mcp: bool,
    pub protocol_version: Option<String>,
    pub server_name: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ProviderCapabilityCache {
    entries: Mutex<HashMap<ProbeKey, ProviderCapabilities>>,
}

impl ProviderCapabilityCache {
    pub fn get(&self, key: &ProbeKey) -> Option<ProviderCapabilities> {
        self.entries
            .lock()
            .expect("provider capability cache poisoned")
            .get(key)
            .cloned()
    }

    pub fn insert(&self, key: ProbeKey, capabilities: ProviderCapabilities) {
        self.entries
            .lock()
            .expect("provider capability cache poisoned")
            .insert(key, capabilities);
    }

    pub fn get_or_probe<F>(&self, key: ProbeKey, probe: F) -> Result<ProviderCapabilities>
    where
        F: FnOnce() -> Result<ProviderCapabilities>,
    {
        if let Some(capabilities) = self.get(&key) {
            return Ok(capabilities);
        }
        let capabilities = probe()?;
        self.insert(key, capabilities.clone());
        Ok(capabilities)
    }
}

fn parse_bool_path(value: &Value, path: &[&str]) -> Option<bool> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    cursor.as_bool()
}

fn parse_provider_capabilities(initialize_result: &Value) -> ProviderCapabilities {
    let http_mcp = [
        &["mcpCapabilities", "http"][..],
        &["mcp_capabilities", "http"][..],
        &["capabilities", "mcpCapabilities", "http"][..],
        &["capabilities", "mcp_capabilities", "http"][..],
        &["agentCapabilities", "mcpCapabilities", "http"][..],
        &["agentCapabilities", "mcp_capabilities", "http"][..],
        &["agent_capabilities", "mcp_capabilities", "http"][..],
    ]
    .iter()
    .find_map(|path| parse_bool_path(initialize_result, path))
    .unwrap_or(false);

    ProviderCapabilities {
        http_mcp,
        protocol_version: initialize_result
            .get("protocolVersion")
            .or_else(|| initialize_result.get("protocol_version"))
            .and_then(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| value.as_i64().map(|number| number.to_string()))
            }),
        server_name: initialize_result
            .pointer("/serverInfo/name")
            .or_else(|| initialize_result.pointer("/server_info/name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        notes: Vec::new(),
    }
}

fn brokered_xcode_intents(req: &ExecutionRequest) -> Vec<&crate::BrokeredXcodeMcpIntent> {
    req.brokered_xcode_intents()
}

/// Owned provider vocabulary for the ACP `session/new` request.
#[derive(Debug, Clone, PartialEq)]
pub struct AcpSessionNewSpec {
    pub model: String,
    pub mode: String,
    pub extra: Option<Value>,
    pub config_options: Vec<(String, String)>,
}

impl AcpSessionNewSpec {
    pub fn new(model: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            mode: mode.into(),
            extra: None,
            config_options: Vec::new(),
        }
    }

    pub fn from_config(config: AcpSessionConfig<'_>) -> Self {
        Self {
            model: config.model.to_string(),
            mode: config.mode.to_string(),
            extra: config.extra,
            config_options: config.config_options,
        }
    }

    pub fn as_config(&self) -> AcpSessionConfig<'_> {
        AcpSessionConfig {
            model: &self.model,
            mode: &self.mode,
            extra: self.extra.clone(),
            config_options: self.config_options.clone(),
        }
    }
}

/// Rolls back launch-time filesystem resources unless they are committed into
/// a live session cleanup plan.
#[derive(Debug, Default)]
pub struct LaunchResourceGuard {
    cleanup_paths: Vec<PathBuf>,
    committed: bool,
}

impl LaunchResourceGuard {
    pub fn add_cleanup_path(&mut self, path: impl Into<PathBuf>) {
        self.cleanup_paths.push(path.into());
    }

    pub fn commit(mut self) -> Vec<PathBuf> {
        self.committed = true;
        std::mem::take(&mut self.cleanup_paths)
    }
}

impl Drop for LaunchResourceGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for path in self.cleanup_paths.drain(..) {
            cleanup_path(&path);
        }
    }
}

fn cleanup_path(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

/// Common interface for all ACP provider adapters.
#[async_trait]
pub trait AcpAdapter: Send + Sync {
    /// Returns the canonical provider name for this adapter.
    fn provider_name(&self) -> &str;

    /// Prepare process launch details before any provider subprocess is
    /// spawned. Filesystem resources that need rollback before session transfer
    /// should be registered on `resources`.
    fn prepare_launch_spec(
        &self,
        _req: &ExecutionRequest,
        _resources: &mut LaunchResourceGuard,
    ) -> Result<AcpLaunchSpec> {
        bail!(
            "ACP adapter '{}' does not provide process launch specs",
            self.provider_name()
        )
    }

    /// Prepare provider-specific `session/new` params independently from
    /// process launch. Broker/capability code can inspect this without owning
    /// subprocess lifecycle.
    fn prepare_session_new_spec(&self, _req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
        bail!(
            "ACP adapter '{}' does not provide session/new specs",
            self.provider_name()
        )
    }

    /// Whether this adapter is in the P051 supported set for live HTTP MCP
    /// capability probing. Unsupported adapters fail before subprocess launch
    /// when brokered Xcode MCP is requested.
    fn supports_http_mcp_capability_probe(&self) -> bool {
        true
    }

    async fn probe_capabilities_from_launch_spec(
        &self,
        launch_spec: &AcpLaunchSpec,
    ) -> Result<ProviderCapabilities> {
        launch_spec.verify_capability_fingerprint()?;
        let mut command = Command::new(&launch_spec.binary_path);
        command
            .args(&launch_spec.args)
            .envs(launch_spec.env.clone())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);

        let child = command.spawn().with_context(|| {
            format!(
                "spawn {} ACP capability probe subprocess: {}",
                self.provider_name(),
                launch_spec.binary_path
            )
        })?;
        let initialize_result = crate::transport::probe_initialize(child).await?;
        Ok(parse_provider_capabilities(&initialize_result))
    }

    async fn ensure_brokered_xcode_http_capability(
        &self,
        req: &ExecutionRequest,
        launch_spec: &AcpLaunchSpec,
        capability_cache: &ProviderCapabilityCache,
    ) -> Result<()> {
        let intents = brokered_xcode_intents(req);
        if intents.is_empty() {
            return Ok(());
        }

        if !self.supports_http_mcp_capability_probe() {
            bail!(
                "provider_http_mcp_unsupported: provider '{}' does not advertise HTTP MCP support for brokered Xcode MCP",
                self.provider_name()
            );
        }

        let runtime_profile_id = intents
            .iter()
            .find_map(|intent| intent.runtime_profile_id.as_deref());
        let key =
            ProbeKey::from_launch_spec(self.provider_name(), runtime_profile_id, launch_spec, None);
        let capabilities = match capability_cache.get(&key) {
            Some(capabilities) => capabilities,
            None => {
                let capabilities = self
                    .probe_capabilities_from_launch_spec(launch_spec)
                    .await?;
                capability_cache.insert(key, capabilities.clone());
                capabilities
            }
        };

        if !capabilities.http_mcp {
            bail!(
                "provider_http_mcp_unsupported: provider '{}' initialize response did not advertise mcpCapabilities.http=true",
                self.provider_name()
            );
        }

        Ok(())
    }

    fn reject_unconverted_broker_intents(&self, req: &ExecutionRequest) -> Result<()> {
        if let Some(intent) = brokered_xcode_intents(req).into_iter().next() {
            bail!(
                "ACP: brokered Xcode MCP intent '{}' must be converted to an HTTP lease before session/new",
                intent.runtime_id
            );
        }
        Ok(())
    }

    /// Open a session from precomputed launch and session specs.
    async fn open_session_with_specs(
        &self,
        req: &ExecutionRequest,
        mut launch_spec: AcpLaunchSpec,
        session_new_spec: AcpSessionNewSpec,
    ) -> Result<AcpSessionHandle> {
        let mut command = Command::new(&launch_spec.binary_path);
        launch_spec.apply_chainworks_meta_root_env(req);
        launch_spec.verify_capability_fingerprint()?;
        command
            .args(&launch_spec.args)
            .envs(launch_spec.env.drain(..))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);

        let args = if launch_spec.args.is_empty() {
            String::new()
        } else {
            format!(" {}", launch_spec.args.join(" "))
        };
        let spawn_context = format!(
            "spawn {} ACP subprocess: {}{}",
            self.provider_name(),
            launch_spec.binary_path,
            args
        );
        let child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                for path in launch_spec.cleanup_paths.drain(..) {
                    cleanup_path(&path);
                }
                return Err(err).with_context(|| spawn_context);
            }
        };
        let xcode_shim_grant = match launch_spec.register_xcode_shim_grant_for_child(&child) {
            Ok(grant) => grant,
            Err(err) => {
                for path in launch_spec.cleanup_paths.drain(..) {
                    cleanup_path(&path);
                }
                return Err(err);
            }
        };

        let config = session_new_spec.as_config();
        let session = crate::session::AcpSession::start_with_cleanup_paths_and_xcode_shim_grants(
            child,
            req,
            &config,
            launch_spec.cleanup_paths,
            xcode_shim_grant.into_iter().collect(),
        )
        .await?;
        Ok(AcpSessionHandle::new(session))
    }

    /// Open a live transport-backed ACP session.
    async fn open_session(&self, req: &ExecutionRequest) -> Result<AcpSessionHandle> {
        let capability_cache = ProviderCapabilityCache::default();
        self.open_session_with_capability_cache(req, &capability_cache)
            .await
    }

    /// Open a live transport-backed ACP session using the caller-owned provider
    /// capability cache.
    async fn open_session_with_capability_cache(
        &self,
        req: &ExecutionRequest,
        capability_cache: &ProviderCapabilityCache,
    ) -> Result<AcpSessionHandle> {
        let mut resources = LaunchResourceGuard::default();
        let mut launch_spec = self.prepare_launch_spec(req, &mut resources)?;
        launch_spec.apply_chainworks_meta_root_env(req);
        launch_spec.record_capability_fingerprint(None, None);
        self.ensure_brokered_xcode_http_capability(req, &launch_spec, capability_cache)
            .await?;
        self.reject_unconverted_broker_intents(req)?;
        let session_new_spec = self.prepare_session_new_spec(req)?;
        launch_spec.cleanup_paths.extend(resources.commit());
        self.open_session_with_specs(req, launch_spec, session_new_spec)
            .await
    }

    /// Execute an agent session and return the result.
    async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
        let session = self.open_session(&req).await?;
        let mut result = match session.prompt(&req).await {
            Ok(result) => result,
            Err(err) => {
                let _ = session.close().await;
                return Err(err);
            }
        };
        session.close().await?;
        result.session_generation_id = None;
        Ok(result)
    }
}

fn chainworks_meta_root_env_value(req: &ExecutionRequest) -> Option<String> {
    let meta_root = req.chainworks_meta_root.as_ref()?;
    let meta_root_path = Path::new(meta_root);
    if meta_root_path.is_absolute() {
        return Some(meta_root.clone());
    }
    Some(
        Path::new(&req.workspace_root)
            .join(meta_root_path)
            .to_string_lossy()
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        AcpLaunchSpec, ProbeKey, ProviderCapabilities, ProviderCapabilityCache,
        XcodeShimLaunchRuntime,
    };
    use crate::{XcodeShimGrantRecord, XcodeShimGrantStore};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct CapturingGrantStore {
        inserted: Mutex<Vec<XcodeShimGrantRecord>>,
    }

    impl XcodeShimGrantStore for CapturingGrantStore {
        fn insert_xcode_shim_grant(&self, record: XcodeShimGrantRecord) {
            self.inserted
                .lock()
                .expect("inserted poisoned")
                .push(record);
        }

        fn set_xcode_shim_grant_active_prompt(
            &self,
            _token_id: &str,
            _active_prompt: bool,
        ) -> bool {
            false
        }

        fn remove_xcode_shim_grant(&self, _token_id: &str) -> Option<XcodeShimGrantRecord> {
            None
        }
    }

    #[test]
    fn probe_key_changes_when_binary_content_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = tmp.path().join("provider-acp");
        std::fs::write(&binary, "first").unwrap();

        let spec = AcpLaunchSpec::new(binary.to_string_lossy());
        let first = ProbeKey::from_launch_spec("codex", Some("profile-a"), &spec, None);

        std::fs::write(&binary, "second").unwrap();
        let second = ProbeKey::from_launch_spec("codex", Some("profile-a"), &spec, None);

        assert_ne!(
            first.binary_fingerprint.sha256,
            second.binary_fingerprint.sha256
        );
        assert_ne!(first, second);
    }

    #[test]
    fn provider_capability_cache_hits_only_identical_probe_key() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = tmp.path().join("provider-acp");
        std::fs::write(&binary, "fixture").unwrap();

        let mut first_spec = AcpLaunchSpec::new(binary.to_string_lossy());
        first_spec.args.push("--acp".to_string());
        let first_key = ProbeKey::from_launch_spec("gemini", None, &first_spec, None);

        let mut second_spec = first_spec.clone();
        second_spec.args.push("--debug".to_string());
        let second_key = ProbeKey::from_launch_spec("gemini", None, &second_spec, None);

        let cache = ProviderCapabilityCache::default();
        cache.insert(
            first_key.clone(),
            ProviderCapabilities {
                http_mcp: true,
                protocol_version: Some("1".to_string()),
                server_name: Some("fixture".to_string()),
                notes: Vec::new(),
            },
        );

        assert!(cache.get(&first_key).is_some());
        assert!(cache.get(&second_key).is_none());
    }

    #[test]
    fn launch_spec_rejects_capability_drift_after_preflight() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = tmp.path().join("provider-acp");
        std::fs::write(&binary, "fixture").unwrap();

        let mut spec = AcpLaunchSpec::new(binary.to_string_lossy());
        spec.record_capability_fingerprint(Some("profile-a"), None);
        spec.args.push("--changed-after-preflight".to_string());

        let err = spec.verify_capability_fingerprint().unwrap_err();
        assert!(
            err.to_string()
                .contains("provider_launch_spec_capability_drift"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn chainworks_meta_root_is_frozen_before_capability_fingerprint() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = tmp.path().join("provider-acp");
        std::fs::write(&binary, "fixture").unwrap();

        let req = crate::ExecutionRequest {
            agent_execution_id: None,
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_meta".to_string(),
            attempt_number: 1,
            agent_id: "agent_meta".to_string(),
            provider: "claude".to_string(),
            model: None,
            effort: None,
            workspace_root: tmp.path().to_string_lossy().into_owned(),
            prompt: "prompt".to_string(),
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
            chainworks_meta_root: Some(".chainworks/run-meta".to_string()),
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
        };

        let mut spec = AcpLaunchSpec::new(binary.to_string_lossy());
        spec.apply_chainworks_meta_root_env(&req);
        let expected = spec.record_capability_fingerprint(Some("profile-a"), None);

        assert!(spec
            .env
            .iter()
            .any(|(name, _)| name == "CHAINWORKS_META_ROOT"));
        assert_eq!(
            expected,
            spec.capability_fingerprint(Some("profile-a"), None)
        );

        spec.env.retain(|(name, _)| name != "CHAINWORKS_META_ROOT");
        spec.env.push((
            "CHAINWORKS_META_ROOT".to_string(),
            tmp.path()
                .join(".chainworks/other")
                .to_string_lossy()
                .into_owned(),
        ));
        let err = spec.verify_capability_fingerprint().unwrap_err();
        assert!(
            err.to_string()
                .contains("provider_launch_spec_capability_drift"),
            "unexpected error: {err:#}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn register_xcode_shim_grant_for_child_captures_live_process_binding() {
        let store = Arc::new(CapturingGrantStore::default());
        let mut spec = AcpLaunchSpec::new("/bin/sh");
        spec.attach_xcode_shim_runtime(XcodeShimLaunchRuntime {
            token_id: "token-live".to_string(),
            token_secret: "secret-live".to_string(),
            lease_id: "lease-live".to_string(),
            socket_path: "/tmp/xcode-shim.sock".to_string(),
            shim_dir: "/tmp/xcode-shims".to_string(),
            workspace_root: "/tmp".to_string(),
            agent_execution_id: None,
            store: store.clone(),
        });

        let mut child = tokio::process::Command::new("/bin/sh")
            .args(["-c", "sleep 5"])
            .spawn()
            .unwrap();
        let child_pid = child.id().unwrap();

        let cleanup = spec
            .register_xcode_shim_grant_for_child(&child)
            .unwrap()
            .unwrap();

        let record = store
            .inserted
            .lock()
            .expect("inserted poisoned")
            .last()
            .cloned()
            .expect("grant record");
        let expected =
            crate::inspect_xcode_shim_process_binding(child_pid, crate::current_process_uid())
                .unwrap();

        assert_eq!(record.grant.provider_process, expected);
        assert_eq!(record.grant.provider_process.pid, child_pid);
        assert_eq!(
            record.grant.provider_process.uid,
            crate::current_process_uid()
        );
        assert_eq!(
            record
                .grant
                .provider_process
                .start_time_fingerprint
                .as_ref()
                .map(String::len),
            Some(64)
        );
        assert_eq!(
            record
                .grant
                .provider_process
                .executable_fingerprint
                .as_ref()
                .map(String::len),
            Some(64)
        );

        cleanup.remove();
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}
