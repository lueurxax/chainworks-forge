use acp::AcpRuntimeManager;
use anyhow::{Context, Result};
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{code_writer_completion_receipts, ideas, runs, stages};
use db::work_item::WorkItemKind;
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::contracts::DeclaredOutput;
use engine::event_bus;
use engine::executor::BackgroundExecutor;
use engine::orchestrator::Orchestrator;
use engine::work_queue::WorkQueue;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use workflow::plan::{DegradedOutputPolicy, OutputSchema};

#[derive(Clone)]
struct CatalogBinding {
    catalog_path: PathBuf,
    catalog_sha256: String,
    agent_id: String,
    backend_profile: String,
    provider: String,
    model: String,
    effort: String,
    runtime_profile: String,
    outputs: Vec<String>,
    contract_ids: BTreeMap<String, String>,
    contract_schemas: BTreeMap<String, OutputSchema>,
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))?;
    Ok(())
}

fn yaml_key<'a>(value: &'a serde_yaml::Value, key: &str) -> Option<&'a serde_yaml::Value> {
    value
        .as_mapping()?
        .get(serde_yaml::Value::String(key.to_string()))
}

fn yaml_string(value: &serde_yaml::Value, key: &str) -> Result<String> {
    yaml_key(value, key)
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing string field {key}"))
}

fn yaml_optional_string(value: &serde_yaml::Value, key: &str) -> Option<String> {
    yaml_key(value, key)
        .and_then(serde_yaml::Value::as_str)
        .map(str::to_string)
}

fn yaml_string_vec(value: &serde_yaml::Value, key: &str) -> Vec<String> {
    yaml_key(value, key)
        .and_then(serde_yaml::Value::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_yaml::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn load_catalog_binding(repo_root: &Path) -> Result<CatalogBinding> {
    let catalog_path = repo_root.join("examples/agents/agents.yaml");
    let catalog_text = fs::read_to_string(&catalog_path)?;
    let catalog: serde_yaml::Value = serde_yaml::from_str(&catalog_text)?;
    let agents = yaml_key(&catalog, "agents")
        .and_then(serde_yaml::Value::as_sequence)
        .context("missing agents list")?;
    let agent = agents
        .iter()
        .find(|agent| {
            yaml_key(agent, "id").and_then(serde_yaml::Value::as_str) == Some("code_writer")
        })
        .context("missing code_writer agent")?;
    let backend_profile = yaml_string(agent, "backend_profile")?;
    let profile = yaml_key(&catalog, "backend_profiles")
        .and_then(serde_yaml::Value::as_mapping)
        .and_then(|profiles| profiles.get(serde_yaml::Value::String(backend_profile.clone())))
        .with_context(|| format!("missing backend profile {backend_profile}"))?;
    let outputs = yaml_key(agent, "outputs")
        .and_then(serde_yaml::Value::as_sequence)
        .context("missing code_writer outputs")?
        .iter()
        .map(|output| {
            output
                .as_str()
                .map(str::to_string)
                .context("code_writer output must be string")
        })
        .collect::<Result<Vec<_>>>()?;
    let output_contract = yaml_string(agent, "output_contract")?;
    let output_schemas = yaml_key(&catalog, "contracts")
        .and_then(serde_yaml::Value::as_mapping)
        .context("missing contracts")?;
    let mut contract_ids = BTreeMap::new();
    let mut contract_schemas = BTreeMap::new();
    for output in &outputs {
        let contract_id = if output == "implementation_self_assessment" {
            output_contract.clone()
        } else {
            output.clone()
        };
        let contract = output_schemas
            .get(serde_yaml::Value::String(contract_id.clone()))
            .with_context(|| format!("missing output schema contract {contract_id}"))?;
        contract_schemas.insert(
            output.clone(),
            OutputSchema {
                contract_id: contract_id.clone(),
                format: yaml_string(contract, "format")?,
                human_format: yaml_optional_string(contract, "human_format"),
                machine_format: yaml_optional_string(contract, "machine_format"),
                validation_mode: yaml_optional_string(contract, "validation_mode"),
                normalized_artifact_name: yaml_optional_string(
                    contract,
                    "normalized_artifact_name",
                ),
                raw_artifact_name: yaml_optional_string(contract, "raw_artifact_name"),
                required_fields: yaml_string_vec(contract, "required_fields"),
            },
        );
        contract_ids.insert(output.clone(), contract_id);
    }
    Ok(CatalogBinding {
        catalog_sha256: sha256_file(&catalog_path)?,
        catalog_path,
        agent_id: yaml_string(agent, "id")?,
        backend_profile,
        provider: yaml_string(profile, "provider")?,
        model: yaml_string(profile, "model")?,
        effort: yaml_string(profile, "effort")?,
        runtime_profile: yaml_string(profile, "runtime_profile")?,
        outputs,
        contract_ids,
        contract_schemas,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let workspace_root =
        PathBuf::from(env::var("P090_WORKTREE_ROOT").context("P090_WORKTREE_ROOT")?);
    let evidence_dir = PathBuf::from(env::var("P090_EVIDENCE_DIR").context("P090_EVIDENCE_DIR")?);
    let repo_root = env::current_dir()?
        .parent()
        .context("control-plane directory must have repo parent")?
        .to_path_buf();
    let binding = load_catalog_binding(&repo_root)?;
    fs::create_dir_all(&workspace_root)?;
    fs::write(
        workspace_root.join("AGENTS.md"),
        "P090 live canary workspace. Keep edits minimal and report via CHAINWORKS_OUTPUT.\n",
    )?;
    fs::create_dir_all(workspace_root.join("src"))?;
    fs::write(
        workspace_root.join("src/lib.rs"),
        "pub fn p090_canary_subject() -> &'static str { \"before\" }\n",
    )?;

    let run_id = RunId::new();
    let idea_id = IdeaId::new();
    let stage_execution_id = StageExecutionId::new();
    let artifact_root = evidence_dir.join("artifacts");
    let meta_root = evidence_dir.join("meta");
    fs::create_dir_all(&artifact_root)?;
    fs::create_dir_all(&meta_root)?;

    let outputs: BTreeMap<String, PathBuf> = binding
        .outputs
        .iter()
        .map(|name| (name.clone(), artifact_root.join(format!("{name}.json"))))
        .collect();
    let declared_outputs = binding
        .outputs
        .iter()
        .map(|name| DeclaredOutput {
            output_name: name.clone(),
            target_path: outputs[name].to_string_lossy().into_owned(),
            schema: binding.contract_schemas.get(name).cloned(),
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        })
        .collect::<Vec<_>>();

    let pool = create_pool("sqlite::memory:").await?;
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await?;
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P090 refine-like live canary".into(),
            body: "Prove Junie code_writer runtime hardening through BackgroundExecutor.".into(),
            workspace_root_path: Some(workspace_root.to_string_lossy().into_owned()),
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await?;
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "p090-live-canary".into(),
            workflow_title: "P090 Live Canary".into(),
            workspace_root: workspace_root.to_string_lossy().into_owned(),
            artifact_root: artifact_root.to_string_lossy().into_owned(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("p090_refine_like_canary".into()),
            workflow_yaml_path: None,
            agent_catalog_yaml_path: Some(binding.catalog_path.to_string_lossy().into_owned()),
            worktree_root: Some(workspace_root.to_string_lossy().into_owned()),
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: None,
            delivery_preflight_json: None,
            workflow_family: Some("p090_live_canary".into()),
            project_key: None,
            risk_class: None,
            stack: Some("rust-backend".into()),
            workflow_snapshot_hash: None,
            catalog_snapshot_hash: Some(binding.catalog_sha256.clone()),
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: Some(meta_root.to_string_lossy().into_owned()),
            review_routing_json: None,
            closeout_readiness_mode: None,
        },
    )
    .await?;
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "p090_refine_like_canary".into(),
            label: "P090 refine-like canary".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
            owner_agent: Some(binding.agent_id.clone()),
            provider: Some(binding.provider.clone()),
            model: Some(binding.model.clone()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await?;

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(AcpRuntimeManager::new()),
        events,
    );
    let prompt = format!(
        r#"You are running the P090 Junie refine-like canary through the production Chainworks executor path.

Inspect the small Rust file if useful, but keep the task bounded. Return only a final CHAINWORKS_OUTPUT object.
Do not include markdown, code fences, headings, or prose outside JSON.
Do not include changed_files_manifest; the control plane owns that output.

Required output payloads:
- implementation_progress: status/current_phase/completed_items/deferred_items/notes
- implementation_self_assessment: implementation_complete/verification_green/remaining_code_tasks/handoff_tasks/known_risks/tests_run/docs_impacted
- tests_result: status/summary

Use these canonical paths if you write files directly:
implementation_progress={}
implementation_self_assessment={}
tests_result={}
"#,
        outputs["implementation_progress"].display(),
        outputs["implementation_self_assessment"].display(),
        outputs["tests_result"].display(),
    );
    work_queue
        .enqueue(
            WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("p090_refine_like_canary".into()),
            json!({
                "run_id": run_id.to_string(),
                "stage_id": "p090_refine_like_canary",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": binding.agent_id,
                "provider": binding.provider,
                "model": binding.model,
                "effort": binding.effort,
                "task_name": "p090_refine_like_canary",
                "backend_profile_id": binding.backend_profile,
                "runtime_profile": binding.runtime_profile,
                "prompt": prompt,
                "declared_outputs": declared_outputs,
                "stage_degraded_output_policy": DegradedOutputPolicy::default(),
                "session_reuse_scope": "stage_execution",
                "session_family_id": format!("p090-refine-like-{run_id}"),
                "worktree_write_enabled": true,
                "worktree_strategy": "dedicated",
                "legacy_broad_discovery_policy": "disabled"
            }),
        )
        .await?;

    let started_at = Utc::now();
    let processed = executor.process_next_item().await?;
    let completed_at = Utc::now();
    let receipts = code_writer_completion_receipts::list_by_run(&pool, run_id).await?;
    let receipt = receipts
        .last()
        .context("missing code_writer completion receipt")?;
    let output_files = outputs
        .iter()
        .map(|(name, path)| {
            Ok(json!({
                "output_name": name,
                "path": path,
                "exists": path.exists(),
                "sha256": if path.exists() { Some(sha256_file(path)?) } else { None },
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let active_contracts = sqlx::query(
        "SELECT contract_id, generation_id FROM active_artifact_contracts WHERE run_id = ?1 ORDER BY contract_id",
    )
    .bind(run_id.to_string())
    .fetch_all(&pool)
    .await?
    .into_iter()
    .map(|row| {
        use sqlx::Row;
        json!({
            "contract_id": row.get::<String, _>("contract_id"),
            "generation_id": row.get::<String, _>("generation_id"),
        })
    })
    .collect::<Vec<_>>();

    let result = json!({
        "schema_version": "p090_refine_like_live_canary_v1",
        "status": if processed { "passed" } else { "not_processed" },
        "started_at": started_at,
        "completed_at": completed_at,
        "run_id": run_id,
        "stage_execution_id": stage_execution_id,
        "workspace_root": workspace_root,
        "artifact_root": artifact_root,
        "catalog_binding": {
            "catalog_path": binding.catalog_path,
            "catalog_sha256": binding.catalog_sha256,
            "contract_ids": binding.contract_ids,
        },
        "receipt": receipt,
        "output_files": output_files,
        "active_contracts": active_contracts,
        "coverage": {
            "live_canary_scope": "junie_hardened_happy_path",
            "staged_repair_exercised": receipt.receipt.staged_repair_settlement_enabled,
            "repair_turn_attempted": receipt.receipt.completion_turn_attempted,
            "staged_repair_proof": "focused_engine_and_startup_recovery_tests",
        },
        "hardened_flags": {
            "strict_final_payload": env::var("CHAINWORKS_P090_STRICT_FINAL_PAYLOAD").ok(),
            "preflight_enforce": env::var("CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE").ok(),
            "staged_repair_settlement": env::var("CHAINWORKS_P090_STAGED_REPAIR_SETTLEMENT").ok(),
        }
    });
    write_json(&evidence_dir.join("harness-result.json"), &result)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
