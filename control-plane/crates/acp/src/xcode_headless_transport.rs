//! A dedicated, non-retrying stdio bridge for one prepared headless workspace.

use std::{collections::BTreeSet, process::Stdio, time::Duration};

use anyhow::{bail, ensure, Context, Result};
use domain::xcode_contract as contract;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::{timeout, timeout_at, Instant},
};

use crate::{
    xcode_headless::{HeadlessPeer, HeadlessPeerFactory},
    xcode_headless_host::HostSnapshot,
};

pub struct ProcessHeadlessPeerFactory;

#[async_trait::async_trait]
impl HeadlessPeerFactory for ProcessHeadlessPeerFactory {
    async fn connect(
        &self,
        host: &HostSnapshot,
        deadline: Instant,
    ) -> Result<Box<dyn HeadlessPeer>> {
        ensure!(cfg!(target_os = "macos"), "headless_host_unsupported");
        let bridge = host
            .generation
            .developer_dir
            .join("usr/bin/mcpbridge")
            .canonicalize()
            .context("headless_bridge_missing")?;
        ensure!(
            bridge.starts_with(&host.generation.developer_dir),
            "headless_bridge_installation_mismatch"
        );
        let mut command = Command::new(bridge);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", &host.operator_home)
            .env("USER", &host.account_name)
            .env("LOGNAME", &host.account_name)
            .env("TMPDIR", &host.darwin_tmpdir)
            .env("DEVELOPER_DIR", &host.generation.developer_dir)
            .env("MCP_XCODE_PID", host.generation.pid.to_string())
            .current_dir("/");
        Ok(Box::new(ProcessPeer::spawn(command, deadline)?))
    }
}

const RESPONSE_BUDGET: usize = 8 * contract::MAX_JSON_BYTES;

struct ProcessPeer {
    child: Option<Child>,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    stopped: bool,
    initialized: bool,
    deadline: Instant,
}

impl ProcessPeer {
    fn spawn(mut command: Command, deadline: Instant) -> Result<Self> {
        ensure!(Instant::now() < deadline, "headless_bridge_timeout");
        crate::transport::isolate_process_group(&mut command);
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("headless_bridge_spawn_failed")?;
        let stdin = child
            .stdin
            .take()
            .context("headless_bridge_stdin_missing")?;
        let stdout = BufReader::new(
            child
                .stdout
                .take()
                .context("headless_bridge_stdout_missing")?,
        );
        Ok(Self {
            child: Some(child),
            stdin,
            stdout,
            next_id: 1,
            stopped: false,
            initialized: false,
            deadline,
        })
    }

    async fn exchange(&mut self, method: &str, params: Value) -> Result<Value> {
        ensure!(!self.stopped, "headless_transport_stopped");
        ensure!(Instant::now() < self.deadline, "headless_bridge_timeout");
        ensure!(
            matches!(method, "initialize" | "tools/list" | "tools/call"),
            "headless_method_denied"
        );
        let id = self.next_id;
        ensure!(
            id < contract::MAX_SAFE_INTEGER,
            "headless_request_id_exhausted"
        );
        self.next_id += 1;
        let mut bytes =
            serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        ensure!(
            bytes.len() < contract::MAX_JSON_BYTES,
            "headless_request_size"
        );
        bytes.push(b'\n');
        // An interrupted write/read cannot later reuse this stream or its IDs.
        self.stopped = true;
        let result = timeout_at(self.deadline, async {
            self.stdin
                .write_all(&bytes)
                .await
                .context("headless_bridge_write_failed")?;
            let mut received = 0;
            for _ in 0..=64 {
                let line = bounded_line(&mut self.stdout).await?;
                received += line.len();
                ensure!(received <= RESPONSE_BUDGET, "headless_response_budget");
                if let Some(response) = decode_response(&line, id)? {
                    return Ok(response);
                }
            }
            bail!("headless_notification_limit")
        })
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("headless_bridge_timeout")));
        if result.is_ok() {
            self.stopped = false;
        } else {
            self.stop().await;
        }
        result
    }

    async fn stop(&mut self) {
        self.stopped = true;
        if let Some(mut child) = self.child.take() {
            kill_bridge(&mut child);
            let _ = timeout(Duration::from_secs(5), child.wait()).await;
        }
    }
}

#[async_trait::async_trait]
impl HeadlessPeer for ProcessPeer {
    async fn initialize(&mut self) -> Result<()> {
        ensure!(
            !self.initialized && !self.stopped,
            "headless_initialize_state"
        );
        let result = self
            .exchange(
                "initialize",
                json!({
                    "protocolVersion":"2025-06-18", "capabilities":{},
                    "clientInfo":{"name":"chainworks-headless","version":"1"}
                }),
            )
            .await?;
        contract::validate_initialize(&result)?;
        self.stopped = true;
        timeout_at(
            self.deadline,
            self.stdin
                .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n"),
        )
        .await
        .map_err(|_| anyhow::anyhow!("headless_bridge_timeout"))??;
        self.stopped = false;
        let mut tools = Vec::new();
        let mut cursor = None;
        let mut seen = BTreeSet::new();
        for _ in 0..16 {
            let params = cursor
                .as_ref()
                .map_or(json!({}), |cursor| json!({"cursor":cursor}));
            let page = self.exchange("tools/list", params).await?;
            let object = page.as_object().context("headless_tool_list_shape")?;
            ensure!(
                object
                    .keys()
                    .all(|key| matches!(key.as_str(), "tools" | "nextCursor")),
                "headless_tool_list_shape"
            );
            let items = page["tools"]
                .as_array()
                .context("headless_tool_list_shape")?;
            ensure!(tools.len() + items.len() <= 256, "headless_tool_list_size");
            tools.extend(items.iter().cloned());
            if let Some(next) = page.get("nextCursor") {
                let next = next
                    .as_str()
                    .filter(|value| {
                        !value.is_empty()
                            && value.len() <= 1024
                            && !value.chars().any(char::is_control)
                    })
                    .context("headless_cursor_shape")?;
                ensure!(seen.insert(next.to_owned()), "headless_cursor_cycle");
                cursor = Some(next.to_owned());
            } else {
                contract::validate_upstream_tools(&tools)?;
                self.initialized = true;
                return Ok(());
            }
        }
        bail!("headless_tool_list_pages")
    }

    async fn open(&mut self, project_path: &str) -> Result<Value> {
        ensure!(self.initialized, "headless_contract_unverified");
        let arguments = json!({"path":project_path});
        contract::validate_open_arguments(&arguments)?;
        self.exchange(
            "tools/call",
            json!({"name":"XcodeOpenWorkspace","arguments":arguments}),
        )
        .await
    }

    async fn read(&mut self, arguments: Value) -> Result<Value> {
        ensure!(self.initialized, "headless_contract_unverified");
        self.exchange(
            "tools/call",
            json!({"name":"XcodeRead","arguments":arguments}),
        )
        .await
    }
}

fn kill_bridge(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        crate::transport::signal_process_group(pid, libc::SIGKILL);
    }
    let _ = child.start_kill();
}

impl Drop for ProcessPeer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            kill_bridge(&mut child);
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = timeout(Duration::from_secs(5), child.wait()).await;
                });
            }
        }
    }
}

async fn bounded_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let bytes = reader
            .fill_buf()
            .await
            .context("headless_bridge_read_failed")?;
        ensure!(!bytes.is_empty(), "headless_bridge_eof");
        let count = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| index + 1);
        ensure!(
            line.len() + count <= contract::MAX_JSON_BYTES,
            "headless_line_size"
        );
        let complete = bytes[count - 1] == b'\n';
        line.extend_from_slice(&bytes[..count]);
        reader.consume(count);
        if complete {
            return Ok(line);
        }
    }
}

fn decode_response(bytes: &[u8], id: u64) -> Result<Option<Value>> {
    let value = contract::parse_unique_json(bytes)?;
    let object = value.as_object().context("headless_rpc_envelope")?;
    ensure!(value["jsonrpc"] == "2.0", "headless_rpc_envelope");
    if let Some(method) = value.get("method").and_then(Value::as_str) {
        ensure!(
            object
                .keys()
                .all(|key| matches!(key.as_str(), "jsonrpc" | "method" | "params"))
                && matches!(method, "notifications/message" | "notifications/progress")
                && object.get("params").is_none_or(Value::is_object),
            "headless_rpc_notification"
        );
        return Ok(None);
    }
    ensure!(
        object
            .keys()
            .all(|key| matches!(key.as_str(), "jsonrpc" | "id" | "result" | "error"))
            && value["id"].as_u64() == Some(id)
            && (object.contains_key("result") ^ object.contains_key("error")),
        "headless_rpc_envelope"
    );
    ensure!(!object.contains_key("error"), "headless_upstream_rpc_error");
    Ok(Some(value["result"].clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn response_requires_unique_keys_exact_id_and_exclusive_result() {
        let valid = br#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
        assert_eq!(decode_response(valid, 1).unwrap(), Some(json!({"ok":true})));
        for value in [
            r#"{"jsonrpc":"2.0","id":2,"result":{}}"#,
            r#"{"jsonrpc":"2.0","id":null,"result":{}}"#,
            r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{}}"#,
            r#"{"jsonrpc":"2.0","id":1,"result":{"a":1,"a":2}}"#,
            r#"{"jsonrpc":"2.0","id":1,"result":{},"result":{}}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","result":{}}"#,
            r#"[{"jsonrpc":"2.0","id":1,"result":{}}]"#,
        ] {
            assert!(decode_response(value.as_bytes(), 1).is_err(), "{value}");
        }
    }

    #[test]
    fn only_known_bounded_notifications_are_accepted_without_result() {
        assert_eq!(
            decode_response(
                br#"{"jsonrpc":"2.0","method":"notifications/progress","params":{}}"#,
                1
            )
            .unwrap(),
            None
        );
        for method in [
            "tools/call",
            "notifications/tools/list_changed",
            "notifications/unknown",
        ] {
            let value = json!({"jsonrpc":"2.0","method":method,"params":{}});
            assert!(decode_response(&serde_json::to_vec(&value).unwrap(), 1).is_err());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_response_is_correlated_and_an_error_permanently_stops_transport() {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("IFS= read -r line; printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ready\":true}}'; IFS= read -r line; printf '%s\\n' '{\"jsonrpc\":\"2.0\",\"id\":999,\"result\":{}}'");
        let mut peer =
            ProcessPeer::spawn(command, Instant::now() + Duration::from_secs(120)).unwrap();
        assert_eq!(
            peer.exchange("initialize", json!({})).await.unwrap(),
            json!({"ready":true})
        );
        assert!(peer.exchange("tools/list", json!({})).await.is_err());
        assert!(peer.exchange("tools/list", json!({})).await.is_err());
    }

    #[test]
    fn backend_errors_are_static_and_never_include_raw_payloads() {
        let value = br#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"secret filesystem /private/example"}}"#;
        let error = decode_response(value, 1).unwrap_err().to_string();
        assert!(!error.contains("secret"));
        assert!(!error.contains("/private/example"));
    }

    #[tokio::test]
    async fn framing_bounds_unterminated_lines_and_rejects_eof() {
        let mut valid = std::io::Cursor::new(b"first\nsecond\n");
        assert_eq!(bounded_line(&mut valid).await.unwrap(), b"first\n");
        assert_eq!(bounded_line(&mut valid).await.unwrap(), b"second\n");
        assert!(bounded_line(&mut valid).await.is_err());
        let mut too_large = std::io::Cursor::new(vec![b'x'; contract::MAX_JSON_BYTES + 1]);
        assert!(bounded_line(&mut too_large).await.is_err());
        let mut partial = std::io::Cursor::new(b"partial");
        assert!(bounded_line(&mut partial).await.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn handshake_checks_pinned_tools_before_one_explicit_open_and_read() {
        let capture: Value = serde_json::from_str(include_str!(
            "../../../contracts/xcode-headless/v1/upstream-capture.json"
        ))
        .unwrap();
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg(r#"
IFS= read -r line
printf '%s\n' "$1"
IFS= read -r line
case "$line" in *notifications/initialized*) ;; *) exit 11 ;; esac
IFS= read -r line
printf '%s\n' "$2"
IFS= read -r line
case "$line" in *XcodeOpenWorkspace*) ;; *) exit 12 ;; esac
printf '%s\n' "$3"
IFS= read -r line
case "$line" in *workspace-bound*) ;; *) exit 13 ;; esac
printf '%s\n' "$4"
"#).arg("fixture")
            .arg(json!({"jsonrpc":"2.0","id":1,"result":{
                "protocolVersion":"2025-06-18","capabilities":{"tools":{}},
                "serverInfo":{"name":"xcode-tools","version":"25317"}
            }}).to_string())
            .arg(json!({"jsonrpc":"2.0","id":2,"result":{"tools":capture["tools"]}}).to_string())
            .arg(json!({"jsonrpc":"2.0","id":3,"result":{"structuredContent":{
                "workspaceIdentifier":"workspace-bound","workspacePath":"/fixture/A.xcodeproj"
            }}}).to_string())
            .arg(json!({"jsonrpc":"2.0","id":4,"result":{"structuredContent":{"content":"1\tknown"}}}).to_string());
        let mut peer =
            ProcessPeer::spawn(command, Instant::now() + Duration::from_secs(120)).unwrap();
        assert!(peer.open("/fixture/A.xcodeproj").await.is_err());
        peer.initialize().await.unwrap();
        assert_eq!(
            peer.open("/fixture/A.xcodeproj").await.unwrap()["structuredContent"]
                ["workspaceIdentifier"],
            "workspace-bound"
        );
        assert_eq!(
            peer.read(json!({"workspaceIdentifier":"workspace-bound","filePath":"A/Sentinel.txt"}))
                .await
                .unwrap()["structuredContent"]["content"],
            "1\tknown"
        );
        peer.stop().await;
        assert!(peer.child.is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_mid_response_poisoned_stream_never_sends_another_call() {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("IFS= read -r line; sleep 20");
        let mut peer =
            ProcessPeer::spawn(command, Instant::now() + Duration::from_secs(120)).unwrap();
        let result = timeout(
            Duration::from_millis(25),
            peer.exchange("initialize", json!({})),
        )
        .await;
        assert!(result.is_err());
        assert!(peer.stopped);
        assert!(peer.exchange("initialize", json!({})).await.is_err());
        peer.stop().await;
        assert!(peer.child.is_none());
    }

    #[cfg(unix)]
    #[tokio::test(start_paused = true)]
    async fn open_can_wait_beyond_sixty_seconds_within_its_inherited_deadline() {
        let gate = tempfile::tempdir().unwrap();
        let release = gate.path().join("release");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(
                r#"
IFS= read -r line
while [ ! -f "$1" ]; do sleep 0.01; done
printf '%s\n' '{"jsonrpc":"2.0","id":1,"result":{"ok":true}}'
"#,
            )
            .arg("fixture")
            .arg(&release);
        let mut peer =
            ProcessPeer::spawn(command, Instant::now() + Duration::from_secs(120)).unwrap();
        peer.initialized = true;
        let open = peer.open("/fixture/A.xcodeproj");
        tokio::pin!(open);
        tokio::select! {
            biased;
            result = &mut open => panic!("response before fixture release: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }
        tokio::time::advance(Duration::from_secs(61)).await;
        tokio::select! {
            biased;
            result = &mut open => panic!("premature timeout: {result:?}"),
            _ = tokio::task::yield_now() => {}
        }
        tokio::time::resume();
        std::fs::write(release, b"ready").unwrap();
        let result = timeout(Duration::from_secs(5), &mut open).await.unwrap();
        assert_eq!(result.unwrap(), json!({"ok":true}));
    }

    #[cfg(unix)]
    #[tokio::test(start_paused = true)]
    async fn expired_deadline_rejects_spawn_and_sends_no_request() {
        let expired = Instant::now();
        assert!(ProcessPeer::spawn(Command::new("/bin/sh"), expired).is_err());
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("IFS= read -r line; sleep 20");
        let mut peer = ProcessPeer::spawn(command, expired + Duration::from_secs(5)).unwrap();
        tokio::time::advance(Duration::from_secs(6)).await;
        assert_eq!(
            peer.exchange("initialize", json!({}))
                .await
                .unwrap_err()
                .to_string(),
            "headless_bridge_timeout"
        );
        assert_eq!(peer.next_id, 1);
        tokio::time::resume();
        peer.stop().await;
    }
}
