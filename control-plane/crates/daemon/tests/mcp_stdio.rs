use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn mcp_mode_keeps_stdout_protocol_clean_for_initialize() {
    let binary = std::env::var("CARGO_BIN_EXE_control-plane")
        .expect("cargo should provide path to control-plane test binary");

    let db_path = temp_db_path("mcp-stdio");
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let mut child = Command::new(binary)
        .env("MODE", "mcp")
        .env("DATABASE_URL", database_url)
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
}

fn temp_db_path(prefix: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{unique}.db"))
}
