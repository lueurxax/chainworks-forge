use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use workflow::compiler;

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct FixtureDir(PathBuf);

impl FixtureDir {
    fn new() -> Self {
        let unique = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "chainworks_agent_context_skills_{}_{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&path).expect("fixture directory should be created");
        Self(path)
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.0.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture parent should be created");
        }
        fs::write(&path, content).expect("fixture should be written");
        path
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn workflow_yaml() -> &'static str {
    r#"
workflow:
  id: agent-context-skills-test
  family: agent_context_skills_test
initial_state: only_state
states:
  only_state:
    label: Only State
    type: end
    owner: lead
"#
}

fn catalog_yaml(skill_ref: &str, extra_top_level: &str) -> String {
    format!(
        r#"
schema_version: 1
{extra_top_level}permission_profiles:
  ORCH: {{}}
contracts:
  LeadResolutionContract:
    format: json
    required_fields: [resolution_mode]
backend_profiles:
  test:
    provider: codex
    model: test-model
skills:
  test_skill:
    type: external_skill
    path: skills/test-skill
agents:
  - id: lead
    system_role: lead
    backend_profile: test
    permission_profile: ORCH
    lead_resolution_contract: LeadResolutionContract
    skill_ref: {skill_ref}
    prompt: Lead the run.
"#
    )
}

fn compile_fixture(root: &FixtureDir, catalog: &str) -> anyhow::Result<workflow::plan::RunPlan> {
    let workflow = root.write("workflow.yaml", workflow_yaml());
    let catalog = root.write("catalog.yaml", catalog);
    compiler::compile(path_string(&workflow), path_string(&catalog))
}

fn path_string(path: &Path) -> &str {
    path.to_str().expect("fixture path should be utf-8")
}

#[derive(Debug, Deserialize)]
struct CatalogParitySource {
    skills: BTreeMap<String, serde_yaml::Value>,
    agents: Vec<CatalogParityAgentSource>,
}

#[derive(Debug, Deserialize)]
struct CatalogParityAgentSource {
    id: String,
    #[serde(default)]
    skill_ref: Option<String>,
    #[serde(default)]
    permission_profile: Option<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    output_contract: Option<String>,
    #[serde(default)]
    worktree_policy: Option<CatalogParityWorktreePolicy>,
}

#[derive(Debug, Deserialize)]
struct CatalogParityWorktreePolicy {
    #[serde(default)]
    write_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct CatalogParityFixture {
    unrelated_skills_sha256: String,
    affected_agents: Vec<CatalogParityExpectedAgent>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct CatalogParityExpectedAgent {
    agent_id: String,
    skill_ref: String,
    permission_profile: String,
    outputs: Vec<String>,
    output_contract: String,
    worktree_write_enabled: bool,
}

#[test]
fn active_catalog_preserves_affected_contracts_and_unrelated_skill_bytes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let source: CatalogParitySource =
        serde_yaml::from_slice(&fs::read(root.join("examples/agents/agents.yaml")).unwrap())
            .unwrap();
    let fixture: CatalogParityFixture = serde_json::from_slice(
        &fs::read(root.join(
            "control-plane/crates/workflow/tests/fixtures/agent_context/catalog_parity.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let affected_skill_refs = [
        "proposal_review_router_skill",
        "code_writer_core",
        "proposal_implementation_audit",
    ];

    let mut actual = source
        .agents
        .iter()
        .filter_map(|agent| {
            let skill_ref = agent.skill_ref.as_deref()?;
            affected_skill_refs
                .contains(&skill_ref)
                .then(|| CatalogParityExpectedAgent {
                    agent_id: agent.id.clone(),
                    skill_ref: skill_ref.to_string(),
                    permission_profile: agent.permission_profile.clone().unwrap_or_default(),
                    outputs: agent.outputs.clone(),
                    output_contract: agent.output_contract.clone().unwrap_or_default(),
                    worktree_write_enabled: agent
                        .worktree_policy
                        .as_ref()
                        .is_some_and(|policy| policy.write_enabled),
                })
        })
        .collect::<Vec<_>>();
    let mut expected = fixture.affected_agents;
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected, "affected agent contracts drifted");

    for (skill_id, expected_path) in [
        (
            "proposal_review_router_skill",
            "skills/proposal-review-router",
        ),
        ("code_writer_core", "skills/code-implementation"),
        (
            "proposal_implementation_audit",
            "skills/implementation-audit",
        ),
    ] {
        let skill = serde_json::to_value(&source.skills[skill_id]).unwrap();
        assert_eq!(skill["type"], "external_skill");
        assert_eq!(skill["path"], expected_path);
    }

    let unrelated = source
        .skills
        .iter()
        .filter(|(skill_id, _)| !affected_skill_refs.contains(&skill_id.as_str()))
        .map(|(skill_id, definition)| {
            (
                skill_id.clone(),
                serde_json::to_value(definition).expect("skill definition should be JSON-safe"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let unrelated_bytes = serde_json::to_vec(&unrelated).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&unrelated_bytes)),
        fixture.unrelated_skills_sha256,
        "unrelated skill definitions changed"
    );
}

#[test]
fn initial_compile_embeds_valid_external_skill_in_catalog_v2() {
    let root = FixtureDir::new();
    let skill_md = "---\nname: test-skill\ndescription: Applies the test procedure when compiling a run.\n---\nPerform the exact test procedure.\n";
    root.write("skills/test-skill/SKILL.md", skill_md);

    let plan = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect("valid external skill should compile");
    let catalog: serde_json::Value =
        serde_json::from_str(&plan.catalog_snapshot_json).expect("snapshot should be JSON");

    assert_eq!(catalog["catalog_snapshot_format_version"], 2);
    assert_eq!(
        catalog["chainworks_compiled"]["mission_context_version"],
        "agent_mission_context_v1"
    );
    assert_eq!(
        catalog["chainworks_compiled"]["skill_bundles"]["test_skill"]["skill_md"],
        skill_md
    );
    assert_eq!(
        catalog["chainworks_compiled"]["skill_bundles"]["test_skill"]["skill_bundle_sha256"],
        format!("{:x}", Sha256::digest(skill_md.as_bytes()))
    );

    let procedure = plan.states["only_state"]
        .owner
        .resolved_skill
        .as_ref()
        .expect("resolved procedure should be frozen");
    assert!(procedure
        .injected_content
        .contains("Perform the exact test procedure."));
    assert!(!procedure.injected_content.contains("description:"));
    assert_eq!(
        plan.states["only_state"]
            .owner
            .skill_snapshot_hash
            .as_deref()
            .map(str::len),
        Some(64)
    );
}

#[test]
fn bundle_and_role_specialization_mutations_change_declared_hashes() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Mutable test procedure.\n---\nProcedure revision one.\n",
    );
    let base = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect("base procedure should compile");
    let base_hash = base.states["only_state"]
        .owner
        .skill_snapshot_hash
        .clone()
        .unwrap();
    let base_catalog: serde_json::Value =
        serde_json::from_str(&base.catalog_snapshot_json).unwrap();
    let base_bundle_hash = base_catalog["chainworks_compiled"]["skill_bundles"]["test_skill"]
        ["skill_bundle_sha256"]
        .as_str()
        .unwrap()
        .to_string();

    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Mutable test procedure.\n---\nProcedure revision two.\n",
    );
    let mutated = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect("mutated procedure should compile");
    let mutated_catalog: serde_json::Value =
        serde_json::from_str(&mutated.catalog_snapshot_json).unwrap();
    assert_ne!(
        mutated.states["only_state"]
            .owner
            .skill_snapshot_hash
            .as_deref(),
        Some(base_hash.as_str())
    );
    assert_ne!(
        mutated_catalog["chainworks_compiled"]["skill_bundles"]["test_skill"]
            ["skill_bundle_sha256"]
            .as_str(),
        Some(base_bundle_hash.as_str())
    );

    let role_catalog = |role: &str| {
        format!(
            r#"
schema_version: 1
permission_profiles:
  ORCH: {{}}
contracts:
  LeadResolutionContract:
    format: json
    required_fields: [resolution_mode]
backend_profiles:
  test:
    provider: codex
    model: test-model
skills:
  proposal_review_router_skill:
    type: inline_skill
    description: Review the proposal.
agents:
  - id: lead
    system_role: lead
    backend_profile: test
    permission_profile: ORCH
    lead_resolution_contract: LeadResolutionContract
    skill_ref: proposal_review_router_skill
    skill_role: {role}
    prompt: Lead the run.
"#
        )
    };
    let architect = compile_fixture(&root, &role_catalog("architect"))
        .expect("architect specialization should compile");
    let product_owner = compile_fixture(&root, &role_catalog("product_owner"))
        .expect("product-owner specialization should compile");
    assert_ne!(
        architect.states["only_state"].owner.skill_snapshot_hash,
        product_owner.states["only_state"].owner.skill_snapshot_hash
    );
}

#[test]
fn initial_compile_rejects_author_supplied_compiler_extension() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Test procedure.\n---\nDo the test.\n",
    );
    let injected = "chainworks_compiled:\n  schema_version: 1\n  mission_context_version: agent_mission_context_v1\n  skill_bundles: {}\n";

    let error = compile_fixture(&root, &catalog_yaml("test_skill", injected))
        .expect_err("author-owned compiler extension must be rejected")
        .to_string();

    assert!(
        error.contains("chainworks_compiled"),
        "unexpected error: {error}"
    );
}

#[test]
fn declared_unknown_skill_fails_compilation() {
    let root = FixtureDir::new();

    let error = compile_fixture(&root, &catalog_yaml("missing_skill", ""))
        .expect_err("unknown skill_ref must fail closed")
        .to_string();

    assert!(error.contains("missing_skill"), "unexpected error: {error}");
    assert!(error.contains("not found"), "unexpected error: {error}");
}

#[test]
fn frozen_v2_recompile_uses_embedded_skill_after_source_is_removed() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Stable frozen procedure.\n---\nUse frozen bytes only.\n",
    );
    let workflow = root.write("workflow.yaml", workflow_yaml());
    let catalog = root.write("catalog.yaml", &catalog_yaml("test_skill", ""));
    let initial = compiler::compile(path_string(&workflow), path_string(&catalog))
        .expect("initial V2 compile should pass");
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Mutated live procedure.\n---\nDo not use these changed bytes.\n",
    );
    let changed_source = compiler::compile_from_snapshot_json(
        &initial.workflow_snapshot_json,
        &initial.catalog_snapshot_json,
        path_string(&catalog),
    )
    .expect("V2 snapshot must ignore changed source bundle bytes");
    assert_eq!(
        changed_source.states["only_state"]
            .owner
            .resolved_skill
            .as_ref()
            .map(|skill| &skill.injected_content),
        initial.states["only_state"]
            .owner
            .resolved_skill
            .as_ref()
            .map(|skill| &skill.injected_content)
    );
    fs::remove_dir_all(root.0.join("skills")).expect("source bundle should be removed");

    let frozen = compiler::compile_from_snapshot_json(
        &initial.workflow_snapshot_json,
        &initial.catalog_snapshot_json,
        path_string(&catalog),
    )
    .expect("V2 snapshot must not re-read the source bundle");

    assert_eq!(
        frozen.mission_context_version.as_deref(),
        Some("agent_mission_context_v1")
    );
    assert_eq!(
        frozen.states["only_state"].owner.skill_snapshot_hash,
        initial.states["only_state"].owner.skill_snapshot_hash
    );
    assert_eq!(
        frozen.states["only_state"]
            .owner
            .resolved_skill
            .as_ref()
            .map(|skill| &skill.injected_content),
        initial.states["only_state"]
            .owner
            .resolved_skill
            .as_ref()
            .map(|skill| &skill.injected_content)
    );
}

#[test]
fn frozen_v2_rejects_corrupted_bundle_digest() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Stable frozen procedure.\n---\nUse frozen bytes only.\n",
    );
    let plan = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect("initial V2 compile should pass");
    let mut catalog: serde_json::Value =
        serde_json::from_str(&plan.catalog_snapshot_json).expect("snapshot should parse");
    catalog["chainworks_compiled"]["skill_bundles"]["test_skill"]["skill_bundle_sha256"] =
        serde_json::json!("0".repeat(64));

    let error = compiler::compile_from_snapshot_json(
        &plan.workflow_snapshot_json,
        &serde_json::to_string(&catalog).expect("snapshot should serialize"),
        path_string(&root.0.join("catalog.yaml")),
    )
    .expect_err("corrupted embedded bundle must fail closed");
    let error = format!("{error:#}");

    assert!(
        error.contains("digest mismatch"),
        "unexpected error: {error}"
    );
}

#[test]
fn frozen_catalog_accepts_legacy_absent_and_v1_without_extension() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Legacy procedure source.\n---\nLegacy prompt bytes.\n",
    );
    let plan = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect("initial V2 compile should pass");
    let original: serde_json::Value =
        serde_json::from_str(&plan.catalog_snapshot_json).expect("snapshot should parse");

    for version in [None, Some(1)] {
        let mut legacy = original.clone();
        let object = legacy.as_object_mut().expect("catalog should be an object");
        object.remove("chainworks_compiled");
        match version {
            Some(version) => {
                object.insert("catalog_snapshot_format_version".into(), version.into());
            }
            None => {
                object.remove("catalog_snapshot_format_version");
            }
        }
        let compiled = compiler::compile_from_snapshot_json(
            &plan.workflow_snapshot_json,
            &serde_json::to_string(&legacy).expect("legacy snapshot should serialize"),
            path_string(&root.0.join("catalog.yaml")),
        )
        .expect("supported legacy snapshot should compile");
        assert_eq!(compiled.mission_context_version, None);
        assert!(compiled.states["only_state"]
            .owner
            .resolved_skill
            .as_ref()
            .expect("legacy external skill should resolve")
            .injected_content
            .contains("description: Legacy procedure source."));
    }
}

#[test]
fn frozen_catalog_rejects_mixed_and_unknown_versions() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Frozen procedure.\n---\nDo work.\n",
    );
    let plan = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect("initial V2 compile should pass");
    let original: serde_json::Value =
        serde_json::from_str(&plan.catalog_snapshot_json).expect("snapshot should parse");

    let mut mixed = original.clone();
    mixed
        .as_object_mut()
        .expect("catalog should be an object")
        .remove("catalog_snapshot_format_version");
    let mixed_error = compiler::compile_from_snapshot_json(
        &plan.workflow_snapshot_json,
        &serde_json::to_string(&mixed).unwrap(),
        path_string(&root.0.join("catalog.yaml")),
    )
    .expect_err("extension without V2 must fail")
    .to_string();
    assert!(mixed_error.contains("frozen_snapshot_contract_incompatible"));

    let mut unknown = original;
    unknown["catalog_snapshot_format_version"] = serde_json::json!(3);
    let unknown_error = compiler::compile_from_snapshot_json(
        &plan.workflow_snapshot_json,
        &serde_json::to_string(&unknown).unwrap(),
        path_string(&root.0.join("catalog.yaml")),
    )
    .expect_err("unknown snapshot version must fail")
    .to_string();
    assert!(unknown_error.contains("unsupported catalog snapshot format version 3"));
}

#[test]
fn frozen_catalog_version_extension_matrix_is_exhaustive() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Matrix procedure.\n---\nDo work.\n",
    );
    let plan = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect("initial V2 compile should pass");
    let v2: serde_json::Value = serde_json::from_str(&plan.catalog_snapshot_json).unwrap();
    let extension = v2["chainworks_compiled"].clone();

    for version in [None, Some(1), Some(2), Some(3)] {
        for has_extension in [false, true] {
            let mut candidate = v2.clone();
            let object = candidate.as_object_mut().unwrap();
            match version {
                Some(version) => {
                    object.insert("catalog_snapshot_format_version".into(), version.into());
                }
                None => {
                    object.remove("catalog_snapshot_format_version");
                }
            }
            if has_extension {
                object.insert("chainworks_compiled".into(), extension.clone());
            } else {
                object.remove("chainworks_compiled");
            }
            let result = compiler::compile_from_snapshot_json(
                &plan.workflow_snapshot_json,
                &serde_json::to_string(&candidate).unwrap(),
                path_string(&root.0.join("catalog.yaml")),
            );
            let should_pass = matches!(
                (version, has_extension),
                (None, false) | (Some(1), false) | (Some(2), true)
            );
            assert_eq!(
                result.is_ok(),
                should_pass,
                "unexpected matrix result for version {version:?}, extension={has_extension}: {result:?}"
            );
        }
    }

    let mut malformed = v2;
    malformed["chainworks_compiled"]["mission_context_version"] =
        serde_json::json!("unknown_context_version");
    assert!(compiler::compile_from_snapshot_json(
        &plan.workflow_snapshot_json,
        &serde_json::to_string(&malformed).unwrap(),
        path_string(&root.0.join("catalog.yaml")),
    )
    .is_err());
}

#[test]
fn strict_bundle_rejects_auxiliary_entry_and_allowed_tools() {
    let root = FixtureDir::new();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Strict procedure.\n---\nDo work.\n",
    );
    root.write("skills/test-skill/extra.md", "not allowed\n");
    let extra_error = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect_err("auxiliary bundle entries must fail");
    let extra_error = format!("{extra_error:#}");
    assert!(
        extra_error.contains("exactly one"),
        "unexpected error: {extra_error}"
    );

    fs::remove_file(root.0.join("skills/test-skill/extra.md")).unwrap();
    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Strict procedure.\nallowed-tools: Read\n---\nDo work.\n",
    );
    let tools_error = compile_fixture(&root, &catalog_yaml("test_skill", ""))
        .expect_err("allowed-tools must fail");
    let tools_error = format!("{tools_error:#}");
    assert!(
        tools_error.contains("allowed-tools"),
        "unexpected error: {tools_error}"
    );
}

#[test]
fn strict_bundle_rejects_oversized_malformed_and_non_utf8_documents() {
    let root = FixtureDir::new();
    let oversized = format!(
        "---\nname: test-skill\ndescription: Oversized procedure.\n---\n{}",
        "x".repeat(65_536)
    );
    root.write("skills/test-skill/SKILL.md", &oversized);
    let error = format!(
        "{:#}",
        compile_fixture(&root, &catalog_yaml("test_skill", ""))
            .expect_err("oversized SKILL.md must fail")
    );
    assert!(
        error.contains("exceeds 65536 bytes"),
        "unexpected error: {error}"
    );

    root.write(
        "skills/test-skill/SKILL.md",
        "---\nname: test-skill\ndescription: Unclosed procedure.\nDo work.\n",
    );
    let error = format!(
        "{:#}",
        compile_fixture(&root, &catalog_yaml("test_skill", ""))
            .expect_err("unclosed frontmatter must fail")
    );
    assert!(
        error.contains("frontmatter is not closed"),
        "unexpected error: {error}"
    );

    fs::write(
        root.0.join("skills/test-skill/SKILL.md"),
        b"---\nname: test-skill\ndescription: Invalid UTF-8.\n---\n\xff\n",
    )
    .unwrap();
    let error = format!(
        "{:#}",
        compile_fixture(&root, &catalog_yaml("test_skill", ""))
            .expect_err("non-UTF-8 SKILL.md must fail")
    );
    assert!(error.contains("valid UTF-8"), "unexpected error: {error}");
}

#[cfg(unix)]
#[test]
fn strict_bundle_rejects_symlinked_skill_file_and_parent_escape() {
    use std::os::unix::fs::symlink;

    let root = FixtureDir::new();
    let outside = root.write(
        "outside.md",
        "---\nname: test-skill\ndescription: Outside procedure.\n---\nDo work.\n",
    );
    fs::create_dir_all(root.0.join("skills/test-skill")).unwrap();
    symlink(&outside, root.0.join("skills/test-skill/SKILL.md")).unwrap();
    assert!(compile_fixture(&root, &catalog_yaml("test_skill", "")).is_err());

    let escaping_catalog =
        catalog_yaml("test_skill", "").replace("path: skills/test-skill", "path: ../outside-skill");
    let escape_error =
        compile_fixture(&root, &escaping_catalog).expect_err("parent traversal must fail");
    let escape_error = format!("{escape_error:#}");
    assert!(
        escape_error.contains("may not contain"),
        "unexpected error: {escape_error}"
    );
}
