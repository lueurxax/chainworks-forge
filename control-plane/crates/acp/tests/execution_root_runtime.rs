#![cfg(unix)]

use acp::adapters::claude::ClaudeAgentAdapter;
use acp::{AcpRuntimeManager, ExecutionRequest, XcodeShimGrantRecord, XcodeShimGrantStore};
use domain::agent::AgentStatus;
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecordingGrants {
    records: Mutex<BTreeMap<String, XcodeShimGrantRecord>>,
    prompt_activations: Mutex<usize>,
}

impl XcodeShimGrantStore for RecordingGrants {
    fn insert_xcode_shim_grant(&self, record: XcodeShimGrantRecord) {
        self.records
            .lock()
            .unwrap()
            .insert(record.grant.token_id.clone(), record);
    }

    fn set_xcode_shim_grant_active_prompt(&self, token_id: &str, active: bool) -> bool {
        let mut records = self.records.lock().unwrap();
        let Some(record) = records.get_mut(token_id) else {
            return false;
        };
        record.active_prompt = active;
        if active {
            *self.prompt_activations.lock().unwrap() += 1;
        }
        true
    }

    fn remove_xcode_shim_grant(&self, token_id: &str) -> Option<XcodeShimGrantRecord> {
        self.records.lock().unwrap().remove(token_id)
    }
}

struct RootFixture {
    _tmp: tempfile::TempDir,
    manager: AcpRuntimeManager,
    grants: Arc<RecordingGrants>,
    worktree: PathBuf,
    capture: PathBuf,
    request: ExecutionRequest,
}

impl RootFixture {
    fn new(shim_enabled: bool) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let repository = tmp.path().join("repository");
        let worktree = tmp.path().join("readonly-worktree");
        let shim_dir = tmp.path().join("empty-shims");
        for path in [&repository, &worktree, &shim_dir] {
            std::fs::create_dir(path).unwrap();
        }
        let repository = repository.canonicalize().unwrap();
        let worktree = worktree.canonicalize().unwrap();
        let capture = tmp.path().join("root-observations.jsonl");
        std::fs::write(&capture, "").unwrap();
        let script = tmp.path().join("root_fixture.py");
        // Only the synthetic provider runs. No shim socket or Apple process exists.
        let code = r#"#!/usr/bin/env python3
import json, os, sys
capture = __CAPTURE__
session_cwd = None
session_id = "fixture-worktree-session"
for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    params = request.get("params", {})
    if method == "session/new":
        session_cwd = params["cwd"]
    observation = {
        "method": method,
        "pid": os.getpid(),
        "process_cwd": os.getcwd(),
        "session_cwd": session_cwd,
        "shim_root": os.environ.get("CHAINWORKS_XCODE_SHIM_WORKSPACE_ROOT"),
        "resume_session_id": params.get("resumeSessionId"),
    }
    with open(capture, "a") as output:
        output.write(json.dumps(observation) + "\n")
    if "id" not in request:
        continue
    result = {}
    if method == "initialize":
        result = {"protocolVersion": 1, "serverInfo": {"name": "root-fixture", "version": "1"}}
    elif method == "session/new":
        result = {"sessionId": session_id}
    elif method == "session/prompt":
        result = {"stopReason": "end_turn", "sessionId": session_id}
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
    if method == "session/close":
        break
"#
        .replace(
            "__CAPTURE__",
            &serde_json::to_string(capture.to_str().unwrap()).unwrap(),
        );
        std::fs::write(&script, code).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();

        let manager = AcpRuntimeManager::new_with_adapters(vec![Arc::new(
            ClaudeAgentAdapter::new_with_binary(script.to_string_lossy().into_owned()),
        )]);
        let grants = Arc::new(RecordingGrants::default());
        manager.set_xcode_shim_runtime(
            grants.clone(),
            tmp.path().join("absent.sock").to_str().unwrap(),
            shim_dir.to_str().unwrap(),
        );
        let request: ExecutionRequest = serde_json::from_value(serde_json::json!({
            "run_id": "00000000-0000-4000-8000-000000000191",
            "stage_id": "review", "agent_id": "reviewer", "provider": "claude",
            "model": null, "effort": null,
            "workspace_root": repository, "worktree_root": worktree,
            "worktree_write_enabled": false,
            "worktree_strategy": "shared_implementation_worktree",
            "prompt": "fixture prompt",
            "keep_session_alive": true,
            "session_generation_id": "root-generation-1",
            "xcode_shim_injection_signal": shim_enabled
        }))
        .unwrap();

        Self {
            _tmp: tmp,
            manager,
            grants,
            worktree,
            capture,
            request,
        }
    }

    fn observations(&self) -> Vec<serde_json::Value> {
        std::fs::read_to_string(&self.capture)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn prompt_count(&self) -> usize {
        self.observations()
            .iter()
            .filter(|event| event["method"] == "session/prompt")
            .count()
    }

    fn assert_roots(&self, shim_enabled: bool) {
        for event in self.observations() {
            assert_eq!(
                event["process_cwd"],
                self.worktree.to_str().unwrap(),
                "{event}"
            );
            if shim_enabled {
                assert_eq!(
                    event["shim_root"],
                    self.worktree.to_str().unwrap(),
                    "{event}"
                );
            } else {
                assert!(event["shim_root"].is_null(), "{event}");
            }
            if matches!(
                event["method"].as_str(),
                Some("session/new" | "session/prompt")
            ) {
                assert_eq!(
                    event["session_cwd"],
                    self.worktree.to_str().unwrap(),
                    "{event}"
                );
            }
        }
    }
}

#[tokio::test]
async fn readonly_worktree_agrees_across_process_session_reuse_and_resurrection_without_shims() {
    let fixture = RootFixture::new(false);
    let manager = &fixture.manager;
    let grants = &fixture.grants;
    let mut request = fixture.request.clone();
    let first = manager.start_session(request.clone()).await.unwrap();
    assert_eq!(first.status, AgentStatus::Completed);
    request.reuse_existing_session = true;
    let reused = manager
        .prompt_session("root-generation-1", request.clone())
        .await
        .unwrap();
    assert_eq!(reused.status, AgentStatus::Completed);
    assert!(grants.records.lock().unwrap().is_empty());
    manager.close_session("root-generation-1").await.unwrap();
    assert!(grants.records.lock().unwrap().is_empty());

    request.session_generation_id = Some("root-generation-2".into());
    request.provider_session_id = Some("fixture-worktree-session".into());
    let restored = manager
        .attach_provider_session_for_resurrection(request.clone())
        .await
        .unwrap();
    assert_eq!(
        restored.actual_provider_session_id,
        "fixture-worktree-session"
    );
    let result = manager
        .prompt_session("root-generation-2", request)
        .await
        .unwrap();
    assert_eq!(result.status, AgentStatus::Completed);
    manager.close_session("root-generation-2").await.unwrap();
    assert!(grants.records.lock().unwrap().is_empty());
    assert_eq!(*grants.prompt_activations.lock().unwrap(), 0);

    let observations = fixture.observations();
    let new_sessions: Vec<_> = observations
        .iter()
        .filter(|event| event["method"] == "session/new")
        .collect();
    assert_eq!(new_sessions.len(), 2);
    assert!(new_sessions[0]["resume_session_id"].is_null());
    assert_eq!(
        new_sessions[1]["resume_session_id"],
        "fixture-worktree-session"
    );
    let prompts: Vec<_> = observations
        .iter()
        .filter(|event| event["method"] == "session/prompt")
        .collect();
    assert_eq!(prompts.len(), 3);
    assert_eq!(prompts[0]["pid"], prompts[1]["pid"]);
    assert_ne!(prompts[1]["pid"], prompts[2]["pid"]);
    let process_ids: BTreeSet<_> = observations
        .iter()
        .map(|event| event["pid"].as_u64().unwrap())
        .collect();
    assert_eq!(process_ids.len(), 2);
    fixture.assert_roots(false);
}

#[tokio::test]
async fn readonly_worktree_shims_require_fresh_sessions_and_revoke_grants() {
    let fixture = RootFixture::new(true);
    let manager = &fixture.manager;
    let grants = &fixture.grants;
    let mut tokens = BTreeSet::new();
    for generation in ["root-generation-1", "root-generation-2"] {
        let mut request = fixture.request.clone();
        request.session_generation_id = Some(generation.into());
        let before = fixture.prompt_count();
        let result = manager.start_session(request.clone()).await.unwrap();
        assert_eq!(result.status, AgentStatus::Completed);
        assert_eq!(fixture.prompt_count(), before + 1);
        let record = {
            let records = grants.records.lock().unwrap();
            assert_eq!(records.len(), 1);
            records.values().next().unwrap().clone()
        };
        assert!(!record.active_prompt);
        assert!(tokens.insert(record.grant.token_id.clone()));
        let activations = *grants.prompt_activations.lock().unwrap();
        request.reuse_existing_session = true;
        let error = manager
            .prompt_session(generation, request)
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "headless_shim_reuse_requires_fresh_session"
        );
        assert_eq!(fixture.prompt_count(), before + 1);
        assert_eq!(*grants.prompt_activations.lock().unwrap(), activations);
        assert_eq!(
            grants.records.lock().unwrap().get(&record.grant.token_id),
            Some(&record)
        );
        manager.close_session(generation).await.unwrap();
        assert!(grants.records.lock().unwrap().is_empty());
    }

    // Resurrection attach still proves the root, but cannot prompt with a shim grant.
    let mut request = fixture.request.clone();
    request.session_generation_id = Some("root-generation-3".into());
    request.provider_session_id = Some("fixture-worktree-session".into());
    request.reuse_existing_session = true;
    let restored = manager
        .attach_provider_session_for_resurrection(request.clone())
        .await
        .unwrap();
    assert_eq!(
        restored.actual_provider_session_id,
        "fixture-worktree-session"
    );
    let record = {
        let records = grants.records.lock().unwrap();
        assert_eq!(records.len(), 1);
        records.values().next().unwrap().clone()
    };
    assert!(!record.active_prompt);
    assert!(tokens.insert(record.grant.token_id.clone()));
    assert_eq!(fixture.prompt_count(), 2);
    let error = manager
        .prompt_session("root-generation-3", request)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "headless_shim_reuse_requires_fresh_session"
    );
    assert_eq!(fixture.prompt_count(), 2);
    assert_eq!(*grants.prompt_activations.lock().unwrap(), 2);
    assert_eq!(
        grants.records.lock().unwrap().get(&record.grant.token_id),
        Some(&record)
    );
    manager.close_session("root-generation-3").await.unwrap();
    assert!(grants.records.lock().unwrap().is_empty());

    let observations = fixture.observations();
    let new_sessions: Vec<_> = observations
        .iter()
        .filter(|event| event["method"] == "session/new")
        .collect();
    assert_eq!(new_sessions.len(), 3);
    assert!(new_sessions[0]["resume_session_id"].is_null());
    assert!(new_sessions[1]["resume_session_id"].is_null());
    assert_eq!(
        new_sessions[2]["resume_session_id"],
        "fixture-worktree-session"
    );
    let process_ids: BTreeSet<_> = new_sessions
        .iter()
        .map(|event| event["pid"].as_u64().unwrap())
        .collect();
    assert_eq!(process_ids.len(), 3);
    fixture.assert_roots(true);
}
