use anyhow::{bail, Result};
use async_trait::async_trait;
use tracing::info;

use crate::adapters::{AcpAdapter, AcpLaunchSpec, AcpSessionNewSpec, LaunchResourceGuard};
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_JUNIE_ACP_BINARY";

/// Adapter for the Junie provider.
///
/// Spawns the binary given by `CHAINWORKS_JUNIE_ACP_BINARY` (defaulting to
/// `junie` on PATH) and communicates using the ACP JSON-RPC 2.0 protocol
/// over ndjson stdio.
pub struct JunieAdapter {
    binary_path: String,
}

impl JunieAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_JUNIE_ACP_BINARY`
    /// or falling back to `junie` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR).unwrap_or_else(|_| "junie".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl Default for JunieAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for JunieAdapter {
    fn provider_name(&self) -> &str {
        "junie"
    }

    fn prepare_launch_spec(
        &self,
        req: &ExecutionRequest,
        _resources: &mut LaunchResourceGuard,
    ) -> Result<AcpLaunchSpec> {
        if self.binary_path.is_empty() {
            bail!(
                "JunieAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure junie is on PATH"
            );
        }

        info!(
            provider = "junie",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Junie ACP subprocess"
        );

        Ok(AcpLaunchSpec::new(&self.binary_path)
            .with_arg("--acp")
            .with_arg("true"))
    }

    fn prepare_session_new_spec(&self, _req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
        Ok(AcpSessionNewSpec::new("default", "bypassPermissions"))
    }

    fn supports_http_mcp_capability_probe(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;

    fn request() -> ExecutionRequest {
        ExecutionRequest {
            agent_execution_id: None,
            run_id: RunId::new(),
            stage_execution_id: None,
            stage_id: "junie_stage".into(),
            attempt_number: 1,
            agent_id: "junie_agent".into(),
            provider: "junie".into(),
            model: Some("default".into()),
            effort: None,
            prompt: "test".into(),
            workspace_root: "/tmp/workspace".into(),
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: Default::default(),
            worktree_root: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".into(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,
        }
    }

    #[test]
    fn launch_spec_enters_junie_acp_mode() {
        let adapter = JunieAdapter::new_with_binary("/bin/junie-fixture");
        let mut resources = LaunchResourceGuard::default();

        let launch_spec = adapter
            .prepare_launch_spec(&request(), &mut resources)
            .expect("junie launch spec");

        assert_eq!(launch_spec.binary_path, "/bin/junie-fixture");
        assert_eq!(launch_spec.args, vec!["--acp", "true"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fixture_execution_tolerates_junie_acp_launch_args() {
        let temp = tempfile::tempdir().expect("tempdir");
        let script_path = temp.path().join("junie-acp-fixture.py");
        let result_path = temp.path().join("result.json");
        std::fs::write(
            &script_path,
            format!(
                r#"#!/usr/bin/env python3
import json
import pathlib
import sys

result_path = pathlib.Path({result_path:?})

for line in sys.stdin:
    if not line.strip():
        continue
    request = json.loads(line)
    method = request.get("method")
    request_id = request.get("id")
    if method == "initialize":
        print(json.dumps({{"jsonrpc": "2.0", "id": request_id, "result": {{"protocolVersion": 1}}}}), flush=True)
    elif method == "session/new":
        print(json.dumps({{"jsonrpc": "2.0", "id": request_id, "result": {{"sessionId": "junie-fixture-session"}}}}), flush=True)
    elif method == "session/prompt":
        result_path.write_text("{{\"ok\": true}}\n", encoding="utf-8")
        print(json.dumps({{"jsonrpc": "2.0", "method": "session/update", "params": {{"type": "agent_message_chunk", "content": "done"}}}}), flush=True)
        print(json.dumps({{"jsonrpc": "2.0", "id": request_id, "result": {{"stopReason": "end_turn"}}}}), flush=True)
    elif method == "session/close":
        print(json.dumps({{"jsonrpc": "2.0", "id": request_id, "result": {{}}}}), flush=True)
        break
"#,
                result_path = result_path.to_string_lossy()
            ),
        )
        .expect("write fixture");

        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&script_path)
            .expect("fixture metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script_path, permissions).expect("chmod fixture");

        let adapter = JunieAdapter::new_with_binary(script_path.to_string_lossy().into_owned());
        let mut req = request();
        req.workspace_root = temp.path().to_string_lossy().into_owned();
        req.expected_output_paths = vec![result_path.to_string_lossy().into_owned()];

        let result = adapter.execute(req).await.expect("fixture execution");

        assert_eq!(result.status, AgentStatus::Completed);
        assert!(
            result
                .artifact_paths
                .iter()
                .any(|path| path.ends_with("result.json")),
            "expected declared fixture artifact to be captured: {:?}",
            result.artifact_paths
        );
    }

    #[tokio::test]
    async fn live_junie_smoke_is_gated_by_environment() {
        if std::env::var("CHAINWORKS_JUNIE_ACP_LIVE_SMOKE").as_deref() != Ok("1") {
            return;
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let adapter = JunieAdapter::new();
        let mut req = request();
        req.workspace_root = temp.path().to_string_lossy().into_owned();
        req.prompt = "Reply with the exact text: OK".into();

        let result = adapter.execute(req).await.expect("live Junie ACP smoke");

        assert_eq!(result.status, AgentStatus::Completed);
    }
}
