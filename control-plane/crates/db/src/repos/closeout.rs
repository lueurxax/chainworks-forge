// P077: State-9 closeout repository helper.
//
// R14 §architecture.state_9_transaction_api:
//   "Add a small state-9 closeout repository helper that activates gate and readiness
//    generations, persists summary rows, rebuilds projections once, commits, and only
//    then returns data to transition evaluation."
//
// Crash semantics:
//   - Crash before commit leaves previous active truth authoritative.
//   - Crash after commit exposes a coherent gate/readiness pair; projection
//     rebuild can be retried from active truth.
//
// Invariant: no transition is evaluated between gate activation and readiness activation.

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use domain::closeout_readiness::{
    CloseoutReadiness, CloseoutReadinessDecision, CloseoutReadinessStatus,
    IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID,
};
use domain::proposal_gate_result::{ProposalGateResult, PROPOSAL_GATE_RESULT_V1_CONTRACT_ID};

use crate::pool::begin_immediate_with_retry;

/// Inputs for the closeout transaction.
pub struct CloseoutTransactionInputs<'a> {
    pub gate_result: &'a ProposalGateResult,
    pub readiness: &'a CloseoutReadiness,
    /// P077 BLK-011: persisted on the readiness generation row so the next
    /// synthesis can pass it as `previous_blocker_digest` and detect soft
    /// convergence. None means the assessment was unavailable at synth time.
    pub blocker_digest: Option<&'a str>,
}

/// Result returned to transition evaluation after the transaction commits.
/// Transition evaluation MUST only proceed after this is returned.
pub struct CloseoutTransactionResult {
    pub gate_generation_id: String,
    pub readiness_generation_id: String,
    pub readiness_status: CloseoutReadinessStatus,
    pub readiness_decision: CloseoutReadinessDecision,
}

/// Execute the state-9 closeout transaction atomically.
///
/// Per R14: Activates gate + readiness generations in one transaction.
/// No transition is evaluated between gate activation and readiness activation.
/// Returns CloseoutTransactionResult only after commit succeeds.
pub async fn execute_closeout_transaction(
    pool: &SqlitePool,
    inputs: CloseoutTransactionInputs<'_>,
) -> Result<CloseoutTransactionResult> {
    let mut tx = begin_immediate_with_retry(pool, "closeout.execute_closeout_transaction").await?;

    // Step 1: Deactivate any previous active gate generation for this run.
    deactivate_previous_gate_generation(&mut tx, &inputs.gate_result.run_id).await?;

    // Step 2: Insert the new gate generation record.
    let gate_row_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO closeout_gate_generations
            (id, run_id, stage_id, contract_id, status, decision, generation_id,
             readiness_mode, diagnostic_reason, primary_unblock, code_blocker_count,
             handoff_owner, risk_settlement_required, fingerprint_json,
             active, superseded_by_generation_id, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL, ?7, NULL, 0, NULL, 0, NULL, 1, NULL, ?8, ?8)
        "#,
    )
    .bind(&gate_row_id)
    .bind(&inputs.gate_result.run_id)
    .bind(&inputs.gate_result.stage_id)
    .bind(PROPOSAL_GATE_RESULT_V1_CONTRACT_ID)
    .bind(inputs.gate_result.status.as_str())
    .bind(&inputs.gate_result.generation_id)
    .bind(&inputs.gate_result.diagnostic_reason)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .context("insert closeout gate generation")?;

    // Step 3: Deactivate any previous active readiness generation for this run.
    deactivate_previous_readiness_generation(&mut tx, &inputs.readiness.run_id).await?;

    // Step 4: Insert the new readiness generation record.
    let readiness_row_id = Uuid::new_v4().to_string();
    let fingerprint_json = inputs
        .readiness
        .fingerprint
        .as_ref()
        .and_then(|fp| serde_json::to_string(fp).ok());

    sqlx::query(
        r#"
        INSERT INTO closeout_gate_generations
            (id, run_id, stage_id, contract_id, status, decision, generation_id,
             readiness_mode, diagnostic_reason, primary_unblock, code_blocker_count,
             handoff_owner, risk_settlement_required, fingerprint_json, blocker_digest,
             active, superseded_by_generation_id, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 1, NULL, ?16, ?16)
        "#,
    )
    .bind(&readiness_row_id)
    .bind(&inputs.readiness.run_id)
    .bind(&inputs.readiness.stage_id)
    .bind(IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID)
    .bind(inputs.readiness.status.as_str())
    .bind(inputs.readiness.decision.as_str())
    .bind(&inputs.readiness.generation_id)
    .bind(&inputs.readiness.readiness_mode)
    .bind(&inputs.readiness.diagnostic_reason)
    .bind(&inputs.readiness.primary_unblock)
    .bind(inputs.readiness.code_blocker_count as i64)
    .bind(&inputs.readiness.handoff_owner)
    .bind(inputs.readiness.risk_settlement_required as i64)
    .bind(&fingerprint_json)
    .bind(inputs.blocker_digest)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .context("insert closeout readiness generation")?;

    // Commit: crash before this leaves prior active truth authoritative.
    // Crash after commit exposes a coherent gate/readiness pair.
    tx.commit()
        .await
        .context("commit closeout transaction")?;

    // Return data to transition evaluation ONLY after commit.
    Ok(CloseoutTransactionResult {
        gate_generation_id: inputs.gate_result.generation_id.clone(),
        readiness_generation_id: inputs.readiness.generation_id.clone(),
        readiness_status: inputs.readiness.status.clone(),
        readiness_decision: inputs.readiness.decision.clone(),
    })
}

/// Read the active gate generation for a run (if any).
pub async fn find_active_gate_generation(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<ActiveGateRow>> {
    let row = sqlx::query(
        r#"
        SELECT status, generation_id, diagnostic_reason, created_at
        FROM closeout_gate_generations
        WHERE run_id = ?1 AND contract_id = ?2 AND active = 1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(PROPOSAL_GATE_RESULT_V1_CONTRACT_ID)
    .fetch_optional(pool)
    .await
    .context("find active gate generation")?;

    row.map(|r| {
        Ok(ActiveGateRow {
            status: r.try_get::<String, _>("status").context("gate status")?,
            generation_id: r
                .try_get::<String, _>("generation_id")
                .context("gate generation_id")?,
            diagnostic_reason: r
                .try_get::<Option<String>, _>("diagnostic_reason")
                .context("gate diagnostic_reason")?,
        })
    })
    .transpose()
}

/// Read the active readiness generation for a run (if any).
pub async fn find_active_readiness_generation(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<ActiveReadinessRow>> {
    let row = sqlx::query(
        r#"
        SELECT status, decision, generation_id, readiness_mode,
               diagnostic_reason, primary_unblock, code_blocker_count,
               handoff_owner, risk_settlement_required, fingerprint_json
        FROM closeout_gate_generations
        WHERE run_id = ?1 AND contract_id = ?2 AND active = 1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(run_id)
    .bind(IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID)
    .fetch_optional(pool)
    .await
    .context("find active readiness generation")?;

    row.map(|r| {
        Ok(ActiveReadinessRow {
            status: r
                .try_get::<String, _>("status")
                .context("readiness status")?,
            decision: r
                .try_get::<String, _>("decision")
                .context("readiness decision")?,
            generation_id: r
                .try_get::<String, _>("generation_id")
                .context("readiness generation_id")?,
            readiness_mode: r
                .try_get::<Option<String>, _>("readiness_mode")
                .context("readiness_mode")?,
            diagnostic_reason: r
                .try_get::<Option<String>, _>("diagnostic_reason")
                .context("readiness diagnostic_reason")?,
            primary_unblock: r
                .try_get::<Option<String>, _>("primary_unblock")
                .context("readiness primary_unblock")?,
            code_blocker_count: r
                .try_get::<i64, _>("code_blocker_count")
                .context("readiness code_blocker_count")? as u32,
            handoff_owner: r
                .try_get::<Option<String>, _>("handoff_owner")
                .context("readiness handoff_owner")?,
            risk_settlement_required: r
                .try_get::<i64, _>("risk_settlement_required")
                .context("readiness risk_settlement_required")?
                != 0,
        })
    })
    .transpose()
}

/// P077 BLK-011: Read the blocker_digest persisted on the most recent active
/// implementation_closeout_readiness_v1 generation. Used as
/// `previous_blocker_digest` on the next state-9 synthesis to detect
/// soft-convergence (repeated identical blockers without diff or gate progress).
pub async fn find_active_blocker_digest(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<String>> {
    let row: Option<Option<String>> = sqlx::query_scalar(
        r#"SELECT blocker_digest
           FROM closeout_gate_generations
           WHERE run_id = ?1 AND contract_id = ?2 AND active = 1
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(run_id)
    .bind(IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID)
    .fetch_optional(pool)
    .await
    .context("find_active_blocker_digest")?;
    Ok(row.flatten())
}

/// Read the closeout_readiness_mode for a run from the frozen column.
pub async fn find_closeout_readiness_mode(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<String>> {
    let row = sqlx::query_scalar(
        "SELECT closeout_readiness_mode FROM runs WHERE id = ?1",
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .context("find closeout_readiness_mode")?;
    Ok(row.flatten())
}

/// Check for an enforcement-migration override record for a run.
pub async fn has_enforcement_migration_record(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM closeout_readiness_mode_overrides WHERE run_id = ?1 AND mode = 'enforcement'",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .context("check enforcement migration record")?;
    Ok(count > 0)
}

/// P077 BLK-006: Compute controlled_reports_green from active artifact-contract truth.
///
/// Returns:
///   * `Some(true)` when audit, docs, security, prepush, and tests are all present
///     and canonically green (audit=implemented, docs=pass|not_needed,
///     security=pass, prepush=pass, tests=green).
///   * `Some(false)` when any of those reports is present but not green
///     (definitive block diagnostic).
///   * `None` when one or more reports has no active generation yet — the
///     synthesizer treats this as "not yet wired" and fails closed in
///     enforcement mode while leaving advisory mode unaffected.
pub async fn compute_controlled_reports_green(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<bool>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT g.contract_id, g.canonical_status
           FROM active_artifact_contracts a
           JOIN artifact_contract_generations g ON g.generation_id = a.generation_id
           WHERE a.run_id = ?1 AND g.contract_id IN (
               'audit_report_v1', 'docs_report_v1', 'security_report_v1',
               'prepush_review_v1', 'tests_result_v1'
           )"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("compute_controlled_reports_green: load active controlled reports")?;

    let mut found = std::collections::HashMap::new();
    for (cid, status) in rows {
        found.insert(cid, status);
    }

    let required = [
        "audit_report_v1",
        "docs_report_v1",
        "security_report_v1",
        "prepush_review_v1",
        "tests_result_v1",
    ];

    let mut any_missing = false;
    let mut any_not_green = false;
    for contract_id in required {
        match found.get(contract_id) {
            None => any_missing = true,
            Some(status) if !is_canonical_status_green(contract_id, status) => {
                any_not_green = true;
            }
            Some(_) => {}
        }
    }

    if any_not_green {
        Ok(Some(false))
    } else if any_missing {
        Ok(None)
    } else {
        Ok(Some(true))
    }
}

fn is_canonical_status_green(contract_id: &str, canonical_status: &str) -> bool {
    match contract_id {
        "audit_report_v1" => canonical_status == "implemented",
        "docs_report_v1" => canonical_status == "pass" || canonical_status == "not_needed",
        "security_report_v1" => canonical_status == "pass",
        "prepush_review_v1" => canonical_status == "pass",
        "tests_result_v1" => canonical_status == "green",
        _ => false,
    }
}

async fn deactivate_previous_gate_generation(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE closeout_gate_generations
        SET active = 0, updated_at = ?1
        WHERE run_id = ?2 AND contract_id = ?3 AND active = 1
        "#,
    )
    .bind(Utc::now().to_rfc3339())
    .bind(run_id)
    .bind(PROPOSAL_GATE_RESULT_V1_CONTRACT_ID)
    .execute(&mut **tx)
    .await
    .context("deactivate previous gate generation")?;
    Ok(())
}

async fn deactivate_previous_readiness_generation(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE closeout_gate_generations
        SET active = 0, updated_at = ?1
        WHERE run_id = ?2 AND contract_id = ?3 AND active = 1
        "#,
    )
    .bind(Utc::now().to_rfc3339())
    .bind(run_id)
    .bind(IMPLEMENTATION_CLOSEOUT_READINESS_V1_CONTRACT_ID)
    .execute(&mut **tx)
    .await
    .context("deactivate previous readiness generation")?;
    Ok(())
}

#[derive(Debug)]
pub struct ActiveGateRow {
    pub status: String,
    pub generation_id: String,
    pub diagnostic_reason: Option<String>,
}

#[derive(Debug)]
pub struct ActiveReadinessRow {
    pub status: String,
    pub decision: String,
    pub generation_id: String,
    pub readiness_mode: Option<String>,
    pub diagnostic_reason: Option<String>,
    pub primary_unblock: Option<String>,
    pub code_blocker_count: u32,
    pub handoff_owner: Option<String>,
    pub risk_settlement_required: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::closeout_readiness::{CloseoutReadiness, CloseoutReadinessDecision, CloseoutReadinessStatus};
    use domain::proposal_gate_result::{ProposalGateResult, ProposalGateStatus};

    async fn setup_test_db() -> SqlitePool {
        crate::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    fn make_gate(run_id: &str, status: ProposalGateStatus) -> ProposalGateResult {
        ProposalGateResult {
            gate_id: "p077:077".into(),
            proposal_id: "077".into(),
            run_id: run_id.into(),
            stage_id: "state_9".into(),
            status,
            generation_id: format!("gate-gen-{}", Uuid::new_v4()),
            diagnostic_reason: None,
            executor_version: Some("p077-v1".into()),
            evidence_digest: None,
            exit_code: Some(0),
            elapsed_ms: Some(1234),
            settled_at: Utc::now(),
            authorization_lineage: None,
            failure_classification: None,
        }
    }

    fn make_readiness(run_id: &str, status: CloseoutReadinessStatus, decision: CloseoutReadinessDecision) -> CloseoutReadiness {
        CloseoutReadiness {
            run_id: run_id.into(),
            stage_id: "state_9".into(),
            status,
            decision,
            generation_id: format!("readiness-gen-{}", Uuid::new_v4()),
            readiness_mode: "advisory".into(),
            diagnostic_reason: None,
            primary_unblock: None,
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            fingerprint: None,
            synthesized_at: Utc::now(),
        }
    }

    async fn insert_test_run(pool: &SqlitePool) -> String {
        use domain::idea::{Idea, IdeaStatus};
        use domain::ids::IdeaId;
        use domain::run::{Run, RunStatus};

        let idea = Idea {
            id: IdeaId::new(),
            title: "Test Idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        };
        crate::repos::ideas::insert(pool, &idea).await.unwrap();

        let run = Run {
            id: domain::ids::RunId::new(),
            idea_id: idea.id,
            status: RunStatus::Running,
            workflow_id: "wf-test".into(),
            workflow_title: "Test Workflow".into(),
            workspace_root: "/workspace".into(),
            artifact_root: "/artifacts".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_9".into()),
            workflow_yaml_path: Some("workflow.yaml".into()),
            agent_catalog_yaml_path: Some("agents.yaml".into()),
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
        };
        let run_id = run.id.to_string();
        crate::repos::runs::insert(pool, &run).await.unwrap();
        run_id
    }

    #[tokio::test]
    async fn closeout_transaction_commits_gate_and_readiness_atomically() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        let gate = make_gate(&run_id, ProposalGateStatus::Passed);
        let readiness = make_readiness(
            &run_id,
            CloseoutReadinessStatus::Ready,
            CloseoutReadinessDecision::EnterManualRelease,
        );

        let result = execute_closeout_transaction(
            &pool,
            CloseoutTransactionInputs {
                gate_result: &gate,
                readiness: &readiness,
                blocker_digest: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.readiness_status, CloseoutReadinessStatus::Ready);
        assert_eq!(result.readiness_decision, CloseoutReadinessDecision::EnterManualRelease);

        let active_gate = find_active_gate_generation(&pool, &run_id).await.unwrap();
        assert!(active_gate.is_some());
        assert_eq!(active_gate.unwrap().status, "passed");

        let active_readiness = find_active_readiness_generation(&pool, &run_id).await.unwrap();
        assert!(active_readiness.is_some());
        let active_readiness = active_readiness.unwrap();
        assert_eq!(active_readiness.status, "ready");
        assert_eq!(active_readiness.decision, "enter_manual_release");
    }

    #[tokio::test]
    async fn second_closeout_transaction_supersedes_first() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        let gate1 = make_gate(&run_id, ProposalGateStatus::Failed);
        let readiness1 = make_readiness(
            &run_id,
            CloseoutReadinessStatus::NotReady,
            CloseoutReadinessDecision::ReturnToCodeRefine,
        );
        execute_closeout_transaction(
            &pool,
            CloseoutTransactionInputs {
                gate_result: &gate1,
                readiness: &readiness1,
                blocker_digest: Some("digest-attempt-1"),
            },
        )
        .await
        .unwrap();

        let gate2 = make_gate(&run_id, ProposalGateStatus::Passed);
        let readiness2 = make_readiness(
            &run_id,
            CloseoutReadinessStatus::Ready,
            CloseoutReadinessDecision::EnterManualRelease,
        );
        execute_closeout_transaction(
            &pool,
            CloseoutTransactionInputs {
                gate_result: &gate2,
                readiness: &readiness2,
                blocker_digest: Some("digest-attempt-2"),
            },
        )
        .await
        .unwrap();

        let active_gate = find_active_gate_generation(&pool, &run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active_gate.status, "passed");

        let active_readiness = find_active_readiness_generation(&pool, &run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(active_readiness.status, "ready");
    }

    #[tokio::test]
    async fn no_active_generation_returns_none() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        let gate = find_active_gate_generation(&pool, &run_id).await.unwrap();
        assert!(gate.is_none(), "no gate generation should exist yet");

        let readiness = find_active_readiness_generation(&pool, &run_id).await.unwrap();
        assert!(readiness.is_none(), "no readiness generation should exist yet");
    }

    #[tokio::test]
    async fn find_closeout_readiness_mode_returns_none_for_null_column() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        let mode = find_closeout_readiness_mode(&pool, &run_id).await.unwrap();
        assert!(mode.is_none(), "mode column should be NULL for new runs");
    }

    #[tokio::test]
    async fn closeout_readiness_mode_can_be_set_on_run() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        sqlx::query("UPDATE runs SET closeout_readiness_mode = 'enforcement' WHERE id = ?1")
            .bind(&run_id)
            .execute(&pool)
            .await
            .unwrap();

        let mode = find_closeout_readiness_mode(&pool, &run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(mode, "enforcement");
    }

    /// P077 BLK-006: helpers for inserting active artifact-contract generations
    /// in tests. Bypasses the upsert API and writes directly so tests can
    /// freely fixture the `compute_controlled_reports_green` matrix.
    async fn insert_active_contract(
        pool: &SqlitePool,
        run_id: &str,
        contract_id: &str,
        canonical_status: &str,
    ) {
        let generation_id = format!("gen-{}", Uuid::new_v4());
        let artifact_id = format!("art-{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO artifact_contract_generations
                (generation_id, run_id, artifact_id, contract_id, canonical_path, raw_path,
                 raw_status, canonical_status, valid, partial, warnings_json, validation_errors_json,
                 created_at)
               VALUES (?1, ?2, ?3, ?4, '/canonical', '/raw', ?5, ?5, 1, 0, '[]', '[]', ?6)"#,
        )
        .bind(&generation_id)
        .bind(run_id)
        .bind(&artifact_id)
        .bind(contract_id)
        .bind(canonical_status)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            r#"INSERT INTO active_artifact_contracts (run_id, contract_id, generation_id, updated_at)
               VALUES (?1, ?2, ?3, ?4)
               ON CONFLICT(run_id, contract_id) DO UPDATE SET
                 generation_id = excluded.generation_id,
                 updated_at = excluded.updated_at"#,
        )
        .bind(run_id)
        .bind(contract_id)
        .bind(&generation_id)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn controlled_reports_green_is_none_when_all_missing() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        let result = compute_controlled_reports_green(&pool, &run_id).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn controlled_reports_green_is_some_true_when_all_green() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        insert_active_contract(&pool, &run_id, "audit_report_v1", "implemented").await;
        insert_active_contract(&pool, &run_id, "docs_report_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "security_report_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "prepush_review_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "tests_result_v1", "green").await;

        let result = compute_controlled_reports_green(&pool, &run_id).await.unwrap();
        assert_eq!(result, Some(true));
    }

    #[tokio::test]
    async fn controlled_reports_green_treats_docs_not_needed_as_green() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        insert_active_contract(&pool, &run_id, "audit_report_v1", "implemented").await;
        insert_active_contract(&pool, &run_id, "docs_report_v1", "not_needed").await;
        insert_active_contract(&pool, &run_id, "security_report_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "prepush_review_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "tests_result_v1", "green").await;

        let result = compute_controlled_reports_green(&pool, &run_id).await.unwrap();
        assert_eq!(result, Some(true));
    }

    #[tokio::test]
    async fn controlled_reports_green_is_some_false_when_any_not_green() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        insert_active_contract(&pool, &run_id, "audit_report_v1", "needs_code_fixes").await;
        insert_active_contract(&pool, &run_id, "docs_report_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "security_report_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "prepush_review_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "tests_result_v1", "green").await;

        let result = compute_controlled_reports_green(&pool, &run_id).await.unwrap();
        assert_eq!(result, Some(false));
    }

    #[tokio::test]
    async fn controlled_reports_green_definitive_block_overrides_missing() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        // tests are red; docs_report missing → block dominates over missing.
        insert_active_contract(&pool, &run_id, "audit_report_v1", "implemented").await;
        insert_active_contract(&pool, &run_id, "security_report_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "prepush_review_v1", "pass").await;
        insert_active_contract(&pool, &run_id, "tests_result_v1", "red").await;

        let result = compute_controlled_reports_green(&pool, &run_id).await.unwrap();
        assert_eq!(result, Some(false));
    }

    #[tokio::test]
    async fn blocker_digest_persists_and_round_trips() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        // No prior generation → None.
        let digest = find_active_blocker_digest(&pool, &run_id).await.unwrap();
        assert_eq!(digest, None);

        // First attempt with a digest.
        let gate1 = make_gate(&run_id, ProposalGateStatus::Failed);
        let readiness1 = make_readiness(
            &run_id,
            CloseoutReadinessStatus::NotReady,
            CloseoutReadinessDecision::ReturnToCodeRefine,
        );
        execute_closeout_transaction(
            &pool,
            CloseoutTransactionInputs {
                gate_result: &gate1,
                readiness: &readiness1,
                blocker_digest: Some("abc123"),
            },
        )
        .await
        .unwrap();

        let digest = find_active_blocker_digest(&pool, &run_id).await.unwrap();
        assert_eq!(digest.as_deref(), Some("abc123"));

        // Second attempt supersedes — only the active row is read.
        let gate2 = make_gate(&run_id, ProposalGateStatus::Failed);
        let readiness2 = make_readiness(
            &run_id,
            CloseoutReadinessStatus::NotReady,
            CloseoutReadinessDecision::AwaitOperatorDecision,
        );
        execute_closeout_transaction(
            &pool,
            CloseoutTransactionInputs {
                gate_result: &gate2,
                readiness: &readiness2,
                blocker_digest: Some("def456"),
            },
        )
        .await
        .unwrap();

        let digest = find_active_blocker_digest(&pool, &run_id).await.unwrap();
        assert_eq!(digest.as_deref(), Some("def456"));
    }

    #[tokio::test]
    async fn controlled_reports_green_is_none_when_partial() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool).await;

        insert_active_contract(&pool, &run_id, "audit_report_v1", "implemented").await;
        insert_active_contract(&pool, &run_id, "docs_report_v1", "pass").await;
        // security, prepush, tests missing.

        let result = compute_controlled_reports_green(&pool, &run_id).await.unwrap();
        assert_eq!(result, None);
    }
}
