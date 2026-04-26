//! Proposal 029 §7.1 dogfood-config consistency tests.
//!
//! The repo-root `.mcp.json` registers this daemon as an MCP server for
//! Claude Code, and `CLAUDE.md` documents the same connection details.
//! Drift between the two files breaks `claude mcp list` and the onboarding
//! flow — so we gate both files in the repo test suite.
//!
//! These tests read files from the repo root (discovered via
//! `CARGO_MANIFEST_DIR`) and do not touch the database or network.

use std::fs;
use std::path::PathBuf;

/// Resolve the repo root from the crate manifest location.
/// `CARGO_MANIFEST_DIR` for `daemon` is `<repo>/control-plane/crates/daemon`,
/// so two `parent()` hops land on the repo root.
fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("crate dir has parent")
        .parent()
        .expect("crates dir has parent")
        .parent()
        .expect("control-plane dir has parent")
        .to_path_buf()
}

#[test]
fn test_dogfood_mcp_json_contains_chainworks_server_with_auth_header() {
    let path = repo_root().join(".mcp.json");
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let value: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("parse {} as JSON: {e}", path.display()));

    let servers = value
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .expect(".mcp.json must have an mcpServers object");

    let entry = servers
        .get("chainworks-control-plane")
        .expect(".mcp.json must register an 'chainworks-control-plane' server");

    // HTTP transport with the MCP endpoint URL.
    assert_eq!(
        entry.get("type").and_then(|v| v.as_str()),
        Some("http"),
        "chainworks-control-plane must declare type=http"
    );
    assert_eq!(
        entry.get("url").and_then(|v| v.as_str()),
        Some("http://127.0.0.1:4000/mcp"),
        "chainworks-control-plane must point at http://127.0.0.1:4000/mcp"
    );

    // Authorization header must be present AND use ${CHAINWORKS_MCP_TOKEN}.
    let auth = entry
        .get("headers")
        .and_then(|h| h.get("Authorization"))
        .and_then(|v| v.as_str())
        .expect("chainworks-control-plane must set an Authorization header");

    assert!(
        auth.starts_with("Bearer "),
        "Authorization header must be a Bearer token, got {auth:?}"
    );
    assert!(
        auth.contains("${CHAINWORKS_MCP_TOKEN}"),
        "Authorization header must reference ${{CHAINWORKS_MCP_TOKEN}}, not a hard-coded token. Got: {auth:?}"
    );
}

#[test]
fn test_dogfood_claude_md_matches_committed_mcp_json() {
    let root = repo_root();

    let mcp_raw = fs::read_to_string(root.join(".mcp.json")).expect("read .mcp.json");
    let mcp: serde_json::Value = serde_json::from_str(&mcp_raw).expect("parse .mcp.json");

    let entry = mcp
        .pointer("/mcpServers/chainworks-control-plane")
        .expect(".mcp.json must contain chainworks-control-plane");
    let url = entry
        .get("url")
        .and_then(|v| v.as_str())
        .expect(".mcp.json chainworks-control-plane.url must be a string");

    let claude_md = fs::read_to_string(root.join("CLAUDE.md")).expect("read CLAUDE.md");

    // CLAUDE.md must reference the same URL as .mcp.json so operators
    // following the onboarding guide land on the right endpoint.
    assert!(
        claude_md.contains(url),
        "CLAUDE.md does not mention the MCP URL {url:?} from .mcp.json — the dogfood docs have drifted"
    );

    // CLAUDE.md must document the bearer-token requirement so operators
    // know they need `CHAINWORKS_MCP_TOKEN` in their shell.
    assert!(
        claude_md.contains("CHAINWORKS_MCP_TOKEN"),
        "CLAUDE.md does not mention CHAINWORKS_MCP_TOKEN — onboarding guide does not match auth requirement"
    );

    // CLAUDE.md must declare the Authorization header contract so operators
    // who inspect .mcp.json understand the shape.
    assert!(
        claude_md.contains("Authorization: Bearer"),
        "CLAUDE.md does not mention 'Authorization: Bearer' — auth contract is not documented"
    );
}
