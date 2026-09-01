use std::fs;
use std::path::{Path, PathBuf};

use workflow::compiler::compile_for_new_run_v1;

const WORKFLOW: &str = r#"workflow:
  id: codex-planned-variant-test
  family: codex_planned_variant_test
initial_state: work
states:
  work:
    label: Work
    owner: orchestrator
    run:
      parallel:
        - agent: architect
          task: Architecture
        - agent: auditor
          task: Audit
        - agent: writer
          task: Write
        - agent: builder
          task: Build
        - agent: routine_orchestrator
          task: Coordinate
        - agent: operator
          task: Operate
"#;

const CATALOG: &str = r#"schema_version: 1
backend_profiles:
  codex_orchestrator_high:
    provider: codex_acp
    model: gpt-5.6-sol
    effort: max
  codex_architect_high:
    provider: codex_acp
    model: gpt-5.6-sol
    effort: xhigh
  codex_audit_high:
    provider: codex_acp
    model: gpt-5.6-sol
    effort: ultra
  codex_writer_high:
    provider: codex_acp
    model: gpt-5.6-terra
    effort: high
  codex_builder_high:
    provider: codex_acp
    model: gpt-5.6-terra
    effort: high
  codex_orchestrator_acp:
    provider: codex_acp
    model: gpt-5.6-terra
    effort: high
  codex_ops_low:
    provider: codex_acp
    model: gpt-5.6-luna
    effort: high
permission_profiles:
  TEST: {}
contracts:
  LeadResolutionContract:
    format: json
    required_fields: [resolution_mode]
agents:
  - id: orchestrator
    system_role: lead
    backend_profile: codex_orchestrator_high
    permission_profile: TEST
    lead_resolution_contract: LeadResolutionContract
  - id: architect
    backend_profile: codex_architect_high
    permission_profile: TEST
  - id: auditor
    backend_profile: codex_audit_high
    permission_profile: TEST
  - id: writer
    backend_profile: codex_writer_high
    permission_profile: TEST
  - id: builder
    backend_profile: codex_builder_high
    permission_profile: TEST
  - id: routine_orchestrator
    backend_profile: codex_orchestrator_acp
    permission_profile: TEST
  - id: operator
    backend_profile: codex_ops_low
    permission_profile: TEST
"#;

fn policy_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../examples/agents/codex-model-variant-matrix.v1.json")
}

fn write_sources(catalog: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let workflow = root.path().join("workflow.yaml");
    let catalog_path = root.path().join("agents.yaml");
    fs::write(&workflow, WORKFLOW).unwrap();
    fs::write(&catalog_path, catalog).unwrap();
    fs::copy(
        policy_fixture(),
        root.path().join("codex-model-variant-matrix.v1.json"),
    )
    .unwrap();
    (root, workflow, catalog_path)
}

fn compile(catalog: &str) -> anyhow::Result<workflow::compiler::NewRunAdmissionV1> {
    let (_root, workflow, catalog_path) = write_sources(catalog);
    compile_for_new_run_v1(path(&workflow), path(&catalog_path))
}

fn path(value: &Path) -> &str {
    value.to_str().unwrap()
}

#[test]
fn canonical_production_sources_satisfy_new_run_admission() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let workflow = root.join("examples/workflows/full-mvp-live.yaml");
    let catalog = root.join("examples/agents/agents.yaml");
    compile_for_new_run_v1(path(&workflow), path(&catalog))
        .expect("canonical production sources must satisfy the pinned matrix");
}

#[test]
fn new_run_admission_freezes_all_seven_canonical_bindings() {
    let admission = compile(CATALOG).expect("approved matrix must compile");
    let plan = admission.plan();
    let state = plan.states.get("work").unwrap();
    let bindings = std::iter::once(&state.owner)
        .chain(state.tasks.iter().map(|task| &task.agent))
        .map(|agent| {
            (
                agent.backend_profile_id.as_deref().unwrap(),
                agent.provider.as_str(),
                agent.model.as_deref().unwrap(),
                agent.effort.as_deref().unwrap(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(bindings.len(), 7);
    assert!(bindings
        .iter()
        .all(|(_, provider, _, _)| *provider == "codex"));
    assert!(bindings.contains(&("codex_orchestrator_high", "codex", "gpt-5.6-sol", "max")));
    assert!(bindings.contains(&("codex_architect_high", "codex", "gpt-5.6-sol", "xhigh")));
    assert!(bindings.contains(&("codex_audit_high", "codex", "gpt-5.6-sol", "ultra")));
    assert!(bindings.contains(&("codex_writer_high", "codex", "gpt-5.6-terra", "high")));
    assert!(bindings.contains(&("codex_builder_high", "codex", "gpt-5.6-terra", "high")));
    assert!(bindings.contains(&("codex_orchestrator_acp", "codex", "gpt-5.6-terra", "high")));
    assert!(bindings.contains(&("codex_ops_low", "codex", "gpt-5.6-luna", "high")));
}

#[test]
fn new_run_admission_rejects_duplicate_root_and_nested_yaml_keys() {
    let duplicate_root = format!("{CATALOG}schema_version: 1\n");
    let error = compile(&duplicate_root)
        .expect_err("duplicate root key must fail")
        .to_string();
    assert!(error.contains("duplicate YAML mapping key"), "{error}");

    let duplicate_nested = CATALOG.replacen(
        "    model: gpt-5.6-sol\n    effort: max",
        "    model: gpt-5.6-sol\n    model: gpt-5.6-terra\n    effort: max",
        1,
    );
    let error = compile(&duplicate_nested)
        .expect_err("duplicate nested key must fail")
        .to_string();
    assert!(error.contains("duplicate YAML mapping key"), "{error}");
}

#[test]
fn new_run_admission_rejects_every_reserved_matrix_shape_mutation() {
    let cases = [
        (
            "generic model",
            CATALOG.replacen("model: gpt-5.6-sol", "model: gpt-5.6", 1),
        ),
        (
            "wrong effort",
            CATALOG.replacen("effort: max", "effort: medium", 1),
        ),
        (
            "wrong authored provider",
            CATALOG.replacen("provider: codex_acp", "provider: codex", 1),
        ),
        (
            "missing reserved profile",
            CATALOG.replacen(
                "  codex_ops_low:\n    provider: codex_acp\n    model: gpt-5.6-luna\n    effort: high\n",
                "",
                1,
            ),
        ),
        (
            "extra Codex profile",
            CATALOG.replacen(
                "permission_profiles:",
                "  codex_extra:\n    provider: codex_acp\n    model: gpt-5.6-terra\n    effort: high\npermission_profiles:",
                1,
            ),
        ),
    ];

    for (name, catalog) in cases {
        let error = compile(&catalog).expect_err(name).to_string();
        assert!(
            error.contains("codex_model_variant_matrix_v1"),
            "{name}: {error}"
        );
    }
}

#[test]
fn new_run_admission_rejects_missing_or_mutated_policy_bytes() {
    let (root, workflow, catalog) = write_sources(CATALOG);
    let policy = root.path().join("codex-model-variant-matrix.v1.json");
    fs::remove_file(&policy).unwrap();
    let error = compile_for_new_run_v1(path(&workflow), path(&catalog))
        .expect_err("missing policy must fail")
        .to_string();
    assert!(error.contains("codex model variant policy"), "{error}");

    fs::write(&policy, b"{}\n").unwrap();
    let error = compile_for_new_run_v1(path(&workflow), path(&catalog))
        .expect_err("mutated policy must fail")
        .to_string();
    assert!(error.contains("policy_bytes_mismatch"), "{error}");
}

#[test]
fn verified_generic_and_custom_historical_replay_is_byte_identical() {
    let historical_catalog = CATALOG
        .replacen("model: gpt-5.6-sol", "model: gpt-5.6", 1)
        .replacen("model: gpt-5.6-sol", "model: custom-model", 1)
        .replacen("effort: xhigh", "effort: custom-effort", 1);
    let (_root, workflow_path, catalog_path) = write_sources(&historical_catalog);
    let original = workflow::compiler::compile(path(&workflow_path), path(&catalog_path))
        .expect("the compatibility compiler must retain historical generic/custom tuples");

    fs::write(&workflow_path, "not: valid: yaml").unwrap();
    fs::write(&catalog_path, "not: valid: yaml").unwrap();
    let replayed = workflow::compiler::compile_from_snapshot_json(
        &original.workflow_snapshot_json,
        &original.catalog_snapshot_json,
        path(&catalog_path),
    )
    .expect("verified replay must not re-admit frozen tuples against the current matrix");

    assert_eq!(
        replayed.workflow_snapshot_json,
        original.workflow_snapshot_json
    );
    assert_eq!(
        replayed.catalog_snapshot_json,
        original.catalog_snapshot_json
    );
    assert_eq!(
        replayed.workflow_snapshot_hash,
        original.workflow_snapshot_hash
    );
    assert_eq!(
        replayed.catalog_snapshot_hash,
        original.catalog_snapshot_hash
    );
    assert_eq!(
        replayed.states["work"].owner.model.as_deref(),
        Some("gpt-5.6")
    );
    assert!(replayed.states["work"]
        .tasks
        .iter()
        .any(|task| task.agent.model.as_deref() == Some("custom-model")
            && task.agent.effort.as_deref() == Some("custom-effort")));
}
