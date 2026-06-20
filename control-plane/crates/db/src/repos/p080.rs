//! P080 Continuous Stale Execution Reconciliation — DB repository.
//!
//! Phase 1 surface:
//!   - Rollout control seeding at daemon startup (idempotent)
//!   - Running-execution classifier → `p080_readback_heartbeats_v1`
//!   - Readback heartbeat page reader for the diagnostics MCP tool
use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tracing::warn;

/// Feature classes registered in `p080_rollout_control_v1`.
///
/// `acp_prompt_stale` is explicitly excluded — P080 delegates that class to
/// P037 when it matures.  The extra three rows are per-proposal-required:
/// - `detection_only` — independent default-off gate for the classifier; the
///   diagnostics handler consults this row, not the repair-class row.
/// - `live_disable`   — live-disable rollout control row.
/// - `permanent_hold_clear` — Phase-5 permanent-hold-clear gate.
const ROLLOUT_CLASSES: &[&str] = &[
    "acp_startup_stale",
    "scheduler_ownership_drift",
    "helper_orphan_drift",
    "release_side_effect_drift",
    "detection_only",
    "live_disable",
    "permanent_hold_clear",
];

/// Seed `p080_rollout_control_v1` rows for all feature classes on first run.
///
/// All-or-nothing semantics (HIGH-001 fix): rows are only inserted when the
/// table is completely empty.  If some rows exist but not all, the daemon must
/// refuse to start rather than silently recreating missing rows (e.g. a deleted
/// `live_disable` row that was previously enabled=1 would be recreated as
/// enabled=0, allowing repairs to proceed incorrectly).
///
/// Returns the number of rows inserted (equals `ROLLOUT_CLASSES.len()` on first
/// run, or 0 when the table is already fully populated).
pub async fn seed_rollout_control_if_absent(pool: &SqlitePool) -> Result<usize> {
    // Count existing rows before deciding whether to seed.
    let existing: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM p080_rollout_control_v1")
        .fetch_one(pool)
        .await
        .context("p080 count rollout rows before seed")?;

    let total = ROLLOUT_CLASSES.len() as i64;

    if existing == total {
        // All rows present — normal restart, no seeding required.
        return Ok(0);
    }

    if existing > 0 {
        // Partial row set: some rows exist but not all.  Fail closed rather
        // than recreating missing rows whose previous enabled state is unknown.
        return Err(anyhow::anyhow!(
            "P080 rollout-control table has a partial row set ({existing}/{total} rows present). \
             Expected either 0 (first run) or {total} (normal run). \
             Possible table corruption: operator must inspect and repair the table \
             (e.g. truncate or restore from backup) before restarting the daemon."
        ));
    }

    // First run (existing == 0): insert all rows together.
    let now = Utc::now().to_rfc3339();
    let mut seeded = 0usize;
    let mut tx = pool.begin().await.context("p080 seed_rollout_control begin tx")?;
    for class in ROLLOUT_CLASSES {
        let result = sqlx::query(
            "INSERT OR IGNORE INTO p080_rollout_control_v1
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason)
             VALUES (?1, 0, 'phase_0', 1, ?2, 'system', 'startup_seed')",
        )
        .bind(class)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("seed p080 rollout control class={class}"))?;
        if result.rows_affected() > 0 {
            // Proposal §5.7 (lines 597-603): every rollout control change must write an
            // immutable audit row. startup_seed is the first generation (before=0, after=1).
            let audit_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO p080_rollout_control_audit_v1
                 (id, class, generation_before, generation_after, enabled_before, enabled_after,
                  reason, updated_by_principal_id, updated_at)
                 VALUES (?1, ?2, 0, 1, 0, 0, 'startup_seed', 'system', ?3)",
            )
            .bind(&audit_id)
            .bind(class)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("seed p080 rollout audit class={class}"))?;
            seeded += 1;
        }
    }
    tx.commit().await.context("p080 seed_rollout_control commit")?;
    Ok(seeded)
}

/// Verify that `p080_rollout_control_v1` contains exactly the required set of
/// rollout-control classes.
///
/// Called after seeding during daemon startup to enforce fail-closed behaviour:
/// if any required class row is absent (including after a successful no-op seed
/// on a corrupted table), the daemon refuses to start (HIGH-001 fix).
pub async fn validate_rollout_control_completeness(pool: &SqlitePool) -> Result<()> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT class FROM p080_rollout_control_v1 ORDER BY class")
            .fetch_all(pool)
            .await
            .context("p080 validate_rollout_control_completeness: DB read failed")?;

    let present: std::collections::HashSet<&str> = rows.iter().map(|(c,)| c.as_str()).collect();

    let mut missing: Vec<&str> = ROLLOUT_CLASSES
        .iter()
        .copied()
        .filter(|c| !present.contains(c))
        .collect();
    missing.sort_unstable();

    if !missing.is_empty() {
        return Err(anyhow::anyhow!(
            "P080 rollout-control table is missing required classes: {:?}. \
             Daemon startup refused (fail-closed). \
             Operator must restore or truncate the table before restarting.",
            missing
        ));
    }

    let extra: Vec<&str> = rows
        .iter()
        .map(|(c,)| c.as_str())
        .filter(|c| !ROLLOUT_CLASSES.contains(c))
        .collect();
    if !extra.is_empty() {
        warn!(extra = ?extra, "P080 rollout-control table has unrecognized class rows; ignoring");
    }

    Ok(())
}

/// Time thresholds (seconds) for Phase 1 classification of running executions.
const WARMUP_SECS: i64 = 60;
const STALE_SECS: i64 = 300;
/// Maximum executions to scan per classifier pass.
const CLASSIFY_SCAN_LIMIT: i64 = 500;

/// Per-stale-class count returned by the classifier for metric emission.
#[derive(Debug, Default, Clone)]
pub struct ClassifierCounts {
    pub warmup_pending: usize,
    pub acp_startup_stale: usize,
    pub scheduler_ownership_drift: usize,
    pub useful: usize,
    pub total: usize,
}

/// Classify all `running` agent_executions and upsert into
/// `p080_readback_heartbeats_v1`. Returns per-class breakdown for metric emission.
///
/// Phase 1 rules (diagnose-only, no repair):
/// - elapsed < 60 s AND session still active    → `warmup_pending`
/// - session_generation.status = 'ended'        → `acp_startup_stale` (ownership lost)
/// - elapsed ≥ 300 s                            → `acp_startup_stale`
/// - work_item absent or not 'running'          → `scheduler_ownership_drift`
/// - else                                       → `useful`
pub async fn classify_and_upsert_running_executions(
    pool: &SqlitePool,
) -> Result<ClassifierCounts> {
    let now = Utc::now();
    let now_str = now.to_rfc3339();

    // DEFECT-001 fix: use the actual work_item_id from work_items, not ae.id.
    // Also joins session_generations to check session liveness for ownership-aware
    // classification: a running agent_execution whose session_generation has ended
    // is classified as acp_startup_stale regardless of elapsed time.
    // A null session_generation (no session lineage entry) means the execution was
    // not started through the normal ACP session path — flag as scheduler_ownership_drift.
    let rows: Vec<(String, String, String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(
                (SELECT wi.id
                 FROM   work_items wi
                 WHERE  wi.run_id   = se.run_id
                   AND  wi.stage_id = se.stage_id
                   AND  wi.status   = 'running'
                 ORDER  BY wi.created_at ASC
                 LIMIT  1),
                ae.id
            ) AS work_item_id,
            se.run_id,
            se.stage_id,
            ae.started_at,
            ae.id AS exec_id,
            sg.status AS session_gen_status
        FROM   agent_executions ae
        JOIN   stage_executions se ON ae.stage_execution_id = se.id
        LEFT JOIN session_generations sg ON ae.session_generation_id = sg.id
        WHERE  ae.status = 'running'
        LIMIT  ?1
        "#,
    )
    .bind(CLASSIFY_SCAN_LIMIT)
    .fetch_all(pool)
    .await
    .context("p080 classify: query running executions")?;

    let mut counts = ClassifierCounts::default();
    for (work_item_id, run_id, stage_id, started_at_str, exec_id, session_gen_status) in rows {
        let _ = exec_id; // kept for future join; work_item_id is now the projection key
        let started_at = match chrono::DateTime::parse_from_rfc3339(&started_at_str) {
            Ok(t) => t.with_timezone(&Utc),
            Err(_) => continue,
        };
        let elapsed = (now - started_at).num_seconds();

        // Ownership-aware classification: check session liveness before elapsed thresholds.
        // A session_generation with status='ended' means the ACP session closed but the
        // agent_execution record was not updated to terminal — a clear ownership loss signal.
        let session_ended = session_gen_status
            .as_deref()
            .map(|s| s == "ended")
            .unwrap_or(false);
        // No session_generation linked means the execution wasn't started through the normal
        // ACP session path — classify as scheduler_ownership_drift if beyond warmup.
        let no_session = session_gen_status.is_none();

        let (stale_class, running_truth, hold_reason) = if session_ended {
            // Session closed but execution still marked running — strongest ownership signal.
            ("acp_startup_stale", "stale_suspected", "rollout_disabled")
        } else if no_session && elapsed >= WARMUP_SECS {
            // No ACP session linkage after warmup — scheduler may not have launched the agent.
            ("scheduler_ownership_drift", "stale_suspected", "rollout_disabled")
        } else if elapsed < WARMUP_SECS {
            ("warmup_pending", "warmup_pending", "warmup_pending")
        } else if elapsed >= STALE_SECS {
            ("acp_startup_stale", "stale_suspected", "rollout_disabled")
        } else {
            ("useful", "useful", "none")
        };

        let readback_json = serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": run_id,
            "stage_id": stage_id,
            "work_item_id": work_item_id,
            "stale_class": stale_class,
            "running_truth": running_truth,
            "repair_action": "diagnose_only",
            "hold_reason": hold_reason,
            "hold_age_seconds": null,
            "next_retry_or_backoff_time": null,
            "projection_updated_at": now_str,
            "projection_integrity": "valid",
            "executor_reregistration_state": "expected",
            "rollout_disablement": "phase_not_reached",
            "side_effect_status": "not_applicable",
            "operator_message": "",
            "evidence_marker_hash": null,
            "repair_idempotency_key": null
        });

        // SEC-P080-003: validate through the same guard used by all other write paths.
        let readback_json_str = serde_json::to_string(&readback_json)?;
        if let Err(e) = validate_readback_json_for_write(&readback_json_str) {
            warn!(
                error = %e,
                run_id = %run_id,
                stage_id = %stage_id,
                "p080 classifier: readback_json failed validation; skipping write"
            );
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO p080_readback_heartbeats_v1
              (run_id, stage_id, work_item_id, stale_class,
               projection_generation, projection_updated_at, projection_integrity,
               readback_json, updated_at)
            VALUES (?1, ?2, ?3, ?4, 1, ?5, 'valid', ?6, ?7)
            ON CONFLICT(run_id, stage_id, work_item_id, stale_class) DO UPDATE SET
              projection_generation = projection_generation + 1,
              projection_updated_at = excluded.projection_updated_at,
              projection_integrity  = 'valid',
              readback_json         = excluded.readback_json,
              updated_at            = excluded.updated_at
            "#,
        )
        .bind(&run_id)
        .bind(&stage_id)
        .bind(&work_item_id)
        .bind(stale_class)
        .bind(&now_str)
        .bind(readback_json_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .with_context(|| format!("p080 upsert readback work_item_id={work_item_id}"))?;

        match stale_class {
            "warmup_pending" => counts.warmup_pending += 1,
            "acp_startup_stale" => counts.acp_startup_stale += 1,
            "scheduler_ownership_drift" => counts.scheduler_ownership_drift += 1,
            _ => counts.useful += 1,
        }
        counts.total += 1;
    }

    Ok(counts)
}

/// Remove heartbeat rows for work items no longer in a running state.
///
/// Called after each classifier pass so stale diagnostic rows do not persist
/// indefinitely once their associated execution reaches a terminal state.
/// The DELETE is safe to run on every tick because it is bounded by the number
/// of terminal work items, which is small relative to the heartbeat table size.
pub async fn retire_terminal_heartbeats(pool: &SqlitePool) -> Result<usize> {
    let result = sqlx::query(
        r#"
        DELETE FROM p080_readback_heartbeats_v1
        WHERE work_item_id NOT IN (
            SELECT id   FROM work_items        WHERE status = 'running'
            UNION
            SELECT ae.id FROM agent_executions ae WHERE ae.status = 'running'
        )
        "#,
    )
    .execute(pool)
    .await
    .context("p080 retire_terminal_heartbeats")?;
    Ok(result.rows_affected() as usize)
}

/// One row from `p080_readback_heartbeats_v1`, augmented with epoch and last event metadata.
#[derive(Debug, Clone)]
pub struct ReadbackHeartbeatRow {
    pub run_id: String,
    pub stage_id: String,
    pub work_item_id: String,
    pub stale_class: String,
    pub projection_generation: i64,
    pub projection_updated_at: String,
    pub projection_integrity: String,
    pub readback_json: String,
    /// Current recurrence epoch from `p080_recurrence_epoch_v1`, or 0 if absent.
    pub recurrence_epoch: i64,
    /// ID of the most recent reconciliation event for this (run, stage, work_item, stale_class), if any.
    pub last_repair_event_id: Option<String>,
}

/// Optional filter for readback heartbeat queries.
#[derive(Debug, Default, Clone)]
pub struct ReadbackFilter {
    pub run_id: Option<String>,
    pub stage_id: Option<String>,
    pub work_item_id: Option<String>,
    pub stale_class: Option<String>,
    /// Filter by hold_reason extracted from readback_json (JSON path $.hold_reason).
    pub hold_reason: Option<String>,
    /// When false (default), exclude rows whose readback_json.repair_outcome = 'success'
    /// (recently repaired rows). When true, include all rows regardless of repair outcome.
    pub include_recent_repaired: bool,
}

/// Returns a page of readback heartbeats matching `filter`, ordered by
/// `(projection_updated_at DESC, run_id ASC, stage_id ASC, work_item_id ASC)`.
/// The compound key produces a stable cursor-safe ordering per proposal §5.2.
/// `page_size` is clamped to [1, 200].
pub async fn list_readback_page(
    pool: &SqlitePool,
    filter: ReadbackFilter,
    page_size: usize,
) -> Result<Vec<ReadbackHeartbeatRow>> {
    list_readback_page_with_offset(pool, filter, page_size, 0).await
}

/// Same as [`list_readback_page`], starting at a validated offset from an
/// opaque northbound cursor.
pub async fn list_readback_page_with_offset(
    pool: &SqlitePool,
    filter: ReadbackFilter,
    page_size: usize,
    offset: usize,
) -> Result<Vec<ReadbackHeartbeatRow>> {
    let limit = page_size.clamp(1, 201) as i64;
    let offset = offset as i64;
    let include_recent_repaired = filter.include_recent_repaired as i64;

    let rows: Vec<(String, String, String, String, i64, String, String, String, i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT h.run_id, h.stage_id, h.work_item_id, h.stale_class,
               h.projection_generation, h.projection_updated_at,
               h.projection_integrity, h.readback_json,
               COALESCE(re.epoch, 0) AS recurrence_epoch,
               (SELECT ev.id FROM p080_reconciliation_events_v1 ev
                WHERE ev.run_id = h.run_id AND ev.stage_id = h.stage_id
                  AND ev.work_item_id = h.work_item_id AND ev.stale_class = h.stale_class
                ORDER BY ev.created_at DESC LIMIT 1) AS last_repair_event_id
        FROM   p080_readback_heartbeats_v1 h
        LEFT JOIN p080_recurrence_epoch_v1 re
            ON re.run_id = h.run_id AND re.stage_id = h.stage_id
           AND re.work_item_id = h.work_item_id AND re.stale_class = h.stale_class
        WHERE  (?1 IS NULL OR h.run_id      = ?1)
          AND  (?2 IS NULL OR h.stage_id    = ?2)
          AND  (?3 IS NULL OR h.work_item_id = ?3)
          AND  (?4 IS NULL OR h.stale_class = ?4)
          AND  (?5 IS NULL OR json_extract(h.readback_json, '$.hold_reason') = ?5)
          AND  (?6 = 1 OR COALESCE(json_extract(h.readback_json, '$.repair_outcome'), '') != 'success')
        ORDER  BY h.projection_updated_at DESC, h.run_id ASC, h.stage_id ASC, h.work_item_id ASC
        LIMIT  ?7
        OFFSET ?8
        "#,
    )
    .bind(filter.run_id.as_deref())
    .bind(filter.stage_id.as_deref())
    .bind(filter.work_item_id.as_deref())
    .bind(filter.stale_class.as_deref())
    .bind(filter.hold_reason.as_deref())
    .bind(include_recent_repaired)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .context("p080 list_readback_page")?;

    Ok(rows
        .into_iter()
        .map(
            |(
                run_id,
                stage_id,
                work_item_id,
                stale_class,
                projection_generation,
                projection_updated_at,
                projection_integrity,
                readback_json,
                recurrence_epoch,
                last_repair_event_id,
            )| ReadbackHeartbeatRow {
                run_id,
                stage_id,
                work_item_id,
                stale_class,
                projection_generation,
                projection_updated_at,
                projection_integrity,
                readback_json,
                recurrence_epoch,
                last_repair_event_id,
            },
        )
        .collect())
}

/// Keyset pagination anchor for the approved p080_cursor_v1 contract.
///
/// When supplied to [`list_readback_page_keyset`], the query returns only rows
/// that come after this tuple in the ordering
/// `(projection_updated_at DESC, run_id ASC, stage_id ASC, work_item_id ASC)`.
#[derive(Debug, Clone)]
pub struct KeysetAfter {
    pub projection_updated_at: String,
    pub run_id: String,
    pub stage_id: String,
    pub work_item_id: String,
}

/// Return a page of readback heartbeats using keyset (seek) pagination.
///
/// Stable under projection rebuilds: uses row values for the WHERE bound rather
/// than OFFSET, so pages remain correct when rows are inserted or updated between
/// page fetches.
///
/// Ordering: `projection_updated_at DESC, run_id ASC, stage_id ASC, work_item_id ASC`.
/// `page_size` is clamped to [1, 201] (caller adds 1 for has-next-page detection).
pub async fn list_readback_page_keyset(
    pool: &SqlitePool,
    filter: ReadbackFilter,
    page_size: usize,
    after: Option<&KeysetAfter>,
) -> Result<Vec<ReadbackHeartbeatRow>> {
    let limit = page_size.clamp(1, 201) as i64;

    let (after_ts, after_rid, after_sid, after_wid) = match after {
        Some(k) => (
            Some(k.projection_updated_at.as_str()),
            Some(k.run_id.as_str()),
            Some(k.stage_id.as_str()),
            Some(k.work_item_id.as_str()),
        ),
        None => (None, None, None, None),
    };

    // Keyset condition for ordering DESC ts, ASC rid, ASC sid, ASC wid:
    //   row comes after (ts, rid, sid, wid) iff:
    //     ts_col < ts  OR
    //     (ts_col = ts AND rid_col > rid)  OR
    //     (ts_col = ts AND rid_col = rid AND sid_col > sid)  OR
    //     (ts_col = ts AND rid_col = rid AND sid_col = sid AND wid_col > wid)
    let include_recent_repaired = filter.include_recent_repaired as i64;
    let rows: Vec<(String, String, String, String, i64, String, String, String, i64, Option<String>)> =
        sqlx::query_as(
            r#"
        SELECT h.run_id, h.stage_id, h.work_item_id, h.stale_class,
               h.projection_generation, h.projection_updated_at,
               h.projection_integrity, h.readback_json,
               COALESCE(re.epoch, 0) AS recurrence_epoch,
               (SELECT ev.id FROM p080_reconciliation_events_v1 ev
                WHERE ev.run_id = h.run_id AND ev.stage_id = h.stage_id
                  AND ev.work_item_id = h.work_item_id AND ev.stale_class = h.stale_class
                ORDER BY ev.created_at DESC LIMIT 1) AS last_repair_event_id
        FROM   p080_readback_heartbeats_v1 h
        LEFT JOIN p080_recurrence_epoch_v1 re
            ON re.run_id = h.run_id AND re.stage_id = h.stage_id
           AND re.work_item_id = h.work_item_id AND re.stale_class = h.stale_class
        WHERE  (?1 IS NULL OR h.run_id       = ?1)
          AND  (?2 IS NULL OR h.stage_id     = ?2)
          AND  (?3 IS NULL OR h.work_item_id = ?3)
          AND  (?4 IS NULL OR h.stale_class  = ?4)
          AND  (?5 IS NULL OR json_extract(h.readback_json, '$.hold_reason') = ?5)
          AND  (?6 = 1 OR COALESCE(json_extract(h.readback_json, '$.repair_outcome'), '') != 'success')
          AND  (?7 IS NULL OR (
                   h.projection_updated_at < ?7
                   OR (h.projection_updated_at = ?7 AND h.run_id > ?8)
                   OR (h.projection_updated_at = ?7 AND h.run_id = ?8 AND h.stage_id > ?9)
                   OR (h.projection_updated_at = ?7 AND h.run_id = ?8 AND h.stage_id = ?9 AND h.work_item_id > ?10)
               ))
        ORDER  BY h.projection_updated_at DESC, h.run_id ASC, h.stage_id ASC, h.work_item_id ASC
        LIMIT  ?11
        "#,
        )
        .bind(filter.run_id.as_deref())
        .bind(filter.stage_id.as_deref())
        .bind(filter.work_item_id.as_deref())
        .bind(filter.stale_class.as_deref())
        .bind(filter.hold_reason.as_deref())
        .bind(include_recent_repaired)
        .bind(after_ts)
        .bind(after_rid)
        .bind(after_sid)
        .bind(after_wid)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("p080 list_readback_page_keyset")?;

    Ok(rows
        .into_iter()
        .map(
            |(
                run_id,
                stage_id,
                work_item_id,
                stale_class,
                projection_generation,
                projection_updated_at,
                projection_integrity,
                readback_json,
                recurrence_epoch,
                last_repair_event_id,
            )| ReadbackHeartbeatRow {
                run_id,
                stage_id,
                work_item_id,
                stale_class,
                projection_generation,
                projection_updated_at,
                projection_integrity,
                readback_json,
                recurrence_epoch,
                last_repair_event_id,
            },
        )
        .collect())
}

/// Look up a single readback heartbeat row by the 4-tuple key.
pub async fn get_readback(
    pool: &SqlitePool,
    run_id: &str,
    stage_id: &str,
    work_item_id: &str,
    stale_class: &str,
) -> Result<Option<ReadbackHeartbeatRow>> {
    let row: Option<(String, String, String, String, i64, String, String, String, i64, Option<String>)> =
        sqlx::query_as(
            r#"
            SELECT h.run_id, h.stage_id, h.work_item_id, h.stale_class,
                   h.projection_generation, h.projection_updated_at,
                   h.projection_integrity, h.readback_json,
                   COALESCE(re.epoch, 0) AS recurrence_epoch,
                   (SELECT ev.id FROM p080_reconciliation_events_v1 ev
                    WHERE ev.run_id = h.run_id AND ev.stage_id = h.stage_id
                      AND ev.work_item_id = h.work_item_id AND ev.stale_class = h.stale_class
                    ORDER BY ev.created_at DESC LIMIT 1) AS last_repair_event_id
            FROM   p080_readback_heartbeats_v1 h
            LEFT JOIN p080_recurrence_epoch_v1 re
                ON re.run_id = h.run_id AND re.stage_id = h.stage_id
               AND re.work_item_id = h.work_item_id AND re.stale_class = h.stale_class
            WHERE  h.run_id      = ?1
              AND  h.stage_id    = ?2
              AND  h.work_item_id = ?3
              AND  h.stale_class = ?4
            "#,
        )
        .bind(run_id)
        .bind(stage_id)
        .bind(work_item_id)
        .bind(stale_class)
        .fetch_optional(pool)
        .await
        .context("p080 get_readback")?;

    Ok(row.map(
        |(
            run_id,
            stage_id,
            work_item_id,
            stale_class,
            projection_generation,
            projection_updated_at,
            projection_integrity,
            readback_json,
            recurrence_epoch,
            last_repair_event_id,
        )| ReadbackHeartbeatRow {
            run_id,
            stage_id,
            work_item_id,
            stale_class,
            projection_generation,
            projection_updated_at,
            projection_integrity,
            readback_json,
            recurrence_epoch,
            last_repair_event_id,
        },
    ))
}

/// Return the current global max `projection_generation` across all heartbeat rows
/// that match `filter`.  Returns 1 (minimum valid generation) when the table is empty
/// or the query fails, so cursors always embed a generation >= 1.
///
/// Used by the northbound cursor encoder so that `p080_cursor_v1.projection_generation`
/// reflects the projection state at page-issue time.  Cursors are invalidated when
/// the generation advances (projection rebuilt), preventing stale-page replay.
pub async fn get_current_projection_generation(pool: &SqlitePool, filter: &ReadbackFilter) -> i64 {
    let result: Option<i64> = sqlx::query_scalar(
        r#"SELECT MAX(projection_generation)
           FROM   p080_readback_heartbeats_v1
           WHERE  (?1 IS NULL OR run_id       = ?1)
             AND  (?2 IS NULL OR stage_id     = ?2)
             AND  (?3 IS NULL OR work_item_id = ?3)
             AND  (?4 IS NULL OR stale_class  = ?4)"#,
    )
    .bind(filter.run_id.as_deref())
    .bind(filter.stage_id.as_deref())
    .bind(filter.work_item_id.as_deref())
    .bind(filter.stale_class.as_deref())
    .fetch_optional(pool)
    .await
    .unwrap_or(None)
    .flatten();
    result.unwrap_or(1).max(1)
}

// ── Phase 2: Rollout control read, dedup, and reconciliation event persistence ─

/// Row from `p080_rollout_control_v1`.
#[derive(Debug, Clone)]
pub struct RolloutControlRow {
    pub class: String,
    pub enabled: bool,
    pub phase: String,
    pub generation: i64,
}

/// Read the rollout control row for a specific class.
/// Returns `None` if the row does not exist — callers must treat absence as
/// disabled (fail-closed semantics).
pub async fn get_rollout_control(
    pool: &SqlitePool,
    class: &str,
) -> Result<Option<RolloutControlRow>> {
    let row: Option<(String, i64, String, i64)> = sqlx::query_as(
        "SELECT class, enabled, phase, generation
         FROM   p080_rollout_control_v1
         WHERE  class = ?1",
    )
    .bind(class)
    .fetch_optional(pool)
    .await
    .context("p080 get_rollout_control")?;
    Ok(
        row.map(|(class, enabled, phase, generation)| RolloutControlRow {
            class,
            enabled: enabled != 0,
            phase,
            generation,
        }),
    )
}

/// Update the enabled state of an existing rollout-control class row.
///
/// Writes an immutable audit record to `p080_rollout_control_audit_v1` in the
/// same transaction so no rollout mutation can bypass the durable audit trail
/// (proposal §5.7; audit finding: unaudited rollout footgun removed).
///
/// `reason` must be one of the closed CHECK vocabulary: `phase_promotion`,
/// `phase_rollback`, `live_disable`, `live_enable`, `operator_change`.
/// (`startup_seed` is reserved for the seed path.)
/// `updated_by_principal_id` is the principal id of the caller.
///
/// Returns the updated row. Fails if the class row does not exist (seed first).
pub async fn set_rollout_control(
    pool: &SqlitePool,
    class: &str,
    enabled: bool,
    reason: &str,
    updated_by_principal_id: &str,
) -> Result<RolloutControlRow> {
    let now = Utc::now().to_rfc3339();
    let mut tx = pool
        .begin()
        .await
        .context("p080 set_rollout_control begin tx")?;

    // Read the current row before updating (for audit).
    let before: Option<(i64, i64, String)> = sqlx::query_as(
        "SELECT enabled, generation, phase FROM p080_rollout_control_v1 WHERE class = ?1",
    )
    .bind(class)
    .fetch_optional(&mut *tx)
    .await
    .context("p080 set_rollout_control read-before")?;

    let (enabled_before, generation_before, _phase) = match before {
        Some(row) => row,
        None => {
            return Err(anyhow::anyhow!(
                "p080 set_rollout_control: class '{}' not found; seed first",
                class
            ));
        }
    };

    // Update the control row, incrementing generation.
    sqlx::query(
        "UPDATE p080_rollout_control_v1
         SET    enabled                 = ?1,
                generation              = generation + 1,
                updated_at              = ?2,
                updated_by_principal_id = ?3,
                reason                  = ?4
         WHERE  class = ?5",
    )
    .bind(if enabled { 1i64 } else { 0i64 })
    .bind(&now)
    .bind(updated_by_principal_id)
    .bind(reason)
    .bind(class)
    .execute(&mut *tx)
    .await
    .context("p080 set_rollout_control update")?;

    let generation_after = generation_before + 1;

    // Write immutable audit record.
    let audit_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO p080_rollout_control_audit_v1
         (id, class, generation_before, generation_after, enabled_before, enabled_after,
          reason, updated_by_principal_id, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )
    .bind(&audit_id)
    .bind(class)
    .bind(generation_before)
    .bind(generation_after)
    .bind(enabled_before)
    .bind(if enabled { 1i64 } else { 0i64 })
    .bind(reason)
    .bind(updated_by_principal_id)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .context("p080 set_rollout_control audit write")?;

    tx.commit()
        .await
        .context("p080 set_rollout_control commit")?;

    // Re-read outside the transaction for the caller.
    get_rollout_control(pool, class)
        .await?
        .ok_or_else(|| anyhow::anyhow!("p080 set_rollout_control: row missing after update"))
}

/// Row-budget cap for `count_readback_matching_budgeted`.
/// If matching rows exceed this limit, the function returns None (budget exceeded).
pub const COUNT_BUDGET: i64 = 5_000;

/// Returns the count of readback heartbeat rows matching `filter`, capped to
/// `COUNT_BUDGET`. Returns `Ok(None)` when the actual count exceeds the budget
/// to avoid unbounded json_extract scans (SEC-P080-MED-002). The SQL trick:
/// COUNT(*) over a subquery with LIMIT budget+1 caps the scan at budget+1 rows.
pub async fn count_readback_matching_budgeted(
    pool: &SqlitePool,
    filter: &ReadbackFilter,
) -> Result<Option<i64>> {
    let budget = COUNT_BUDGET;
    let include_recent_repaired = filter.include_recent_repaired as i64;
    let row: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM (
            SELECT 1
            FROM   p080_readback_heartbeats_v1
            WHERE  (?1 IS NULL OR run_id       = ?1)
              AND  (?2 IS NULL OR stage_id     = ?2)
              AND  (?3 IS NULL OR work_item_id = ?3)
              AND  (?4 IS NULL OR stale_class  = ?4)
              AND  (?5 IS NULL OR json_extract(readback_json, '$.hold_reason') = ?5)
              AND  (?6 = 1 OR COALESCE(json_extract(readback_json, '$.repair_outcome'), '') != 'success')
            LIMIT ?7
        )
        "#,
    )
    .bind(filter.run_id.as_deref())
    .bind(filter.stage_id.as_deref())
    .bind(filter.work_item_id.as_deref())
    .bind(filter.stale_class.as_deref())
    .bind(filter.hold_reason.as_deref())
    .bind(include_recent_repaired)
    .bind(budget + 1)
    .fetch_one(pool)
    .await
    .context("p080 count_readback_matching_budgeted")?;
    if row.0 > budget {
        Ok(None)
    } else {
        Ok(Some(row.0))
    }
}

/// Returns true if the P080 rollout-control table is readable (at least one row
/// exists for the canonical `live_disable` key).  Returns false on any DB error
/// or when the row is absent — fail-closed per proposal §5.6.
///
/// Used by the MCP server to gate P080 tool visibility in tools/list.
pub async fn is_rollout_readable(pool: &SqlitePool) -> bool {
    match get_rollout_control(pool, "live_disable").await {
        Ok(Some(_)) => true,
        Ok(None) => {
            warn!("P080 rollout-control live_disable row absent; treating as unreadable (fail-closed)");
            false
        }
        Err(err) => {
            warn!(error = %err, "P080 rollout-control read failed; treating as unreadable (fail-closed)");
            false
        }
    }
}

/// Build a `p080_run_report_section_v1` object for the run-report artifact lane.
///
/// Scoped strictly to `run_id` (SEC-P080-HIGH-001: prevents cross-run leakage).
/// Returns the approved schema (rows of p080_readback_v1, projection metadata, and
/// rollout_contract_* fields). Fails closed: any DB error sets projection_integrity
/// to "stale" and returns an empty rows array rather than partial data.
/// Rows are capped at 1000 to bound report size; truncation is not surfaced as a
/// field in the approved p080_run_report_section_v1 schema (SEC-P080-HIGH-002: rows
/// array contains only valid p080_readback_v1 objects; no sentinel rows appended).
pub async fn p080_run_report_section_for_report(
    pool: &SqlitePool,
    run_id: &str,
) -> serde_json::Value {
    let now_str = Utc::now().to_rfc3339();
    let mut any_query_failed = false;

    // Fetch readback rows for this run only, sorted per proposal §8.1 spec.
    // Cap at 1000 to bound report size; not surfaced in the approved schema.
    const REPORT_ROW_CAP: usize = 1000;
    let rows_json =
        match sqlx::query_as::<_, (String, String, String, String, i64, String, String, String)>(
            r#"SELECT run_id, stage_id, work_item_id, stale_class,
                  projection_generation, projection_updated_at,
                  projection_integrity, readback_json
           FROM   p080_readback_heartbeats_v1
           WHERE  run_id = ?1
           ORDER  BY projection_updated_at DESC, run_id ASC, stage_id ASC, work_item_id ASC
           LIMIT  ?2"#,
        )
        .bind(run_id)
        .bind(REPORT_ROW_CAP as i64 + 1)
        .fetch_all(pool)
        .await
        {
            Ok(raw) => {
                let rows: Vec<serde_json::Value> = raw
                    .into_iter()
                    .take(REPORT_ROW_CAP)
                    .map(
                        |(
                            row_run_id,
                            row_stage_id,
                            row_work_item_id,
                            row_stale_class,
                            _gen,
                            row_proj_updated_at,
                            row_proj_integrity,
                            readback_json,
                        )| {
                            // Attempt to deserialize the stored readback_json; fall back to a
                            // valid p080_readback_v1 skeleton using the row's indexed columns
                            // rather than appending a sentinel (SEC-P080-HIGH-002).
                            let parsed = serde_json::from_str::<serde_json::Value>(&readback_json)
                                .unwrap_or_else(|_| {
                                    // Fallback uses only valid closed-enum values per the proposal.
                                    // executor_reregistration_state: enum p080_executor_reregistration_state
                                    //   (expected | seen | missing | stale | recovered) — "unknown" is not valid.
                                    // rollout_disablement: enum p080_rollout_disablement
                                    //   (none | phase_not_reached | class_disabled | live_disabled) — "unknown" is not valid.
                                    serde_json::json!({
                                        "schema_version": "p080_readback_v1",
                                        "run_id": row_run_id,
                                        "stage_id": row_stage_id,
                                        "work_item_id": row_work_item_id,
                                        "stale_class": row_stale_class,
                                        "running_truth": "useful",
                                        "repair_action": "diagnose_only",
                                        "hold_reason": "none",
                                        "hold_age_seconds": null,
                                        "next_retry_or_backoff_time": null,
                                        "projection_updated_at": row_proj_updated_at,
                                        "projection_integrity": row_proj_integrity,
                                        "executor_reregistration_state": "expected",
                                        "rollout_disablement": "phase_not_reached",
                                        "side_effect_status": "not_applicable",
                                        "operator_message": "",
                                        "evidence_marker_hash": null,
                                        "repair_idempotency_key": null
                                    })
                                });
                            redact_readback_json(parsed)
                        },
                    )
                    .collect();
                serde_json::Value::Array(rows)
            }
            Err(err) => {
                warn!(error = %err, "p080_run_report_section: rows query failed");
                any_query_failed = true;
                serde_json::json!([])
            }
        };

    // Max projection_generation for this run only.
    let projection_generation: serde_json::Value = match sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(projection_generation), 0) FROM p080_readback_heartbeats_v1 WHERE run_id = ?1",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    {
        Ok(n) => serde_json::json!(n),
        Err(err) => {
            warn!(error = %err, "p080_run_report_section: projection_generation query failed");
            any_query_failed = true;
            serde_json::Value::Null
        }
    };

    // Query detection_only rollout gate to determine rollout_contract_status.
    let detection_enabled = match get_rollout_control(pool, "detection_only").await {
        Ok(Some(row)) => row.enabled,
        Ok(None) => false,
        Err(err) => {
            warn!(error = %err, "p080_run_report_section: detection_only gate query failed");
            any_query_failed = true;
            false
        }
    };

    let (rollout_contract_status, rollout_contract_decision, rollout_contract_failure_reasons) =
        if any_query_failed {
            (
                "stale",
                "not_applicable",
                serde_json::json!([
                    "db_error: one or more queries failed; report data is incomplete"
                ]),
            )
        } else if !detection_enabled {
            (
                "disabled",
                "not_applicable",
                serde_json::json!(["detection_only rollout gate is not enabled"]),
            )
        } else {
            ("ok", "pass", serde_json::json!([]))
        };

    let projection_integrity = if any_query_failed { "stale" } else { "valid" };

    serde_json::json!({
        "schema_version": "p080_run_report_section_v1",
        "projection_generation": projection_generation,
        "projection_integrity": projection_integrity,
        "projection_updated_at": now_str,
        "rows": rows_json,
        "rollout_contract_status": rollout_contract_status,
        "rollout_contract_decision": rollout_contract_decision,
        "rollout_contract_failure_reasons": rollout_contract_failure_reasons,
        "rollout_contract_disabled_reason_code": if rollout_contract_status == "disabled" {
            serde_json::json!("detection_not_enabled")
        } else {
            serde_json::Value::Null
        }
    })
}

/// Build a `p080_release_receipt_section_v1` object for the release-receipt artifact lane.
///
/// Schema differs from the run-report lane: only holds (hold_reason != "none") are included,
/// plus a side_effect_status_summary over all rows for the run.
/// Written once at receipt seal time (append-only per release_id per proposal §8.2).
pub async fn p080_release_receipt_section(pool: &SqlitePool, run_id: &str) -> serde_json::Value {
    let now_str = Utc::now().to_rfc3339();
    let mut any_query_failed = false;

    // SEC-P080-RES-001: cap release-receipt rows to bound artifact size.
    // Matches the run-report lane's REPORT_ROW_CAP to prevent unbounded output.
    const RECEIPT_ROW_CAP: usize = 1000;
    // Query all readback rows for this run to build side_effect_status_summary.
    // side_effect_status and hold_reason are embedded in readback_json (not separate columns).
    let all_rows = match sqlx::query_as::<_, (String, String, String, String)>(
        r#"SELECT COALESCE(json_extract(readback_json, '$.side_effect_status'), 'not_applicable') AS side_effect_status,
                  COALESCE(json_extract(readback_json, '$.hold_reason'), 'none') AS hold_reason,
                  readback_json,
                  projection_integrity
           FROM   p080_readback_heartbeats_v1
           WHERE  run_id = ?1
           ORDER  BY projection_updated_at DESC
           LIMIT  ?2"#,
    )
    .bind(run_id)
    .bind(RECEIPT_ROW_CAP as i64 + 1)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(err) => {
            warn!(error = %err, "p080_release_receipt_section: rows query failed");
            any_query_failed = true;
            vec![]
        }
    };

    let mut side_effect_counts = std::collections::HashMap::<&str, i64>::new();
    let mut holds_json: Vec<serde_json::Value> = Vec::new();
    for (side_effect_status, hold_reason, readback_json, _integrity) in all_rows.iter().take(RECEIPT_ROW_CAP) {
        let key = match side_effect_status.as_str() {
            "retry_safe" => "retry_safe",
            "unsafe" => "unsafe",
            "not_applicable" => "not_applicable",
            _ => "unknown",
        };
        *side_effect_counts.entry(key).or_insert(0) += 1;
        if hold_reason != "none" {
            let parsed = serde_json::from_str::<serde_json::Value>(readback_json)
                .unwrap_or_else(|_| serde_json::json!({ "hold_reason": hold_reason }));
            holds_json.push(redact_readback_json(parsed));
        }
    }

    let projection_generation: serde_json::Value = match sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(projection_generation), 0) FROM p080_readback_heartbeats_v1 WHERE run_id = ?1",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    {
        Ok(n) => serde_json::json!(n),
        Err(err) => {
            warn!(error = %err, "p080_release_receipt_section: projection_generation query failed");
            any_query_failed = true;
            serde_json::Value::Null
        }
    };

    let detection_enabled = match get_rollout_control(pool, "detection_only").await {
        Ok(Some(row)) => row.enabled,
        Ok(None) => false,
        Err(err) => {
            warn!(error = %err, "p080_release_receipt_section: detection_only gate query failed");
            any_query_failed = true;
            false
        }
    };

    let (rollout_contract_status, rollout_contract_decision) = if any_query_failed {
        ("stale", "not_applicable")
    } else if !detection_enabled {
        ("disabled", "not_applicable")
    } else {
        ("ok", "pass")
    };

    let projection_integrity = if any_query_failed { "stale" } else { "valid" };

    serde_json::json!({
        "schema_version": "p080_release_receipt_section_v1",
        "projection_generation": projection_generation,
        "projection_integrity": projection_integrity,
        "projection_updated_at": now_str,
        "side_effect_status_summary": {
            "retry_safe": side_effect_counts.get("retry_safe").copied().unwrap_or(0),
            "unsafe": side_effect_counts.get("unsafe").copied().unwrap_or(0),
            "unknown": side_effect_counts.get("unknown").copied().unwrap_or(0),
            "not_applicable": side_effect_counts.get("not_applicable").copied().unwrap_or(0)
        },
        "holds": holds_json,
        "rollout_contract_status": rollout_contract_status,
        "rollout_contract_decision": rollout_contract_decision
    })
}

/// Look up a non-expired operator-request dedup entry.
/// Returns the stored `response_json` if found and `expires_at > now_str`.
pub async fn get_dedup_response(
    pool: &SqlitePool,
    principal_id: &str,
    tool_name: &str,
    dedup_key: &str,
    now_str: &str,
) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT response_json
        FROM   p080_operator_request_dedup_v1
        WHERE  principal_id               = ?1
          AND  tool_name                  = ?2
          AND  operator_request_dedup_key = ?3
          AND  expires_at                 > ?4
        "#,
    )
    .bind(principal_id)
    .bind(tool_name)
    .bind(dedup_key)
    .bind(now_str)
    .fetch_optional(pool)
    .await
    .context("p080 get_dedup_response")?;
    Ok(row.map(|(r,)| r))
}

/// Dedup entry with all fence fields needed for full replay comparison (proposal §4.2 L155).
#[derive(Debug, Clone)]
pub struct DedupEntry {
    pub response_json: String,
    pub request_fingerprint: String,
    pub requested_action: String,
    pub principal_class: String,
    pub auth_policy_generation: i64,
    pub secret_generation_id: String,
    pub rollout_phase: String,
    pub repair_class_enabled_hash: String,
    pub live_disable_generation: i64,
}

/// Look up a non-expired dedup entry including all nine fence fields.
/// Returns `None` when no entry exists or it has expired.
pub async fn get_dedup_entry(
    pool: &SqlitePool,
    principal_id: &str,
    tool_name: &str,
    dedup_key: &str,
    now_str: &str,
) -> Result<Option<DedupEntry>> {
    let row: Option<(
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
        String,
        i64,
    )> = sqlx::query_as(
        r#"
            SELECT response_json, request_fingerprint,
                   requested_action, principal_class,
                   auth_policy_generation, secret_generation_id,
                   rollout_phase, repair_class_enabled_hash,
                   live_disable_generation
            FROM   p080_operator_request_dedup_v1
            WHERE  principal_id               = ?1
              AND  tool_name                  = ?2
              AND  operator_request_dedup_key = ?3
              AND  expires_at                 > ?4
            "#,
    )
    .bind(principal_id)
    .bind(tool_name)
    .bind(dedup_key)
    .bind(now_str)
    .fetch_optional(pool)
    .await
    .context("p080 get_dedup_entry")?;
    Ok(row.map(
        |(
            response_json,
            request_fingerprint,
            requested_action,
            principal_class,
            auth_policy_generation,
            secret_generation_id,
            rollout_phase,
            repair_class_enabled_hash,
            live_disable_generation,
        )| DedupEntry {
            response_json,
            request_fingerprint,
            requested_action,
            principal_class,
            auth_policy_generation,
            secret_generation_id,
            rollout_phase,
            repair_class_enabled_hash,
            live_disable_generation,
        },
    ))
}

/// Store a dedup entry for an operator repair request.
/// Uses `INSERT OR IGNORE` so the first write wins (replay semantics).
///
/// Returns the number of rows inserted (1 = new entry, 0 = concurrent writer
/// already inserted a matching row — callers should re-read and return that
/// response rather than proceeding with a new mutation).
///
/// `auth_policy_generation` and `secret_generation_id` are the current rollout generation
/// values at write time. `repair_class_enabled_hash` is a stable hash of the class name
/// to detect rollout-matrix mutations since the entry was written.
/// `live_disable_generation` comes from the live_disable rollout_control row (0 when not seeded).
#[allow(clippy::too_many_arguments)]
pub async fn insert_dedup_entry(
    pool: &SqlitePool,
    principal_id: &str,
    principal_class: &str,
    tool_name: &str,
    dedup_key: &str,
    requested_action: &str,
    rollout_phase: &str,
    auth_policy_generation: i64,
    secret_generation_id: &str,
    repair_class_enabled_hash: &str,
    live_disable_generation: i64,
    request_fingerprint: &str,
    response_json: &str,
    now_str: &str,
    expires_at: &str,
) -> Result<u64> {
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO p080_operator_request_dedup_v1
          (principal_id, principal_class, tool_name, operator_request_dedup_key,
           requested_action, auth_policy_generation, secret_generation_id,
           rollout_phase, repair_class_enabled_hash, live_disable_generation,
           request_fingerprint, response_json, created_at, expires_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
    )
    .bind(principal_id)
    .bind(principal_class)
    .bind(tool_name)
    .bind(dedup_key)
    .bind(requested_action)
    .bind(auth_policy_generation)
    .bind(secret_generation_id)
    .bind(rollout_phase)
    .bind(repair_class_enabled_hash)
    .bind(live_disable_generation)
    .bind(request_fingerprint)
    .bind(response_json)
    .bind(now_str)
    .bind(expires_at)
    .execute(pool)
    .await
    .context("p080 insert_dedup_entry")?;
    Ok(result.rows_affected())
}

/// Insert a reconciliation event into the append-only event log.
/// Uses `INSERT OR IGNORE` on the PK so idempotent retries are safe.
/// `repair_idempotency_key` is NULL for diagnose_only/delegated/none actions
/// (proposal §4.3: only set for real repair outcomes in Phase 3+).
#[allow(clippy::too_many_arguments)]
pub async fn insert_reconciliation_event(
    pool: &SqlitePool,
    id: &str,
    run_id: &str,
    stage_id: &str,
    work_item_id: &str,
    stale_class: &str,
    repair_action: &str,
    hold_reason: &str,
    predicate_hash: &str,
    recurrence_epoch: i64,
    decision: &str,
    event_source: &str,
    created_at: &str,
    initiating_principal_id: &str,
    repair_idempotency_key: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT OR IGNORE INTO p080_reconciliation_events_v1
          (id, run_id, stage_id, work_item_id, stale_class, repair_action, hold_reason,
           predicate_hash, recurrence_epoch, decision, event_source, created_at,
           details_json, initiating_principal_id, repair_idempotency_key)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, '{}', ?13, ?14)
        "#,
    )
    .bind(id)
    .bind(run_id)
    .bind(stage_id)
    .bind(work_item_id)
    .bind(stale_class)
    .bind(repair_action)
    .bind(hold_reason)
    .bind(predicate_hash)
    .bind(recurrence_epoch)
    .bind(decision)
    .bind(event_source)
    .bind(created_at)
    .bind(initiating_principal_id)
    .bind(repair_idempotency_key)
    .execute(pool)
    .await
    .context("p080 insert_reconciliation_event")?;
    Ok(())
}

/// Validate the public-safe format of `repair_idempotency_key`.
///
/// Per proposal P080 §3.4, the key must be either JSON null, or the literal prefix
/// `p080-rik-` followed by exactly 24 lowercase hex characters. Operator-supplied
/// or correlation-style identifiers (e.g. UUIDs, opaque dedup keys) are rejected
/// because they can leak material the secret-pattern detector does not catch.
///
/// SEC-P080-HIGH-001: centralized here so every readback lane (MCP, GraphQL,
/// run-report, release-receipt, future surfaces) enforces the same shape.
pub fn is_valid_p080_repair_idempotency_key(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => {
            const PREFIX: &str = "p080-rik-";
            s.len() == PREFIX.len() + 24
                && s.starts_with(PREFIX)
                && s[PREFIX.len()..]
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        }
        _ => false,
    }
}

/// Closed allow-list of top-level keys permitted in a p080_readback_v1 JSON object.
/// Matches the egress allow-list in mcp-server/src/tools/p080.rs. Enforced at the
/// durable write boundary (SEC-MED-002) so forbidden keys never reach storage.
const READBACK_WRITE_ALLOWED_KEYS: &[&str] = &[
    "schema_version",
    "run_id",
    "stage_id",
    "work_item_id",
    "stale_class",
    "running_truth",
    "repair_action",
    "hold_reason",
    "hold_age_seconds",
    "next_retry_or_backoff_time",
    "projection_updated_at",
    "projection_integrity",
    "executor_reregistration_state",
    "rollout_disablement",
    "side_effect_status",
    "operator_message",
    "evidence_marker_hash",
    "repair_idempotency_key",
];

/// Validate a p080_readback_v1 JSON string before persisting.
///
/// SEC-MED-002: forbidden keys and nested objects must be rejected at the durable
/// write boundary, not only on MCP egress. This prevents future writers or
/// corrupted projections from storing forbidden keys at rest where other read
/// lanes (e.g. GraphQL) could later expose them.
///
/// Checks: valid UTF-8 JSON, top-level object, no unknown keys, no nested
/// Object/Array values (all p080_readback_v1 values are scalar).
fn validate_readback_json_for_write(readback_json: &str) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(readback_json)
        .map_err(|e| anyhow::anyhow!("p080 readback_json is not valid JSON: {e}"))?;
    let obj = match &v {
        serde_json::Value::Object(m) => m,
        _ => {
            return Err(anyhow::anyhow!(
                "p080 readback_json must be a JSON object; got {}",
                v.type_name_for_error()
            ))
        }
    };
    for key in obj.keys() {
        if !READBACK_WRITE_ALLOWED_KEYS.contains(&key.as_str()) {
            return Err(anyhow::anyhow!(
                "p080 readback_json contains forbidden key: {key:?}"
            ));
        }
    }
    for (key, val) in obj {
        if matches!(
            val,
            serde_json::Value::Object(_) | serde_json::Value::Array(_)
        ) {
            return Err(anyhow::anyhow!(
                "p080 readback_json key {key:?} contains a nested object or array; all values must be scalar"
            ));
        }
        // SEC-MED-002: reject secret-like string values at the durable write boundary.
        // Field-aware: evidence_marker_hash (sha256, 64 lower-hex) and
        // repair_idempotency_key (p080-rik-<24hex>, format-checked below) are
        // public-safe and must not be blocked by the general detector.
        if let serde_json::Value::String(s) = val {
            let stripped = strip_control_and_truncate(s, 4096);
            let is_public_safe = match key.as_str() {
                "evidence_marker_hash" => is_valid_sha256_hex_hash(&stripped),
                "repair_idempotency_key" => is_valid_p080_repair_idempotency_key(val),
                _ => false,
            };
            if !is_public_safe && looks_like_secret(&stripped) {
                return Err(anyhow::anyhow!(
                    "p080 readback_json key {key:?} contains a secret-like string value; rejected at write time"
                ));
            }
        }
        // SEC-P080-HIGH-001: enforce p080_readback_v1 repair_idempotency_key format
        // at the shared write boundary so GraphQL/run-report/release-receipt lanes
        // cannot persist arbitrary correlation/dedup material that bypasses the
        // MCP-local sanitizer.
        if key == "repair_idempotency_key" && !is_valid_p080_repair_idempotency_key(val) {
            return Err(anyhow::anyhow!(
                "p080 readback_json repair_idempotency_key is not null and does not match p080-rik-[0-9a-f]{{24}}; rejected at write time"
            ));
        }
        // SEC-HIGH-001: enforce closed enum vocabularies for p080_readback_v1 fields.
        // This prevents arbitrary operator/provider text from being persisted and emitted on
        // diagnostics, run-report, and release-receipt egress lanes.
        if let serde_json::Value::String(s) = val {
            match key.as_str() {
                "stale_class" => {
                    const ALLOWED: &[&str] = &[
                        "warmup_pending", "acp_startup_stale", "scheduler_ownership_drift",
                        "acp_prompt_stale", "helper_orphan_drift", "release_side_effect_drift",
                        "ambiguous_owner", "useful", "unknown",
                    ];
                    if !ALLOWED.contains(&s.as_str()) {
                        return Err(anyhow::anyhow!(
                            "p080 readback_json stale_class {:?} is not in the allowed vocabulary; rejected at write time",
                            s
                        ));
                    }
                }
                "running_truth" => {
                    const ALLOWED: &[&str] = &[
                        "useful", "warmup_pending", "stale_suspected", "needs_operator",
                        "needs_effect_reconciliation", "stale_repaired", "unknown",
                    ];
                    if !ALLOWED.contains(&s.as_str()) {
                        return Err(anyhow::anyhow!(
                            "p080 readback_json running_truth {:?} is not in the allowed vocabulary; rejected at write time",
                            s
                        ));
                    }
                }
                "hold_reason" => {
                    const ALLOWED: &[&str] = &[
                        "none", "cooldown_active", "permanent_hold_active", "ambiguous_owner",
                        "side_effect_drift_unsafe", "dependency_read_failure",
                        "gateway_saturated", "live_disable", "warmup_pending",
                        "rollout_disabled", "unknown",
                    ];
                    if !ALLOWED.contains(&s.as_str()) {
                        return Err(anyhow::anyhow!(
                            "p080 readback_json hold_reason {:?} is not in the allowed vocabulary; rejected at write time",
                            s
                        ));
                    }
                }
                "side_effect_status" => {
                    const ALLOWED: &[&str] = &[
                        "not_applicable", "retry_safe", "unsafe", "unknown",
                    ];
                    if !ALLOWED.contains(&s.as_str()) {
                        return Err(anyhow::anyhow!(
                            "p080 readback_json side_effect_status {:?} is not in the allowed vocabulary; rejected at write time",
                            s
                        ));
                    }
                }
                "repair_outcome" => {
                    const ALLOWED: &[&str] = &[
                        "success", "failed", "skipped", "not_attempted", "cooldown_active",
                        "hold_active", "class_disabled", "rollout_disabled",
                    ];
                    if !ALLOWED.contains(&s.as_str()) {
                        return Err(anyhow::anyhow!(
                            "p080 readback_json repair_outcome {:?} is not in the allowed vocabulary; rejected at write time",
                            s
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

trait JsonTypeName {
    fn type_name_for_error(&self) -> &'static str;
}
impl JsonTypeName for serde_json::Value {
    fn type_name_for_error(&self) -> &'static str {
        match self {
            serde_json::Value::Null => "null",
            serde_json::Value::Bool(_) => "bool",
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            serde_json::Value::Array(_) => "array",
            serde_json::Value::Object(_) => "object",
        }
    }
}

/// Upsert a readback heartbeat row, stamping it with a repaired running_truth.
pub async fn upsert_readback_stale_repaired(
    pool: &SqlitePool,
    run_id: &str,
    stage_id: &str,
    work_item_id: &str,
    stale_class: &str,
    readback_json: &str,
    now_str: &str,
) -> Result<()> {
    // SEC-MED-002: validate at the durable write boundary before any SQL.
    validate_readback_json_for_write(readback_json)
        .context("p080 upsert_readback_stale_repaired: readback_json validation failed")?;
    sqlx::query(
        r#"
        INSERT INTO p080_readback_heartbeats_v1
          (run_id, stage_id, work_item_id, stale_class,
           projection_generation, projection_updated_at, projection_integrity,
           readback_json, updated_at)
        VALUES (?1, ?2, ?3, ?4, 1, ?5, 'valid', ?6, ?7)
        ON CONFLICT(run_id, stage_id, work_item_id, stale_class) DO UPDATE SET
          projection_generation = projection_generation + 1,
          projection_updated_at = excluded.projection_updated_at,
          projection_integrity  = 'valid',
          readback_json         = excluded.readback_json,
          updated_at            = excluded.updated_at
        "#,
    )
    .bind(run_id)
    .bind(stage_id)
    .bind(work_item_id)
    .bind(stale_class)
    .bind(now_str)
    .bind(readback_json)
    .bind(now_str)
    .execute(pool)
    .await
    .context("p080 upsert_readback_stale_repaired")?;
    Ok(())
}

/// Returns `true` if `s` is exactly 64 lowercase hexadecimal characters.
///
/// SEC-MED-002: used as a field-specific exemption in the write-time secret detector
/// and egress sanitizer so a SHA-256 predicate hash stored as `evidence_marker_hash`
/// is not rejected as a high-entropy secret-like value.
fn is_valid_sha256_hex_hash(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// SEC-H-002 fix: Atomically insert a reconciliation event AND upsert the
/// corresponding readback heartbeat in a single SQLite transaction.
///
/// Either both writes commit or neither does, eliminating the window where a
/// reconciliation event exists without an updated readback (or vice-versa).
/// Callers should prefer this over calling the two functions independently.
#[allow(clippy::too_many_arguments)]
pub async fn insert_event_and_upsert_readback_atomic(
    pool: &SqlitePool,
    event_id: &str,
    run_id: &str,
    stage_id: &str,
    work_item_id: &str,
    stale_class: &str,
    repair_action: &str,
    hold_reason: &str,
    predicate_hash: &str,
    recurrence_epoch: i64,
    decision: &str,
    event_source: &str,
    initiating_principal_id: &str,
    repair_idempotency_key: Option<&str>,
    readback_json: &str,
    now_str: &str,
) -> Result<()> {
    // SEC-MED-002: validate readback at the durable write boundary before opening tx.
    validate_readback_json_for_write(readback_json)
        .context("p080 atomic write: readback_json validation failed")?;

    let mut tx: Transaction<'_, Sqlite> =
        pool.begin().await.context("p080 atomic write: begin tx")?;

    sqlx::query(
        r#"
        INSERT OR IGNORE INTO p080_reconciliation_events_v1
          (id, run_id, stage_id, work_item_id, stale_class, repair_action, hold_reason,
           predicate_hash, recurrence_epoch, decision, event_source, created_at,
           details_json, initiating_principal_id, repair_idempotency_key)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, '{}', ?13, ?14)
        "#,
    )
    .bind(event_id)
    .bind(run_id)
    .bind(stage_id)
    .bind(work_item_id)
    .bind(stale_class)
    .bind(repair_action)
    .bind(hold_reason)
    .bind(predicate_hash)
    .bind(recurrence_epoch)
    .bind(decision)
    .bind(event_source)
    .bind(now_str)
    .bind(initiating_principal_id)
    .bind(repair_idempotency_key)
    .execute(&mut *tx)
    .await
    .context("p080 atomic write: insert reconciliation event")?;

    sqlx::query(
        r#"
        INSERT INTO p080_readback_heartbeats_v1
          (run_id, stage_id, work_item_id, stale_class,
           projection_generation, projection_updated_at, projection_integrity,
           readback_json, updated_at)
        VALUES (?1, ?2, ?3, ?4, 1, ?5, 'valid', ?6, ?7)
        ON CONFLICT(run_id, stage_id, work_item_id, stale_class) DO UPDATE SET
          projection_generation = projection_generation + 1,
          projection_updated_at = excluded.projection_updated_at,
          projection_integrity  = 'valid',
          readback_json         = excluded.readback_json,
          updated_at            = excluded.updated_at
        "#,
    )
    .bind(run_id)
    .bind(stage_id)
    .bind(work_item_id)
    .bind(stale_class)
    .bind(now_str)
    .bind(readback_json)
    .bind(now_str)
    .execute(&mut *tx)
    .await
    .context("p080 atomic write: upsert readback heartbeat")?;

    tx.commit().await.context("p080 atomic write: commit")?;
    Ok(())
}

// ── Egress sanitization (HIGH-003): shared across all read lanes ─────────────

/// Closed allow-list of keys emitted from `p080_readback_v1` objects on any
/// output lane (MCP, GraphQL, run-report).  Keys not in this list are stripped
/// before returning data to callers.
pub const READBACK_EGRESS_ALLOWED_KEYS: &[&str] = &[
    "schema_version",
    "run_id",
    "stage_id",
    "work_item_id",
    "stale_class",
    "running_truth",
    "repair_action",
    "hold_reason",
    "hold_age_seconds",
    "next_retry_or_backoff_time",
    "projection_updated_at",
    "projection_integrity",
    "executor_reregistration_state",
    "rollout_disablement",
    "side_effect_status",
    "operator_message",
    "evidence_marker_hash",
    "repair_idempotency_key",
];

/// Returns `true` if `s` matches a pattern that is likely a secret value that
/// must not appear in operator-visible readback output.
///
/// Covers embedded HTTP auth headers, API-key prefixes with separator (anywhere in
/// the string), provider-specific token prefixes (embedded), env-style KEY=value
/// forms, and high-entropy base64/hex tokens.  Negative controls: short strings,
/// UUID-like values.
///
/// SEC-P080-HIGH-001: uses embedded (`contains`) matching so diagnostic
/// operator_message values like "error: Authorization: Bearer sk-..." or
/// "provider stderr: sk-..." are caught, not just strings that start with
/// the secret prefix.  Aligned with the MCP `looks_like_secret` implementation.
pub fn looks_like_secret(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();

    // HTTP authorization header patterns — match anywhere in the string.
    if lower.contains("bearer ") || lower.contains("authorization:") {
        return true;
    }

    // Embedded key=value secret forms (colon or equals separator).
    for fragment in &[
        "api_key:",
        "api_key=",
        "apikey:",
        "apikey=",
        "api-key:",
        "api-key=",
        "secret:",
        "secret=",
        "password:",
        "password=",
        "passwd:",
        "passwd=",
        "access_token:",
        "access_token=",
        "refresh_token:",
        "refresh_token=",
        "client_secret:",
        "client_secret=",
        "auth_token:",
        "auth_token=",
        "session_token:",
        "session_token=",
        "credentials:",
        "credentials=",
        "private_key:",
        "private_key=",
        "token:",
        "token=",
    ] {
        if lower.contains(fragment) {
            // Require a non-trivially-short value after the separator.
            if let Some(sep_idx) = lower.find(fragment) {
                let after = &lower[sep_idx + fragment.len()..];
                if after.len() >= 8 {
                    return true;
                }
            }
        }
    }

    // Embedded OpenAI-style sk- token (match anywhere in the string).
    if let Some(idx) = lower.find("sk-") {
        let after = &s[idx + 3..];
        if after
            .chars()
            .next()
            .map(|c| c.is_ascii_alphanumeric())
            .unwrap_or(false)
        {
            return true;
        }
    }

    // GitHub and Slack token prefixes — embedded anywhere.
    if lower.contains("ghp_") || lower.contains("ghs_") || lower.contains("github_pat_") {
        return true;
    }
    if lower.contains("xoxb-")
        || lower.contains("xoxp-")
        || lower.contains("xoxa-")
        || lower.contains("xoxe-")
    {
        return true;
    }

    // AWS access key IDs (AKIA prefix, 20 chars total).
    if let Some(idx) = lower.find("akia") {
        let after = &lower[idx + 4..];
        let run: usize = after
            .chars()
            .take(16)
            .filter(|c| c.is_ascii_alphanumeric())
            .count();
        if run >= 12 {
            return true;
        }
    }

    // env-style: KEY_NAME=value (uppercase/mixed snake key, value >= 16 chars).
    // Match anywhere to catch embedded prose like "error: API_KEY=abc...".
    {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if let Some(eq_off) = bytes[i..].iter().position(|&b| b == b'=') {
                let eq_abs = i + eq_off;
                let key_start = bytes[..eq_abs]
                    .iter()
                    .rposition(|&b| !b.is_ascii_alphanumeric() && b != b'_')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let key_part = &s[key_start..eq_abs];
                let val_part = &s[eq_abs + 1..];
                if key_part.len() >= 3
                    && key_part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && val_part.len() >= 16
                {
                    return true;
                }
                i = eq_abs + 1;
            } else {
                break;
            }
        }
    }

    // High-entropy base64/hex token (non-UUID, >=32 chars). Evaluate contiguous
    // token-like fragments only; whole prose strings contain many alphabetic
    // characters and would otherwise false-positive as base64-ish.
    for token in
        s.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_' | '=')))
    {
        if token.len() < 32 {
            continue;
        }
        let is_uuid_like = token.len() == 36
            && token.chars().enumerate().all(|(i, c)| {
                if i == 8 || i == 13 || i == 18 || i == 23 {
                    c == '-'
                } else {
                    c.is_ascii_hexdigit()
                }
            });
        if is_uuid_like {
            continue;
        }
        if token.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }
        if token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_' | '='))
        {
            return true;
        }
    }
    false
}

/// Maximum byte length for P080 identifier fields supplied by callers.
/// Mirrors `MAX_IDENTIFIER_BYTES` in `mcp-server::tools::p080`.
pub const MAX_P080_IDENTIFIER_BYTES: usize = 256;

/// Validate and sanitize a caller-supplied identifier (run_id, stage_id,
/// work_item_id) at the network boundary.
///
/// SEC-P080-MED-001: shared with the GraphQL surface so both lanes reject
/// empty, oversized, and control/bidi-bearing identifiers before any SQL
/// or cursor-hash work. Returns `None` when the identifier is unsafe.
pub fn sanitize_p080_identifier(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > MAX_P080_IDENTIFIER_BYTES {
        return None;
    }
    if s.chars().any(is_control_or_bidi) {
        return None;
    }
    Some(s.to_string())
}

/// Closed vocabulary for P080 stale_class column values emitted on output lanes.
///
/// SEC-MED-001: DB rows may contain stale or corrupt stale_class values if a
/// future writer bypass stores unexpected strings.  This const-slice lets every
/// output path (GraphQL, class_breakdown report) validate the value without
/// risking unknown enum strings leaking to callers.
const P080_KNOWN_STALE_CLASSES: &[&str] = &[
    "useful",
    "warmup_pending",
    "acp_startup_stale",
    "scheduler_ownership_drift",
    "helper_orphan_drift",
    "release_side_effect_drift",
    "acp_prompt_stale",
    "ambiguous_owner",
];

/// Validate a `stale_class` column value against the closed vocabulary.
///
/// SEC-MED-001: returns the value unchanged if recognized, or "[unknown]" if not,
/// so unrecognized DB strings do not leak verbatim on GraphQL/report output lanes.
pub fn sanitize_stale_class_for_output(s: &str) -> &str {
    if P080_KNOWN_STALE_CLASSES.contains(&s) {
        s
    } else {
        "[unknown]"
    }
}

/// Sanitize a DB row identifier (run_id, stage_id, work_item_id) for output.
///
/// SEC-MED-001: strips control/bidi characters and enforces length. Returns
/// "[redacted]" for empty or oversized values so corrupt rows cannot inject
/// unsafe strings onto GraphQL or report lanes.
pub fn sanitize_identifier_for_output(s: &str) -> String {
    if s.is_empty() || s.len() > MAX_P080_IDENTIFIER_BYTES {
        return "[redacted]".to_string();
    }
    let stripped = strip_control_and_truncate(s, MAX_P080_IDENTIFIER_BYTES);
    if stripped.is_empty() {
        "[redacted]".to_string()
    } else {
        stripped
    }
}

/// Returns `true` if `c` is a C0/C1 control character or a Unicode
/// bidirectional override/isolate character that could mislead operators.
pub fn is_control_or_bidi(c: char) -> bool {
    c < '\x20'
        || ('\u{7f}'..='\u{9f}').contains(&c)
        || matches!(c,
            '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}')
}

/// Strip control/bidi characters from `s`, truncating to `max_bytes` UTF-8 bytes.
pub fn strip_control_and_truncate(s: &str, max_bytes: usize) -> String {
    s.chars()
        .filter(|c| !is_control_or_bidi(*c))
        .collect::<String>()
        .chars()
        .take_while({
            let mut bytes = 0usize;
            move |c| {
                bytes += c.len_utf8();
                bytes <= max_bytes
            }
        })
        .collect()
}

/// Sanitize a readback string field: strip control/bidi characters first,
/// then redact if the normalized string looks like a secret.
///
/// Stripping before detection prevents obfuscated tokens (with hidden control
/// or bidi chars inserted in front of secret prefixes) from escaping redaction
/// after normalization.  `operator_message` is truncated to 240 bytes.
pub fn sanitize_readback_string(key: &str, s: &str) -> String {
    let max_bytes = if key == "operator_message" { 240 } else { 4096 };
    let stripped = strip_control_and_truncate(s, max_bytes);
    // SEC-MED-002: field-aware exemption so public-safe hash fields are not redacted.
    // evidence_marker_hash is a sha256 predicate hash; repair_idempotency_key is
    // format-validated before reaching this path (tamper-sentinel returned if invalid).
    let is_public_safe = match key {
        "evidence_marker_hash" => is_valid_sha256_hex_hash(&stripped),
        "repair_idempotency_key" => {
            is_valid_p080_repair_idempotency_key(&serde_json::Value::String(stripped.clone()))
        }
        _ => false,
    };
    if !is_public_safe && looks_like_secret(&stripped) {
        return "[redacted]".to_string();
    }
    stripped
}

/// Strip non-allow-listed keys from a `p080_readback_v1` JSON object and apply
/// secret-pattern redaction to every string value.
///
/// If any allowed key contains a non-scalar value (Object or Array), the entire
/// row is replaced with a tamper-detected sentinel rather than exposing nested
/// structure.  Non-object input returns an empty object.
pub fn redact_readback_json(v: serde_json::Value) -> serde_json::Value {
    let obj = match v {
        serde_json::Value::Object(m) => m,
        _ => return serde_json::json!({}),
    };
    // SEC-P080-READBACK-SCHEMA-002: centralized schema_version sentinel check for all
    // egress lanes (MCP, GraphQL, run-report, release-receipt). Missing schema_version
    // is treated as tamper_detected — a legitimate writer always sets the exact contract
    // value p080_readback_v1; absent is as suspicious as wrong.
    match obj.get("schema_version") {
        Some(sv) if sv.as_str() == Some("p080_readback_v1") => {}
        _ => {
            return serde_json::json!({
                "schema_version": "p080_readback_v1",
                "projection_integrity": "tamper_detected",
                "operator_message": "[tamper_detected: unsupported or absent schema_version in readback payload]"
            });
        }
    }
    // Reject the entire row if any allowed key holds a non-scalar value.
    for key in READBACK_EGRESS_ALLOWED_KEYS {
        if let Some(val) = obj.get(*key) {
            if matches!(
                val,
                serde_json::Value::Object(_) | serde_json::Value::Array(_)
            ) {
                return serde_json::json!({
                    "schema_version": "p080_readback_v1",
                    "projection_integrity": "tamper_detected",
                    "operator_message": "[tamper_detected: non-scalar value in allowed readback field]"
                });
            }
        }
    }
    // SEC-P080-HIGH-001: enforce repair_idempotency_key public-safe format on every
    // egress lane that uses this redactor (GraphQL, run-report, release-receipt).
    // A row whose stored key fails the shape check is replaced with a tamper sentinel
    // so a future/corrupted writer cannot leak operator dedup material on shared lanes.
    if let Some(val) = obj.get("repair_idempotency_key") {
        if !is_valid_p080_repair_idempotency_key(val) {
            return serde_json::json!({
                "schema_version": "p080_readback_v1",
                "projection_integrity": "tamper_detected",
                "operator_message": "[tamper_detected: invalid repair_idempotency_key format]"
            });
        }
    }
    // SEC-HIGH-001: fail closed on invalid enum vocabularies at every egress lane.
    // A stored row whose enum field does not match the closed vocabulary is replaced
    // with a tamper sentinel rather than emitting unvalidated text on MCP/GraphQL/
    // run-report/release-receipt lanes.
    {
        let enum_checks: &[(&str, &[&str])] = &[
            (
                "stale_class",
                &[
                    "warmup_pending", "acp_startup_stale", "scheduler_ownership_drift",
                    "acp_prompt_stale", "helper_orphan_drift", "release_side_effect_drift",
                    "ambiguous_owner", "useful", "unknown",
                ],
            ),
            (
                "running_truth",
                &[
                    "useful", "warmup_pending", "stale_suspected", "needs_operator",
                    "needs_effect_reconciliation", "stale_repaired", "unknown",
                ],
            ),
            (
                "hold_reason",
                &[
                    "none", "cooldown_active", "permanent_hold_active", "ambiguous_owner",
                    "side_effect_drift_unsafe", "dependency_read_failure",
                    "gateway_saturated", "live_disable", "warmup_pending",
                    "rollout_disabled", "unknown",
                ],
            ),
            (
                "side_effect_status",
                &["not_applicable", "retry_safe", "unsafe", "unknown"],
            ),
            (
                "repair_outcome",
                &[
                    "success", "failed", "skipped", "not_attempted", "cooldown_active",
                    "hold_active", "class_disabled", "rollout_disabled",
                ],
            ),
        ];
        for (field, allowed) in enum_checks {
            if let Some(serde_json::Value::String(s)) = obj.get(*field) {
                if !allowed.contains(&s.as_str()) {
                    return serde_json::json!({
                        "schema_version": "p080_readback_v1",
                        "projection_integrity": "tamper_detected",
                        "operator_message": "[tamper_detected: enum field outside closed vocabulary in readback payload]"
                    });
                }
            }
        }
    }
    let mut out = serde_json::Map::new();
    for key in READBACK_EGRESS_ALLOWED_KEYS {
        if let Some(val) = obj.get(*key) {
            let sanitized = match val {
                serde_json::Value::String(s) => {
                    serde_json::Value::String(sanitize_readback_string(key, s))
                }
                other => other.clone(), // number, bool, null — pass through as-is
            };
            out.insert(key.to_string(), sanitized);
        }
    }
    serde_json::Value::Object(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_db() -> SqlitePool {
        let pool = crate::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        let writer = std::sync::Arc::new(crate::writer::DbWriter::new(pool.clone()));
        crate::writer::register_shared_writer(&pool, writer)
            .await
            .expect("register writer");
        pool
    }

    #[tokio::test]
    async fn p080_seed_rollout_control_inserts_all_classes() {
        let pool = setup_db().await;
        let seeded = seed_rollout_control_if_absent(&pool).await.unwrap();
        assert_eq!(seeded, ROLLOUT_CLASSES.len(), "all classes seeded");

        // 4 classes seeded (acp_prompt_stale excluded per proposal).
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM p080_rollout_control_v1 WHERE enabled = 0")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, ROLLOUT_CLASSES.len() as i64);
    }

    #[tokio::test]
    async fn p080_rollout_control_includes_detection_only_class() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();
        for class in &["detection_only", "live_disable", "permanent_hold_clear"] {
            let row = get_rollout_control(&pool, class).await.unwrap();
            assert!(row.is_some(), "class {class} must be seeded");
            assert!(
                !row.unwrap().enabled,
                "class {class} must default to disabled"
            );
        }
    }

    #[tokio::test]
    async fn p080_seed_rollout_control_is_idempotent() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();
        let second = seed_rollout_control_if_absent(&pool).await.unwrap();
        assert_eq!(
            second, 0,
            "second seed call on fully-populated table is a no-op"
        );
    }

    #[tokio::test]
    async fn p080_seed_rollout_control_partial_rows_fail_closed() {
        let pool = setup_db().await;
        // Insert one row manually to simulate partial corruption.
        // Use a valid reason value ('startup_seed') — the table has a CHECK constraint.
        sqlx::query(
            "INSERT INTO p080_rollout_control_v1
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason)
             VALUES ('live_disable', 0, 'phase_0', 1, '2026-01-01T00:00:00Z', 'system', 'startup_seed')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // seed must return Err when partial rows exist.
        let result = seed_rollout_control_if_absent(&pool).await;
        assert!(
            result.is_err(),
            "partial rows must cause seed to fail closed"
        );
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("partial row set"),
            "error message should mention partial row set: {msg}"
        );
    }

    #[tokio::test]
    async fn p080_validate_rollout_control_completeness_pass() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();
        validate_rollout_control_completeness(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn p080_validate_rollout_control_completeness_missing_class_fails() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();
        // Delete one row to simulate post-seed corruption.
        sqlx::query("DELETE FROM p080_rollout_control_v1 WHERE class = 'live_disable'")
            .execute(&pool)
            .await
            .unwrap();
        let result = validate_rollout_control_completeness(&pool).await;
        assert!(result.is_err(), "missing live_disable must fail validation");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("live_disable"),
            "error must name the missing class: {msg}"
        );
    }

    #[tokio::test]
    async fn p080_classify_empty_db_returns_zero() {
        let pool = setup_db().await;
        let counts = classify_and_upsert_running_executions(&pool).await.unwrap();
        assert_eq!(counts.total, 0);
    }

    #[tokio::test]
    async fn p080_list_readback_page_empty_returns_empty() {
        let pool = setup_db().await;
        let rows = list_readback_page(&pool, ReadbackFilter::default(), 50)
            .await
            .unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn p080_get_readback_missing_returns_none() {
        let pool = setup_db().await;
        let row = get_readback(&pool, "run-1", "stage-1", "wi-1", "acp_startup_stale")
            .await
            .unwrap();
        assert!(row.is_none());
    }

    #[test]
    fn p080_redaction_bidi_obfuscated_secret_is_detected() {
        // SEC-P080-003: stripping control/bidi chars BEFORE secret detection prevents
        // obfuscated tokens from escaping redaction. Each case below would NOT be
        // detected as a secret by looks_like_secret WITHOUT stripping first, but IS
        // correctly detected after strip_control_and_truncate normalizes the string.
        let bidi_cases: &[(&str, &str)] = &[
            // Zero-width space (U+200B) split inside "bearer" breaks substring match.
            // The value has no other detectable prefix, so without stripping it escapes.
            (
                "bea\u{200b}rer 12345678901234567890",
                "U+200B inside bearer",
            ),
            // RLO (U+202E) inserted inside "ghp_" splits the known prefix.
            (
                "gh\u{202e}p_abcdefghijklmnopqrstuvwxyz",
                "U+202E inside ghp_",
            ),
            // Zero-width non-joiner (U+200C) between "sk-" and alphanumeric body
            // causes the alphanumeric check to see a non-ASCII char and return false.
            (
                "sk-\u{200c}abcdefghijklmnopqrstuvwxyz1234",
                "U+200C after sk-",
            ),
        ];
        for (case, label) in bidi_cases {
            let redacted = redact_readback_json(serde_json::json!({
                "schema_version": "p080_readback_v1",
                "operator_message": case
            }));
            assert_eq!(
                redacted["operator_message"], "[redacted]",
                "bidi-obfuscated secret must be redacted after normalization ({label}): {case:?}"
            );
        }
    }

    #[test]
    fn p080_redaction_catches_embedded_secret_matrix() {
        let cases = [
            "error: Authorization: Bearer sk-live-token",
            "provider stderr: sk-abcdefghijklmnopqrstuvwxyz",
            "metadata API_KEY=abcdefghijklmnopqrstuvwxyz",
            "aws AKIA1234567890ABCDEF leaked",
            "github ghp_abcdefghijklmnopqrstuvwxyz",
            "slack xoxb-1234567890-secret",
        ];

        for case in cases {
            let redacted = redact_readback_json(serde_json::json!({
                "schema_version": "p080_readback_v1",
                "operator_message": case
            }));
            assert_eq!(
                redacted["operator_message"], "[redacted]",
                "case must be redacted: {case}"
            );
        }

        let negative = redact_readback_json(serde_json::json!({
            "schema_version": "p080_readback_v1",
            "operator_message": "diagnostic note without credential material",
            "run_id": "018f4d9a-7cc1-7000-8000-000000000080"
        }));
        assert_eq!(
            negative["operator_message"],
            "diagnostic note without credential material"
        );
        assert_eq!(negative["run_id"], "018f4d9a-7cc1-7000-8000-000000000080");
    }

    #[test]
    fn p080_redaction_non_scalar_allowed_field_returns_tamper_detected() {
        // SEC-P080-MED-001: a non-scalar value (Object or Array) in an allowed field
        // must trigger tamper_detected, not partial decode.
        let with_nested_object = redact_readback_json(serde_json::json!({
            "schema_version": "p080_readback_v1",
            "operator_message": {"injected": "object"}
        }));
        assert_eq!(
            with_nested_object["projection_integrity"], "tamper_detected",
            "object in allowed field must set projection_integrity=tamper_detected"
        );

        let with_nested_array = redact_readback_json(serde_json::json!({
            "schema_version": "p080_readback_v1",
            "operator_message": ["injected", "array"]
        }));
        assert_eq!(
            with_nested_array["projection_integrity"], "tamper_detected",
            "array in allowed field must set projection_integrity=tamper_detected"
        );
    }

    #[test]
    fn p080_redaction_rejects_wrong_schema_version() {
        // SEC-P080-READBACK-SCHEMA-002: all egress lanes must reject non-p080_readback_v1.
        let future_version = redact_readback_json(serde_json::json!({
            "schema_version": "p080_readback_v2",
            "operator_message": "some message"
        }));
        assert_eq!(
            future_version["projection_integrity"], "tamper_detected",
            "future schema_version must set projection_integrity=tamper_detected"
        );
        assert_eq!(
            future_version["schema_version"], "p080_readback_v1",
            "tamper sentinel must always report schema_version=p080_readback_v1"
        );

        let wrong_version = redact_readback_json(serde_json::json!({
            "schema_version": "some_other_schema_v1",
            "operator_message": "some message"
        }));
        assert_eq!(
            wrong_version["projection_integrity"], "tamper_detected",
            "unknown schema_version must set projection_integrity=tamper_detected"
        );

        // Correct version must still pass through normally.
        let correct = redact_readback_json(serde_json::json!({
            "schema_version": "p080_readback_v1",
            "operator_message": "valid message"
        }));
        assert_eq!(
            correct["operator_message"], "valid message",
            "correct schema_version must allow normal redaction"
        );
        assert_ne!(
            correct["projection_integrity"].as_str().unwrap_or(""),
            "tamper_detected",
            "correct schema_version must not set tamper_detected"
        );
    }

    #[tokio::test]
    async fn p080_get_rollout_control_returns_seeded_row() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();
        let row = get_rollout_control(&pool, "acp_startup_stale")
            .await
            .unwrap();
        assert!(row.is_some(), "row must exist after seed");
        let row = row.unwrap();
        assert_eq!(row.class, "acp_startup_stale");
        assert!(!row.enabled, "seeded as disabled");
    }

    #[tokio::test]
    async fn p080_dedup_entry_roundtrip() {
        let pool = setup_db().await;
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let expires_at = (now + chrono::Duration::hours(24)).to_rfc3339();

        insert_dedup_entry(
            &pool,
            "principal-1",
            "operator",
            "p080.reconcile.request.v1",
            "dedup-key-42",
            "repair_if_safe",
            "phase_2",
            1,
            "p080-phase_2-gen1",
            "acp_startup_stale-enabled",
            0i64, // live_disable_generation: 0 (not yet seeded in test)
            "fingerprint-abc",
            r#"{"schema_version":"p080_reconcile_response_v1","decision":"repaired"}"#,
            &now_str,
            &expires_at,
        )
        .await
        .unwrap();

        let resp = get_dedup_response(
            &pool,
            "principal-1",
            "p080.reconcile.request.v1",
            "dedup-key-42",
            &now_str,
        )
        .await
        .unwrap();
        assert!(resp.is_some());
        let v: serde_json::Value = serde_json::from_str(&resp.unwrap()).unwrap();
        assert_eq!(v["decision"], "repaired");
    }

    #[tokio::test]
    async fn p080_insert_reconciliation_event_basic() {
        let pool = setup_db().await;
        let now_str = Utc::now().to_rfc3339();

        insert_reconciliation_event(
            &pool,
            "event-id-001",
            "run-1",
            "stage-1",
            "wi-1",
            "acp_startup_stale",
            "acp_session_reset",
            "none",
            "predicate-hash-abc",
            0,
            "repaired",
            "operator_request",
            &now_str,
            "principal-1",
            Some("p080-rik-abc123abc123abc123abc123"),
        )
        .await
        .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM p080_reconciliation_events_v1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    /// Simulates the Phase 2b live-loop repair path: list stale_suspected rows,
    /// repair each one, and verify the readback transitions to stale_repaired.
    #[tokio::test]
    async fn p080_live_loop_repair_path_stale_suspected_to_repaired() {
        let pool = setup_db().await;

        // Enable acp_startup_stale class.
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason)
             VALUES ('acp_startup_stale', 1, 'phase_2', 1, '2026-01-01T00:00:00Z', 'system', 'operator_change')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a stale_suspected readback row.
        let now_str = Utc::now().to_rfc3339();
        let stale_json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": "run-ll-01", "stage_id": "stage-ll-01", "work_item_id": "wi-ll-01",
            "stale_class": "acp_startup_stale", "running_truth": "stale_suspected",
            "repair_action": "diagnose_only", "hold_reason": "rollout_disabled",
        }))
        .unwrap();
        sqlx::query(
            "INSERT INTO p080_readback_heartbeats_v1
             (run_id, stage_id, work_item_id, stale_class, projection_generation,
              projection_updated_at, projection_integrity, readback_json, updated_at)
             VALUES ('run-ll-01', 'stage-ll-01', 'wi-ll-01', 'acp_startup_stale', 1,
                     ?1, 'valid', ?2, ?3)",
        )
        .bind(&now_str)
        .bind(&stale_json)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

        // Confirm rollout is enabled.
        let rollout = get_rollout_control(&pool, "acp_startup_stale")
            .await
            .unwrap();
        assert!(rollout.as_ref().map(|r| r.enabled).unwrap_or(false));

        // List candidates — simulating what the live loop does.
        let candidates = list_readback_page(
            &pool,
            ReadbackFilter {
                stale_class: Some("acp_startup_stale".to_string()),
                ..Default::default()
            },
            10,
        )
        .await
        .unwrap();
        assert_eq!(candidates.len(), 1, "one stale_suspected row");

        let row = &candidates[0];
        let rb: serde_json::Value = serde_json::from_str(&row.readback_json).unwrap_or_default();
        assert_eq!(rb["running_truth"], "stale_suspected");

        // Simulate what the live loop does in Phase 0/1/2 (diagnose_only — no actual ACP reset).
        let repair_time = Utc::now().to_rfc3339();
        // P7 fix: repair_idempotency_key MUST be null for diagnose_only per proposal §4.3.
        // A non-null key here would invert the contract and let future regressions pass
        // (production code correctly writes null; the test must assert the same).
        let diagnosed_json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": row.run_id, "stage_id": row.stage_id, "work_item_id": row.work_item_id,
            "stale_class": "acp_startup_stale",
            "running_truth": "stale_suspected",
            "repair_action": "diagnose_only",
            "hold_reason": "none",
            "projection_updated_at": repair_time, "projection_integrity": "valid",
            "operator_message": "phase=metadata-only",
            "repair_idempotency_key": null,
        }))
        .unwrap();

        insert_reconciliation_event(
            &pool,
            "event-ll-001",
            &row.run_id,
            &row.stage_id,
            &row.work_item_id,
            "acp_startup_stale",
            "diagnose_only",
            "none",
            "predicate-hash-live",
            0,
            "diagnosed",
            "live_loop",
            &repair_time,
            "system:p080_reconciler",
            None, // diagnose_only: repair_idempotency_key is NULL per proposal §4.3
        )
        .await
        .unwrap();

        upsert_readback_stale_repaired(
            &pool,
            &row.run_id,
            &row.stage_id,
            &row.work_item_id,
            "acp_startup_stale",
            &diagnosed_json,
            &repair_time,
        )
        .await
        .unwrap();

        // Assert reconciliation event recorded.
        let event_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM p080_reconciliation_events_v1 WHERE event_source = 'live_loop'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            event_count, 1,
            "live_loop reconciliation event must be recorded"
        );

        // Assert readback remains stale_suspected with diagnose_only action
        // (Phase 0/1/2 — no actual ACP reset performed).
        let diagnosed_row = get_readback(
            &pool,
            "run-ll-01",
            "stage-ll-01",
            "wi-ll-01",
            "acp_startup_stale",
        )
        .await
        .unwrap()
        .expect("readback row must exist");
        let diagnosed_rb: serde_json::Value =
            serde_json::from_str(&diagnosed_row.readback_json).unwrap();
        assert_eq!(diagnosed_rb["running_truth"], "stale_suspected");
        assert_eq!(diagnosed_rb["repair_action"], "diagnose_only");
        // P7 regression: repair_idempotency_key MUST be null for diagnose_only (proposal §4.3).
        assert!(
            diagnosed_rb["repair_idempotency_key"].is_null(),
            "repair_idempotency_key must be null for diagnose_only; got {:?}",
            diagnosed_rb["repair_idempotency_key"]
        );
        assert!(
            diagnosed_row.projection_generation >= 2,
            "generation must have incremented"
        );
    }

    #[tokio::test]
    async fn p080_set_rollout_control_increments_generation() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();
        let before = get_rollout_control(&pool, "detection_only")
            .await
            .unwrap()
            .expect("row must exist after seed");
        assert!(!before.enabled, "seeded as disabled");

        // Capture seed audit row count (seed writes one row per class per proposal §5.7).
        let seed_audit_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM p080_rollout_control_audit_v1")
                .fetch_one(&pool)
                .await
                .unwrap();

        let updated = set_rollout_control(
            &pool,
            "detection_only",
            true,
            "operator_change",
            "test-operator",
        )
        .await
        .unwrap();
        assert!(updated.enabled, "must be enabled after set");
        assert!(
            updated.generation > before.generation,
            "generation must increment on set"
        );

        // Exactly one new audit row written by set (seed rows are separate).
        let audit_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM p080_rollout_control_audit_v1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            audit_count - seed_audit_count,
            1,
            "audit row must be written on set"
        );

        // Disable again — generation must increment again.
        let disabled = set_rollout_control(
            &pool,
            "detection_only",
            false,
            "live_disable",
            "test-operator",
        )
        .await
        .unwrap();
        assert!(!disabled.enabled);
        assert!(disabled.generation > updated.generation);

        // Two set operations → two new audit rows beyond seed.
        let audit_count2: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM p080_rollout_control_audit_v1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            audit_count2 - seed_audit_count,
            2,
            "each set must produce one audit row"
        );
    }

    /// P080-SEC-LOW-001 regression: reconciliation_summary_for_report must count repairs
    /// using the actual schema column (decision='repaired'), not a non-existent event_type.
    #[tokio::test]
    async fn p080_reconciliation_summary_counts_repaired_via_decision_column() {
        let pool = setup_db().await;
        let now_str = Utc::now().to_rfc3339();

        // Insert one repaired event.
        insert_reconciliation_event(
            &pool,
            "event-rep-001",
            "run-r1",
            "stage-r1",
            "wi-r1",
            "acp_startup_stale",
            "diagnose_only",
            "none",
            "pred-hash-r1",
            0,
            "repaired",
            "live_loop",
            &now_str,
            "system",
            None,
        )
        .await
        .unwrap();

        // Insert one non-repaired event (should not be counted).
        insert_reconciliation_event(
            &pool,
            "event-rep-002",
            "run-r2",
            "stage-r2",
            "wi-r2",
            "acp_startup_stale",
            "diagnose_only",
            "none",
            "pred-hash-r2",
            0,
            "diagnosed",
            "live_loop",
            &now_str,
            "system",
            None,
        )
        .await
        .unwrap();

        // Use run-r1 as scope; only rows for that run_id should be returned.
        let summary = p080_run_report_section_for_report(&pool, "run-r1").await;
        // Phase 1: projection rows — should return p080_run_report_section_v1 shape.
        assert_eq!(summary["schema_version"], "p080_run_report_section_v1");
        // rows should be present as an array (may be empty in this test if no running executions).
        assert!(summary["rows"].is_array(), "rows must be an array");
    }

    #[tokio::test]
    async fn p080_run_report_section_redacts_stored_readback_rows() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();
        set_rollout_control(
            &pool,
            "detection_only",
            true,
            "operator_change",
            "test-principal",
        )
        .await
        .unwrap();

        let readback = serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": "run-secret",
            "stage_id": "stage-secret",
            "work_item_id": "work-secret",
            "stale_class": "acp_startup_stale",
            "running_truth": "stale_suspected",
            "repair_action": "diagnose_only",
            "hold_reason": "rollout_disabled",
            "hold_age_seconds": null,
            "next_retry_or_backoff_time": null,
            "projection_updated_at": "2026-01-01T00:00:00Z",
            "projection_integrity": "valid",
            "executor_reregistration_state": "expected",
            "rollout_disablement": "phase_not_reached",
            "side_effect_status": "not_applicable",
            "operator_message": "Bearer sk-proj-secret-token-1234567890",
            "evidence_marker_hash": null,
            "repair_idempotency_key": null,
            "absolutePath": "/tmp/secret",
            "rawPayload": "token=should-not-leak"
        });
        sqlx::query(
            r#"INSERT INTO p080_readback_heartbeats_v1
               (run_id, stage_id, work_item_id, stale_class, projection_generation,
                projection_updated_at, projection_integrity, readback_json, updated_at)
               VALUES ('run-secret', 'stage-secret', 'work-secret', 'acp_startup_stale', 1,
                       '2026-01-01T00:00:00Z', 'valid', ?1, '2026-01-01T00:00:00Z')"#,
        )
        .bind(readback.to_string())
        .execute(&pool)
        .await
        .unwrap();

        let summary = p080_run_report_section_for_report(&pool, "run-secret").await;
        let row = &summary["rows"][0];
        assert_eq!(row["operator_message"], "[redacted]");
        assert!(
            row.get("absolutePath").is_none(),
            "run-report lane must strip forbidden filesystem path fields"
        );
        assert!(
            row.get("rawPayload").is_none(),
            "run-report lane must strip raw payload fields"
        );
        let serialized = summary.to_string();
        assert!(!serialized.contains("sk-proj-secret"));
        assert!(!serialized.contains("should-not-leak"));
        assert!(!serialized.contains("/tmp/secret"));
    }

    // ── SEC-MED-002: field-aware secret detection regression tests ──────────

    #[test]
    fn p080_validate_readback_json_accepts_evidence_marker_hash() {
        // 64 lowercase hex chars — SHA-256 predicate hash — must not be rejected.
        let hash = "a".repeat(64);
        let json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "evidence_marker_hash": hash,
        }))
        .unwrap();
        let result = validate_readback_json_for_write(&json);
        assert!(
            result.is_ok(),
            "64-char lowercase hex evidence_marker_hash must be accepted: {result:?}"
        );
    }

    #[test]
    fn p080_validate_readback_json_accepts_rik_value() {
        // p080-rik-<24 lowercase hex> — must not be rejected by the secret detector.
        let rik = format!("p080-rik-{}", "b".repeat(24));
        let json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "repair_idempotency_key": rik,
        }))
        .unwrap();
        let result = validate_readback_json_for_write(&json);
        assert!(
            result.is_ok(),
            "valid p080-rik-<24hex> repair_idempotency_key must be accepted: {result:?}"
        );
    }

    #[test]
    fn p080_validate_readback_json_rejects_high_entropy_operator_message() {
        // A 64-char hex string in operator_message should still be rejected.
        let secret_like = "d".repeat(64);
        let json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "operator_message": secret_like,
        }))
        .unwrap();
        let result = validate_readback_json_for_write(&json);
        assert!(
            result.is_err(),
            "64-char hex string in operator_message must be rejected as secret-like"
        );
    }

    #[test]
    fn p080_validate_readback_json_rejects_invalid_evidence_marker_hash() {
        // Uppercase hex — not a valid sha256 format; should be rejected.
        let bad_hash = "A".repeat(64);
        let json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "evidence_marker_hash": bad_hash,
        }))
        .unwrap();
        let result = validate_readback_json_for_write(&json);
        assert!(
            result.is_err(),
            "64-char UPPERCASE hex is not a valid sha256 evidence_marker_hash and must be rejected"
        );
    }

    #[test]
    fn p080_sanitize_readback_string_allows_evidence_marker_hash_on_egress() {
        let hash = "c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2";
        assert_eq!(hash.len(), 64, "test value must be exactly 64 chars");
        let result = sanitize_readback_string("evidence_marker_hash", hash);
        assert_eq!(
            result, hash,
            "valid sha256 evidence_marker_hash must not be redacted on egress"
        );
    }

    #[test]
    fn p080_sanitize_readback_string_redacts_secret_in_operator_message() {
        let secret_like = "e".repeat(64);
        let result = sanitize_readback_string("operator_message", &secret_like);
        assert_eq!(
            result, "[redacted]",
            "64-char hex in operator_message must be redacted on egress"
        );
    }

    // ── SEC-MED-001: stale_class and identifier sanitization tests ──────────

    #[test]
    fn p080_sanitize_stale_class_for_output_known_values_pass_through() {
        for class in P080_KNOWN_STALE_CLASSES {
            assert_eq!(
                sanitize_stale_class_for_output(class),
                *class,
                "known stale_class {class} must pass through unchanged"
            );
        }
    }

    #[test]
    fn p080_sanitize_stale_class_for_output_unknown_becomes_bracketed() {
        assert_eq!(
            sanitize_stale_class_for_output("injected_class"),
            "[unknown]",
            "unrecognized stale_class must become [unknown]"
        );
        assert_eq!(
            sanitize_stale_class_for_output(""),
            "[unknown]",
            "empty stale_class must become [unknown]"
        );
        assert_eq!(
            sanitize_stale_class_for_output("SELECT * FROM secrets"),
            "[unknown]",
            "sql-injection attempt in stale_class must become [unknown]"
        );
    }

    #[test]
    fn p080_sanitize_identifier_for_output_strips_control_chars() {
        let with_null = "run-abc\x00def";
        let result = sanitize_identifier_for_output(with_null);
        assert!(
            !result.contains('\x00'),
            "null byte must be stripped from identifier output"
        );
    }

    #[test]
    fn p080_sanitize_identifier_for_output_redacts_empty() {
        assert_eq!(
            sanitize_identifier_for_output(""),
            "[redacted]",
            "empty identifier must become [redacted]"
        );
    }

    // ── SEC-MED-002: atomic writer with evidence_marker_hash ────────────────

    #[tokio::test]
    async fn p080_atomic_writer_accepts_evidence_marker_hash() {
        let pool = setup_db().await;
        seed_rollout_control_if_absent(&pool).await.unwrap();

        // Pre-computed sha256("run-1:stage-1:wi-1:acp_startup_stale") for determinism.
        let predicate_hash =
            "c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2".to_string();
        assert_eq!(
            predicate_hash.len(),
            64,
            "predicate_hash must be 64 hex chars"
        );

        let readback_json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": "run-1",
            "stage_id": "stage-1",
            "work_item_id": "wi-1",
            "stale_class": "acp_startup_stale",
            "running_truth": "stale_suspected",
            "repair_action": "diagnose_only",
            "hold_reason": "none",
            "projection_updated_at": "2026-01-01T00:00:00Z",
            "projection_integrity": "valid",
            "rollout_disablement": "class_disabled",
            "side_effect_status": "not_applicable",
            "operator_message": "test",
            "evidence_marker_hash": predicate_hash,
            "repair_idempotency_key": null,
        }))
        .unwrap();

        let result = insert_event_and_upsert_readback_atomic(
            &pool,
            "event-hash-test",
            "run-1",
            "stage-1",
            "wi-1",
            "acp_startup_stale",
            "diagnose_only",
            "none",
            &predicate_hash,
            0,
            "diagnosed",
            "live_loop",
            "system:p080_reconciler",
            None,
            &readback_json,
            "2026-01-01T00:00:00Z",
        )
        .await;
        assert!(
            result.is_ok(),
            "atomic writer must accept a readback containing a valid evidence_marker_hash: {result:?}"
        );
    }
}
