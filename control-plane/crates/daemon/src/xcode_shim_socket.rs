use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use acp::{
    handle_xcode_shim_unix_stream_with_grant_resolver, XcodeHostExecutorProcessConfig,
    XcodeRuntimeObservationSink, XcodeShimDispatchOutcome, XcodeShimGrantRecord,
    XcodeShimGrantResolver, XcodeShimGrantStore, XcodeShimProcessInspector,
    XcodeShimResolvedDispatch,
};
use anyhow::Context;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
#[cfg(unix)]
use tracing::warn;

#[derive(Default)]
pub struct XcodeShimGrantRegistry {
    grants: RwLock<HashMap<String, XcodeShimGrantRecord>>,
}

impl XcodeShimGrantRegistry {
    pub fn insert(&self, record: XcodeShimGrantRecord) {
        self.grants
            .write()
            .expect("xcode shim grant registry lock poisoned")
            .insert(record.grant.token_id.clone(), record);
    }

    pub fn remove(&self, token_id: &str) -> Option<XcodeShimGrantRecord> {
        self.grants
            .write()
            .expect("xcode shim grant registry lock poisoned")
            .remove(token_id)
    }

    pub fn set_active_prompt(&self, token_id: &str, active_prompt: bool) -> bool {
        let mut grants = self
            .grants
            .write()
            .expect("xcode shim grant registry lock poisoned");
        let Some(record) = grants.get_mut(token_id) else {
            return false;
        };
        record.active_prompt = active_prompt;
        true
    }

    pub fn socket_path(app_support_dir: &Path) -> PathBuf {
        app_support_dir.join("xcode-shim.sock")
    }

    pub fn shim_dir(app_support_dir: &Path) -> PathBuf {
        app_support_dir.join("xcode-shims")
    }
}

impl XcodeShimGrantStore for XcodeShimGrantRegistry {
    fn insert_xcode_shim_grant(&self, record: XcodeShimGrantRecord) {
        self.insert(record);
    }

    fn set_xcode_shim_grant_active_prompt(&self, token_id: &str, active_prompt: bool) -> bool {
        self.set_active_prompt(token_id, active_prompt)
    }

    fn remove_xcode_shim_grant(&self, token_id: &str) -> Option<XcodeShimGrantRecord> {
        self.remove(token_id)
    }
}

#[cfg(unix)]
pub fn ensure_xcode_shim_dir(app_support_dir: &Path) -> anyhow::Result<PathBuf> {
    let shim_dir = XcodeShimGrantRegistry::shim_dir(app_support_dir);
    std::fs::create_dir_all(&shim_dir)
        .with_context(|| format!("create xcode shim dir {}", shim_dir.display()))?;
    for tool in ["xcodebuild", "simctl", "mcpbridge", "xcrun"] {
        let path = shim_dir.join(tool);
        std::fs::write(&path, shim_script(tool))
            .with_context(|| format!("write xcode shim executable {}", path.display()))?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .with_context(|| format!("chmod xcode shim executable {}", path.display()))?;
    }
    Ok(shim_dir)
}

#[cfg(unix)]
fn shim_script(tool: &str) -> String {
    format!(
        r#"#!/usr/bin/env python3
import json, os, socket, sys, time

tool = {tool:?}
socket_path = os.environ.get("CHAINWORKS_XCODE_SHIM_SOCKET")
token_id = os.environ.get("CHAINWORKS_XCODE_SHIM_TOKEN_ID")
token_secret = os.environ.get("CHAINWORKS_XCODE_SHIM_TOKEN")
workspace_root = os.environ.get("CHAINWORKS_XCODE_SHIM_WORKSPACE_ROOT") or os.getcwd()
if not socket_path or not token_id or not token_secret:
    sys.stderr.write("p051_xcode_shim_credentials_missing\n")
    sys.exit(126)
request = {{
    "agent_execution_id": os.environ.get("CHAINWORKS_XCODE_SHIM_AGENT_EXECUTION_ID"),
    "token_id": token_id,
    "token_secret": token_secret,
    "now_epoch_ms": int(time.time() * 1000),
    "plan_input": {{
        "invoked_tool": tool,
        "args": sys.argv[1:],
        "cwd": os.getcwd(),
        "workspace_root": workspace_root,
        "provider_env": dict(os.environ),
        "simulator_candidates": [],
    }},
}}
try:
    client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    client.connect(socket_path)
    client.sendall((json.dumps(request) + "\n").encode("utf-8"))
    chunks = []
    while True:
        chunk = client.recv(65536)
        if not chunk:
            break
        chunks.append(chunk)
        if b"\n" in chunk:
            break
    response = json.loads(b"".join(chunks).decode("utf-8").splitlines()[0])
except Exception as exc:
    sys.stderr.write("p051_xcode_shim_dispatch_failed: %s\n" % exc)
    sys.exit(126)
process_output = response.get("process_output") or {{}}
if process_output.get("stdout"):
    sys.stdout.write(process_output["stdout"])
if process_output.get("stderr"):
    sys.stderr.write(process_output["stderr"])
reason = response.get("reason_code")
if reason:
    sys.stderr.write(reason + "\n")
sys.exit(int(response.get("exit_status", 126)))
"#
    )
}

#[async_trait::async_trait]
impl XcodeShimGrantResolver for XcodeShimGrantRegistry {
    async fn resolve_grant(&self, token_id: &str) -> anyhow::Result<XcodeShimResolvedDispatch> {
        let record = self
            .grants
            .read()
            .expect("xcode shim grant registry lock poisoned")
            .get(token_id)
            .cloned()
            .with_context(|| format!("xcode_shim_unknown_token_id: {token_id}"))?;
        Ok(XcodeShimResolvedDispatch {
            grant: record.grant,
            active_prompt: record.active_prompt,
        })
    }
}

#[cfg(unix)]
pub fn bind_xcode_shim_listener(socket_path: &Path) -> anyhow::Result<UnixListener> {
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create xcode shim socket parent {}", parent.display()))?;
    }
    match std::fs::remove_file(socket_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!("remove stale xcode shim socket {}", socket_path.display())
            })
        }
    }
    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("bind xcode shim socket {}", socket_path.display()))?;
    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("chmod xcode shim socket {}", socket_path.display()))?;
    Ok(listener)
}

#[cfg(unix)]
pub fn cleanup_xcode_shim_socket(socket_path: &Path) -> anyhow::Result<()> {
    match std::fs::remove_file(socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("remove xcode shim socket {}", socket_path.display())),
    }
}

#[cfg(unix)]
pub async fn handle_xcode_shim_connection(
    stream: UnixStream,
    grant_resolver: &dyn XcodeShimGrantResolver,
    process_config: &XcodeHostExecutorProcessConfig,
    observation_sink: &dyn XcodeRuntimeObservationSink,
    process_inspector: &dyn XcodeShimProcessInspector,
) -> anyhow::Result<XcodeShimDispatchOutcome> {
    handle_xcode_shim_unix_stream_with_grant_resolver(
        stream,
        process_config,
        observation_sink,
        process_inspector,
        grant_resolver,
    )
    .await
}

#[cfg(unix)]
pub fn spawn_xcode_shim_socket_service(
    listener: UnixListener,
    grant_resolver: std::sync::Arc<dyn XcodeShimGrantResolver>,
    process_config: std::sync::Arc<XcodeHostExecutorProcessConfig>,
    observation_sink: std::sync::Arc<dyn XcodeRuntimeObservationSink>,
    process_inspector: std::sync::Arc<dyn XcodeShimProcessInspector>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let (stream, _) = match listener.accept().await {
                Ok(parts) => parts,
                Err(error) => {
                    warn!(error = %error, "xcode shim socket accept failed");
                    continue;
                }
            };
            let grant_resolver = grant_resolver.clone();
            let process_config = process_config.clone();
            let observation_sink = observation_sink.clone();
            let process_inspector = process_inspector.clone();
            tokio::spawn(async move {
                if let Err(error) = handle_xcode_shim_connection(
                    stream,
                    &*grant_resolver,
                    &process_config,
                    &*observation_sink,
                    &*process_inspector,
                )
                .await
                {
                    warn!(error = %error, "xcode shim socket request failed");
                }
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use acp::{
        current_process_uid, xcode_shim_peer_credentials, DefaultXcodeShimProcessInspector,
        XcodeShimDispatchGrant, XcodeShimPeerCredentials, XcodeShimProcessBinding,
        XcodeShimSocketDispatchRequest,
    };
    use async_trait::async_trait;
    use domain::ids::AgentExecutionId;
    use domain::xcode_runtime::XcodeRuntimeObservationUpdate;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[derive(Default)]
    struct CapturingObservationSink {
        updates: Mutex<Vec<XcodeRuntimeObservationUpdate>>,
    }

    #[async_trait]
    impl XcodeRuntimeObservationSink for CapturingObservationSink {
        async fn append_xcode_runtime_observation(
            &self,
            _agent_execution_id: AgentExecutionId,
            update: XcodeRuntimeObservationUpdate,
        ) -> anyhow::Result<()> {
            self.updates.lock().expect("updates poisoned").push(update);
            Ok(())
        }
    }

    struct StaticPeerInspector {
        expected_credentials: XcodeShimPeerCredentials,
        peer_process: XcodeShimProcessBinding,
    }

    impl XcodeShimProcessInspector for StaticPeerInspector {
        fn inspect_peer(
            &self,
            credentials: XcodeShimPeerCredentials,
        ) -> anyhow::Result<XcodeShimProcessBinding> {
            assert_eq!(credentials, self.expected_credentials);
            Ok(self.peer_process.clone())
        }
    }

    fn provider_process(pid: u32, uid: u32) -> XcodeShimProcessBinding {
        XcodeShimProcessBinding {
            pid,
            uid,
            parent_pid: Some(7),
            ancestor_pids: Vec::new(),
            start_time_fingerprint: Some("started-at-123".to_string()),
            executable_fingerprint: Some("provider-sha256".to_string()),
        }
    }

    fn host_input() -> acp::XcodeHostExecutorPlanInput {
        acp::XcodeHostExecutorPlanInput {
            invoked_tool: "xcodebuild".to_string(),
            args: vec!["-scheme".to_string(), "Chainworks Forge".to_string()],
            cwd: ".".to_string(),
            workspace_root: "/tmp".to_string(),
            provider_env: BTreeMap::new(),
            simulator_candidates: Vec::new(),
            toolchain_mapping_root: None,
        }
    }

    fn process_config(tool_path: &str) -> XcodeHostExecutorProcessConfig {
        XcodeHostExecutorProcessConfig {
            tool_paths: BTreeMap::from([("xcodebuild".to_string(), tool_path.to_string())]),
            timeout: Duration::from_secs(5),
        }
    }

    fn short_tempdir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("cw-xc-")
            .tempdir_in("/tmp")
            .expect("short tempdir")
    }

    #[tokio::test]
    async fn daemon_handler_uses_registry_active_prompt_truth() {
        let registry = Arc::new(XcodeShimGrantRegistry::default());
        let sink = Arc::new(CapturingObservationSink::default());
        let request = XcodeShimSocketDispatchRequest {
            agent_execution_id: Some(AgentExecutionId::new()),
            token_id: "token-live".to_string(),
            token_secret: "secret-live".to_string(),
            now_epoch_ms: 1_500,
            active_prompt: true,
            plan_input: host_input(),
        };

        let (client, server) = UnixStream::pair().expect("unix stream pair");
        let credentials = xcode_shim_peer_credentials(&server).expect("peer credentials");
        let peer_process = provider_process(credentials.pid, credentials.uid);
        registry.insert(XcodeShimGrantRecord {
            grant: XcodeShimDispatchGrant::new(
                "token-live",
                "secret-live",
                "lease-live",
                peer_process.clone(),
                1_000,
                2_000,
            ),
            active_prompt: false,
        });

        let handler_sink = sink.clone();
        let handler_registry = registry.clone();
        let handler = tokio::spawn(async move {
            let inspector = StaticPeerInspector {
                expected_credentials: credentials,
                peer_process,
            };
            handle_xcode_shim_connection(
                server,
                &*handler_registry,
                &process_config("/bin/sh"),
                &*handler_sink,
                &inspector,
            )
            .await
        });

        let (client_reader, mut client_writer) = client.into_split();
        client_writer
            .write_all(serde_json::to_string(&request).expect("payload").as_bytes())
            .await
            .expect("write payload");
        client_writer.write_all(b"\n").await.expect("write newline");
        client_writer
            .shutdown()
            .await
            .expect("shutdown client write");

        let mut response_line = String::new();
        let mut reader = BufReader::new(client_reader);
        reader
            .read_line(&mut response_line)
            .await
            .expect("read response");
        let response: XcodeShimDispatchOutcome =
            serde_json::from_str(&response_line).expect("response json");
        let handler_outcome = handler
            .await
            .expect("handler task")
            .expect("handler result");

        assert_eq!(response, handler_outcome);
        assert!(!response.authorization.allowed);
        assert_eq!(
            response.authorization.reason_code.as_deref(),
            Some("p051_shim_no_active_prompt")
        );
        assert_eq!(
            registry.remove("token-live").unwrap().grant.token_id,
            "token-live"
        );
    }

    #[tokio::test]
    async fn bind_listener_replaces_stale_socket_path() {
        let tempdir = short_tempdir();
        let socket_path = XcodeShimGrantRegistry::socket_path(tempdir.path());
        std::fs::write(&socket_path, "stale").expect("write stale placeholder");

        let listener = bind_xcode_shim_listener(&socket_path).expect("bind listener");

        assert!(socket_path.exists());
        drop(listener);
    }

    #[tokio::test]
    async fn cleanup_removes_socket_path() {
        let tempdir = short_tempdir();
        let socket_path = XcodeShimGrantRegistry::socket_path(tempdir.path());
        let listener = bind_xcode_shim_listener(&socket_path).expect("bind listener");

        assert!(socket_path.exists());

        drop(listener);
        cleanup_xcode_shim_socket(&socket_path).expect("cleanup socket");

        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn generated_shim_dispatches_through_socket_with_descendant_grant() {
        let tempdir = short_tempdir();
        let shim_dir = ensure_xcode_shim_dir(tempdir.path()).expect("shim dir");
        let socket_path = XcodeShimGrantRegistry::socket_path(tempdir.path());
        let listener = bind_xcode_shim_listener(&socket_path).expect("bind listener");
        let registry = Arc::new(XcodeShimGrantRegistry::default());
        let sink = Arc::new(CapturingObservationSink::default());
        let token_id = "token-script";
        let token_secret = "secret-script";
        let now_epoch_ms = chrono::Utc::now().timestamp_millis();
        registry.insert(XcodeShimGrantRecord {
            grant: XcodeShimDispatchGrant::new(
                token_id,
                token_secret,
                "lease-script",
                XcodeShimProcessBinding {
                    pid: std::process::id(),
                    uid: current_process_uid(),
                    parent_pid: None,
                    ancestor_pids: Vec::new(),
                    start_time_fingerprint: None,
                    executable_fingerprint: None,
                },
                now_epoch_ms - 1_000,
                now_epoch_ms + 60_000,
            ),
            active_prompt: true,
        });
        let service = spawn_xcode_shim_socket_service(
            listener,
            registry,
            Arc::new(XcodeHostExecutorProcessConfig {
                tool_paths: BTreeMap::new(),
                timeout: Duration::from_secs(5),
            }),
            sink,
            Arc::new(DefaultXcodeShimProcessInspector),
        );

        let output = tokio::process::Command::new(shim_dir.join("mcpbridge"))
            .current_dir(tempdir.path())
            .env("CHAINWORKS_XCODE_SHIM_SOCKET", &socket_path)
            .env("CHAINWORKS_XCODE_SHIM_TOKEN_ID", token_id)
            .env("CHAINWORKS_XCODE_SHIM_TOKEN", token_secret)
            .env("CHAINWORKS_XCODE_SHIM_WORKSPACE_ROOT", tempdir.path())
            .output()
            .await
            .expect("run generated shim");
        service.abort();

        assert_eq!(output.status.code(), Some(126));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("p051_shim_mcpbridge_broker_only"));
        assert!(!stderr.contains("p051_shim_peer_pid_mismatch"));
        assert!(!stderr.contains("p051_shim_no_active_prompt"));
    }
}
