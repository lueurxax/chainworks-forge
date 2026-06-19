use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};

use db::repos::sessions;
use domain::agent::{AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement};
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

    let mut lineage =
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

    repair_duplicate_active_generations(pool, &mut lineage).await?;
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
            if active_generation_has_claude_long_context_credits_failure(
                pool,
                &active_generation.id,
            )
            .await?
            {
                invalidate_generation(
                    pool,
                    &lineage,
                    &active_generation,
                    CLAUDE_LONG_CONTEXT_CREDITS_REQUIRED_REASON,
                )
                .await?;
                return create_generation(
                    pool,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshAfterInvalidation,
                    Some(CLAUDE_LONG_CONTEXT_CREDITS_REQUIRED_REASON.to_string()),
                    None,
                )
                .await;
            }

            if active_generation_has_missing_required_outputs_failure(pool, &active_generation.id)
                .await?
            {
                invalidate_generation(
                    pool,
                    &lineage,
                    &active_generation,
                    PREVIOUS_MISSING_REQUIRED_OUTPUTS_REASON,
                )
                .await?;
                return create_generation(
                    pool,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshAfterInvalidation,
                    None,
                    None,
                )
                .await;
            }

            if active_generation_has_tool_output_budget_failure(pool, &active_generation.id).await?
            {
                invalidate_generation(
                    pool,
                    &lineage,
                    &active_generation,
                    TOOL_OUTPUT_BUDGET_EXCEEDED_REASON,
                )
                .await?;
                return create_generation(
                    pool,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshAfterInvalidation,
                    Some(TOOL_OUTPUT_BUDGET_EXCEEDED_REASON.to_string()),
                    None,
                )
                .await;
            }

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

    let mut lineage =
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

    repair_duplicate_active_generations_tx(tx, &mut lineage).await?;
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
            if active_generation_has_claude_long_context_credits_failure_tx(
                tx,
                &active_generation.id,
            )
            .await?
            {
                invalidate_generation_tx(
                    tx,
                    &lineage,
                    &active_generation,
                    CLAUDE_LONG_CONTEXT_CREDITS_REQUIRED_REASON,
                )
                .await?;
                return create_generation_tx(
                    tx,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshAfterInvalidation,
                    Some(CLAUDE_LONG_CONTEXT_CREDITS_REQUIRED_REASON.to_string()),
                    None,
                )
                .await;
            }

            if active_generation_has_missing_required_outputs_failure_tx(tx, &active_generation.id)
                .await?
            {
                invalidate_generation_tx(
                    tx,
                    &lineage,
                    &active_generation,
                    PREVIOUS_MISSING_REQUIRED_OUTPUTS_REASON,
                )
                .await?;
                return create_generation_tx(
                    tx,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshAfterInvalidation,
                    None,
                    None,
                )
                .await;
            }

            if active_generation_has_tool_output_budget_failure_tx(tx, &active_generation.id)
                .await?
            {
                invalidate_generation_tx(
                    tx,
                    &lineage,
                    &active_generation,
                    TOOL_OUTPUT_BUDGET_EXCEEDED_REASON,
                )
                .await?;
                return create_generation_tx(
                    tx,
                    lineage,
                    input,
                    SessionReuseDisposition::FreshAfterInvalidation,
                    Some(TOOL_OUTPUT_BUDGET_EXCEEDED_REASON.to_string()),
                    None,
                )
                .await;
            }

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

const PREVIOUS_MISSING_REQUIRED_OUTPUTS_REASON: &str = "previous_missing_required_outputs";
const CLAUDE_LONG_CONTEXT_CREDITS_REQUIRED_REASON: &str = "claude_long_context_credits_required";
const TOOL_OUTPUT_BUDGET_EXCEEDED_REASON: &str = "tool_output_budget_exceeded";
const DUPLICATE_ACTIVE_SESSION_OWNER_REASON: &str = "duplicate_active_session_owner_rejected";

async fn repair_duplicate_active_generations(
    pool: &SqlitePool,
    lineage: &mut SessionLineage,
) -> Result<()> {
    let generations = sessions::list_generations_for_lineage(pool, &lineage.id).await?;
    let active_generations = generations
        .into_iter()
        .filter(|generation| generation.status == SessionGenerationStatus::Active)
        .collect::<Vec<_>>();
    if active_generations.len() <= 1 {
        return Ok(());
    }

    let survivor = select_active_generation_survivor(&active_generations, lineage);
    if lineage.active_generation_id.as_deref() != Some(survivor.id.as_str()) {
        sessions::set_active_generation(pool, &lineage.id, Some(&survivor.id)).await?;
        lineage.active_generation_id = Some(survivor.id.clone());
    }
    for duplicate in active_generations
        .iter()
        .filter(|generation| generation.id != survivor.id)
    {
        invalidate_duplicate_active_generation(pool, lineage, duplicate, &survivor.id).await?;
    }
    Ok(())
}

async fn repair_duplicate_active_generations_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage: &mut SessionLineage,
) -> Result<()> {
    let generations = sessions::list_generations_for_lineage_tx(tx, &lineage.id).await?;
    let active_generations = generations
        .into_iter()
        .filter(|generation| generation.status == SessionGenerationStatus::Active)
        .collect::<Vec<_>>();
    if active_generations.len() <= 1 {
        return Ok(());
    }

    let survivor = select_active_generation_survivor(&active_generations, lineage);
    if lineage.active_generation_id.as_deref() != Some(survivor.id.as_str()) {
        sessions::set_active_generation_tx(tx, &lineage.id, Some(&survivor.id)).await?;
        lineage.active_generation_id = Some(survivor.id.clone());
    }
    for duplicate in active_generations
        .iter()
        .filter(|generation| generation.id != survivor.id)
    {
        invalidate_duplicate_active_generation_tx(tx, lineage, duplicate, &survivor.id).await?;
    }
    Ok(())
}

fn select_active_generation_survivor(
    active_generations: &[SessionGeneration],
    lineage: &SessionLineage,
) -> SessionGeneration {
    if let Some(active_id) = lineage.active_generation_id.as_deref() {
        if let Some(generation) = active_generations
            .iter()
            .find(|generation| generation.id == active_id)
        {
            return generation.clone();
        }
    }
    active_generations
        .iter()
        .max_by_key(|generation| (generation.generation, generation.created_at))
        .expect("active_generations is non-empty")
        .clone()
}

async fn invalidate_duplicate_active_generation(
    pool: &SqlitePool,
    lineage: &SessionLineage,
    generation: &SessionGeneration,
    surviving_generation_id: &str,
) -> Result<()> {
    sessions::end_generation(
        pool,
        &generation.id,
        SessionGenerationStatus::Invalidated,
        DUPLICATE_ACTIVE_SESSION_OWNER_REASON,
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
            details_json: Some(
                serde_json::json!({
                    "reason": DUPLICATE_ACTIVE_SESSION_OWNER_REASON,
                    "surviving_generation_id": surviving_generation_id,
                })
                .to_string(),
            ),
        },
    )
    .await?;
    Ok(())
}

async fn invalidate_duplicate_active_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage: &SessionLineage,
    generation: &SessionGeneration,
    surviving_generation_id: &str,
) -> Result<()> {
    sessions::end_generation_tx(
        tx,
        &generation.id,
        SessionGenerationStatus::Invalidated,
        DUPLICATE_ACTIVE_SESSION_OWNER_REASON,
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
            details_json: Some(
                serde_json::json!({
                    "reason": DUPLICATE_ACTIVE_SESSION_OWNER_REASON,
                    "surviving_generation_id": surviving_generation_id,
                })
                .to_string(),
            ),
        },
    )
    .await?;
    Ok(())
}

pub async fn invalidate_generation_after_missing_required_outputs(
    pool: &SqlitePool,
    session_generation_id: Option<&str>,
    facts: &AgentExecutionRuntimeFacts,
) -> Result<bool> {
    let Some(session_generation_id) = session_generation_id else {
        return Ok(false);
    };
    if !runtime_facts_require_session_invalidation(facts) {
        return Ok(false);
    }
    let Some(generation) = sessions::find_generation_by_id(pool, session_generation_id).await?
    else {
        return Ok(false);
    };
    if generation.status != SessionGenerationStatus::Active {
        return Ok(false);
    }
    let Some(lineage) = sessions::find_lineage_by_id(pool, &generation.lineage_id).await? else {
        return Ok(false);
    };
    invalidate_generation(
        pool,
        &lineage,
        &generation,
        PREVIOUS_MISSING_REQUIRED_OUTPUTS_REASON,
    )
    .await?;
    Ok(true)
}

pub async fn invalidate_generation_after_missing_required_outputs_tx(
    tx: &mut Transaction<'_, Sqlite>,
    session_generation_id: Option<&str>,
    facts: &AgentExecutionRuntimeFacts,
) -> Result<bool> {
    let Some(session_generation_id) = session_generation_id else {
        return Ok(false);
    };
    if !runtime_facts_require_session_invalidation(facts) {
        return Ok(false);
    }
    let Some(generation) = sessions::find_generation_by_id_tx(tx, session_generation_id).await?
    else {
        return Ok(false);
    };
    if generation.status != SessionGenerationStatus::Active {
        return Ok(false);
    }
    let Some(lineage) = sessions::find_lineage_by_id_tx(tx, &generation.lineage_id).await? else {
        return Ok(false);
    };
    invalidate_generation_tx(
        tx,
        &lineage,
        &generation,
        PREVIOUS_MISSING_REQUIRED_OUTPUTS_REASON,
    )
    .await?;
    Ok(true)
}

fn runtime_facts_require_session_invalidation(facts: &AgentExecutionRuntimeFacts) -> bool {
    matches!(
        &facts.failure_kind,
        Some(AgentFailureKind::MissingRequiredOutputs | AgentFailureKind::ToolOutputBudgetExceeded)
    ) || facts.output_settlement == AgentOutputSettlement::MissingRequiredOutputs
        || facts
            .supervision_classification
            .as_deref()
            .is_some_and(|classification| {
                classification.contains("tool_output_budget_exceeded")
                    || classification.contains("codex_unbounded_tool_output")
            })
}

async fn active_generation_has_missing_required_outputs_failure(
    pool: &SqlitePool,
    generation_id: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT 1
           FROM agent_executions ae
           INNER JOIN agent_execution_runtime_facts facts
             ON facts.agent_execution_id = ae.id
           WHERE ae.session_generation_id = ?1
             AND ae.status = 'failed'
             AND (
               facts.failure_kind = 'missing_required_outputs'
               OR facts.output_settlement = 'missing_required_outputs'
             )
           LIMIT 1"#,
    )
    .bind(generation_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

async fn active_generation_has_missing_required_outputs_failure_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation_id: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT 1
           FROM agent_executions ae
           INNER JOIN agent_execution_runtime_facts facts
             ON facts.agent_execution_id = ae.id
           WHERE ae.session_generation_id = ?1
             AND ae.status = 'failed'
             AND (
               facts.failure_kind = 'missing_required_outputs'
               OR facts.output_settlement = 'missing_required_outputs'
             )
           LIMIT 1"#,
    )
    .bind(generation_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.is_some())
}

async fn active_generation_has_tool_output_budget_failure(
    pool: &SqlitePool,
    generation_id: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT 1
           FROM agent_executions ae
           INNER JOIN agent_execution_runtime_facts facts
             ON facts.agent_execution_id = ae.id
           WHERE ae.session_generation_id = ?1
             AND ae.status = 'failed'
             AND (
               facts.failure_kind = 'tool_output_budget_exceeded'
               OR COALESCE(facts.supervision_classification, '') LIKE '%tool_output_budget_exceeded%'
               OR COALESCE(facts.supervision_classification, '') LIKE '%codex_unbounded_tool_output%'
               OR COALESCE(facts.failure_kind_raw_debug, '') LIKE '%tool_output_budget_exceeded%'
               OR COALESCE(facts.failure_kind_raw_debug, '') LIKE '%codex_unbounded_tool_output%'
               OR COALESCE(facts.failure_message_redacted, '') LIKE '%tool_output_budget_exceeded%'
               OR COALESCE(facts.failure_message_redacted, '') LIKE '%codex_unbounded_tool_output%'
             )
           LIMIT 1"#,
    )
    .bind(generation_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

async fn active_generation_has_tool_output_budget_failure_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation_id: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT 1
           FROM agent_executions ae
           INNER JOIN agent_execution_runtime_facts facts
             ON facts.agent_execution_id = ae.id
           WHERE ae.session_generation_id = ?1
             AND ae.status = 'failed'
             AND (
               facts.failure_kind = 'tool_output_budget_exceeded'
               OR COALESCE(facts.supervision_classification, '') LIKE '%tool_output_budget_exceeded%'
               OR COALESCE(facts.supervision_classification, '') LIKE '%codex_unbounded_tool_output%'
               OR COALESCE(facts.failure_kind_raw_debug, '') LIKE '%tool_output_budget_exceeded%'
               OR COALESCE(facts.failure_kind_raw_debug, '') LIKE '%codex_unbounded_tool_output%'
               OR COALESCE(facts.failure_message_redacted, '') LIKE '%tool_output_budget_exceeded%'
               OR COALESCE(facts.failure_message_redacted, '') LIKE '%codex_unbounded_tool_output%'
             )
           LIMIT 1"#,
    )
    .bind(generation_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.is_some())
}

async fn active_generation_has_claude_long_context_credits_failure(
    pool: &SqlitePool,
    generation_id: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT 1
           FROM agent_executions ae
           INNER JOIN agent_execution_runtime_facts facts
             ON facts.agent_execution_id = ae.id
           WHERE ae.session_generation_id = ?1
             AND ae.status = 'failed'
             AND ae.provider IN ('claude', 'claude_acp')
             AND facts.failure_kind = 'provider_quota'
             AND (
               COALESCE(facts.failure_kind_raw_debug, '') LIKE '%claude_long_context_credits_required%'
               OR COALESCE(facts.failure_kind_raw_debug, '') LIKE '%Usage credits are required for long context requests%'
               OR COALESCE(facts.failure_message_redacted, '') LIKE '%Usage credits are required for long context requests%'
             )
           LIMIT 1"#,
    )
    .bind(generation_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

async fn active_generation_has_claude_long_context_credits_failure_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation_id: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT 1
           FROM agent_executions ae
           INNER JOIN agent_execution_runtime_facts facts
             ON facts.agent_execution_id = ae.id
           WHERE ae.session_generation_id = ?1
             AND ae.status = 'failed'
             AND ae.provider IN ('claude', 'claude_acp')
             AND facts.failure_kind = 'provider_quota'
             AND (
               COALESCE(facts.failure_kind_raw_debug, '') LIKE '%claude_long_context_credits_required%'
               OR COALESCE(facts.failure_kind_raw_debug, '') LIKE '%Usage credits are required for long context requests%'
               OR COALESCE(facts.failure_message_redacted, '') LIKE '%Usage credits are required for long context requests%'
             )
           LIMIT 1"#,
    )
    .bind(generation_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.is_some())
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
    use db::repos::{agent_execution_runtime_facts, agent_executions, ideas, runs, sessions};
    use domain::agent::{
        AgentExecution, AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement,
        AgentStatus,
    };
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{AgentExecutionId, IdeaId, RunId};
    use domain::run::{Run, RunStatus};
    use domain::session::{
        SessionEventType, SessionGeneration, SessionGenerationStatus, SessionLineage,
        SessionReuseDisposition,
    };

    use super::{
        ensure_policy, invalidate_generation_after_missing_required_outputs,
        runtime_facts_require_session_invalidation, SessionPolicyInput,
    };

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed");
        let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .expect("test shared DbWriter registration failed");
        pool
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
    async fn p082_duplicate_active_session_generations_converge_to_single_survivor() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-duplicate-active".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: Some("generation-survivor".into()),
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        for (id, generation) in [("generation-duplicate", 1), ("generation-survivor", 2)] {
            sessions::insert_generation(
                &pool,
                &SessionGeneration {
                    id: id.into(),
                    lineage_id: lineage.id.clone(),
                    generation,
                    invocation_owner_key: "owner".into(),
                    provider_session_id: Some(format!("provider-{generation}")),
                    binding_fingerprint: "fingerprint".into(),
                    rehydrated_from_checkpoint_artifact_id: None,
                    working_directory: "/tmp/ws".into(),
                    workspace_mode: "read_only".into(),
                    runtime_provider: "claude".into(),
                    runtime_model: "sonnet".into(),
                    status: SessionGenerationStatus::Active,
                    turn_count: generation,
                    estimated_input_tokens: 0,
                    latest_cached_input_tokens: None,
                    latest_output_tokens: None,
                    latest_model_context_window: None,
                    cumulative_prompt_tokens: 0,
                    cumulative_cost_cents: 0,
                    created_at: now + chrono::Duration::seconds(generation),
                    last_activity_at: None,
                    ended_at: None,
                    end_reason: None,
                },
            )
            .await
            .unwrap();
        }

        let decision = ensure_policy(&pool, base_input()).await.unwrap();

        assert_eq!(decision.disposition, SessionReuseDisposition::Reused);
        assert_eq!(decision.generation.id, "generation-survivor");
        let duplicate = sessions::find_generation_by_id(&pool, "generation-duplicate")
            .await
            .unwrap()
            .expect("duplicate generation still exists as durable evidence");
        assert_eq!(duplicate.status, SessionGenerationStatus::Invalidated);
        assert_eq!(
            duplicate.end_reason.as_deref(),
            Some("duplicate_active_session_owner_rejected")
        );
        let active_count = sessions::list_generations_for_lineage(&pool, &lineage.id)
            .await
            .unwrap()
            .into_iter()
            .filter(|generation| generation.status == SessionGenerationStatus::Active)
            .count();
        assert_eq!(active_count, 1);
        assert_eq!(
            sessions::count_generation_events(
                &pool,
                "generation-duplicate",
                SessionEventType::Invalidated,
            )
            .await
            .unwrap(),
            1,
            "duplicate rejection must leave session event evidence"
        );
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
    async fn missing_required_outputs_failure_invalidates_generation_before_reuse() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-missing-outputs".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: Some("generation-missing-outputs".into()),
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        sessions::insert_generation(
            &pool,
            &SessionGeneration {
                id: "generation-missing-outputs".into(),
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
                status: SessionGenerationStatus::Active,
                turn_count: 2,
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

        let agent_execution_id = AgentExecutionId::new();
        agent_executions::insert(
            &pool,
            &AgentExecution {
                id: agent_execution_id,
                stage_execution_id: None,
                agent_id: "proposal_writer".into(),
                provider: "claude".into(),
                model: Some("sonnet".into()),
                started_at: now,
                completed_at: Some(now),
                status: AgentStatus::Failed,
                owner_execution_lineage_id: Some("owner".into()),
                session_lineage_id: Some(lineage.id.clone()),
                session_generation_id: Some("generation-missing-outputs".into()),
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: Some("owner".into()),
                session_reuse_scope: Some("same_agent_family_within_run".into()),
                session_family_id: Some("proposal-loop".into()),
                session_reuse_disposition: Some("reused".into()),
                session_reset_reason: None,
                backend_profile_id: None,
                requested_mcp_extensions_json: None,
                predicted_mcp_extensions_json: None,
                predicted_mcp_runtime_ids_json: None,
                actual_mcp_extensions_json: None,
                actual_mcp_runtime_ids_json: None,
                denied_mcp_extensions_json: None,
                mcp_blocking_issues_json: None,
                actual_mcp_observation_json: None,
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: None,
                owner_kind: Some("lead_conflict_mediation".into()),
                owner_id: Some("mediation-owner".into()),
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
                escalation_policy_id: None,
                escalation_policy_hash: None,
                escalation_tier_id: None,
                escalation_tier_kind_raw: None,
                escalation_trigger_raw: None,
                escalation_digest_version: None,
                escalation_ledger_id: None,
            },
        )
        .await
        .unwrap();
        let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
        facts.failure_kind = Some(AgentFailureKind::MissingRequiredOutputs);
        facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
        agent_execution_runtime_facts::upsert(&pool, &facts)
            .await
            .unwrap();

        let invalidated = invalidate_generation_after_missing_required_outputs(
            &pool,
            Some("generation-missing-outputs"),
            &facts,
        )
        .await
        .unwrap();
        assert!(invalidated);
        let prior = sessions::find_generation_by_id(&pool, "generation-missing-outputs")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(prior.status, SessionGenerationStatus::Invalidated);
        assert_eq!(
            prior.end_reason.as_deref(),
            Some("previous_missing_required_outputs")
        );

        let decision = ensure_policy(&pool, base_input()).await.unwrap();

        assert_eq!(
            decision.disposition,
            SessionReuseDisposition::FreshAfterInvalidation
        );
        assert_eq!(decision.generation.generation, 2);
        assert!(!decision.should_reuse_live_session);
    }

    #[test]
    fn tool_output_budget_failure_requires_session_invalidation() {
        let now = chrono::Utc::now();
        let mut facts = AgentExecutionRuntimeFacts::defaults_for(AgentExecutionId::new(), now);

        assert!(!runtime_facts_require_session_invalidation(&facts));

        facts.failure_kind = Some(AgentFailureKind::ToolOutputBudgetExceeded);
        assert!(runtime_facts_require_session_invalidation(&facts));

        facts.failure_kind = Some(AgentFailureKind::ToolOutputBudgetPreflightDenied);
        facts.supervision_classification = Some("tool_output_budget_preflight_denied".into());
        assert!(!runtime_facts_require_session_invalidation(&facts));

        facts.failure_kind = None;
        facts.supervision_classification = Some("codex_unbounded_tool_output".into());
        assert!(runtime_facts_require_session_invalidation(&facts));
    }

    #[tokio::test]
    async fn claude_long_context_credits_failure_invalidates_generation_before_reuse() {
        let pool = test_pool().await;
        seed_run(&pool).await;
        let now = chrono::Utc::now();
        let lineage = SessionLineage {
            id: "lineage-long-context-credits".into(),
            run_id: "00000000-0000-0000-0000-000000000001".into(),
            agent_id: "proposal_writer".into(),
            lineage_id: "proposal-loop".into(),
            session_reuse_scope: "same_agent_family_within_run".into(),
            session_family_id: Some("proposal-loop".into()),
            active_generation_id: Some("generation-long-context-credits".into()),
            created_at: now,
            closed_at: None,
        };
        sessions::insert_lineage(&pool, &lineage).await.unwrap();
        sessions::insert_generation(
            &pool,
            &SessionGeneration {
                id: "generation-long-context-credits".into(),
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
                status: SessionGenerationStatus::Active,
                turn_count: 2,
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

        let agent_execution_id = AgentExecutionId::new();
        agent_executions::insert(
            &pool,
            &AgentExecution {
                id: agent_execution_id,
                stage_execution_id: None,
                agent_id: "proposal_writer".into(),
                provider: "claude".into(),
                model: Some("sonnet".into()),
                started_at: now,
                completed_at: Some(now),
                status: AgentStatus::Failed,
                owner_execution_lineage_id: Some("owner".into()),
                session_lineage_id: Some(lineage.id.clone()),
                session_generation_id: Some("generation-long-context-credits".into()),
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: Some("owner".into()),
                session_reuse_scope: Some("same_agent_family_within_run".into()),
                session_family_id: Some("proposal-loop".into()),
                session_reuse_disposition: Some("reused".into()),
                session_reset_reason: None,
                backend_profile_id: None,
                requested_mcp_extensions_json: None,
                predicted_mcp_extensions_json: None,
                predicted_mcp_runtime_ids_json: None,
                actual_mcp_extensions_json: None,
                actual_mcp_runtime_ids_json: None,
                denied_mcp_extensions_json: None,
                mcp_blocking_issues_json: None,
                actual_mcp_observation_json: None,
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: None,
                owner_kind: Some("lead_conflict_mediation".into()),
                owner_id: Some("mediation-owner".into()),
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
                escalation_policy_id: None,
                escalation_policy_hash: None,
                escalation_tier_id: None,
                escalation_tier_kind_raw: None,
                escalation_trigger_raw: None,
                escalation_digest_version: None,
                escalation_ledger_id: None,
            },
        )
        .await
        .unwrap();
        let mut facts = AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
        facts.failure_kind = Some(AgentFailureKind::ProviderQuota);
        facts.failure_kind_raw_debug = Some("claude_long_context_credits_required".to_string());
        facts.failure_message_redacted =
            Some("Usage credits are required for long context requests.".to_string());
        agent_execution_runtime_facts::upsert(&pool, &facts)
            .await
            .unwrap();

        let decision = ensure_policy(&pool, base_input()).await.unwrap();

        let prior = sessions::find_generation_by_id(&pool, "generation-long-context-credits")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(prior.status, SessionGenerationStatus::Invalidated);
        assert_eq!(
            prior.end_reason.as_deref(),
            Some("claude_long_context_credits_required")
        );
        assert_eq!(
            decision.disposition,
            SessionReuseDisposition::FreshAfterInvalidation
        );
        assert_eq!(
            decision.session_reset_reason.as_deref(),
            Some("claude_long_context_credits_required")
        );
        assert_eq!(decision.generation.generation, 2);
        assert!(!decision.should_reuse_live_session);
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
