use chrono::{TimeZone, Utc};
use db::{
    repos::work_items,
    work_item::{WorkItem, WorkItemKind, WorkItemStatus},
};
use domain::ids::RunId;
use serde_json::json;
use sqlx::{Row, SqlitePool};

struct Fixture {
    pool: SqlitePool,
    source: RunId,
    other: RunId,
}

impl Fixture {
    async fn new() -> Self {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        db::writer::register_shared_writer(
            &pool,
            std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .unwrap();
        let source = RunId::new();
        let other = RunId::new();
        for run in [source, other] {
            sqlx::query(
                "INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','now')",
            )
            .bind(run.to_string())
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,worktree_root,artifact_root,started_at) VALUES (?,?,'blocked','workflow','Workflow',?,?,'/meta','now')")
                .bind(run.to_string()).bind(run.to_string()).bind(format!("/repo/{run}"))
                .bind(format!("/repo/{run}")).execute(&pool).await.unwrap();
        }
        Self {
            pool,
            source,
            other,
        }
    }

    async fn fence(&self, historical: bool) {
        // Install only the SQL fixture: reservation admission belongs to the F1 suite.
        sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES ('op','ContinueBlocked','{}','now')")
            .execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES ('op',?,?,'successor','operator','request',?,'implementation_restart_v1','plan',?,'target',?,'op','now','now','later')")
            .bind(self.source.to_string()).bind(self.source.to_string())
            .bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64))
            .execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO run_execution_fences VALUES (?,'op',1,'reserved','now')")
            .bind(self.source.to_string())
            .execute(&self.pool)
            .await
            .unwrap();
        if historical {
            sqlx::query("UPDATE run_continuations SET phase='prepared',version=2,manifest_ref='manifest',manifest_sha256=?")
                .bind("d".repeat(64)).execute(&self.pool).await.unwrap();
            sqlx::query("UPDATE run_execution_fences SET disposition='historical'")
                .execute(&self.pool)
                .await
                .unwrap();
        }
    }

    async fn enqueue(&self, id: &str, kind: WorkItemKind, payload: String, run: Option<RunId>) {
        let due = Utc.timestamp_opt(1_600_000_000, 0).unwrap();
        work_items::enqueue(
            &self.pool,
            &WorkItem {
                id: id.into(),
                kind,
                payload_json: payload,
                status: WorkItemStatus::Pending,
                run_id: run,
                stage_id: None,
                created_at: due,
                scheduled_at: due,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await
        .unwrap();
    }

    async fn valid(&self, id: &str) {
        self.enqueue(
            id,
            WorkItemKind::AdvanceRun,
            json!({"run_id":self.other}).to_string(),
            Some(self.other),
        )
        .await;
    }

    async fn claim(&self, non_invoke: bool) -> anyhow::Result<Option<WorkItem>> {
        if non_invoke {
            work_items::claim_next_non_invoke(&self.pool).await
        } else {
            work_items::claim_next(&self.pool).await
        }
    }

    async fn row(&self, id: &str) -> String {
        sqlx::query_scalar("SELECT json_object('id',id,'kind',kind,'payload',payload_json,'status',status,'run',run_id,'stage',stage_id,'created',created_at,'scheduled',scheduled_at,'attempts',attempt_count,'error',last_error,'started',started_at,'failed',failed_at) FROM work_items WHERE id=?")
            .bind(id).fetch_one(&self.pool).await.unwrap()
    }

    async fn protected_state(&self) -> String {
        sqlx::query_scalar("SELECT json_object('run',r.id,'status',r.status,'fence',f.disposition,'generation',f.generation,'operation',c.operation_id,'phase',c.phase,'version',c.version,'worker_claimed',c.worker_claimed,'manifest',c.manifest_sha256) FROM runs r JOIN run_execution_fences f ON f.source_run_id=r.id JOIN run_continuations c ON c.operation_id=f.operation_id")
            .fetch_one(&self.pool).await.unwrap()
    }

    async fn assert_quarantined(&self, id: &str, original: &str) {
        let before: serde_json::Value = serde_json::from_str(original).unwrap();
        let mut expected = before.clone();
        expected["status"] = json!("failed");
        expected["error"] = json!("p039_queue_owner_unresolved");
        let after: serde_json::Value = serde_json::from_str(&self.row(id).await).unwrap();
        assert!(after["failed"]
            .as_str()
            .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok()));
        expected["failed"] = after["failed"].clone();
        assert_eq!(
            after, expected,
            "quarantine must retain payload, identity, timestamps and attempts"
        );
    }
}

async fn missing_owner_progress(historical: bool, non_invoke: bool) {
    let f = Fixture::new().await;
    f.fence(historical).await;
    let protected = f.protected_state().await;
    let mut originals = Vec::new();
    for key in [
        "continuation_id",
        "run_id",
        "stage_execution_id",
        "agent_execution_id",
        "mediation_record_id",
        "lead_mediation_record_id",
    ] {
        let id = format!("missing-{key}");
        f.enqueue(
            &id,
            WorkItemKind::ProcessContinuation,
            json!({key:"missing"}).to_string(),
            None,
        )
        .await;
        originals.push((id.clone(), f.row(&id).await));
    }
    f.enqueue(
        "absent-owner",
        WorkItemKind::ProcessContinuation,
        "{}".into(),
        None,
    )
    .await;
    originals.push(("absent-owner".into(), f.row("absent-owner").await));
    // Same timestamp for all rows proves the rowid FIFO cursor as well as progress.
    f.valid("valid-first").await;
    f.valid("valid-second").await;
    for id in ["valid-first", "valid-second"] {
        let claimed = f
            .claim(non_invoke)
            .await
            .expect("unresolved ownership must not poison the FIFO")
            .unwrap();
        assert_eq!(claimed.id, id);
        assert_eq!(claimed.status, WorkItemStatus::Running);
        assert_eq!(claimed.attempt_count, 1);
    }
    assert!(f.claim(non_invoke).await.unwrap().is_none());
    for (id, original) in originals {
        f.assert_quarantined(&id, &original).await;
    }
    assert_eq!(f.protected_state().await, protected);
    for table in [
        "background_leases",
        "side_effects",
        "run_continuation_steps",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM {table}"
            )))
            .fetch_one(&f.pool)
            .await
            .unwrap(),
            0
        );
    }
    f.pool.close().await;
}

#[tokio::test]
async fn claim_next_reserved_unresolved_owners_do_not_poison_fifo() {
    missing_owner_progress(false, false).await;
}
#[tokio::test]
async fn claim_next_historical_unresolved_owners_do_not_poison_fifo() {
    missing_owner_progress(true, false).await;
}
#[tokio::test]
async fn non_invoke_reserved_unresolved_owners_do_not_poison_fifo() {
    missing_owner_progress(false, true).await;
}
#[tokio::test]
async fn non_invoke_historical_unresolved_owners_do_not_poison_fifo() {
    missing_owner_progress(true, true).await;
}

async fn removed_owner_progress(non_invoke: bool) {
    let f = Fixture::new().await;
    f.fence(true).await;
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES ('other-stage',?,'s','S','completed','now')")
        .bind(f.other.to_string()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,owner_id,agent_id,provider,status,started_at) VALUES ('other-agent','other-stage','other-stage','a','codex','completed','now')")
        .execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO agent_work_continuations(id,run_id,stage_execution_id,agent_execution_id,mode,trigger_kind,status,idempotency_scope,idempotency_key,request_fingerprint_sha256,created_at,updated_at) VALUES ('removed',?,'other-stage','other-agent','live_handle_continuation','operator_mcp','succeeded','scope','key',?,'now','now')")
        .bind(f.other.to_string()).bind("a".repeat(64)).execute(&f.pool).await.unwrap();
    f.enqueue(
        "stale",
        WorkItemKind::ProcessContinuation,
        json!({"continuation_id":"removed"}).to_string(),
        None,
    )
    .await;
    let before = f.row("stale").await;
    sqlx::query("DELETE FROM agent_work_continuations WHERE id='removed'")
        .execute(&f.pool)
        .await
        .unwrap();
    let protected = f.protected_state().await;
    f.valid("valid").await;
    assert_eq!(f.claim(non_invoke).await.unwrap().unwrap().id, "valid");
    f.assert_quarantined("stale", &before).await;
    assert!(f.claim(non_invoke).await.unwrap().is_none());
    assert_eq!(f.protected_state().await, protected);
    f.pool.close().await;
}

#[tokio::test]
async fn claim_next_rechecks_owner_removed_after_enqueue() {
    removed_owner_progress(false).await;
}
#[tokio::test]
async fn non_invoke_rechecks_owner_removed_after_enqueue() {
    removed_owner_progress(true).await;
}

async fn protected_history_progress(non_invoke: bool) {
    for historical in [false, true] {
        let f = Fixture::new().await;
        // Seed stale source work before the fence, without bypassing any trigger.
        f.enqueue(
            "protected",
            WorkItemKind::ProcessContinuation,
            json!({"run_id":f.source,"continuation_id":"missing"}).to_string(),
            Some(f.source),
        )
        .await;
        f.enqueue(
            "protected-malformed",
            WorkItemKind::AdvanceRun,
            "{".into(),
            Some(f.source),
        )
        .await;
        f.fence(historical).await;
        // Mixed known/unknown ownership is retained, never asserted to be unowned.
        f.enqueue(
            "mixed",
            WorkItemKind::ProcessContinuation,
            json!({"run_id":f.other,"continuation_id":"missing"}).to_string(),
            None,
        )
        .await;
        let mut originals = Vec::new();
        for id in ["protected", "protected-malformed", "mixed"] {
            originals.push((id, f.row(id).await));
        }
        let protected = f.protected_state().await;
        f.valid("valid").await;
        assert_eq!(f.claim(non_invoke).await.unwrap().unwrap().id, "valid");
        assert!(f.claim(non_invoke).await.unwrap().is_none());
        for (id, original) in originals {
            assert_eq!(f.row(id).await, original);
        }
        assert_eq!(f.protected_state().await, protected);
        f.pool.close().await;
    }
}

#[tokio::test]
async fn claim_next_retains_protected_and_ambiguous_history() {
    protected_history_progress(false).await;
}
#[tokio::test]
async fn non_invoke_retains_protected_and_ambiguous_history() {
    protected_history_progress(true).await;
}

async fn no_fence_policy(non_invoke: bool) {
    let f = Fixture::new().await;
    f.enqueue(
        "legacy-missing-owner",
        WorkItemKind::ProcessContinuation,
        json!({"continuation_id":"missing"}).to_string(),
        None,
    )
    .await;
    f.enqueue(
        "malformed-advance",
        WorkItemKind::AdvanceRun,
        "{".into(),
        Some(f.other),
    )
    .await;
    f.valid("valid").await;
    assert_eq!(
        f.claim(non_invoke).await.unwrap().unwrap().id,
        "legacy-missing-owner"
    );
    assert_eq!(f.claim(non_invoke).await.unwrap().unwrap().id, "valid");
    let row = sqlx::query(
        "SELECT status,last_error,attempt_count FROM work_items WHERE id='malformed-advance'",
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert!(row.get::<Option<String>, _>("last_error").is_some());
    assert_eq!(row.get::<i64, _>("attempt_count"), 0);
    f.pool.close().await;
}

#[tokio::test]
async fn claim_next_preserves_ordinary_no_fence_policy() {
    no_fence_policy(false).await;
}
#[tokio::test]
async fn non_invoke_preserves_ordinary_no_fence_policy() {
    no_fence_policy(true).await;
}

#[tokio::test]
async fn ordinary_sql_errors_roll_back_quarantine_and_are_not_fence_denials() {
    for non_invoke in [false, true] {
        let f = Fixture::new().await;
        f.fence(true).await;
        f.enqueue(
            "unowned",
            WorkItemKind::ProcessContinuation,
            json!({"continuation_id":"missing"}).to_string(),
            None,
        )
        .await;
        f.valid("valid").await;
        let before = f.row("unowned").await;
        let valid_before = f.row("valid").await;
        sqlx::raw_sql("CREATE TRIGGER fixture_claim_error BEFORE UPDATE OF status ON work_items WHEN NEW.id='valid' AND NEW.status='running' BEGIN SELECT RAISE(ABORT,'continuation_in_progress fixture_sql_failure'); END;")
            .execute(&f.pool).await.unwrap();
        let error = f.claim(non_invoke).await.unwrap_err();
        assert!(
            format!("{error:#}").contains("fixture_sql_failure"),
            "{error:#}"
        );
        assert_eq!(f.row("unowned").await, before);
        assert_eq!(f.row("valid").await, valid_before);
        sqlx::query("DROP TRIGGER fixture_claim_error")
            .execute(&f.pool)
            .await
            .unwrap();
        assert_eq!(f.claim(non_invoke).await.unwrap().unwrap().id, "valid");
        f.assert_quarantined("unowned", &before).await;
        f.pool.close().await;
    }
}

#[tokio::test]
async fn claim_cursor_preserves_due_order_and_existing_kind_filters() {
    for non_invoke in [false, true] {
        let f = Fixture::new().await;
        f.fence(true).await;
        f.enqueue(
            "prepare",
            WorkItemKind::PrepareRunContinuation,
            "{}".into(),
            None,
        )
        .await;
        f.valid("later").await;
        sqlx::query(
            "UPDATE work_items SET scheduled_at='2020-09-13T12:27:00+00:00' WHERE id='later'",
        )
        .execute(&f.pool)
        .await
        .unwrap();
        f.valid("future").await;
        sqlx::query(
            "UPDATE work_items SET scheduled_at='2099-01-01T00:00:00+00:00' WHERE id='future'",
        )
        .execute(&f.pool)
        .await
        .unwrap();
        f.enqueue(
            "invoke",
            WorkItemKind::InvokeAgent,
            json!({"run_id":f.other}).to_string(),
            Some(f.other),
        )
        .await;
        f.enqueue(
            "unowned",
            WorkItemKind::ProcessContinuation,
            json!({"continuation_id":"missing"}).to_string(),
            None,
        )
        .await;
        let original = f.row("unowned").await;
        let prepare = f.row("prepare").await;
        let future = f.row("future").await;
        let invoke = f.row("invoke").await;
        if !non_invoke {
            assert_eq!(f.claim(false).await.unwrap().unwrap().id, "invoke");
        }
        // The later due row has a smaller rowid than the rejected candidate.
        assert_eq!(f.claim(non_invoke).await.unwrap().unwrap().id, "later");
        f.assert_quarantined("unowned", &original).await;
        assert!(f.claim(non_invoke).await.unwrap().is_none());
        assert_eq!(f.row("prepare").await, prepare);
        assert_eq!(f.row("future").await, future);
        if non_invoke {
            assert_eq!(f.row("invoke").await, invoke);
        }
        // Existing system work without a run owner is not globally quarantined.
        f.enqueue(
            "projection",
            WorkItemKind::RebuildProjection,
            "{}".into(),
            None,
        )
        .await;
        assert_eq!(f.claim(non_invoke).await.unwrap().unwrap().id, "projection");
        f.pool.close().await;
    }
}
