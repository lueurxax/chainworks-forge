use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;

use chrono::Utc;
use db::repos::{scheduler, work_items};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::events::DomainEvent;
use domain::ids::RunId;
use domain::provider::InvokeAgentCapacityConfig;

use crate::event_bus::EventSender;

#[derive(Clone)]
pub struct WorkQueue {
    pool: SqlitePool,
    events: Option<EventSender>,
    capacity_config: Arc<InvokeAgentCapacityConfig>,
}

impl WorkQueue {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            events: None,
            capacity_config: Arc::new(InvokeAgentCapacityConfig::default()),
        }
    }

    pub fn with_events(pool: SqlitePool, events: EventSender) -> Self {
        Self::with_events_and_capacity(pool, events, InvokeAgentCapacityConfig::default())
    }

    pub fn with_events_and_capacity(
        pool: SqlitePool,
        events: EventSender,
        capacity_config: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            events: Some(events),
            capacity_config: Arc::new(capacity_config),
        }
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

    pub fn invoke_agent_capacity_config(&self) -> InvokeAgentCapacityConfig {
        (*self.capacity_config).clone()
    }

    pub async fn complete(&self, id: &str) -> Result<()> {
        let result =
            work_items::complete_with_capacity(&self.pool, id, &self.capacity_config).await?;
        self.publish_scheduler_notification(result);
        Ok(())
    }

    pub async fn fail(&self, id: &str, error: &str) -> Result<()> {
        let result =
            work_items::fail_with_capacity(&self.pool, id, error, &self.capacity_config).await?;
        self.publish_scheduler_notification(result);
        Ok(())
    }

    pub async fn fail_if_terminal_failed_invoke_without_valid_outputs(
        &self,
        id: &str,
        error: &str,
    ) -> Result<bool> {
        if !work_items::running_invoke_agent_has_terminal_failed_outputs(&self.pool, id).await? {
            return Ok(false);
        }
        self.fail(id, error).await?;
        Ok(true)
    }

    pub async fn requeue_after_transient_persistence_contention(
        &self,
        id: &str,
        error: &str,
    ) -> Result<bool> {
        let requeued = work_items::requeue_running_after_transient_persistence_contention(
            &self.pool,
            id,
            Utc::now(),
            error,
        )
        .await?;
        if requeued {
            self.refresh_scheduler_projection().await?;
        }
        Ok(requeued)
    }

    pub async fn requeue_running_advance_item(&self, id: &str, reason: &str) -> Result<bool> {
        let requeued =
            work_items::requeue_running_advance_work_item_by_id(&self.pool, id, Utc::now(), reason)
                .await?;
        if requeued > 0 {
            self.refresh_scheduler_projection().await?;
        }
        Ok(requeued > 0)
    }

    pub async fn refresh_scheduler_projection(&self) -> Result<()> {
        self.refresh_scheduler_projection_with_capacity(&self.capacity_config)
            .await
    }

    pub async fn refresh_scheduler_projection_with_capacity(
        &self,
        capacity: &InvokeAgentCapacityConfig,
    ) -> Result<()> {
        let result =
            scheduler::refresh_queue_summaries_for_notification(&self.pool, capacity).await?;
        self.publish_scheduler_notification(result);
        Ok(())
    }

    pub(crate) fn publish_scheduler_notification(
        &self,
        result: scheduler::RefreshQueueSummariesResult,
    ) {
        if let (Some(events), Some(notification)) = (&self.events, result.notification) {
            let _ = events.send(DomainEvent::SchedulerBackpressureChanged {
                run_id: notification.run_id,
                stage_execution_id: notification.stage_execution_id,
                provider_family: notification.provider_family,
                top_reason: notification.top_reason,
                queued_count: notification.queued_count,
                oldest_queued_age_ms: notification.oldest_queued_age_ms,
                global_queue_depth: notification.global_queue_depth,
                state: notification.state,
                updated_at: notification.updated_at,
                stale_after_ms: notification.stale_after_ms,
            });
        }
    }
}
