use std::fs;
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn mcp_mode_keeps_stdout_protocol_clean_for_initialize() {
    let binary = std::env::var("CARGO_BIN_EXE_control-plane")
        .expect("cargo should provide path to control-plane test binary");

    let db_path = temp_db_path("mcp-stdio");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let principal_path = write_principal_fixture("mcp-stdio-principals");

    let mut child = Command::new(binary)
        .env("MODE", "mcp")
        .env("DATABASE_URL", database_url)
        .env("CHAINWORKS_AUTH_PRINCIPALS_PATH", &principal_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn control-plane in mcp mode");

    let stdin = child.stdin.as_mut().expect("stdin should be piped");
    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{}}}}"
    )
    .expect("write initialize request");
    stdin.flush().expect("flush initialize request");

    let stdout = child.stdout.take().expect("stdout should be piped");
    let mut stdout_reader = BufReader::new(stdout);
    let mut first_line = String::new();
    stdout_reader
        .read_line(&mut first_line)
        .expect("read first stdout line");

    assert!(
        first_line.trim_start().starts_with("{\"jsonrpc\":"),
        "stdout must start with MCP JSON-RPC, got: {}",
        first_line
    );

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(principal_path);
}

#[test]
fn test_mcp_stdio_rejects_first_frame_other_than_initialize() {
    let binary = std::env::var("CARGO_BIN_EXE_control-plane")
        .expect("cargo should provide path to control-plane test binary");

    let db_path = temp_db_path("mcp-stdio-preinit");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let principal_path = write_principal_fixture("mcp-stdio-preinit-principals");

    let mut child = Command::new(binary)
        .env("MODE", "mcp")
        .env("DATABASE_URL", database_url)
        .env("CHAINWORKS_AUTH_PRINCIPALS_PATH", &principal_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn control-plane in mcp mode");

    let stdin = child.stdin.as_mut().expect("stdin should be piped");
    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"tools/list\",\"params\":{{}}}}"
    )
    .expect("write pre-initialize request");
    stdin.flush().expect("flush pre-initialize request");

    let stdout = child.stdout.take().expect("stdout should be piped");
    let mut stdout_reader = BufReader::new(stdout);
    let mut first_line = String::new();
    stdout_reader
        .read_line(&mut first_line)
        .expect("read first stdout line");

    let response: serde_json::Value =
        serde_json::from_str(first_line.trim()).expect("stdio response should be JSON");
    assert_eq!(response["error"]["code"], -32002);
    assert_eq!(response["error"]["message"], "server not initialized");

    assert_child_exits(&mut child, Duration::from_secs(2));
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(principal_path);
}

#[test]
fn test_mcp_stdio_rejects_initialize_without_principal_token() {
    let binary = std::env::var("CARGO_BIN_EXE_control-plane")
        .expect("cargo should provide path to control-plane test binary");

    let db_path = temp_db_path("mcp-stdio-missing-token");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let principal_path = write_principal_fixture("mcp-stdio-missing-token-principals");

    let mut child = Command::new(binary)
        .env("MODE", "mcp")
        .env("DATABASE_URL", database_url)
        .env("CHAINWORKS_AUTH_PRINCIPALS_PATH", &principal_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn control-plane in mcp mode");

    let stdin = child.stdin.as_mut().expect("stdin should be piped");
    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"clientInfo\":{{}}}}}}"
    )
    .expect("write initialize request");
    stdin.flush().expect("flush initialize request");

    let stdout = child.stdout.take().expect("stdout should be piped");
    let mut stdout_reader = BufReader::new(stdout);
    let mut first_line = String::new();
    stdout_reader
        .read_line(&mut first_line)
        .expect("read first stdout line");

    let response: serde_json::Value =
        serde_json::from_str(first_line.trim()).expect("stdio response should be JSON");
    assert_eq!(response["error"]["code"], -32000);
    // SEC-REQ-1: All auth failures collapse to a single opaque message.
    assert_eq!(response["error"]["message"], "unauthorized");

    assert_child_exits(&mut child, Duration::from_secs(2));
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(principal_path);
}

#[test]
fn test_mcp_stdio_rejects_initialize_with_unknown_principal_token() {
    let binary = std::env::var("CARGO_BIN_EXE_control-plane")
        .expect("cargo should provide path to control-plane test binary");

    let db_path = temp_db_path("mcp-stdio-unknown-token");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let principal_path = temp_db_path("mcp-stdio-principals").with_extension("json");
    fs::write(&principal_path, principal_fixture_json()).expect("write principal fixture");
    set_owner_only_permissions(&principal_path);

    let mut child = Command::new(binary)
        .env("MODE", "mcp")
        .env("DATABASE_URL", database_url)
        .env("CHAINWORKS_AUTH_PRINCIPALS_PATH", &principal_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn control-plane in mcp mode");

    let stdin = child.stdin.as_mut().expect("stdin should be piped");
    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"clientInfo\":{{\"principal_token\":\"unknown-mcp-token-xxxxxxxxxxxxxx\"}}}}}}"
    )
    .expect("write initialize request");
    stdin.flush().expect("flush initialize request");

    let stdout = child.stdout.take().expect("stdout should be piped");
    let mut stdout_reader = BufReader::new(stdout);
    let mut first_line = String::new();
    stdout_reader
        .read_line(&mut first_line)
        .expect("read first stdout line");

    let response: serde_json::Value =
        serde_json::from_str(first_line.trim()).expect("stdio response should be JSON");
    assert_eq!(response["error"]["code"], -32000);
    // SEC-REQ-1: All auth failures collapse to a single opaque message.
    assert_eq!(response["error"]["message"], "unauthorized");

    assert_child_exits(&mut child, Duration::from_secs(2));
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(principal_path);
}

#[test]
fn test_mcp_stdio_binds_principal_for_session_lifetime() {
    let binary = std::env::var("CARGO_BIN_EXE_control-plane")
        .expect("cargo should provide path to control-plane test binary");

    let db_path = temp_db_path("mcp-stdio-session");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let principal_path = write_principal_fixture("mcp-stdio-session-principals");

    let mut child = Command::new(binary)
        .env("MODE", "mcp")
        .env("DATABASE_URL", database_url)
        .env("CHAINWORKS_AUTH_PRINCIPALS_PATH", &principal_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn control-plane in mcp mode");

    let mut stdin = child.stdin.take().expect("stdin should be piped");
    let stdout = child.stdout.take().expect("stdout should be piped");
    let mut stdout_reader = BufReader::new(stdout);

    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"clientInfo\":{{\"principal_token\":\"known-mcp-token-xxxxxxxxxxxxxxxx\"}}}}}}"
    )
    .expect("write initialize request");
    stdin.flush().expect("flush initialize request");
    let init_response = read_json_line(&mut stdout_reader);
    assert!(init_response["result"]["serverInfo"]["name"]
        .as_str()
        .is_some());

    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{{}}}}"
    )
    .expect("write tools/list request");
    stdin.flush().expect("flush tools/list request");
    let tools_response = read_json_line(&mut stdout_reader);
    let tools = tools_response["result"]["tools"].as_array().unwrap();
    assert!(tools.iter().any(|tool| tool["name"] == "runs_start"));

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(principal_path);
}

#[test]
fn test_mcp_stdio_rejects_reinitialize_mid_session() {
    let binary = std::env::var("CARGO_BIN_EXE_control-plane")
        .expect("cargo should provide path to control-plane test binary");

    let db_path = temp_db_path("mcp-stdio-reinitialize");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let principal_path = write_principal_fixture("mcp-stdio-reinit-principals");

    let mut child = Command::new(binary)
        .env("MODE", "mcp")
        .env("DATABASE_URL", database_url)
        .env("CHAINWORKS_AUTH_PRINCIPALS_PATH", &principal_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn control-plane in mcp mode");

    let mut stdin = child.stdin.take().expect("stdin should be piped");
    let stdout = child.stdout.take().expect("stdout should be piped");
    let mut stdout_reader = BufReader::new(stdout);

    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"clientInfo\":{{\"principal_token\":\"known-mcp-token-xxxxxxxxxxxxxxxx\"}}}}}}"
    )
    .expect("write initialize request");
    stdin.flush().expect("flush initialize request");
    let _ = read_json_line(&mut stdout_reader);

    writeln!(
        stdin,
        "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"initialize\",\"params\":{{\"clientInfo\":{{\"principal_token\":\"known-mcp-token-xxxxxxxxxxxxxxxx\"}}}}}}"
    )
    .expect("write second initialize request");
    stdin.flush().expect("flush second initialize request");
    let response = read_json_line(&mut stdout_reader);
    assert_eq!(response["error"]["code"], -32600);
    assert_eq!(response["error"]["message"], "Session already initialized");

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(principal_path);
}

fn read_json_line<R: BufRead>(reader: &mut R) -> serde_json::Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read stdout line");
    serde_json::from_str(line.trim()).expect("stdio response should be JSON")
}

fn write_principal_fixture(prefix: &str) -> PathBuf {
    let principal_path = temp_db_path(prefix).with_extension("json");
    fs::write(&principal_path, principal_fixture_json()).expect("write principal fixture");
    set_owner_only_permissions(&principal_path);
    principal_path
}

fn principal_fixture_json() -> &'static str {
    r#"{
      "schema_version": 2,
      "principals": [
        {
          "token": "known-mcp-token-xxxxxxxxxxxxxxxx",
          "id": "default-operator",
          "class": "operator",
          "surface_policies": {
            "graphql": {
              "allow_queries": true,
              "allow_subscriptions": true,
              "allowed_mutations": ["approveApproval", "rejectApproval"]
            },
            "mcp": {
              "allowed_tools": ["runs.start"]
            }
          }
        }
      ]
    }"#
}

fn set_owner_only_permissions(path: &PathBuf) {
    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(path)
            .expect("principal fixture metadata")
            .permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(path, permissions).expect("set principal fixture permissions");
    }
}

fn assert_child_exits(child: &mut std::process::Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().expect("poll child exit").is_some() {
            return;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not exit within {:?}", timeout);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn temp_db_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{unique}.db"))
}
