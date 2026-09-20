use super::{
    host::HostFacts,
    project::{FixturePair, FixtureProject},
    protocol::{Bridge, ExitRecord},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenResult {
    pub id: String,
    pub path: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadResult {
    pub content: String,
    pub file_path: String,
    pub file_size: u64,
    pub total_lines: u64,
    pub lines_read: u64,
    pub start_line: u64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MappingStatus {
    Observed,
    Unverified,
}
fn payload(v: &Value) -> Result<Value> {
    ensure!(v.is_object(), "i1_tool_shape");
    if let Some(error) = v.get("isError") {
        ensure!(error == &json!(false), "i1_tool_error");
    }
    let structured = v.get("structuredContent");
    let text = if let Some(content) = v.get("content") {
        let items = content
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("i1_tool_shape"))?;
        ensure!(
            items.len() == 1 && items[0]["type"] == "text",
            "i1_tool_shape"
        );
        Some(serde_json::from_str::<Value>(
            items[0]["text"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("i1_tool_shape"))?,
        )?)
    } else {
        None
    };
    if let (Some(a), Some(b)) = (structured, &text) {
        ensure!(a == b, "i1_conflicting_encoding");
    }
    let result = structured
        .cloned()
        .or(text)
        .ok_or_else(|| anyhow::anyhow!("i1_tool_shape"))?;
    ensure!(result.is_object(), "i1_tool_shape");
    Ok(result)
}
fn string(v: &Value, key: &str, limit: usize) -> Result<String> {
    let s = v[key]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("i1_tool_shape"))?;
    ensure!(
        !s.is_empty() && s.len() <= limit && !s.contains('\0'),
        "i1_tool_shape"
    );
    Ok(s.into())
}
pub fn decode_open(v: &Value) -> Result<OpenResult> {
    decode_open_inner(v).map_err(|e| super::evidence::retain(e, v))
}
fn decode_open_inner(v: &Value) -> Result<OpenResult> {
    let p = payload(v)?;
    Ok(OpenResult {
        id: string(&p, "workspaceIdentifier", 256)?,
        path: if p.get("workspacePath").is_some() {
            Some(string(&p, "workspacePath", 4096)?)
        } else {
            None
        },
    })
}
pub fn decode_read(v: &Value) -> Result<ReadResult> {
    decode_read_inner(v).map_err(|e| super::evidence::retain(e, v))
}
fn decode_read_inner(v: &Value) -> Result<ReadResult> {
    let p = payload(v)?;
    let number = |key| {
        p.get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow::anyhow!("i1_tool_shape"))
    };
    Ok(ReadResult {
        content: string(&p, "content", 4096)?,
        file_path: string(&p, "filePath", 4096)?,
        file_size: number("fileSize")?,
        total_lines: number("totalLines")?,
        lines_read: number("linesRead")?,
        start_line: number("startLine")?,
    })
}
pub fn mapping_status(p: &FixtureProject, r: &OpenResult) -> Result<MappingStatus> {
    let Some(path) = &r.path else {
        return Ok(MappingStatus::Unverified);
    };
    let path = std::path::Path::new(path);
    ensure!(path.is_absolute(), "i1_mapping_shape");
    let actual = path
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("i1_mapping_mismatch"))?;
    ensure!(actual == p.package, "i1_mapping_mismatch");
    Ok(MappingStatus::Observed)
}
pub fn verify_sentinel(p: &FixtureProject, r: &ReadResult) -> Result<()> {
    ensure!(
        r.file_path == p.read_path
            && r.file_size == (p.marker.len() + 1) as u64
            && r.total_lines == 2
            && r.lines_read == 2
            && r.start_line == 1,
        "i1_read_mismatch"
    );
    // Xcode includes the empty editor line after the fixture's final newline.
    let mut lines = r.content.split('\n');
    for (expected_number, expected_text) in [("1", p.marker.as_str()), ("2", "")] {
        let (number, text) = lines
            .next()
            .and_then(|line| line.split_once('\t'))
            .ok_or_else(|| anyhow::anyhow!("i1_read_shape"))?;
        ensure!(
            number.trim() == expected_number && text == expected_text,
            "i1_read_mismatch"
        );
    }
    ensure!(lines.next().is_none(), "i1_read_mismatch");
    Ok(())
}

fn validate_initialize(v: &Value) -> Result<()> {
    ensure!(
        v["protocolVersion"] == "2025-06-18"
            && v["serverInfo"]["name"] == "xcode-tools"
            && v["serverInfo"]["version"] == "25317"
            && v["capabilities"]["tools"].is_object(),
        "i1_server_contract"
    );
    Ok(())
}
fn validate_tools(tools: &[Value]) -> Result<()> {
    let mut names = std::collections::BTreeSet::new();
    for tool in tools {
        let name = tool["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("i1_tool_list_shape"))?;
        ensure!(names.insert(name), "i1_duplicate_tool");
    }
    let saved: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/xcode_headless_i1/apple-contract.json"
    ))?;
    for expected in saved["tools"].as_array().unwrap() {
        let actual = tools
            .iter()
            .find(|t| t["name"] == expected["name"])
            .ok_or_else(|| anyhow::anyhow!("i1_missing_tool"))?;
        ensure!(
            actual["inputSchema"] == expected["inputSchema"]
                && actual["outputSchema"] == expected["outputSchema"],
            "i1_schema_drift"
        );
    }
    Ok(())
}
fn open_arguments(p: &FixtureProject) -> Value {
    json!({"name":"XcodeOpenWorkspace","arguments":{"path":p.package}})
}
fn read_arguments(p: &FixtureProject, id: &str) -> Result<Value> {
    ensure!(!id.is_empty() && id.len() <= 256, "i1_missing_workspace_id");
    Ok(
        json!({"name":"XcodeRead","arguments":{"workspaceIdentifier":id,"filePath":p.read_path,"offset":1,"limit":8}}),
    )
}
#[async_trait::async_trait]
pub trait Peer: Send {
    async fn initialize(&mut self) -> Result<()>;
    async fn open(&mut self, project: &FixtureProject) -> Result<OpenResult>;
    async fn read(&mut self, project: &FixtureProject, id: &str) -> Result<ReadResult>;
    async fn close(&mut self) -> Result<ExitRecord>;
}
#[async_trait::async_trait]
pub trait PeerFactory: Send {
    async fn connect(
        &mut self,
        facts: &HostFacts,
        pair: &FixturePair,
        deadline: Instant,
    ) -> Result<Box<dyn Peer>>;
}
pub struct LivePeerFactory {
    record_dir: std::path::PathBuf,
    connections: u8,
}
impl LivePeerFactory {
    pub fn new(record_dir: std::path::PathBuf) -> Self {
        Self {
            record_dir,
            connections: 0,
        }
    }
}
#[async_trait::async_trait]
impl PeerFactory for LivePeerFactory {
    async fn connect(
        &mut self,
        facts: &HostFacts,
        pair: &FixturePair,
        deadline: Instant,
    ) -> Result<Box<dyn Peer>> {
        ensure!(self.connections < 2, "i1_connection_limit");
        self.connections += 1;
        let mut bridge = Bridge::spawn(facts, pair, deadline).await?;
        bridge.start_transcript(&self.record_dir.join(format!("bridge-{}", self.connections)))?;
        Ok(Box::new(bridge))
    }
}
#[async_trait::async_trait]
impl Peer for Bridge {
    async fn initialize(&mut self) -> Result<()> {
        ensure!(!self.initialized, "i1_already_initialized");
        let response = self
            .exchange(
                "initialize",
                json!({"protocolVersion":"2025-06-18","capabilities":{},
            "clientInfo":{"name":"Chainworks I1","version":"1"}}),
            )
            .await?;
        validate_initialize(&response)?;
        self.initialized_notification().await?;
        let mut tools = Vec::new();
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..16 {
            let page = self
                .exchange(
                    "tools/list",
                    cursor.as_ref().map_or(json!({}), |c| json!({"cursor":c})),
                )
                .await?;
            let items = page["tools"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("i1_tool_list_shape"))?;
            ensure!(tools.len() + items.len() <= 512, "i1_tool_list_size");
            tools.extend(items.iter().cloned());
            if let Some(next) = page.get("nextCursor") {
                let next = next
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 1024)
                    .ok_or_else(|| anyhow::anyhow!("i1_cursor_shape"))?;
                ensure!(seen.insert(next.to_string()), "i1_cursor_cycle");
                cursor = Some(next.to_string());
            } else {
                validate_tools(&tools)?;
                self.initialized = true;
                return Ok(());
            }
        }
        anyhow::bail!("i1_tool_pages")
    }
    async fn open(&mut self, project: &FixtureProject) -> Result<OpenResult> {
        self.check_project(project)?;
        decode_open(&self.exchange("tools/call", open_arguments(project)).await?)
    }
    async fn read(&mut self, project: &FixtureProject, id: &str) -> Result<ReadResult> {
        self.check_project(project)?;
        decode_read(
            &self
                .exchange("tools/call", read_arguments(project, id)?)
                .await?,
        )
    }
    async fn close(&mut self) -> Result<ExitRecord> {
        Bridge::close(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_rejected_result_retains_bounded_shape_and_hash_only() {
        let payload = json!({"structuredContent":{"workspaceIdentifier":null,"workspacePath":"/foreign/private-project"},
            "secret-token":"never expose this"});
        let error = decode_open(&payload).unwrap_err();
        let evidence = error
            .downcast_ref::<super::super::evidence::DeviationError>()
            .expect("deviation evidence");
        let public = serde_json::to_string(&evidence.0).unwrap();
        assert!(!public.contains("foreign"));
        assert!(!public.contains("secret-token"));
        assert_eq!(
            evidence.0.payload_sha256,
            super::super::project::digest(&serde_json::to_vec(&payload).unwrap())
        );
        assert_eq!(
            evidence.0.shape["structuredContent"]["workspaceIdentifier"],
            "null"
        );
    }
    #[test]
    fn i1_optional_path_does_not_fabricate_mapping() {
        let opened = decode_open(
            &json!({"isError":false,"structuredContent":{"workspaceIdentifier":"workspace1"}}),
        )
        .unwrap();
        assert_eq!(opened.id, "workspace1");
        assert!(opened.path.is_none());
        let t = tempfile::tempdir().unwrap();
        let p = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        assert_eq!(
            mapping_status(&p.projects[0], &opened).unwrap(),
            MappingStatus::Unverified
        );
    }
    #[test]
    fn i1_result_encodings_and_contradictions() {
        let payload = json!({"workspaceIdentifier":"workspace1"});
        assert!(
            decode_open(&json!({"content":[{"type":"text","text":payload.to_string()}]})).is_ok()
        );
        assert!(decode_open(&json!({"structuredContent":payload,"content":[{"type":"text","text":payload.to_string()}]})).is_ok());
        for v in [
            json!({"isError":true,"structuredContent":payload}),
            json!({"structuredContent":{"workspaceIdentifier":null}}),
            json!({"structuredContent":{"workspaceIdentifier":""}}),
            json!({"structuredContent":{"workspaceIdentifier":"a","workspacePath":null}}),
            json!({"structuredContent":payload,"content":[{"type":"text","text":"{\"workspaceIdentifier\":\"b\"}"}]}),
            json!({"content":[{"type":"text","text":"opened workspace1"}]}),
        ] {
            assert!(decode_open(&v).is_err());
        }
    }
    #[test]
    fn i1_read_metadata_and_marker_are_exact() {
        let t = tempfile::tempdir().unwrap();
        let p = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        let raw = json!({"content":"     1\tCW_I1_A\n     2\t","filePath":"Fixture/Sources/Sentinel.txt","fileSize":8,
            "totalLines":2,"linesRead":2,"startLine":1});
        let read = decode_read(&json!({"structuredContent":raw})).unwrap();
        verify_sentinel(&p.projects[0], &read).unwrap();
        assert!(verify_sentinel(&p.projects[1], &read).is_err());
        for content in [
            "1\tCW_I1_A",
            "1\tCW_I1_A\n2\twrong",
            "2\tCW_I1_A\n1\t",
            "1\tCW_I1_B\n2\t",
            "1\tCW_I1_A\n2\t\n",
        ] {
            let mut bad = read.clone();
            bad.content = content.into();
            assert!(verify_sentinel(&p.projects[0], &bad).is_err());
        }
        for key in ["fileSize", "totalLines", "linesRead", "startLine"] {
            let mut bad = raw.clone();
            bad[key] = json!(-1);
            assert!(decode_read(&json!({"structuredContent":bad})).is_err());
        }
        let mut bad = read;
        bad.start_line = 2;
        assert!(verify_sentinel(&p.projects[0], &bad).is_err());
    }
    #[test]
    fn i1_schema_version_and_exact_arguments() {
        let contract: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/xcode_headless_i1/apple-contract.json"
        ))
        .unwrap();
        let tools = contract["tools"].as_array().unwrap().clone();
        validate_tools(&tools).unwrap();
        for key in ["inputSchema", "outputSchema"] {
            let mut bad = tools.clone();
            bad[0][key] = json!({});
            assert!(validate_tools(&bad).is_err());
        }
        let mut duplicate = tools.clone();
        duplicate.push(tools[0].clone());
        assert!(validate_tools(&duplicate).is_err());
        let mut init = json!({"protocolVersion":"2025-06-18","serverInfo":{"name":"xcode-tools","version":"25317"},"capabilities":{"tools":{}}});
        validate_initialize(&init).unwrap();
        init["serverInfo"]["version"] = json!("changed");
        assert!(validate_initialize(&init).is_err());
        let t = tempfile::tempdir().unwrap();
        let pair = super::super::project::create_pair(&t.path().join("pair")).unwrap();
        let p = &pair.projects[0];
        assert_eq!(open_arguments(p)["arguments"], json!({"path":p.package}));
        assert_eq!(
            read_arguments(p, "returned").unwrap()["arguments"],
            json!({"workspaceIdentifier":"returned","filePath":p.read_path,"offset":1,"limit":8})
        );
        assert!(read_arguments(p, "").is_err());
        assert!(mapping_status(
            p,
            &OpenResult {
                id: "a".into(),
                path: Some(pair.projects[1].package.to_string_lossy().into())
            }
        )
        .is_err());
    }
}
