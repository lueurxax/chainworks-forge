//! P039 transaction-local semantic witness. Reads only canonical authority tables.

use anyhow::{ensure, Context, Result};
use domain::{
    ids::RunId,
    run_carry_forward::{canonical_digest, ContentDigest},
};
use serde_json::{json, Value};
use sqlx::{Column, Row, SqliteConnection, TypeInfo, ValueRef};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

/// Presentation evidence extracted from exactly the rows used for the digest.
/// Callers requiring a snapshot must supply a transaction-bound connection.
#[derive(Debug)]
pub struct Observation {
    pub semantic_sha256: ContentDigest,
    pub idea_sha256: ContentDigest,
    pub stage_executions: Vec<Value>,
    pub transition_cursor: Option<Value>,
    pub journal_ids: Vec<String>,
    pub approval_ids: Vec<String>,
    pub artifact_generation_ids: Vec<String>,
}

/// Observe on the caller's connection, so reservation/activation can compare under
/// their existing writer transaction. No transaction, PRAGMA or write is issued.
/// An exclusion is valid only for an operation owned by this exact source.
pub async fn observe(
    connection: &mut SqliteConnection,
    source: RunId,
    exclude_operation: Option<&str>,
) -> Result<ContentDigest> {
    Ok(observe_details(connection, source, exclude_operation)
        .await?
        .semantic_sha256)
}

pub async fn observe_details(
    connection: &mut SqliteConnection,
    source: RunId,
    exclude_operation: Option<&str>,
) -> Result<Observation> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let source = source.to_string();
    let (idea, repository, checkout): (String, String, Option<String>) =
        sqlx::query_as("SELECT idea_id,workspace_root,worktree_root FROM runs WHERE id=?")
            .bind(&source)
            .fetch_optional(&mut *connection)
            .await?
            .context("source_not_blocked")?;
    let root = checkout.as_deref().unwrap_or(&repository);
    if let Some(operation) = exclude_operation {
        let owner: Option<String> =
            sqlx::query_scalar("SELECT source_run_id FROM run_continuations WHERE operation_id=?")
                .bind(operation)
                .fetch_optional(&mut *connection)
                .await?;
        ensure!(
            owner.as_deref() == Some(source.as_str()),
            "source_changed: excluded operation does not own source"
        );
    }
    let mut evidence = BTreeMap::new();
    let mut total_bytes = 0usize;
    let mut total_rows = 0usize;
    // All identifiers and predicates are constants, never caller-supplied SQL.
    // ?4 excludes only exact owned-operation bookkeeping, never real source history.
    for (table, filter) in [
        ("runs", "id IN (SELECT run_id FROM owners) OR idea_id=?2"), ("ideas", "id=?2"),
        ("stage_executions", "run_id IN (SELECT run_id FROM owners)"), ("artifacts", "run_id IN (SELECT id FROM runs WHERE idea_id=?2)"),
        ("artifact_contract_generations", "run_id=?1"), ("active_artifact_contracts", "run_id=?1"),
        ("artifact_contract_overrides", "run_id=?1"), ("artifact_source_generation_claims", "run_id=?1"),
        ("approvals", "run_id=?1"),
        ("command_journal", "run_id=?1 AND (?4 IS NULL OR (id NOT IN (SELECT journal_id FROM run_continuations WHERE operation_id=?4 AND source_run_id=?1) AND id NOT IN (SELECT json_extract(result_json,'$.journal_id') FROM run_continuation_commands WHERE operation_id=?4 AND source_run_id=?1 AND json_type(result_json,'$.journal_id')='text')))"),
        ("work_items", "run_id IN (SELECT run_id FROM owners) OR worktree_resource_key IN (SELECT resource_key FROM resources) OR (json_valid(payload_json) AND (json_extract(payload_json,'$.run_id') IN (SELECT run_id FROM owners) OR json_extract(payload_json,'$.mediation_record_id') IN (SELECT id FROM lead_conflict_mediations WHERE run_id IN (SELECT run_id FROM owners)) OR json_extract(payload_json,'$.agent_execution_id') IN (SELECT id FROM source_agents) OR json_extract(payload_json,'$.stage_execution_id') IN (SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners))))"),
        ("agent_executions", "id IN (SELECT id FROM source_agents)"),
        ("agent_execution_runtime_facts", "agent_execution_id IN (SELECT id FROM source_agents)"),
        ("runtime_invocations", "agent_execution_id IN (SELECT id FROM source_agents)"),
        ("lead_conflict_mediations", "run_id IN (SELECT run_id FROM owners)"), ("lead_mediation_confirmations", "run_id IN (SELECT run_id FROM owners)"),
        ("workflow_conflicts", "run_id=?1"), ("workflow_transition_cursors", "run_id=?1"),
        ("session_lineages", "run_id IN (SELECT run_id FROM owners)"), ("session_generations", "lineage_id IN (SELECT id FROM session_lineages WHERE run_id IN (SELECT run_id FROM owners))"),
        ("provider_sessions", "provider_session_id IN (SELECT id FROM providers)"),
        ("provider_cancellation_intents", "provider_session_id IN (SELECT id FROM providers)"),
        ("shutdown_signal_side_effects", "provider_session_id IN (SELECT id FROM providers)"),
        ("shutdown_interrupted_receipts", "provider_session_id IN (SELECT id FROM providers)"),
        ("agent_work_continuations", "id IN (SELECT id FROM continuations)"),
        ("agent_external_side_effect_ledger", "continuation_id IN (SELECT id FROM continuations)"),
        ("supervised_workers_continuation", "continuation_id IN (SELECT id FROM continuations)"),
        ("output_contract_repair_leases", "run_id IN (SELECT run_id FROM owners)"),
        ("output_contract_repair_events", "run_id IN (SELECT run_id FROM owners)"), ("code_writer_output_settlement_rows", "run_id IN (SELECT run_id FROM owners)"),
        ("main_sync_attempts", "run_id IN (SELECT run_id FROM owners)"),
        ("worktree_mutation_barriers", "run_id IN (SELECT run_id FROM owners) OR worktree_resource_key IN (SELECT resource_key FROM resources)"),
        ("background_leases", "run_id IN (SELECT run_id FROM owners) OR resource_key IN (SELECT resource_key FROM resources) OR worktree_resource_key IN (SELECT resource_key FROM resources)"),
        ("p080_helper_leases_v1", "run_id IN (SELECT run_id FROM owners)"),
        ("p080_helper_lease_members_v1", "lease_id IN (SELECT id FROM p080_helper_leases_v1 WHERE run_id IN (SELECT run_id FROM owners))"),
        ("side_effects", "run_id IN (SELECT run_id FROM owners)"),
        ("side_effect_attempts", "side_effect_id IN (SELECT id FROM side_effects WHERE run_id IN (SELECT run_id FROM owners))"),
        ("xcode_effect_attempts", "owner_lineage=?1 OR json_extract(project_key,'$.canonical_path')=?3 OR substr(json_extract(project_key,'$.canonical_path'),1,length(?3)+1)=?3||'/'"),
        ("xcode_project_holds", "canonical_path=?3 OR substr(canonical_path,1,length(?3)+1)=?3||'/'"),
        ("run_continuations", "idea_id=?2 AND (?4 IS NULL OR operation_id<>?4)"),
        ("run_execution_fences", "source_run_id IN (SELECT id FROM runs WHERE idea_id=?2) AND (?4 IS NULL OR operation_id<>?4)"),
        ("run_continuation_inputs", "(successor_run_id IN (SELECT id FROM runs WHERE idea_id=?2) OR source_run_id IN (SELECT id FROM runs WHERE idea_id=?2)) AND (?4 IS NULL OR operation_id<>?4)"),
        ("run_continuation_commands", "source_run_id=?1 AND (?4 IS NULL OR operation_id IS NULL OR operation_id<>?4)"),
        ("run_continuation_approval_bindings", "successor_run_id=?1"),
        ("escalation_ledger", "run_id=?1"),
    ] {
        ensure!(Instant::now() < deadline, "continuation_budget_exceeded: DB witness deadline");
        let mut query = sqlx::QueryBuilder::<sqlx::Sqlite>::new(r#"WITH
            resources(resource_key) AS (
                SELECT ?3 UNION SELECT worktree_resource_key FROM work_items WHERE run_id=?1
                UNION SELECT worktree_resource_key FROM background_leases WHERE run_id=?1
                UNION SELECT worktree_resource_key FROM worktree_mutation_barriers WHERE run_id=?1),
            owners(run_id) AS (SELECT ?1 UNION SELECT id FROM runs
                WHERE COALESCE(NULLIF(worktree_root,''),workspace_root) IN (SELECT resource_key FROM resources)),
            source_agents(id) AS (SELECT a.id FROM agent_executions a WHERE
                a.stage_execution_id IN (SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners)) OR
                a.origin_stage_execution_id IN (SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners)) OR
                a.owner_id IN (SELECT run_id FROM owners) OR
                (a.owner_kind='stage_execution' AND a.owner_id IN (SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners))) OR
                a.owner_id IN (SELECT id FROM lead_conflict_mediations WHERE run_id IN (SELECT run_id FROM owners)) OR
                a.lead_mediation_record_id IN (SELECT id FROM lead_conflict_mediations WHERE run_id IN (SELECT run_id FROM owners)) OR
                a.session_lineage_id IN (SELECT id FROM session_lineages WHERE run_id IN (SELECT run_id FROM owners)) OR
                a.session_generation_id IN (SELECT id FROM session_generations WHERE lineage_id IN (SELECT id FROM session_lineages WHERE run_id IN (SELECT run_id FROM owners)))),
            providers(id) AS (SELECT provider_session_id FROM provider_sessions
                WHERE run_id IN (SELECT run_id FROM owners) OR agent_execution_id IN (SELECT id FROM source_agents)),
            continuations(id) AS (SELECT id FROM agent_work_continuations WHERE run_id IN (SELECT run_id FROM owners)
                OR stage_execution_id IN (SELECT id FROM stage_executions WHERE run_id IN (SELECT run_id FROM owners))
                OR agent_execution_id IN (SELECT id FROM source_agents))
            SELECT * FROM "#);
        query.push(table).push(" WHERE (").push(filter).push(") AND ?1 IS NOT NULL AND ?2 IS NOT NULL AND ?3 IS NOT NULL AND (?4 IS NULL OR ?4 IS NOT NULL) LIMIT 50001");
        let mut rows = query.build().bind(&source).bind(&idea).bind(root).bind(exclude_operation).fetch(&mut *connection);
        let mut values = Vec::new();
        while let Some(row) = tokio::time::timeout(deadline.saturating_duration_since(Instant::now()),
            std::future::poll_fn(|context| rows.as_mut().poll_next(context))).await.context("continuation_budget_exceeded: DB query deadline")? {
            ensure!(Instant::now() < deadline, "continuation_budget_exceeded: DB witness deadline");
            total_rows += 1;
            ensure!(total_rows <= 50_000, "continuation_budget_exceeded: DB witness rows");
            let row = row?;
            let mut value = BTreeMap::new();
            for column in row.columns() {
                let name = column.name();
                let raw = row.try_get_raw(name)?;
                // Wall-clock observations do not confer authority. Presence still records
                // cancellation, settlement and expiry transitions.
                if name.ends_with("_at") || name.ends_with("_at_wall_clock") || name.ends_with("_at_monotonic_ms") {
                    value.insert(format!("{name}_present"), json!(!raw.is_null()));
                    continue;
                }
                // P079 evidence export freshness is projection bookkeeping, not execution truth.
                if table == "output_contract_repair_events" && ["projection_integrity", "projection_stale_since", "projection_schema_version", "projection_rebuild_attempts"].contains(&name) { continue; }
                let field = if raw.is_null() { Value::Null } else {
                    match raw.type_info().name() {
                        "INTEGER" | "BOOLEAN" => json!(row.try_get::<i64, _>(name)?),
                        "REAL" => json!(row.try_get::<f64, _>(name)?),
                        "BLOB" => {
                            let bytes: &[u8] = row.try_get(name)?;
                            total_bytes = total_bytes.checked_add(bytes.len()).context("continuation_budget_exceeded")?;
                            ensure!(total_bytes <= 64 * 1024 * 1024, "continuation_budget_exceeded: DB witness bytes");
                            json!(ContentDigest::of(bytes))
                        }
                        "TEXT" => {
                            let text: &str = row.try_get(name)?;
                            total_bytes = total_bytes.checked_add(text.len()).context("continuation_budget_exceeded")?;
                            ensure!(total_bytes <= 64 * 1024 * 1024, "continuation_budget_exceeded: DB witness bytes");
                            if name.ends_with("snapshot_json") { json!(ContentDigest::of(text.as_bytes())) } else { json!(text) }
                        }
                        _ => anyhow::bail!("storage_unavailable: unknown DB witness type"),
                    }
                };
                value.insert(name.into(), field);
            }
            values.push(serde_json::to_value(value)?);
        }
        values.sort_by_cached_key(|v| serde_json::to_string(v).expect("JSON Value serialization"));
        evidence.insert(table, values);
    }
    let ids = |table: &str, column: &str| -> Result<Vec<String>> {
        let mut ids = evidence
            .get(table)
            .context("storage_unavailable: missing witness table")?
            .iter()
            .map(|row| {
                row[column]
                    .as_str()
                    .map(str::to_owned)
                    .context("storage_unavailable: witness ID")
            })
            .collect::<Result<Vec<_>>>()?;
        ids.sort();
        Ok(ids)
    };
    ensure!(
        evidence["ideas"].len() == 1,
        "source_changed: missing owning idea"
    );
    Ok(Observation {
        semantic_sha256: canonical_digest(&evidence)?,
        idea_sha256: canonical_digest(&evidence["ideas"][0])?,
        stage_executions: evidence["stage_executions"]
            .iter()
            .filter(|row| row["run_id"] == source)
            .cloned()
            .collect(),
        transition_cursor: evidence["workflow_transition_cursors"].first().cloned(),
        journal_ids: ids("command_journal", "id")?,
        approval_ids: ids("approvals", "id")?,
        artifact_generation_ids: ids("artifact_contract_generations", "generation_id")?,
    })
}
