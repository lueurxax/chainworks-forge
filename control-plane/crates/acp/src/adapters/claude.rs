use anyhow::{bail, Result};
use async_trait::async_trait;
use tracing::info;

use crate::adapters::{
    provider_session_resurrection_failure_classes, AcpAdapter, AcpLaunchSpec, AcpSessionNewSpec,
    LaunchResourceGuard, ProviderSessionResurrectionCapability,
};
use crate::transport::AcpSessionConfig;
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_CLAUDE_ACP_BINARY";

/// Adapter for the Claude Agent provider (`claude-agent-acp`).
///
/// Spawns the binary given by `CHAINWORKS_CLAUDE_ACP_BINARY` (defaulting to
/// `claude-agent-acp` on PATH) and communicates with it using the ACP
/// JSON-RPC 2.0 protocol over ndjson stdio.
///
/// Protocol: `initialize` → `session/new` → `session/prompt` (streaming)
///   → `session/close` → graceful shutdown.
///
/// Artifact paths are discovered by diffing the workspace filesystem before
/// and after the session.
pub struct ClaudeAgentAdapter {
    binary_path: String,
    set_mode_after_session_new: bool,
}

impl ClaudeAgentAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_CLAUDE_ACP_BINARY`
    /// or falling back to `claude-agent-acp` on PATH.
    pub fn new() -> Self {
        let binary_path =
            std::env::var(BINARY_ENV_VAR).unwrap_or_else(|_| "claude-agent-acp".to_string());
        Self {
            binary_path,
            set_mode_after_session_new: true,
        }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
            set_mode_after_session_new: false,
        }
    }
}

impl Default for ClaudeAgentAdapter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn claude_provider_session_resurrection_capability() -> ProviderSessionResurrectionCapability {
    ProviderSessionResurrectionCapability {
        provider_family: "claude".to_string(),
        adapter_id: "claude-agent-acp".to_string(),
        capability_version: "provider_session_resurrection_v1".to_string(),
        attach_resume_supported: true,
        session_new_fields: vec!["resumeSessionId".to_string()],
        attach_request_shape: "session/new.params.resumeSessionId".to_string(),
        attach_result_shape: "session/new.result.sessionId".to_string(),
        identity_proof_supported: true,
        identity_proof_source: "session_new_result.sessionId".to_string(),
        write_enabled_safe: true,
        failure_classes: provider_session_resurrection_failure_classes(),
    }
}

#[async_trait]
impl AcpAdapter for ClaudeAgentAdapter {
    fn provider_name(&self) -> &str {
        "claude"
    }

    fn prepare_launch_spec(
        &self,
        req: &ExecutionRequest,
        _resources: &mut LaunchResourceGuard,
    ) -> Result<AcpLaunchSpec> {
        if self.binary_path.is_empty() {
            bail!(
                "ClaudeAgentAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure claude-agent-acp is on PATH"
            );
        }

        info!(
            provider = "claude",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Claude ACP subprocess"
        );

        Ok(AcpLaunchSpec::new(&self.binary_path))
    }

    fn prepare_session_new_spec(&self, req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
        let default_config = AcpSessionConfig::default();
        let model_str = req
            .model
            .as_deref()
            .unwrap_or(default_config.model)
            .to_string();
        let mut extra = default_config.extra.clone();
        if let Some(provider_session_id) = req
            .provider_session_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let extra_obj = extra.get_or_insert_with(|| serde_json::json!({}));
            if let Some(obj) = extra_obj.as_object_mut() {
                obj.insert(
                    "resumeSessionId".to_string(),
                    serde_json::Value::String(provider_session_id.to_string()),
                );
            }
        }
        let required_config_options = req
            .model
            .as_ref()
            .map(|model| vec![("model".to_string(), model.to_string())])
            .unwrap_or_default();
        let config = AcpSessionConfig {
            model: &model_str,
            extra,
            required_config_options,
            set_mode_after_session_new: self.set_mode_after_session_new,
            ..default_config
        };
        Ok(AcpSessionNewSpec::from_config(config))
    }

    fn provider_session_resurrection_capability(
        &self,
    ) -> Option<ProviderSessionResurrectionCapability> {
        Some(claude_provider_session_resurrection_capability())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::RunId;

    fn request(provider_session_id: Option<&str>) -> ExecutionRequest {
        ExecutionRequest {
            agent_execution_id: None,
            run_id: RunId::new(),
            stage_execution_id: Some("stage-exec".to_string()),
            stage_id: "implementation".to_string(),
            attempt_number: 1,
            agent_id: "code_writer".to_string(),
            provider: "claude".to_string(),
            model: Some("fixture-model".to_string()),
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
            provider_session_id: provider_session_id.map(str::to_string),
            provider_runtime_home: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".to_string(),
            owner_id: Some("stage-exec".to_string()),
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,
            p079_repair_canonical_paths: None,
        }
    }

    #[test]
    fn claude_resurrection_capability_declares_session_new_identity_proof() {
        let adapter = ClaudeAgentAdapter::new_with_binary("/bin/echo");
        let capability = adapter
            .provider_session_resurrection_capability()
            .expect("claude capability");
        assert!(capability.attach_resume_supported);
        assert!(capability.identity_proof_supported);
        assert_eq!(
            capability.capability_version,
            "provider_session_resurrection_v1"
        );
        assert_eq!(capability.session_new_fields, vec!["resumeSessionId"]);
        assert_eq!(
            capability.identity_proof_source,
            "session_new_result.sessionId"
        );
        assert!(capability
            .failure_classes
            .contains(&"actual_session_mismatch".to_string()));
    }

    #[test]
    fn claude_resurrection_request_includes_resume_session_id() {
        let adapter = ClaudeAgentAdapter::new_with_binary("/bin/echo");
        let spec = adapter
            .prepare_session_new_spec(&request(Some("provider-session-123")))
            .expect("session spec");
        assert_eq!(
            spec.extra
                .as_ref()
                .and_then(|extra| extra.get("resumeSessionId"))
                .and_then(serde_json::Value::as_str),
            Some("provider-session-123")
        );
    }
}
