use super::*;
use domain::artifact::Artifact;

pub(super) struct GenerationRead {
    pub relative: String,
    pub proof: Value,
}

#[derive(sqlx::FromRow)]
struct Generation {
    generation_id: String,
    artifact_id: String,
    canonical_path: String,
    raw_path: String,
    source_agent_execution_id: Option<String>,
    source_stage_execution_id: Option<String>,
    source_work_item_id: Option<String>,
    supersedes_generation_id: Option<String>,
    active: bool,
}

/// Separate current canonical bytes from preservation. A newer successful review
/// never supplies evidence about the contents or disposition of an older review.
pub(super) async fn resolve(
    pool: &SqlitePool,
    source: &Run,
    plan: &RunPlan,
    roots: &Roots,
    artifacts: &[Artifact],
    selected: &BTreeMap<InputReference, bool>,
) -> anyhow::Result<BTreeMap<Uuid, GenerationRead>> {
    let mut groups: BTreeMap<(&str, &str), Vec<&Artifact>> = BTreeMap::new();
    for artifact in artifacts {
        groups
            .entry((&artifact.name, &artifact.file_path))
            .or_default()
            .push(artifact);
    }
    let mut result = BTreeMap::new();
    for ((name, canonical), group) in groups {
        if group.len() < 2
            || !group
                .iter()
                .any(|a| selected.contains_key(&InputReference::Artifact { id: a.id.0 }))
        {
            continue;
        }
        let contract = &group[0].contract_id;
        ensure!(
            group.iter().all(|a| &a.contract_id == contract),
            "artifact_provenance_invalid: mixed canonical generations"
        );
        let generations: Vec<Generation> = sqlx::query_as("SELECT g.generation_id,g.artifact_id,g.canonical_path,g.raw_path,g.source_agent_execution_id,g.source_stage_execution_id,g.source_work_item_id,g.supersedes_generation_id,EXISTS(SELECT 1 FROM active_artifact_contracts a WHERE a.run_id=g.run_id AND a.contract_id=g.contract_id AND a.generation_id=g.generation_id) AS active FROM artifact_contract_generations g WHERE g.run_id=? AND g.contract_id=? AND g.valid=1 AND g.source_generation_verified=1 AND g.output_settlement IN ('valid_outputs_from_completed_execution','valid_outputs_from_failed_execution','valid_outputs_from_repair') LIMIT 1001")
            .bind(source.id.to_string()).bind(contract).fetch_all(pool).await?;
        ensure!(
            generations.len() <= 1000,
            "continuation_budget_exceeded: historical generations"
        );
        let current: Vec<_> = generations.iter().filter(|g| g.active).collect();
        let [current] = current.as_slice() else {
            anyhow::bail!("artifact_provenance_invalid: current canonical generation unproven");
        };
        let current_artifact = group
            .iter()
            .find(|a| a.id.to_string() == current.artifact_id)
            .context("artifact_provenance_invalid: active generation outside canonical group")?;
        ensure!(
            current.raw_path == canonical,
            "artifact_provenance_invalid: active canonical path"
        );
        let mut chain = BTreeSet::new();
        let mut cursor = Some(current.generation_id.as_str());
        while let Some(id) = cursor {
            ensure!(
                chain.insert(id),
                "artifact_provenance_invalid: generation cycle"
            );
            let generation = generations
                .iter()
                .find(|g| g.generation_id == id)
                .context("artifact_provenance_invalid: historical generation gap")?;
            cursor = generation.supersedes_generation_id.as_deref();
        }
        for artifact in &group {
            let matches: Vec<_> = generations
                .iter()
                .filter(|g| g.artifact_id == artifact.id.to_string())
                .collect();
            let [generation] = matches.as_slice() else {
                anyhow::bail!(
                    "artifact_provenance_invalid: historical generation missing or ambiguous"
                );
            };
            let mut proof = super::provenance::declared_output(
                pool,
                source,
                plan,
                artifact,
                !generation.active,
            )
            .await?;
            ensure!(
                chain.contains(generation.generation_id.as_str())
                    && generation.canonical_path == name
                    && generation.source_agent_execution_id.as_deref()
                        == artifact.agent_execution_id.as_deref()
                    && generation.source_stage_execution_id.as_deref()
                        == proof["stage_execution_id"].as_str()
                    && generation.source_work_item_id.as_deref() == proof["work_item_id"].as_str(),
                "artifact_provenance_invalid: historical generation provenance"
            );
            if !generation.active {
                ensure!(
                    artifact.checksum_sha256.is_some(),
                    "artifact_provenance_invalid: historical preservation checksum missing"
                );
                ensure!(
                    generation.raw_path != canonical
                        || artifact.checksum_sha256 == current_artifact.checksum_sha256,
                    "artifact_provenance_invalid: historical findings preservation unproven"
                );
            }
            let relative = Path::new(&generation.raw_path)
                .strip_prefix(&roots.metadata)
                .context(
                    "artifact_provenance_invalid: historical generation outside metadata root",
                )?
                .to_str()
                .context("unsafe_path: historical generation path")?
                .to_string();
            validate_relative(&relative)?;
            proof["generation_id"] = json!(generation.generation_id);
            proof["current_canonical_artifact_id"] = json!(current.artifact_id);
            proof["generation_selection"] = json!(if generation.active {
                "verified_active_canonical_generation"
            } else {
                "preserved_historical_generation"
            });
            result.insert(artifact.id.0, GenerationRead { relative, proof });
        }
    }
    Ok(result)
}
