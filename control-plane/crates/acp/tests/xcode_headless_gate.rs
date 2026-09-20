#![cfg(unix)]

use acp::{
    dispatch_headless_canonical_gate, pin_headless_canonical_gate, NoopXcodeRuntimeObservationSink,
    XcodeHostExecutorPlanInput, XcodeHostExecutorProcessConfig, XcodeShimDispatchAttempt,
    XcodeShimDispatchGrant, XcodeShimDispatchRequest, XcodeShimProcessBinding,
};
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt, path::Path, time::Duration};

fn script(root: &Path, body: &str) {
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    let path = root.join("scripts/test-gate.sh");
    std::fs::write(&path, format!("#!/bin/bash\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn request(root: &Path) -> XcodeShimDispatchRequest {
    let peer = XcodeShimProcessBinding {
        pid: std::process::id(),
        uid: acp::current_process_uid(),
        parent_pid: None,
        ancestor_pids: vec![],
        start_time_fingerprint: None,
        executable_fingerprint: None,
    };
    let now = chrono::Utc::now().timestamp_millis();
    XcodeShimDispatchRequest {
        agent_execution_id: Some(domain::ids::AgentExecutionId::new()),
        grant: XcodeShimDispatchGrant::new(
            "fixture",
            "secret",
            "lease",
            peer.clone(),
            now - 1000,
            now + 60000,
        ),
        attempt: XcodeShimDispatchAttempt {
            token_id: "fixture".into(),
            token_secret: "secret".into(),
            peer_process: peer,
            now_epoch_ms: now,
            active_prompt: true,
        },
        plan_input: XcodeHostExecutorPlanInput {
            invoked_tool: "chainworks-test-gate".into(),
            args: vec!["build".into()],
            cwd: root.to_str().unwrap().into(),
            workspace_root: root.to_str().unwrap().into(),
            provider_env: BTreeMap::new(),
            simulator_candidates: vec![],
            toolchain_mapping_root: None,
        },
    }
}

#[tokio::test]
async fn pinned_gate_runs_exact_script_args_and_clears_recursive_authority() {
    let root = tempfile::tempdir().unwrap();
    script(root.path(), "set -e; printf '%s\\n' \"$PWD\" \"$@\"; test -z \"${CHAINWORKS_XCODE_SHIM_TOKEN:-}\"; test -z \"${CHAINWORKS_TEST_GATE_ROOT_DIR:-}\"; exit 7");
    let mut request = request(root.path());
    request
        .plan_input
        .provider_env
        .insert("CHAINWORKS_XCODE_SHIM_TOKEN".into(), "must-not-leak".into());
    request
        .plan_input
        .provider_env
        .insert("CHAINWORKS_TEST_GATE_ROOT_DIR".into(), "/wrong".into());
    let identity = pin_headless_canonical_gate(root.path(), &request.plan_input.args).unwrap();
    let config = XcodeHostExecutorProcessConfig {
        tool_paths: BTreeMap::from([("chainworks-test-gate".into(), "/bin/false".into())]),
        timeout: Duration::from_secs(3),
    };
    let outcome = dispatch_headless_canonical_gate(
        request,
        &identity,
        &config,
        &NoopXcodeRuntimeObservationSink,
    )
    .await
    .unwrap();
    assert_eq!(outcome.exit_status, 7);
    assert_eq!(
        outcome.reason_code.as_deref(),
        Some("xcode_gate_nonzero_exit")
    );
    let output = outcome.process_output.unwrap();
    assert!(output.process_group_settled);
    assert_eq!(
        output.stdout,
        format!("{}\nbuild\n", root.path().canonicalize().unwrap().display())
    );
}

#[tokio::test]
async fn script_drift_and_raw_tools_never_dispatch() {
    let root = tempfile::tempdir().unwrap();
    script(root.path(), "touch ran");
    let request = request(root.path());
    let identity = pin_headless_canonical_gate(root.path(), &request.plan_input.args).unwrap();
    script(root.path(), "touch changed");
    assert!(dispatch_headless_canonical_gate(
        request.clone(),
        &identity,
        &XcodeHostExecutorProcessConfig::default(),
        &NoopXcodeRuntimeObservationSink
    )
    .await
    .is_err());
    let identity = pin_headless_canonical_gate(root.path(), &request.plan_input.args).unwrap();
    for tool in [
        "xcodebuild",
        "xcrun",
        "simctl",
        "mcpbridge",
        "sh",
        "/tmp/chainworks-test-gate",
    ] {
        let mut request = request.clone();
        request.plan_input.invoked_tool = tool.into();
        assert!(dispatch_headless_canonical_gate(
            request,
            &identity,
            &XcodeHostExecutorProcessConfig::default(),
            &NoopXcodeRuntimeObservationSink
        )
        .await
        .is_err());
    }
    assert!(!root.path().join("ran").exists());
    assert!(!root.path().join("changed").exists());
}

#[test]
fn gate_schema_is_closed_and_argument_identity_is_exact() {
    let root = tempfile::tempdir().unwrap();
    script(root.path(), "exit 0");
    for args in [
        vec!["build; touch ran".into()],
        vec!["not-a-gate".into()],
        vec!["build".into(), "-project".into()],
        vec!["../scripts/test-gate.sh".into()],
    ] {
        assert!(pin_headless_canonical_gate(root.path(), &args).is_err());
    }
    for gate in [
        "ui-smoke",
        "xcode-headless-catalog",
        "xcode-headless-project-trust",
    ] {
        assert!(pin_headless_canonical_gate(root.path(), &[gate.into()]).is_ok());
    }
}

#[tokio::test]
async fn gate_timeout_is_distinct_and_output_is_bounded() {
    let root = tempfile::tempdir().unwrap();
    script(
        root.path(),
        "while :; do printf '012345678901234567890123456789\\n'; done",
    );
    let request = request(root.path());
    let identity = pin_headless_canonical_gate(root.path(), &request.plan_input.args).unwrap();
    let config = XcodeHostExecutorProcessConfig {
        tool_paths: BTreeMap::new(),
        timeout: Duration::from_millis(150),
    };
    let outcome = dispatch_headless_canonical_gate(
        request,
        &identity,
        &config,
        &NoopXcodeRuntimeObservationSink,
    )
    .await
    .unwrap();
    assert_eq!(outcome.reason_code.as_deref(), Some("xcode_gate_timeout"));
    assert_eq!(outcome.exit_status, 124);
    let output = outcome.process_output.unwrap();
    assert!(!output.process_group_settled);
    assert!(output.stdout.len() <= 65536 + 64);
}

#[test]
fn canonical_script_routes_before_bootstrap_and_preserves_exact_argv() {
    let root = tempfile::tempdir().unwrap();
    let installed = root.path().join("chainworks-test-gate");
    std::fs::write(&installed, "#!/bin/bash\nprintf '<%s>\\n' \"$@\"\nexit 9\n").unwrap();
    std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o700)).unwrap();
    let canonical = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/test-gate.sh");
    let output = std::process::Command::new("/bin/bash")
        .arg(&canonical)
        .args(["build", "argument with spaces", ""])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CHAINWORKS_TEST_GATE_ROOT_DIR", "/must-not-bootstrap")
        .env("CHAINWORKS_XCODE_SHIM_SOCKET", "/fixture/socket")
        .env("CHAINWORKS_XCODE_SHIM_TOKEN_ID", "fixture")
        .env("CHAINWORKS_XCODE_SHIM_TOKEN", "secret")
        .env("CHAINWORKS_XCODE_SHIM_DIR", root.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(9));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "<build>\n<argument with spaces>\n<>\n"
    );
    let output = std::process::Command::new("/bin/bash")
        .arg(canonical)
        .arg("build")
        .env_clear()
        .env("CHAINWORKS_XCODE_SHIM_TOKEN", "partial")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(126));
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("xcode_gate_shim_credentials_or_executable_missing"));
}

async fn wait_pid_file(path: &Path) -> i32 {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Ok(value) = std::fs::read_to_string(path) {
                if let Ok(pid) = value.trim().parse() {
                    return pid;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture process started")
}

async fn assert_process_gone(pid: i32) {
    tokio::time::timeout(Duration::from_secs(4), async {
        while unsafe { libc::kill(pid, 0) } == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture process cleaned up");
}

#[tokio::test]
async fn detached_stdio_descendants_prevent_settlement_and_are_cleaned() {
    let root = tempfile::tempdir().unwrap();
    script(
        root.path(),
        "/bin/sleep 30 >/dev/null 2>&1 &\necho $! > child-pid\nexit 0",
    );
    let request = request(root.path());
    let identity = pin_headless_canonical_gate(root.path(), &request.plan_input.args).unwrap();
    let config = XcodeHostExecutorProcessConfig {
        tool_paths: BTreeMap::new(),
        timeout: Duration::from_secs(2),
    };
    let task = tokio::spawn(async move {
        dispatch_headless_canonical_gate(
            request,
            &identity,
            &config,
            &NoopXcodeRuntimeObservationSink,
        )
        .await
    });
    let pid = wait_pid_file(&root.path().join("child-pid")).await;
    assert!(
        !task.is_finished(),
        "live descendant must prevent settlement"
    );
    let outcome = task.await.unwrap().unwrap();
    assert_eq!(outcome.reason_code.as_deref(), Some("xcode_gate_timeout"));
    assert!(!outcome.process_output.unwrap().process_group_settled);
    assert_process_gone(pid).await;
}

#[tokio::test]
async fn cancelling_dispatch_driver_stops_descendants_and_reaps_leader() {
    let root = tempfile::tempdir().unwrap();
    script(
        root.path(),
        "echo $$ > leader-pid\nsleep 30 &\necho $! > child-pid\nwait",
    );
    let request = request(root.path());
    let identity = pin_headless_canonical_gate(root.path(), &request.plan_input.args).unwrap();
    let task = tokio::spawn(async move {
        dispatch_headless_canonical_gate(
            request,
            &identity,
            &XcodeHostExecutorProcessConfig::default(),
            &NoopXcodeRuntimeObservationSink,
        )
        .await
    });
    let leader = wait_pid_file(&root.path().join("leader-pid")).await;
    let child = wait_pid_file(&root.path().join("child-pid")).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_process_gone(leader).await;
    assert_process_gone(child).await;
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(leader, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[tokio::test]
async fn changed_root_arguments_and_replaced_inode_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    script(root.path(), "touch ran");
    script(other.path(), "touch ran");
    let original = request(root.path());
    let identity = pin_headless_canonical_gate(root.path(), &original.plan_input.args).unwrap();
    let mut wrong_root = original.clone();
    wrong_root.plan_input.workspace_root = other.path().to_string_lossy().into_owned();
    let mut wrong_args = original.clone();
    wrong_args.plan_input.args = vec!["fast".into()];
    for invalid in [wrong_root, wrong_args] {
        assert!(dispatch_headless_canonical_gate(
            invalid,
            &identity,
            &XcodeHostExecutorProcessConfig::default(),
            &NoopXcodeRuntimeObservationSink
        )
        .await
        .is_err());
    }
    std::fs::rename(
        root.path().join("scripts/test-gate.sh"),
        root.path().join("old-gate"),
    )
    .unwrap();
    script(root.path(), "touch ran");
    assert!(dispatch_headless_canonical_gate(
        original,
        &identity,
        &XcodeHostExecutorProcessConfig::default(),
        &NoopXcodeRuntimeObservationSink
    )
    .await
    .is_err());
    assert!(!root.path().join("ran").exists());
    assert!(!other.path().join("ran").exists());
}
