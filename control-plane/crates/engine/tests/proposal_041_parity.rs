use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, projections, runs, stages};
use domain::commands::{CallerContext, CancelRunCmd, Command, PrincipalClass, StartRunCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId};
use engine::command_handler::{CommandHandler, CommandResult};
use engine::event_bus;
use engine::work_queue::WorkQueue;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

const REQUIRED_FIXTURES: &[&str] = &[
    "proposal-loop-basic",
    "implementation-refine-review",
    "approval-pause-resume",
    "retry-recovery-flow",
    "cancelled-or-blocked-run",
    "terminal-report-evidence",
    "projection-readback-surface",
];

const REQUIRED_COMPARISON_SURFACES: &[&str] = &[
    "canonical_domain_state",
    "projections",
    "graphql_readback",
    "mcp_report_readback",
    "artifact_identity",
    "operator_summary",
];

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

fn target_parity_reports_root() -> PathBuf {
    target_parity_root().join("reports")
}

fn repo_relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .display()
        .to_string()
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
    create_pool(&format!("sqlite://{}", path.to_string_lossy()))
        .await
        .with_context(|| format!("create fixture DB {}", path.display()))
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
    for fixture_id in REQUIRED_FIXTURES {
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
        assert_eq!(report["run_fixture_id"], *fixture_id);
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
    for fixture_id in REQUIRED_FIXTURES {
        let (dir, fixture) = load_fixture(fixture_id)?;
        let provider_profile = read_json(&dir.join(fixture.frozen_inputs.provider_profile))?;
        assert_eq!(provider_profile["runtime_policy"], "stubbed");
        assert_eq!(provider_profile["live_adapter_invocation"], "forbidden");
        assert!(validate_shadow_replay_request(&ShadowReplayRequest {
            source_run_id: format!("source-{fixture_id}"),
            shadow_run_id: format!("shadow-{fixture_id}"),
            storage_namespace: "shadow".into(),
            artifact_root: format!("target/parity/shadow/{fixture_id}"),
            runtime_policy: "stubbed".into(),
            idempotency_key: format!("p041-{fixture_id}"),
        })
        .is_ok());
        assert!(validate_shadow_replay_request(&ShadowReplayRequest {
            source_run_id: format!("source-{fixture_id}"),
            shadow_run_id: format!("shadow-{fixture_id}"),
            storage_namespace: "production".into(),
            artifact_root: format!("target/parity/shadow/{fixture_id}"),
            runtime_policy: "stubbed".into(),
            idempotency_key: format!("p041-{fixture_id}"),
        })
        .is_err());
        assert!(validate_shadow_replay_request(&ShadowReplayRequest {
            source_run_id: format!("source-{fixture_id}"),
            shadow_run_id: format!("shadow-{fixture_id}"),
            storage_namespace: "shadow".into(),
            artifact_root: format!("target/parity/shadow/{fixture_id}"),
            runtime_policy: "live".into(),
            idempotency_key: format!("p041-{fixture_id}"),
        })
        .is_err());
        let shadow_report = replay_shadow_fixture_and_write_report(fixture_id).await?;
        assert_eq!(shadow_report["schema_version"], "behavioral-diff-report.v1");
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

    let path = workspace_root().join(
        "docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p041-parity.md",
    );
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    assert!(
        raw.contains("| Status | Ready |"),
        "handoff artifact must be ready"
    );
    assert!(raw.contains("| Gate | `./scripts/test-gate.sh proposal-041` |"));
    for fixture_id in REQUIRED_FIXTURES {
        assert!(
            raw.contains(fixture_id),
            "handoff artifact missing fixture {fixture_id}"
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
            raw.contains(&format!(
                "control-plane/target/parity/reports/{fixture_id}/behavioral-diff-report.json"
            )),
            "handoff artifact does not name generated report path for {fixture_id}"
        );
        let replay_path = target_parity_root()
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
        artifact_root: format!("target/parity/shadow/{fixture_id}"),
        runtime_policy: "stubbed".into(),
        idempotency_key: format!("p041-shadow-{fixture_id}"),
    })?;
    replay_fixture_with_mode(
        fixture_id,
        ReplayMode::LiveShadow {
            replay_id: unique_shadow_replay_id(fixture_id),
        },
    )
    .await
}

enum ReplayMode {
    OfflineFixtureReplay,
    LiveShadow { replay_id: String },
}

impl ReplayMode {
    fn mode(&self) -> &'static str {
        match self {
            Self::OfflineFixtureReplay => "offline_fixture_replay",
            Self::LiveShadow { .. } => "live_shadow",
        }
    }

    fn replay_dir(&self, fixture_id: &str) -> PathBuf {
        match self {
            Self::OfflineFixtureReplay => target_parity_root().join(fixture_id),
            Self::LiveShadow { replay_id } => target_parity_root().join("shadow").join(replay_id),
        }
    }

    fn report_dir(&self, fixture_id: &str) -> PathBuf {
        match self {
            Self::OfflineFixtureReplay => target_parity_reports_root().join(fixture_id),
            Self::LiveShadow { .. } => target_parity_root().join("shadow/reports").join(fixture_id),
        }
    }

    fn database_path(&self, fixture_id: &str) -> PathBuf {
        self.replay_dir(fixture_id).join("parity.sqlite")
    }
}

fn unique_shadow_replay_id(fixture_id: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{fixture_id}-{}-{nanos}", std::process::id())
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
            "file_path": artifact.file_path,
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
    fs::write(&replay_path, serde_json::to_string_pretty(&server_replay)?)?;

    let report = json!({
        "schema_version": "behavioral-diff-report.v1",
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
        report_dir.join("behavioral-diff-report.json"),
        serde_json::to_string_pretty(&report)?,
    )?;

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
                    Command::CancelRun(CancelRunCmd { run_id }),
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

    async fn open_session(&self, _req: &acp::ExecutionRequest) -> Result<acp::AcpSessionHandle> {
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
            acp_pre_initialize_local_latency_ms: None,
            acp_initialize_latency_ms: None,
            acp_session_new_latency_ms: None,
            acp_prompt_duration_ms: None,
            acp_pre_prompt_metadata_latency_ms: None,
            acp_pre_prompt_metadata_timeout: false,
            acp_pre_prompt_metadata_digest_bytes: 0,
            legacy_broad_discovery_snapshot: None,
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
    fs::write(
        &catalog_path,
        format!(
            r#"schema_version: 1
artifacts:
{artifact_map}
backend_profiles:
  p041_fixture_profile:
    provider: {}
    model: p041-fixture
agents:
  - id: {}
    backend_profile: p041_fixture_profile
    prompt: "Replay P041 fixture stage through the canonical executor boundary."
"#,
            fixture_agent.provider, fixture_agent.id
        ),
    )?;

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
