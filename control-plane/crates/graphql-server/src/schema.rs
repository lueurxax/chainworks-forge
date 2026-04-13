use std::sync::Arc;

use async_graphql::*;
use async_graphql::futures_util::StreamExt;
use sqlx::SqlitePool;
use tokio_stream::wrappers::BroadcastStream;

use db::repos::{approvals, ideas, runs, projections};
use domain::commands::{
    ApproveStageCmd, CancelRunCmd, Command, RejectStageCmd, RetryStageCmd, StartRunCmd,
};
use domain::events::DomainEvent;
use domain::ids::{IdeaId, RunId};
use engine::command_handler::CommandHandler;
use engine::event_bus::EventSender;

use crate::types::approval::GqlApproval;
use crate::types::artifact::GqlArtifact;
use crate::types::idea::GqlIdea;
use crate::types::run::GqlRun;
use crate::types::stage::GqlStageExecution;

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

pub fn build_schema(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
) -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(cmd_handler)
        .data(events)
        .finish()
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn ideas(
        &self,
        ctx: &Context<'_>,
        include_archived: Option<bool>,
    ) -> Result<Vec<GqlIdea>> {
        let pool = ctx.data::<SqlitePool>()?;
        let include = include_archived.unwrap_or(false);
        let items = ideas::list(pool, include).await?;
        Ok(items.into_iter().map(GqlIdea::from).collect())
    }

    async fn idea(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlIdea>> {
        let pool = ctx.data::<SqlitePool>()?;
        let idea_id: IdeaId = id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let item = ideas::find_by_id(pool, idea_id).await?;
        Ok(item.map(GqlIdea::from))
    }

    async fn runs(&self, ctx: &Context<'_>, idea_id: Option<ID>) -> Result<Vec<GqlRun>> {
        let pool = ctx.data::<SqlitePool>()?;
        if let Some(id) = idea_id {
            let items = projections::list_by_idea_projection(pool, id.as_str()).await?;
            Ok(items.into_iter().map(GqlRun::from).collect())
        } else {
            let items = projections::list_active_projection(pool).await?;
            Ok(items.into_iter().map(GqlRun::from).collect())
        }
    }

    async fn run(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlRun>> {
        let pool = ctx.data::<SqlitePool>()?;
        let item = projections::find_run_projection(pool, id.as_str()).await?;
        Ok(item.map(GqlRun::from))
    }

    async fn approval_inbox(&self, ctx: &Context<'_>) -> Result<Vec<GqlApproval>> {
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_pending_inbox_projection(pool).await?;
        Ok(items.into_iter().map(GqlApproval::from).collect())
    }

    async fn artifacts(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlArtifact>> {
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_artifacts_projection(pool, run_id.as_str()).await?;
        Ok(items.into_iter().map(GqlArtifact::from).collect())
    }

    async fn stages(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlStageExecution>> {
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_stages_projection(pool, run_id.as_str()).await?;
        Ok(items.into_iter().map(GqlStageExecution::from).collect())
    }
}

pub struct MutationRoot;

#[Object]
impl MutationRoot {
    async fn start_run(
        &self,
        ctx: &Context<'_>,
        idea_id: ID,
        workflow_id: String,
        workflow_title: String,
        workspace_root: String,
        artifact_root: String,
    ) -> Result<GqlRun> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let iid: IdeaId =
            idea_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::StartRun(StartRunCmd {
            idea_id: iid,
            workflow_id,
            workflow_title,
            workspace_root,
            artifact_root,
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
        });

        let result = cmd_handler.handle(cmd).await?;
        let run_id = match result {
            engine::command_handler::CommandResult::RunStarted { run_id } => run_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let run = runs::find_by_id(pool, run_id)
            .await?
            .ok_or_else(|| Error::new("Run not found after creation"))?;

        Ok(GqlRun::from(run))
    }

    async fn approve_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
        comment: Option<String>,
    ) -> Result<GqlApproval> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::ApproveStage(ApproveStageCmd {
            run_id: rid,
            stage_id,
            comment,
        });

        let result = cmd_handler.handle(cmd).await?;
        let approval_id = match result {
            engine::command_handler::CommandResult::StageApproved { approval_id } => approval_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let approval = approvals::find_by_id(pool, approval_id)
            .await?
            .ok_or_else(|| Error::new("Approval not found after update"))?;

        Ok(GqlApproval::from(approval))
    }

    async fn reject_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
        comment: Option<String>,
    ) -> Result<GqlApproval> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::RejectStage(RejectStageCmd {
            run_id: rid,
            stage_id,
            comment,
        });

        let result = cmd_handler.handle(cmd).await?;
        let approval_id = match result {
            engine::command_handler::CommandResult::StageRejected { approval_id } => approval_id,
            _ => return Err(Error::new("Unexpected command result")),
        };

        let approval = approvals::find_by_id(pool, approval_id)
            .await?
            .ok_or_else(|| Error::new("Approval not found after update"))?;

        Ok(GqlApproval::from(approval))
    }

    async fn retry_stage(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        stage_id: String,
    ) -> Result<bool> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::RetryStage(RetryStageCmd {
            run_id: rid,
            stage_id,
        });

        cmd_handler.handle(cmd).await?;
        Ok(true)
    }

    async fn cancel_run(&self, ctx: &Context<'_>, run_id: ID) -> Result<bool> {
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let rid: RunId =
            run_id.parse().map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let cmd = Command::CancelRun(CancelRunCmd { run_id: rid });
        cmd_handler.handle(cmd).await?;
        Ok(true)
    }
}

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn run_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRun>>> + '_ {
        let pool = ctx.data::<SqlitePool>().unwrap().clone();
        let events = ctx.data::<EventSender>().unwrap().clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::RunStatusChanged { run_id, .. }
                    | DomainEvent::RunStarted { run_id, .. } => {
                        if let Some(fid) = filter_run_id {
                            if run_id != fid {
                                return None;
                            }
                        }
                        let run = runs::find_by_id(&pool, run_id).await.ok()??;
                        Some(Ok(Some(GqlRun::from(run))))
                    }
                    _ => None,
                }
            };
            fut
        })
    }

    async fn stage_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlStageExecution>>> + '_ {
        let pool = ctx.data::<SqlitePool>().unwrap().clone();
        let events = ctx.data::<EventSender>().unwrap().clone();
        let filter_run_id: RunId = run_id.parse().unwrap_or_else(|_| RunId::new());

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id,
                        ..
                    } => {
                        if run_id != filter_run_id {
                            return None;
                        }
                        let stage =
                            db::repos::stages::find_by_id(&pool, stage_execution_id)
                                .await
                                .ok()??;
                        Some(Ok(Some(GqlStageExecution::from(stage))))
                    }
                    _ => None,
                }
            };
            fut
        })
    }

    async fn approval_requested(
        &self,
        ctx: &Context<'_>,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlApproval>>> + '_ {
        let pool = ctx.data::<SqlitePool>().unwrap().clone();
        let events = ctx.data::<EventSender>().unwrap().clone();

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::ApprovalRequested { approval_id, .. } => {
                        let approval =
                            approvals::find_by_id(&pool, approval_id).await.ok()??;
                        Some(Ok(Some(GqlApproval::from(approval))))
                    }
                    _ => None,
                }
            };
            fut
        })
    }

    /// Live stream of ACP runtime/session lifecycle events.
    /// Emits on session_started, session_completed, and session_failed.
    /// Required for the SwiftUI thin-client's runtime health surface (P027 §8.1).
    async fn runtime_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRuntimeEvent>>> + '_ {
        let events = ctx.data::<EventSender>().unwrap().clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        BroadcastStream::new(rx).filter_map(move |msg| {
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id,
                        agent_id,
                        provider,
                        event_kind,
                    } => {
                        if let Some(fid) = filter_run_id {
                            if run_id != fid {
                                return None;
                            }
                        }
                        Some(Ok(Some(GqlRuntimeEvent {
                            run_id: ID(run_id.to_string()),
                            stage_id,
                            agent_id,
                            provider,
                            event_kind,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        })))
                    }
                    _ => None,
                }
            };
            fut
        })
    }
}

/// Runtime lifecycle event surfaced to GraphQL subscribers.
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlRuntimeEvent {
    pub run_id: ID,
    pub stage_id: String,
    pub agent_id: String,
    pub provider: String,
    /// "session_started" | "session_completed" | "session_failed"
    pub event_kind: String,
    pub timestamp: String,
}
