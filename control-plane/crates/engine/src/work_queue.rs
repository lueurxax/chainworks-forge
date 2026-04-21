use anyhow::Result;
use sqlx::SqlitePool;
use uuid::Uuid;

use chrono::Utc;
use db::repos::work_items;
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::ids::RunId;

#[derive(Clone)]
pub struct WorkQueue {
    pool: SqlitePool,
}

impl WorkQueue {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn enqueue(
        &self,
        kind: WorkItemKind,
        run_id: Option<RunId>,
        stage_id: Option<String>,
        payload: serde_json::Value,
    ) -> Result<()> {
        let now = Utc::now();
        let item = WorkItem {
            id: Uuid::new_v4().to_string(),
            kind,
            payload_json: serde_json::to_string(&payload)?,
            status: WorkItemStatus::Pending,
            run_id,
            stage_id,
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        };
        work_items::enqueue(&self.pool, &item).await
    }

    pub async fn enqueue_with_id(
        &self,
        id: String,
        kind: WorkItemKind,
        run_id: Option<RunId>,
        stage_id: Option<String>,
        payload: serde_json::Value,
    ) -> Result<()> {
        let now = Utc::now();
        let item = WorkItem {
            id,
            kind,
            payload_json: serde_json::to_string(&payload)?,
            status: WorkItemStatus::Pending,
            run_id,
            stage_id,
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        };
        work_items::enqueue(&self.pool, &item).await
    }

    pub async fn claim_next(&self) -> Result<Option<WorkItem>> {
        work_items::claim_next_non_invoke(&self.pool).await
    }

    pub async fn complete(&self, id: &str) -> Result<()> {
        work_items::complete(&self.pool, id).await
    }

    pub async fn fail(&self, id: &str, error: &str) -> Result<()> {
        work_items::fail(&self.pool, id, error).await
    }
}
