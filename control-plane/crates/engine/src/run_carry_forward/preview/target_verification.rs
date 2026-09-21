use super::super::manifest::ManifestV1;
use super::*;

/// Recheck current definitions/policy without source admission: a reserved source
/// is expected. The caller must separately authenticate the manifest and operation,
/// recheck source/material witnesses, and perform activation under its writer lock.
pub async fn verify_current_target(pool: &SqlitePool, manifest: &ManifestV1) -> anyhow::Result<()> {
    let deadline = Deadline::after(Duration::from_secs(120));
    tokio::time::timeout(Duration::from_secs(120), verify(pool, manifest, deadline))
        .await
        .context("continuation_budget_exceeded: target verification deadline")?
}

async fn verify(
    pool: &SqlitePool,
    manifest: &ManifestV1,
    deadline: Deadline,
) -> anyhow::Result<()> {
    ensure!(
        manifest.schema_version == "run_carry_forward_manifest_v1"
            && manifest.plan["source_run_id"] == json!(manifest.source_run_id),
        "prepared_material_changed: target source binding"
    );
    let expected: TargetSummary = serde_json::from_value(manifest.plan["target"].clone())
        .context("prepared_material_changed: target summary")?;
    ensure!(
        expected.workflow_snapshot_hash == manifest.original_workflow_sha256
            && expected.catalog_snapshot_hash == manifest.original_catalog_sha256
            && expected.definition_files_sha256 == manifest.definition_files_sha256
            && expected.compiled_plan_sha256 == manifest.target_sha256
            && canonical_digest(&manifest.target)? == manifest.target_sha256,
        "prepared_material_changed: target digest binding"
    );
    let source = db::repos::runs::find_by_id(pool, manifest.source_run_id)
        .await?
        .context("source_changed: missing source")?;
    ensure!(
        source.idea_id == manifest.idea_id,
        "source_changed: target idea binding"
    );
    let source_policy = policy_identity(&source)?;
    let request = PreviewTarget {
        workflow_yaml_path: expected.workflow_yaml_path.clone(),
        agent_catalog_yaml_path: expected.agent_catalog_yaml_path.clone(),
        expected_workflow_snapshot_hash: Some(manifest.original_workflow_sha256.clone()),
        expected_catalog_snapshot_hash: Some(manifest.original_catalog_sha256.clone()),
        delivery_configuration_json: serde_json::to_string(&expected.delivery_configuration)?,
    };
    let expected_roots: BTreeMap<String, (u64, u64)> =
        serde_json::from_value(manifest.plan["source_witness"]["directory_identities"].clone())
            .context("prepared_material_changed: source root witness")?;
    let current = tokio::task::spawn_blocking(move || -> anyhow::Result<TargetSummary> {
        deadline.check()?;
        let roots = Roots::derive(&source)?;
        ensure!(
            roots.identities == expected_roots,
            "source_changed: canonical target roots"
        );
        let (_, current) = compile_target(&request, &source, &roots, deadline)?;
        roots.verify()?;
        deadline.check()?;
        Ok(current)
    })
    .await
    .context("target_profile_invalid: target compiler task")??;
    ensure!(
        canonical_digest(&current)? == canonical_digest(&expected)?,
        "source_changed: current definition or derived target"
    );
    let fresh_source = db::repos::runs::find_by_id(pool, manifest.source_run_id)
        .await?
        .context("source_changed: missing source")?;
    ensure!(
        policy_identity(&fresh_source)? == source_policy,
        "source_changed: canonical delivery policy"
    );
    deadline.check()?;
    Ok(())
}

fn policy_identity(source: &Run) -> anyhow::Result<ContentDigest> {
    Ok(canonical_digest(
        &json!({"run":source.id,"idea":source.idea_id,"repository":source.workspace_root,
        "checkout":source.worktree_root,"metadata":source.chainworks_meta_root,"artifact_root":source.artifact_root,
        "delivery":source.delivery_configuration_json}),
    )?)
}
