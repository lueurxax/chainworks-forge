use super::*;
use crate::contracts::DeclaredOutput;
use domain::artifact::Artifact;
use sqlx::Row;

/// Compatibility is bound to the persisted invocation, not a plausible output alias.
pub(super) async fn declared_output(
    pool: &SqlitePool,
    source: &Run,
    plan: &RunPlan,
    artifact: &Artifact,
    preserved_generation: bool,
) -> anyhow::Result<Value> {
    let execution = artifact
        .agent_execution_id
        .as_deref()
        .context("artifact_provenance_invalid: missing producer execution")?;
    let row = sqlx::query("SELECT e.stage_execution_id,e.status,f.failure_kind,f.valid_required_outputs,f.output_settlement,s.iteration,s.attempt_number FROM agent_executions e
        JOIN stage_executions s ON s.id=e.stage_execution_id
        JOIN agent_execution_runtime_facts f ON f.agent_execution_id=e.id
        WHERE e.id=? AND s.run_id=? AND s.stage_id=? AND e.agent_id=? AND e.provider=?
          AND e.owner_kind='stage_execution' AND e.owner_id=s.id
          AND e.status IN ('completed','failed') AND e.completed_at IS NOT NULL
          AND s.status IN ('completed','failed','blocked')
          AND f.output_settlement IN ('valid_outputs_from_completed_execution','valid_outputs_from_failed_execution','valid_outputs_from_repair')
          AND NOT EXISTS(SELECT 1 FROM validation_failure_records v WHERE v.artifact_id=?)")
        .bind(execution).bind(source.id.to_string()).bind(&artifact.stage_id)
        .bind(&artifact.agent_id).bind(&artifact.provider).bind(artifact.id.to_string())
        .fetch_optional(pool).await?.context("artifact_provenance_invalid: unsettled or foreign producer")?;
    let stage: String = row.try_get("stage_execution_id")?;
    let payloads: Vec<(String,String)> = sqlx::query_as("SELECT id,payload_json FROM work_items WHERE kind='invoke_agent' AND run_id=? AND stage_id=? AND status IN ('completed','failed') AND json_valid(payload_json) AND json_extract(payload_json,'$.stage_execution_id')=? AND json_extract(payload_json,'$.p058_claimed.agent_execution_id')=? LIMIT 2")
        .bind(source.id.to_string()).bind(&artifact.stage_id).bind(&stage).bind(execution).fetch_all(pool).await?;
    let [(work, raw)] = payloads.as_slice() else {
        anyhow::bail!("artifact_provenance_invalid: missing or ambiguous producer invocation");
    };
    ensure!(
        raw.len() <= 4 * 1024 * 1024,
        "continuation_budget_exceeded: producer invocation"
    );
    let payload: Value = serde_json::from_str(raw)?;
    ensure!(
        payload["run_id"] == source.id.to_string()
            && payload["stage_id"] == artifact.stage_id
            && payload["agent_id"] == artifact.agent_id
            && payload["provider"] == artifact.provider,
        "artifact_provenance_invalid: producer invocation identity"
    );
    let frozen = plan
        .states
        .get(&artifact.stage_id)
        .context("artifact_provenance_invalid: frozen producer stage")?;
    let tasks: Vec<_> = frozen
        .tasks
        .iter()
        .chain(&frozen.post_approval_tasks)
        .filter(|task| {
            task.task_name == payload["task_name"]
                && task.agent.agent_id == artifact.agent_id
                && task.outputs.contains(&artifact.name)
        })
        .collect();
    let dynamic = tasks.is_empty();
    let task = match tasks.as_slice() {
        [task] => (*task).clone(),
        [] => dynamic_task(plan, artifact, &payload)?,
        _ => anyhow::bail!("artifact_provenance_invalid: ambiguous frozen producer task"),
    };
    crate::run_carry_forward::inputs::verify_effective_provider(
        pool,
        source,
        plan,
        &task,
        &stage,
        execution,
        &artifact.provider,
    )
    .await?;
    let outputs: Vec<DeclaredOutput> = serde_json::from_value(payload["declared_outputs"].clone())
        .context("artifact_provenance_invalid: declared outputs")?;
    let outputs: Vec<_> = outputs
        .iter()
        .filter(|output| output.output_name == artifact.name)
        .collect();
    let [output] = outputs.as_slice() else {
        anyhow::bail!("artifact_provenance_invalid: missing or ambiguous declared output");
    };
    ensure!(
        output.target_path == artifact.file_path
            && serde_json::to_value(&output.schema)?
                == serde_json::to_value(task.output_schemas.get(&artifact.name))?,
        "artifact_provenance_invalid: declared path or frozen schema"
    );
    let expected_contract = output
        .schema
        .as_ref()
        .map(|schema| schema.contract_id.clone())
        .unwrap_or_else(|| format!("{}.output", artifact.provider));
    let legacy_backlog = output.schema.is_none()
        && artifact.name == "implementation_backlog"
        && ["implementation_backlog", "implementation_backlog_v1"]
            .contains(&artifact.contract_id.as_str());
    ensure!(
        artifact.contract_id == expected_contract || legacy_backlog,
        "artifact_provenance_invalid: producer contract identity"
    );
    if output.schema.is_none() {
        ensure!(
            row.try_get::<String, _>("status")? == "completed"
                && row.try_get::<Option<String>, _>("failure_kind")?.is_none()
                && row.try_get::<String, _>("output_settlement")?
                    == "valid_outputs_from_completed_execution",
            "artifact_provenance_invalid: plain output settlement"
        );
    } else if !row.try_get::<bool, _>("valid_required_outputs")? {
        // Successful mixed tasks contain NoContractDeclared results. Their completed
        // settlement still proves that no required result failed or was missing.
        ensure!(
            row.try_get::<String, _>("status")? == "completed"
                && row.try_get::<Option<String>, _>("failure_kind")?.is_none()
                && row.try_get::<String, _>("output_settlement")?
                    == "valid_outputs_from_completed_execution",
            "artifact_provenance_invalid: mixed output settlement"
        );
        if domain::artifact_contracts::known_contract_id(&expected_contract) {
            let valid: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM artifact_contract_generations g WHERE g.run_id=? AND g.artifact_id=? AND g.contract_id=? AND g.source_agent_execution_id=? AND g.source_stage_execution_id=? AND g.valid=1 AND g.source_generation_verified=1 AND g.output_settlement='valid_outputs_from_completed_execution' AND (? OR EXISTS(SELECT 1 FROM active_artifact_contracts a WHERE a.generation_id=g.generation_id AND a.run_id=g.run_id AND a.contract_id=g.contract_id)))")
                .bind(source.id.to_string()).bind(artifact.id.to_string()).bind(&expected_contract).bind(execution).bind(&stage).bind(preserved_generation).fetch_one(pool).await?;
            ensure!(
                valid,
                "artifact_provenance_invalid: mixed output canonical generation"
            );
        }
    }
    let mut proof = json!({"agent_execution_id":execution,"stage_execution_id":stage,"work_item_id":work,
        "task_name":task.task_name,"iteration":row.try_get::<i64,_>("iteration")?,
        "attempt_number":row.try_get::<i64,_>("attempt_number")?,"source_catalog_sha256":source.catalog_snapshot_hash,
        "declared_output_schema":output.schema});
    if dynamic {
        proof["dynamic_binding_id"] = payload["p060_binding_id"].clone();
        if !plan.artifact_paths.contains_key(&artifact.name) {
            let path = crate::orchestrator::p060_dynamic_review_target_path(
                &artifact.name,
                output.schema.as_ref(),
                plan,
                source,
                &task,
            )
            .context("artifact_provenance_invalid: dynamic canonical path unavailable")?;
            ensure!(
                path == artifact.file_path,
                "artifact_provenance_invalid: dynamic canonical path"
            );
            proof["dynamic_canonical_path"] = json!(path);
        }
    }
    Ok(proof)
}

fn dynamic_task(
    plan: &RunPlan,
    artifact: &Artifact,
    payload: &Value,
) -> anyhow::Result<workflow::plan::CompiledTask> {
    use crate::orchestrator::{p060_dynamic_review_output_name, p060_dynamic_review_output_schema};
    let dynamic = plan
        .states
        .get(&artifact.stage_id)
        .and_then(|state| state.dynamic_parallel.as_ref())
        .context("artifact_provenance_invalid: frozen dynamic producer stage")?;
    let bindings: Vec<_> = plan
        .dynamic_candidate_bindings
        .iter()
        .filter(|binding| {
            binding.binding_id == payload["p060_binding_id"]
                && binding.agent_id == artifact.agent_id
        })
        .collect();
    let [binding] = bindings.as_slice() else {
        anyhow::bail!("artifact_provenance_invalid: missing or ambiguous frozen dynamic binding");
    };
    ensure!(
        binding.enabled_for_proposal_review
            && binding.catalog_snapshot_hash == plan.catalog_snapshot_hash
            && dynamic.output_contract == "proposal_review_v1"
            && binding.output_contracts.contains(&dynamic.output_contract),
        "artifact_provenance_invalid: frozen dynamic contract"
    );
    let agent: workflow::plan::ResolvedAgent =
        serde_json::from_str(&binding.resolved_agent_snapshot_json)?;
    let name = p060_dynamic_review_output_name(&agent.agent_id);
    let task_name = format!("dynamic_review_{}", agent.agent_id);
    ensure!(
        agent.agent_id == artifact.agent_id
            && agent.output_contract.as_deref() == Some(dynamic.output_contract.as_str())
            && name == artifact.name
            && payload["task_name"] == task_name
            && payload["task_inputs"] == json!(dynamic.inputs)
            && payload["task_outputs"] == json!([name])
            && payload["p060_dynamic_phase"] == 0,
        "artifact_provenance_invalid: frozen dynamic invocation"
    );
    Ok(workflow::plan::CompiledTask {
        agent,
        task_name,
        inputs: dynamic.inputs.clone(),
        outputs: vec![name.clone()],
        output_schemas: [(
            name,
            p060_dynamic_review_output_schema(&dynamic.output_contract),
        )]
        .into(),
        output_policies: Default::default(),
        parallel: true,
        phase: 0,
        selected_outputs_from: None,
    })
}
