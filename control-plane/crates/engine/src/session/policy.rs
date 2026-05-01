use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};

use db::repos::sessions;
use domain::session::{
    SessionEvent, SessionEventType, SessionGeneration, SessionGenerationStatus, SessionLineage,
    SessionReuseDisposition,
};

use crate::session::budget::{self, BudgetDecision};

#[derive(Clone, Debug)]
pub struct SessionPolicyInput {
    pub run_id: String,
    pub agent_id: String,
    pub provider: String,
    pub model: String,
    pub working_directory: String,
    pub workspace_mode: String,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub invocation_owner_key: String,
    pub binding_fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionPolicyDecision {
    pub lineage: SessionLineage,
    pub generation: SessionGeneration,
    pub disposition: SessionReuseDisposition,
    pub should_reuse_live_session: bool,
    pub session_reset_reason: Option<String>,
}

pub async fn ensure_policy(
    pool: &SqlitePool,
    input: SessionPolicyInput,
) -> Result<SessionPolicyDecision> {
    let lineage_key = input
        .session_family_id
        .clone()
        .unwrap_or_else(|| input.agent_id.clone());

    let lineage =
        match sessions::find_lineage_by_run_and_key(pool, &input.run_id, &lineage_key).await? {
            Some(existing) => existing,
            None => {
                let lineage = SessionLineage {
                    id: uuid::Uuid::new_v4().to_string(),
                    run_id: input.run_id.clone(),
                    agent_id: input.agent_id.clone(),
                    lineage_id: lineage_key.clone(),
                    session_reuse_scope: input
                        .session_reuse_scope
                        .clone()
                        .unwrap_or_else(|| "none".into()),
                    session_family_id: input.session_family_id.clone(),
                    active_generation_id: None,
                    created_at: Utc::now(),
                    closed_at: None,
                };
                sessions::insert_lineage(pool, &lineage).await?;
                lineage
            }
        };

    let active = sessions::find_active_generation(pool, &lineage.id).await?;
    let generations = sessions::list_generations_for_lineage(pool, &lineage.id).await?;
    if lineage.active_generation_id.is_some() && active.is_none() {
        return create_generation(
            pool,
            lineage,
            input,
            SessionReuseDisposition::UnverifiableSessionHistory,
            None,
            None,
        )
        .await;
    }

    if let Some(active_generation) = active {
        if active_generation.status == SessionGenerationStatus::Active {
            let scope = lineage.session_reuse_scope.as_str();
            if scope == "none" {
                invalidate_generation(
                    pool,
                    &lineage,
                    &active_generation,
                    "scope_none_requires_fresh_session",
                )
                .await?;
                return create_generation(
                    pool,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshSessionRequired,
                    Some("policy_forbid".to_string()),
                    None,
                )
                .await;
            }

            if active_generation.binding_fingerprint != input.binding_fingerprint {
                invalidate_generation(
                    pool,
                    &lineage,
                    &active_generation,
                    "binding_fingerprint_changed",
                )
                .await?;
                return create_generation(
                    pool,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshSessionRequired,
                    Some("provider_mismatch".to_string()),
                    None,
                )
                .await;
            }

            if scope == "same_invocation_owner"
                && active_generation.invocation_owner_key != input.invocation_owner_key
            {
                invalidate_generation(
                    pool,
                    &lineage,
                    &active_generation,
                    "invocation_owner_changed",
                )
                .await?;
                return create_generation(
                    pool,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshSessionRequired,
                    Some("policy_forbid".to_string()),
                    None,
                )
                .await;
            }

            match budget::evaluate(
                &budget_signals_from_generation(pool, &active_generation).await,
                &budget::BudgetConfig::default(),
            ) {
                BudgetDecision::ContinueReuse => {
                    sessions::insert_event(
                        pool,
                        &SessionEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            lineage_id: lineage.id.clone(),
                            generation_id: active_generation.id.clone(),
                            event_type: SessionEventType::Reused,
                            recorded_at: Utc::now(),
                            details_json: None,
                        },
                    )
                    .await?;
                    return Ok(SessionPolicyDecision {
                        lineage,
                        generation: active_generation,
                        disposition: SessionReuseDisposition::Reused,
                        should_reuse_live_session: true,
                        session_reset_reason: None,
                    });
                }
                BudgetDecision::Compact { reason } => {
                    let checkpoint_artifact_id = domain::ids::ArtifactId::new().to_string();
                    let end_reason =
                        format!("budget_compaction_checkpoint:{checkpoint_artifact_id}");
                    sessions::end_generation(
                        pool,
                        &active_generation.id,
                        SessionGenerationStatus::Closed,
                        &end_reason,
                        Utc::now(),
                    )
                    .await?;
                    sessions::insert_event(
                        pool,
                        &SessionEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            lineage_id: lineage.id.clone(),
                            generation_id: active_generation.id.clone(),
                            event_type: SessionEventType::Compacted,
                            recorded_at: Utc::now(),
                            details_json: Some(
                                serde_json::json!({
                                    "reason": reason,
                                    "checkpoint_artifact_id": checkpoint_artifact_id,
                                })
                                .to_string(),
                            ),
                        },
                    )
                    .await?;
                    return create_generation(
                        pool,
                        lineage,
                        input,
                        SessionReuseDisposition::ReusedAfterResume,
                        None,
                        Some(checkpoint_artifact_id),
                    )
                    .await;
                }
                BudgetDecision::Invalidate { reason } => {
                    sessions::end_generation(
                        pool,
                        &active_generation.id,
                        SessionGenerationStatus::Invalidated,
                        &reason,
                        Utc::now(),
                    )
                    .await?;
                    sessions::insert_event(
                        pool,
                        &SessionEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            lineage_id: lineage.id.clone(),
                            generation_id: active_generation.id.clone(),
                            event_type: SessionEventType::BudgetExceeded,
                            recorded_at: Utc::now(),
                            details_json: Some(serde_json::json!({ "reason": reason }).to_string()),
                        },
                    )
                    .await?;
                    return create_generation(
                        pool,
                        lineage,
                        input,
                        SessionReuseDisposition::FreshAfterBudget,
                        None,
                        None,
                    )
                    .await;
                }
            }
        }
    }

    let (disposition, checkpoint_artifact_id) = generations
        .last()
        .map(map_last_generation_state)
        .unwrap_or((SessionReuseDisposition::Fresh, None));
    let reset_reason = matches!(disposition, SessionReuseDisposition::FreshAfterReset)
        .then(|| generations.last().and_then(extract_reset_reason))
        .flatten();

    create_generation(
        pool,
        lineage,
        input,
        disposition,
        reset_reason,
        checkpoint_artifact_id,
    )
    .await
}

pub async fn ensure_policy_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: SessionPolicyInput,
) -> Result<SessionPolicyDecision> {
    let lineage_key = input
        .session_family_id
        .clone()
        .unwrap_or_else(|| input.agent_id.clone());

    let lineage =
        match sessions::find_lineage_by_run_and_key_tx(tx, &input.run_id, &lineage_key).await? {
            Some(existing) => existing,
            None => {
                let lineage = SessionLineage {
                    id: uuid::Uuid::new_v4().to_string(),
                    run_id: input.run_id.clone(),
                    agent_id: input.agent_id.clone(),
                    lineage_id: lineage_key.clone(),
                    session_reuse_scope: input
                        .session_reuse_scope
                        .clone()
                        .unwrap_or_else(|| "none".into()),
                    session_family_id: input.session_family_id.clone(),
                    active_generation_id: None,
                    created_at: Utc::now(),
                    closed_at: None,
                };
                sessions::insert_lineage_tx(tx, &lineage).await?;
                lineage
            }
        };

    let active = sessions::find_active_generation_tx(tx, &lineage.id).await?;
    let generations = sessions::list_generations_for_lineage_tx(tx, &lineage.id).await?;
    if lineage.active_generation_id.is_some() && active.is_none() {
        return create_generation_tx(
            tx,
            lineage,
            input,
            SessionReuseDisposition::UnverifiableSessionHistory,
            None,
            None,
        )
        .await;
    }

    if let Some(active_generation) = active {
        if active_generation.status == SessionGenerationStatus::Active {
            let scope = lineage.session_reuse_scope.as_str();
            if scope == "none" {
                invalidate_generation_tx(
                    tx,
                    &lineage,
                    &active_generation,
                    "scope_none_requires_fresh_session",
                )
                .await?;
                return create_generation_tx(
                    tx,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshSessionRequired,
                    Some("policy_forbid".to_string()),
                    None,
                )
                .await;
            }

            if active_generation.binding_fingerprint != input.binding_fingerprint {
                invalidate_generation_tx(
                    tx,
                    &lineage,
                    &active_generation,
                    "binding_fingerprint_changed",
                )
                .await?;
                return create_generation_tx(
                    tx,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshSessionRequired,
                    Some("provider_mismatch".to_string()),
                    None,
                )
                .await;
            }

            if scope == "same_invocation_owner"
                && active_generation.invocation_owner_key != input.invocation_owner_key
            {
                invalidate_generation_tx(
                    tx,
                    &lineage,
                    &active_generation,
                    "invocation_owner_changed",
                )
                .await?;
                return create_generation_tx(
                    tx,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshSessionRequired,
                    Some("policy_forbid".to_string()),
                    None,
                )
                .await;
            }

            match budget::evaluate(
                &budget_signals_from_generation_tx(tx, &active_generation).await,
                &budget::BudgetConfig::default(),
            ) {
                BudgetDecision::ContinueReuse => {
                    sessions::insert_event_tx(
                        tx,
                        &SessionEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            lineage_id: lineage.id.clone(),
                            generation_id: active_generation.id.clone(),
                            event_type: SessionEventType::Reused,
                            recorded_at: Utc::now(),
                            details_json: None,
                        },
                    )
                    .await?;
                    return Ok(SessionPolicyDecision {
                        lineage,
                        generation: active_generation,
                        disposition: SessionReuseDisposition::Reused,
                        should_reuse_live_session: true,
                        session_reset_reason: None,
                    });
                }
                BudgetDecision::Compact { reason } => {
                    let checkpoint_artifact_id = domain::ids::ArtifactId::new().to_string();
                    let end_reason =
                        format!("budget_compaction_checkpoint:{checkpoint_artifact_id}");
                    sessions::end_generation_tx(
                        tx,
                        &active_generation.id,
                        SessionGenerationStatus::Closed,
                        &end_reason,
                        Utc::now(),
                    )
                    .await?;
                    sessions::insert_event_tx(
                        tx,
                        &SessionEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            lineage_id: lineage.id.clone(),
                            generation_id: active_generation.id.clone(),
                            event_type: SessionEventType::Compacted,
                            recorded_at: Utc::now(),
                            details_json: Some(
                                serde_json::json!({
                                    "reason": reason,
                                    "checkpoint_artifact_id": checkpoint_artifact_id,
                                })
                                .to_string(),
                            ),
                        },
                    )
                    .await?;
                    return create_generation_tx(
                        tx,
                        lineage,
                        input,
                        SessionReuseDisposition::ReusedAfterResume,
                        None,
                        Some(checkpoint_artifact_id),
                    )
                    .await;
                }
                BudgetDecision::Invalidate { reason } => {
                    sessions::end_generation_tx(
                        tx,
                        &active_generation.id,
                        SessionGenerationStatus::Invalidated,
                        &reason,
                        Utc::now(),
                    )
                    .await?;
                    sessions::insert_event_tx(
                        tx,
                        &SessionEvent {
                            id: uuid::Uuid::new_v4().to_string(),
                            lineage_id: lineage.id.clone(),
                            generation_id: active_generation.id.clone(),
                            event_type: SessionEventType::BudgetExceeded,
                            recorded_at: Utc::now(),
                            details_json: Some(serde_json::json!({ "reason": reason }).to_string()),
                        },
                    )
                    .await?;
                    return create_generation_tx(
                        tx,
                        lineage,
                        input,
                        SessionReuseDisposition::FreshAfterBudget,
                        None,
                        None,
                    )
                    .await;
                }
            }
        }
    }

    let (disposition, checkpoint_artifact_id) = generations
        .last()
        .map(map_last_generation_state)
        .unwrap_or((SessionReuseDisposition::Fresh, None));
    let reset_reason = matches!(disposition, SessionReuseDisposition::FreshAfterReset)
        .then(|| generations.last().and_then(extract_reset_reason))
        .flatten();

    create_generation_tx(
        tx,
        lineage,
        input,
        disposition,
        reset_reason,
        checkpoint_artifact_id,
    )
    .await
}

async fn budget_signals_from_generation(
    pool: &SqlitePool,
    generation: &SessionGeneration,
) -> budget::BudgetSignals {
    let fresh_baseline_estimate = std::cmp::max(
        1_i64,
        generation.cumulative_prompt_tokens / std::cmp::max(1_i64, generation.turn_count),
    );
    let transcript_growth_ratio =
        Some(generation.estimated_input_tokens as f64 / fresh_baseline_estimate as f64);
    let cached_token_share = generation
        .latest_cached_input_tokens
        .and_then(|cached_tokens| {
            (generation.estimated_input_tokens > 0)
                .then_some(cached_tokens as f64 / generation.estimated_input_tokens as f64)
        });
    let normalized_savings_versus_fresh = cached_token_share.map(|cache_share| {
        let fresh_cost_cents = fresh_baseline_estimate as f64 / 1000.0;
        let reuse_cost_cents =
            generation.estimated_input_tokens as f64 / 1000.0 * (1.0 - cache_share * 0.5);
        fresh_cost_cents - reuse_cost_cents
    });
    let compaction_churn_count =
        sessions::count_generation_events(pool, &generation.id, SessionEventType::Compacted)
            .await
            .unwrap_or(0);
    budget::BudgetSignals {
        turn_count: generation.turn_count,
        estimated_input_tokens: generation.estimated_input_tokens,
        cumulative_prompt_tokens: generation.cumulative_prompt_tokens,
        cumulative_cost_cents: generation.cumulative_cost_cents,
        idle_age_seconds: (Utc::now()
            - generation.last_activity_at.unwrap_or(generation.created_at))
        .num_seconds() as f64,
        transcript_growth_ratio,
        cached_token_share,
        normalized_savings_versus_fresh,
        effective_prompt_size_fraction: generation.latest_model_context_window.and_then(|window| {
            (window > 0).then_some(generation.estimated_input_tokens as f64 / window as f64)
        }),
        compaction_churn_count,
    }
}

async fn budget_signals_from_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation: &SessionGeneration,
) -> budget::BudgetSignals {
    let fresh_baseline_estimate = std::cmp::max(
        1_i64,
        generation.cumulative_prompt_tokens / std::cmp::max(1_i64, generation.turn_count),
    );
    let transcript_growth_ratio =
        Some(generation.estimated_input_tokens as f64 / fresh_baseline_estimate as f64);
    let cached_token_share = generation
        .latest_cached_input_tokens
        .and_then(|cached_tokens| {
            (generation.estimated_input_tokens > 0)
                .then_some(cached_tokens as f64 / generation.estimated_input_tokens as f64)
        });
    let normalized_savings_versus_fresh = cached_token_share.map(|cache_share| {
        let fresh_cost_cents = fresh_baseline_estimate as f64 / 1000.0;
        let reuse_cost_cents =
            generation.estimated_input_tokens as f64 / 1000.0 * (1.0 - cache_share * 0.5);
        fresh_cost_cents - reuse_cost_cents
    });
    let compaction_churn_count =
        sessions::count_generation_events_tx(tx, &generation.id, SessionEventType::Compacted)
            .await
            .unwrap_or(0);
    budget::BudgetSignals {
        turn_count: generation.turn_count,
        estimated_input_tokens: generation.estimated_input_tokens,
        cumulative_prompt_tokens: generation.cumulative_prompt_tokens,
        cumulative_cost_cents: generation.cumulative_cost_cents,
        idle_age_seconds: (Utc::now()
            - generation.last_activity_at.unwrap_or(generation.created_at))
        .num_seconds() as f64,
        transcript_growth_ratio,
        cached_token_share,
        normalized_savings_versus_fresh,
        effective_prompt_size_fraction: generation.latest_model_context_window.and_then(|window| {
            (window > 0).then_some(generation.estimated_input_tokens as f64 / window as f64)
        }),
        compaction_churn_count,
    }
}

fn map_last_generation_state(
    generation: &SessionGeneration,
) -> (SessionReuseDisposition, Option<String>) {
    if generation.status == SessionGenerationStatus::Reset
        || generation
            .end_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("reset"))
    {
        return (SessionReuseDisposition::FreshAfterReset, None);
    }

    match generation.end_reason.as_deref() {
        Some(reason) if checkpoint_artifact_id_from_end_reason(reason).is_some() => (
            SessionReuseDisposition::ReusedAfterResume,
            checkpoint_artifact_id_from_end_reason(reason),
        ),
        Some(reason) if reason.contains("budget") || reason.contains("cost") => {
            (SessionReuseDisposition::FreshAfterBudget, None)
        }
        Some(reason) if reason.contains("compact") => {
            (SessionReuseDisposition::FreshAfterCompaction, None)
        }
        Some(reason) if reason.contains("transport") || reason.contains("missing_live_handle") => {
            (SessionReuseDisposition::FreshAfterTransportError, None)
        }
        Some(reason) if reason.contains("timeout") => {
            (SessionReuseDisposition::FreshAfterTimeout, None)
        }
        Some(reason) if reason.contains("checkpoint") || reason.contains("resume") => {
            (SessionReuseDisposition::ReusedAfterResume, None)
        }
        Some(_) if generation.status == SessionGenerationStatus::Invalidated => {
            (SessionReuseDisposition::FreshAfterInvalidation, None)
        }
        _ => (SessionReuseDisposition::Fresh, None),
    }
}

fn checkpoint_artifact_id_from_end_reason(reason: &str) -> Option<String> {
    reason
        .strip_prefix("budget_compaction_checkpoint:")
        .or_else(|| reason.strip_prefix("checkpoint_resume:"))
        .map(str::to_string)
}

fn extract_reset_reason(generation: &SessionGeneration) -> Option<String> {
    generation.end_reason.clone()
}

async fn invalidate_generation(
    pool: &SqlitePool,
    lineage: &SessionLineage,
    generation: &SessionGeneration,
    reason: &str,
) -> Result<()> {
    sessions::end_generation(
        pool,
        &generation.id,
        SessionGenerationStatus::Invalidated,
        reason,
        Utc::now(),
    )
    .await?;
    sessions::insert_event(
        pool,
        &SessionEvent {
            id: uuid::Uuid::new_v4().to_string(),
            lineage_id: lineage.id.clone(),
            generation_id: generation.id.clone(),
            event_type: SessionEventType::Invalidated,
            recorded_at: Utc::now(),
            details_json: Some(serde_json::json!({ "reason": reason }).to_string()),
        },
    )
    .await?;
    Ok(())
}

async fn invalidate_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage: &SessionLineage,
    generation: &SessionGeneration,
    reason: &str,
) -> Result<()> {
    sessions::end_generation_tx(
        tx,
        &generation.id,
        SessionGenerationStatus::Invalidated,
        reason,
        Utc::now(),
    )
    .await?;
    sessions::insert_event_tx(
        tx,
        &SessionEvent {
            id: uuid::Uuid::new_v4().to_string(),
            lineage_id: lineage.id.clone(),
            generation_id: generation.id.clone(),
            event_type: SessionEventType::Invalidated,
            recorded_at: Utc::now(),
            details_json: Some(serde_json::json!({ "reason": reason }).to_string()),
        },
    )
    .await?;
    Ok(())
}

async fn create_generation(
    pool: &SqlitePool,
    lineage: SessionLineage,
    input: SessionPolicyInput,
    disposition: SessionReuseDisposition,
    session_reset_reason: Option<String>,
    rehydrated_from_checkpoint_artifact_id: Option<String>,
) -> Result<SessionPolicyDecision> {
    let generation_number = sessions::next_generation_number(pool, &lineage.id).await?;
    let generation = SessionGeneration {
        id: uuid::Uuid::new_v4().to_string(),
        lineage_id: lineage.id.clone(),
        generation: generation_number,
        invocation_owner_key: input.invocation_owner_key,
        provider_session_id: None,
        binding_fingerprint: input.binding_fingerprint,
        rehydrated_from_checkpoint_artifact_id,
        working_directory: input.working_directory,
        workspace_mode: input.workspace_mode,
        runtime_provider: input.provider,
        runtime_model: input.model,
        status: SessionGenerationStatus::Active,
        turn_count: 0,
        estimated_input_tokens: 0,
        latest_cached_input_tokens: None,
        latest_output_tokens: None,
        latest_model_context_window: None,
        cumulative_prompt_tokens: 0,
        cumulative_cost_cents: 0,
        created_at: Utc::now(),
        last_activity_at: None,
        ended_at: None,
        end_reason: None,
    };
    sessions::insert_generation(pool, &generation).await?;
    sessions::set_active_generation(pool, &lineage.id, Some(&generation.id)).await?;
    sessions::insert_event(
        pool,
        &SessionEvent {
            id: uuid::Uuid::new_v4().to_string(),
            lineage_id: lineage.id.clone(),
            generation_id: generation.id.clone(),
            event_type: SessionEventType::Created,
            recorded_at: Utc::now(),
            details_json: None,
        },
    )
    .await?;

    Ok(SessionPolicyDecision {
        lineage,
        generation,
        disposition,
        should_reuse_live_session: false,
        session_reset_reason,
    })
}

async fn create_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage: SessionLineage,
    input: SessionPolicyInput,
    disposition: SessionReuseDisposition,
    session_reset_reason: Option<String>,
    rehydrated_from_checkpoint_artifact_id: Option<String>,
) -> Result<SessionPolicyDecision> {
    let generation_number = sessions::next_generation_number_tx(tx, &lineage.id).await?;
    let generation = SessionGeneration {
        id: uuid::Uuid::new_v4().to_string(),
        lineage_id: lineage.id.clone(),
        generation: generation_number,
        invocation_owner_key: input.invocation_owner_key,
        provider_session_id: None,
        binding_fingerprint: input.binding_fingerprint,
        rehydrated_from_checkpoint_artifact_id,
        working_directory: input.working_directory,
        workspace_mode: input.workspace_mode,
        runtime_provider: input.provider,
        runtime_model: input.model,
        status: SessionGenerationStatus::Active,
        turn_count: 0,
        estimated_input_tokens: 0,
        latest_cached_input_tokens: None,
        latest_output_tokens: None,
        latest_model_context_window: None,
        cumulative_prompt_tokens: 0,
        cumulative_cost_cents: 0,
        created_at: Utc::now(),
        last_activity_at: None,
        ended_at: None,
        end_reason: None,
    };
    sessions::insert_generation_tx(tx, &generation).await?;
    sessions::set_active_generation_tx(tx, &lineage.id, Some(&generation.id)).await?;
    sessions::insert_event_tx(
        tx,
        &SessionEvent {
            id: uuid::Uuid::new_v4().to_string(),
            lineage_id: lineage.id.clone(),
            generation_id: generation.id.clone(),
            event_type: SessionEventType::Created,
            recorded_at: Utc::now(),
            details_json: None,
        },
    )
    .await?;

    Ok(SessionPolicyDecision {
        lineage,
        generation,
        disposition,
        should_reuse_live_session: false,
        session_reset_reason,
    })
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{ideas, runs, sessions};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};
    use domain::session::{
        SessionGeneration, SessionGenerationStatus, SessionLineage, SessionReuseDisposition,
    };

    use super::{ensure_policy, SessionPolicyInput};

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    async fn seed_run(pool: &sqlx::SqlitePool) {
        let idea_id = IdeaId::new();
        ideas::insert(
            pool,
            &Idea {
                id: idea_id,
                title: "Policy idea".into(),
                body: "body".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: Utc::now(),
                archived_at: None,
            },
        )
        .await
        .unwrap();
        runs::insert(
            pool,
            &Run {
                id: "00000000-0000-0000-0000-000000000001"
                    .parse::<RunId>()
                    .unwrap(),
                idea_id,
                status: RunStatus::Running,
                workflow_id: "wf".into(),
                workflow_title: "Workflow".into(),
                workspace_root: "/tmp/ws".into(),
                artifact_root: "/tmp/artifacts".into(),
                started_at: Utc::now(),
                completed_at: None,
                cancellation_requested_at: None,
                cancellation_settled_at: None,
                cancellation_settlement_log: None,
                current_state: None,
                workflow_yaml_path: None,
                agent_catalog_yaml_path: None,
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
            },
        )
        .await
        .unwrap();
    }

    fn base_input() -> SessionPolicyInput {
        SessionPolicyInput {
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            provider: "claude".into(),
            model: "sonnet".into(),
            working_directory: "/tmp/ws".into(),
            workspace_mode: "read_only".into(),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("proposal-loop".into()),
            invocation_owner_key: "owner".into(),
            binding_fingerprint: "fingerprint".into(),
        }
    }

    #[tokio::test]
    async fn creates_fresh_generation_when_lineage_is_missing() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let decision = ensure_policy(&pool, base_input()).await.unwrap();
        assert_eq!(decision.disposition, SessionReuseDisposition::Fresh);
        assert_eq!(decision.generation.generation, 1);
    }

    #[tokio::test]
    async fn reuses_matching_active_generation() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-1".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: Some("generation-1".into()),
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        sessions::insert_generation(
            &pool,
            &SessionGeneration {
                id: "generation-1".into(),
                lineage_id: "lineage-1".into(),
                generation: 1,
                invocation_owner_key: "owner".into(),
                provider_session_id: Some("provider-session".into()),
                binding_fingerprint: "fingerprint".into(),
                rehydrated_from_checkpoint_artifact_id: None,
                working_directory: "/tmp/ws".into(),
                workspace_mode: "read_only".into(),
                runtime_provider: "claude".into(),
                runtime_model: "sonnet".into(),
                status: SessionGenerationStatus::Active,
                turn_count: 1,
                estimated_input_tokens: 0,
                latest_cached_input_tokens: None,
                latest_output_tokens: None,
                latest_model_context_window: None,
                cumulative_prompt_tokens: 0,
                cumulative_cost_cents: 0,
                created_at: now,
                last_activity_at: None,
                ended_at: None,
                end_reason: None,
            },
        )
        .await
        .unwrap();

        let decision = ensure_policy(&pool, base_input()).await.unwrap();
        assert_eq!(decision.disposition, SessionReuseDisposition::Reused);
        assert!(decision.should_reuse_live_session);
        assert_eq!(decision.generation.id, "generation-1");
    }

    #[tokio::test]
    async fn maps_reset_end_reason_to_fresh_after_reset() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-reset".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: None,
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        sessions::insert_generation(
            &pool,
            &SessionGeneration {
                id: "generation-reset".into(),
                lineage_id: lineage.id.clone(),
                generation: 1,
                invocation_owner_key: "owner".into(),
                provider_session_id: Some("provider-session".into()),
                binding_fingerprint: "fingerprint".into(),
                rehydrated_from_checkpoint_artifact_id: None,
                working_directory: "/tmp/ws".into(),
                workspace_mode: "read_only".into(),
                runtime_provider: "claude".into(),
                runtime_model: "sonnet".into(),
                status: SessionGenerationStatus::Reset,
                turn_count: 2,
                estimated_input_tokens: 0,
                latest_cached_input_tokens: None,
                latest_output_tokens: None,
                latest_model_context_window: None,
                cumulative_prompt_tokens: 0,
                cumulative_cost_cents: 0,
                created_at: now,
                last_activity_at: None,
                ended_at: Some(now),
                end_reason: Some("operator_reset".into()),
            },
        )
        .await
        .unwrap();

        let decision = ensure_policy(&pool, base_input()).await.unwrap();
        assert_eq!(
            decision.disposition,
            SessionReuseDisposition::FreshAfterReset
        );
        assert_eq!(decision.generation.generation, 2);
    }

    #[tokio::test]
    async fn owner_mismatch_in_same_invocation_owner_scope_requires_fresh_session() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-owner".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal_writer".into(),
            session_reuse_scope: "same_invocation_owner".into(),
            session_family_id: None,
            active_generation_id: Some("generation-owner".into()),
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        sessions::insert_generation(
            &pool,
            &SessionGeneration {
                id: "generation-owner".into(),
                lineage_id: lineage.id.clone(),
                generation: 1,
                invocation_owner_key: "different-owner".into(),
                provider_session_id: Some("provider-session".into()),
                binding_fingerprint: "fingerprint".into(),
                rehydrated_from_checkpoint_artifact_id: None,
                working_directory: "/tmp/ws".into(),
                workspace_mode: "read_only".into(),
                runtime_provider: "claude".into(),
                runtime_model: "sonnet".into(),
                status: SessionGenerationStatus::Active,
                turn_count: 1,
                estimated_input_tokens: 0,
                latest_cached_input_tokens: None,
                latest_output_tokens: None,
                latest_model_context_window: None,
                cumulative_prompt_tokens: 0,
                cumulative_cost_cents: 0,
                created_at: now,
                last_activity_at: None,
                ended_at: None,
                end_reason: None,
            },
        )
        .await
        .unwrap();

        let mut input = base_input();
        input.session_reuse_scope = Some("same_invocation_owner".into());
        input.session_family_id = None;

        let decision = ensure_policy(&pool, input).await.unwrap();
        assert_eq!(
            decision.disposition,
            SessionReuseDisposition::FreshSessionRequired
        );
        assert_eq!(decision.generation.generation, 2);
    }

    #[tokio::test]
    async fn family_scope_relaxes_owner_key_check_when_fingerprint_matches() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-family".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: Some("generation-family".into()),
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        sessions::insert_generation(
            &pool,
            &SessionGeneration {
                id: "generation-family".into(),
                lineage_id: lineage.id.clone(),
                generation: 1,
                invocation_owner_key: "different-owner".into(),
                provider_session_id: Some("provider-session".into()),
                binding_fingerprint: "fingerprint".into(),
                rehydrated_from_checkpoint_artifact_id: None,
                working_directory: "/tmp/ws".into(),
                workspace_mode: "read_only".into(),
                runtime_provider: "claude".into(),
                runtime_model: "sonnet".into(),
                status: SessionGenerationStatus::Active,
                turn_count: 1,
                estimated_input_tokens: 0,
                latest_cached_input_tokens: None,
                latest_output_tokens: None,
                latest_model_context_window: None,
                cumulative_prompt_tokens: 0,
                cumulative_cost_cents: 0,
                created_at: now,
                last_activity_at: None,
                ended_at: None,
                end_reason: None,
            },
        )
        .await
        .unwrap();

        let decision = ensure_policy(&pool, base_input()).await.unwrap();
        assert_eq!(decision.disposition, SessionReuseDisposition::Reused);
        assert_eq!(decision.generation.id, "generation-family");
    }

    #[tokio::test]
    async fn missing_active_generation_row_fails_closed_as_unverifiable_history() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-broken".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: Some("missing-generation".into()),
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();

        let decision = ensure_policy(&pool, base_input()).await.unwrap();
        assert_eq!(
            decision.disposition,
            SessionReuseDisposition::UnverifiableSessionHistory
        );
        assert_eq!(decision.generation.generation, 1);
    }

    #[tokio::test]
    async fn resumes_from_checkpointed_generation_when_last_end_reason_carries_artifact_id() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let checkpoint_artifact_id = domain::ids::ArtifactId::new().to_string();
        let lineage = SessionLineage {
            id: "lineage-checkpoint".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: None,
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        sessions::insert_generation(
            &pool,
            &SessionGeneration {
                id: "generation-checkpoint".into(),
                lineage_id: lineage.id.clone(),
                generation: 1,
                invocation_owner_key: "owner".into(),
                provider_session_id: Some("provider-session".into()),
                binding_fingerprint: "fingerprint".into(),
                rehydrated_from_checkpoint_artifact_id: None,
                working_directory: "/tmp/ws".into(),
                workspace_mode: "read_only".into(),
                runtime_provider: "claude".into(),
                runtime_model: "sonnet".into(),
                status: SessionGenerationStatus::Closed,
                turn_count: 20,
                estimated_input_tokens: 0,
                latest_cached_input_tokens: None,
                latest_output_tokens: None,
                latest_model_context_window: None,
                cumulative_prompt_tokens: 0,
                cumulative_cost_cents: 0,
                created_at: now,
                last_activity_at: None,
                ended_at: Some(now),
                end_reason: Some(format!(
                    "budget_compaction_checkpoint:{checkpoint_artifact_id}"
                )),
            },
        )
        .await
        .unwrap();

        let decision = ensure_policy(&pool, base_input()).await.unwrap();
        assert_eq!(
            decision.disposition,
            SessionReuseDisposition::ReusedAfterResume
        );
        assert_eq!(
            decision
                .generation
                .rehydrated_from_checkpoint_artifact_id
                .as_deref(),
            Some(checkpoint_artifact_id.as_str())
        );
        assert_eq!(decision.generation.generation, 2);
    }
}
