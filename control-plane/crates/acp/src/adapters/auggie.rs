use anyhow::{bail, Result};
use async_trait::async_trait;
use tracing::info;

use crate::adapters::{AcpAdapter, AcpLaunchSpec, AcpSessionNewSpec, LaunchResourceGuard};
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_AUGGIE_ACP_BINARY";

/// Adapter for the Auggie provider.
///
/// Spawns the binary given by `CHAINWORKS_AUGGIE_ACP_BINARY` (defaulting to
/// `auggie` on PATH) and communicates using the ACP JSON-RPC 2.0 protocol
/// over ndjson stdio.
pub struct AuggieAdapter {
    binary_path: String,
}

impl AuggieAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_AUGGIE_ACP_BINARY`
    /// or falling back to `auggie` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR).unwrap_or_else(|_| "auggie".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl Default for AuggieAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for AuggieAdapter {
    fn provider_name(&self) -> &str {
        "auggie"
    }

    fn prepare_launch_spec(
        &self,
        req: &ExecutionRequest,
        _resources: &mut LaunchResourceGuard,
    ) -> Result<AcpLaunchSpec> {
        if self.binary_path.is_empty() {
            bail!(
                "AuggieAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure auggie is on PATH"
            );
        }

        info!(
            provider = "auggie",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Auggie ACP subprocess"
        );

        Ok(auggie_launch_spec(&self.binary_path))
    }

    fn prepare_session_new_spec(&self, _req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
        Ok(AcpSessionNewSpec::new("default", "bypassPermissions"))
    }

    fn supports_http_mcp_capability_probe(&self) -> bool {
        false
    }
}

fn auggie_launch_spec(binary_path: &str) -> AcpLaunchSpec {
    AcpLaunchSpec::new(binary_path).with_arg("--acp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_spec_enters_auggie_acp_mode() {
        let launch_spec = auggie_launch_spec("/bin/auggie-fixture");

        assert_eq!(launch_spec.binary_path, "/bin/auggie-fixture");
        assert_eq!(launch_spec.args, ["--acp"]);
    }
}
