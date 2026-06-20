use acp::adapters::{junie::JunieAdapter, AcpAdapter};
use acp::ExecutionRequest;
use anyhow::{Context, Result};
use chrono::Utc;
use domain::discovery::{LegacyBroadDiscoveryPolicy, OutputReusePolicy};
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
use engine::contracts::{build_expected_output_specs, DeclaredOutput};
use engine::executor::run_production_declared_output_settlement_for_canary;
use engine::worktree_fingerprint::{
    capture_worktree_fingerprint_v1, CapturePhase, WorktreeFingerprintInput,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use workflow::plan::OutputSchema;

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

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_hex(&fs::read(path)?))
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
        PathBuf::from(env::var("P089_WORKTREE_ROOT").context("P089_WORKTREE_ROOT")?);
    let evidence_dir =
        PathBuf::from(env::var("P089_ACP_EVIDENCE_DIR").context("P089_ACP_EVIDENCE_DIR")?);
    let repo_root = env::current_dir()?
        .parent()
        .context("control-plane directory must have repo parent")?
        .to_path_buf();
    let binding = load_catalog_binding(&repo_root)?;
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();
    let session_generation_id = format!("p089-acp-canary-{}", agent_execution_id);
    let run_dir = workspace_root
        .join(".chainworks/tmp/p089-acp-canary")
        .join(run_id.to_string());
    let artifact_root = run_dir.join("artifacts");
    fs::create_dir_all(&artifact_root)?;

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
            reuse_policy: Some(OutputReusePolicy::MustProduce),
            companion_output_name: None,
            companion_path: None,
        })
        .collect::<Vec<_>>();
    let run_dir_string = run_dir.to_string_lossy().into_owned();
    let expected_outputs = build_expected_output_specs(
        &declared_outputs,
        workspace_root.to_string_lossy().as_ref(),
        Some(workspace_root.to_string_lossy().as_ref()),
        Some(run_dir_string.as_str()),
        true,
    );

    let pre = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
        worktree_root: workspace_root.clone(),
        run_id: run_id.to_string(),
        stage_execution_id: stage_execution_id.to_string(),
        agent_execution_id: agent_execution_id.to_string(),
        session_generation_id: session_generation_id.clone(),
        capture_phase: CapturePhase::PreOriginalPrompt,
        active_proposal_id: Some("089".to_string()),
        baseline: None,
    })
    .await?;
    write_json(
        &evidence_dir.join("worktree-fingerprint-pre.json"),
        &serde_json::to_value(&pre)?,
    )?;

    let prompt = format!(
        r#"Return only this JSON object as the final answer. The first byte of your final answer must be `{{` and the last byte must be `}}`. Do not print a task title, heading, label, explanation, markdown, code fence, bullet, or prefix. Do not run tools. Do not modify files. Do not include changed_files_manifest; the control plane generates it.
{{"CHAINWORKS_OUTPUT":{{"implementation_progress":{{"status":"passed","current_phase":"p089_acp_canary","completed_items":["returned structured output through Junie ACP"],"deferred_items":[],"notes":"p089 acp canary completed"}},"implementation_self_assessment":{{"implementation_complete":true,"verification_green":true,"remaining_code_tasks":[],"handoff_tasks":[],"known_risks":[],"tests_run":[],"docs_impacted":[]}},"tests_result":{{"status":"not_run","summary":"p089 acp canary does not run project tests"}}}}}}

Canonical output paths, for context only:
implementation_progress={}
implementation_self_assessment={}
changed_files_manifest={}
tests_result={}
"#,
        outputs["implementation_progress"].display(),
        outputs["implementation_self_assessment"].display(),
        outputs["changed_files_manifest"].display(),
        outputs["tests_result"].display()
    );

    let req = ExecutionRequest {
        agent_execution_id: Some(agent_execution_id),
        run_id,
        stage_execution_id: Some(stage_execution_id.to_string()),
        stage_id: "p089_acp_canary".to_string(),
        attempt_number: 1,
        agent_id: binding.agent_id.clone(),
        provider: binding.provider.clone(),
        model: Some(binding.model.clone()),
        effort: Some(binding.effort.clone()),
        workspace_root: workspace_root.to_string_lossy().into_owned(),
        prompt,
        worktree_root: Some(workspace_root.to_string_lossy().into_owned()),
        worktree_write_enabled: true,
        worktree_strategy: Some("dedicated".to_string()),
        expected_output_paths: expected_outputs
            .iter()
            .map(|spec| spec.target_path.clone())
            .collect(),
        expected_outputs: expected_outputs.clone(),
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: Some(session_generation_id.clone()),
        provider_session_id: None,
        mcp_servers: Vec::new(),
        chainworks_meta_root: Some(run_dir.join("meta").to_string_lossy().into_owned()),
        legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy::Disabled,
        xcode_shim_injection_signal: false,
        requires_xcode_host_execution: false,
        owner_kind: "stage_execution".to_string(),
        owner_id: Some(stage_execution_id.to_string()),
        origin_stage_id: None,
        origin_stage_execution_id: None,
        mediation_record_id: None,
        toolchain_home: None,
        toolchain_go_scope_enabled: false,

        p079_repair_canonical_paths: None,
    };

    let started_at = Utc::now();
    let adapter = JunieAdapter::new();
    let result = adapter.execute(req).await?;
    let completed_at = Utc::now();
    let terminal_text = result
        .completion_text_capture
        .captured_text
        .clone()
        .unwrap_or_default();
    fs::create_dir_all(&evidence_dir)?;
    fs::write(
        evidence_dir.join("terminal-completion.raw.txt"),
        terminal_text,
    )?;

    let production_settlement = run_production_declared_output_settlement_for_canary(
        &declared_outputs,
        &expected_outputs,
        &result.discovered_artifacts,
        &result.pre_prompt_expected_outputs,
        Some(workspace_root.to_string_lossy().as_ref()),
        true,
    )
    .await?;

    let post = capture_worktree_fingerprint_v1(WorktreeFingerprintInput {
        worktree_root: workspace_root.clone(),
        run_id: run_id.to_string(),
        stage_execution_id: stage_execution_id.to_string(),
        agent_execution_id: agent_execution_id.to_string(),
        session_generation_id: session_generation_id.clone(),
        capture_phase: CapturePhase::PostOriginalPrompt,
        active_proposal_id: Some("089".to_string()),
        baseline: Some(&pre),
    })
    .await?;
    write_json(
        &evidence_dir.join("worktree-fingerprint-post.json"),
        &serde_json::to_value(&post)?,
    )?;

    let result_value = json!({
        "schema_version": "p089_acp_harness_result_v1",
        "started_at": started_at,
        "completed_at": completed_at,
        "run_id": run_id,
        "stage_id": "p089_acp_canary",
        "stage_execution_id": stage_execution_id,
        "agent_execution_id": agent_execution_id,
        "session_generation_id": session_generation_id,
        "provider_session_id": result.provider_session_id,
        "workspace_root": workspace_root,
        "run_dir": run_dir,
        "artifact_root": artifact_root,
        "catalog_binding": {
            "catalog_path": binding.catalog_path,
            "catalog_sha256": binding.catalog_sha256,
            "agent_id": binding.agent_id,
            "backend_profile": binding.backend_profile,
            "provider": binding.provider,
            "model": binding.model,
            "effort": binding.effort,
            "runtime_profile": binding.runtime_profile,
            "outputs": binding.outputs,
            "contract_ids": binding.contract_ids,
        },
        "declared_outputs": declared_outputs,
        "expected_outputs": expected_outputs,
        "production_settlement": production_settlement,
        "execution_result": result,
    });
    write_json(&evidence_dir.join("harness-result.json"), &result_value)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": result_value["execution_result"]["status"],
            "run_id": result_value["run_id"],
            "agent_execution_id": result_value["agent_execution_id"],
            "discovered_artifacts": result_value["execution_result"]["discovered_artifacts"].as_array().map(|items| items.len()).unwrap_or(0),
            "completion_capture": result_value["execution_result"]["completion_text_capture"],
        }))?
    );
    Ok(())
}
