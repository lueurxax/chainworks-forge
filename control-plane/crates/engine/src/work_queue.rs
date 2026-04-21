use std::{sync::Arc, time::Duration};

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::sync::Notify;
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
    capacity_config: InvokeAgentCapacityConfig,
    events: Option<EventSender>,
    wakeups: Arc<Notify>,
}

pub struct WorkQueueClaim {
    pub item: Option<WorkItem>,
    pub all_invoke_agent_candidates_blocked: bool,
}

impl WorkQueue {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            capacity_config: crate::capacity::load_invoke_agent_capacity_config_from_env(),
            events: None,
            wakeups: Arc::new(Notify::new()),
        }
    }

    pub fn with_capacity_config(
        pool: SqlitePool,
        capacity_config: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            capacity_config,
            events: None,
            wakeups: Arc::new(Notify::new()),
        }
    }

    pub fn with_event_sender(mut self, events: EventSender) -> Self {
        self.events = Some(events);
        self
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
        work_items::enqueue(&self.pool, &item).await?;
        self.refresh_scheduler_projection().await?;
        self.notify_scheduling_changed();
        Ok(())
    }

    pub async fn claim_next(&self) -> Result<Option<WorkItem>> {
        Ok(self.claim_next_with_status().await?.item)
    }

    pub async fn claim_next_with_status(&self) -> Result<WorkQueueClaim> {
        let result = work_items::claim_next_with_invoke_agent_capacity_result(
            &self.pool,
            &self.capacity_config,
        )
        .await?;
        self.refresh_scheduler_projection().await?;
        Ok(WorkQueueClaim {
            item: result.item,
            all_invoke_agent_candidates_blocked: result.all_invoke_agent_candidates_blocked,
        })
    }

    pub async fn complete(&self, id: &str) -> Result<()> {
        work_items::complete(&self.pool, id).await?;
        self.refresh_scheduler_projection().await?;
        self.notify_scheduling_changed();
        Ok(())
    }

    pub async fn fail(&self, id: &str, error: &str) -> Result<()> {
        work_items::fail(&self.pool, id, error).await?;
        self.refresh_scheduler_projection().await?;
        self.notify_scheduling_changed();
        Ok(())
    }

    pub async fn wait_for_wake_or_timeout(&self, delay: Duration) {
        tokio::select! {
            _ = self.wakeups.notified() => {}
            _ = tokio::time::sleep(delay) => {}
        }
    }

    fn notify_scheduling_changed(&self) {
        self.wakeups.notify_waiters();
    }

    pub async fn refresh_scheduler_projection(&self) -> Result<()> {
        let previous_state = scheduler::latest_health_snapshot(&self.pool)
            .await?
            .map(|snapshot| snapshot.sustained_backpressure_state);
        scheduler::refresh_queue_summaries(&self.pool, &self.capacity_config).await?;
        let Some(events) = &self.events else {
            return Ok(());
        };
        let Some(notification) = scheduler::latest_backpressure_notification(&self.pool).await?
        else {
            return Ok(());
        };
        let Some(previous_state) = previous_state else {
            return Ok(());
        };
        if previous_state == notification.state
            || !matches!(notification.state.as_str(), "active" | "clear")
        {
            return Ok(());
        }

        let now = Utc::now();
        let is_stale = notification.is_stale_at(now);
        let _ = events.send(DomainEvent::SchedulerBackpressureChanged {
            run_id: notification.run_id,
            stage_execution_id: notification.stage_execution_id,
            provider_family: notification.provider_family,
            top_reason: notification.top_reason,
            queued_count: notification.queued_count,
            oldest_queued_age_ms: notification.oldest_queued_age_ms,
            global_queue_depth: notification.global_queue_depth,
            state: notification.state,
            updated_at: notification.updated_at.to_rfc3339(),
            is_stale,
        });
        Ok(())
    }
}
