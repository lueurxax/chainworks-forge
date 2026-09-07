use acp::input_context::{validate_request, InputContextBuilder};
use acp::{AcpRuntimeManager, ExecutionRequest};

fn request(prompt: String) -> ExecutionRequest {
    serde_json::from_value(serde_json::json!({
        "run_id": "00000000-0000-4000-8000-000000000049",
        "stage_id": "review", "agent_id": "reviewer", "provider": "not_registered",
        "model": null, "effort": null, "workspace_root": "/unused", "prompt": prompt
    }))
    .unwrap()
}

#[tokio::test]
async fn p049_oversized_utf8_prompt_is_rejected_before_adapter_lookup() {
    let manager = AcpRuntimeManager::new_with_adapters(vec![]);
    let error = manager
        .execute(request("\u{00e9}".repeat(32769)))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("input_context_prompt_too_large"),
        "{error:#}"
    );
    let receipt = acp::runtime_receipt_from_error(&error).expect("durable no-launch receipt");
    assert_eq!(
        receipt.failure_phase.as_deref(),
        Some("input_context_preflight")
    );
    assert_eq!(receipt.handshake.initialize_sent_at_ms, None);
    assert_eq!(receipt.handshake.prompt_sent_at_ms, None);
    assert_eq!(receipt.counters.total_messages, 0);
}

#[tokio::test]
async fn p049_cap_applies_to_direct_start_and_reused_session() {
    let manager = AcpRuntimeManager::new_with_adapters(vec![]);
    let req = request("x".repeat(65537));
    let error = manager.start_session(req.clone()).await.unwrap_err();
    assert!(
        error.to_string().contains("input_context_prompt_too_large"),
        "{error:#}"
    );
    let error = manager.prompt_session("no_session", req).await.unwrap_err();
    assert!(
        error.to_string().contains("input_context_prompt_too_large"),
        "{error:#}"
    );
}

#[tokio::test]
async fn p049_resurrection_attach_rejects_oversized_request_before_adapter_lookup() {
    let manager = AcpRuntimeManager::new_with_adapters(vec![]);
    let error = manager
        .attach_provider_session_for_resurrection(request("x".repeat(65537)))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("input_context_prompt_too_large"),
        "{error:#}"
    );
}

#[tokio::test]
async fn p049_exact_boundary_is_admitted_without_truncation() {
    let manager = AcpRuntimeManager::new_with_adapters(vec![]);
    let error = manager
        .execute(request("x".repeat(65536)))
        .await
        .unwrap_err();
    assert!(!error.to_string().contains("input_context_"), "{error:#}");
}

fn snapshot_request(tmp: &tempfile::TempDir) -> (ExecutionRequest, std::path::PathBuf) {
    let mut req = request(String::new());
    req.workspace_root = tmp.path().display().to_string();
    let source = tmp.path().join("source.md");
    std::fs::write(&source, "first fact\nlast required fact").unwrap();
    let mut builder = InputContextBuilder::new(&req.workspace_root, req.run_id, 0);
    builder
        .artifact_context(
            "proposal",
            source.to_str().unwrap(),
            source.to_str().unwrap(),
        )
        .unwrap();
    req.prompt = builder
        .finish("Keep system instructions and output contracts".into())
        .unwrap();
    let manifest: serde_json::Value = serde_json::from_str(
        req.prompt
            .lines()
            .next()
            .unwrap()
            .strip_prefix("CHAINWORKS_INPUT_MANIFEST_V1 ")
            .unwrap(),
    )
    .unwrap();
    (
        req,
        manifest["artifacts"][0]["path"].as_str().unwrap().into(),
    )
}

#[test]
fn p049_snapshot_survives_source_change_but_missing_snapshot_fails_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let (req, snapshot) = snapshot_request(&tmp);
    std::fs::write(tmp.path().join("source.md"), "replacement").unwrap();
    validate_request(&req).unwrap();
    assert_eq!(
        std::fs::read_to_string(&snapshot).unwrap(),
        "first fact\nlast required fact"
    );
    std::fs::remove_file(snapshot).unwrap();
    let error = validate_request(&req).unwrap_err();
    assert!(error
        .to_string()
        .contains("input_context_snapshot_unreadable"));
    assert!(!error.to_string().contains(tmp.path().to_str().unwrap()));
}

#[cfg(unix)]
#[test]
fn p049_same_size_tamper_and_writable_snapshot_fail_closed() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let (req, snapshot) = snapshot_request(&tmp);
    std::fs::set_permissions(&snapshot, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_snapshot_not_sealed"));
    std::fs::write(&snapshot, "first fact\nlast REPLACED fact").unwrap();
    std::fs::set_permissions(&snapshot, std::fs::Permissions::from_mode(0o400)).unwrap();
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_snapshot_changed"));
}

#[cfg(unix)]
#[test]
fn p049_snapshot_symlink_and_directory_escape_fail_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let (req, snapshot) = snapshot_request(&tmp);
    std::fs::remove_file(&snapshot).unwrap();
    std::os::unix::fs::symlink(tmp.path().join("source.md"), &snapshot).unwrap();
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_snapshot_unreadable"));

    let other = tempfile::tempdir().unwrap();
    let linked = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(other.path(), linked.path().join(".chainworks")).unwrap();
    let mut builder = InputContextBuilder::new(linked.path().to_str().unwrap(), req.run_id, 0);
    let error = builder
        .artifact_context(
            "proposal",
            tmp.path().join("source.md").to_str().unwrap(),
            "source",
        )
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("input_context_snapshot_directory_unsafe"));
    assert_eq!(std::fs::read_dir(other.path()).unwrap().count(), 0);
}

#[test]
fn p049_manifest_cannot_read_other_run_or_external_path() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut req, _) = snapshot_request(&tmp);
    req.run_id = domain::ids::RunId::new();
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_run_mismatch"));
    let (mut req, snapshot) = snapshot_request(&tmp);
    req.prompt = req
        .prompt
        .replace(snapshot.to_str().unwrap(), "/outside/snapshot.data");
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_snapshot_outside_read_root"));
}

#[test]
fn p049_inline_budget_is_aggregate_and_never_truncates_utf8() {
    let tmp = tempfile::tempdir().unwrap();
    let req = request(String::new());
    let source = tmp.path().join("source.md");
    let text = "\u{00e9}".repeat(5000);
    std::fs::write(&source, &text).unwrap();
    let mut builder = InputContextBuilder::new(tmp.path().to_str().unwrap(), req.run_id, 24576);
    let mut inline_count = 0;
    for name in ["a", "b", "c", "d"] {
        if let Some(inline) = builder
            .artifact_context(name, source.to_str().unwrap(), "source")
            .unwrap()
        {
            assert!(inline.contains(&text));
            inline_count += 1;
        }
    }
    assert_eq!(inline_count, 2);
    let prompt = builder.finish("mandatory".into()).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(
        prompt
            .lines()
            .next()
            .unwrap()
            .strip_prefix("CHAINWORKS_INPUT_MANIFEST_V1 ")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["artifacts"].as_array().unwrap().len(), 2);
    for artifact in manifest["artifacts"].as_array().unwrap() {
        assert_eq!(
            std::fs::read_to_string(artifact["path"].as_str().unwrap()).unwrap(),
            text
        );
    }
}

#[tokio::test]
async fn p049_tampered_snapshot_is_rejected_before_adapter_lookup() {
    let tmp = tempfile::tempdir().unwrap();
    let (req, snapshot) = snapshot_request(&tmp);
    std::fs::remove_file(snapshot).unwrap();
    let manager = AcpRuntimeManager::new_with_adapters(vec![]);
    let error = manager.execute(req).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("input_context_snapshot_unreadable"),
        "{error:#}"
    );
}

#[test]
fn p049_replaced_prompt_keeps_durable_input_binding() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut req, snapshot) = snapshot_request(&tmp);
    acp::input_context::bind_request_manifest(&mut req).unwrap();
    req.prompt = "Repair the output contract without changing the input".into();
    let serialized = serde_json::to_string(&req).unwrap();
    let restored: ExecutionRequest = serde_json::from_str(&serialized).unwrap();
    validate_request(&restored).unwrap();
    std::fs::remove_file(snapshot).unwrap();
    assert!(validate_request(&restored)
        .unwrap_err()
        .to_string()
        .contains("input_context_snapshot_unreadable"));
}

#[test]
fn p049_rendered_manifest_cannot_replace_a_bound_input() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut req, _) = snapshot_request(&tmp);
    acp::input_context::bind_request_manifest(&mut req).unwrap();
    req.prompt = req
        .prompt
        .replace("\"name\":\"proposal\"", "\"name\":\"different\"");
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_manifest_binding_changed"));
}

#[test]
fn p049_worktree_strategy_matches_real_session_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let (mut req, _) = snapshot_request(&tmp);
    req.worktree_root = Some("/not_the_cwd".into());
    req.worktree_strategy = Some("meta_only".into());
    validate_request(&req).unwrap();
    req.worktree_strategy = Some("dedicated".into());
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_read_root_unavailable"));
}

#[cfg(unix)]
#[test]
fn p049_writable_snapshot_directory_and_nonregular_source_fail_closed() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let (req, snapshot) = snapshot_request(&tmp);
    std::fs::set_permissions(
        snapshot.parent().unwrap(),
        std::fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    assert!(validate_request(&req)
        .unwrap_err()
        .to_string()
        .contains("input_context_snapshot_directory_unsafe"));
    let fifo = tmp.path().join("fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(acp::input_context::read_source(&fifo)
        .unwrap_err()
        .to_string()
        .contains("input_context_source_not_regular"));
}

#[cfg(unix)]
#[tokio::test]
async fn p049_real_acp_fixture_reads_every_snapshot_byte_from_session_cwd() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    let tmp = tempfile::tempdir().unwrap();
    let (mut req, _) = snapshot_request(&tmp);
    let script = tmp.path().join("read_snapshot.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import hashlib, json, os, sys
def receive():
    return json.loads(sys.stdin.readline())
def reply(message, result):
    print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':result}), flush=True)
message = receive()
reply(message, {'protocolVersion':1})
message = receive()
cwd = os.path.realpath(message['params']['cwd'])
reply(message, {'sessionId':'input-snapshot-fixture'})
message = receive()
prompt = message['params']['prompt'][0]['text']
assert len(prompt.encode()) <= 65536
manifest = json.loads(prompt.splitlines()[0].split(' ', 1)[1])
readback = []
for artifact in manifest['artifacts']:
    assert os.path.commonpath([cwd, artifact['path']]) == os.path.realpath(cwd)
    with open(artifact['path'], 'rb') as source:
        content = source.read()
    assert len(content) == artifact['size_bytes']
    assert hashlib.sha256(content).hexdigest() == artifact['sha256']
    readback.append(content.decode())
with open(os.path.join(cwd, 'provider-readback.json'), 'w') as output:
    json.dump(readback, output)
reply(message, {'stopReason':'end_turn'})
sys.stdin.readline()
"#,
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    req.provider = "gemini".into();
    let manager = AcpRuntimeManager::new_with_adapters(vec![Arc::new(
        acp::adapters::gemini::GeminiCliAdapter::new_with_binary(script.to_str().unwrap()),
    )]);
    let result = manager.execute(req).await.unwrap();
    assert_eq!(result.status, domain::agent::AgentStatus::Completed);
    let readback: Vec<String> =
        serde_json::from_slice(&std::fs::read(tmp.path().join("provider-readback.json")).unwrap())
            .unwrap();
    assert_eq!(readback, vec!["first fact\nlast required fact"]);
}
