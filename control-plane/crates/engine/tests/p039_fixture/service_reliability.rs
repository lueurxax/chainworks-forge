use super::*;
use crate::run_carry_forward::PreparationObserver;
use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::Notify;
use uuid::Uuid;

#[allow(dead_code, unused_imports)]
mod fixture {
    use crate as engine;
    include!("mod.rs");
}
use fixture::Fixture;

struct Pause {
    label: &'static str,
    entered: Notify,
    resume: Notify,
}

impl Pause {
    fn at(label: &'static str) -> Arc<Self> {
        Arc::new(Self {
            label,
            entered: Notify::new(),
            resume: Notify::new(),
        })
    }

    async fn reached(&self) {
        tokio::time::timeout(Duration::from_secs(30), self.entered.notified())
            .await
            .unwrap();
    }
}

#[async_trait]
impl PreparationObserver for Pause {
    async fn before_effect(&self, label: &str) -> Result<()> {
        if label == self.label {
            self.entered.notify_one();
            tokio::time::timeout(Duration::from_secs(90), self.resume.notified())
                .await
                .context("test barrier resume timed out")?;
        }
        Ok(())
    }
}

#[tokio::test]
async fn queued_abort_cannot_turn_lane_overload_into_a_new_service_fence() {
    let f = Fixture::new().await;
    let (service, principal) = f.service();
    let plan = f.plan().await;
    let first = f.reserve(&plan).await;
    let pause = Pause::at("copy:archive:product.txt");
    let ticket = service
        .lane
        .try_submit(
            f.job(Uuid::parse_str(&first.operation_id).unwrap(), plan)
                .with_observer(pause.clone()),
        )
        .unwrap();
    pause.reached().await;
    let mut queued = Vec::new();
    let mut sources = Vec::new();
    for _ in 0..3 {
        let source = Fixture::sharing_storage(&f).await;
        let request = source.service_request(&service, &principal).await;
        let result = service
            .call("runs.continue_blocked", &principal, request)
            .await
            .unwrap();
        assert_eq!(result["status"], "accepted", "{result:#}");
        queued.push(
            result["operation"]["operation_id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        sources.push(source);
    }
    let op = operations::find(&f.pool, &queued[0])
        .await
        .unwrap()
        .unwrap();
    let abort = service.call("runs.continuation_abort", &principal, json!({
        "operation_id":op.operation_id, "expected_version":op.version, "caller_request_id":Uuid::new_v4(),
        "reason":"Abort a queued reservation before lane dequeue"
    })).await.unwrap();
    assert_eq!(abort["status"], "accepted", "{abort:#}");
    assert_eq!(
        operations::find(&f.pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap()
            .phase,
        "aborted"
    );
    let fresh = Fixture::sharing_storage(&f).await;
    let request = fresh.service_request(&service, &principal).await;
    let result = service
        .call("runs.continue_blocked", &principal, request)
        .await
        .unwrap();
    let rows: (i64, i64) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM run_continuations WHERE source_run_id=?1),(SELECT COUNT(*) FROM run_execution_fences WHERE source_run_id=?1)")
        .bind(fresh.input.source_run_id.to_string()).fetch_one(&f.pool).await.unwrap();
    pause.resume.notify_one();
    ticket.wait().await.unwrap();
    service.lane.shutdown().await.unwrap();
    assert_eq!(
        result["status"], "denied",
        "overload must not be accepted: {result:#}"
    );
    assert_eq!(result["denial"]["code"], "continuation_capacity");
    assert_eq!(
        rows,
        (0, 0),
        "capacity denial must leave the new source unfenced"
    );
    assert!(
        operations::check_source_guard(&f.pool, &fresh.input.source_run_id.to_string())
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn stopped_lane_denies_service_prepare_without_a_reservation_or_fence() {
    let f = Fixture::new().await;
    let (mut service, principal) = f.service();
    crate::run_carry_forward::worker::stop_lane_for_test(&mut service.lane).await;
    let request = f.service_request(&service, &principal).await;
    let result = service
        .call("runs.continue_blocked", &principal, request)
        .await
        .unwrap();
    assert_eq!(
        result["status"], "denied",
        "stopped lane accepted work: {result:#}"
    );
    assert_eq!(result["denial"]["code"], "continuation_capacity");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_execution_fences")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE kind='prepare_run_continuation'"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(std::fs::read_dir(&f.owned).unwrap().count(), 0);
}

#[tokio::test]
async fn durable_reservation_denials_release_every_preacquired_lane_permit() {
    let f = Fixture::new().await;
    let (service, principal) = f.service();
    let mut sources = Vec::new();
    for _ in 0..4 {
        let source = Fixture::sharing_storage(&f).await;
        source.reserve(&source.plan().await).await;
        sources.push(source);
    }
    for _ in 0..5 {
        let request = f.service_request(&service, &principal).await;
        let result = service
            .call("runs.continue_blocked", &principal, request)
            .await
            .unwrap();
        assert_eq!(result["status"], "denied", "{result:#}");
        assert_eq!(result["denial"]["code"], "continuation_capacity");
    }
    let permits: Vec<_> = (0..4)
        .map(|_| service.lane.try_reserve().unwrap())
        .collect();
    assert!(service.lane.try_reserve().is_err());
    drop(permits);
    assert!(service.lane.try_reserve().is_ok());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuations WHERE source_run_id=?"
        )
        .bind(f.input.source_run_id.to_string())
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
    assert!(
        operations::check_source_guard(&f.pool, &f.input.source_run_id.to_string())
            .await
            .is_ok()
    );
    service.lane.shutdown().await.unwrap();
}

#[tokio::test]
async fn service_reconcile_completes_settled_abort_and_replays_original_results() {
    let f = Fixture::new().await;
    let (service, principal) = f.service();
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    let pause = Pause::at("prepared:publish");
    let ticket = service
        .lane
        .try_submit(
            f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                .with_observer(pause.clone()),
        )
        .unwrap();
    pause.reached().await;
    let current = operations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuation_steps WHERE operation_id=? AND status<>'verified'")
        .bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), 0);
    let abort_request = json!({"operation_id":op.operation_id, "expected_version":current.version,
        "caller_request_id":Uuid::new_v4(), "reason":"Stop before prepared publication"});
    let abort = service
        .call("runs.continuation_abort", &principal, abort_request.clone())
        .await
        .unwrap();
    assert_eq!(abort["status"], "accepted");
    let aborting = operations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(aborting.phase, "aborting");
    assert!(aborting.worker_claimed);
    let live = service
        .call(
            "runs.continuation_reconcile",
            &principal,
            json!({
                "operation_id":op.operation_id, "expected_version":aborting.version,
                "caller_request_id":Uuid::new_v4(), "reason":"Do not infer live-owner settlement"
            }),
        )
        .await
        .unwrap();
    assert_eq!(live["operation"]["phase"], "aborting");
    assert!(operations::check_source_guard(&f.pool, &op.source_run_id)
        .await
        .is_err());
    pause.resume.notify_one();
    assert!(ticket.wait().await.is_err());
    let settled = operations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert!(!settled.worker_claimed);
    assert_eq!(settled.phase, "aborting");
    let root = f.owned.join(&op.operation_id);
    let files = fixture::private_file_digests(&root);
    let reconcile_request = json!({"operation_id":op.operation_id, "expected_version":settled.version,
        "caller_request_id":Uuid::new_v4(), "reason":"Complete explicitly settled abort"});
    let reconciled = service
        .call(
            "runs.continuation_reconcile",
            &principal,
            reconcile_request.clone(),
        )
        .await
        .unwrap();
    assert_eq!(reconciled["status"], "accepted", "{reconciled:#}");
    let finished = operations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finished.phase, "aborted", "settled abort never completed");
    assert_eq!(finished.holds_json, "[]");
    assert!(!finished.worker_claimed);
    assert!(finished.successor_run_id.is_none());
    assert!(operations::check_source_guard(&f.pool, &op.source_run_id)
        .await
        .is_ok());
    assert_eq!(fixture::private_file_digests(&root), files);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM runs WHERE id=?")
            .bind(&op.source_run_id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "blocked"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='advance_run'")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    for (tool, request, original) in [
        ("runs.continuation_abort", abort_request, abort),
        ("runs.continuation_reconcile", reconcile_request, reconciled),
    ] {
        let replay = service.call(tool, &principal, request).await.unwrap();
        assert_eq!(replay["status"], "replayed");
        assert_eq!(replay["replay_of"], "accepted");
        for field in ["operation", "journal_id", "next_action"] {
            assert_eq!(replay[field], original[field], "{tool}: {field}");
        }
    }
    service.lane.shutdown().await.unwrap();
}

#[tokio::test]
async fn service_reconcile_keeps_settled_unknown_abort_effects_fenced() {
    let f = Fixture::new().await;
    let (service, principal) = f.service();
    let plan = f.plan().await;
    let op = f.reserve(&plan).await;
    let pause = Pause::at("copy:checkout:product.txt");
    let ticket = service
        .lane
        .try_submit(
            f.job(Uuid::parse_str(&op.operation_id).unwrap(), plan)
                .with_observer(pause.clone()),
        )
        .unwrap();
    pause.reached().await;
    let current = operations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    service
        .call(
            "runs.continuation_abort",
            &principal,
            json!({
                "operation_id":op.operation_id, "expected_version":current.version,
                "caller_request_id":Uuid::new_v4(), "reason":"Abort an incomplete copy intent"
            }),
        )
        .await
        .unwrap();
    pause.resume.notify_one();
    assert!(ticket.wait().await.is_err());
    let current = operations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert!(!current.worker_claimed);
    assert_eq!(current.phase, "aborting");
    assert!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuation_steps WHERE operation_id=? AND status='unknown'"
        )
        .bind(&op.operation_id)
        .fetch_one(&f.pool)
        .await
        .unwrap()
            > 0
    );
    let root = f.owned.join(&op.operation_id);
    let files = fixture::private_file_digests(&root);
    let result = service.call("runs.continuation_reconcile", &principal, json!({
        "operation_id":op.operation_id, "expected_version":current.version,
        "caller_request_id":Uuid::new_v4(), "reason":"Retain unknown effects despite settled owner"
    })).await.unwrap();
    assert_eq!(result["status"], "accepted");
    let held = operations::find(&f.pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.phase, "aborting");
    assert!(held.holds_json.contains("effect_outcome_unknown"));
    assert!(operations::check_source_guard(&f.pool, &op.source_run_id)
        .await
        .is_err());
    assert_eq!(fixture::private_file_digests(&root), files);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM runs WHERE id=?")
            .bind(&op.source_run_id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "blocked"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='advance_run'")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    service.lane.shutdown().await.unwrap();
}
