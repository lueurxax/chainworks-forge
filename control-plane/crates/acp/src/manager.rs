use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{bail, Result};
use domain::agent::AgentStatus;
use domain::provider::ProviderFamily;
use domain::xcode_runtime::{
    McpBrokerObservation, XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate, XcodeShimEvent,
};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::adapters::auggie::AuggieAdapter;
use crate::adapters::claude::ClaudeAgentAdapter;
use crate::adapters::codex::CodexAdapter;
use crate::adapters::gemini::GeminiCliAdapter;
use crate::adapters::junie::JunieAdapter;
use crate::adapters::{
    AcpAdapter, LaunchResourceGuard, ProviderCapabilityCache, XcodeShimLaunchRuntime,
};
use crate::session::AcpSessionHandle;
use crate::{
    AcpPromptProgressSink, ExecutionRequest, ExecutionResult, NoopAcpPromptProgressSink,
    NoopXcodeRuntimeObservationSink, XcodeRuntimeObservationSink, XcodeShimGrantStore,
};

#[derive(Debug)]
pub struct BrokeredXcodeLeaseAttachment {
    pub request: ExecutionRequest,
    pub lease_ids: Vec<String>,
}

impl BrokeredXcodeLeaseAttachment {
    pub fn new(request: ExecutionRequest) -> Self {
        Self {
            request,
            lease_ids: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
pub trait XcodeBrokerLeaseAttacher: Send + Sync {
    async fn attach_brokered_xcode_leases(
        &self,
        req: &ExecutionRequest,
    ) -> Result<BrokeredXcodeLeaseAttachment>;

    async fn warm_up_brokered_xcode_leases(&self, _lease_ids: &[String]) -> Result<()> {
        Ok(())
    }

    async fn release_brokered_xcode_leases(&self, _lease_ids: &[String]) -> Result<()> {
        Ok(())
    }
}

pub struct NoopXcodeBrokerLeaseAttacher;

#[async_trait::async_trait]
impl XcodeBrokerLeaseAttacher for NoopXcodeBrokerLeaseAttacher {
    async fn attach_brokered_xcode_leases(
        &self,
        req: &ExecutionRequest,
    ) -> Result<BrokeredXcodeLeaseAttachment> {
        if let Some(intent) = req.brokered_xcode_intents().into_iter().next() {
            bail!(
                "ACP: brokered Xcode MCP intent '{}' must be converted to an HTTP lease before session/new",
                intent.runtime_id
            );
        }
        Ok(BrokeredXcodeLeaseAttachment::new(req.clone()))
    }
}

#[derive(Clone)]
struct BrokeredXcodeLeaseCleanup {
    attacher: Arc<dyn XcodeBrokerLeaseAttacher>,
    lease_ids: Vec<String>,
}

/// Manages ACP provider adapters and owns any live reusable sessions.
pub struct AcpRuntimeManager {
    adapters: HashMap<String, Arc<dyn AcpAdapter>>,
    live_sessions: Mutex<HashMap<String, AcpSessionHandle>>,
    live_xcode_leases: Mutex<HashMap<String, BrokeredXcodeLeaseCleanup>>,
    provider_capability_cache: ProviderCapabilityCache,
    prompt_progress_sink: RwLock<Arc<dyn AcpPromptProgressSink>>,
    xcode_runtime_observation_sink: RwLock<Arc<dyn XcodeRuntimeObservationSink>>,
    xcode_broker_lease_attacher: RwLock<Arc<dyn XcodeBrokerLeaseAttacher>>,
    xcode_shim_runtime: RwLock<Option<XcodeShimRuntimeConfig>>,
}

#[derive(Clone)]
struct XcodeShimRuntimeConfig {
    store: Arc<dyn XcodeShimGrantStore>,
    socket_path: String,
    shim_dir: String,
}

fn canonical_acp_provider(provider: &str) -> String {
    ProviderFamily::canonicalize_known_alias(provider).unwrap_or_else(|| provider.to_string())
}

impl AcpRuntimeManager {
    /// Create a new manager with all ACP adapters pre-registered.
    /// Each adapter reads its binary path from an env var at construction time;
    /// execution fails fast if the env var is unset when `execute` is called.
    pub fn new() -> Self {
        let mut adapters: HashMap<String, Arc<dyn AcpAdapter>> = HashMap::new();

        let claude = Arc::new(ClaudeAgentAdapter::new()) as Arc<dyn AcpAdapter>;
        let codex = Arc::new(CodexAdapter::new()) as Arc<dyn AcpAdapter>;
        let gemini = Arc::new(GeminiCliAdapter::new()) as Arc<dyn AcpAdapter>;
        let auggie = Arc::new(AuggieAdapter::new()) as Arc<dyn AcpAdapter>;
        let junie = Arc::new(JunieAdapter::new()) as Arc<dyn AcpAdapter>;

        adapters.insert(claude.provider_name().to_string(), claude);
        adapters.insert(codex.provider_name().to_string(), codex);
        adapters.insert(gemini.provider_name().to_string(), gemini);
        adapters.insert(auggie.provider_name().to_string(), auggie);
        adapters.insert(junie.provider_name().to_string(), junie);

        info!(
            registered_providers = ?adapters.keys().collect::<Vec<_>>(),
            "AcpRuntimeManager initialised"
        );

        Self {
            adapters,
            live_sessions: Mutex::new(HashMap::new()),
            live_xcode_leases: Mutex::new(HashMap::new()),
            provider_capability_cache: ProviderCapabilityCache::default(),
            prompt_progress_sink: RwLock::new(Arc::new(NoopAcpPromptProgressSink)),
            xcode_runtime_observation_sink: RwLock::new(Arc::new(NoopXcodeRuntimeObservationSink)),
            xcode_broker_lease_attacher: RwLock::new(Arc::new(NoopXcodeBrokerLeaseAttacher)),
            xcode_shim_runtime: RwLock::new(None),
        }
    }

    /// Retrieve a shared reference to the adapter for the given provider name.
    pub fn get_adapter(&self, provider: &str) -> Option<Arc<dyn AcpAdapter>> {
        let provider = canonical_acp_provider(provider);
        self.adapters.get(&provider).cloned()
    }

    pub fn set_xcode_runtime_observation_sink(&self, sink: Arc<dyn XcodeRuntimeObservationSink>) {
        let mut guard = self
            .xcode_runtime_observation_sink
            .write()
            .expect("xcode runtime observation sink lock poisoned");
        *guard = sink;
    }

    pub fn set_prompt_progress_sink(&self, sink: Arc<dyn AcpPromptProgressSink>) {
        let mut guard = self
            .prompt_progress_sink
            .write()
            .expect("prompt progress sink lock poisoned");
        *guard = sink;
    }

    fn prompt_progress_sink(&self) -> Arc<dyn AcpPromptProgressSink> {
        self.prompt_progress_sink
            .read()
            .expect("prompt progress sink lock poisoned")
            .clone()
    }

    pub fn xcode_runtime_observation_sink(&self) -> Arc<dyn XcodeRuntimeObservationSink> {
        self.xcode_runtime_observation_sink
            .read()
            .expect("xcode runtime observation sink lock poisoned")
            .clone()
    }

    pub fn set_xcode_broker_lease_attacher(&self, attacher: Arc<dyn XcodeBrokerLeaseAttacher>) {
        let mut guard = self
            .xcode_broker_lease_attacher
            .write()
            .expect("xcode broker lease attacher lock poisoned");
        *guard = attacher;
    }

    pub fn set_xcode_shim_runtime(
        &self,
        store: Arc<dyn XcodeShimGrantStore>,
        socket_path: impl Into<String>,
        shim_dir: impl Into<String>,
    ) {
        let mut guard = self
            .xcode_shim_runtime
            .write()
            .expect("xcode shim runtime lock poisoned");
        *guard = Some(XcodeShimRuntimeConfig {
            store,
            socket_path: socket_path.into(),
            shim_dir: shim_dir.into(),
        });
    }

    async fn adapter_for(&self, provider: &str) -> Result<Arc<dyn AcpAdapter>> {
        let provider = canonical_acp_provider(provider);
        self.adapters
            .get(&provider)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No adapter registered for provider '{provider}'"))
    }

    async fn live_session(&self, generation_id: &str) -> Result<AcpSessionHandle> {
        let session = { self.live_sessions.lock().await.get(generation_id).cloned() };
        let Some(session) = session else {
            bail!("No live ACP session registered for generation id '{generation_id}'");
        };
        if session.is_live().await {
            return Ok(session);
        }
        self.live_sessions.lock().await.remove(generation_id);
        let cleanup = self.live_xcode_leases.lock().await.remove(generation_id);
        self.release_xcode_leases(cleanup).await;
        bail!("No live ACP session registered for generation id '{generation_id}'")
    }

    pub async fn has_live_session(
        &self,
        generation_id: &str,
        provider_session_id: Option<&str>,
    ) -> bool {
        let session = {
            let sessions = self.live_sessions.lock().await;
            sessions.get(generation_id).cloned()
        };
        let Some(session) = session else {
            return false;
        };
        if !session.is_live().await {
            self.live_sessions.lock().await.remove(generation_id);
            let cleanup = self.live_xcode_leases.lock().await.remove(generation_id);
            self.release_xcode_leases(cleanup).await;
            return false;
        }
        match provider_session_id {
            Some(expected) => session.provider_session_id().await == expected,
            None => true,
        }
    }

    /// Start a fresh ACP session and keep it alive if requested.
    pub async fn start_session(&self, mut req: ExecutionRequest) -> Result<ExecutionResult> {
        req.provider = canonical_acp_provider(&req.provider);
        let provider = req.provider.clone();
        let adapter = self.adapter_for(&provider).await?;
        info!(
            provider = %provider,
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            "AcpRuntimeManager: starting session"
        );

        let opened = match self.open_ordered_session(adapter.clone(), &req).await {
            Ok(opened) => opened,
            Err(err)
                if req.brokered_xcode_intents().is_empty()
                    && err
                        .to_string()
                        .contains("does not provide process launch specs") =>
            {
                return adapter.execute(req).await;
            }
            Err(err) => return Err(err),
        };
        let session = opened.session;
        let session_req = opened.session_req;
        let lease_cleanup = opened.lease_cleanup;
        let registered_generation_id = req.session_generation_id.clone();
        if let Some(generation_id) = registered_generation_id.as_ref() {
            self.live_sessions
                .lock()
                .await
                .insert(generation_id.clone(), session.clone());
            if let Some(cleanup) = lease_cleanup.clone() {
                self.live_xcode_leases
                    .lock()
                    .await
                    .insert(generation_id.clone(), cleanup);
            }
        }

        let mut result = match session
            .prompt_with_progress_sink(&session_req, self.prompt_progress_sink())
            .await
        {
            Ok(result) => result,
            Err(err) => {
                let cleanup = match registered_generation_id.as_ref() {
                    Some(generation_id) => {
                        self.live_xcode_leases.lock().await.remove(generation_id)
                    }
                    None => lease_cleanup,
                };
                if let Some(generation_id) = registered_generation_id.as_ref() {
                    self.live_sessions.lock().await.remove(generation_id);
                }
                let _ = session.close().await;
                self.release_xcode_leases(cleanup).await;
                return Err(err);
            }
        };
        let keep_session_alive = req.keep_session_alive && result.status == AgentStatus::Completed;
        if keep_session_alive {
            let generation_id = req.session_generation_id.clone().ok_or_else(|| {
                anyhow::anyhow!("keep_session_alive requested without session_generation_id")
            })?;
            result.session_generation_id = Some(generation_id);
            result.reused_existing_session = false;
            self.record_xcode_prompt_observations(&session_req, &result)
                .await;
            return Ok(result);
        }

        let cleanup = match registered_generation_id.as_ref() {
            Some(generation_id) => self.live_xcode_leases.lock().await.remove(generation_id),
            None => lease_cleanup,
        };
        if let Some(generation_id) = registered_generation_id.as_ref() {
            self.live_sessions.lock().await.remove(generation_id);
        }
        let close_result = session.close().await;
        self.release_xcode_leases(cleanup).await;
        close_result?;
        result.session_generation_id = None;
        result.reused_existing_session = false;
        self.record_xcode_prompt_observations(&session_req, &result)
            .await;
        Ok(result)
    }

    async fn open_ordered_session(
        &self,
        adapter: Arc<dyn AcpAdapter>,
        req: &ExecutionRequest,
    ) -> Result<OpenedAcpSession> {
        let mut resources = LaunchResourceGuard::default();
        let mut launch_spec = adapter.prepare_launch_spec(req, &mut resources)?;
        launch_spec.apply_chainworks_meta_root_env(req);
        let runtime_profile_id = req
            .brokered_xcode_intents()
            .into_iter()
            .find_map(|intent| intent.runtime_profile_id.as_deref());
        launch_spec.record_capability_fingerprint(runtime_profile_id, None);

        adapter
            .ensure_brokered_xcode_http_capability(
                req,
                &launch_spec,
                &self.provider_capability_cache,
            )
            .await?;

        let attacher = self
            .xcode_broker_lease_attacher
            .read()
            .expect("xcode broker lease attacher lock poisoned")
            .clone();
        let attachment = attacher.attach_brokered_xcode_leases(req).await?;
        let lease_cleanup = (!attachment.lease_ids.is_empty()).then(|| BrokeredXcodeLeaseCleanup {
            attacher: attacher.clone(),
            lease_ids: attachment.lease_ids.clone(),
        });
        let session_req = attachment.request;
        self.attach_xcode_shim_runtime_if_needed(&session_req, &mut launch_spec)?;

        if let Err(err) = adapter.reject_unconverted_broker_intents(&session_req) {
            self.release_xcode_leases(lease_cleanup).await;
            return Err(err);
        }

        if let Err(err) = attacher
            .warm_up_brokered_xcode_leases(&attachment.lease_ids)
            .await
        {
            self.release_xcode_leases(lease_cleanup).await;
            return Err(err);
        }

        let session_new_spec = match adapter.prepare_session_new_spec(&session_req) {
            Ok(spec) => spec,
            Err(err) => {
                self.release_xcode_leases(lease_cleanup).await;
                return Err(err);
            }
        };
        // P066 T20: Prepare Go session-scoped toolchain mapping root if enabled.
        // The root is registered with the LaunchResourceGuard so it is removed on
        // session startup failure (before resources.commit()) AND on session close
        // (via AcpSession::close → cleanup_paths).
        if req.toolchain_go_scope_enabled {
            if let (Some(toolchain_home), Some(session_gen_id)) = (
                req.toolchain_home.as_deref(),
                req.session_generation_id.as_deref(),
            ) {
                match crate::toolchain_mapper::prepare_toolchain_mapping(
                    Path::new(toolchain_home),
                    crate::toolchain_mapper::ToolchainFamily::Go,
                    session_gen_id,
                    crate::toolchain_mapper::DEFAULT_MIN_FREE_BYTES,
                ) {
                    Ok(result) => {
                        // Register root for cleanup on session failure or close.
                        resources.add_cleanup_path(result.root.clone());
                        // Inject Go env vars into the process launch spec.
                        for (k, v) in result.env_vars {
                            launch_spec.env.push((k, v));
                        }
                    }
                    Err(err) => {
                        // Fail-closed: setup failure prevents session launch.
                        self.release_xcode_leases(lease_cleanup).await;
                        return Err(anyhow::anyhow!(
                            "toolchain_mapping_setup_failed for Go session scope: {}",
                            err.reason.as_str()
                        ));
                    }
                }
            }
        }
        launch_spec.cleanup_paths.extend(resources.commit());
        let session = match adapter
            .open_session_with_specs(&session_req, launch_spec, session_new_spec)
            .await
        {
            Ok(session) => session,
            Err(err) => {
                self.release_xcode_leases(lease_cleanup).await;
                return Err(err);
            }
        };
        Ok(OpenedAcpSession {
            session,
            session_req,
            lease_cleanup,
        })
    }

    fn attach_xcode_shim_runtime_if_needed(
        &self,
        req: &ExecutionRequest,
        launch_spec: &mut crate::adapters::AcpLaunchSpec,
    ) -> Result<()> {
        if env_flag_enabled("CHAINWORKS_XCODE_BROKER_DISABLED") {
            return Ok(());
        }
        if !req.xcode_shim_injection_signal && !req.requires_xcode_host_execution {
            return Ok(());
        }
        let config = self
            .xcode_shim_runtime
            .read()
            .expect("xcode shim runtime lock poisoned")
            .clone()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "p051_xcode_shim_runtime_unavailable: provider requested Xcode shim injection but daemon did not configure a shim socket"
                )
            })?;
        let token_id = uuid::Uuid::new_v4().to_string();
        let token_secret = uuid::Uuid::new_v4().to_string();
        launch_spec.attach_xcode_shim_runtime(XcodeShimLaunchRuntime {
            token_id: token_id.clone(),
            token_secret,
            lease_id: format!("xcode-shim-{token_id}"),
            socket_path: config.socket_path,
            shim_dir: config.shim_dir,
            workspace_root: req.workspace_root.clone(),
            agent_execution_id: req.agent_execution_id,
            store: config.store,
        });
        Ok(())
    }

    async fn release_xcode_leases(&self, cleanup: Option<BrokeredXcodeLeaseCleanup>) {
        let Some(cleanup) = cleanup else {
            return;
        };
        if let Err(err) = cleanup
            .attacher
            .release_brokered_xcode_leases(&cleanup.lease_ids)
            .await
        {
            warn!(
                error = %err,
                lease_count = cleanup.lease_ids.len(),
                "Failed to release brokered Xcode MCP leases"
            );
        }
    }

    /// Prompt an existing live ACP session by generation id.
    pub async fn prompt_session(
        &self,
        session_generation_id: &str,
        req: ExecutionRequest,
    ) -> Result<ExecutionResult> {
        let session = self.live_session(session_generation_id).await?;
        if let Some(expected_provider_session_id) = req.provider_session_id.as_deref() {
            let actual_provider_session_id = session.provider_session_id().await;
            if actual_provider_session_id != expected_provider_session_id {
                return Err(anyhow::anyhow!(
                    "Live ACP session provider_session_id mismatch for generation id '{}': expected '{}', got '{}'",
                    session_generation_id,
                    expected_provider_session_id,
                    actual_provider_session_id
                ));
            }
        }
        info!(
            provider = %req.provider,
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            session_generation_id = %session_generation_id,
            "AcpRuntimeManager: reusing live session"
        );

        let mut result = match session
            .prompt_with_progress_sink(&req, self.prompt_progress_sink())
            .await
        {
            Ok(result) => result,
            Err(err) => {
                self.live_sessions
                    .lock()
                    .await
                    .remove(session_generation_id);
                let cleanup = self
                    .live_xcode_leases
                    .lock()
                    .await
                    .remove(session_generation_id);
                let _ = session.close().await;
                self.release_xcode_leases(cleanup).await;
                return Err(err);
            }
        };
        result.session_generation_id = Some(session_generation_id.to_string());
        result.reused_existing_session = true;
        self.record_xcode_prompt_observations(&req, &result).await;
        Ok(result)
    }

    /// Close and remove a live ACP session.
    pub async fn close_session(&self, session_generation_id: &str) -> Result<()> {
        let session = self
            .live_sessions
            .lock()
            .await
            .remove(session_generation_id);
        let lease_cleanup = self
            .live_xcode_leases
            .lock()
            .await
            .remove(session_generation_id);
        let Some(session) = session else {
            self.release_xcode_leases(lease_cleanup).await;
            bail!("No live ACP session registered for generation id '{session_generation_id}'");
        };
        let close_result = session.close().await;
        self.release_xcode_leases(lease_cleanup).await;
        close_result
    }

    /// Close and remove all live ACP sessions.
    pub async fn close_all_sessions(&self) -> usize {
        let sessions = {
            let mut live_sessions = self.live_sessions.lock().await;
            std::mem::take(&mut *live_sessions)
        };
        let lease_cleanups = {
            let mut live_xcode_leases = self.live_xcode_leases.lock().await;
            std::mem::take(&mut *live_xcode_leases)
        };
        for cleanup in lease_cleanups.into_values() {
            self.release_xcode_leases(Some(cleanup)).await;
        }
        let mut closed = 0;
        for (generation_id, session) in sessions {
            match session.close().await {
                Ok(_) => {
                    closed += 1;
                }
                Err(error) => warn!(
                    session_generation_id = %generation_id,
                    "ACP live session close during shutdown failed: {error}"
                ),
            }
        }
        closed
    }

    /// Route an execution request to the matching adapter or live session.
    pub async fn execute(&self, mut req: ExecutionRequest) -> Result<ExecutionResult> {
        req.provider = canonical_acp_provider(&req.provider);
        let result = if req.reuse_existing_session {
            let session_generation_id = req.session_generation_id.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "reuse_existing_session was requested but no session_generation_id was provided"
                )
            })?;
            self.prompt_session(&session_generation_id, req.clone())
                .await
        } else {
            self.start_session(req.clone()).await
        };

        if let Err(error) = &result {
            self.record_xcode_broker_failure_observation(&req, error)
                .await;
        }

        result
    }

    async fn record_xcode_broker_failure_observation(
        &self,
        req: &ExecutionRequest,
        error: &anyhow::Error,
    ) {
        let Some(agent_execution_id) = req.agent_execution_id else {
            return;
        };
        let Some(intent) = req.brokered_xcode_intents().into_iter().next() else {
            return;
        };

        let update = XcodeRuntimeObservationUpdate::McpBrokerObservation(McpBrokerObservation {
            source: "xcode_mcp_broker".to_string(),
            backend_start_disposition: "failed_closed_before_session_new".to_string(),
            pool_id: None,
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
            backend_failure_class: Some(xcode_failure_class_from_error(error)),
            originating_execution_id: Some(agent_execution_id.to_string()),
            prompt_cycle_index: None,
            status_update: Some(format!(
                "Brokered Xcode MCP intent '{}' failed before session/new: {}",
                intent.runtime_id, error
            )),
        });

        let sink = self
            .xcode_runtime_observation_sink
            .read()
            .expect("xcode runtime observation sink lock poisoned")
            .clone();
        if let Err(sink_error) = sink
            .append_xcode_runtime_observation(agent_execution_id, update)
            .await
        {
            warn!(
                agent_execution_id = %agent_execution_id,
                error = %sink_error,
                "Failed to persist Xcode runtime observation"
            );
        }
    }

    async fn record_xcode_prompt_observations(
        &self,
        req: &ExecutionRequest,
        result: &ExecutionResult,
    ) {
        if result.xcode_shim_warning_events.is_empty() {
            return;
        }
        let Some(agent_execution_id) = req.agent_execution_id else {
            return;
        };
        let sink = self
            .xcode_runtime_observation_sink
            .read()
            .expect("xcode runtime observation sink lock poisoned")
            .clone();
        for warning in &result.xcode_shim_warning_events {
            let update = XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::Warning(
                warning.clone(),
            ));
            if let Err(sink_error) = sink
                .append_xcode_runtime_observation(agent_execution_id, update)
                .await
            {
                warn!(
                    agent_execution_id = %agent_execution_id,
                    error = %sink_error,
                    "Failed to persist Xcode residual path warning"
                );
            }
        }
    }

    /// Register an additional adapter (useful for testing or future dynamic registration).
    pub fn register(&mut self, adapter: Arc<dyn AcpAdapter>) {
        self.adapters
            .insert(adapter.provider_name().to_string(), adapter);
    }

    /// Create a manager pre-loaded with the given adapters.
    /// Useful for injecting fixture adapters in integration tests.
    pub fn new_with_adapters(adapters: Vec<Arc<dyn AcpAdapter>>) -> Self {
        let adapters: HashMap<String, Arc<dyn AcpAdapter>> = adapters
            .into_iter()
            .map(|a| (a.provider_name().to_string(), a))
            .collect();
        Self {
            adapters,
            live_sessions: Mutex::new(HashMap::new()),
            live_xcode_leases: Mutex::new(HashMap::new()),
            provider_capability_cache: ProviderCapabilityCache::default(),
            prompt_progress_sink: RwLock::new(Arc::new(NoopAcpPromptProgressSink)),
            xcode_runtime_observation_sink: RwLock::new(Arc::new(NoopXcodeRuntimeObservationSink)),
            xcode_broker_lease_attacher: RwLock::new(Arc::new(NoopXcodeBrokerLeaseAttacher)),
            xcode_shim_runtime: RwLock::new(None),
        }
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

struct OpenedAcpSession {
    session: AcpSessionHandle,
    session_req: ExecutionRequest,
    lease_cleanup: Option<BrokeredXcodeLeaseCleanup>,
}

fn xcode_failure_class_from_error(error: &anyhow::Error) -> XcodeRuntimeFailureClass {
    let message = error.to_string();
    if message.contains("provider_http_mcp_unsupported") {
        XcodeRuntimeFailureClass::ProviderHttpMcpUnsupported
    } else if message.contains("xcode_mcp_registry_stale_stdio") {
        XcodeRuntimeFailureClass::XcodeMcpRegistryStaleStdio
    } else if message.contains("xcode_mcp_registry_ambiguous") {
        XcodeRuntimeFailureClass::XcodeMcpRegistryAmbiguous
    } else if message.contains("xcode_mcp_capacity_exhausted") {
        XcodeRuntimeFailureClass::XcodeMcpCapacityExhausted
    } else if message.contains("xcode_mcp_initialize_timeout") {
        XcodeRuntimeFailureClass::XcodeMcpInitializeTimeout
    } else if message.contains("xcode_mcp_action_required") {
        XcodeRuntimeFailureClass::XcodeMcpActionRequired
    } else if message.contains("xcode_mcp_first_connect_timeout") {
        XcodeRuntimeFailureClass::XcodeMcpFirstConnectTimeout
    } else if message.contains("pool_pid_drift") {
        XcodeRuntimeFailureClass::PoolPidDrift
    } else if message.contains("xcode_target_not_found") {
        XcodeRuntimeFailureClass::XcodeTargetNotFound
    } else if message.contains("xcode_target_ambiguous") {
        XcodeRuntimeFailureClass::XcodeTargetAmbiguous
    } else if message.contains("host_env_unavailable") {
        XcodeRuntimeFailureClass::HostEnvUnavailable
    } else {
        XcodeRuntimeFailureClass::BrokerInfrastructure
    }
}

impl Default for AcpRuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::AgentExecutionId;
    use tokio::sync::Mutex as TokioMutex;

    struct FixtureObservationSink;

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for FixtureObservationSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            _update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn returns_configured_xcode_runtime_observation_sink() {
        let manager = AcpRuntimeManager::new_with_adapters(Vec::new());
        let sink: Arc<dyn XcodeRuntimeObservationSink> = Arc::new(FixtureObservationSink);

        manager.set_xcode_runtime_observation_sink(sink.clone());

        assert!(Arc::ptr_eq(
            &sink,
            &manager.xcode_runtime_observation_sink()
        ));
    }

    struct RecordingLeaseAttacher {
        released: Arc<TokioMutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl XcodeBrokerLeaseAttacher for RecordingLeaseAttacher {
        async fn attach_brokered_xcode_leases(
            &self,
            req: &ExecutionRequest,
        ) -> anyhow::Result<BrokeredXcodeLeaseAttachment> {
            Ok(BrokeredXcodeLeaseAttachment::new(req.clone()))
        }

        async fn release_brokered_xcode_leases(&self, lease_ids: &[String]) -> anyhow::Result<()> {
            self.released.lock().await.extend_from_slice(lease_ids);
            Ok(())
        }
    }

    #[tokio::test]
    async fn close_session_releases_orphaned_xcode_lease_cleanup_when_live_session_is_missing() {
        let manager = AcpRuntimeManager::new_with_adapters(Vec::new());
        let released = Arc::new(TokioMutex::new(Vec::new()));
        let attacher = Arc::new(RecordingLeaseAttacher {
            released: Arc::clone(&released),
        });
        manager.live_xcode_leases.lock().await.insert(
            "generation-orphaned".to_string(),
            BrokeredXcodeLeaseCleanup {
                attacher,
                lease_ids: vec!["lease-orphaned".to_string()],
            },
        );

        let result = manager.close_session("generation-orphaned").await;

        assert!(
            result
                .as_ref()
                .is_err_and(|error| error.to_string().contains("No live ACP session")),
            "close_session should still report the missing live session"
        );
        assert_eq!(released.lock().await.as_slice(), ["lease-orphaned"]);
        assert!(manager
            .live_xcode_leases
            .lock()
            .await
            .get("generation-orphaned")
            .is_none());
    }

    #[test]
    fn broker_disabled_env_suppresses_xcode_shim_injection() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let previous = std::env::var("CHAINWORKS_XCODE_BROKER_DISABLED").ok();
        std::env::set_var("CHAINWORKS_XCODE_BROKER_DISABLED", "1");

        let manager = AcpRuntimeManager::new_with_adapters(Vec::new());
        let mut launch_spec = crate::adapters::AcpLaunchSpec::new("/bin/sh");
        let req = ExecutionRequest {
            agent_execution_id: None,
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_xcode".to_string(),
            attempt_number: 1,
            agent_id: "agent_xcode".to_string(),
            provider: "claude".to_string(),
            model: None,
            effort: None,
            workspace_root: "/tmp/workspace".to_string(),
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
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: true,
            requires_xcode_host_execution: true,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,
        };

        manager
            .attach_xcode_shim_runtime_if_needed(&req, &mut launch_spec)
            .unwrap();

        assert!(launch_spec.xcode_shim_runtime.is_none());
        assert!(launch_spec
            .env
            .iter()
            .all(|(name, _)| !name.starts_with("CHAINWORKS_XCODE_SHIM_")));

        match previous {
            Some(value) => std::env::set_var("CHAINWORKS_XCODE_BROKER_DISABLED", value),
            None => std::env::remove_var("CHAINWORKS_XCODE_BROKER_DISABLED"),
        }
    }

    #[test]
    fn provider_aliases_resolve_to_registered_acp_adapters() {
        let manager = AcpRuntimeManager::new();

        assert!(manager.get_adapter("claude_acp").is_some());
        assert!(manager.get_adapter("gemini_acp").is_some());
        assert!(manager.get_adapter("codex_acp").is_some());
    }
}
