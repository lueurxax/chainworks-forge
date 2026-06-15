pub mod auggie;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod junie;

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::process::Command;

use crate::session::{
    AcpSessionCloseBehavior, AcpSessionHandle, ProviderSessionStoreArchiveContext,
};
use crate::transport::AcpSessionConfig;
#[cfg(unix)]
use crate::{current_process_uid, inspect_xcode_shim_process_binding};
use crate::{ExecutionRequest, ExecutionResult, XcodeShimGrantRecord, XcodeShimGrantStore};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupPathPolicy {
    DeleteRecursively,
    StageCodexSessionStore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPathSpec {
    pub path: PathBuf,
    pub policy: CleanupPathPolicy,
}

impl CleanupPathSpec {
    pub fn delete(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            policy: CleanupPathPolicy::DeleteRecursively,
        }
    }

    pub fn stage_codex_session_store(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            policy: CleanupPathPolicy::StageCodexSessionStore,
        }
    }
}

/// Process launch details prepared independently from `session/new` params.
#[derive(Clone)]
pub struct AcpLaunchSpec {
    pub binary_path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cleanup_paths: Vec<CleanupPathSpec>,
    pub current_dir_override: Option<PathBuf>,
    pub runtime_tool_path_preflight_json: Option<String>,
    pub provider_launch_gate: Option<Arc<dyn AcpProviderLaunchGate>>,
    pub xcode_shim_runtime: Option<XcodeShimLaunchRuntime>,
    pub provider_runtime_home: Option<PathBuf>,
    expected_capability_fingerprint: Option<CapabilitySliceFingerprint>,
}

impl std::fmt::Debug for AcpLaunchSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcpLaunchSpec")
            .field("binary_path", &self.binary_path)
            .field("args", &self.args)
            .field("env", &self.env)
            .field("cleanup_paths", &self.cleanup_paths)
            .field("current_dir_override", &self.current_dir_override)
            .field(
                "runtime_tool_path_preflight_json",
                &self.runtime_tool_path_preflight_json,
            )
            .field(
                "provider_launch_gate",
                &self.provider_launch_gate.as_ref().map(|_| "<configured>"),
            )
            .field("xcode_shim_runtime", &self.xcode_shim_runtime)
            .field("provider_runtime_home", &self.provider_runtime_home)
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
        let binary_path = binary_path.into();
        let binary_path = resolve_binary_path(&binary_path)
            .unwrap_or_else(|| PathBuf::from(&binary_path))
            .to_string_lossy()
            .into_owned();
        Self {
            binary_path,
            args: Vec::new(),
            env: vec![("PATH".to_string(), default_provider_path_value())],
            cleanup_paths: Vec::new(),
            current_dir_override: None,
            runtime_tool_path_preflight_json: None,
            provider_launch_gate: None,
            xcode_shim_runtime: None,
            provider_runtime_home: None,
            expected_capability_fingerprint: None,
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn with_envs(mut self, env: Vec<(String, String)>) -> Self {
        self.env = env;
        ensure_default_path_env(&mut self.env);
        self
    }

    pub fn with_provider_runtime_home(mut self, path: impl Into<PathBuf>) -> Self {
        self.provider_runtime_home = Some(path.into());
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

    pub fn apply_chainworks_rust_cache_env(&mut self) {
        upsert_env(
            &mut self.env,
            "CARGO_TARGET_DIR",
            chainworks_shared_cargo_target_dir(),
        );
        if !has_env(&self.env, "SCCACHE_DIR") {
            self.env
                .push(("SCCACHE_DIR".to_string(), chainworks_sccache_dir()));
        }
        if !has_env(&self.env, "SCCACHE_CACHE_SIZE") {
            self.env
                .push(("SCCACHE_CACHE_SIZE".to_string(), "20G".to_string()));
        }
        if !has_env(&self.env, "RUSTC_WRAPPER") {
            if let Some(sccache) = find_sccache_binary() {
                self.env.push(("RUSTC_WRAPPER".to_string(), sccache));
            }
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

#[async_trait]
pub trait AcpProviderLaunchGate: Send + Sync {
    async fn before_provider_launch(
        &self,
        req: &ExecutionRequest,
        runtime_tool_path_preflight_json: Option<&str>,
    ) -> Result<()>;
}

pub struct NoopAcpProviderLaunchGate;

#[async_trait]
impl AcpProviderLaunchGate for NoopAcpProviderLaunchGate {
    async fn before_provider_launch(
        &self,
        _req: &ExecutionRequest,
        _runtime_tool_path_preflight_json: Option<&str>,
    ) -> Result<()> {
        Ok(())
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

    for dir in default_provider_path_dirs(std::env::var_os("PATH"), dirs::home_dir()) {
        let candidate = dir.join(binary_path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

const DEFAULT_PROVIDER_BIN_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/local/sbin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

fn default_provider_path_value() -> String {
    std::env::join_paths(default_provider_path_dirs(
        std::env::var_os("PATH"),
        dirs::home_dir(),
    ))
    .unwrap_or_else(|_| OsString::from("/usr/bin:/bin:/usr/sbin:/sbin"))
    .to_string_lossy()
    .into_owned()
}

fn default_provider_path_dirs(
    path_var: Option<OsString>,
    home_dir: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(path_var) = path_var {
        dirs.extend(std::env::split_paths(&path_var));
    }
    if let Some(home_dir) = home_dir {
        dirs.push(home_dir.join(".local/bin"));
        dirs.push(home_dir.join(".cargo/bin"));
    }
    dirs.extend(DEFAULT_PROVIDER_BIN_DIRS.iter().map(PathBuf::from));

    let mut unique = Vec::new();
    for dir in dirs {
        if dir.as_os_str().is_empty() || unique.iter().any(|seen| seen == &dir) {
            continue;
        }
        unique.push(dir);
    }
    unique
}

fn ensure_default_path_env(env: &mut Vec<(String, String)>) {
    if env.iter().any(|(name, _)| name == "PATH") {
        return;
    }
    env.push(("PATH".to_string(), default_provider_path_value()));
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
    pub required_config_options: Vec<(String, String)>,
    pub set_mode_after_session_new: bool,
    pub permission_grant_debounce: Duration,
}

impl AcpSessionNewSpec {
    pub fn new(model: impl Into<String>, mode: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            mode: mode.into(),
            extra: None,
            config_options: Vec::new(),
            required_config_options: Vec::new(),
            set_mode_after_session_new: false,
            permission_grant_debounce: Duration::ZERO,
        }
    }

    pub fn from_config(config: AcpSessionConfig<'_>) -> Self {
        Self {
            model: config.model.to_string(),
            mode: config.mode.to_string(),
            extra: config.extra,
            config_options: config.config_options,
            required_config_options: config.required_config_options,
            set_mode_after_session_new: config.set_mode_after_session_new,
            permission_grant_debounce: config.permission_grant_debounce,
        }
    }

    pub fn as_config(&self) -> AcpSessionConfig<'_> {
        AcpSessionConfig {
            model: &self.model,
            mode: &self.mode,
            extra: self.extra.clone(),
            config_options: self.config_options.clone(),
            required_config_options: self.required_config_options.clone(),
            set_mode_after_session_new: self.set_mode_after_session_new,
            permission_grant_debounce: self.permission_grant_debounce,
        }
    }
}

/// Rolls back launch-time filesystem resources unless they are committed into
/// a live session cleanup plan.
#[derive(Debug, Default)]
pub struct LaunchResourceGuard {
    cleanup_paths: Vec<CleanupPathSpec>,
    committed: bool,
}

impl LaunchResourceGuard {
    pub fn add_cleanup_path(&mut self, path: impl Into<PathBuf>) {
        self.cleanup_paths.push(CleanupPathSpec::delete(path));
    }

    pub fn add_cleanup_spec(&mut self, spec: CleanupPathSpec) {
        self.cleanup_paths.push(spec);
    }

    pub fn commit(mut self) -> Vec<CleanupPathSpec> {
        self.committed = true;
        std::mem::take(&mut self.cleanup_paths)
    }
}

impl Drop for LaunchResourceGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for spec in self.cleanup_paths.drain(..) {
            cleanup_path(&spec.path);
        }
    }
}

fn cleanup_path(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

/// Opened session plus launch-time diagnostics that need to be copied onto the
/// prompt result for durable engine readback.
pub struct OpenedAcpAdapterSession {
    pub session: AcpSessionHandle,
    pub runtime_tool_path_preflight_json: Option<String>,
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

    /// Whether this adapter can start a fresh ACP subprocess attached to a
    /// recorded provider-native session id and prove the requested session
    /// before a prompt is sent. Default is fail-closed.
    fn supports_provider_session_resurrection(&self) -> bool {
        false
    }

    fn capability_probe_timeout(&self) -> Duration {
        crate::transport::handshake_timeout_for_provider(self.provider_name())
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
        crate::transport::isolate_process_group(&mut command);

        let child = command.spawn().with_context(|| {
            format!(
                "spawn {} ACP capability probe subprocess: {}",
                self.provider_name(),
                launch_spec.binary_path
            )
        })?;
        let initialize_result =
            crate::transport::probe_initialize_with_timeout(child, self.capability_probe_timeout())
                .await?;
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

    /// Provider-specific no-launch checks that must pass before the ACP
    /// subprocess is spawned.
    fn preflight_launch(
        &self,
        _req: &ExecutionRequest,
        _launch_spec: &mut AcpLaunchSpec,
    ) -> Result<()> {
        Ok(())
    }

    /// Open a session from precomputed launch and session specs.
    async fn open_session_with_specs(
        &self,
        req: &ExecutionRequest,
        mut launch_spec: AcpLaunchSpec,
        session_new_spec: AcpSessionNewSpec,
    ) -> Result<OpenedAcpAdapterSession> {
        let effective_req;
        let req = if req.provider_runtime_home.is_none() {
            if let Some(runtime_home) = launch_spec.provider_runtime_home.as_ref() {
                effective_req = Some({
                    let mut req = req.clone();
                    req.provider_runtime_home = Some(runtime_home.to_string_lossy().into_owned());
                    req
                });
                effective_req.as_ref().expect("effective request just set")
            } else {
                req
            }
        } else {
            req
        };
        let mut command = Command::new(&launch_spec.binary_path);
        launch_spec.apply_chainworks_meta_root_env(req);
        launch_spec.apply_chainworks_rust_cache_env();
        ensure_chainworks_meta_root_launch_dir(req)?;
        self.preflight_launch(req, &mut launch_spec)?;
        if let Some(gate) = launch_spec.provider_launch_gate.as_ref() {
            gate.before_provider_launch(
                req,
                launch_spec.runtime_tool_path_preflight_json.as_deref(),
            )
            .await?;
        }
        let execution_root = launch_spec
            .current_dir_override
            .clone()
            .unwrap_or_else(|| provider_execution_root(req));
        ensure_provider_execution_root(&execution_root)?;
        launch_spec.verify_capability_fingerprint()?;
        command
            .args(&launch_spec.args)
            .envs(launch_spec.env.drain(..))
            .current_dir(&execution_root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        crate::transport::isolate_process_group(&mut command);

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
                for spec in launch_spec.cleanup_paths.drain(..) {
                    cleanup_path(&spec.path);
                }
                return Err(err).with_context(|| spawn_context);
            }
        };
        let xcode_shim_grant = match launch_spec.register_xcode_shim_grant_for_child(&child) {
            Ok(grant) => grant,
            Err(err) => {
                for spec in launch_spec.cleanup_paths.drain(..) {
                    cleanup_path(&spec.path);
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
        Ok(OpenedAcpAdapterSession {
            session: AcpSessionHandle::new(session),
            runtime_tool_path_preflight_json: launch_spec.runtime_tool_path_preflight_json,
        })
    }

    /// Open a live transport-backed ACP session.
    async fn open_session(&self, req: &ExecutionRequest) -> Result<OpenedAcpAdapterSession> {
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
    ) -> Result<OpenedAcpAdapterSession> {
        let mut resources = LaunchResourceGuard::default();
        let mut launch_spec = self.prepare_launch_spec(req, &mut resources)?;
        launch_spec.apply_chainworks_meta_root_env(req);
        launch_spec.apply_chainworks_rust_cache_env();
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
        let opened = self.open_session(&req).await?;
        let session = opened.session;
        let mut result = match session.prompt(&req).await {
            Ok(result) => result,
            Err(err) => {
                let _ = session
                    .close_with_behavior(AcpSessionCloseBehavior::ArchiveFailure(
                        archive_context_for_failed_request(&req, "acp_prompt_error"),
                    ))
                    .await;
                return Err(err);
            }
        };
        let capture_context = archive_context_for_failed_request(&req, "pending_settlement");
        let staged_capture = session
            .stage_provider_session_store_for_outcome(&capture_context)
            .await?;
        let close_outcome = session
            .close_with_behavior(AcpSessionCloseBehavior::Delete)
            .await?;
        result.close_diagnostic = close_outcome.diagnostic;
        result.provider_session_store_capture =
            staged_capture.or(close_outcome.provider_session_store_capture);
        result.session_generation_id = None;
        result.runtime_tool_path_preflight_json =
            mark_runtime_tool_path_preflight_provider_launched(
                opened.runtime_tool_path_preflight_json,
            );
        Ok(result)
    }
}

pub(crate) fn mark_runtime_tool_path_preflight_provider_launched(
    json: Option<String>,
) -> Option<String> {
    json.map(|json| {
        let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&json) else {
            return json;
        };
        if let Some(object) = value.as_object_mut() {
            object.insert("provider_launched".to_string(), serde_json::json!(true));
        }
        serde_json::to_string(&value).unwrap_or(json)
    })
}

fn archive_context_for_failed_request(
    req: &ExecutionRequest,
    failure_kind: &str,
) -> ProviderSessionStoreArchiveContext {
    ProviderSessionStoreArchiveContext {
        provider: req.provider.clone(),
        run_id: req.run_id.to_string(),
        stage_id: req.stage_id.clone(),
        agent_id: req.agent_id.clone(),
        agent_execution_id: req.agent_execution_id.as_ref().map(ToString::to_string),
        session_generation_id: req.session_generation_id.clone(),
        provider_session_id: req.provider_session_id.clone(),
        failure_kind: failure_kind.to_string(),
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

fn ensure_chainworks_meta_root_launch_dir(req: &ExecutionRequest) -> Result<()> {
    let Some(meta_root) = chainworks_meta_root_env_value(req) else {
        return Ok(());
    };
    let meta_root = Path::new(&meta_root);
    for child in ["", "artifacts", "context", "state", "summaries"] {
        let path = if child.is_empty() {
            meta_root.to_path_buf()
        } else {
            meta_root.join(child)
        };
        std::fs::create_dir_all(&path).with_context(|| {
            format!(
                "missing_chainworks_meta_root: create launch directory {}",
                path.display()
            )
        })?;
    }
    Ok(())
}

fn provider_execution_root(req: &ExecutionRequest) -> PathBuf {
    if req.worktree_write_enabled {
        req.worktree_root
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&req.workspace_root))
    } else {
        PathBuf::from(&req.workspace_root)
    }
}

fn ensure_provider_execution_root(path: &Path) -> Result<()> {
    if path.is_dir() {
        Ok(())
    } else {
        bail!(
            "missing_provider_execution_root: ACP provider cwd {} does not exist or is not a directory",
            path.display()
        )
    }
}

fn has_env(env: &[(String, String)], key: &str) -> bool {
    env.iter().any(|(name, _)| name == key)
}

fn upsert_env(env: &mut Vec<(String, String)>, key: &str, value: String) {
    if let Some((_, existing)) = env.iter_mut().find(|(name, _)| name == key) {
        *existing = value;
    } else {
        env.push((key.to_string(), value));
    }
}

pub(crate) fn chainworks_shared_cargo_target_dir() -> String {
    env_nonempty("CHAINWORKS_AGENT_CARGO_TARGET_DIR")
        .or_else(|| env_nonempty("CHAINWORKS_SHARED_CARGO_TARGET_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| chainworks_cache_root().join("cargo-target").join("agents"))
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn chainworks_sccache_dir() -> String {
    env_nonempty("SCCACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| chainworks_cache_root().join("sccache"))
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn find_sccache_binary() -> Option<String> {
    if matches!(
        env_nonempty("CHAINWORKS_CARGO_SCCACHE")
            .unwrap_or_else(|| "auto".to_string())
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off" | "no"
    ) {
        return None;
    }
    if let Some(explicit) = env_nonempty("CHAINWORKS_SCCACHE_BINARY") {
        return Path::new(&explicit).is_file().then_some(explicit);
    }
    if let Some(existing) = env_nonempty("RUSTC_WRAPPER") {
        return Some(existing);
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .chain([
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".cargo")
                .join("bin"),
        ])
        .map(|dir| dir.join("sccache"))
        .find(|candidate| candidate.is_file())
        .map(|candidate| candidate.to_string_lossy().into_owned())
}

fn chainworks_cache_root() -> PathBuf {
    dirs::home_dir()
        .map(|home| home.join("Library").join("Caches").join("Chainworks Forge"))
        .unwrap_or_else(|| std::env::temp_dir().join("chainworks-forge-cache"))
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        default_provider_path_dirs, ensure_chainworks_meta_root_launch_dir,
        provider_execution_root, AcpAdapter, AcpLaunchSpec, ProbeKey, ProviderCapabilities,
        ProviderCapabilityCache, XcodeShimLaunchRuntime,
    };
    use crate::{XcodeShimGrantRecord, XcodeShimGrantStore};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct ProbeTimeoutAdapter(&'static str);

    #[async_trait::async_trait]
    impl AcpAdapter for ProbeTimeoutAdapter {
        fn provider_name(&self) -> &str {
            self.0
        }
    }

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
    fn capability_probe_timeout_uses_provider_specific_handshake_budget() {
        assert_eq!(
            ProbeTimeoutAdapter("gemini").capability_probe_timeout(),
            Duration::from_secs(120)
        );
        assert_eq!(
            ProbeTimeoutAdapter("claude").capability_probe_timeout(),
            Duration::from_secs(90)
        );
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
    fn launch_spec_default_path_keeps_xcode_shim_attachment_fingerprint_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = tmp.path().join("provider-acp");
        std::fs::write(&binary, "fixture").unwrap();

        let mut spec = AcpLaunchSpec::new(binary.to_string_lossy());
        spec.record_capability_fingerprint(Some("profile-a"), None);

        spec.env.push((
            "CHAINWORKS_XCODE_SHIM_DIR".to_string(),
            "/tmp/cw-shim".to_string(),
        ));
        super::prepend_path_env(&mut spec.env, "/tmp/cw-shim");

        spec.verify_capability_fingerprint()
            .expect("shim-only PATH prefix must not invalidate capability fingerprint");
    }

    #[test]
    fn provider_path_adds_interactive_tool_dirs_to_launchd_path() {
        let dirs = default_provider_path_dirs(
            Some(std::ffi::OsString::from("/usr/bin:/bin:/usr/bin")),
            Some(std::path::PathBuf::from("/Users/operator")),
        );

        assert_eq!(dirs[0], std::path::PathBuf::from("/usr/bin"));
        assert_eq!(dirs[1], std::path::PathBuf::from("/bin"));
        assert!(dirs.contains(&std::path::PathBuf::from("/opt/homebrew/bin")));
        assert!(dirs.contains(&std::path::PathBuf::from("/usr/local/bin")));
        assert!(dirs.contains(&std::path::PathBuf::from("/Users/operator/.local/bin")));
        assert!(dirs.contains(&std::path::PathBuf::from("/Users/operator/.cargo/bin")));
        assert_eq!(
            dirs.iter()
                .filter(|dir| dir.as_path() == std::path::Path::new("/usr/bin"))
                .count(),
            1
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
            provider_runtime_home: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: Some(".chainworks/run-meta".to_string()),
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

    #[test]
    fn rust_cache_env_is_frozen_before_capability_fingerprint() {
        let tmp = tempfile::tempdir().unwrap();
        let target_dir = tmp.path().join("agent-target");
        let fake_sccache = tmp.path().join("sccache");
        std::fs::write(&fake_sccache, "fixture").unwrap();

        let previous_target = std::env::var_os("CHAINWORKS_SHARED_CARGO_TARGET_DIR");
        let previous_sccache = std::env::var_os("CHAINWORKS_SCCACHE_BINARY");
        std::env::set_var("CHAINWORKS_SHARED_CARGO_TARGET_DIR", &target_dir);
        std::env::set_var("CHAINWORKS_SCCACHE_BINARY", &fake_sccache);

        let mut spec = AcpLaunchSpec::new("provider-acp");
        spec.apply_chainworks_rust_cache_env();
        let expected = spec.record_capability_fingerprint(Some("profile-a"), None);

        match previous_target {
            Some(value) => std::env::set_var("CHAINWORKS_SHARED_CARGO_TARGET_DIR", value),
            None => std::env::remove_var("CHAINWORKS_SHARED_CARGO_TARGET_DIR"),
        }
        match previous_sccache {
            Some(value) => std::env::set_var("CHAINWORKS_SCCACHE_BINARY", value),
            None => std::env::remove_var("CHAINWORKS_SCCACHE_BINARY"),
        }

        let target_value = target_dir.to_string_lossy().into_owned();
        let sccache_value = fake_sccache.to_string_lossy().into_owned();
        assert!(spec
            .env
            .iter()
            .any(|(name, value)| name == "CARGO_TARGET_DIR" && value == &target_value));
        assert!(spec
            .env
            .iter()
            .any(|(name, value)| name == "RUSTC_WRAPPER" && value == &sccache_value));
        assert_eq!(
            expected,
            spec.capability_fingerprint(Some("profile-a"), None)
        );
        spec.verify_capability_fingerprint()
            .expect("cache env must be part of the recorded fingerprint");
    }

    #[test]
    fn launch_preflight_creates_missing_chainworks_meta_root_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let req = crate::ExecutionRequest {
            agent_execution_id: None,
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_meta".to_string(),
            attempt_number: 1,
            agent_id: "agent_meta".to_string(),
            provider: "gemini".to_string(),
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
            provider_runtime_home: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: Some(".chainworks/run-meta".to_string()),
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
        let meta_root = tmp.path().join(".chainworks/run-meta");

        assert!(!meta_root.exists());
        ensure_chainworks_meta_root_launch_dir(&req).unwrap();

        for child in ["", "artifacts", "context", "state", "summaries"] {
            let path = if child.is_empty() {
                meta_root.clone()
            } else {
                meta_root.join(child)
            };
            assert!(
                path.is_dir(),
                "ACP launch preflight must create {} before provider initialize",
                path.display()
            );
        }
    }

    #[test]
    fn provider_execution_root_prefers_write_enabled_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        let worktree = tmp.path().join("worktree");
        let mut req = crate::ExecutionRequest {
            agent_execution_id: None,
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_cwd".to_string(),
            attempt_number: 1,
            agent_id: "agent_cwd".to_string(),
            provider: "junie".to_string(),
            model: None,
            effort: None,
            workspace_root: workspace.to_string_lossy().into_owned(),
            prompt: "prompt".to_string(),
            worktree_root: Some(worktree.to_string_lossy().into_owned()),
            worktree_write_enabled: true,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            provider_runtime_home: None,
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

        assert_eq!(provider_execution_root(&req), worktree);

        req.worktree_write_enabled = false;
        assert_eq!(provider_execution_root(&req), workspace);
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
