use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct SourceWitness {
    pub schema_version: &'static str,
    pub source_run_id: RunId,
    pub workflow_snapshot_hash: ContentDigest,
    pub catalog_snapshot_hash: ContentDigest,
    pub current_state: String,
    pub semantic_sha256: ContentDigest,
    pub workspace_sha256: ContentDigest,
    pub selected_inputs_sha256: ContentDigest,
    pub directory_identities: BTreeMap<String, (u64, u64)>,
    pub wire: domain::run_carry_forward_api::SourceWitnessV1,
}

/// Fresh canonical DB evidence, never projections. No write transaction is opened.
/// Reservation must compare this before inserting its own journal/fence/operation rows.
pub async fn source_semantic_witness(
    pool: &SqlitePool,
    run: RunId,
) -> anyhow::Result<ContentDigest> {
    Ok(observe(pool, run).await?.semantic_sha256)
}

pub(super) async fn observe(
    pool: &SqlitePool,
    run: RunId,
) -> anyhow::Result<db::repos::run_continuation_witness::Observation> {
    // BEGIN is deferred: this fixes the read snapshot without reserving a writer.
    let mut transaction = pool.begin().await?;
    let observation =
        db::repos::run_continuation_witness::observe_details(&mut transaction, run, None).await?;
    transaction.rollback().await?;
    Ok(observation)
}

pub(super) async fn check_admission(pool: &SqlitePool, source: &Run) -> anyhow::Result<()> {
    let id = source.id.to_string();
    let mut transaction = pool.begin().await?;
    db::repos::run_continuations::ensure_quiescent_tx(&mut transaction, &id).await?;
    transaction.rollback().await?;
    let outgoing: Option<String> = sqlx::query_scalar(
        "SELECT phase FROM run_continuations WHERE source_run_id=? AND phase<>'aborted'",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await?;
    ensure!(outgoing.as_deref() != Some("activated"), "source_continued");
    ensure!(outgoing.is_none(), "continuation_in_progress");
    let competitor: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs r WHERE r.idea_id=?1 AND r.id<>?2 AND r.status NOT IN ('completed','cancelled','failed') AND NOT EXISTS(SELECT 1 FROM run_execution_fences f WHERE f.source_run_id=r.id AND f.disposition='historical')) OR EXISTS(SELECT 1 FROM run_continuations WHERE idea_id=?1 AND phase IN ('preparing','prepared','aborting','needs_reconciliation'))")
        .bind(source.idea_id.to_string()).bind(&id).fetch_one(pool).await?;
    ensure!(!competitor, "continuation_in_progress");
    ensure!(
        source.cancellation_requested_at.is_none(),
        "source_busy: cancellation"
    );
    let busy: bool = sqlx::query_scalar(r#"SELECT
        EXISTS(SELECT 1 FROM work_items WHERE (run_id=?1 OR (json_valid(payload_json) AND (json_extract(payload_json,'$.run_id')=?1 OR json_extract(payload_json,'$.mediation_record_id') IN (SELECT id FROM lead_conflict_mediations WHERE run_id=?1)))) AND status IN ('pending','running')) OR
        EXISTS(SELECT 1 FROM agent_executions WHERE (owner_id=?1 OR stage_execution_id IN (SELECT id FROM stage_executions WHERE run_id=?1) OR owner_id IN (SELECT id FROM lead_conflict_mediations WHERE run_id=?1) OR lead_mediation_record_id IN (SELECT id FROM lead_conflict_mediations WHERE run_id=?1)) AND status NOT IN ('completed','failed','cancelled')) OR
        EXISTS(SELECT 1 FROM lead_conflict_mediations WHERE run_id=?1 AND status NOT IN ('settled','terminal_unverifiable','canceled','superseded')) OR
        EXISTS(SELECT 1 FROM approvals WHERE run_id=?1 AND decision NOT IN ('granted','rejected','expired')) OR
        EXISTS(SELECT 1 FROM agent_work_continuations WHERE run_id=?1 AND status NOT IN ('succeeded','no_progress','failed','cancelled')) OR
        EXISTS(SELECT 1 FROM output_contract_repair_leases WHERE run_id=?1 AND lease_state<>'settled') OR
        EXISTS(SELECT 1 FROM main_sync_attempts WHERE run_id=?1 AND status NOT IN ('succeeded','no_op','conflicted','failed')) OR
        EXISTS(SELECT 1 FROM worktree_mutation_barriers WHERE run_id=?1 AND status IN ('pending','active')) OR
        EXISTS(SELECT 1 FROM provider_sessions WHERE run_id=?1 AND process_fate<>'absent_verified' AND lifecycle_state<>'spawn_error_no_child') OR
        EXISTS(SELECT 1 FROM provider_cancellation_intents WHERE provider_session_id IN (SELECT provider_session_id FROM provider_sessions WHERE run_id=?1) AND intent_state<>'settled') OR
        EXISTS(SELECT 1 FROM shutdown_signal_side_effects WHERE provider_session_id IN (SELECT provider_session_id FROM provider_sessions WHERE run_id=?1) AND intent_state NOT IN ('observed','suppressed')) OR
        EXISTS(SELECT 1 FROM shutdown_interrupted_receipts WHERE provider_session_id IN (SELECT provider_session_id FROM provider_sessions WHERE run_id=?1) AND recovered_at IS NULL) OR
        EXISTS(SELECT 1 FROM side_effects WHERE run_id=?1 AND status NOT IN ('settled','reconciled'))
        "#).bind(&id).fetch_one(pool).await?;
    ensure!(!busy, "source_busy");
    let headless: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM xcode_effect_attempts WHERE (owner_lineage=?1 OR json_extract(project_key,'$.canonical_path')=?2 OR substr(json_extract(project_key,'$.canonical_path'),1,length(?2)+1)=?2||'/') AND state IN ('prepared','dispatched','unknown')) OR EXISTS(SELECT 1 FROM xcode_project_holds WHERE canonical_path=?2 OR substr(canonical_path,1,length(?2)+1)=?2||'/')")
        .bind(&id).bind(source.worktree_root.as_deref().unwrap_or(&source.workspace_root)).fetch_one(pool).await?;
    ensure!(!headless, "headless_effect_unresolved");
    Ok(())
}
