use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::Duration;
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
        let mut spec = AcpSessionNewSpec::new("default", "bypassPermissions");
        spec.permission_grant_debounce = Duration::from_millis(25);
        Ok(spec)
    }

    fn preflight_launch(
        &self,
        req: &ExecutionRequest,
        launch_spec: &mut AcpLaunchSpec,
    ) -> Result<()> {
        let enforcement_enabled = junie_p090_preflight_enforced();
        match run_junie_p090_tool_path_preflight(req, launch_spec.current_dir_override.as_deref()) {
            Ok(()) => {
                launch_spec.runtime_tool_path_preflight_json = Some(junie_p090_preflight_json(
                    1,
                    None,
                    &["preflight_running", "passed"],
                    enforcement_enabled,
                ));
                Ok(())
            }
            Err(error)
                if error
                    .to_string()
                    .contains("missing_provider_execution_root") =>
            {
                let workspace_root = PathBuf::from(&req.workspace_root);
                if !workspace_root.is_dir() {
                    return Err(error);
                }
                launch_spec.current_dir_override = Some(workspace_root);
                run_junie_p090_tool_path_preflight(
                    req,
                    launch_spec.current_dir_override.as_deref(),
                )
                .context(
                    "p090_junie_preflight_remediation_failed: wrong_cwd fallback to workspace_root",
                )?;
                launch_spec.runtime_tool_path_preflight_json = Some(junie_p090_preflight_json(
                    2,
                    Some("wrong_cwd_workspace_root"),
                    &["preflight_running", "preflight_remediating", "passed"],
                    enforcement_enabled,
                ));
                Ok(())
            }
            Err(error) if error.to_string().contains("missing_junie_runtime_cache") => {
                remediate_junie_runtime_cache(req).with_context(|| {
                    "p090_junie_preflight_remediation_failed: runtime_home_cache_rebuilt"
                })?;
                run_junie_p090_tool_path_preflight(
                    req,
                    launch_spec.current_dir_override.as_deref(),
                )
                .context("p090_junie_preflight_remediation_failed: runtime_home_cache_rebuilt")?;
                launch_spec.runtime_tool_path_preflight_json = Some(junie_p090_preflight_json(
                    2,
                    Some("runtime_home_cache_rebuilt"),
                    &["preflight_running", "preflight_remediating", "passed"],
                    enforcement_enabled,
                ));
                Ok(())
            }
            Err(error) if !enforcement_enabled => {
                launch_spec.runtime_tool_path_preflight_json = Some(
                    junie_p090_failed_diagnostic_json(&error, enforcement_enabled),
                );
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn supports_http_mcp_capability_probe(&self) -> bool {
        true
    }
}

fn junie_p090_preflight_enforced() -> bool {
    matches!(
        std::env::var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE")
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn junie_p090_preflight_json(
    attempt_count: u64,
    remediation_applied: Option<&str>,
    lifecycle_phases: &[&str],
    enforcement_enabled: bool,
) -> String {
    serde_json::json!({
        "schema": "p090_runtime_tool_path_preflight_v1",
        "status": "passed",
        "attempt_count": attempt_count,
        "remediation_applied": remediation_applied,
        "provider_launched": false,
        "enforcement_enabled": enforcement_enabled,
        "failed_operation_class": null,
        "redacted_path_class": null,
        "failure_category": null,
        "remediation_hint": null,
        "lifecycle_phases": lifecycle_phases,
    })
    .to_string()
}

fn junie_p090_failed_diagnostic_json(error: &anyhow::Error, enforcement_enabled: bool) -> String {
    let message = error.to_string();
    let (operation, path_class, category, hint) =
        if message.contains("missing_provider_execution_root") {
            (
                "read_project_file",
                "provider_execution_root",
                "wrong_cwd_or_missing_project_root",
                "retry with the canonical worktree/project root",
            )
        } else if message.contains("missing_junie_runtime_cache")
            || message.contains("missing_chainworks_meta_root")
        {
            (
                "read_runtime_home",
                "junie_runtime_cache",
                "runtime_home_unwritable",
                "repair or recreate the Junie ACP runtime cache directory",
            )
        } else if message.contains("Operation not permitted") {
            (
                "read_project_file",
                "project_root",
                "operation_not_permitted",
                "repair macOS privacy/sandbox access for the project root",
            )
        } else {
            (
                "read_project_file",
                "project_root",
                "permission_denied",
                "repair filesystem permissions for the project root",
            )
        };
    serde_json::json!({
        "schema": "p090_runtime_tool_path_preflight_v1",
        "status": "diagnostic_failed_launch_allowed",
        "attempt_count": 1,
        "remediation_applied": null,
        "provider_launched": false,
        "enforcement_enabled": enforcement_enabled,
        "failed_operation_class": operation,
        "redacted_path_class": path_class,
        "failure_category": category,
        "remediation_hint": hint,
        "lifecycle_phases": ["preflight_running", "diagnostic_failed_launch_allowed"],
    })
    .to_string()
}

fn run_junie_p090_tool_path_preflight(
    req: &ExecutionRequest,
    current_dir_override: Option<&Path>,
) -> Result<()> {
    let execution_root = if let Some(override_root) = current_dir_override {
        override_root.to_path_buf()
    } else if req.worktree_write_enabled {
        req.worktree_root
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&req.workspace_root))
    } else {
        PathBuf::from(&req.workspace_root)
    };
    if !execution_root.is_dir() {
        bail!(
            "missing_provider_execution_root: p090_junie_preflight_failed: ACP provider cwd {} does not exist or is not a directory",
            execution_root.display()
        );
    }

    std::fs::read_dir(&execution_root).with_context(|| {
        format!(
            "p090_junie_preflight_failed: Operation not permitted reading project root {}",
            execution_root.display()
        )
    })?;
    if let Some(probe_path) = junie_project_read_probe_path(&execution_root) {
        let _ = std::fs::File::open(&probe_path).with_context(|| {
            format!(
                "p090_junie_preflight_failed: Operation not permitted reading project file {}",
                probe_path.display()
            )
        })?;
    }

    for output_path in junie_preflight_output_paths(req) {
        let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "missing_chainworks_meta_root: p090_junie_preflight_failed: create output directory {}",
                parent.display()
            )
        })?;
        let probe = parent.join(format!(".chainworks-p090-preflight-{}", std::process::id()));
        std::fs::write(&probe, b"p090-preflight").with_context(|| {
            format!(
                "p090_junie_preflight_failed: permission denied writing output directory {}",
                parent.display()
            )
        })?;
        let _ = std::fs::remove_file(&probe);
    }

    if let Some(runtime_cache) = junie_runtime_cache_dir(req) {
        if !runtime_cache.is_dir() {
            bail!(
                "missing_junie_runtime_cache: p090_junie_preflight_failed: runtime cache {} is missing or not a directory",
                runtime_cache.display()
            );
        }
        let probe = runtime_cache.join(format!(
            ".chainworks-p090-runtime-cache-preflight-{}",
            std::process::id()
        ));
        std::fs::write(&probe, b"p090-preflight").with_context(|| {
            format!(
                "p090_junie_preflight_failed: permission denied writing runtime cache {}",
                runtime_cache.display()
            )
        })?;
        let _ = std::fs::remove_file(&probe);
    }

    let temp_probe = std::env::temp_dir().join(format!(
        "chainworks-p090-junie-preflight-{}",
        std::process::id()
    ));
    std::fs::write(&temp_probe, b"p090-preflight").with_context(|| {
        format!(
            "p090_junie_preflight_failed: permission denied writing runtime temp {}",
            temp_probe.display()
        )
    })?;
    let _ = std::fs::remove_file(&temp_probe);
    Ok(())
}

fn junie_runtime_cache_dir(req: &ExecutionRequest) -> Option<PathBuf> {
    req.chainworks_meta_root
        .as_deref()
        .map(PathBuf::from)
        .map(|root| root.join("acp-runtime/junie/cache"))
}

fn remediate_junie_runtime_cache(req: &ExecutionRequest) -> Result<()> {
    let Some(runtime_cache) = junie_runtime_cache_dir(req) else {
        bail!("missing_junie_runtime_cache: no CHAINWORKS_META_ROOT configured for Junie cache");
    };
    std::fs::create_dir_all(&runtime_cache).with_context(|| {
        format!(
            "p090_junie_preflight_failed: create Junie runtime cache {}",
            runtime_cache.display()
        )
    })?;
    Ok(())
}

fn junie_project_read_probe_path(execution_root: &Path) -> Option<PathBuf> {
    ["AGENTS.md", "CLAUDE.md", "Cargo.toml", "Package.swift"]
        .into_iter()
        .map(|name| execution_root.join(name))
        .find(|path| path.is_file())
        .or_else(|| {
            std::fs::read_dir(execution_root)
                .ok()?
                .flatten()
                .map(|entry| entry.path())
                .find(|path| path.is_file())
        })
}

fn junie_preflight_output_paths(req: &ExecutionRequest) -> Vec<PathBuf> {
    let typed = req
        .expected_outputs
        .iter()
        .filter(|spec| spec.required)
        .map(|spec| PathBuf::from(&spec.target_path));
    let legacy = req.expected_output_paths.iter().map(PathBuf::from);
    typed.chain(legacy).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::agent::AgentStatus;
    use domain::ids::RunId;
    use std::sync::{Mutex, OnceLock};

    fn preflight_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

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

    #[test]
    fn junie_supports_brokered_xcode_http_mcp_probe_path() {
        let adapter = JunieAdapter::new_with_binary("/bin/junie-fixture");

        assert!(
            adapter.supports_http_mcp_capability_probe(),
            "Junie must enter the brokered Xcode MCP capability path instead of failing before probe"
        );
    }

    #[test]
    fn junie_requests_permission_grant_debounce_in_session_config() {
        let adapter = JunieAdapter::new_with_binary("/bin/junie-fixture");

        let spec = adapter
            .prepare_session_new_spec(&request())
            .expect("junie session/new spec");

        assert_eq!(spec.permission_grant_debounce, Duration::from_millis(25));
    }

    #[test]
    fn proposal_090_tool_path_preflight_checks_project_and_output_write_paths_before_launch() {
        let _guard = preflight_env_lock();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("AGENTS.md"), "project proof").unwrap();
        let output_path = temp
            .path()
            .join(".chainworks/runs/run-1/implementation/progress.md");
        let mut req = request();
        req.workspace_root = temp.path().to_string_lossy().into_owned();
        req.expected_output_paths = vec![output_path.to_string_lossy().into_owned()];

        run_junie_p090_tool_path_preflight(&req, None).expect("preflight should pass");

        assert!(
            output_path.parent().unwrap().is_dir(),
            "preflight should prove and prepare output parent writability"
        );
        assert!(
            !output_path
                .parent()
                .unwrap()
                .join(format!(".chainworks-p090-preflight-{}", std::process::id()))
                .exists(),
            "preflight probe file should be cleaned up"
        );
    }

    #[test]
    fn proposal_090_tool_path_preflight_fails_before_launch_for_missing_project_root() {
        let _guard = preflight_env_lock();
        let temp = tempfile::tempdir().expect("tempdir");
        let mut req = request();
        req.workspace_root = temp.path().join("missing").to_string_lossy().into_owned();

        let error = run_junie_p090_tool_path_preflight(&req, None)
            .expect_err("missing project root should fail preflight");

        assert!(error
            .to_string()
            .contains("missing_provider_execution_root"));
    }

    #[test]
    fn proposal_090_tool_path_preflight_remediates_wrong_cwd_once_before_launch() {
        let _guard = preflight_env_lock();
        let previous = std::env::var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE").ok();
        std::env::set_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE", "1");
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("AGENTS.md"), "project proof").unwrap();
        let mut req = request();
        req.workspace_root = temp.path().to_string_lossy().into_owned();
        req.worktree_write_enabled = true;
        req.worktree_root = Some(
            temp.path()
                .join("missing-worktree")
                .to_string_lossy()
                .into_owned(),
        );
        req.expected_output_paths = vec![temp
            .path()
            .join(".chainworks/runs/run-1/implementation/progress.md")
            .to_string_lossy()
            .into_owned()];
        let adapter = JunieAdapter::new_with_binary("/bin/junie-fixture");
        let mut launch_spec = AcpLaunchSpec::new("/bin/junie-fixture");

        adapter
            .preflight_launch(&req, &mut launch_spec)
            .expect("wrong-cwd preflight should fall back to workspace root");

        assert_eq!(
            launch_spec.current_dir_override.as_deref(),
            Some(temp.path()),
            "P090 remediation must happen before subprocess launch by changing cwd"
        );
        let preflight: serde_json::Value = serde_json::from_str(
            launch_spec
                .runtime_tool_path_preflight_json
                .as_deref()
                .expect("preflight lifecycle JSON must be recorded"),
        )
        .unwrap();
        assert_eq!(preflight["status"], "passed");
        assert_eq!(preflight["attempt_count"], 2);
        assert_eq!(preflight["remediation_applied"], "wrong_cwd_workspace_root");
        assert_eq!(
            preflight["lifecycle_phases"],
            serde_json::json!(["preflight_running", "preflight_remediating", "passed"])
        );
        if let Some(previous) = previous {
            std::env::set_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE", previous);
        } else {
            std::env::remove_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE");
        }
    }

    #[test]
    fn proposal_090_tool_path_preflight_runs_in_diagnostic_mode_when_enforce_is_off() {
        let _guard = preflight_env_lock();
        let previous = std::env::var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE").ok();
        std::env::set_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE", "0");
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("AGENTS.md"), "project proof").unwrap();
        let mut req = request();
        req.workspace_root = temp.path().to_string_lossy().into_owned();
        req.expected_output_paths = vec![temp
            .path()
            .join(".chainworks/runs/run-1/implementation/progress.md")
            .to_string_lossy()
            .into_owned()];
        let adapter = JunieAdapter::new_with_binary("/bin/junie-fixture");
        let mut launch_spec = AcpLaunchSpec::new("/bin/junie-fixture");

        adapter
            .preflight_launch(&req, &mut launch_spec)
            .expect("diagnostic preflight should not block a valid launch");

        let preflight: serde_json::Value = serde_json::from_str(
            launch_spec
                .runtime_tool_path_preflight_json
                .as_deref()
                .expect("diagnostic mode must still record real preflight facts"),
        )
        .unwrap();
        assert_eq!(preflight["status"], "passed");
        assert_eq!(preflight["attempt_count"], 1);
        assert_eq!(preflight["enforcement_enabled"], false);
        if let Some(previous) = previous {
            std::env::set_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE", previous);
        } else {
            std::env::remove_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE");
        }
    }

    #[test]
    fn proposal_090_tool_path_preflight_remediates_missing_runtime_cache_once() {
        let _guard = preflight_env_lock();
        let previous = std::env::var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE").ok();
        std::env::set_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE", "1");
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("AGENTS.md"), "project proof").unwrap();
        let meta_root = temp.path().join(".chainworks/runs/run-1");
        let mut req = request();
        req.workspace_root = temp.path().to_string_lossy().into_owned();
        req.chainworks_meta_root = Some(meta_root.to_string_lossy().into_owned());
        req.expected_output_paths = vec![meta_root
            .join("implementation/progress.md")
            .to_string_lossy()
            .into_owned()];
        let adapter = JunieAdapter::new_with_binary("/bin/junie-fixture");
        let mut launch_spec = AcpLaunchSpec::new("/bin/junie-fixture");

        adapter
            .preflight_launch(&req, &mut launch_spec)
            .expect("missing runtime cache should be rebuilt once before launch");

        let runtime_cache = meta_root.join("acp-runtime/junie/cache");
        assert!(
            runtime_cache.is_dir(),
            "runtime-home/cache remediation must create the Junie cache directory"
        );
        let preflight: serde_json::Value = serde_json::from_str(
            launch_spec
                .runtime_tool_path_preflight_json
                .as_deref()
                .expect("runtime cache remediation must be recorded"),
        )
        .unwrap();
        assert_eq!(preflight["status"], "passed");
        assert_eq!(preflight["attempt_count"], 2);
        assert_eq!(
            preflight["remediation_applied"],
            "runtime_home_cache_rebuilt"
        );
        assert_eq!(
            preflight["lifecycle_phases"],
            serde_json::json!(["preflight_running", "preflight_remediating", "passed"])
        );
        if let Some(previous) = previous {
            std::env::set_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE", previous);
        } else {
            std::env::remove_var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE");
        }
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
