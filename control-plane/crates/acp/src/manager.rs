use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{bail, Result};
use domain::agent::AgentStatus;
use domain::provider::ProviderFamily;
use domain::xcode_runtime::{
    McpBrokerObservation, XcodeRuntimeFailureClass, XcodeRuntimeObservationUpdate, XcodeShimEvent,
    XcodeShimRuntimeAttachedEvent,
};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::adapters::auggie::AuggieAdapter;
use crate::adapters::claude::ClaudeAgentAdapter;
use crate::adapters::codex::CodexAdapter;
use crate::adapters::gemini::GeminiCliAdapter;
use crate::adapters::junie::JunieAdapter;
use crate::adapters::{
    AcpAdapter, AcpLaunchObserver, AcpProviderLaunchGate, LaunchResourceGuard,
    NoopAcpProviderLaunchGate, ProviderCapabilityCache, ProviderSessionResurrectionCapability,
    XcodeShimLaunchRuntime,
};
use crate::session::{
    AcpSessionCloseBehavior, AcpSessionHandle, ProviderSessionStoreArchiveContext,
};
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
    provider_launch_gate: RwLock<Arc<dyn AcpProviderLaunchGate>>,
    xcode_runtime_observation_sink: RwLock<Arc<dyn XcodeRuntimeObservationSink>>,
    xcode_broker_lease_attacher: RwLock<Arc<dyn XcodeBrokerLeaseAttacher>>,
    xcode_shim_runtime: RwLock<Option<XcodeShimRuntimeConfig>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpLiveSessionProcessBinding {
    pub child_pid: u32,
    pub process_group_id: u32,
}

#[derive(Clone, Debug)]
pub struct ProviderSessionResurrectionAttachResult {
    pub provider: String,
    pub adapter_id: String,
    pub adapter_capability_version: String,
    pub requested_provider_session_id: String,
    pub actual_provider_session_id: String,
    pub identity_proof_source: String,
    pub identity_proof_observed_at: String,
    pub session_generation_id: String,
    pub managed_child_pid: Option<u32>,
    pub managed_process_group_id: Option<u32>,
}

fn provider_session_id_ref(provider_session_id: &str) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(provider_session_id.as_bytes())
    )
}

fn provider_session_identity_mismatch_message(
    context: &str,
    expected: &str,
    actual: &str,
) -> String {
    format!(
        "{context}: expected_ref '{}', actual_ref '{}'",
        provider_session_id_ref(expected),
        provider_session_id_ref(actual)
    )
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
            provider_launch_gate: RwLock::new(Arc::new(NoopAcpProviderLaunchGate)),
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

    pub fn set_provider_launch_gate(&self, gate: Arc<dyn AcpProviderLaunchGate>) {
        let mut guard = self
            .provider_launch_gate
            .write()
            .expect("provider launch gate lock poisoned");
        *guard = gate;
    }

    fn prompt_progress_sink(&self) -> Arc<dyn AcpPromptProgressSink> {
        self.prompt_progress_sink
            .read()
            .expect("prompt progress sink lock poisoned")
            .clone()
    }

    fn provider_launch_gate(&self) -> Arc<dyn AcpProviderLaunchGate> {
        self.provider_launch_gate
            .read()
            .expect("provider launch gate lock poisoned")
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

    pub async fn live_session_process_binding(
        &self,
        generation_id: &str,
    ) -> Option<AcpLiveSessionProcessBinding> {
        let session = {
            let sessions = self.live_sessions.lock().await;
            sessions.get(generation_id).cloned()
        }?;
        if !session.is_live().await {
            self.live_sessions.lock().await.remove(generation_id);
            let cleanup = self.live_xcode_leases.lock().await.remove(generation_id);
            self.release_xcode_leases(cleanup).await;
            return None;
        }
        let child_pid = session.child_pid().await?;
        Some(AcpLiveSessionProcessBinding {
            child_pid,
            // ACP adapters launch children into a fresh process group with the
            // child pid as pgid, so recording both values makes recovery
            // explicit without probing arbitrary processes by command line.
            process_group_id: child_pid,
        })
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
        let runtime_tool_path_preflight_json = opened.runtime_tool_path_preflight_json.clone();
        let session = opened.session;
        let session_req = opened.session_req;
        let mut lease_cleanup = opened.lease_cleanup;
        if let Some(expected_provider_session_id) = req.provider_session_id.as_deref() {
            let actual_provider_session_id = session.provider_session_id().await;
            if actual_provider_session_id != expected_provider_session_id {
                let _ = session
                    .close_with_behavior(AcpSessionCloseBehavior::Delete)
                    .await;
                self.release_xcode_leases(lease_cleanup.take()).await;
                bail!(
                    "{}",
                    provider_session_identity_mismatch_message(
                        "ACP provider_session_id mismatch before prompt",
                        expected_provider_session_id,
                        &actual_provider_session_id,
                    )
                );
            }
        }
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
                let provider_session_id = session.provider_session_id().await;
                let _ = session
                    .close_with_behavior(AcpSessionCloseBehavior::ArchiveFailure(
                        archive_context_for_runtime_manager_request(
                            &req,
                            registered_generation_id.as_deref(),
                            Some(provider_session_id),
                            "acp_prompt_error",
                        ),
                    ))
                    .await;
                self.release_xcode_leases(cleanup).await;
                return Err(err);
            }
        };
        let capture_context = archive_context_for_runtime_manager_request(
            &req,
            registered_generation_id.as_deref(),
            result.provider_session_id.clone(),
            "pending_settlement",
        );
        match session
            .stage_provider_session_store_for_outcome(&capture_context)
            .await
        {
            Ok(capture) => result.provider_session_store_capture = capture,
            Err(error) => warn!(
                provider = %req.provider,
                run_id = %req.run_id,
                stage_id = %req.stage_id,
                error = %error,
                "Failed to stage ACP provider session store for settlement"
            ),
        }
        let mut keep_session_alive =
            should_keep_session_alive_after_prompt(req.keep_session_alive, &result.status);
        if keep_session_alive && result.status == AgentStatus::Failed && !session.is_live().await {
            keep_session_alive = false;
        }
        if keep_session_alive {
            let generation_id = req.session_generation_id.clone().ok_or_else(|| {
                anyhow::anyhow!("keep_session_alive requested without session_generation_id")
            })?;
            result.session_generation_id = Some(generation_id);
            result.reused_existing_session = false;
            result.runtime_tool_path_preflight_json =
                crate::adapters::mark_runtime_tool_path_preflight_provider_launched(
                    runtime_tool_path_preflight_json.clone(),
                );
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
        let close_result = session
            .close_with_behavior(AcpSessionCloseBehavior::Delete)
            .await;
        self.release_xcode_leases(cleanup).await;
        result.close_diagnostic = close_result?.diagnostic;
        result.session_generation_id = None;
        result.reused_existing_session = false;
        result.runtime_tool_path_preflight_json =
            crate::adapters::mark_runtime_tool_path_preflight_provider_launched(
                runtime_tool_path_preflight_json,
            );
        self.record_xcode_prompt_observations(&session_req, &result)
            .await;
        Ok(result)
    }

    pub async fn provider_session_resurrection_capability(
        &self,
        provider: &str,
    ) -> Result<Option<ProviderSessionResurrectionCapability>> {
        let provider = canonical_acp_provider(provider);
        let adapter = self.adapter_for(&provider).await?;
        Ok(adapter.provider_session_resurrection_capability())
    }

    pub async fn attach_provider_session_for_resurrection(
        &self,
        req: ExecutionRequest,
    ) -> Result<ProviderSessionResurrectionAttachResult> {
        self.attach_provider_session_for_resurrection_with_launch_observer(req, None)
            .await
    }

    pub async fn attach_provider_session_for_resurrection_with_launch_observer(
        &self,
        mut req: ExecutionRequest,
        launch_observer: Option<Arc<dyn AcpLaunchObserver>>,
    ) -> Result<ProviderSessionResurrectionAttachResult> {
        req.provider = canonical_acp_provider(&req.provider);
        let provider = req.provider.clone();
        let adapter = self.adapter_for(&provider).await?;
        let capability = adapter
            .provider_session_resurrection_capability()
            .filter(|capability| {
                capability.attach_resume_supported && capability.identity_proof_supported
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "provider_session_resurrection_unsupported: adapter '{}' does not support provider-session resurrection",
                    provider
                )
            })?;
        let requested_provider_session_id = req
            .provider_session_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("provider_session_id_required"))?;
        let session_generation_id = req.session_generation_id.clone().ok_or_else(|| {
            anyhow::anyhow!("session_generation_id_required_for_resurrection_attach")
        })?;

        info!(
            provider = %provider,
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            session_generation_id = %session_generation_id,
            requested_provider_session_ref = %provider_session_id_ref(&requested_provider_session_id),
            "AcpRuntimeManager: attaching provider session for resurrection"
        );

        let opened = self
            .open_ordered_session_with_launch_observer(adapter, &req, launch_observer)
            .await?;
        let actual_provider_session_id = opened.session.provider_session_id().await;
        if actual_provider_session_id != requested_provider_session_id {
            let _ = opened
                .session
                .close_with_behavior(AcpSessionCloseBehavior::Delete)
                .await;
            self.release_xcode_leases(opened.lease_cleanup).await;
            bail!(
                "{}",
                provider_session_identity_mismatch_message(
                    "provider_session_resurrection_identity_mismatch",
                    &requested_provider_session_id,
                    &actual_provider_session_id,
                )
            );
        }

        let managed_child_pid = opened.session.child_pid().await;
        let managed_process_group_id = managed_child_pid;
        self.live_sessions
            .lock()
            .await
            .insert(session_generation_id.clone(), opened.session);
        if let Some(cleanup) = opened.lease_cleanup {
            self.live_xcode_leases
                .lock()
                .await
                .insert(session_generation_id.clone(), cleanup);
        }

        Ok(ProviderSessionResurrectionAttachResult {
            provider,
            adapter_id: capability.adapter_id,
            adapter_capability_version: capability.capability_version,
            requested_provider_session_id,
            actual_provider_session_id,
            identity_proof_source: capability.identity_proof_source,
            identity_proof_observed_at: chrono::Utc::now().to_rfc3339(),
            session_generation_id,
            managed_child_pid,
            managed_process_group_id,
        })
    }

    async fn open_ordered_session(
        &self,
        adapter: Arc<dyn AcpAdapter>,
        req: &ExecutionRequest,
    ) -> Result<OpenedAcpSession> {
        self.open_ordered_session_with_launch_observer(adapter, req, None)
            .await
    }

    async fn open_ordered_session_with_launch_observer(
        &self,
        adapter: Arc<dyn AcpAdapter>,
        req: &ExecutionRequest,
        launch_observer: Option<Arc<dyn AcpLaunchObserver>>,
    ) -> Result<OpenedAcpSession> {
        let mut resources = LaunchResourceGuard::default();
        let mut launch_spec = adapter.prepare_launch_spec(req, &mut resources)?;
        launch_spec.provider_launch_gate = Some(self.provider_launch_gate());
        launch_spec.apply_chainworks_meta_root_env(req);
        launch_spec.apply_chainworks_rust_cache_env();
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
        let mut session_req = attachment.request;
        if session_req.provider_runtime_home.is_none() {
            session_req.provider_runtime_home = launch_spec
                .provider_runtime_home
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned());
        }
        self.attach_xcode_shim_runtime_if_needed(&session_req, &mut launch_spec)
            .await?;

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
            let Some(toolchain_home) = req.toolchain_home.as_deref() else {
                self.release_xcode_leases(lease_cleanup).await;
                return Err(anyhow::anyhow!(
                    "toolchain_mapping_setup_failed for Go session scope: missing toolchain_home"
                ));
            };
            let Some(session_gen_id) = req.session_generation_id.as_deref() else {
                self.release_xcode_leases(lease_cleanup).await;
                return Err(anyhow::anyhow!(
                    "toolchain_mapping_setup_failed for Go session scope: missing session_generation_id"
                ));
            };

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
        launch_spec.cleanup_paths.extend(resources.commit());
        let opened = match adapter
            .open_session_with_specs_and_launch_observer(
                &session_req,
                launch_spec,
                session_new_spec,
                launch_observer,
            )
            .await
        {
            Ok(opened) => opened,
            Err(err) => {
                self.release_xcode_leases(lease_cleanup).await;
                return Err(err);
            }
        };
        Ok(OpenedAcpSession {
            session: opened.session,
            session_req,
            lease_cleanup,
            runtime_tool_path_preflight_json: opened.runtime_tool_path_preflight_json,
        })
    }

    async fn attach_xcode_shim_runtime_if_needed(
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
        let lease_id = format!("xcode-shim-{token_id}");
        launch_spec.attach_xcode_shim_runtime(XcodeShimLaunchRuntime {
            token_id: token_id.clone(),
            token_secret,
            lease_id: lease_id.clone(),
            socket_path: config.socket_path,
            shim_dir: config.shim_dir,
            workspace_root: req.workspace_root.clone(),
            agent_execution_id: req.agent_execution_id,
            store: config.store,
        });
        if let Some(agent_execution_id) = req.agent_execution_id {
            let runtime = launch_spec
                .xcode_shim_runtime
                .as_ref()
                .expect("xcode shim runtime just attached");
            let reason = if req.requires_xcode_host_execution {
                "requires_xcode_host_execution"
            } else {
                "xcode_shim_injection_signal"
            };
            let update = XcodeRuntimeObservationUpdate::XcodeShimEvent(
                XcodeShimEvent::ShimRuntimeAttached(XcodeShimRuntimeAttachedEvent {
                    ts: chrono::Utc::now(),
                    source: "xcode_shim_runtime".to_string(),
                    reason: reason.to_string(),
                    lease_id,
                    shim_dir: runtime.shim_dir.clone(),
                    socket_path: runtime.socket_path.clone(),
                    workspace_root: runtime.workspace_root.clone(),
                    agent_execution_id: Some(agent_execution_id.to_string()),
                }),
            );
            self.xcode_runtime_observation_sink()
                .append_xcode_runtime_observation(agent_execution_id, update)
                .await?;
        }
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
                    "{} for generation id '{}'",
                    provider_session_identity_mismatch_message(
                        "Live ACP session provider_session_id mismatch",
                        expected_provider_session_id,
                        &actual_provider_session_id,
                    ),
                    session_generation_id
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
                let provider_session_id = session.provider_session_id().await;
                let _ = session
                    .close_with_behavior(AcpSessionCloseBehavior::ArchiveFailure(
                        archive_context_for_runtime_manager_request(
                            &req,
                            Some(session_generation_id),
                            Some(provider_session_id),
                            "acp_prompt_error",
                        ),
                    ))
                    .await;
                self.release_xcode_leases(cleanup).await;
                return Err(err);
            }
        };
        result.session_generation_id = Some(session_generation_id.to_string());
        result.reused_existing_session = true;
        let capture_context = archive_context_for_runtime_manager_request(
            &req,
            Some(session_generation_id),
            result.provider_session_id.clone(),
            "pending_settlement",
        );
        match session
            .stage_provider_session_store_for_outcome(&capture_context)
            .await
        {
            Ok(capture) => result.provider_session_store_capture = capture,
            Err(error) => warn!(
                provider = %req.provider,
                run_id = %req.run_id,
                stage_id = %req.stage_id,
                error = %error,
                "Failed to stage reused ACP provider session store for settlement"
            ),
        }
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
            provider_launch_gate: RwLock::new(Arc::new(NoopAcpProviderLaunchGate)),
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
    runtime_tool_path_preflight_json: Option<String>,
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

fn should_keep_session_alive_after_prompt(requested: bool, status: &AgentStatus) -> bool {
    requested && matches!(status, AgentStatus::Completed | AgentStatus::Failed)
}

fn archive_context_for_runtime_manager_request(
    req: &ExecutionRequest,
    session_generation_id: Option<&str>,
    provider_session_id: Option<String>,
    failure_kind: &str,
) -> ProviderSessionStoreArchiveContext {
    ProviderSessionStoreArchiveContext {
        provider: req.provider.clone(),
        run_id: req.run_id.to_string(),
        stage_id: req.stage_id.clone(),
        agent_id: req.agent_id.clone(),
        agent_execution_id: req.agent_execution_id.as_ref().map(ToString::to_string),
        session_generation_id: session_generation_id
            .map(ToString::to_string)
            .or_else(|| req.session_generation_id.clone()),
        provider_session_id: provider_session_id.or_else(|| req.provider_session_id.clone()),
        failure_kind: failure_kind.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{AcpLaunchSpec, AcpSessionNewSpec, LaunchResourceGuard};
    use domain::discovery::LegacyBroadDiscoveryPolicy;
    use domain::ids::AgentExecutionId;
    use tokio::sync::Mutex as TokioMutex;

    static XCODE_BROKER_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    #[derive(Default)]
    struct RecordingObservationSink {
        updates: TokioMutex<Vec<(AgentExecutionId, XcodeRuntimeObservationUpdate)>>,
    }

    #[async_trait::async_trait]
    impl XcodeRuntimeObservationSink for RecordingObservationSink {
        async fn append_xcode_runtime_observation(
            &self,
            agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().await.push((agent_execution_id, update));
            Ok(())
        }
    }

    #[derive(Default)]
    struct NoopGrantStore;

    impl XcodeShimGrantStore for NoopGrantStore {
        fn insert_xcode_shim_grant(&self, _record: crate::XcodeShimGrantRecord) {}

        fn set_xcode_shim_grant_active_prompt(
            &self,
            _token_id: &str,
            _active_prompt: bool,
        ) -> bool {
            false
        }

        fn remove_xcode_shim_grant(&self, _token_id: &str) -> Option<crate::XcodeShimGrantRecord> {
            None
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

    #[test]
    fn keep_alive_request_preserves_failed_prompt_for_output_repair() {
        assert!(
            should_keep_session_alive_after_prompt(true, &AgentStatus::Failed),
            "a failed prompt may still have a live session that executor can repair in-place"
        );
        assert!(should_keep_session_alive_after_prompt(
            true,
            &AgentStatus::Completed
        ));
        assert!(!should_keep_session_alive_after_prompt(
            false,
            &AgentStatus::Failed
        ));
    }

    #[test]
    fn provider_session_id_ref_is_stable_and_redacted() {
        let raw = "claude-provider-session-secret-123";
        let first = provider_session_id_ref(raw);
        let second = provider_session_id_ref(raw);

        assert_eq!(first, second);
        assert!(first.starts_with("sha256:"));
        assert_eq!(first.len(), "sha256:".len() + 64);
        assert!(
            !first.contains(raw),
            "provider session refs must not expose the raw continuation handle"
        );
    }

    #[test]
    fn provider_session_identity_mismatch_message_omits_raw_handles() {
        let expected = "claude-expected-session-secret";
        let actual = "claude-actual-session-secret";
        let message = provider_session_identity_mismatch_message(
            "provider_session_resurrection_identity_mismatch",
            expected,
            actual,
        );

        assert!(message.contains("provider_session_resurrection_identity_mismatch"));
        assert!(message.contains("expected_ref 'sha256:"));
        assert!(message.contains("actual_ref 'sha256:"));
        assert!(
            !message.contains(expected) && !message.contains(actual),
            "mismatch diagnostics must not leak raw provider session ids: {message}"
        );
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

    struct SpawnMarkerAdapter {
        script_path: String,
    }

    #[async_trait::async_trait]
    impl AcpAdapter for SpawnMarkerAdapter {
        fn provider_name(&self) -> &str {
            "fixture"
        }

        fn prepare_launch_spec(
            &self,
            _req: &ExecutionRequest,
            _resources: &mut LaunchResourceGuard,
        ) -> Result<AcpLaunchSpec> {
            Ok(AcpLaunchSpec::new(&self.script_path))
        }

        fn prepare_session_new_spec(&self, _req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
            Ok(AcpSessionNewSpec::new("fixture-model", "default"))
        }
    }

    fn request_with_go_toolchain_scope() -> ExecutionRequest {
        ExecutionRequest {
            agent_execution_id: None,
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_go".to_string(),
            attempt_number: 1,
            agent_id: "agent_go".to_string(),
            provider: "fixture".to_string(),
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
            session_generation_id: Some("generation-go".to_string()),
            provider_session_id: None,
            provider_runtime_home: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: Some("/tmp/toolchain-home".to_string()),
            toolchain_go_scope_enabled: true,

            p079_repair_canonical_paths: None,
        }
    }

    #[cfg(unix)]
    fn spawn_marker_script(tmp: &tempfile::TempDir, marker: &Path) -> String {
        use std::os::unix::fs::PermissionsExt;

        let script = tmp.path().join("spawn_marker.sh");
        std::fs::write(
            &script,
            format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();
        script.to_string_lossy().into_owned()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn go_toolchain_scope_enabled_without_toolchain_home_fails_before_launch() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("spawned.txt");
        let adapter = Arc::new(SpawnMarkerAdapter {
            script_path: spawn_marker_script(&tmp, &marker),
        });
        let manager = AcpRuntimeManager::new_with_adapters(vec![adapter]);
        let mut req = request_with_go_toolchain_scope();
        req.toolchain_home = None;

        let err = manager
            .start_session(req)
            .await
            .expect_err("missing toolchain_home must fail closed");

        assert!(
            err.to_string()
                .contains("toolchain_mapping_setup_failed for Go session scope"),
            "error should identify Go toolchain mapping failure: {err}"
        );
        assert!(
            !marker.exists(),
            "manager must fail before spawning provider subprocess"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn go_toolchain_scope_enabled_without_session_generation_id_fails_before_launch() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("spawned.txt");
        let adapter = Arc::new(SpawnMarkerAdapter {
            script_path: spawn_marker_script(&tmp, &marker),
        });
        let manager = AcpRuntimeManager::new_with_adapters(vec![adapter]);
        let mut req = request_with_go_toolchain_scope();
        req.toolchain_home = Some(tmp.path().join("toolchains").to_string_lossy().into_owned());
        req.session_generation_id = None;

        let err = manager
            .start_session(req)
            .await
            .expect_err("missing session_generation_id must fail closed");

        assert!(
            err.to_string()
                .contains("toolchain_mapping_setup_failed for Go session scope"),
            "error should identify Go toolchain mapping failure: {err}"
        );
        assert!(
            !marker.exists(),
            "manager must fail before spawning provider subprocess"
        );
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

    #[tokio::test]
    async fn broker_disabled_env_suppresses_xcode_shim_injection() {
        let _guard = XCODE_BROKER_ENV_LOCK.lock().expect("env lock poisoned");
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
            provider_runtime_home: None,
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

            p079_repair_canonical_paths: None,
        };

        manager
            .attach_xcode_shim_runtime_if_needed(&req, &mut launch_spec)
            .await
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

    #[tokio::test]
    async fn xcode_shim_runtime_attachment_is_persisted_as_observation() {
        let _guard = XCODE_BROKER_ENV_LOCK.lock().expect("env lock poisoned");
        let previous = std::env::var("CHAINWORKS_XCODE_BROKER_DISABLED").ok();
        std::env::remove_var("CHAINWORKS_XCODE_BROKER_DISABLED");

        let manager = AcpRuntimeManager::new_with_adapters(Vec::new());
        let sink = Arc::new(RecordingObservationSink::default());
        manager.set_xcode_runtime_observation_sink(sink.clone());
        manager.set_xcode_shim_runtime(
            Arc::new(NoopGrantStore),
            "/tmp/chainworks-test-xcode-shim.sock",
            "/tmp/chainworks-test-xcode-shims",
        );

        let agent_execution_id = AgentExecutionId::new();
        let req = ExecutionRequest {
            agent_execution_id: Some(agent_execution_id),
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_xcode".to_string(),
            attempt_number: 1,
            agent_id: "agent_xcode".to_string(),
            provider: "junie".to_string(),
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
            provider_runtime_home: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: true,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,

            p079_repair_canonical_paths: None,
        };
        let mut launch_spec = crate::adapters::AcpLaunchSpec::new("/bin/sh");

        manager
            .attach_xcode_shim_runtime_if_needed(&req, &mut launch_spec)
            .await
            .expect("xcode shim attach");

        assert!(
            launch_spec.xcode_shim_runtime.is_some(),
            "requires_xcode_host_execution must inject shim runtime"
        );
        let updates = sink.updates.lock().await;
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].0, agent_execution_id);
        match &updates[0].1 {
            XcodeRuntimeObservationUpdate::XcodeShimEvent(XcodeShimEvent::ShimRuntimeAttached(
                event,
            )) => {
                assert_eq!(event.source, "xcode_shim_runtime");
                assert_eq!(event.reason, "requires_xcode_host_execution");
                assert_eq!(
                    event.lease_id,
                    launch_spec.xcode_shim_runtime.as_ref().unwrap().lease_id
                );
                assert_eq!(
                    event.agent_execution_id.as_deref(),
                    Some(agent_execution_id.to_string().as_str())
                );
            }
            other => panic!("unexpected xcode observation update: {other:?}"),
        }
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
