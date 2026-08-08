use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use sqlx::SqlitePool;
use tracing::{info, warn};

/// Bounded P075 startup evidence-orphan reconciliation summary.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StartupEvidenceOrphanSweepSummary {
    pub roots_inspected: u64,
    pub roots_missing: u64,
    pub scanned_files: u64,
    pub already_indexed: u64,
    pub recovered_orphans: u64,
    pub skipped_files: u64,
    pub bytes_read: u64,
    pub truncated: bool,
    pub errors: u64,
}

/// Run the P075 startup sweep for active run artifact roots.
///
/// The sweep is intentionally bounded and best-effort. A bad or missing artifact
/// root increments counters but does not abort daemon startup; manual MCP
/// reconciliation can still inspect individual roots later.
pub async fn run_startup_evidence_orphan_sweep(
    pool: &SqlitePool,
) -> Result<StartupEvidenceOrphanSweepSummary> {
    let runs = db::repos::runs::list_active(pool).await?;
    let mut roots = BTreeSet::new();
    for run in runs {
        if !run.artifact_root.trim().is_empty() {
            roots.insert((run.artifact_root, run.id.to_string()));
        }
    }

    let mut summary = StartupEvidenceOrphanSweepSummary::default();
    for (root, run_id) in roots {
        let path = Path::new(&root);
        if !path.exists() {
            summary.roots_missing += 1;
            continue;
        }
        summary.roots_inspected += 1;
        match db::evidence_spool::sweep_evidence_orphans(
            pool,
            path,
            Some(&run_id),
            db::evidence_spool::SWEEP_DEFAULT_MAX_FILES,
            db::evidence_spool::SWEEP_DEFAULT_MAX_BYTES,
            false,
        )
        .await
        {
            Ok(report) => {
                summary.scanned_files += report.scanned_files;
                summary.already_indexed += report.already_indexed;
                summary.recovered_orphans += report.recovered_orphans;
                summary.skipped_files += report.skipped_files;
                summary.bytes_read += report.bytes_read;
                summary.truncated |= report.truncated;
            }
            Err(_) => {
                summary.errors += 1;
            }
        }
    }

    Ok(summary)
}

/// Start the best-effort P075 reconciliation after the daemon is ready.
///
/// Filesystem enumeration is deliberately isolated from the readiness path:
/// an unavailable artifact root must not make the operator shell disappear.
pub fn spawn_startup_evidence_orphan_sweep(pool: SqlitePool) -> tokio::task::JoinHandle<()> {
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        // sweep_evidence_orphans performs bounded synchronous directory I/O.
        // Keep that work off Tokio's request workers so GraphQL remains
        // responsive while an artifact root is slow or unavailable.
        match runtime.block_on(run_startup_evidence_orphan_sweep(&pool)) {
            Ok(summary) => {
                info!(
                    roots_inspected = summary.roots_inspected,
                    roots_missing = summary.roots_missing,
                    scanned_files = summary.scanned_files,
                    already_indexed = summary.already_indexed,
                    recovered_orphans = summary.recovered_orphans,
                    skipped_files = summary.skipped_files,
                    bytes_read = summary.bytes_read,
                    truncated = summary.truncated,
                    errors = summary.errors,
                    "P075 background evidence orphan sweep complete"
                );
            }
            Err(err) => {
                warn!(
                    err = %err,
                    "P075 background evidence orphan sweep could not enumerate active runs"
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};

    use super::*;

    #[tokio::test]
    async fn background_startup_sweep_recovers_active_run_orphan_without_blocking_startup() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        db::writer::register_shared_writer(
            &pool,
            Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        let run_id = RunId::new();
        let run_id_str = run_id.to_string();

        let idea = Idea {
            id: IdeaId::new(),
            title: "P075 startup sweep".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        db::repos::runs::insert(
            &pool,
            &Run {
                id: run_id,
                idea_id: idea.id,
                status: RunStatus::Running,
                workflow_id: "wf-p075".into(),
                workflow_title: "P075".into(),
                workspace_root: "/tmp/ws".into(),
                artifact_root: dir.path().to_string_lossy().into_owned(),
                started_at: Utc::now(),
                completed_at: None,
                cancellation_requested_at: None,
                cancellation_settled_at: None,
                cancellation_settlement_log: None,
                current_state: None,
                workflow_yaml_path: None,
                agent_catalog_yaml_path: None,
                worktree_root: None,
                base_branch: None,
                base_revision: None,
                target_branch: None,
                delivery_configuration_json: None,
                delivery_preflight_json: None,
                workflow_family: None,
                project_key: None,
                risk_class: None,
                stack: None,
                workflow_snapshot_hash: None,
                catalog_snapshot_hash: None,
                workflow_snapshot_json: None,
                catalog_snapshot_json: None,
                drift_detected_at: None,
                drift_details_json: None,
                chainworks_meta_root: None,
                review_routing_json: None,
                closeout_readiness_mode: None,
            },
        )
        .await
        .unwrap();

        let relative_path =
            format!("evidence/runs/{run_id_str}/stages/s-1/agents/a-1/transcripts/transcript.md");
        db::evidence_spool::write_spool_file(
            dir.path(),
            &run_id_str,
            &relative_path,
            b"startup orphan",
        )
        .await
        .unwrap();

        let handle = spawn_startup_evidence_orphan_sweep(pool.clone());
        handle.await.unwrap();

        let row = db::repos::evidence_spool_refs::find_by_run_and_path(
            &pool,
            &run_id_str,
            &relative_path,
        )
        .await
        .unwrap();
        assert!(row.is_some());
    }
}
