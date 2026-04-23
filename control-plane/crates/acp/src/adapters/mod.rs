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
use crate::{ExecutionRequest, ExecutionResult};

/// Process launch details prepared independently from `session/new` params.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpLaunchSpec {
    pub binary_path: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cleanup_paths: Vec<PathBuf>,
    expected_capability_fingerprint: Option<CapabilitySliceFingerprint>,
}

impl AcpLaunchSpec {
    pub fn new(binary_path: impl Into<String>) -> Self {
        Self {
            binary_path: binary_path.into(),
            args: Vec::new(),
            env: Vec::new(),
            cleanup_paths: Vec::new(),
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
        let mut env = launch_spec.env.clone();
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
        launch_spec.verify_capability_fingerprint()?;
        if let Some(meta_root) = chainworks_meta_root_env_value(req) {
            launch_spec
                .env
                .retain(|(name, _)| name != "CHAINWORKS_META_ROOT");
            launch_spec
                .env
                .push(("CHAINWORKS_META_ROOT".to_string(), meta_root));
        }
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

        let config = session_new_spec.as_config();
        let session = crate::session::AcpSession::start_with_cleanup_paths(
            child,
            req,
            &config,
            launch_spec.cleanup_paths,
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
    use super::{AcpLaunchSpec, ProbeKey, ProviderCapabilities, ProviderCapabilityCache};

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
}
