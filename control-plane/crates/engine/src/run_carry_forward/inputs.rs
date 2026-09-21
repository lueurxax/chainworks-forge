//! Installed historical inputs are prompt context, never active output authority.
use anyhow::{ensure, Context, Result};
use db::repos::{run_continuation_inputs::InstalledInput, run_continuations};
use domain::{
    run::Run,
    run_carry_forward::{ContentDigest, EntryRole},
};
use sqlx::{Row, SqlitePool};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use workflow::plan::{CompiledTask, RunPlan};

use super::{
    budget::Deadline,
    manifest::read_manifest,
    safe_files::{identity, validate_relative, SafeRoot},
};

const MAX_CONTEXT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputOrigin {
    ExecutionSeed,
    ReferenceOnly,
    ProviderOutput,
    ApprovalSnapshot,
}

#[derive(Debug, Clone)]
pub struct ResolvedInput {
    pub name: String,
    pub content: String,
    pub display_path: String,
    pub origin: InputOrigin,
    pub artifact_id: Option<domain::ids::ArtifactId>,
}

/// `None` is an explicit missing ordinary input, not permission for file fallback.
#[derive(Debug)]
pub struct TaskInputs {
    pub inputs: BTreeMap<String, Option<ResolvedInput>>,
    pub references: Vec<ResolvedInput>,
}

#[derive(Debug)]
pub enum OutputResolution {
    OrdinaryRun,
    Missing,
    Output(ResolvedInput),
}

async fn incoming(pool: &SqlitePool, run: &Run) -> Result<Option<run_continuations::Continuation>> {
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT operation_id FROM run_continuations WHERE reserved_successor_run_id=? LIMIT 2",
    )
    .bind(run.id.to_string())
    .fetch_all(pool)
    .await?;
    ensure!(
        ids.len() <= 1,
        "artifact_provenance_invalid: incoming operation"
    );
    let Some(id) = ids.first() else {
        return Ok(None);
    };
    let op = run_continuations::find(pool, id)
        .await?
        .context("artifact_provenance_invalid: operation missing")?;
    ensure!(
        op.phase == "activated"
            && op.successor_run_id.as_deref() == Some(run.id.to_string().as_str())
            && op.idea_id == run.idea_id.to_string(),
        "artifact_provenance_invalid: input not activated"
    );
    Ok(Some(op))
}

fn metadata_root(run: &Run) -> Result<PathBuf> {
    let path = Path::new(
        run.chainworks_meta_root
            .as_deref()
            .context("artifact_provenance_invalid: metadata root")?,
    );
    Ok(if path.is_absolute() {
        path.into()
    } else {
        Path::new(&run.workspace_root).join(path)
    })
}

fn canonical_relative(run: &Run, plan: &RunPlan, name: &str) -> Result<Option<String>> {
    let Some(template) = plan.artifact_paths.get(name) else {
        return Ok(None);
    };
    let path = crate::orchestrator::resolve_path_template(
        template,
        &run.workspace_root,
        run.chainworks_meta_root.as_deref(),
    );
    let root = metadata_root(run)?;
    let path = Path::new(&path);
    let relative = path
        .strip_prefix(root)
        .context("unsafe_path: input outside canonical metadata")?
        .to_str()
        .context("unsafe_path: non-UTF8 input")?;
    validate_relative(relative)?;
    Ok(Some(relative.into()))
}

async fn read_bound(
    root: PathBuf,
    relative: String,
    expected: ContentDigest,
    expected_size: Option<u64>,
) -> Result<String> {
    tokio::time::timeout(
        Duration::from_secs(120),
        tokio::task::spawn_blocking(move || {
            let deadline = Deadline::after(Duration::from_secs(120));
            let safe = SafeRoot::open(&root)?;
            let mut file = safe.open_file(&relative)?;
            let before = file.metadata()?;
            ensure!(
                before.len() <= MAX_CONTEXT_BYTES,
                "input_context_sources_too_large"
            );
            if let Some(size) = expected_size {
                ensure!(
                    size == before.len(),
                    "artifact_provenance_invalid: input size"
                );
            }
            let mut bytes = Vec::new();
            let mut buffer = vec![0; 1024 * 1024];
            loop {
                deadline.check()?;
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                ensure!(
                    bytes.len() as u64 + n as u64 <= MAX_CONTEXT_BYTES,
                    "input_context_sources_too_large"
                );
                bytes.extend_from_slice(&buffer[..n]);
            }
            ensure!(
                ContentDigest::of(&bytes) == expected,
                "artifact_provenance_invalid: input digest"
            );
            safe.verify_path(&root)?;
            ensure!(
                identity(&before) == identity(&file.metadata()?)
                    && identity(&before) == identity(&safe.open_file(&relative)?.metadata()?),
                "artifact_provenance_invalid: input changed"
            );
            Ok(String::from_utf8(bytes).context("artifact_provenance_invalid: non-text input")?)
        }),
    )
    .await
    .context("continuation_budget_exceeded: input deadline")?
    .context("input read task failed")?
}

/// The existing active contract index remains authoritative for structured outputs.
/// Mixed tasks retain their aggregate facts and prove each output against its frozen declaration.
async fn active_output(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
    name: &str,
) -> Result<Option<ResolvedInput>> {
    let Some(relative) = canonical_relative(run, plan, name)? else {
        return Ok(None);
    };
    let root = metadata_root(run)?;
    let expected_path = root.join(&relative);
    let contract = db::repos::artifact_contracts::contract_id_for_alias(name);
    let rows = sqlx::query("SELECT a.id,a.file_path,a.checksum_sha256,a.size_bytes,a.created_at,a.contract_id,a.agent_id,a.provider,s.stage_id,s.id AS stage_execution_id,e.id AS agent_execution_id,f.valid_required_outputs FROM artifacts a
        JOIN agent_executions e ON e.id=a.agent_execution_id
        JOIN stage_executions s ON s.id=e.stage_execution_id AND s.run_id=a.run_id
        JOIN agent_execution_runtime_facts f ON f.agent_execution_id=e.id
        WHERE a.run_id=? AND a.name=?
          AND a.provider=e.provider AND a.agent_id=e.agent_id
          AND (f.valid_required_outputs=1 OR (e.status='completed' AND f.failure_kind IS NULL AND f.output_settlement='valid_outputs_from_completed_execution'))
          AND f.output_settlement IN ('valid_outputs_from_completed_execution','valid_outputs_from_failed_execution','valid_outputs_from_repair')
          AND e.status IN ('completed','failed')
          AND NOT EXISTS(SELECT 1 FROM artifact_source_generation_claims c WHERE c.agent_execution_id=e.id AND (c.claim_state LIKE 'superseded%' OR c.superseded_by_agent_execution_id IS NOT NULL))
          AND NOT EXISTS(SELECT 1 FROM validation_failure_records v WHERE v.artifact_id=a.id)
          AND (? IS NULL OR EXISTS(SELECT 1 FROM artifact_contract_generations g JOIN active_artifact_contracts ac ON ac.generation_id=g.generation_id AND ac.run_id=g.run_id AND ac.contract_id=g.contract_id WHERE g.artifact_id=a.id AND g.run_id=a.run_id AND g.valid=1 AND g.contract_id=?))
        ORDER BY a.created_at DESC,a.id DESC LIMIT 2")
        .bind(run.id.to_string()).bind(name).bind(contract).bind(contract).fetch_all(pool).await?;
    let Some(row) = rows.first() else {
        return if name == "approved_proposal" {
            approved_output(pool, run, plan).await
        } else {
            Ok(None)
        };
    };
    let mut declared_schema = None;
    if !row.try_get::<bool, _>("valid_required_outputs")? {
        let stage: String = row.try_get("stage_id")?;
        let agent: String = row.try_get("agent_id")?;
        let provider: String = row.try_get("provider")?;
        let tasks: Vec<_> = plan
            .states
            .get(&stage)
            .into_iter()
            .flat_map(|state| state.tasks.iter().chain(&state.post_approval_tasks))
            .filter(|task| {
                task.agent.agent_id == agent && task.outputs.iter().any(|output| output == name)
            })
            .collect();
        let [task] = tasks.as_slice() else {
            anyhow::bail!(
                "artifact_provenance_invalid: undeclared plain output or mixed-output task"
            );
        };
        verify_effective_provider(
            pool,
            run,
            plan,
            task,
            &row.try_get::<String, _>("stage_execution_id")?,
            &row.try_get::<String, _>("agent_execution_id")?,
            &provider,
        )
        .await?;
        declared_schema = task.output_schemas.get(name);
        let artifact_contract: String = row.try_get("contract_id")?;
        match declared_schema {
            None => ensure!(
                contract.is_none() && artifact_contract == format!("{provider}.output"),
                "artifact_provenance_invalid: plain output contract"
            ),
            Some(schema) => {
                ensure!(
                    artifact_contract == schema.contract_id,
                    "artifact_provenance_invalid: frozen output contract"
                );
                if domain::artifact_contracts::known_contract_id(&schema.contract_id) {
                    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM artifact_contract_generations g JOIN active_artifact_contracts ac ON ac.generation_id=g.generation_id AND ac.run_id=g.run_id AND ac.contract_id=g.contract_id WHERE g.run_id=? AND g.artifact_id=? AND g.contract_id=? AND g.valid=1 AND g.source_agent_execution_id=? AND g.source_stage_execution_id=?)")
                        .bind(run.id.to_string()).bind(row.try_get::<String,_>("id")?).bind(&schema.contract_id)
                        .bind(row.try_get::<String,_>("agent_execution_id")?).bind(row.try_get::<String,_>("stage_execution_id")?)
                        .fetch_one(pool).await?;
                    ensure!(
                        active,
                        "artifact_provenance_invalid: inactive structured output"
                    );
                }
            }
        }
    }
    if let Some(other) = rows.get(1) {
        ensure!(
            row.try_get::<String, _>("created_at")? != other.try_get::<String, _>("created_at")?,
            "artifact_provenance_invalid: ambiguous output"
        );
    }
    ensure!(
        Path::new(&row.try_get::<String, _>("file_path")?) == expected_path,
        "artifact_provenance_invalid: output path"
    );
    let hash: Option<String> = row.try_get("checksum_sha256")?;
    let hash = ContentDigest::from_sha256_hex(
        hash.as_deref()
            .context("artifact_provenance_invalid: output digest missing")?,
    )?;
    let size: Option<i64> = row.try_get("size_bytes")?;
    let size = u64::try_from(size.context("artifact_provenance_invalid: output size missing")?)?;
    let content = read_bound(root, relative, hash, Some(size)).await?;
    if let Some(schema) = declared_schema {
        ensure!(
            crate::contracts::validate_output(name, content.as_bytes(), Some(schema)).status
                == domain::validation::ValidationStatus::Passed,
            "artifact_provenance_invalid: structured output validation"
        );
    }
    Ok(Some(ResolvedInput {
        name: name.into(),
        content,
        display_path: expected_path.to_string_lossy().into_owned(),
        origin: InputOrigin::ProviderOutput,
        artifact_id: Some(row.try_get::<String, _>("id")?.parse()?),
    }))
}

/// An alternate provider is evidence only when the exact claimed invocation records
/// an allowed fallback from this frozen task to this frozen backend.
pub(crate) async fn verify_effective_provider(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
    task: &CompiledTask,
    stage_execution_id: &str,
    agent_execution_id: &str,
    provider: &str,
) -> Result<()> {
    use crate::orchestrator::{
        is_health_fallback_eligible_task, is_health_fallback_source_provider,
        provider_health_fallback_failure, run_local_health_fallback_profile_candidates,
        same_provider_family_for_health_fallback,
    };
    use serde_json::{json, Value};
    if provider == task.agent.provider {
        return Ok(());
    }
    ensure!(
        is_health_fallback_eligible_task(
            &task.agent.agent_id,
            &task.outputs,
            task.agent.output_contract.as_deref()
        ) && is_health_fallback_source_provider(&task.agent.provider)
            && !same_provider_family_for_health_fallback(provider, &task.agent.provider),
        "artifact_provenance_invalid: provider fallback policy"
    );
    let rows = sqlx::query("SELECT w.payload_json,e.model,s.stage_id FROM work_items w
        JOIN agent_executions e ON e.id=json_extract(w.payload_json,'$.p058_claimed.agent_execution_id')
        JOIN stage_executions s ON s.id=e.stage_execution_id
        WHERE w.kind='invoke_agent' AND w.status IN ('running','completed','failed') AND w.run_id=?
          AND s.run_id=w.run_id AND s.stage_id=w.stage_id AND s.id=? AND e.id=?
          AND e.owner_kind='stage_execution' AND e.owner_id=s.id AND e.provider=?
          AND json_valid(w.payload_json) LIMIT 2")
        .bind(run.id.to_string()).bind(stage_execution_id).bind(agent_execution_id).bind(provider)
        .fetch_all(pool).await?;
    let [row] = rows.as_slice() else {
        anyhow::bail!("artifact_provenance_invalid: provider dispatch missing or ambiguous");
    };
    let raw: String = row.try_get("payload_json")?;
    ensure!(
        raw.len() <= 4 * 1024 * 1024,
        "continuation_budget_exceeded: provider dispatch"
    );
    let payload: Value = serde_json::from_str(&raw)?;
    ensure!(
        payload["run_id"] == run.id.to_string()
            && payload["stage_execution_id"] == stage_execution_id
            && payload["stage_id"] == row.try_get::<String, _>("stage_id")?
            && payload["task_name"] == task.task_name
            && payload["task_outputs"] == json!(task.outputs)
            && payload["agent_id"] == task.agent.agent_id
            && payload["provider"] == provider,
        "artifact_provenance_invalid: provider dispatch identity"
    );
    let fallback = &payload["provider_health_fallback"];
    let target = fallback["to_backend_profile_id"]
        .as_str()
        .context("artifact_provenance_invalid: fallback backend")?;
    ensure!(
        fallback["from_provider"] == task.agent.provider
            && fallback["from_backend_profile_id"] == json!(task.agent.backend_profile_id)
            && fallback["to_provider"] == provider
            && payload["backend_profile_id"] == target
            && run_local_health_fallback_profile_candidates(
                &task.agent.agent_id,
                &task.outputs,
                task.agent.output_contract.as_deref(),
                &task.agent.provider
            )
            .contains(&target),
        "artifact_provenance_invalid: fallback attestation"
    );
    ensure!(
        run.catalog_snapshot_hash.as_deref() == Some(plan.catalog_snapshot_hash.as_str())
            && ContentDigest::of(plan.catalog_snapshot_json.as_bytes())
                .as_str()
                .strip_prefix("sha256:")
                == Some(plan.catalog_snapshot_hash.as_str()),
        "artifact_provenance_invalid: fallback frozen catalog"
    );
    let catalog: Value = serde_json::from_str(&plan.catalog_snapshot_json)?;
    let profile = &catalog["backend_profiles"][target];
    ensure!(
        profile["provider"] == provider
            && payload["model"] == profile["model"]
            && payload["effort"] == profile["effort"]
            && payload["max_turns"] == profile["max_turns"]
            && json!(row.try_get::<Option<String>, _>("model")?) == payload["model"],
        "artifact_provenance_invalid: fallback frozen backend"
    );
    match fallback["reason"].as_str() {
        Some("prior_run_local_provider_output_failure") => {
            let id: domain::ids::AgentExecutionId = fallback["failed_agent_execution_id"]
                .as_str()
                .context("artifact_provenance_invalid: fallback failure identity")?
                .parse()?;
            let execution = db::repos::agent_executions::find_by_id(pool, id)
                .await?
                .context("artifact_provenance_invalid: fallback failure missing")?;
            let stage = db::repos::stages::find_by_id(
                pool,
                execution
                    .stage_execution_id
                    .context("artifact_provenance_invalid: fallback failure owner")?,
            )
            .await?
            .context("artifact_provenance_invalid: fallback failure stage")?;
            let facts = db::repos::agent_execution_runtime_facts::find_by_execution_id(pool, id)
                .await?
                .context("artifact_provenance_invalid: fallback failure facts")?;
            ensure!(
                stage.run_id == run.id
                    && execution.agent_id == task.agent.agent_id
                    && execution.status == domain::agent::AgentStatus::Failed
                    && same_provider_family_for_health_fallback(
                        &execution.provider,
                        &task.agent.provider
                    )
                    && provider_health_fallback_failure(&facts),
                "artifact_provenance_invalid: fallback failure provenance"
            );
        }
        Some("junie_code_writer_unavailable") => ensure!(
            task.agent.agent_id == "code_writer"
                && matches!(task.agent.provider.as_str(), "junie" | "junie_acp")
                && target == "claude_builder_high",
            "artifact_provenance_invalid: forced fallback provenance"
        ),
        _ => anyhow::bail!("artifact_provenance_invalid: fallback reason"),
    }
    Ok(())
}

async fn approved_output(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
) -> Result<Option<ResolvedInput>> {
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM artifacts WHERE run_id=? AND name='approved_proposal' AND contract_id='approved_proposal' AND agent_id='engine' AND provider='engine' AND is_pinned=1 AND agent_execution_id IS NULL ORDER BY created_at DESC LIMIT 2")
        .bind(run.id.to_string()).fetch_all(pool).await?;
    let Some(id) = ids.first() else {
        return Ok(None);
    };
    ensure!(
        ids.len() == 1,
        "artifact_provenance_invalid: ambiguous approval snapshot"
    );
    let proof = Box::pin(super::approval::consumed_entry_evidence(pool, run.id))
        .await?
        .context("stale_approval_binding: snapshot authority")?;
    let artifact = db::repos::artifacts::find_by_id(pool, id.parse()?)
        .await?
        .context("artifact_provenance_invalid: snapshot artifact missing")?;
    let root = metadata_root(run)?;
    let relative = canonical_relative(run, plan, "approved_proposal")?
        .context("artifact_provenance_invalid: approved proposal path")?;
    let expected = root.join(&relative);
    let stored_path = Path::new(&artifact.file_path);
    let stored_path = if stored_path.is_absolute() {
        stored_path.to_path_buf()
    } else {
        Path::new(&run.workspace_root).join(stored_path)
    };
    let op = incoming(pool, run)
        .await?
        .context("artifact_provenance_invalid: snapshot operation")?;
    let manifest = read_manifest(&op).await?;
    ensure!(
        artifact.stage_id == manifest.target.profile.preparation_state
            && expected == stored_path
            && artifact.checksum_sha256.as_deref()
                == proof.proposal_sha256.as_str().strip_prefix("sha256:"),
        "artifact_provenance_invalid: approval snapshot binding"
    );
    let size = u64::try_from(
        artifact
            .size_bytes
            .context("artifact_provenance_invalid: snapshot size")?,
    )?;
    let content = read_bound(root, relative, proof.proposal_sha256, Some(size)).await?;
    Ok(Some(ResolvedInput {
        name: "approved_proposal".into(),
        content,
        display_path: expected.to_string_lossy().into_owned(),
        origin: InputOrigin::ApprovalSnapshot,
        artifact_id: Some(artifact.id),
    }))
}

/// Publish only a new engine-owned snapshot from the freshly approved current
/// proposal. Existing unregistered files are never promoted or overwritten.
/// This is the explicit mutating counterpart to the read-only input resolvers.
pub async fn ensure_approved_snapshot(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
    stage: &domain::stage::StageExecution,
) -> Result<Option<domain::artifact::Artifact>> {
    let Some(op) = incoming(pool, run).await? else {
        return Ok(None);
    };
    if let Some(output) = active_output(pool, run, plan, "approved_proposal").await? {
        return db::repos::artifacts::find_by_id(
            pool,
            output
                .artifact_id
                .context("artifact_provenance_invalid: output identity")?,
        )
        .await;
    }
    let manifest = read_manifest(&op).await?;
    ensure!(
        stage.run_id == run.id
            && stage.stage_id == manifest.target.profile.preparation_state
            && run.current_state.as_deref() == Some(stage.stage_id.as_str()),
        "stale_approval_binding: snapshot outside preparation"
    );
    let current_stage: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM stage_executions s JOIN runs r ON r.id=s.run_id WHERE s.id=? AND s.run_id=? AND s.stage_id=? AND r.current_state=s.stage_id)")
        .bind(stage.id.to_string()).bind(run.id.to_string()).bind(&stage.stage_id).fetch_one(pool).await?;
    ensure!(current_stage, "stale_approval_binding: snapshot stage");
    let root_path = metadata_root(run)?;
    ensure!(
        root_path == manifest.workspace.metadata_root,
        "artifact_provenance_invalid: snapshot root"
    );
    let relative = canonical_relative(run, &manifest.target.plan, "approved_proposal")?
        .context("artifact_provenance_invalid: snapshot path")?;
    ensure!(
        canonical_relative(run, plan, "approved_proposal")?.as_deref() == Some(relative.as_str()),
        "artifact_provenance_invalid: snapshot target"
    );
    let root = SafeRoot::open(&root_path)?;
    ensure!(
        root.inspect(&relative)?.is_none(),
        "artifact_provenance_invalid: unregistered approved proposal"
    );
    let proof = Box::pin(super::approval::consumed_entry_evidence(pool, run.id))
        .await?
        .context("stale_approval_binding: snapshot authority")?;
    let proposal = resolve_execution_input(pool, run, &manifest.target.plan, "proposal_current")
        .await?
        .context("artifact_provenance_invalid: current proposal missing")?;
    ensure!(
        ContentDigest::of(proposal.content.as_bytes()) == proof.proposal_sha256,
        "stale_approval_binding: snapshot proposal"
    );
    let failures =
        crate::rollout_contract_preflight::approved_proposal_rollout_contract_lint_failures(
            proposal.content.as_bytes(),
            Path::new(&proposal.display_path),
            &run.workspace_root,
        )?;
    ensure!(
        failures.is_empty(),
        "artifact_provenance_invalid: snapshot rollout contract"
    );
    let (parent, leaf) = relative
        .rsplit_once('/')
        .context("unsafe_path: approved snapshot parent")?;
    let mut parent_path = root_path.clone();
    let mut directory = SafeRoot::open(&parent_path)?;
    for name in parent.split('/') {
        let next = if directory.inspect(name)?.is_none() {
            directory.create_directory(name)?
        } else {
            SafeRoot::open(&parent_path.join(name))?
        };
        parent_path.push(name);
        directory = next;
    }
    root.verify_path(&root_path)?;
    directory.publish(
        leaf,
        proposal.content.as_bytes(),
        Deadline::after(Duration::from_secs(120)),
    )?;
    root.verify_path(&root_path)?;
    let fresh = Box::pin(super::approval::consumed_entry_evidence(pool, run.id))
        .await?
        .context("stale_approval_binding: snapshot authority changed")?;
    ensure!(
        fresh == proof,
        "stale_approval_binding: snapshot evidence changed"
    );
    let artifact = domain::artifact::Artifact {
        id: domain::ids::ArtifactId::new(),
        run_id: run.id,
        stage_id: stage.stage_id.clone(),
        agent_id: "engine".into(),
        name: "approved_proposal".into(),
        contract_id: "approved_proposal".into(),
        format: domain::artifact::ArtifactFormat::Markdown,
        file_path: root_path.join(relative).to_string_lossy().into_owned(),
        checksum_sha256: proof
            .proposal_sha256
            .as_str()
            .strip_prefix("sha256:")
            .map(str::to_owned),
        size_bytes: Some(proposal.content.len() as i64),
        provider: "engine".into(),
        model: None,
        created_at: chrono::Utc::now(),
        is_pinned: true,
        report_kind: None,
        report_version: None,
        agent_execution_id: None,
    };
    db::repos::artifacts::insert(pool, &artifact).await?;
    Ok(Some(artifact))
}

/// Only a validated successor output or freshly bound engine approval snapshot
/// can satisfy filesystem-based predicates. Seeds and references cannot.
pub async fn output_for_predicate(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
    name: &str,
) -> Result<OutputResolution> {
    if incoming(pool, run).await?.is_none() {
        return Ok(OutputResolution::OrdinaryRun);
    }
    Ok(match active_output(pool, run, plan, name).await? {
        Some(input) => OutputResolution::Output(input),
        None => OutputResolution::Missing,
    })
}

/// Collect verified bytes before synchronous prompt construction. No DB writes,
/// output publication, historical schema reinterpretation, or approval reuse.
pub async fn resolve_task_inputs(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
    task: &CompiledTask,
) -> Result<Option<TaskInputs>> {
    tokio::time::timeout(
        Duration::from_secs(120),
        collect_inputs(pool, run, plan, &task.inputs, true),
    )
    .await
    .context("continuation_budget_exceeded: input deadline")?
}

/// Read a continuation's installed logical input: validated successor output
/// first, then an installed execution seed. References never satisfy this lookup.
/// Ordinary runs and missing inputs return None; this is not predicate authority.
pub async fn resolve_execution_input(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
    logical_name: &str,
) -> Result<Option<ResolvedInput>> {
    let names = [logical_name.to_owned()];
    let Some(mut context) = tokio::time::timeout(
        Duration::from_secs(120),
        collect_inputs(pool, run, plan, &names, false),
    )
    .await
    .context("continuation_budget_exceeded: input deadline")??
    else {
        return Ok(None);
    };
    Ok(context.inputs.remove(logical_name).flatten())
}

async fn collect_inputs(
    pool: &SqlitePool,
    run: &Run,
    plan: &RunPlan,
    names: &[String],
    include_references: bool,
) -> Result<Option<TaskInputs>> {
    let Some(op) = incoming(pool, run).await? else {
        return Ok(None);
    };
    let manifest = read_manifest(&op).await?;
    ensure!(
        manifest.successor.id == run.id
            && manifest.successor.idea_id == run.idea_id
            && manifest.successor.chainworks_meta_root == run.chainworks_meta_root
            && manifest.successor.workspace_root == run.workspace_root
            && manifest.successor.worktree_root == run.worktree_root
            && manifest.target.plan.workflow_snapshot_hash == plan.workflow_snapshot_hash
            && manifest.target.plan.catalog_snapshot_hash == plan.catalog_snapshot_hash,
        "artifact_provenance_invalid: successor binding"
    );
    let rows: Vec<InstalledInput> = sqlx::query_as("SELECT i.* FROM run_continuation_inputs i WHERE operation_id=? AND successor_run_id=? AND installed=1 ORDER BY ordinal LIMIT 130")
        .bind(&op.operation_id).bind(run.id.to_string()).fetch_all(pool).await?;
    ensure!(
        !rows.is_empty() && rows.len() <= 129 && rows.len() == manifest.inputs.len(),
        "artifact_provenance_invalid: installed input set"
    );
    let root = metadata_root(run)?;
    ensure!(
        root == manifest.workspace.metadata_root,
        "artifact_provenance_invalid: metadata binding"
    );
    let mut context = TaskInputs {
        inputs: BTreeMap::new(),
        references: vec![],
    };
    let mut total = 0usize;
    for name in names {
        if plan.artifact_paths.contains_key(name) {
            let input = active_output(pool, run, plan, name).await?;
            total += input.as_ref().map_or(0, |input| input.content.len());
            ensure!(
                total as u64 <= MAX_CONTEXT_BYTES,
                "input_context_sources_too_large"
            );
            context.inputs.insert(name.clone(), input);
        }
    }
    for ((row, descriptor), provenance) in rows
        .iter()
        .zip(&manifest.inputs)
        .zip(&manifest.input_provenance)
    {
        ensure!(
            row.input_id == descriptor.input_id.to_string()
                && row.ordinal == descriptor.ordinal as i64
                && row.source_kind == descriptor.source_kind
                && row.source_id == descriptor.source_id.to_string()
                && row.source_run_id == descriptor.source_run_id.to_string()
                && row.source_run_id == op.source_run_id
                && row.original_artifact_id == descriptor.original_artifact_id.to_string()
                && row.source_schema == descriptor.source_schema
                && ContentDigest::from_sha256_hex(&row.content_sha256)?
                    == descriptor.content_sha256
                && row.target_logical_name == descriptor.target_logical_name
                && row.target_relative_path == descriptor.target_relative_path
                && serde_json::from_str::<serde_json::Value>(&row.historical_relation_json)?
                    == descriptor.historical_relation
                && provenance.input_id == descriptor.input_id,
            "artifact_provenance_invalid: installed descriptor changed"
        );
        let origin = match descriptor.role {
            EntryRole::ExecutionSeed => {
                ensure!(
                    row.role == "execution_seed"
                        && row.target_logical_name == "proposal_current"
                        && canonical_relative(run, plan, "proposal_current")?.as_deref()
                            == Some(row.target_relative_path.as_str()),
                    "artifact_provenance_invalid: seed path"
                );
                InputOrigin::ExecutionSeed
            }
            EntryRole::ReferenceOnly => {
                ensure!(
                    row.role == "reference_only"
                        && row.target_relative_path
                            == format!("carry-forward/references/{}/content", row.input_id),
                    "artifact_provenance_invalid: reference namespace"
                );
                InputOrigin::ReferenceOnly
            }
            _ => anyhow::bail!("artifact_provenance_invalid: installed role"),
        };
        if names.contains(&row.target_logical_name)
            && origin == InputOrigin::ExecutionSeed
            && context
                .inputs
                .get(&row.target_logical_name)
                .is_none_or(Option::is_none)
        {
            let input = ResolvedInput {
                name: row.target_logical_name.clone(),
                content: read_bound(
                    root.clone(),
                    row.target_relative_path.clone(),
                    descriptor.content_sha256.clone(),
                    Some(provenance.bytes),
                )
                .await?,
                display_path: root
                    .join(&row.target_relative_path)
                    .to_string_lossy()
                    .into_owned(),
                origin,
                artifact_id: None,
            };
            total += input.content.len();
            context
                .inputs
                .insert(row.target_logical_name.clone(), Some(input));
        }
        if origin == InputOrigin::ReferenceOnly && include_references {
            let content = read_bound(
                root.clone(),
                row.target_relative_path.clone(),
                descriptor.content_sha256.clone(),
                Some(provenance.bytes),
            )
            .await?;
            total += content.len();
            context.references.push(ResolvedInput {
                name: format!(
                    "carry_forward_reference/{}/{}",
                    row.input_id, row.target_logical_name
                ),
                content,
                display_path: root
                    .join(&row.target_relative_path)
                    .to_string_lossy()
                    .into_owned(),
                origin,
                artifact_id: None,
            });
        }
        ensure!(
            total as u64 <= MAX_CONTEXT_BYTES,
            "input_context_sources_too_large"
        );
    }
    let again = incoming(pool, run)
        .await?
        .context("artifact_provenance_invalid: input operation disappeared")?;
    ensure!(
        again.version == op.version && again.manifest_sha256 == op.manifest_sha256,
        "artifact_provenance_invalid: input operation changed"
    );
    Ok(Some(context))
}
