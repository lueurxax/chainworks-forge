use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{bail, Result};
use tracing::info;

use crate::adapters::auggie::AuggieAdapter;
use crate::adapters::claude::ClaudeAgentAdapter;
use crate::adapters::gemini::GeminiCliAdapter;
use crate::adapters::junie::JunieAdapter;
use crate::adapters::AcpAdapter;
use crate::{ExecutionRequest, ExecutionResult};

/// Manages all registered ACP provider adapters and routes execution
/// requests to the appropriate adapter by provider name.
pub struct AcpRuntimeManager {
    adapters: HashMap<String, Arc<dyn AcpAdapter>>,
}

impl AcpRuntimeManager {
    /// Create a new manager with all ACP adapters pre-registered.
    /// Each adapter reads its binary path from an env var at construction time;
    /// execution fails fast if the env var is unset when `execute` is called.
    pub fn new() -> Self {
        let mut adapters: HashMap<String, Arc<dyn AcpAdapter>> = HashMap::new();

        let claude = Arc::new(ClaudeAgentAdapter::new()) as Arc<dyn AcpAdapter>;
        let gemini = Arc::new(GeminiCliAdapter::new()) as Arc<dyn AcpAdapter>;
        let auggie = Arc::new(AuggieAdapter::new()) as Arc<dyn AcpAdapter>;
        let junie = Arc::new(JunieAdapter::new()) as Arc<dyn AcpAdapter>;

        adapters.insert(claude.provider_name().to_string(), claude);
        adapters.insert(gemini.provider_name().to_string(), gemini);
        adapters.insert(auggie.provider_name().to_string(), auggie);
        adapters.insert(junie.provider_name().to_string(), junie);

        info!(
            registered_providers = ?adapters.keys().collect::<Vec<_>>(),
            "AcpRuntimeManager initialised"
        );

        Self { adapters }
    }

    /// Retrieve a shared reference to the adapter for the given provider name.
    pub fn get_adapter(&self, provider: &str) -> Option<Arc<dyn AcpAdapter>> {
        self.adapters.get(provider).cloned()
    }

    /// Route an execution request to the matching adapter.
    pub async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
        let provider = req.provider.clone();
        match self.adapters.get(&provider) {
            Some(adapter) => {
                info!(
                    provider = %provider,
                    run_id = %req.run_id,
                    stage_id = %req.stage_id,
                    "AcpRuntimeManager: dispatching execution"
                );
                adapter.execute(req).await
            }
            None => bail!("No adapter registered for provider '{provider}'"),
        }
    }

    /// Register an additional adapter (useful for testing or future dynamic registration).
    pub fn register(&mut self, adapter: Arc<dyn AcpAdapter>) {
        self.adapters
            .insert(adapter.provider_name().to_string(), adapter);
    }
}

impl Default for AcpRuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}
