use super::{
    host::{command, validate_host, HostFacts},
    project::{FixturePair, FixtureProject},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs::File, io::Write, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    time::{timeout, timeout_at, Instant},
};

const LINE_LIMIT: usize = 1_048_576;
const TOTAL_LIMIT: usize = 8 * LINE_LIMIT;
pub fn decode_line(bytes: &[u8]) -> Result<Value> {
    ensure!(bytes.len() <= LINE_LIMIT, "i1_line_size");
    let v: Value = serde_json::from_slice(bytes).map_err(|_| anyhow::anyhow!("i1_json_shape"))?;
    ensure!(v.is_object() && v["jsonrpc"] == "2.0", "i1_rpc_envelope");
    let response = v.get("result").is_some() ^ v.get("error").is_some();
    let notification = v.get("method").and_then(Value::as_str).is_some() && v.get("id").is_none();
    ensure!(
        (response && v.get("id").is_some() && v.get("method").is_none())
            || (notification && v.get("result").is_none() && v.get("error").is_none()),
        "i1_rpc_envelope"
    );
    Ok(v)
}

async fn bounded_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let buf = reader.fill_buf().await?;
        ensure!(!buf.is_empty(), "i1_eof");
        let n = buf
            .iter()
            .position(|c| *c == b'\n')
            .map_or(buf.len(), |i| i + 1);
        ensure!(line.len() + n <= LINE_LIMIT, "i1_line_size");
        let end = buf[n - 1] == b'\n';
        line.extend_from_slice(&buf[..n]);
        reader.consume(n);
        if end {
            return Ok(line);
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ExitRecord {
    pub code: Option<i32>,
    pub forced: bool,
}
pub struct Bridge {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    stderr_task: Option<tokio::task::JoinHandle<Vec<u8>>>,
    next_id: u64,
    received_bytes: usize,
    deadline: Instant,
    pub(super) pair: FixturePair,
    pub(super) initialized: bool,
    exit: Option<ExitRecord>,
    failed: bool,
    transcript: Option<(File, File)>,
    #[cfg(test)]
    write_limit: Option<usize>,
}
impl Bridge {
    pub(super) fn start_transcript(&mut self, dir: &Path) -> Result<()> {
        ensure!(
            self.next_id == 1 && self.transcript.is_none(),
            "i1_trace_started"
        );
        super::project::private_dir(dir)?;
        let open = |name: &str| -> Result<File> {
            let path = dir.join(name);
            super::project::write_new(&path, b"")?;
            Ok(std::fs::OpenOptions::new().append(true).open(path)?)
        };
        self.transcript = Some((open("requests.ndjson")?, open("responses.ndjson")?));
        Ok(())
    }
    fn trace(&mut self, request: bool, bytes: &[u8]) -> Result<()> {
        if let Some((requests, responses)) = &mut self.transcript {
            let file = if request { requests } else { responses };
            file.write_all(bytes)?;
            file.sync_data()?;
        }
        Ok(())
    }
    pub async fn spawn(facts: &HostFacts, pair: &FixturePair, deadline: Instant) -> Result<Self> {
        validate_host(facts)?;
        ensure!(cfg!(target_os = "macos"), "i1_unsupported_host");
        let mut cmd = command(&facts.generation.developer_dir.join("usr/bin/mcpbridge"))?;
        cmd.env("DEVELOPER_DIR", &facts.generation.developer_dir)
            .env("MCP_XCODE_PID", facts.generation.pid.to_string());
        Self::from_command(cmd, pair, deadline)
    }
    fn from_command(
        mut cmd: tokio::process::Command,
        pair: &FixturePair,
        deadline: Instant,
    ) -> Result<Self> {
        ensure!(Instant::now() < deadline, "i1_deadline");
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut stderr = child.stderr.take().unwrap();
        let stderr_task = tokio::spawn(async move {
            let mut retained = Vec::new();
            let mut block = [0u8; 4096];
            while let Ok(n) = stderr.read(&mut block).await {
                if n == 0 {
                    break;
                }
                let keep = n.min(4096 - retained.len());
                retained.extend_from_slice(&block[..keep]);
            }
            retained
        });
        Ok(Self {
            child: Some(child),
            stdin,
            stdout,
            stderr_task: Some(stderr_task),
            next_id: 1,
            received_bytes: 0,
            deadline,
            pair: pair.clone(),
            initialized: false,
            exit: None,
            failed: false,
            transcript: None,
            #[cfg(test)]
            write_limit: None,
        })
    }
    pub(super) fn check_project(&self, project: &FixtureProject) -> Result<()> {
        ensure!(
            self.initialized && self.pair.projects.contains(project),
            "i1_project_not_allowed"
        );
        Ok(())
    }
    pub(super) async fn initialized_notification(&mut self) -> Result<()> {
        timeout_at(
            self.deadline.min(Instant::now() + Duration::from_secs(30)),
            self.drain_pending(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("i1_timeout"))??;
        let bytes = b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n";
        self.trace(true, bytes)?;
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("i1_closed"))?;
        timeout_at(
            self.deadline.min(Instant::now() + Duration::from_secs(30)),
            stdin.write_all(bytes),
        )
        .await
        .map_err(|_| anyhow::anyhow!("i1_timeout"))??;
        Ok(())
    }
    pub(super) async fn exchange(&mut self, method: &str, params: Value) -> Result<Value> {
        ensure!(!self.failed, "i1_transport_stopped");
        ensure!(
            matches!(method, "initialize" | "tools/list" | "tools/call"),
            "i1_method_not_allowed"
        );
        let deadline = self.deadline.min(Instant::now() + Duration::from_secs(30));
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("i1_id_overflow"))?;
        let mut bytes =
            serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        bytes.push(b'\n');
        let result = timeout_at(deadline, async {
            self.drain_pending().await?;
            self.trace(true, &bytes)?;
            #[cfg(test)]
            if let Some(limit) = self.write_limit.take() {
                self.stdin
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("i1_closed"))?
                    .write_all(&bytes[..limit.min(bytes.len())])
                    .await?;
                return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into());
            }
            self.stdin
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("i1_closed"))?
                .write_all(&bytes)
                .await?;
            for _ in 0..=64 {
                let line = bounded_line(&mut self.stdout).await?;
                self.received_bytes += line.len();
                ensure!(self.received_bytes <= TOTAL_LIMIT, "i1_receive_size");
                self.trace(false, &line)?;
                let v = decode_line(&line).map_err(|e| {
                    super::evidence::retain(e, &json!({"raw_sha256":super::project::digest(&line)}))
                })?;
                if let Some(method) = v.get("method").and_then(Value::as_str) {
                    ensure!(
                        matches!(method, "notifications/message" | "notifications/progress"),
                        "i1_notification"
                    );
                    continue;
                }
                if v["id"].as_u64() != Some(id) {
                    return Err(super::evidence::retain(
                        anyhow::anyhow!("i1_response_id"),
                        &v,
                    ));
                }
                if v.get("error").is_some() {
                    return Err(super::evidence::retain(anyhow::anyhow!("i1_rpc_error"), &v));
                }
                return Ok(v["result"].clone());
            }
            anyhow::bail!("i1_notification_limit")
        })
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("i1_timeout")));
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn pending(&self) -> Result<bool> {
        if !self.stdout.buffer().is_empty() {
            return Ok(true);
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let mut fd = libc::pollfd {
                fd: self.stdout.get_ref().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            ensure!(unsafe { libc::poll(&mut fd, 1, 0) } >= 0, "i1_pipe_unknown");
            ensure!(
                fd.revents & (libc::POLLERR | libc::POLLNVAL) == 0,
                "i1_pipe_unknown"
            );
            let mut bytes: libc::c_int = 0;
            ensure!(
                unsafe { libc::ioctl(fd.fd, libc::FIONREAD, &mut bytes) } == 0,
                "i1_pipe_unknown"
            );
            Ok(bytes > 0)
        }
        #[cfg(not(unix))]
        {
            anyhow::bail!("i1_unsupported_host")
        }
    }
    async fn drain_pending(&mut self) -> Result<()> {
        for _ in 0..64 {
            if !self.pending()? {
                return Ok(());
            }
            let line = bounded_line(&mut self.stdout).await?;
            self.received_bytes += line.len();
            ensure!(self.received_bytes <= TOTAL_LIMIT, "i1_receive_size");
            self.trace(false, &line)?;
            let value = decode_line(&line)?;
            ensure!(
                matches!(
                    value["method"].as_str(),
                    Some("notifications/message" | "notifications/progress")
                ),
                "i1_pending_invalidation"
            );
        }
        anyhow::bail!("i1_notification_limit")
    }
    pub async fn close(&mut self) -> Result<ExitRecord> {
        if let Some(exit) = self.exit {
            return Ok(exit);
        }
        let pending = if self.initialized && !self.failed {
            timeout(Duration::from_secs(5), self.drain_pending())
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("i1_timeout")))
        } else {
            Ok(())
        };
        self.stdin.take();
        let child = self
            .child
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("i1_closed"))?;
        let mut forced = false;
        let status = match timeout(Duration::from_secs(5), child.wait()).await {
            Ok(status) => status?,
            Err(_) => {
                forced = true;
                child.start_kill()?;
                timeout(Duration::from_secs(5), child.wait())
                    .await
                    .map_err(|_| anyhow::anyhow!("i1_reap_timeout"))??
            }
        };
        self.child.take();
        if let Some(mut drain) = self.stderr_task.take() {
            if timeout(Duration::from_secs(5), &mut drain).await.is_err() {
                drain.abort();
            }
        }
        let exit = ExitRecord {
            code: status.code(),
            forced,
        };
        self.exit = Some(exit);
        pending?;
        if self.initialized && !self.failed {
            timeout(Duration::from_secs(5), self.drain_pending())
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("i1_timeout")))?;
        }
        Ok(exit)
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = timeout(Duration::from_secs(5), child.wait()).await;
                });
            }
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn i1_private_transcript_keeps_complete_requests_and_tool_error_responses() {
        use super::super::tools::Peer;
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let pair = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        let wire = t.path().join("wire");
        let raw = t.path().join("raw");
        let mut cmd = command(std::path::Path::new("/usr/bin/python3")).unwrap();
        cmd.arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/xcode_headless_i1/fake_mcpbridge.py"
        ))
        .arg("full_tool_error")
        .arg(&wire)
        .arg(&raw);
        let mut bridge =
            Bridge::from_command(cmd, &pair, Instant::now() + Duration::from_secs(10)).unwrap();
        let trace = t.path().join("private-trace");
        bridge.start_transcript(&trace).unwrap();
        Peer::initialize(&mut bridge).await.unwrap();
        assert!(Peer::open(&mut bridge, &pair.projects[0]).await.is_err());
        bridge.close().await.unwrap();
        let requests = std::fs::read(trace.join("requests.ndjson")).unwrap();
        assert_eq!(requests, std::fs::read(raw).unwrap());
        let responses: Vec<Value> = std::fs::read_to_string(trace.join("responses.ndjson"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 3);
        assert_eq!(responses[2]["result"]["isError"], true);
        assert_eq!(
            responses[2]["result"]["content"][0]["text"],
            format!("synthetic error: {}", "detail ".repeat(1024))
        );
        for path in [
            &trace,
            &trace.join("requests.ndjson"),
            &trace.join("responses.ndjson"),
        ] {
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
        assert!(bridge.start_transcript(&trace).is_err());
    }
    #[tokio::test]
    async fn review_lost_malformed_and_cancelled_owned_processes_keep_uncertainty() {
        use super::super::{
            evidence::{LocalRecorder, Recorder},
            host::fixture_facts,
            tools::Peer,
        };
        let t = tempfile::tempdir().unwrap();
        let pair = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        for scenario in ["full_lost", "full_malformed", "full_pending"] {
            let log = t.path().join(scenario);
            let mut cmd = command(std::path::Path::new("/usr/bin/python3")).unwrap();
            cmd.arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/xcode_headless_i1/fake_mcpbridge.py"
            ))
            .arg(scenario)
            .arg(&log);
            let mut bridge =
                Bridge::from_command(cmd, &pair, Instant::now() + Duration::from_secs(10)).unwrap();
            Peer::initialize(&mut bridge).await.unwrap();
            let pid = bridge.child.as_ref().unwrap().id().unwrap() as i32;
            let dir = t.path().join(format!("record-{scenario}"));
            let mut record = LocalRecorder::create(&dir).unwrap();
            record
                .begin_open(&pair.projects[0], &fixture_facts().generation)
                .unwrap();
            if scenario == "full_pending" {
                assert!(timeout(
                    Duration::from_millis(100),
                    Peer::open(&mut bridge, &pair.projects[0])
                )
                .await
                .is_err());
                drop(bridge);
                #[cfg(unix)]
                timeout(Duration::from_secs(5), async {
                    loop {
                        if unsafe { libc::kill(pid, 0) } != 0
                            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
                        {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                })
                .await
                .expect("owned child reaped after cancellation");
            } else {
                assert!(Peer::open(&mut bridge, &pair.projects[0]).await.is_err());
                bridge.close().await.unwrap();
                assert!(bridge.child.is_none());
            }
            assert!(LocalRecorder::unresolved(&dir).unwrap());
            assert_eq!(
                std::fs::read_to_string(log)
                    .unwrap()
                    .lines()
                    .filter(|line| line.contains("XcodeOpenWorkspace"))
                    .count(),
                1
            );
            assert!(LocalRecorder::create(&dir).is_err());
        }
    }
    #[tokio::test]
    async fn review_partial_pipe_write_retains_intent_and_reaps_child() {
        use super::super::{
            evidence::{LocalRecorder, Recorder},
            host::fixture_facts,
            tools::Peer,
        };
        let t = tempfile::tempdir().unwrap();
        let pair = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        let log = t.path().join("wire");
        let raw = t.path().join("raw");
        let mut cmd = command(std::path::Path::new("/usr/bin/python3")).unwrap();
        cmd.arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/xcode_headless_i1/fake_mcpbridge.py"
        ))
        .arg("full_partial")
        .arg(&log)
        .arg(&raw);
        let mut bridge =
            Bridge::from_command(cmd, &pair, Instant::now() + Duration::from_secs(10)).unwrap();
        Peer::initialize(&mut bridge).await.unwrap();
        let dir = t.path().join("record");
        let mut record = LocalRecorder::create(&dir).unwrap();
        record
            .begin_open(&pair.projects[0], &fixture_facts().generation)
            .unwrap();
        bridge.write_limit = Some(13);
        assert!(Peer::open(&mut bridge, &pair.projects[0]).await.is_err());
        bridge.close().await.unwrap();
        assert!(bridge.child.is_none());
        assert!(LocalRecorder::unresolved(&dir).unwrap());
        let bytes = std::fs::read(raw).unwrap();
        assert_eq!(bytes.rsplit(|c| *c == b'\n').next().unwrap().len(), 13);
        assert!(!std::fs::read_to_string(log)
            .unwrap()
            .contains("XcodeOpenWorkspace"));
    }
    #[tokio::test]
    async fn review_pending_invalidation_prevents_write_and_survives_final_response() {
        use super::super::tools::Peer;
        let t = tempfile::tempdir().unwrap();
        let pair = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        for scenario in ["full_list_changed", "full_final_changed"] {
            let log = t.path().join(scenario);
            let mut cmd = command(std::path::Path::new("/usr/bin/python3")).unwrap();
            cmd.arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/xcode_headless_i1/fake_mcpbridge.py"
            ))
            .arg(scenario)
            .arg(&log);
            let mut bridge =
                Bridge::from_command(cmd, &pair, Instant::now() + Duration::from_secs(10)).unwrap();
            Peer::initialize(&mut bridge).await.unwrap();
            if scenario == "full_list_changed" {
                assert!(Peer::open(&mut bridge, &pair.projects[0]).await.is_err());
                let _ = bridge.close().await;
                assert!(!std::fs::read_to_string(log)
                    .unwrap()
                    .contains("XcodeOpenWorkspace"));
            } else {
                Peer::read(&mut bridge, &pair.projects[0], "A")
                    .await
                    .unwrap();
                assert!(bridge.close().await.is_err());
                assert!(bridge.child.is_none());
            }
        }
    }
    #[test]
    fn i1_transport_rejects_oversized_line() {
        let mut bytes = br#"{"jsonrpc":"2.0","id":1,"result":{}}"#.to_vec();
        bytes.resize(1024 * 1024 + 1, b' ');
        assert!(decode_line(&bytes).is_err());
    }
    #[test]
    fn i1_transport_rejects_bad_envelopes() {
        for bytes in [
            b"[]".as_slice(),
            br#"{"id":1,"result":{}}"#,
            br#"{"jsonrpc":"2.0","id":1,"result":{},"error":{}}"#,
        ] {
            assert!(decode_line(bytes).is_err());
        }
    }
    #[tokio::test]
    async fn i1_owned_fake_process_bounds_and_reaping() {
        let temp = tempfile::tempdir().unwrap();
        let pair = super::super::project::create_pair(&temp.path().join("pair")).unwrap();
        for scenario in [
            "normal",
            "flood",
            "oversized",
            "malformed",
            "wrong_id",
            "eof",
            "timeout",
            "changed",
            "exit_one",
        ] {
            let mut cmd = command(std::path::Path::new("/usr/bin/python3")).unwrap();
            cmd.arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/xcode_headless_i1/fake_mcpbridge.py"
            ))
            .arg(scenario);
            let budget = if scenario == "timeout" {
                Duration::from_millis(200)
            } else {
                Duration::from_secs(5)
            };
            let mut bridge = Bridge::from_command(cmd, &pair, Instant::now() + budget).unwrap();
            let result = bridge.exchange("initialize", json!({})).await;
            if matches!(scenario, "normal" | "exit_one") {
                let value = result.unwrap_or_else(|e| panic!("{scenario}: {e}"));
                assert_eq!(value["environment_clean"], true);
            } else {
                assert!(result.is_err(), "{scenario}");
            }
            let exit = bridge.close().await.unwrap();
            assert!(bridge.child.is_none());
            assert_eq!(bridge.close().await.unwrap().code, exit.code);
            if scenario == "exit_one" {
                assert_eq!(exit.code, Some(1));
            }
        }
    }
    #[tokio::test]
    async fn i1_two_real_pipes_discover_and_read_without_reopen() {
        use super::super::tools::{verify_sentinel, Peer};
        let temp = tempfile::tempdir().unwrap();
        let pair = super::super::project::create_pair(&temp.path().join("pair")).unwrap();
        let log = temp.path().join("exchanges.jsonl");
        let mut ids = Vec::new();
        for connection in 0..2 {
            let mut cmd = command(std::path::Path::new("/usr/bin/python3")).unwrap();
            cmd.arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/xcode_headless_i1/fake_mcpbridge.py"
            ))
            .arg("full")
            .arg(&log);
            let mut bridge =
                Bridge::from_command(cmd, &pair, Instant::now() + Duration::from_secs(10)).unwrap();
            Peer::initialize(&mut bridge).await.unwrap();
            if connection == 0 {
                for p in &pair.projects {
                    ids.push(Peer::open(&mut bridge, p).await.unwrap().id);
                }
            }
            for i in [0, 1, 0] {
                let read = Peer::read(&mut bridge, &pair.projects[i], &ids[i])
                    .await
                    .unwrap();
                verify_sentinel(&pair.projects[i], &read).unwrap();
            }
            bridge.close().await.unwrap();
        }
        let entries: Vec<Value> = std::fs::read_to_string(log)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            entries
                .iter()
                .filter(|v| v["params"]["name"] == "XcodeOpenWorkspace")
                .count(),
            2
        );
        assert_eq!(
            entries
                .iter()
                .filter(|v| v["params"]["name"] == "XcodeRead")
                .count(),
            6
        );
    }
}
