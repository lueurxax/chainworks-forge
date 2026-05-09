use anyhow::{bail, Result};
use async_trait::async_trait;
use tracing::info;

use crate::adapters::{AcpAdapter, AcpLaunchSpec, AcpSessionNewSpec, LaunchResourceGuard};
use crate::transport::AcpSessionConfig;
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_GEMINI_ACP_BINARY";

/// Adapter for the Gemini CLI provider (`gemini --acp`).
///
/// Spawns the binary given by `CHAINWORKS_GEMINI_ACP_BINARY` (defaulting to
/// `gemini` on PATH) with the `--acp` flag, and communicates using the ACP
/// JSON-RPC 2.0 protocol over ndjson stdio.
///
/// Note: Gemini CLI requires the `--acp` flag to enter ACP server mode.
/// Some early Gemini versions may not support `session/close` — the transport
/// ignores errors from that phase gracefully.
pub struct GeminiCliAdapter {
    binary_path: String,
}

impl GeminiCliAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_GEMINI_ACP_BINARY`
    /// or falling back to `gemini` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR).unwrap_or_else(|_| "gemini".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl Default for GeminiCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for GeminiCliAdapter {
    fn provider_name(&self) -> &str {
        "gemini"
    }

    fn prepare_launch_spec(
        &self,
        req: &ExecutionRequest,
        _resources: &mut LaunchResourceGuard,
    ) -> Result<AcpLaunchSpec> {
        if self.binary_path.is_empty() {
            bail!(
                "GeminiCliAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure gemini is on PATH"
            );
        }

        info!(
            provider = "gemini",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Gemini ACP subprocess"
        );

        let mut launch_spec = AcpLaunchSpec::new(&self.binary_path);
        for arg in gemini_args_for_request(req) {
            launch_spec = launch_spec.with_arg(arg);
        }
        Ok(launch_spec)
    }

    fn prepare_session_new_spec(&self, req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
        // Gemini uses bypassPermissions mode; no _meta block needed.
        // Gemini CLI currently applies model selection from the launch
        // `--model` flag, not from session/new. Keep the session/new model
        // populated for ACP readback parity.
        let model_str = req.model.as_deref().unwrap_or("default").to_string();
        let config = AcpSessionConfig {
            model: &model_str,
            mode: "bypassPermissions",
            extra: None,
            config_options: Vec::new(),
            required_config_options: Vec::new(),
            set_mode_after_session_new: false,
        };
        Ok(AcpSessionNewSpec::from_config(config))
    }
}

fn gemini_args_for_request(req: &ExecutionRequest) -> Vec<String> {
    let mut args = vec!["--acp".to_string()];
    if let Some(model) = req
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    if let Some(meta_root) = absolute_meta_root(req) {
        args.push("--include-directories".to_string());
        args.push(meta_root);
    }
    args
}

fn absolute_meta_root(req: &ExecutionRequest) -> Option<String> {
    let meta_root = req.chainworks_meta_root.as_ref()?;
    if meta_root.starts_with('/') {
        Some(meta_root.clone())
    } else {
        Some(format!("{}/{}", req.workspace_root, meta_root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::RunId;

    fn request_with_meta_root(workspace_root: &str, meta_root: Option<&str>) -> ExecutionRequest {
        ExecutionRequest {
            run_id: RunId::new(),
            stage_execution_id: None,
            stage_id: "stage".into(),
            attempt_number: 1,
            agent_execution_id: None,
            agent_id: "docs_guardian".into(),
            provider: "gemini".into(),
            model: None,
            effort: None,
            workspace_root: workspace_root.into(),
            prompt: "prompt".into(),
            worktree_root: Some(format!("{workspace_root}/.chainworks/worktrees/impl")),
            worktree_write_enabled: true,
            worktree_strategy: Some("shared_implementation_worktree".into()),
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: meta_root.map(str::to_string),
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
        }
    }

    #[test]
    fn gemini_args_include_run_meta_root_as_workspace_directory() {
        let req = request_with_meta_root(
            "/workspace",
            Some(".chainworks/runs/9318de0d-9c75-40ad-9d0a-74c3610b021d"),
        );

        let args = gemini_args_for_request(&req);

        assert_eq!(args[0], "--acp");
        assert_eq!(args[1], "--include-directories");
        assert_eq!(
            args[2],
            "/workspace/.chainworks/runs/9318de0d-9c75-40ad-9d0a-74c3610b021d"
        );
    }

    #[test]
    fn gemini_args_pin_requested_model_at_process_launch() {
        let mut req = request_with_meta_root(
            "/workspace",
            Some(".chainworks/runs/9318de0d-9c75-40ad-9d0a-74c3610b021d"),
        );
        req.model = Some("gemini-3.1-pro-preview".to_string());

        let args = gemini_args_for_request(&req);

        assert_eq!(
            args,
            vec![
                "--acp".to_string(),
                "--model".to_string(),
                "gemini-3.1-pro-preview".to_string(),
                "--include-directories".to_string(),
                "/workspace/.chainworks/runs/9318de0d-9c75-40ad-9d0a-74c3610b021d".to_string(),
            ]
        );
    }

    #[test]
    fn launch_spec_includes_run_meta_root_as_workspace_directory() {
        let adapter = GeminiCliAdapter::new_with_binary("/bin/gemini");
        let req = request_with_meta_root(
            "/workspace",
            Some(".chainworks/runs/9318de0d-9c75-40ad-9d0a-74c3610b021d"),
        );
        let mut resources = crate::adapters::LaunchResourceGuard::default();

        let spec = adapter.prepare_launch_spec(&req, &mut resources).unwrap();

        assert_eq!(
            spec.args,
            vec![
                "--acp".to_string(),
                "--include-directories".to_string(),
                "/workspace/.chainworks/runs/9318de0d-9c75-40ad-9d0a-74c3610b021d".to_string(),
            ]
        );
    }

    #[test]
    fn gemini_args_preserve_absolute_run_meta_root() {
        let req = request_with_meta_root("/workspace", Some("/tmp/chainworks-meta"));

        let args = gemini_args_for_request(&req);

        assert_eq!(
            args,
            vec![
                "--acp".to_string(),
                "--include-directories".to_string(),
                "/tmp/chainworks-meta".to_string(),
            ]
        );
    }
}
