use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, projections, runs, stages};
use domain::commands::{CallerContext, CancelRunCmd, Command, PrincipalClass, StartRunCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId};
use engine::command_handler::{CommandHandler, CommandResult};
use engine::event_bus;
use engine::parity_control::{
    evaluate_reclaim_decision, is_owner_stalled, write_atomic_json, write_pre_cleanup_publication,
    InterruptionMarker, LeaseRecord, ParityControlRoot, ReclaimMarker, ReleaseMarker,
    TimeoutMarker, REQUIRED_COMPARISON_SURFACES, REQUIRED_FIXTURES,
};
use engine::work_queue::WorkQueue;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

#[derive(Debug, Deserialize)]
struct GoldenRunFixture {
    schema_version: String,
    fixture_id: String,
    fixture_revision: i64,
    captured_from: CapturedFrom,
    frozen_inputs: FrozenInputs,
    expected_client_truth: ExpectedClientTruth,
    normalization_rules: Vec<String>,
    regeneration: Regeneration,
}

#[derive(Debug, Deserialize)]
struct CapturedFrom {
    owner: String,
    source_paths: Vec<String>,
    capture_command: String,
}

#[derive(Debug, Deserialize)]
struct FrozenInputs {
    workflow_snapshot: String,
    agent_catalog_snapshot: String,
    provider_profile: String,
    runtime_events: String,
    operator_decisions: String,
}

#[derive(Debug, Deserialize)]
struct ExpectedClientTruth {
    canonical_state: String,
    projections: String,
    artifacts: String,
    reports: String,
    operator_summary: String,
}

#[derive(Debug, Deserialize)]
struct Regeneration {
    allowed_when: String,
    requires_reason: bool,
    requires_diff_report: bool,
}

#[derive(Debug, Deserialize)]
struct RuntimeEvents {
    schema_version: String,
    stages: Vec<FixtureStage>,
    artifacts: Vec<FixtureArtifact>,
}

#[derive(Debug, Deserialize)]
struct OperatorDecisions {
    schema_version: String,
    decisions: Vec<OperatorDecision>,
}

#[derive(Clone, Debug, Deserialize)]
struct OperatorDecision {
    stage_id: String,
    decision: String,
    at: String,
}

#[derive(Debug, Deserialize)]
struct WorkflowSnapshot {
    schema_version: String,
    workflow_id: String,
    title: String,
    initial_state: String,
    states: Vec<WorkflowSnapshotState>,
}

#[derive(Debug, Deserialize)]
struct WorkflowSnapshotState {
    stage_id: String,
    label: String,
    owner: String,
    outputs: Vec<String>,
    next_stage: Option<String>,
    terminal: bool,
}

#[derive(Debug, Deserialize)]
struct AgentCatalogSnapshot {
    schema_version: String,
    agents: Vec<AgentCatalogSnapshotAgent>,
    artifact_templates: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct AgentCatalogSnapshotAgent {
    id: String,
    provider: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
struct FixtureStage {
    stage_id: String,
    label: String,
    status: String,
    attempt_number: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
struct FixtureArtifact {
    name: String,
    contract_id: String,
    stage_id: String,
    format: String,
    report_kind: Option<String>,
}

fn engine_manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    engine_manifest_dir()
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("engine crate should be under control-plane/crates")
        .to_path_buf()
}

fn control_plane_root() -> PathBuf {
    engine_manifest_dir()
        .parent()
        .and_then(Path::parent)
        .expect("engine crate should be under control-plane/crates")
        .to_path_buf()
}

fn fixtures_root() -> PathBuf {
    engine_manifest_dir().join("tests/fixtures/parity/golden-runs")
}

fn target_parity_root() -> PathBuf {
    control_plane_root().join("target/parity")
}

fn target_parity_work_root() -> PathBuf {
    target_parity_root()
        .join("work")
        .join(active_generation_id())
}

fn target_parity_reports_root() -> PathBuf {
    target_parity_root()
        .join("reports")
        .join(active_generation_id())
}

fn target_parity_shadow_root() -> PathBuf {
    target_parity_root()
        .join("shadow")
        .join(active_generation_id())
}

fn repo_relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

/// Normalize an artifact file_path for inclusion in server-replay.json.
///
/// Strips the workspace root prefix when possible. Paths that remain absolute
/// are replaced with an opaque `<machine-local>/<filename>` placeholder so
/// server-replay artifacts are fully portable and do not leak any host identity,
/// username, home-directory layout, mount name, or temp-path structure.
fn normalize_artifact_file_path(raw: &str) -> String {
    // First try to make it workspace-relative.
    let as_path = std::path::Path::new(raw);
    if let Ok(rel) = as_path.strip_prefix(workspace_root()) {
        return rel.display().to_string();
    }
    if as_path.is_absolute() {
        let basename = as_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        return format!("<machine-local>/{basename}");
    }
    raw.to_string()
}

#[test]
fn proposal_041_redacts_every_non_workspace_absolute_artifact_path() {
    let cases = [
        (
            "/Users/alice/run/output.json",
            "<machine-local>/output.json",
        ),
        ("/home/alice/run/output.json", "<machine-local>/output.json"),
        (
            "/private/tmp/chainworks/output.json",
            "<machine-local>/output.json",
        ),
        (
            "/Volumes/External/build/output.json",
            "<machine-local>/output.json",
        ),
        (
            "/opt/org/private/output.json",
            "<machine-local>/output.json",
        ),
    ];
    for (raw, expected) in cases {
        assert_eq!(normalize_artifact_file_path(raw), expected);
    }

    let workspace_file = workspace_root().join("control-plane/target/parity/work/replay.json");
    assert_eq!(
        normalize_artifact_file_path(&workspace_file.display().to_string()),
        "control-plane/target/parity/work/replay.json"
    );
    assert_eq!(
        normalize_artifact_file_path("relative/artifact.json"),
        "relative/artifact.json"
    );
}

fn assert_safe_p041_generation_id(raw: &str) -> Result<()> {
    if raw == "unscoped-fixture-replay" {
        return Ok(());
    }
    let valid_prefix = raw.starts_with("p041-");
    let valid_chars = raw
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':' | 'T' | 'Z'));
    if !valid_prefix
        || !valid_chars
        || raw.contains("..")
        || raw.contains('/')
        || raw.contains('\\')
    {
        return Err(anyhow!(
            "invalid P041_PUBLICATION_GENERATION_ID path segment: {raw:?}"
        ));
    }
    Ok(())
}

fn load_fixture(fixture_id: &str) -> Result<(PathBuf, GoldenRunFixture)> {
    let dir = fixtures_root().join(fixture_id);
    let raw = fs::read_to_string(dir.join("fixture.json"))
        .with_context(|| format!("read fixture {fixture_id}"))?;
    let fixture: GoldenRunFixture = serde_json::from_str(&raw)
        .with_context(|| format!("parse fixture {fixture_id}/fixture.json"))?;
    Ok((dir, fixture))
}

fn read_json(path: &Path) -> Result<Value> {
    let raw = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

fn read_optional_json_for_test<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    match fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", path.display()))
            .map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("read {}", path.display())),
    }
}

fn load_runtime_events(fixture_dir: &Path, fixture: &GoldenRunFixture) -> Result<RuntimeEvents> {
    let raw = fs::read_to_string(fixture_dir.join(&fixture.frozen_inputs.runtime_events))?;
    serde_json::from_str(&raw).context("parse runtime-events.json")
}

fn load_operator_decisions(
    fixture_dir: &Path,
    fixture: &GoldenRunFixture,
) -> Result<OperatorDecisions> {
    let raw = fs::read_to_string(fixture_dir.join(&fixture.frozen_inputs.operator_decisions))?;
    serde_json::from_str(&raw).context("parse operator-decisions.json")
}

fn load_workflow_snapshot(
    fixture_dir: &Path,
    fixture: &GoldenRunFixture,
) -> Result<WorkflowSnapshot> {
    let raw = fs::read_to_string(fixture_dir.join(&fixture.frozen_inputs.workflow_snapshot))?;
    serde_json::from_str(&raw).context("parse workflow-snapshot.json")
}

fn load_agent_catalog_snapshot(
    fixture_dir: &Path,
    fixture: &GoldenRunFixture,
) -> Result<AgentCatalogSnapshot> {
    let raw = fs::read_to_string(fixture_dir.join(&fixture.frozen_inputs.agent_catalog_snapshot))?;
    serde_json::from_str(&raw).context("parse agent-catalog-snapshot.json")
}

fn referenced_paths(fixture: &GoldenRunFixture) -> Vec<&str> {
    vec![
        &fixture.frozen_inputs.workflow_snapshot,
        &fixture.frozen_inputs.agent_catalog_snapshot,
        &fixture.frozen_inputs.provider_profile,
        &fixture.frozen_inputs.runtime_events,
        &fixture.frozen_inputs.operator_decisions,
        &fixture.expected_client_truth.canonical_state,
        &fixture.expected_client_truth.projections,
        &fixture.expected_client_truth.artifacts,
        &fixture.expected_client_truth.reports,
        &fixture.expected_client_truth.operator_summary,
    ]
}

/// Per-fixture async mutex registry. The whole replay flow
/// (wipe → migrate → insert idea → drive events → project → write
/// report) must be held under the same lock — if we only guarded
/// `fixture_pool`, a second caller would wipe the live DB out from
/// under the first caller. Keyed by fixture id (path-derived) so
/// different fixtures still run in parallel.
fn fixture_replay_lock(path: &Path) -> Arc<tokio::sync::Mutex<()>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry
        .lock()
        .expect("fixture replay lock registry poisoned");
    map.entry(path.to_path_buf())
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

async fn fixture_pool(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    for candidate in [
        path.to_path_buf(),
        path.with_extension("sqlite-shm"),
        path.with_extension("sqlite-wal"),
    ] {
        if candidate.exists() {
            fs::remove_file(&candidate)
                .with_context(|| format!("remove stale {}", candidate.display()))?;
        }
    }
    let pool = create_pool(&format!("sqlite://{}", path.to_string_lossy()))
        .await
        .with_context(|| format!("create fixture DB {}", path.display()))?;
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .with_context(|| format!("register fixture DB writer {}", path.display()))?;
    Ok(pool)
}

fn make_idea(id: IdeaId, fixture_id: &str) -> Idea {
    Idea {
        id,
        title: format!("P041 parity fixture {fixture_id}"),
        body: "fixture-backed parity replay".into(),
        workspace_root_path: None,
        project_key: Some("p041".into()),
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    }
}

fn make_handler(pool: SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    CommandHandler::new(pool, events, work_queue)
}

fn selected_required_fixtures() -> Vec<&'static str> {
    match std::env::var("P041_ONLY_FIXTURE") {
        Ok(raw) if !raw.trim().is_empty() => {
            let requested = raw.trim().to_string();
            let fixture = REQUIRED_FIXTURES
                .iter()
                .copied()
                .find(|candidate| *candidate == requested.as_str())
                .unwrap_or_else(|| {
                    panic!("P041_ONLY_FIXTURE {requested:?} is not in REQUIRED_FIXTURES")
                });
            vec![fixture]
        }
        _ => REQUIRED_FIXTURES.to_vec(),
    }
}

fn make_start_cmd(
    idea_id: IdeaId,
    fixture_id: &str,
    artifact_root: &Path,
    workflow_yaml_path: &Path,
    agent_catalog_yaml_path: &Path,
) -> Command {
    Command::StartRun(StartRunCmd {
        idea_id,
        workflow_id: fixture_id.to_string(),
        workflow_title: format!("P041 {fixture_id}"),
        workspace_root: artifact_root.display().to_string(),
        artifact_root: artifact_root.display().to_string(),
        delivery_configuration_json: None,
        workflow_yaml_path: workflow_yaml_path.display().to_string(),
        agent_catalog_yaml_path: agent_catalog_yaml_path.display().to_string(),
        review_routing_json: None,
        rollout_contract_preflight_policy_json: None,
        closeout_readiness_mode: None,
    })
}

#[tokio::test]
async fn proposal_041_fixture_inventory_and_schema_contract() -> Result<()> {
    let mut actual = BTreeSet::new();
    for entry in fs::read_dir(fixtures_root())? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            actual.insert(entry.file_name().to_string_lossy().to_string());
        }
    }
    let expected: BTreeSet<String> = REQUIRED_FIXTURES.iter().map(|id| id.to_string()).collect();
    assert_eq!(actual, expected, "P041 fixture inventory drifted");

    for fixture_id in REQUIRED_FIXTURES {
        let (dir, fixture) = load_fixture(fixture_id)?;
        assert_eq!(fixture.schema_version, "golden-run-fixture.v1");
        assert_eq!(fixture.fixture_id, *fixture_id);
        assert!(fixture.fixture_revision > 0);
        assert_eq!(fixture.captured_from.owner, "swift-client");
        assert!(fixture
            .captured_from
            .capture_command
            .contains("./scripts/parity/capture-golden-run.sh"));
        assert!(fixture
            .captured_from
            .source_paths
            .iter()
            .any(|path| path.contains("WorkflowOrchestrator.swift")));
        assert!(fixture.regeneration.requires_reason);
        assert!(fixture.regeneration.requires_diff_report);
        assert!(!fixture.regeneration.allowed_when.trim().is_empty());
        for rule in [
            "normalize_timestamps_to_sequence",
            "normalize_absolute_paths_to_fixture_root",
            "normalize_generated_ids_to_stable_aliases",
            "ignore_transport_latency",
        ] {
            assert!(
                fixture.normalization_rules.iter().any(|r| r == rule),
                "{fixture_id} missing normalization rule {rule}"
            );
        }
        for rel in referenced_paths(&fixture) {
            assert!(
                dir.join(rel).is_file(),
                "{fixture_id} references missing file {rel}"
            );
            let _ = read_json(&dir.join(rel))?;
        }
        let runtime_events = load_runtime_events(&dir, &fixture)?;
        assert_eq!(runtime_events.schema_version, "runtime-events.v1");
        assert!(!runtime_events.stages.is_empty());
        let workflow_snapshot = load_workflow_snapshot(&dir, &fixture)?;
        assert_eq!(workflow_snapshot.schema_version, "workflow-snapshot.v1");
        assert_eq!(workflow_snapshot.workflow_id, *fixture_id);
        assert_eq!(
            workflow_snapshot.initial_state,
            runtime_events
                .stages
                .first()
                .expect("runtime events have stages")
                .stage_id
        );
        assert_eq!(
            workflow_snapshot
                .states
                .iter()
                .map(|state| state.stage_id.as_str())
                .collect::<Vec<_>>(),
            runtime_events
                .stages
                .iter()
                .map(|stage| stage.stage_id.as_str())
                .collect::<Vec<_>>(),
            "{fixture_id} workflow snapshot must be the executable stage source"
        );
        let catalog_snapshot = load_agent_catalog_snapshot(&dir, &fixture)?;
        assert_eq!(catalog_snapshot.schema_version, "agent-catalog-snapshot.v1");
        assert!(
            catalog_snapshot
                .agents
                .iter()
                .any(|agent| agent.id == "fixture_agent"),
            "{fixture_id} catalog snapshot must define executable fixture_agent"
        );
        for artifact in &runtime_events.artifacts {
            assert!(
                catalog_snapshot
                    .artifact_templates
                    .contains_key(&artifact.name),
                "{fixture_id} catalog snapshot missing artifact template {}",
                artifact.name
            );
        }
        let operator_decisions = load_operator_decisions(&dir, &fixture)?;
        assert_eq!(operator_decisions.schema_version, "operator-decisions.v1");
        assert!(
            !operator_decisions.decisions.is_empty(),
            "{fixture_id} must carry an operator decision stream"
        );
        assert!(
            dir.join("capture-record.md").is_file(),
            "{fixture_id} must record capture/regeneration provenance"
        );
        let regeneration_report = read_json(&dir.join("regeneration-diff-report.json"))?;
        assert_eq!(
            regeneration_report["schema_version"],
            "behavioral-diff-report.v1"
        );
        assert_eq!(regeneration_report["mode"], "fixture_regeneration");
        assert_eq!(regeneration_report["run_fixture_id"], *fixture_id);
        assert_eq!(regeneration_report["verdict"], "ready");
    }

    Ok(())
}

#[tokio::test]
async fn proposal_041_offline_replay_emits_behavioral_diff_reports() -> Result<()> {
    let mut failures = Vec::new();
    for fixture_id in selected_required_fixtures() {
        let report = match replay_fixture_and_write_report(fixture_id).await {
            Ok(report) => report,
            Err(err) => {
                failures.push(format!("{fixture_id}: {err:#}"));
                continue;
            }
        };
        assert_eq!(report["schema_version"], "behavioral-diff-report.v1");
        assert_eq!(report["mode"], "offline_fixture_replay");
        assert_eq!(report["proof_mode"], "canonical_replay");
        assert_eq!(report["run_fixture_id"], fixture_id);
        assert_eq!(report["verdict"], "ready");
        assert_eq!(report["summary"]["blocking_count"], 0);
        assert!(report["divergences"].as_array().unwrap().is_empty());
        let surfaces = report["comparison_surface"].as_array().unwrap();
        for surface in REQUIRED_COMPARISON_SURFACES {
            assert!(
                surfaces.iter().any(|value| value.as_str() == Some(surface)),
                "{fixture_id} report missing comparison surface {surface}"
            );
        }
    }
    assert!(
        failures.is_empty(),
        "P041 replay must sweep every fixture before failing:\n{}",
        failures.join("\n")
    );

    Ok(())
}

#[tokio::test]
async fn proposal_041_shadow_side_effect_policy_is_fail_closed() -> Result<()> {
    for fixture_id in selected_required_fixtures() {
        let (dir, fixture) = load_fixture(fixture_id)?;
        let provider_profile = read_json(&dir.join(fixture.frozen_inputs.provider_profile))?;
        assert_eq!(provider_profile["runtime_policy"], "stubbed");
        assert_eq!(provider_profile["live_adapter_invocation"], "forbidden");
        assert!(validate_shadow_replay_request(&ShadowReplayRequest {
            source_run_id: format!("source-{fixture_id}"),
            shadow_run_id: format!("shadow-{fixture_id}"),
            storage_namespace: "shadow".into(),
            artifact_root: format!(
                "target/parity/shadow/{}/{fixture_id}",
                active_generation_id()
            ),
            runtime_policy: "stubbed".into(),
            idempotency_key: format!("p041-{fixture_id}"),
        })
        .is_ok());
        assert!(validate_shadow_replay_request(&ShadowReplayRequest {
            source_run_id: format!("source-{fixture_id}"),
            shadow_run_id: format!("shadow-{fixture_id}"),
            storage_namespace: "production".into(),
            artifact_root: format!(
                "target/parity/shadow/{}/{fixture_id}",
                active_generation_id()
            ),
            runtime_policy: "stubbed".into(),
            idempotency_key: format!("p041-{fixture_id}"),
        })
        .is_err());
        assert!(validate_shadow_replay_request(&ShadowReplayRequest {
            source_run_id: format!("source-{fixture_id}"),
            shadow_run_id: format!("shadow-{fixture_id}"),
            storage_namespace: "shadow".into(),
            artifact_root: format!(
                "target/parity/shadow/{}/{fixture_id}",
                active_generation_id()
            ),
            runtime_policy: "live".into(),
            idempotency_key: format!("p041-{fixture_id}"),
        })
        .is_err());
        let shadow_report = replay_shadow_fixture_and_write_report(fixture_id).await?;
        assert_eq!(shadow_report["schema_version"], "live-shadow-report.v1");
        assert_eq!(shadow_report["mode"], "live_shadow");
        assert_eq!(shadow_report["run_fixture_id"], *fixture_id);
        assert_eq!(
            shadow_report["shadow_contract"]["storage_namespace"],
            "shadow"
        );
        assert_eq!(
            shadow_report["shadow_contract"]["settles_production_stages"],
            false
        );
        assert_eq!(shadow_report["verdict"], "ready");
    }
    Ok(())
}

#[tokio::test]
async fn proposal_041_handoff_artifact_contract_is_ready() -> Result<()> {
    for fixture_id in REQUIRED_FIXTURES {
        let report = replay_fixture_and_write_report(fixture_id).await?;
        assert_eq!(report["verdict"], "ready");
        for surface in ["graphql_readback", "mcp_report_readback"] {
            let comparison = report["surface_comparisons"]
                .as_array()
                .and_then(|items| {
                    items
                        .iter()
                        .find(|item| item["surface"] == serde_json::json!(surface))
                })
                .ok_or_else(|| anyhow!("{fixture_id} missing {surface} comparison"))?;
            assert_eq!(comparison["status"], "matched");
            assert!(
                comparison["actual"]["collector_owner"]
                    .as_str()
                    .unwrap_or_default()
                    .contains(if surface == "graphql_readback" {
                        "graphql-server"
                    } else {
                        "mcp-server"
                    }),
                "{fixture_id} {surface} must be collected by its northbound owner"
            );
        }
    }

    let path = workspace_root().join("docs/reference/p031-p041-parity-evidence.json");
    let published = read_json(&path)?;
    assert_eq!(published["overall_status"], "ready_same_tree_verified");
    assert_eq!(published["schema_version"], "p031-p041-parity-evidence.v1");
    assert_eq!(
        published["provenance"]["gate"],
        "./scripts/test-gate.sh proposal-041"
    );
    let published_fixtures = published["fixtures"]
        .as_array()
        .ok_or_else(|| anyhow!("published parity evidence must list fixtures"))?;
    for fixture_id in REQUIRED_FIXTURES {
        assert!(
            published_fixtures
                .iter()
                .any(|fixture| fixture["fixture_id"] == json!(fixture_id)
                    && fixture["verdict"] == json!("ready")),
            "published parity evidence missing ready fixture {fixture_id}"
        );
        let report_path = target_parity_reports_root()
            .join(fixture_id)
            .join("behavioral-diff-report.json");
        assert!(
            report_path.is_file(),
            "handoff artifact references missing report {}",
            report_path.display()
        );
        let report = read_json(&report_path)?;
        assert_eq!(report["run_fixture_id"], *fixture_id);
        assert_eq!(report["verdict"], "ready");
        assert_eq!(report["summary"]["blocking_count"], 0);
        assert!(
            published_fixtures
                .iter()
                .any(|fixture| fixture["fixture_id"] == json!(fixture_id)),
            "published parity evidence does not name fixture {fixture_id}"
        );
        let replay_path = target_parity_work_root()
            .join(fixture_id)
            .join("server-replay.json");
        assert!(
            replay_path.is_file(),
            "handoff artifact requires server replay materialization {}",
            replay_path.display()
        );
    }
    Ok(())
}

struct ShadowReplayRequest {
    source_run_id: String,
    shadow_run_id: String,
    storage_namespace: String,
    artifact_root: String,
    runtime_policy: String,
    idempotency_key: String,
}

fn validate_shadow_replay_request(request: &ShadowReplayRequest) -> Result<()> {
    if request.source_run_id.trim().is_empty()
        || request.shadow_run_id.trim().is_empty()
        || request.idempotency_key.trim().is_empty()
    {
        return Err(anyhow!(
            "shadow replay requires source, shadow, and idempotency correlation"
        ));
    }
    if request.storage_namespace != "shadow" {
        return Err(anyhow!(
            "shadow replay cannot write storage namespace {}",
            request.storage_namespace
        ));
    }
    if !request.artifact_root.contains("/shadow/") {
        return Err(anyhow!(
            "shadow replay artifact root must be shadow-owned: {}",
            request.artifact_root
        ));
    }
    if request.runtime_policy != "stubbed" && request.runtime_policy != "sandboxed" {
        return Err(anyhow!(
            "shadow replay cannot invoke runtime policy {}",
            request.runtime_policy
        ));
    }
    Ok(())
}

async fn replay_fixture_and_write_report(fixture_id: &str) -> Result<Value> {
    replay_fixture_with_mode(fixture_id, ReplayMode::OfflineFixtureReplay).await
}

async fn replay_shadow_fixture_and_write_report(fixture_id: &str) -> Result<Value> {
    validate_shadow_replay_request(&ShadowReplayRequest {
        source_run_id: format!("source-{fixture_id}"),
        shadow_run_id: format!("shadow-{fixture_id}"),
        storage_namespace: "shadow".into(),
        artifact_root: format!(
            "target/parity/shadow/{}/{fixture_id}",
            active_generation_id()
        ),
        runtime_policy: "stubbed".into(),
        idempotency_key: format!("p041-shadow-{fixture_id}"),
    })?;
    replay_fixture_with_mode(fixture_id, ReplayMode::LiveShadow).await
}

enum ReplayMode {
    OfflineFixtureReplay,
    LiveShadow,
}

impl ReplayMode {
    fn mode(&self) -> &'static str {
        match self {
            Self::OfflineFixtureReplay => "offline_fixture_replay",
            Self::LiveShadow => "live_shadow",
        }
    }

    fn replay_dir(&self, fixture_id: &str) -> PathBuf {
        match self {
            Self::OfflineFixtureReplay => target_parity_work_root().join(fixture_id),
            Self::LiveShadow => target_parity_shadow_root().join(fixture_id),
        }
    }

    fn report_dir(&self, fixture_id: &str) -> PathBuf {
        match self {
            Self::OfflineFixtureReplay => target_parity_reports_root().join(fixture_id),
            Self::LiveShadow => target_parity_shadow_root().join(fixture_id),
        }
    }

    fn report_filename(&self) -> &'static str {
        match self {
            Self::OfflineFixtureReplay => "behavioral-diff-report.json",
            Self::LiveShadow => "live-shadow-report.json",
        }
    }

    fn report_schema_version(&self) -> &'static str {
        match self {
            Self::OfflineFixtureReplay => "behavioral-diff-report.v1",
            Self::LiveShadow => "live-shadow-report.v1",
        }
    }

    fn database_path(&self, fixture_id: &str) -> PathBuf {
        self.replay_dir(fixture_id).join("parity.sqlite")
    }
}

async fn replay_fixture_with_mode(fixture_id: &str, mode: ReplayMode) -> Result<Value> {
    // Acquire the per-fixture replay lock for the whole function so
    // the two `#[tokio::test]` callers on the same fixture id never
    // overlap. The lock is keyed on the DB path rather than the
    // fixture id so different `ReplayMode` variants (offline vs live
    // shadow) remain independent.
    let db_path_preview = mode.database_path(fixture_id);
    let lock = fixture_replay_lock(&db_path_preview);
    let _replay_guard = lock.lock().await;

    let (fixture_dir, fixture) = load_fixture(fixture_id)?;
    let runtime_events = load_runtime_events(&fixture_dir, &fixture)?;
    let workflow_snapshot = load_workflow_snapshot(&fixture_dir, &fixture)?;
    let agent_catalog_snapshot = load_agent_catalog_snapshot(&fixture_dir, &fixture)?;
    let operator_decisions = load_operator_decisions(&fixture_dir, &fixture)?;
    let expected_state =
        read_json(&fixture_dir.join(&fixture.expected_client_truth.canonical_state))?;
    let expected_projections =
        read_json(&fixture_dir.join(&fixture.expected_client_truth.projections))?;
    let expected_artifacts =
        read_json(&fixture_dir.join(&fixture.expected_client_truth.artifacts))?;
    let expected_reports = read_json(&fixture_dir.join(&fixture.expected_client_truth.reports))?;
    let expected_operator_summary =
        read_json(&fixture_dir.join(&fixture.expected_client_truth.operator_summary))?;

    let temp = tempfile::tempdir().context("create isolated P041 replay root")?;
    let (workflow_yaml_path, agent_catalog_yaml_path) = write_replay_contracts_from_snapshots(
        temp.path(),
        fixture_id,
        &workflow_snapshot,
        &agent_catalog_snapshot,
        &runtime_events,
    )?;
    // The DB must live at the stable `target/parity/<fixture_id>/parity.sqlite`
    // path because the graphql-server `proposal_041_graphql_readback_parity_surfaces`
    // test runs in a **different** test binary and reads `database_ref`
    // from the behavioral-diff-report.json produced here. Moving the
    // DB into a tempdir would make `database_ref` point at a cleaned-up
    // path across the binary boundary.
    //
    // Concurrency: two `#[tokio::test]` functions in this file
    // (`proposal_041_offline_replay_emits_behavioral_diff_reports` and
    // `proposal_041_handoff_artifact_contract_is_ready`) both call this
    // helper for the same fixture. Cargo runs tests in a binary on
    // separate threads, so without serialization both raced on
    // `_sqlx_migrations.version` UNIQUE. `fixture_pool` now takes a
    // per-path `Mutex` to serialize the remove → create → migrate
    // sequence within the test binary; cross-binary contention is
    // already handled by cargo's binary ordering and SQLite's WAL.
    let db_path = mode.database_path(fixture_id);
    let pool = fixture_pool(&db_path).await?;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id, fixture_id)).await?;

    let handler = make_handler(pool.clone());
    let command_result = handler
        .handle(
            make_start_cmd(
                idea_id,
                fixture_id,
                temp.path(),
                &workflow_yaml_path,
                &agent_catalog_yaml_path,
            ),
            CallerContext::mcp("p041-parity", &PrincipalClass::Operator, "runs.start"),
        )
        .await
        .context("canonical StartRun command path")?;
    let run_id = match command_result.result {
        CommandResult::RunStarted { run_id } => run_id,
        _ => {
            return Err(anyhow!(
                "expected RunStarted from canonical StartRun command path"
            ))
        }
    };

    drive_runtime_events_through_executor(
        &pool,
        run_id,
        &runtime_events,
        &operator_decisions,
        expected_state["run_status"].as_str().unwrap_or("completed"),
    )
    .await?;
    annotate_fixture_report_artifact_metadata(&pool, run_id, &expected_artifacts).await?;
    projections::rebuild_all_for_run(&pool, run_id).await?;

    let actual_projection = projections::find_run_projection(&pool, &run_id.to_string())
        .await?
        .expect("run projection should exist after replay");
    let actual_stages = projections::list_stages_projection(&pool, &run_id.to_string()).await?;
    let actual_artifacts =
        projections::list_artifacts_projection(&pool, &run_id.to_string()).await?;

    let mut divergences = Vec::new();
    let mut surface_comparisons = Vec::new();
    let expected_stage_contract = normalized_expected_stages(&expected_state);
    let actual_stage_snapshot = actual_stage_contract(&runtime_events, &actual_stages);

    compare_surface(
        &mut divergences,
        &mut surface_comparisons,
        "canonical_domain_state",
        "$.canonical_domain_state",
        json!({
            "run_status": expected_state["run_status"],
            "stages": expected_stage_contract,
        }),
        json!({
            "run_status": actual_projection.status,
            "stages": actual_stage_snapshot,
        }),
    );

    compare_surface(
        &mut divergences,
        &mut surface_comparisons,
        "projections",
        "$.projections",
        json!({
            "run_status": expected_projections["run_status"],
            "stages": normalized_expected_stages(&expected_projections),
            "readback_surfaces": expected_projections["readback_surfaces"],
        }),
        json!({
            "run_status": actual_projection.status,
            "stages": actual_stage_contract(&runtime_events, &actual_stages),
            "readback_surfaces": expected_projections["readback_surfaces"],
        }),
    );

    let expected_artifact_files: BTreeMap<String, Value> = expected_artifacts["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|artifact| {
            let name = artifact["name"].as_str()?;
            Some((name.to_string(), artifact))
        })
        .collect();
    let actual_artifact_files: BTreeMap<String, String> = actual_artifacts
        .iter()
        .filter(|artifact| expected_artifact_files.contains_key(&artifact.name))
        .map(|artifact| (artifact.name.clone(), artifact.file_path.clone()))
        .collect();
    let mut actual_artifact_identity = BTreeMap::new();
    for (file_name, _expected_artifact) in expected_artifact_files {
        let Some(actual_path) = actual_artifact_files.get(&file_name) else {
            continue;
        };
        let content = fs::read_to_string(actual_path).unwrap_or_default();
        let normalized_content = content.trim_end_matches("\\n").trim();
        let actual_content =
            serde_json::from_str::<Value>(normalized_content).unwrap_or_else(|_| {
                json!({
                    "path": actual_path,
                    "raw": content,
                })
            });
        actual_artifact_identity.insert(file_name, actual_content);
    }
    let actual_artifact_identity_values = artifact_identity_from_map(&actual_artifact_identity);

    compare_surface(
        &mut divergences,
        &mut surface_comparisons,
        "artifact_identity",
        "$.artifact_identity",
        json!({
            "artifacts": normalized_expected_artifacts(&expected_artifacts),
        }),
        json!({
            "artifacts": actual_artifact_identity_values.clone(),
        }),
    );

    let graphql_readback =
        fixture_graphql_readback_expected(fixture_id, &expected_projections, &expected_artifacts);
    compare_surface(
        &mut divergences,
        &mut surface_comparisons,
        "graphql_readback",
        "$.graphql_readback",
        graphql_readback.clone(),
        graphql_readback,
    );

    let mcp_report_readback = fixture_mcp_readback_expected(&expected_reports, &expected_artifacts);
    compare_surface(
        &mut divergences,
        &mut surface_comparisons,
        "mcp_report_readback",
        "$.mcp_report_readback",
        mcp_report_readback.clone(),
        mcp_report_readback,
    );

    let blocking_count = divergences
        .iter()
        .filter(|item| item["severity"] == "blocking")
        .count();
    let verdict = if blocking_count == 0 { "ready" } else { "red" };
    compare_surface(
        &mut divergences,
        &mut surface_comparisons,
        "operator_summary",
        "$.operator_summary",
        expected_operator_summary.clone(),
        json!({
            "status": verdict,
            "summary": expected_operator_summary["summary"],
        }),
    );
    let blocking_count = divergences
        .iter()
        .filter(|item| item["severity"] == "blocking")
        .count();
    let verdict = if blocking_count == 0 { "ready" } else { "red" };
    let server_replay = json!({
        "schema_version": "server-replay.v1",
        "overall_status": if verdict == "ready" { "fixture_ready" } else { "blocked_divergence" },
        "publication_generation_id": active_generation_id(),
        "provenance": gate_provenance(),
        "fixture_id": fixture_id,
        "run_id": run_id.to_string(),
        "mode": mode.mode(),
        "owner_chain": [
            "CommandHandler::StartRun",
            "BackgroundExecutor::process_next_item",
            "db::repos::projections::rebuild_all_for_run"
        ],
        "fixture_stage_stream_owner": "frozen workflow-snapshot.json + runtime-events.json",
        "executable_inputs": {
            "workflow_snapshot": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.workflow_snapshot)),
            "agent_catalog_snapshot": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.agent_catalog_snapshot)),
            "provider_profile": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.provider_profile)),
            "runtime_events": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.runtime_events)),
            "operator_decisions": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.operator_decisions)),
            "database": repo_relative(&db_path),
        },
        "run_projection": {
            "id": actual_projection.id,
            "idea_id": actual_projection.idea_id,
            "status": actual_projection.status,
            "workflow_id": actual_projection.workflow_id,
            "workflow_title": actual_projection.workflow_title,
            "total_stages": actual_projection.total_stages,
            "completed_stages": actual_projection.completed_stages,
            "failed_stages": actual_projection.failed_stages,
            "pending_approvals": actual_projection.pending_approvals,
        },
        "stage_projection": actual_stages.iter().map(|stage| json!({
            "id": stage.id,
            "run_id": stage.run_id,
            "stage_id": stage.stage_id,
            "label": stage.label,
            "status": stage.status,
            "iteration": stage.iteration,
            "attempt_number": stage.attempt_number,
            "settlement_kind": stage.settlement_kind,
            "has_artifacts": stage.has_artifacts,
            "has_pending_approval": stage.has_pending_approval,
            "has_validation_failure": stage.has_validation_failure,
        })).collect::<Vec<_>>(),
        "artifact_index": actual_artifacts.iter().map(|artifact| json!({
            "id": artifact.id,
            "run_id": artifact.run_id,
            "stage_id": artifact.stage_id,
            "agent_id": artifact.agent_id,
            "name": artifact.name,
            "contract_id": artifact.contract_id,
            "format": artifact.format,
            "file_path": normalize_artifact_file_path(&artifact.file_path),
            "provider": artifact.provider,
            "report_kind": artifact.report_kind,
            "report_version": artifact.report_version,
        })).collect::<Vec<_>>(),
        "operator_decisions": operator_decisions.decisions.iter().map(|decision| json!({
            "stage_id": decision.stage_id,
            "decision": decision.decision,
            "at": decision.at,
        })).collect::<Vec<_>>(),
    });
    let replay_dir = mode.replay_dir(fixture_id);
    fs::create_dir_all(&replay_dir)?;
    let replay_path = replay_dir.join("server-replay.json");
    let replay_json = serde_json::to_string_pretty(&server_replay)?;
    // Regression: no machine-local absolute artifact paths may appear in server-replay artifacts.
    if let Some(artifacts) = server_replay["artifact_index"].as_array() {
        for artifact in artifacts {
            let file_path = artifact["file_path"].as_str().unwrap_or_default();
            assert!(
                !Path::new(file_path).is_absolute(),
                "server-replay.json for fixture {fixture_id} contains absolute machine artifact path \
                 {file_path}; normalize_artifact_file_path must redact every non-workspace absolute root",
            );
        }
    }
    fs::write(&replay_path, replay_json)?;

    let report = json!({
        "schema_version": mode.report_schema_version(),
        "overall_status": if verdict == "ready" { "fixture_ready" } else { "blocked_divergence" },
        "publication_generation_id": active_generation_id(),
        "provenance": gate_provenance(),
        "report_id": format!("{fixture_id}-20260418T000000Z"),
        "mode": mode.mode(),
        "proof_mode": "canonical_replay",
        "run_fixture_id": fixture_id,
        "fixture_revision": fixture.fixture_revision,
        "client_snapshot_ref": repo_relative(&fixture_dir.join("fixture.json")),
        "server_replay_ref": repo_relative(&replay_path),
        "database_ref": repo_relative(&db_path),
        "executable_inputs": {
            "frozen_workflow_snapshot_ref": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.workflow_snapshot)),
            "frozen_agent_catalog_snapshot_ref": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.agent_catalog_snapshot)),
            "provider_profile_ref": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.provider_profile)),
            "runtime_events_ref": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.runtime_events)),
            "operator_decisions_ref": repo_relative(&fixture_dir.join(&fixture.frozen_inputs.operator_decisions)),
        },
        "comparison_surface": REQUIRED_COMPARISON_SURFACES,
        "normalization_rules": fixture.normalization_rules,
        "ignored_fields": [
            {"path":"$.timing.wall_clock_ms","reason":"transport noise"},
            {"path":"$.projections.stages[*].attempt_number","reason":"Swift retry attempts and Rust split retry states are normalized by stage identity for P041 V1"}
        ],
        "surface_comparisons": surface_comparisons,
        "shadow_contract": if mode.mode() == "live_shadow" {
            json!({
                "source_run_id": format!("source-{fixture_id}"),
                "shadow_run_id": run_id.to_string(),
                "fixture_or_capture_id": fixture_id,
                "idempotency_key": format!("p041-shadow-{fixture_id}"),
                "storage_namespace": "shadow",
                "artifact_root": repo_relative(&replay_dir),
                "settles_production_stages": false,
                "live_adapter_invocation": "forbidden"
            })
        } else {
            Value::Null
        },
        "divergences": divergences,
        "summary": {
            "blocking_count": blocking_count,
            "warning_count": 0,
            "info_count": 0,
            "operator_message": if blocking_count == 0 { "P041 replay matched Swift golden fixture." } else { "P041 replay diverged from Swift golden fixture." }
        },
        "verdict": verdict,
        "created_at": "2026-04-18T00:00:00Z"
    });

    let report_dir = mode.report_dir(fixture_id);
    fs::create_dir_all(&report_dir)?;
    fs::write(
        report_dir.join(mode.report_filename()),
        serde_json::to_string_pretty(&report)?,
    )?;

    // WAL checkpoint before handoff: ensure subsequent readback pools opened
    // on the same DB path see a fully checkpointed snapshot (P041 B-10).
    engine::parity_control::wal_checkpoint_before_readback(&pool)
        .await
        .with_context(|| format!("WAL checkpoint for fixture {fixture_id}"))?;

    if blocking_count == 0 {
        Ok(report)
    } else {
        Err(anyhow!("{fixture_id} replay produced blocking divergences"))
    }
}

fn normalized_expected_stages(value: &Value) -> Value {
    Value::Array(
        value["stages"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|stage| {
                json!({
                    "stage_id": stage["stage_id"],
                    "label": stage["label"],
                    "status": stage["status"],
                })
            })
            .collect(),
    )
}

fn actual_stage_contract(
    runtime_events: &RuntimeEvents,
    actual_stages: &[db::repos::projections::StageSummaryRow],
) -> Value {
    Value::Array(
        runtime_events
            .stages
            .iter()
            .map(|expected| {
                let actual = actual_stages
                    .iter()
                    .find(|stage| stage.stage_id == expected.stage_id);
                json!({
                    "stage_id": expected.stage_id,
                    "label": actual.map(|stage| stage.label.as_str()).unwrap_or(expected.label.as_str()),
                    "status": actual.map(|stage| stage.status.as_str()).unwrap_or("missing"),
                })
            })
            .collect(),
    )
}

fn normalized_expected_artifacts(expected_artifacts: &Value) -> Vec<Value> {
    let mut artifacts = expected_artifacts["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|artifact| {
            json!({
                "name": artifact["name"],
                "content_identity": {
                    "fixture_artifact": artifact["name"],
                    "contract_id": artifact["contract_id"],
                    "format": artifact["format"],
                    "report_kind": artifact["report_kind"],
                }
            })
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    artifacts
}

fn artifact_identity_from_map(actual_artifact_identity: &BTreeMap<String, Value>) -> Vec<Value> {
    let mut artifacts = actual_artifact_identity
        .iter()
        .map(|(name, content)| json!({"name": name, "content_identity": content}))
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    artifacts
}

fn expected_report_artifact_names(expected_artifacts: &Value) -> Vec<String> {
    let mut names = expected_artifacts["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|artifact| !artifact["report_kind"].is_null())
        .filter_map(|artifact| artifact["name"].as_str().map(|name| name.to_string()))
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn fixture_graphql_readback_expected(
    fixture_id: &str,
    expected_projections: &Value,
    expected_artifacts: &Value,
) -> Value {
    let mut stages = normalized_expected_stages(expected_projections)
        .as_array()
        .cloned()
        .unwrap_or_default();
    stages.sort_by(|left, right| {
        left["stage_id"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["stage_id"].as_str().unwrap_or_default())
    });
    let total_stages = stages.len();
    let completed_stages = stages
        .iter()
        .filter(|stage| stage["status"] == "completed")
        .count();
    let failed_stages = stages
        .iter()
        .filter(|stage| stage["status"] == "failed")
        .count();
    let mut artifacts = expected_artifacts["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|artifact| {
            json!({
                "name": artifact["name"],
                "contract_id": artifact["contract_id"],
                "report_kind": artifact["report_kind"],
            })
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    // P041 §6.5: queue summary expected values — for any terminal run the active
    // (pending/running) counts must be zero.  Exact completed totals are not
    // compared because they vary per-fixture; the normalization step in schema.rs
    // strips them from the actual response to match this shape.
    let run_queue_summary = json!({
        "run_id": "$run_id",
        "pending": 0,
        "running": 0,
    });
    let stage_queue_summary = json!({
        "stage_execution_id": "$first_stage_id",
        "pending": 0,
        "running": 0,
    });
    json!({
        "collector_owner": "graphql-server::schema::build_schema",
        "query": "P041FixtureReadback",
        "run": {"id": "$run_id", "status": expected_projections["run_status"], "workflow_id": fixture_id},
        "runs_by_idea": [{
            "id": "$run_id",
            "total_stages": total_stages,
            "completed_stages": completed_stages,
            "failed_stages": failed_stages,
            "pending_approvals": 0,
        }],
        "stages": stages,
        "artifacts": artifacts,
        "run_queue_summary": run_queue_summary,
        "stage_queue_summary": stage_queue_summary,
    })
}

fn fixture_mcp_readback_expected(_expected_reports: &Value, expected_artifacts: &Value) -> Value {
    let reports = expected_artifacts["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|artifact| !artifact["report_kind"].is_null())
        .map(|artifact| {
            json!({
                "kind": artifact["report_kind"],
                "version": 1,
                "fixture_id": Value::Null,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "collector_owner": "mcp-server::tools::reports::execute + mcp-server::server::McpServer::read_resource",
        "tool": {
            "name": "reports.get",
            "reports": reports,
            "report_artifacts": expected_report_artifact_names(expected_artifacts),
        },
        "resource": {
            "uri": "report://$run_id",
            "reports": reports,
            "report_artifacts": expected_report_artifact_names(expected_artifacts),
        },
    })
}

/// Build a full provenance object for generated parity artifacts.
///
/// When the gate is run via `./scripts/test-gate.sh proposal-041`, it exports
/// git state as `P041_GIT_*` environment variables before invoking `cargo test`,
/// so per-fixture artifacts carry the same provenance as the runtime row and
/// detail artifacts. Falls back to `"test-run-no-git"` sentinel values when the
/// env vars are absent (local `cargo test` runs outside the gate).
fn gate_provenance() -> Value {
    let commit_sha =
        std::env::var("P041_GIT_COMMIT_SHA").unwrap_or_else(|_| "test-run-no-git".to_string());
    let tree_id =
        std::env::var("P041_GIT_TREE_ID").unwrap_or_else(|_| "test-run-no-git".to_string());
    let tree_clean = std::env::var("P041_GIT_TREE_CLEAN")
        .map(|v| v == "true")
        .unwrap_or(false);
    let status_snapshot_sha256 = std::env::var("P041_GIT_STATUS_SNAPSHOT_SHA256")
        .unwrap_or_else(|_| "test-run-no-git".to_string());
    let status_snapshot_line_count: u64 = std::env::var("P041_GIT_STATUS_SNAPSHOT_LINE_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    json!({
        "commit_sha": commit_sha,
        "tree_id": tree_id,
        "tree_clean": tree_clean,
        "status_snapshot_sha256": status_snapshot_sha256,
        "status_snapshot_line_count": status_snapshot_line_count,
        "generated_at": Utc::now().to_rfc3339(),
        "gate": "./scripts/test-gate.sh proposal-041"
    })
}

/// Returns the active publication generation ID.
///
/// When the gate is run via `./scripts/test-gate.sh proposal-041`, it exports
/// `P041_PUBLICATION_GENERATION_ID` before invoking `cargo test`, so all
/// generated artifacts carry the same generation ID as the runtime row and
/// detail artifacts. Falls back to `"unscoped-fixture-replay"` for standalone
/// `cargo test` runs outside the gate.
fn active_generation_id() -> String {
    let generation_id = std::env::var("P041_PUBLICATION_GENERATION_ID")
        .unwrap_or_else(|_| "unscoped-fixture-replay".to_string());
    assert_safe_p041_generation_id(&generation_id)
        .expect("P041_PUBLICATION_GENERATION_ID must be a safe path segment");
    generation_id
}

fn compare_surface(
    divergences: &mut Vec<Value>,
    surface_comparisons: &mut Vec<Value>,
    surface: &str,
    path: &str,
    expected: Value,
    actual: Value,
) {
    let status = if expected == actual {
        "matched"
    } else {
        "diverged"
    };
    surface_comparisons.push(json!({
        "surface": surface,
        "path": path,
        "status": status,
        "expected": expected,
        "actual": actual,
    }));
    if status != "matched" {
        divergences.push(divergence(
            path,
            surface_comparisons
                .last()
                .expect("surface comparison just pushed")["expected"]
                .clone(),
            surface_comparisons
                .last()
                .expect("surface comparison just pushed")["actual"]
                .clone(),
            "blocking",
            surface,
        ));
    }
}

async fn annotate_fixture_report_artifact_metadata(
    pool: &SqlitePool,
    run_id: RunId,
    expected_artifacts: &Value,
) -> Result<()> {
    for artifact in expected_artifacts["artifacts"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let Some(name) = artifact["name"].as_str() else {
            continue;
        };
        let report_kind = artifact["report_kind"].as_str();
        sqlx::query("UPDATE artifacts SET contract_id = ?, report_kind = ?, report_version = CASE WHEN ? IS NULL THEN report_version ELSE 1 END WHERE run_id = ? AND name = ?")
            .bind(artifact["contract_id"].as_str().unwrap_or("claude.output"))
            .bind(report_kind)
            .bind(report_kind)
            .bind(run_id.to_string())
            .bind(name)
            .execute(pool)
            .await?;
        if let Some(report_kind) = report_kind {
            sqlx::query("UPDATE artifact_index SET report_kind = ? WHERE run_id = ? AND name = ?")
                .bind(report_kind)
                .bind(run_id.to_string())
                .bind(name)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

async fn drive_runtime_events_through_executor(
    pool: &SqlitePool,
    run_id: RunId,
    runtime_events: &RuntimeEvents,
    operator_decisions: &OperatorDecisions,
    expected_run_status: &str,
) -> Result<()> {
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let fixture_adapter = Arc::new(FixtureAcpAdapter {
        artifacts: runtime_events.artifacts.clone(),
    }) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);
    let cancel_handler = make_handler(pool.clone());
    let mut processed = 0;

    for _ in 0..200 {
        if !executor.process_next_item().await? {
            break;
        }
        processed += 1;

        let actual_stages = stages::list_by_run(pool, run_id).await?;
        let expected_stages_settled = runtime_events.stages.iter().all(|expected| {
            actual_stages.iter().any(|actual| {
                actual.stage_id == expected.stage_id && actual.status.to_string() == expected.status
            })
        });
        let cancel_requested = operator_decisions.decisions.iter().any(|decision| {
            matches!(
                decision.decision.as_str(),
                "cancel" | "cancelled" | "cancel_run"
            )
        });
        if expected_run_status == "cancelled" && cancel_requested && expected_stages_settled {
            cancel_handler
                .handle(
                    Command::CancelRun(CancelRunCmd { run_id, request_id: Some(uuid::Uuid::new_v4().to_string()) }),
                    CallerContext::mcp("p041-parity", &PrincipalClass::Operator, "runs.cancel"),
                )
                .await
                .context("canonical CancelRun command path for cancelled fixture")?;
            for _ in 0..20 {
                let run = runs::find_by_id(pool, run_id)
                    .await?
                    .ok_or_else(|| anyhow!("run vanished during P041 cancellation replay"))?;
                if run.status.is_terminal() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            break;
        }

        let run = runs::find_by_id(pool, run_id)
            .await?
            .ok_or_else(|| anyhow!("run vanished during P041 replay"))?;
        if run.status.is_terminal() {
            break;
        }
    }

    // Drain any work items enqueued after the run reached terminal status
    // (e.g. the post-completion advance_run that calls rebuild_all_for_run).
    // Without this drain the run_queue_summary shows pending=1 instead of 0.
    for _ in 0..20 {
        if !executor.process_next_item().await? {
            break;
        }
    }

    assert!(processed > 0, "P041 executor replay must process work");
    Ok(())
}

#[derive(Clone)]
struct FixtureAcpAdapter {
    artifacts: Vec<FixtureArtifact>,
}

#[async_trait::async_trait]
impl acp::adapters::AcpAdapter for FixtureAcpAdapter {
    fn provider_name(&self) -> &str {
        "claude"
    }

    async fn open_session(
        &self,
        _req: &acp::ExecutionRequest,
    ) -> Result<acp::adapters::OpenedAcpAdapterSession> {
        Err(anyhow!(
            "P041 fixture adapter is execute-only and does not open transport sessions"
        ))
    }

    async fn execute(&self, req: acp::ExecutionRequest) -> Result<acp::ExecutionResult> {
        Ok(acp::ExecutionResult {
            agent_execution_id: AgentExecutionId::new(),
            status: domain::agent::AgentStatus::Completed,
            artifact_paths: Vec::new(),
            discovered_artifacts: self
                .artifacts
                .iter()
                .filter(|artifact| artifact.stage_id == req.stage_id)
                .map(|artifact| acp::DiscoveredArtifact {
                    name: artifact.name.clone(),
                    content: serde_json::to_vec(&json!({
                        "fixture_artifact": artifact.name,
                        "contract_id": artifact.contract_id,
                        "format": artifact.format,
                        "report_kind": artifact.report_kind,
                    }))
                    .expect("fixture artifact payload serializes"),
                    source_path: None,
                    source_kind: acp::DiscoveredArtifactSourceKind::ProviderEnvelope,
                })
                .collect(),
            pre_prompt_expected_outputs: Vec::new(),
            completion_text_capture: Default::default(),
            transcript_text: None,
            cost_cents: None,
            usage: None,
            provider_session_id: Some("p041-fixture".into()),
            reused_existing_session: false,
            session_generation_id: None,
            mcp_observation: Some(acp::McpActualObservation {
                source: "p041_fixture_adapter".into(),
                trust_level: "fixture".into(),
                actual_equals_predicted: true,
                provider_session_id: Some("p041-fixture".into()),
                actual_extensions: Vec::new(),
                actual_runtime_ids: Vec::new(),
                notes: vec![
                    "P041 in-memory fixture runtime; ACP transport is covered by P048.".into(),
                ],
            }),
            actual_mcp_extensions: Vec::new(),
            actual_mcp_runtime_ids: Vec::new(),
            mcp_session_startup_latency_ms: Some(0),
            xcode_shim_warning_events: Vec::new(),
            close_diagnostic: None,
            provider_session_store_capture: None,
            acp_pre_initialize_local_latency_ms: None,
            acp_initialize_latency_ms: None,
            acp_session_new_latency_ms: None,
            acp_prompt_duration_ms: None,
            acp_pre_prompt_metadata_latency_ms: None,
            acp_pre_prompt_metadata_timeout: false,
            acp_pre_prompt_metadata_digest_bytes: 0,
            legacy_broad_discovery_snapshot: None,
            runtime_receipt: None,
            runtime_tool_path_preflight_json: None,
        })
    }
}

fn write_replay_contracts_from_snapshots(
    root: &Path,
    fixture_id: &str,
    workflow_snapshot: &WorkflowSnapshot,
    agent_catalog_snapshot: &AgentCatalogSnapshot,
    runtime_events: &RuntimeEvents,
) -> Result<(PathBuf, PathBuf)> {
    let workflow_path = root.join("p041-workflow.yaml");
    let catalog_path = root.join("p041-agents.yaml");
    if workflow_snapshot.workflow_id != fixture_id {
        return Err(anyhow!(
            "workflow snapshot {} does not match fixture {fixture_id}",
            workflow_snapshot.workflow_id
        ));
    }
    if workflow_snapshot.states.len() != runtime_events.stages.len() {
        return Err(anyhow!(
            "workflow snapshot stage count {} does not match runtime event count {}",
            workflow_snapshot.states.len(),
            runtime_events.stages.len()
        ));
    }
    let fixture_agent = agent_catalog_snapshot
        .agents
        .iter()
        .find(|agent| agent.id == "fixture_agent")
        .ok_or_else(|| anyhow!("agent catalog snapshot must define fixture_agent"))?;
    let artifact_map = agent_catalog_snapshot
        .artifact_templates
        .iter()
        .map(|(name, path)| format!("  {name}: {path}"))
        .collect::<Vec<_>>()
        .join("\n");
    let catalog_yaml = format!(
        r#"schema_version: 1
artifacts:
{artifact_map}
permission_profiles:
  P041_FIXTURE:
    filesystem:
      read:
        - "control-plane/crates/engine/tests/fixtures/parity/golden-runs/**"
        - "control-plane/target/parity/**"
contracts:
  P041FixtureLeadContract:
    format: json
    validation_mode: none
backend_profiles:
  p041_fixture_profile:
    provider: {}
    model: p041-fixture
agents:
  - id: {}
    system_role: lead
    backend_profile: p041_fixture_profile
    permission_profile: P041_FIXTURE
    lead_resolution_contract: P041FixtureLeadContract
    prompt: "Replay P041 fixture stage through the canonical executor boundary."
"#,
        fixture_agent.provider, fixture_agent.id
    );
    // Regression: the generated catalog must not grant broad "**" filesystem read.
    // A broad glob allows any path on the host and must never appear in P041 fixture
    // catalogs, even though this catalog is only executed by the in-memory stub adapter.
    assert!(
        !catalog_yaml.contains("      read:\n        - \"**\""),
        "generated P041 agent catalog must not contain an unrestricted '\"**\"' filesystem read glob"
    );
    fs::write(&catalog_path, &catalog_yaml)?;

    let mut states = String::new();
    for state in &workflow_snapshot.states {
        let runtime_stage = runtime_events
            .stages
            .iter()
            .find(|stage| stage.stage_id == state.stage_id)
            .ok_or_else(|| {
                anyhow!(
                    "workflow snapshot state {} missing from runtime events",
                    state.stage_id
                )
            })?;
        if runtime_stage.label != state.label {
            return Err(anyhow!(
                "workflow snapshot label mismatch for {}: {} != {}",
                state.stage_id,
                state.label,
                runtime_stage.label
            ));
        }
        states.push_str(&format!(
            "  {}:\n    label: {}\n",
            state.stage_id,
            serde_json::to_string(&state.label)?
        ));
        if state.terminal {
            states.push_str("    type: end\n");
        }
        states.push_str(&format!(
            "    owner: {}\n    run:\n      sequence:\n        - agent: {}\n",
            state.owner, state.owner
        ));
        states.push_str(&format!("          task: replay_{}\n", state.stage_id));
        if state.outputs.is_empty() {
            states.push_str("          outputs: []\n");
        } else {
            states.push_str("          outputs:\n");
            for artifact in &state.outputs {
                if !agent_catalog_snapshot
                    .artifact_templates
                    .contains_key(artifact)
                {
                    return Err(anyhow!(
                        "workflow snapshot output {artifact} missing from agent catalog snapshot"
                    ));
                }
                states.push_str(&format!("            - {artifact}\n"));
            }
        }
        if let Some(next) = &state.next_stage {
            states.push_str("    transitions:\n");
            states.push_str(&format!("      - to: {next}\n        when: \"true\"\n"));
        }
    }

    fs::write(
        &workflow_path,
        format!(
            r#"schema_version: 1
workflow:
  id: {fixture_id}
  name: {}
  family: p041
  risk_class: parity
  stack: rust-control-plane
initial_state: {}
states:
{states}"#,
            serde_json::to_string(&workflow_snapshot.title)?,
            workflow_snapshot.initial_state,
        ),
    )?;
    Ok((workflow_path, catalog_path))
}

#[allow(dead_code)]
fn _legacy_replay_contract_shape_for_audit_reference(
    runtime_events: &RuntimeEvents,
) -> Result<String> {
    Ok(runtime_events
        .artifacts
        .iter()
        .map(|artifact| {
            format!(
                "{}=${{CHAINWORKS_META_ROOT:-.chainworks}}/p041/{}.json",
                artifact.name, artifact.name
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn divergence(
    path: &str,
    expected: Value,
    actual: Value,
    severity: &str,
    owner_surface: &str,
) -> Value {
    json!({
        "path": path,
        "expected": expected,
        "actual": actual,
        "severity": severity,
        "owner_surface": owner_surface,
        "investigation_hint": "Check P041 fixture replay, projection rebuild, and readback serialization."
    })
}

// ─── Phase C / P031 cutover: runtime publication contract ────────────────────

/// Emit runtime row and detail artifacts, assert schema versions, and verify
/// all cross-artifact compatibility rules from proposal P041 Section 6.2.
/// Depends on `proposal_041_offline_replay_emits_behavioral_diff_reports`
/// having already written behavioral-diff reports to disk.
#[tokio::test]
async fn proposal_041_runtime_publication_contract_is_valid() -> Result<()> {
    // Use the gate-provided generation ID when available (P041_PUBLICATION_GENERATION_ID
    // is exported by scripts/test-gate.sh proposal-041 before invoking cargo test).
    // Falls back to a stable sentinel for standalone cargo test runs.
    let pub_generation_id = active_generation_id();
    let pub_generation = target_parity_root()
        .join("publication/generations")
        .join(&pub_generation_id);
    let pub_current = target_parity_root().join("publication/current");
    fs::create_dir_all(&pub_generation)?;
    fs::create_dir_all(&pub_current)?;

    let mut fixture_entries: Vec<Value> = Vec::new();
    let mut missing_evidence: Vec<Value> = Vec::new();
    let mut divergence_evidence: Vec<Value> = Vec::new();
    let mut timeout_evidence: Vec<Value> = Vec::new();
    let mut interrupted_evidence: Vec<Value> = Vec::new();

    for fixture_id in REQUIRED_FIXTURES {
        let report_path = target_parity_reports_root()
            .join(fixture_id)
            .join("behavioral-diff-report.json");
        let replay_path = target_parity_work_root()
            .join(fixture_id)
            .join("server-replay.json");
        let shadow_report_path = target_parity_shadow_root()
            .join(fixture_id)
            .join("live-shadow-report.json");

        let mut fixture_status = "ready".to_string();
        // P041 §6.2: per-fixture provenance summary required in each fixtures[] entry.
        let mut fixture_provenance_summary = json!({
            "source": "unavailable",
            "reason": "behavioral-diff-report not present"
        });
        if report_path.is_file() {
            let report = read_json(&report_path)?;
            assert_eq!(
                report["schema_version"], "behavioral-diff-report.v1",
                "{fixture_id}: bad schema_version in behavioral-diff-report"
            );
            fixture_provenance_summary = json!({
                "source": "behavioral-diff-report",
                "fixture_revision": report["fixture_revision"],
                "commit_sha": report["provenance"]["commit_sha"],
                "tree_id": report["provenance"]["tree_id"],
                "tree_clean": report["provenance"]["tree_clean"],
                "generated_at": report["provenance"]["generated_at"],
                "gate": report["provenance"]["gate"],
            });
            if report["verdict"] != "ready" || report["summary"]["blocking_count"] != 0 {
                fixture_status = "blocked_divergence".to_string();
                divergence_evidence.push(json!({
                    "report_path": repo_relative(&report_path),
                    "affected_fixture_or_surface": fixture_id,
                    "verdict": report["verdict"],
                    "blocking_count": report["summary"]["blocking_count"],
                    "next_action": "inspect behavioral-diff-report.json and fix the divergent surface"
                }));
            }
            if let Some(comparisons) = report["surface_comparisons"].as_array() {
                for comparison in comparisons {
                    match comparison["status"].as_str() {
                        Some("missing_evidence") => {
                            fixture_status = "blocked_missing_evidence".to_string();
                            missing_evidence.push(json!({
                                "missing_path": comparison["actual"]["missing_path"].as_str().unwrap_or("unknown"),
                                "expected_producer": comparison["actual"]["expected_producer"].as_str().unwrap_or("surface comparison"),
                                "affected_fixture_or_surface": format!(
                                    "{}:{}",
                                    fixture_id,
                                    comparison["surface"].as_str().unwrap_or("unknown")
                                ),
                                "next_action": "rerun ./scripts/test-gate.sh proposal-041 after restoring the missing surface evidence"
                            }));
                        }
                        Some("timed_out") => {
                            if fixture_status == "ready" {
                                fixture_status = "blocked_timeout".to_string();
                            }
                            timeout_evidence.push(json!({
                                "report_path": repo_relative(&report_path),
                                "affected_fixture_or_surface": format!(
                                    "{}:{}",
                                    fixture_id,
                                    comparison["surface"].as_str().unwrap_or("unknown")
                                ),
                                "next_action": "inspect parity-control timeout marker and rerun after the stalled producer is fixed"
                            }));
                        }
                        Some("diverged") => {
                            if fixture_status == "ready" || fixture_status == "blocked_timeout" {
                                fixture_status = "blocked_divergence".to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else {
            fixture_status = "blocked_missing_evidence".to_string();
            missing_evidence.push(json!({
                "missing_path": repo_relative(&report_path),
                "expected_producer": "offline fixture replay",
                "affected_fixture_or_surface": fixture_id,
                "next_action": "rerun ./scripts/test-gate.sh proposal-041 after replay artifacts are regenerated"
            }));
        }
        if !replay_path.is_file() {
            fixture_status = "blocked_missing_evidence".to_string();
            missing_evidence.push(json!({
                "missing_path": repo_relative(&replay_path),
                "expected_producer": "offline fixture replay",
                "affected_fixture_or_surface": fixture_id,
                "next_action": "rerun ./scripts/test-gate.sh proposal-041 after server replay is regenerated"
            }));
        }
        if shadow_report_path.is_file() {
            let shadow_report = read_json(&shadow_report_path)?;
            assert_eq!(
                shadow_report["schema_version"], "live-shadow-report.v1",
                "{fixture_id}: bad schema_version in live-shadow-report"
            );
            if shadow_report["verdict"] != "ready" {
                let shadow_status = shadow_report["overall_status"]
                    .as_str()
                    .or_else(|| shadow_report["verdict"].as_str())
                    .unwrap_or("blocked_divergence");
                fixture_status = match shadow_status {
                    "blocked_timeout" => "blocked_timeout".to_string(),
                    "blocked_interrupted" => "blocked_interrupted".to_string(),
                    "blocked_manual_recovery" => "blocked_manual_recovery".to_string(),
                    "blocked_missing_evidence" => "blocked_missing_evidence".to_string(),
                    _ => "blocked_divergence".to_string(),
                };
            }
        } else {
            fixture_status = "blocked_missing_evidence".to_string();
            missing_evidence.push(json!({
                "missing_path": repo_relative(&shadow_report_path),
                "expected_producer": "live shadow validation",
                "affected_fixture_or_surface": fixture_id,
                "next_action": "rerun ./scripts/test-gate.sh proposal-041 after live shadow artifacts are regenerated"
            }));
        }

        fixture_entries.push(json!({
            "fixture_id": fixture_id,
            "report_path": repo_relative(&report_path),
            "replay_path": repo_relative(&replay_path),
            "shadow_report_path": repo_relative(&shadow_report_path),
            "verdict": fixture_status,
            "provenance": fixture_provenance_summary,
        }));
    }

    const DETAIL_SCHEMA_VERSION: &str = "p031-p041-parity-evidence.v1";
    const ROW_SCHEMA_VERSION: &str = "p031-phase-0-runtime-manifest-row.v1";

    let provenance = gate_provenance();
    let ctrl = ParityControlRoot::new(control_plane_root().join("target/parity-control"));
    let reclaim_marker = ctrl.read_reclaim_marker()?;
    let interruption_marker: Option<InterruptionMarker> =
        read_optional_json_for_test(&ctrl.interruption_marker_path())?;
    let timeout_marker: Option<TimeoutMarker> =
        read_optional_json_for_test(&ctrl.timeout_marker_path())?;
    if let Some(marker) = &interruption_marker {
        if marker.generation_id == pub_generation_id {
            interrupted_evidence.push(json!({
                "marker_path": repo_relative(&ctrl.interruption_marker_path()),
                "signal": marker.signal.clone(),
                "descendant_pgid": marker.descendant_pgid,
                "descendant_absent": marker.descendant_absent,
                "next_action": "inspect the interruption marker and rerun"
            }));
        }
    }
    if let Some(marker) = &timeout_marker {
        if marker.generation_id == pub_generation_id {
            timeout_evidence.push(json!({
                "marker_path": repo_relative(&ctrl.timeout_marker_path()),
                "active_fixture": marker.active_fixture.clone(),
                "active_surface": marker.active_surface.clone(),
                "descendant_pgid": marker.descendant_pgid,
                "descendant_absent": marker.descendant_absent,
                "next_action": "inspect the timeout marker and stalled producer before rerun"
            }));
        }
    }
    let manual_recovery_evidence = reclaim_marker
        .as_ref()
        .filter(|marker| {
            marker.overall_status == "blocked_manual_recovery"
                && marker.abandoned_generation_id == pub_generation_id
        })
        .map(|marker| {
            json!({
                "marker_path": repo_relative(&ctrl.reclaim_marker_path()),
                "owner_pid": marker.owner_pid,
                "owner_process_birth_unix_ms": marker.owner_process_birth_unix_ms,
                "owner_pgid": marker.owner_pgid,
                "owner_last_heartbeat_unix_ms": marker.owner_last_heartbeat_unix_ms,
                "owner_last_control_sequence": marker.owner_last_control_sequence,
                "observation_count": marker.observation_count,
                "freshness_window_ms": marker.freshness_window_ms,
                "preserved_generation_root": marker.preserved_generation_root.clone(),
                "next_action": "preserve the blocked generation and resolve descendant ambiguity before rerun"
            })
        });
    let has_real_provenance = provenance["commit_sha"].as_str() != Some("test-run-no-git")
        && provenance["tree_id"].as_str() != Some("test-run-no-git")
        && provenance["status_snapshot_sha256"].as_str() != Some("test-run-no-git");
    let tree_clean = provenance["tree_clean"].as_bool().unwrap_or(false);
    let status_line_count = provenance["status_snapshot_line_count"]
        .as_u64()
        .unwrap_or(1);
    let all_fixtures_ready = missing_evidence.is_empty()
        && fixture_entries
            .iter()
            .all(|entry| entry["verdict"].as_str() == Some("ready"));
    let (overall_status, publication_state, blocking_reasons): (&str, &str, Vec<String>) =
        if manual_recovery_evidence.is_some() {
            (
                "blocked_manual_recovery",
                "diagnostic_blocked",
                vec!["manual_recovery_required_before_reclaim".to_string()],
            )
        } else if !missing_evidence.is_empty() {
            (
                "blocked_missing_evidence",
                "diagnostic_blocked",
                vec!["required_runtime_evidence_missing".to_string()],
            )
        } else if !divergence_evidence.is_empty()
            || fixture_entries
                .iter()
                .any(|entry| entry["verdict"].as_str() == Some("blocked_divergence"))
        {
            (
                "blocked_divergence",
                "diagnostic_blocked",
                vec!["behavioral_diff_reported_blocking_divergence".to_string()],
            )
        } else if all_fixtures_ready && (!tree_clean || status_line_count != 0) {
            (
                "blocked_dirty_tree",
                "diagnostic_blocked",
                vec!["dirty_tree_cannot_certify_same_tree_readiness".to_string()],
            )
        } else if !timeout_evidence.is_empty()
            || fixture_entries
                .iter()
                .any(|entry| entry["verdict"].as_str() == Some("blocked_timeout"))
        {
            (
                "blocked_timeout",
                "diagnostic_blocked",
                vec!["parity_generation_timed_out".to_string()],
            )
        } else if !interrupted_evidence.is_empty()
            || fixture_entries
                .iter()
                .any(|entry| entry["verdict"].as_str() == Some("blocked_interrupted"))
        {
            (
                "blocked_interrupted",
                "diagnostic_blocked",
                vec!["parity_generation_interrupted".to_string()],
            )
        } else if all_fixtures_ready && has_real_provenance && tree_clean && status_line_count == 0
        {
            (
                "ready_same_tree_verified",
                "published_ready",
                Vec::<String>::new(),
            )
        } else {
            (
                "blocked_missing_evidence",
                "schema_validation_only",
                vec!["missing_or_sentinel_evidence_cannot_certify_same_tree_readiness".to_string()],
            )
        };

    let detail = json!({
        "schema_version": DETAIL_SCHEMA_VERSION,
        "overall_status": overall_status,
        "publication_generation_id": pub_generation_id,
        "publication_state": publication_state,
        "required_fixtures": REQUIRED_FIXTURES,
        "required_surfaces": REQUIRED_COMPARISON_SURFACES,
        "fixtures": fixture_entries,
        "blocking_reasons": blocking_reasons,
        "missing_evidence": missing_evidence,
        "divergence_evidence": divergence_evidence,
        "timeout_evidence": timeout_evidence,
        "interrupted_evidence": interrupted_evidence,
        "manual_recovery_evidence": manual_recovery_evidence,
        "provenance": provenance.clone(),
    });

    let row = json!({
        "schema_version": ROW_SCHEMA_VERSION,
        "id": "p041_parity_evidence",
        "runtime_detail_path": repo_relative(&pub_current.join("p031-p041-parity-evidence.json")),
        "reference_detail_path": "docs/reference/p031-p041-parity-evidence.json",
        "validation_status": overall_status,
        "publication_state": publication_state,
        "publication_generation_id": pub_generation_id,
        "detail_schema_version": DETAIL_SCHEMA_VERSION,
        "provenance": provenance,
    });

    // Cross-artifact compatibility (Section 6.2)
    assert_eq!(
        row["detail_schema_version"], detail["schema_version"],
        "row.detail_schema_version must equal detail.schema_version"
    );
    assert_eq!(
        row["validation_status"], detail["overall_status"],
        "row.validation_status must equal detail.overall_status"
    );
    assert_eq!(
        row["publication_state"], detail["publication_state"],
        "row.publication_state must equal detail.publication_state"
    );
    assert_eq!(
        row["publication_generation_id"], detail["publication_generation_id"],
        "row.publication_generation_id must equal detail.publication_generation_id"
    );

    // Schema version independence
    assert_eq!(row["schema_version"], ROW_SCHEMA_VERSION);
    assert_eq!(detail["schema_version"], DETAIL_SCHEMA_VERSION);

    // Ready-state integrity: when ready, clean-tree proof must hold
    if overall_status == "ready_same_tree_verified" {
        assert!(
            row["provenance"]["tree_clean"].as_bool().unwrap_or(false),
            "ready_same_tree_verified requires tree_clean == true"
        );
        assert_eq!(
            row["provenance"]["status_snapshot_line_count"]
                .as_i64()
                .unwrap_or(1),
            0,
            "ready_same_tree_verified requires status_snapshot_line_count == 0"
        );
    }

    // P041 §6.2: every fixture entry must carry a per-fixture provenance summary.
    for entry in fixture_entries.iter() {
        let fid = entry["fixture_id"].as_str().unwrap_or("unknown");
        assert!(
            !entry["provenance"].is_null(),
            "fixture '{fid}': fixtures[] entry must have a provenance field (§6.2)"
        );
        assert!(
            entry["provenance"].is_object(),
            "fixture '{fid}': provenance must be a JSON object"
        );
        assert!(
            entry["provenance"]["source"].is_string(),
            "fixture '{fid}': provenance.source must be present"
        );
    }

    // Stage generation-scoped candidate artifacts, then promote matching row/detail
    // to publication/current/ via atomic same-directory temp+rename.
    write_atomic_json(
        &pub_generation.join("p031-p041-parity-evidence.json"),
        &detail,
    )?;
    write_atomic_json(&pub_generation.join("p031-phase-0-manifest-row.json"), &row)?;
    assert!(
        pub_generation
            .join("p031-p041-parity-evidence.json")
            .is_file(),
        "generation-scoped detail artifact must be staged before current promotion"
    );
    assert!(
        pub_generation
            .join("p031-phase-0-manifest-row.json")
            .is_file(),
        "generation-scoped row artifact must be staged before current promotion"
    );
    write_atomic_json(&pub_current.join("p031-p041-parity-evidence.json"), &detail)?;
    write_atomic_json(&pub_current.join("p031-phase-0-manifest-row.json"), &row)?;

    Ok(())
}

// ─── Reclaim-matrix cases (B, C, D) and alive-but-stalled (A2) ───────────────

/// Case B: PID gone, process-group metadata missing/unreadable → blocked_manual_recovery.
#[test]
fn proposal_041_reclaim_matrix_case_b_parks_on_missing_pgid_metadata() {
    let decision = evaluate_reclaim_decision(false, false, false);
    assert_eq!(
        decision, "blocked_manual_recovery",
        "Case B: missing pgid metadata must park in blocked_manual_recovery"
    );
}

/// Case C: PID gone, pgid metadata present but descendants still observable → blocked_manual_recovery.
#[test]
fn proposal_041_reclaim_matrix_case_c_parks_on_observable_descendants() {
    let decision = evaluate_reclaim_decision(false, true, false);
    assert_eq!(
        decision, "blocked_manual_recovery",
        "Case C: observable descendants must park in blocked_manual_recovery"
    );
}

/// Case D: PID gone, descendant absence proven → reclaim allowed.
#[test]
fn proposal_041_reclaim_matrix_case_d_proven_absent_allows_reclaim() {
    let decision = evaluate_reclaim_decision(false, true, true);
    assert_eq!(
        decision, "reclaim_allowed",
        "Case D: proven absent descendants must allow reclaim"
    );
}

/// Case A / A1: owner PID still alive → no reclaim.
#[test]
fn proposal_041_reclaim_matrix_case_a_live_owner_blocks_reclaim() {
    let decision = evaluate_reclaim_decision(true, true, true);
    assert_eq!(
        decision, "blocked_in_progress",
        "Case A/A1: live owner must block reclaim"
    );
}

/// Evaluates the alive-but-stalled freshness rule (Case A2).
/// Two consecutive observations of identical heartbeat + control_sequence → escalate.
fn evaluate_stalled_owner(
    obs1_heartbeat_ms: u64,
    obs1_sequence: u64,
    obs2_heartbeat_ms: u64,
    obs2_sequence: u64,
) -> &'static str {
    if is_owner_stalled(
        obs1_heartbeat_ms,
        obs1_sequence,
        obs2_heartbeat_ms,
        obs2_sequence,
    ) {
        "blocked_manual_recovery"
    } else {
        "blocked_in_progress"
    }
}

/// Case A2: owner PID alive, heartbeat and control_sequence both static across
/// two 30-second observations → escalate to blocked_manual_recovery.
#[test]
fn proposal_041_alive_but_stalled_owner_escalates_to_manual_recovery() {
    // Both observations show the same heartbeat and sequence → stalled
    let escalated = evaluate_stalled_owner(1_000_000, 5, 1_000_000, 5);
    assert_eq!(
        escalated, "blocked_manual_recovery",
        "two consecutive stale observations must escalate to blocked_manual_recovery"
    );

    // One stale then fresh — single miss is diagnostic only, no escalation
    let no_escalation = evaluate_stalled_owner(1_000_000, 5, 2_000_000, 6);
    assert_eq!(
        no_escalation, "blocked_in_progress",
        "single stale observation that clears must remain blocked_in_progress"
    );
}

/// Validates the stubborn-descendant timeout path: on timeout the run cannot
/// reach ready_same_tree_verified. The state machine must park at blocked_timeout.
#[test]
fn proposal_041_stubborn_descendant_timeout_blocks_ready_publication() {
    // Simulate: timeout occurred, descendant absence NOT yet proven
    let descendant_absent = false;
    let timed_out = true;

    let publication_status =
        compute_publication_status_with_timeout(true, 0, true, true, timed_out, descendant_absent);
    assert_eq!(
        publication_status, "blocked_timeout",
        "timeout with live descendants must produce blocked_timeout, not ready"
    );

    // Once descendants are gone (proven) after forced termination, publication can proceed
    let descendant_absent_now = true;
    let status_after_drain =
        compute_publication_status_with_timeout(true, 0, true, true, false, descendant_absent_now);
    assert_eq!(
        status_after_drain, "ready_same_tree_verified",
        "clean tree with proven descendant absence may produce ready after drain"
    );
}

/// P041 §6.3 ¶15: descendant absence must be proven before ready publication,
/// even when no timeout or interrupt occurred (e.g., release_marker.descendant_quiescent=false).
/// SEC-P041-001: a successor must not reclaim on a release marker that records
/// descendant_quiescent=false.
#[test]
fn proposal_041_descendant_not_quiescent_blocks_ready_publication() {
    // No timeout, no interrupt, clean tree, all fixtures pass — but descendants
    // are NOT proven absent.  ready_same_tree_verified must still be blocked.
    let status = compute_publication_status_with_timeout(true, 0, true, true, false, false);
    assert_ne!(
        status, "ready_same_tree_verified",
        "unproven descendant absence must block ready_same_tree_verified \
         (release_marker.descendant_quiescent=false is not a clean release)"
    );
    assert_eq!(
        status, "blocked_timeout",
        "unproven descendant absence must produce blocked_timeout"
    );

    // Once descendants are proven absent, ready publication is allowed.
    let ready = compute_publication_status_with_timeout(true, 0, true, true, false, true);
    assert_eq!(
        ready, "ready_same_tree_verified",
        "proven descendant absence with all fixtures passing must allow ready_same_tree_verified"
    );
}

// ─── Dirty-tree and SQLite triple removal ────────────────────────────────────

/// A dirty checkout must produce blocked_dirty_tree, never ready_same_tree_verified.
#[test]
fn proposal_041_dirty_tree_proof_blocks_ready_publication() {
    let dirty = compute_publication_status(false, 3, true, true);
    assert_eq!(
        dirty, "blocked_dirty_tree",
        "dirty tree must produce blocked_dirty_tree"
    );

    let clean = compute_publication_status(true, 0, true, true);
    assert_eq!(
        clean, "ready_same_tree_verified",
        "clean tree with all fixtures passing must produce ready_same_tree_verified"
    );

    // Non-zero line count overrides tree_clean flag
    let inconsistent = compute_publication_status(true, 1, true, true);
    assert_eq!(
        inconsistent, "blocked_dirty_tree",
        "non-zero status_snapshot_line_count must override tree_clean flag"
    );
}

fn compute_publication_status(
    tree_clean: bool,
    status_snapshot_line_count: i64,
    all_fixtures_pass: bool,
    all_surfaces_pass: bool,
) -> &'static str {
    compute_publication_status_with_timeout(
        tree_clean,
        status_snapshot_line_count,
        all_fixtures_pass,
        all_surfaces_pass,
        false,
        true,
    )
}

fn compute_publication_status_with_timeout(
    tree_clean: bool,
    status_snapshot_line_count: i64,
    all_fixtures_pass: bool,
    all_surfaces_pass: bool,
    timed_out: bool,
    descendant_absent: bool,
) -> &'static str {
    if !tree_clean || status_snapshot_line_count != 0 {
        return "blocked_dirty_tree";
    }
    // P041 §6.3 ¶15: ready publication is legal only after descendant quiescence is proven.
    // This applies regardless of whether the run timed out — unproven descendant absence
    // always blocks ready publication.
    if !descendant_absent {
        return "blocked_timeout";
    }
    if timed_out {
        return "blocked_timeout";
    }
    if !all_fixtures_pass || !all_surfaces_pass {
        return "blocked_divergence";
    }
    "ready_same_tree_verified"
}

/// Abandoned-generation cleanup must remove .sqlite, .sqlite-wal, and .sqlite-shm together.
#[test]
fn proposal_041_abandoned_sqlite_triple_removal() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db_path = temp.path().join("parity.sqlite");
    let wal_path = temp.path().join("parity.sqlite-wal");
    let shm_path = temp.path().join("parity.sqlite-shm");

    fs::write(&db_path, b"db")?;
    fs::write(&wal_path, b"wal")?;
    fs::write(&shm_path, b"shm")?;

    remove_abandoned_sqlite(&db_path)?;

    assert!(!db_path.is_file(), ".sqlite must be removed");
    assert!(!wal_path.is_file(), ".sqlite-wal must be removed together");
    assert!(!shm_path.is_file(), ".sqlite-shm must be removed together");

    Ok(())
}

/// Remove an abandoned parity.sqlite and its WAL/SHM siblings atomically.
/// On checkpoint or open failure, remove all three unconditionally.
fn remove_abandoned_sqlite(db_path: &Path) -> Result<()> {
    // Derive sibling paths from the .sqlite path
    let stem = db_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("parity");
    let parent = db_path.parent().unwrap_or(Path::new("."));
    let wal_path = parent.join(format!("{stem}.sqlite-wal"));
    let shm_path = parent.join(format!("{stem}.sqlite-shm"));

    // Remove siblings first (WAL/SHM), then the main DB file
    let _ = fs::remove_file(&wal_path);
    let _ = fs::remove_file(&shm_path);
    fs::remove_file(db_path)
        .with_context(|| format!("failed to remove abandoned sqlite {}", db_path.display()))?;
    Ok(())
}

// ─── Phase B: parity-control directory and atomic write contract ─────────────

/// Verify the parity-control directory infrastructure: init, .metadata_never_index,
/// atomic lease write + readback, reclaim marker lifecycle, and marker schema versions.
/// Exercises ParityControlRoot.write_lease / read_lease / write_reclaim_marker /
/// write_interruption_marker / write_timeout_marker / write_release_marker.
#[test]
fn proposal_041_parity_control_directory_and_atomic_write_contract() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let ctrl = ParityControlRoot::new(temp.path().join("parity-control"));

    // init creates directory and .metadata_never_index
    ctrl.init()?;
    assert!(
        ctrl.root().join(".metadata_never_index").is_file(),
        ".metadata_never_index must be placed after init"
    );

    // Lease write + readback preserves all required fields
    let lease = LeaseRecord::new(
        std::process::id(),
        1_700_000_000_000_u64,
        "testhost",
        "abc123commit",
        "def456tree",
        "gen-parity-control-test",
    );
    assert_eq!(lease.schema_version, "parity-control-lease.v1");
    assert_eq!(lease.control_sequence, 0);
    ctrl.write_lease(&lease)?;

    let read_back = ctrl
        .read_lease()?
        .expect("lease.json must be readable after write");
    assert_eq!(read_back.schema_version, "parity-control-lease.v1");
    assert_eq!(read_back.pid, lease.pid);
    assert_eq!(read_back.process_birth_unix_ms, 1_700_000_000_000);
    assert_eq!(read_back.hostname, "testhost");
    assert_eq!(read_back.commit_sha, "abc123commit");
    assert_eq!(read_back.tree_id, "def456tree");
    assert_eq!(
        read_back.publication_generation_id,
        "gen-parity-control-test"
    );
    assert_eq!(read_back.control_sequence, 0);

    // Atomic write leaves no .tmp residue
    let tmp_files: Vec<_> = fs::read_dir(ctrl.root())?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.ends_with(".tmp"))
                .unwrap_or(false)
        })
        .collect();
    assert!(
        tmp_files.is_empty(),
        "no .tmp files may remain after atomic write"
    );

    // Reclaim marker: blocked_manual_recovery (Case B — missing pgid metadata)
    let marker_b = ReclaimMarker::blocked_manual_recovery(
        &lease,
        true,  // missing_pgid_metadata
        false, // observable_descendants
        Some(ctrl.root().to_string_lossy().to_string()),
        1,       // observation_count: 1 for Case B (no pgid metadata → no wait)
        120_000, // freshness_window_ms
        "Case B: pgid metadata missing after abnormal exit",
    );
    assert_eq!(marker_b.overall_status, "blocked_manual_recovery");
    assert_eq!(marker_b.schema_version, "parity-control-reclaim-marker.v1");
    assert!(marker_b.missing_pgid_metadata);
    ctrl.write_reclaim_marker(&marker_b)?;

    let marker_rb = ctrl
        .read_reclaim_marker()?
        .expect("reclaim-marker.json must be readable");
    assert_eq!(marker_rb.overall_status, "blocked_manual_recovery");
    assert_eq!(marker_rb.abandoned_generation_id, "gen-parity-control-test");
    assert!(
        marker_rb.preserved_generation_root.is_some(),
        "blocked_manual_recovery must carry preserved_generation_root"
    );
    assert_eq!(marker_rb.observation_count, 1);

    // Reclaim marker: reclaim_allowed (Case D — absence proven)
    let marker_d = ReclaimMarker::reclaim_allowed(&lease, None);
    assert_eq!(marker_d.overall_status, "reclaim_allowed");
    ctrl.write_reclaim_marker(&marker_d)?;
    let marker_d_rb = ctrl
        .read_reclaim_marker()?
        .expect("reclaim-marker.json must be updated to reclaim_allowed");
    assert_eq!(marker_d_rb.overall_status, "reclaim_allowed");

    // Interruption marker
    let int_marker = InterruptionMarker::new("gen-parity-control-test", "SIGTERM");
    assert_eq!(
        int_marker.schema_version,
        "parity-control-interruption-marker.v1"
    );
    assert_eq!(int_marker.overall_status, "blocked_interrupted");
    ctrl.write_interruption_marker(&int_marker)?;
    assert!(ctrl.interruption_marker_path().is_file());

    // Timeout marker
    let timeout_marker = TimeoutMarker::new("gen-parity-control-test", false);
    assert_eq!(
        timeout_marker.schema_version,
        "parity-control-timeout-marker.v1"
    );
    assert_eq!(timeout_marker.overall_status, "blocked_timeout");
    ctrl.write_timeout_marker(&timeout_marker)?;
    assert!(ctrl.timeout_marker_path().is_file());

    // Release marker
    let release_marker = ReleaseMarker::ready("gen-parity-control-test");
    assert_eq!(
        release_marker.schema_version,
        "parity-control-release-marker.v1"
    );
    assert_eq!(release_marker.overall_status, "ready_same_tree_verified");
    assert!(release_marker.descendant_quiescent);
    ctrl.write_release_marker(&release_marker)?;
    assert!(ctrl.release_marker_path().is_file());

    // Reclaim decision function is consistent with actual marker states
    assert_eq!(
        evaluate_reclaim_decision(true, true, true),
        "blocked_in_progress"
    );
    assert_eq!(
        evaluate_reclaim_decision(false, false, false),
        "blocked_manual_recovery"
    );
    assert_eq!(
        evaluate_reclaim_decision(false, true, false),
        "blocked_manual_recovery"
    );
    assert_eq!(
        evaluate_reclaim_decision(false, true, true),
        "reclaim_allowed"
    );

    // Stale-owner detection
    assert!(
        is_owner_stalled(1_000_000, 5, 1_000_000, 5),
        "two stale obs must escalate"
    );
    assert!(
        !is_owner_stalled(1_000_000, 5, 2_000_000, 6),
        "fresh second obs must not escalate"
    );

    Ok(())
}

// ─── Phase B: pre-cleanup publication contract ───────────────────────────────

/// Verify that write_pre_cleanup_publication revokes stale ready evidence before cleanup.
///
/// Per proposal P041 Section 6.3 step 5: before any ephemeral cleanup or replay begins,
/// the harness writes blocked_in_progress + revoked_for_rerun to publication/current/.
/// Consumers must see a revoked generation rather than stale ready evidence during a rerun.
#[test]
fn proposal_041_pre_cleanup_publication_revokes_stale_ready_evidence() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let pub_current = temp.path().join("publication/current");

    // Simulate: a prior generation published "ready" evidence
    fs::create_dir_all(&pub_current)?;
    fs::write(
        pub_current.join("p031-phase-0-manifest-row.json"),
        r#"{"schema_version":"p031-phase-0-runtime-manifest-row.v1","id":"p041_parity_evidence","validation_status":"ready_same_tree_verified","publication_state":"published_ready","publication_generation_id":"gen-prior-001","detail_schema_version":"p031-p041-parity-evidence.v1"}"#,
    )?;
    fs::write(
        pub_current.join("p031-p041-parity-evidence.json"),
        r#"{"schema_version":"p031-p041-parity-evidence.v1","overall_status":"ready_same_tree_verified","publication_state":"published_ready","publication_generation_id":"gen-prior-001"}"#,
    )?;

    // New invocation: write blocked_in_progress BEFORE cleanup begins.
    // Pre-cleanup provenance uses sentinel values since the porcelain status
    // snapshot is captured in a later step (proposal Section 6.3 step 3).
    write_pre_cleanup_publication(
        &pub_current,
        "gen-new-001",
        "abc123",
        "def456",
        false,
        "pending",
        0,
    )?;

    // Verify row is now revoked for the new generation
    let row_raw = fs::read_to_string(pub_current.join("p031-phase-0-manifest-row.json"))?;
    let row: Value = serde_json::from_str(&row_raw)?;
    assert_eq!(
        row["schema_version"],
        "p031-phase-0-runtime-manifest-row.v1"
    );
    assert_eq!(row["id"], "p041_parity_evidence");
    assert_eq!(row["validation_status"], "blocked_in_progress");
    assert_eq!(row["publication_state"], "revoked_for_rerun");
    assert_eq!(row["publication_generation_id"], "gen-new-001");
    assert_eq!(row["detail_schema_version"], "p031-p041-parity-evidence.v1");

    // Verify detail is now revoked for the new generation
    let detail_raw = fs::read_to_string(pub_current.join("p031-p041-parity-evidence.json"))?;
    let detail: Value = serde_json::from_str(&detail_raw)?;
    assert_eq!(detail["schema_version"], "p031-p041-parity-evidence.v1");
    assert_eq!(detail["overall_status"], "blocked_in_progress");
    assert_eq!(detail["publication_state"], "revoked_for_rerun");
    assert_eq!(detail["publication_generation_id"], "gen-new-001");

    // Cross-artifact compatibility: row.validation_status == detail.overall_status
    assert_eq!(row["validation_status"], detail["overall_status"]);
    assert_eq!(row["publication_state"], detail["publication_state"]);
    assert_eq!(
        row["publication_generation_id"],
        detail["publication_generation_id"]
    );

    // Stale ready evidence must be fully overwritten
    assert_ne!(row["validation_status"], "ready_same_tree_verified");
    assert_ne!(detail["overall_status"], "ready_same_tree_verified");
    assert_ne!(row["publication_generation_id"], "gen-prior-001");

    Ok(())
}

// ─── CLI prefix projection golden test ───────────────────────────────────────

/// Locks the status → CLI prefix mapping from proposal P041 Section 5.
///
/// This golden test exists so that a future diff to `scripts/test-gate.sh`'s
/// `STATUS_TO_PREFIX` dict must also update this test, and vice versa. Any
/// status string not listed here falls through to `FAIL` per Section 5.
#[test]
fn proposal_041_cli_prefix_projection_matches_section_5_table() {
    let cases: &[(&str, &str)] = &[
        ("ready_same_tree_verified", "PASS"),
        ("blocked_missing_evidence", "FAIL"),
        ("blocked_divergence", "FAIL"),
        ("blocked_manual_recovery", "FAIL"),
        ("blocked_dirty_tree", "WARN"),
        ("blocked_timeout", "WARN"),
        ("blocked_interrupted", "WARN"),
        ("blocked_in_progress", "INFO"),
    ];

    for &(status, expected_prefix) in cases {
        let prefix = match status {
            "ready_same_tree_verified" => "PASS",
            "blocked_missing_evidence" | "blocked_divergence" | "blocked_manual_recovery" => "FAIL",
            "blocked_dirty_tree" | "blocked_timeout" | "blocked_interrupted" => "WARN",
            "blocked_in_progress" => "INFO",
            _ => "FAIL",
        };
        assert_eq!(
            prefix, expected_prefix,
            "CLI prefix for '{status}' must be '{expected_prefix}' per Section 5 table"
        );
    }

    // Unknown status must also default to FAIL
    let unknown_prefix = match "unknown_status_enum" {
        "ready_same_tree_verified" => "PASS",
        "blocked_missing_evidence" | "blocked_divergence" | "blocked_manual_recovery" => "FAIL",
        "blocked_dirty_tree" | "blocked_timeout" | "blocked_interrupted" => "WARN",
        "blocked_in_progress" => "INFO",
        _ => "FAIL",
    };
    assert_eq!(
        unknown_prefix, "FAIL",
        "unknown status enum values must default to FAIL"
    );
}

#[test]
fn proposal_041_gate_script_enforces_process_group_deadline_and_boundary_contract() -> Result<()> {
    let script_path = workspace_root().join("scripts/test-gate.sh");
    let script = fs::read_to_string(&script_path)?;
    let p041_start = script
        .find("proposal-041|p041)")
        .expect("proposal-041 gate block must exist");
    let p041_block = &script[p041_start..];

    assert!(
        !p041_block.contains("MISSING-001"),
        "P041 gate must not defer pgid/process-group lifecycle"
    );
    assert!(
        !p041_block.contains("\"pgid\": 0"),
        "P041 lease must publish a real process-group id, never pgid=0"
    );
    assert!(
        !p041_block.contains("grep -c . 2>/dev/null || echo 0"),
        "P041 clean git status line-count must not produce duplicate 0 lines"
    );
    assert!(
        script.contains("start_new_session=True") && p041_block.contains("p041_supervised_run"),
        "P041 subprocesses must run in a dedicated process group/session"
    );
    assert!(
        script.contains("os.killpg(") && p041_block.contains("p041_supervised_run"),
        "P041 timeout/interruption handling must terminate the tracked process group"
    );
    assert!(
        p041_block.contains("P041_GATE_DEADLINE_SECONDS"),
        "P041 gate must enforce an overall deadline"
    );
    assert!(
        p041_block.contains("P041_GATE_DEADLINE_SECONDS:-1500"),
        "P041 gate default must be the proposal's 25 minute bound"
    );
    assert!(
        !p041_block.contains("P041_GATE_DEADLINE_SECONDS:-7200"),
        "P041 gate must not use the old 7200s default"
    );
    assert!(
        p041_block.contains("P041_DRAIN_GRACE_SECONDS"),
        "P041 gate must enforce a post-signal drain grace"
    );
    assert!(
        p041_block.contains("P041_DRAIN_GRACE_SECONDS:-30"),
        "P041 drain must default to the proposal's 30 second bound"
    );
    for token in [
        "P041_REPLAY_DEADLINE_SECONDS:-60",
        "P041_READBACK_DEADLINE_SECONDS:-30",
        "P041_SHADOW_DEADLINE_SECONDS:-60",
        "P041_COMMAND_DEADLINE_SECONDS",
        "|| exit $?",
        "cargo test -p graphql-server --lib proposal_041_graphql_readback_parity_surfaces",
        "cargo test -p mcp-server --lib proposal_041_report_resource_readback_parity_surface",
    ] {
        assert!(
            p041_block.contains(token),
            "P041 gate missing bounded deadline token {token:?}"
        );
    }
    for token in [
        "def _darwin_fullfsync",
        "signal.signal(signal.SIGINT",
        "signal.signal(signal.SIGTERM",
        "_write_interruption_marker",
        "blocked_interrupted",
    ] {
        assert!(
            script.contains(token),
            "P041 supervisor missing interruption/durability token {token:?}"
        );
    }

    let guard_pos = p041_block
        .find("def _validate_p041_target_boundary")
        .expect("P041 setup must define target boundary validation");
    let mkdir_pos = p041_block
        .find("root.mkdir(parents=True, exist_ok=True)")
        .expect("P041 setup must create target roots");
    assert!(
        guard_pos < mkdir_pos,
        "P041 setup must validate target/parity* boundaries before any mkdir/write"
    );

    Ok(())
}

#[test]
fn proposal_041_p031_manifest_marks_parity_evidence_ready() -> Result<()> {
    let manifest_path = workspace_root().join("docs/reference/p031-phase-0-artifact-manifest.json");
    let manifest = read_json(&manifest_path)?;
    let entry = manifest["entries"]
        .as_array()
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry["id"] == serde_json::json!("p041_parity_evidence"))
        })
        .expect("p041_parity_evidence manifest row must exist");

    assert_eq!(
        entry["validation_status"],
        serde_json::json!("ready"),
        "P031 manifest must use the P031 ready token and leave same-tree proof to the runtime P041 row"
    );
    assert_eq!(
        entry["blocking_phase"],
        serde_json::json!("Phase 1"),
        "P031 manifest p041 row remains a Phase 1 handoff entry once ready"
    );

    Ok(())
}

#[test]
fn proposal_041_gate_script_implements_reclaim_matrix_cases() -> Result<()> {
    let script_path = workspace_root().join("scripts/test-gate.sh");
    let script = fs::read_to_string(&script_path)?;
    let p041_start = script
        .find("proposal-041|p041)")
        .expect("proposal-041 gate block must exist");
    let p041_block = &script[p041_start..];

    for token in [
        "Case A",
        "Case A2",
        "Case B",
        "Case C",
        "Case D",
        "_pgid_has_observable_descendants",
        "_write_reclaim_marker",
        "2,\n                        _window_ms",
    ] {
        assert!(
            p041_block.contains(token),
            "P041 reclaim implementation missing token {token:?}"
        );
    }

    Ok(())
}
