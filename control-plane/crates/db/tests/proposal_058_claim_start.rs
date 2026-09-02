use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::work_items;
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

fn work_item(id: &str, kind: WorkItemKind) -> WorkItem {
    let now = Utc::now();
    WorkItem {
        id: id.to_string(),
        kind,
        payload_json: "{}".to_string(),
        status: WorkItemStatus::Pending,
        run_id: None,
        stage_id: None,
        created_at: now,
        scheduled_at: now - Duration::seconds(2),
        attempt_count: 0,
        last_error: None,
    }
}

#[tokio::test]
async fn proposal_058_generic_claim_skips_invoke_agent_items() {
    let pool = test_pool().await;
    work_items::enqueue(&pool, &work_item("invoke-1", WorkItemKind::InvokeAgent))
        .await
        .unwrap();
    work_items::enqueue(&pool, &work_item("advance-1", WorkItemKind::AdvanceRun))
        .await
        .unwrap();

    let claimed = work_items::claim_next_non_invoke(&pool)
        .await
        .unwrap()
        .expect("non-invoke item should be claimable");
    assert_eq!(claimed.id, "advance-1");
    assert_eq!(claimed.kind, WorkItemKind::AdvanceRun);
    assert_eq!(claimed.status, WorkItemStatus::Running);

    let invoke = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id == "invoke-1")
        .expect("invoke item remains pending for claim/start transaction");
    assert_eq!(invoke.status, WorkItemStatus::Pending);
}

#[tokio::test]
async fn proposal_058_invoke_claim_helpers_select_then_mark_in_one_transaction() {
    let pool = test_pool().await;
    work_items::enqueue(&pool, &work_item("invoke-1", WorkItemKind::InvokeAgent))
        .await
        .unwrap();

    let now = Utc::now();
    let mut tx = pool.begin().await.unwrap();
    let selected = work_items::select_next_pending_invoke_agent_for_start_tx(&mut tx, now)
        .await
        .unwrap()
        .expect("invoke item should be selectable");
    assert_eq!(selected.id, "invoke-1");
    assert_eq!(selected.status, WorkItemStatus::Pending);

    let claimed = work_items::mark_claimed_running_tx(&mut tx, &selected.id, now)
        .await
        .unwrap();
    assert_eq!(claimed.id, "invoke-1");
    assert_eq!(claimed.status, WorkItemStatus::Running);
    assert_eq!(claimed.attempt_count, 1);
    tx.commit().await.unwrap();

    let persisted = work_items::list_by_status(&pool, WorkItemStatus::Running)
        .await
        .unwrap();
    assert_eq!(persisted.len(), 1);
    assert_eq!(persisted[0].id, "invoke-1");
}
