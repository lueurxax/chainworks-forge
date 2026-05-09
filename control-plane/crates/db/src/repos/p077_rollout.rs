use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::writer::begin_registered_immediate_transaction;

#[derive(Clone, Debug)]
pub struct P077RolloutMetricEventInput {
    pub metric: String,
    pub run_id: Option<String>,
    pub numerator: i64,
    pub denominator: i64,
    pub threshold: String,
    pub owner: String,
    pub source: String,
    pub go_no_go_action: String,
    pub evidence_json: Value,
}

#[derive(Clone, Debug)]
pub struct P077RolloutDecisionInput {
    pub decision_scope: String,
    pub decision: String,
    pub principal: String,
    pub reason: String,
    pub metric_snapshot_json: Value,
    pub cohort: String,
    pub eligible_closeouts: i64,
    pub primary_metric_values_json: Value,
    pub diagnostic_metric_snapshot_json: Value,
    pub dependency_checklist_snapshot_id: String,
    pub fingerprint_p95_threshold_ms: i64,
    pub measurement_window: String,
    pub waivers_json: Value,
    pub next_review_date: String,
    pub readiness_links_json: Value,
    pub rollback_trigger: Option<String>,
    pub rollback_action: Option<String>,
    pub affected_run_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P077RolloutDecisionResult {
    pub decision_id: String,
    pub advisory_migration_count: usize,
}

pub async fn record_metric_event(
    pool: &SqlitePool,
    input: P077RolloutMetricEventInput,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let recorded_at = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO p077_rollout_metric_events
           (id, metric, run_id, numerator, denominator, threshold, owner, source,
            go_no_go_action, evidence_json, recorded_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
    )
    .bind(&id)
    .bind(&input.metric)
    .bind(&input.run_id)
    .bind(input.numerator)
    .bind(input.denominator)
    .bind(&input.threshold)
    .bind(&input.owner)
    .bind(&input.source)
    .bind(&input.go_no_go_action)
    .bind(serde_json::to_string(&input.evidence_json)?)
    .bind(recorded_at)
    .execute(pool)
    .await
    .context("record p077 rollout metric event")?;
    Ok(id)
}

pub async fn record_decision(
    pool: &SqlitePool,
    input: P077RolloutDecisionInput,
) -> Result<P077RolloutDecisionResult> {
    validate_decision_payload(&input)?;
    if input.decision == "rollback_to_advisory" {
        if input.rollback_trigger.as_deref().unwrap_or("").is_empty() {
            bail!("p077 rollback_to_advisory requires rollback_trigger");
        }
        if input.rollback_action.as_deref().unwrap_or("").is_empty() {
            bail!("p077 rollback_to_advisory requires rollback_action");
        }
        if input.affected_run_ids.is_empty() {
            bail!("p077 rollback_to_advisory requires affected_run_ids");
        }
    } else if !input.affected_run_ids.is_empty() {
        bail!("p077 affected_run_ids are only valid for rollback_to_advisory");
    }

    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "p077_rollout.record_decision",
            crate::write_class::WriteLane::CriticalBarrier,
            "p077_rollout.record_decision",
        ),
        "p077_rollout.record_decision",
    )
    .await?;
    let decision_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    sqlx::query(
        r#"INSERT INTO p077_rollout_decisions
           (id, decision_scope, decision, principal, reason, metric_snapshot_json,
            decision_type, cohort, eligible_closeouts, primary_metric_values_json,
            diagnostic_metric_snapshot_json, dependency_checklist_snapshot_id,
            fingerprint_p95_threshold_ms, measurement_window, waivers_json,
            next_review_date, readiness_links_json, rollback_trigger, rollback_action,
            created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                   ?14, ?15, ?16, ?17, ?18, ?19, ?20)"#,
    )
    .bind(&decision_id)
    .bind(&input.decision_scope)
    .bind(&input.decision)
    .bind(&input.principal)
    .bind(&input.reason)
    .bind(serde_json::to_string(&input.metric_snapshot_json)?)
    .bind(&input.decision)
    .bind(&input.cohort)
    .bind(input.eligible_closeouts)
    .bind(serde_json::to_string(&input.primary_metric_values_json)?)
    .bind(serde_json::to_string(
        &input.diagnostic_metric_snapshot_json,
    )?)
    .bind(&input.dependency_checklist_snapshot_id)
    .bind(input.fingerprint_p95_threshold_ms)
    .bind(&input.measurement_window)
    .bind(serde_json::to_string(&input.waivers_json)?)
    .bind(&input.next_review_date)
    .bind(serde_json::to_string(&input.readiness_links_json)?)
    .bind(&input.rollback_trigger)
    .bind(&input.rollback_action)
    .bind(&created_at)
    .execute(&mut *tx)
    .await
    .context("insert p077 rollout decision")?;

    let mut advisory_migration_count = 0usize;
    if input.decision == "rollback_to_advisory" {
        for run_id in &input.affected_run_ids {
            let previous_mode =
                sqlx::query("SELECT closeout_readiness_mode FROM runs WHERE id = ?1")
                    .bind(run_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .context("load p077 rollback run mode")?
                    .map(|row| row.get::<Option<String>, _>("closeout_readiness_mode"))
                    .ok_or_else(|| anyhow::anyhow!("p077 rollback run not found: {run_id}"))?;

            sqlx::query("UPDATE runs SET closeout_readiness_mode = 'advisory' WHERE id = ?1")
                .bind(run_id)
                .execute(&mut *tx)
                .await
                .context("rollback p077 run to advisory")?;

            sqlx::query(
                r#"INSERT INTO p077_rollout_advisory_migrations
                   (id, decision_id, run_id, previous_mode, new_mode, created_at)
                   VALUES (?1, ?2, ?3, ?4, 'advisory', ?5)"#,
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&decision_id)
            .bind(run_id)
            .bind(previous_mode)
            .bind(&created_at)
            .execute(&mut *tx)
            .await
            .context("insert p077 advisory migration")?;
            advisory_migration_count += 1;
        }
    }

    tx.commit().await.context("commit p077 rollout decision")?;

    Ok(P077RolloutDecisionResult {
        decision_id,
        advisory_migration_count,
    })
}

fn validate_decision_payload(input: &P077RolloutDecisionInput) -> Result<()> {
    require_non_empty("decision_scope", &input.decision_scope)?;
    require_non_empty("decision_type", &input.decision)?;
    require_non_empty("rationale", &input.reason)?;
    require_non_empty("cohort", &input.cohort)?;
    require_non_empty(
        "dependency_checklist_snapshot_id",
        &input.dependency_checklist_snapshot_id,
    )?;
    require_non_empty("measurement_window", &input.measurement_window)?;
    require_non_empty("next_review_date", &input.next_review_date)?;

    if input.eligible_closeouts < 0 {
        bail!("p077 rollout decision payload eligible_closeouts must be non-negative");
    }
    if input.fingerprint_p95_threshold_ms <= 0 {
        bail!("p077 rollout decision payload fingerprint_p95_threshold_ms must be positive");
    }
    if !input.metric_snapshot_json.is_object() {
        bail!("p077 rollout decision payload diagnostic metric snapshot must be an object");
    }
    if !input.primary_metric_values_json.is_object()
        || input
            .primary_metric_values_json
            .as_object()
            .is_some_and(|v| v.is_empty())
    {
        bail!("p077 rollout decision payload primary metric values are required");
    }
    if !input.diagnostic_metric_snapshot_json.is_object()
        || input
            .diagnostic_metric_snapshot_json
            .as_object()
            .is_some_and(|v| v.is_empty())
    {
        bail!("p077 rollout decision payload diagnostic metric snapshot is required");
    }
    if !input.waivers_json.is_array() {
        bail!("p077 rollout decision payload waivers must be an array");
    }
    if !input.readiness_links_json.is_array()
        || input
            .readiness_links_json
            .as_array()
            .is_some_and(|v| v.is_empty())
    {
        bail!("p077 rollout decision payload readiness links are required");
    }

    Ok(())
}

fn require_non_empty(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("p077 rollout decision payload {field} is required");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};

    async fn setup_test_db() -> SqlitePool {
        crate::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    async fn insert_test_run(pool: &SqlitePool, closeout_readiness_mode: Option<&str>) -> String {
        let idea = Idea {
            id: IdeaId::new(),
            title: "P077 rollout test".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        };
        crate::repos::ideas::insert(pool, &idea).await.unwrap();

        let run = Run {
            id: RunId::new(),
            idea_id: idea.id,
            status: RunStatus::Running,
            workflow_id: "wf-p077".into(),
            workflow_title: "P077 rollout".into(),
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
            closeout_readiness_mode: closeout_readiness_mode.map(str::to_string),
        };
        let run_id = run.id.to_string();
        crate::repos::runs::insert(pool, &run).await.unwrap();
        run_id
    }

    fn decision_input(decision: &str) -> P077RolloutDecisionInput {
        P077RolloutDecisionInput {
            decision_scope: "first cohort fixture".into(),
            decision: decision.into(),
            principal: "release-owner".into(),
            reason: "neutral observation until cohort is complete".into(),
            metric_snapshot_json: serde_json::json!({"false_blocks": "1/20"}),
            cohort: "p077-first-cohort".into(),
            eligible_closeouts: 10,
            primary_metric_values_json: serde_json::json!({
                "false_ready_prevented": "0/10",
                "false_blocks": "1/20"
            }),
            diagnostic_metric_snapshot_json: serde_json::json!({
                "pause_to_action": "4h",
                "code_writer_loops_avoided": "100%"
            }),
            dependency_checklist_snapshot_id: "dependency-snapshot-1".into(),
            fingerprint_p95_threshold_ms: 250,
            measurement_window: "2026-05-01/2026-05-10".into(),
            waivers_json: serde_json::json!([]),
            next_review_date: "2026-05-11".into(),
            readiness_links_json: serde_json::json!(["closeout-readiness-generation:fixture"]),
            rollback_trigger: None,
            rollback_action: None,
            affected_run_ids: vec![],
        }
    }

    #[tokio::test]
    async fn p077_rollout_records_live_metric_and_continue_decision() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool, Some("advisory")).await;

        let metric_id = record_metric_event(
            &pool,
            P077RolloutMetricEventInput {
                metric: "false_blocks".into(),
                run_id: Some(run_id.clone()),
                numerator: 1,
                denominator: 20,
                threshold: "<= 5% or <= 2".into(),
                owner: "control-plane owner".into(),
                source: "closeout readiness decision log".into(),
                go_no_go_action: "continue advisory".into(),
                evidence_json: serde_json::json!({"cohort": "fixture"}),
            },
        )
        .await
        .unwrap();

        let metric_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM p077_rollout_metric_events WHERE id = ?1 AND run_id = ?2",
        )
        .bind(metric_id)
        .bind(&run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(metric_count, 1);

        let decision = record_decision(&pool, decision_input("continue_advisory"))
            .await
            .unwrap();

        assert_eq!(decision.advisory_migration_count, 0);
        assert!(!decision.decision_id.is_empty());

        let stored: (String, i64, String, String) = sqlx::query_as(
            r#"SELECT cohort, eligible_closeouts, dependency_checklist_snapshot_id,
                      next_review_date
               FROM p077_rollout_decisions
               WHERE id = ?1"#,
        )
        .bind(&decision.decision_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(stored.0, "p077-first-cohort");
        assert_eq!(stored.1, 10);
        assert_eq!(stored.2, "dependency-snapshot-1");
        assert_eq!(stored.3, "2026-05-11");
    }

    #[tokio::test]
    async fn p077_rollout_rollback_to_advisory_updates_runs_and_records_migrations() {
        let pool = setup_test_db().await;
        let first_run_id = insert_test_run(&pool, Some("enforcement")).await;
        let second_run_id = insert_test_run(&pool, Some("enforcement")).await;

        let mut input = decision_input("rollback_to_advisory");
        input.decision_scope = "false-block breach fixture".into();
        input.reason = "false block threshold breached".into();
        input.metric_snapshot_json = serde_json::json!({"false_blocks": "3/20"});
        input.rollback_trigger = Some("rollback_trigger_false_blocks".into());
        input.rollback_action = Some("rollback_action".into());
        input.affected_run_ids = vec![first_run_id.clone(), second_run_id.clone()];

        let decision = record_decision(&pool, input).await.unwrap();

        assert_eq!(decision.advisory_migration_count, 2);
        for run_id in [&first_run_id, &second_run_id] {
            let mode: Option<String> =
                sqlx::query_scalar("SELECT closeout_readiness_mode FROM runs WHERE id = ?1")
                    .bind(run_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(mode.as_deref(), Some("advisory"));
        }

        let migration_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM p077_rollout_advisory_migrations WHERE decision_id = ?1",
        )
        .bind(&decision.decision_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(migration_count, 2);
    }

    #[tokio::test]
    async fn p077_rollout_rejects_rollback_without_governed_trigger() {
        let pool = setup_test_db().await;
        let run_id = insert_test_run(&pool, Some("enforcement")).await;

        let mut input = decision_input("rollback_to_advisory");
        input.decision_scope = "invalid rollback".into();
        input.reason = "missing trigger".into();
        input.rollback_action = Some("rollback_action".into());
        input.affected_run_ids = vec![run_id];

        let err = record_decision(&pool, input)
            .await
            .expect_err("rollback requires a trigger");

        assert!(err.to_string().contains("requires rollback_trigger"));
    }

    #[tokio::test]
    async fn p077_rollout_rejects_incomplete_decision_payload() {
        let pool = setup_test_db().await;
        let mut input = decision_input("expand_enforcement");
        input.dependency_checklist_snapshot_id = " ".into();

        let err = record_decision(&pool, input)
            .await
            .expect_err("dependency checklist snapshot id is required");

        assert!(err.to_string().contains("dependency_checklist_snapshot_id"));
    }
}
