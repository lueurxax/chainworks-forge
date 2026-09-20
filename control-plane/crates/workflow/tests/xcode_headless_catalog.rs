use serde_json::{json, Value};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn catalog() -> Value {
    serde_json::to_value(
        workflow::catalog::load(root().join("examples/agents/agents.yaml").to_str().unwrap())
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn current_catalog_has_explicit_headless_capabilities_only_for_selected_profiles() {
    let catalog = catalog();
    for name in ["RO_VERIFY", "CODE_WRITE"] {
        assert_eq!(
            catalog["permission_profiles"][name]["xcode_headless"],
            json!({"read":true,"gates":["build","fast","full","guardrails","list"]}),
            "{name}"
        );
        let commands = catalog["permission_profiles"][name]["shell"]["allow"]
            .as_array()
            .unwrap();
        assert!(commands.contains(&json!("./scripts/test-gate.sh build")));
        assert!(commands.contains(&json!("./scripts/test-gate.sh fast")));
        assert!(!commands
            .iter()
            .any(|value| value.as_str().unwrap_or("").contains("xcodebuild")));
    }
    for name in ["RO_REVIEW", "PROPOSAL_WRITE", "DOC_WRITE"] {
        assert_eq!(
            catalog["permission_profiles"][name]["xcode_headless"],
            json!({"read":true,"gates":[]}),
            "{name}"
        );
    }
    for name in [
        "ORCH",
        "RO_PREPUSH_VERIFY",
        "RELEASE_GIT",
        "RELEASE_PUBLISH",
    ] {
        assert!(
            catalog["permission_profiles"][name]
                .get("xcode_headless")
                .is_none(),
            "{name}"
        );
    }
    for id in ["code_writer", "proposal_implementation_auditor"] {
        let agent = catalog["agents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|agent| agent["id"] == id)
            .unwrap();
        let tools = agent["required_tools"].as_array().unwrap();
        assert!(tools.contains(&json!("./scripts/test-gate.sh build")));
        assert!(tools.contains(&json!("./scripts/test-gate.sh fast")));
        assert!(!tools
            .iter()
            .any(|value| value.as_str().unwrap_or("").contains("xcodebuild")));
    }
}

#[test]
fn canonical_gate_profile_and_required_tools_keep_the_host_signal() {
    let catalog_path = root().join("examples/agents/agents.yaml");
    let catalog = workflow::catalog::load(catalog_path.to_str().unwrap()).unwrap();
    let raw = serde_yaml::to_value(&catalog).unwrap();
    let workflow_raw: serde_yaml::Value =
        serde_yaml::from_str("initial_state: start\nstates: {}\n").unwrap();
    let definition = serde_yaml::from_value(workflow_raw.clone()).unwrap();
    let scan = workflow::direct_command::scan_catalog(&catalog, &definition, &workflow_raw, &raw);
    scan.ensure_no_errors().unwrap();
    for (id, profile) in [
        ("code_writer", "CODE_WRITE"),
        ("proposal_implementation_auditor", "RO_VERIFY"),
    ] {
        let signals = scan.signals_for_agent(id, Some(profile));
        assert!(signals.xcode_shim_injection_signal);
        assert!(signals.requires_xcode_host_execution);
        assert!(scan.declarations.iter().any(|declaration| declaration
            .source_path
            .starts_with(&format!("agents.{id}.required_tools"))
            && declaration.matched_xcode_tool.as_deref() == Some("chainworks-test-gate")));
    }
}

#[test]
fn new_run_snapshot_freezes_explicit_capabilities_without_changing_historical_snapshots() {
    let root = root();
    let plan = workflow::compiler::compile(
        root.join("examples/workflows/full-mvp-live.yaml")
            .to_str()
            .unwrap(),
        root.join("examples/agents/agents.yaml").to_str().unwrap(),
    )
    .unwrap();
    let frozen: Value = serde_json::from_str(&plan.catalog_snapshot_json).unwrap();
    assert_eq!(
        frozen["permission_profiles"]["CODE_WRITE"]["xcode_headless"],
        json!({"read":true,"gates":["build","fast","full","guardrails","list"]})
    );
    let old: Value = serde_json::from_str(include_str!(
        "fixtures/agent_context/security_prepush_catalog_v2.json"
    ))
    .unwrap();
    let old_profile = &old["catalog_snapshot"]["permission_profiles"]["RO_VERIFY"];
    assert!(old_profile.is_object());
    assert!(old_profile.get("xcode_headless").is_none());
}
