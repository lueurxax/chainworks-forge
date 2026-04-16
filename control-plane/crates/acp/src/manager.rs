use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tracing::info;

use crate::adapters::auggie::AuggieAdapter;
use crate::adapters::claude::ClaudeAgentAdapter;
use crate::adapters::codex::CodexAdapter;
use crate::adapters::gemini::GeminiCliAdapter;
use crate::adapters::junie::JunieAdapter;
use crate::adapters::AcpAdapter;
use crate::session::AcpSessionHandle;
use crate::{ExecutionRequest, ExecutionResult};

/// Manages ACP provider adapters and owns any live reusable sessions.
pub struct AcpRuntimeManager {
    adapters: HashMap<String, Arc<dyn AcpAdapter>>,
    live_sessions: Mutex<HashMap<String, AcpSessionHandle>>,
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
        }
    }

    /// Retrieve a shared reference to the adapter for the given provider name.
    pub fn get_adapter(&self, provider: &str) -> Option<Arc<dyn AcpAdapter>> {
        self.adapters.get(provider).cloned()
    }

    async fn adapter_for(&self, provider: &str) -> Result<Arc<dyn AcpAdapter>> {
        self.adapters
            .get(provider)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No adapter registered for provider '{provider}'"))
    }

    async fn live_session(&self, generation_id: &str) -> Result<AcpSessionHandle> {
        self.live_sessions
            .lock()
            .await
            .get(generation_id)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No live ACP session registered for generation id '{generation_id}'"
                )
            })
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
        match provider_session_id {
            Some(expected) => session.provider_session_id().await == expected,
            None => true,
        }
    }

    /// Start a fresh ACP session and keep it alive if requested.
    pub async fn start_session(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
        let provider = req.provider.clone();
        let adapter = self.adapter_for(&provider).await?;
        info!(
            provider = %provider,
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            "AcpRuntimeManager: starting session"
        );

        let session = adapter.open_session(&req).await?;
        let mut result = session.prompt(&req).await?;
        if req.keep_session_alive {
            let generation_id = req.session_generation_id.clone().ok_or_else(|| {
                anyhow::anyhow!("keep_session_alive requested without session_generation_id")
            })?;
            self.live_sessions
                .lock()
                .await
                .insert(generation_id.clone(), session);
            result.session_generation_id = Some(generation_id);
            result.reused_existing_session = false;
            return Ok(result);
        }

        session.close().await?;
        result.session_generation_id = None;
        result.reused_existing_session = false;
        Ok(result)
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

        let mut result = session.prompt(&req).await?;
        result.session_generation_id = Some(session_generation_id.to_string());
        result.reused_existing_session = true;
        Ok(result)
    }

    /// Close and remove a live ACP session.
    pub async fn close_session(&self, session_generation_id: &str) -> Result<()> {
        let session = self
            .live_sessions
            .lock()
            .await
            .remove(session_generation_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No live ACP session registered for generation id '{session_generation_id}'"
                )
            })?;
        session.close().await
    }

    /// Route an execution request to the matching adapter or live session.
    pub async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
        if req.reuse_existing_session {
            let session_generation_id = req.session_generation_id.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "reuse_existing_session was requested but no session_generation_id was provided"
                )
            })?;
            return self.prompt_session(&session_generation_id, req).await;
        }

        self.start_session(req).await
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
        }
    }
}

impl Default for AcpRuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}
