/// P058 escalation tier selector.
///
/// Classifies an escalation trigger from an agent execution's failure kind and
/// records what tier the scheduler selects in the would_select_* compatibility
/// columns and the durable escalation ledger.
///
/// The write is best-effort relative to primary settlement, but when it succeeds
/// it is production truth: the next scheduler pass uses the advanced ledger tier.
///
/// Shadow writes are best-effort observability: failures are logged but never
/// propagated to callers.
use anyhow::Result;
use chrono::{DateTime, Utc};
use db::repos::escalation as escalation_repo;
use domain::agent::AgentFailureKind;
use domain::escalation::EscalationEvent;
use domain::ids::AgentExecutionId;
use sqlx::SqlitePool;
use tracing::warn;

/// Classify an escalation trigger from an agent execution's failure kind.
///
/// Maps `AgentFailureKind` to the P058 trigger vocabulary.  Returns `None` for
/// non-escalatable outcomes (operator cancellation, supersession, permission
/// stalls) so the shadow writer skips those executions entirely.
pub fn classify_trigger_from_failure_kind(
    failure_kind: Option<&AgentFailureKind>,
) -> Option<&'static str> {
    match failure_kind? {
        AgentFailureKind::ProviderQuota => Some("provider_quota_exhausted"),
        AgentFailureKind::TransportEpipe
        | AgentFailureKind::TransportProtocolError
        | AgentFailureKind::TransportClosed
        | AgentFailureKind::McpStartupTimeout => Some("transport_failure"),
        AgentFailureKind::MissingRequiredOutputs => Some("contract_output_failure"),
        AgentFailureKind::InvalidOutputContract => Some("contract_output_failure"),
        AgentFailureKind::ProviderInternalError
        | AgentFailureKind::ProviderTimeout
        | AgentFailureKind::XcodeHostEnvironmentError => Some("stale_no_output"),
        // Non-escalatable outcomes — no shadow selection written.
        AgentFailureKind::CancelledByOperator
        | AgentFailureKind::SupersededByRetry
        | AgentFailureKind::ProviderPermissionRequired
        | AgentFailureKind::ProviderPermissionRejected
        | AgentFailureKind::HostInterruption
        | AgentFailureKind::McpPermissionModalStall
        | AgentFailureKind::ToolOutputBudgetExceeded
        | AgentFailureKind::Unknown => None,
    }
}

/// Find the next tier in the policy's tier list after `current_tier_id`.
/// Returns `None` when `current_tier_id` is not found or is the last tier.
fn find_next_tier<'a>(
    tiers: &'a [workflow::plan::EscalationTierSnapshot],
    current_tier_id: &str,
) -> Option<&'a workflow::plan::EscalationTierSnapshot> {
    let pos = tiers.iter().position(|t| t.tier_id == current_tier_id)?;
    tiers.get(pos + 1)
}

/// P058: Write escalation selection for a completed agent execution.
///
/// Best-effort — all errors are logged and silently swallowed so scheduler readback
/// failures never affect the primary execution path.
pub async fn try_write_shadow_escalation(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
    failure_kind: Option<&AgentFailureKind>,
    completed_at: DateTime<Utc>,
) {
    if let Err(e) =
        write_shadow_escalation_inner(pool, agent_execution_id, failure_kind, completed_at).await
    {
        warn!(
            agent_execution_id = %agent_execution_id,
            error = %e,
            "P058 escalation scheduler write failed (non-blocking to primary execution)"
        );
    }
}

async fn write_shadow_escalation_inner(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
    failure_kind: Option<&AgentFailureKind>,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let agent_exec_id_str = agent_execution_id.to_string();

    // 1. Look up escalation execution metadata — returns None when agent ran without
    //    escalation coverage (no policy matched at execution start).
    let Some(meta) =
        escalation_repo::find_execution_metadata_for_agent(pool, &agent_exec_id_str).await?
    else {
        return Ok(());
    };

    // 2. Look up the escalation ledger to get the policy_id and run_id.
    let Some(ledger) = escalation_repo::find_ledger_by_id(pool, &meta.escalation_ledger_id).await?
    else {
        return Ok(());
    };

    // 3. Compile the frozen run plan to access the policy snapshot.
    let Some(run) = db::repos::runs::find_by_id(pool, ledger.run_id).await? else {
        return Ok(());
    };
    let Some(plan) = crate::command_handler::compile_run_plan_from_snapshot(&run)? else {
        return Ok(());
    };

    // 4. Find the policy by policy_id from the ledger.
    let Some(policy) = plan
        .escalation_policies
        .iter()
        .find(|p| p.policy_id == ledger.policy_id)
    else {
        return Ok(());
    };

    // 5. Classify trigger from failure kind — skip non-escalatable outcomes.
    let Some(trigger_raw) = classify_trigger_from_failure_kind(failure_kind) else {
        return Ok(());
    };

    // 6. Verify the trigger is in this policy's trigger list.
    if !policy.triggers.iter().any(|t| t.as_str() == trigger_raw) {
        return Ok(());
    }

    // 7. Find the next tier after the current execution's tier.
    let Some(next_tier) = find_next_tier(&policy.tiers, &meta.tier_id) else {
        return Ok(());
    };

    let mut next_ledger = ledger.clone();
    next_ledger.current_tier_id = Some(next_tier.tier_id.clone());
    next_ledger.current_tier_kind_raw = Some(next_tier.kind.clone());
    next_ledger.trigger_raw = Some(trigger_raw.to_string());
    next_ledger.chain_attempt_index += 1;
    next_ledger.updated_at = completed_at;
    if next_tier.kind == "pause" {
        next_ledger.status_raw = "paused".to_string();
        next_ledger.pause_reason_raw = Some("escalation_chain_exhausted".to_string());
        next_ledger.operator_action_hint =
            Some("Extend the chain or accept terminal pause.".to_string());
        next_ledger.runbook_anchor = Some("escalation/chain-exhausted".to_string());
    }

    // 8. Build shadow decision JSON (keys restricted by escalation repo validator).
    let decision_json = serde_json::json!({
        "tier_id": next_tier.tier_id,
        "trigger_raw": trigger_raw,
        "tier_kind_raw": next_tier.kind,
        "policy_id": policy.policy_id,
        "policy_hash": policy.policy_hash,
        "chain_attempt_index": next_ledger.chain_attempt_index,
        "decision_reason": "scheduler_trigger_classifier",
        "timestamp_utc": completed_at.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    });
    let decision_json_str = serde_json::to_string(&decision_json)?;

    let mut tx = pool.begin().await?;
    escalation_repo::update_shadow_escalation_columns_tx(
        &mut tx,
        &agent_exec_id_str,
        Some(&next_tier.tier_id),
        Some(trigger_raw),
        Some(&decision_json_str),
    )
    .await?;

    escalation_repo::update_ledger_tx(&mut tx, &next_ledger).await?;

    let payload_json = serde_json::json!({
        "digest_inputs": {
            "failure_kind": failure_kind_digest_value(failure_kind),
            "output_settlement_state": output_settlement_state_for_trigger(trigger_raw),
            "validation_evidence_kind": "runtime_facts",
        },
        "redacted_evidence_ref": format!("ref/escalation/{}", agent_exec_id_str),
        "tier_id": next_tier.tier_id,
        "tier_kind_raw": next_tier.kind,
        "trigger_raw": trigger_raw,
        "event_kind_raw": if next_tier.kind == "pause" {
            "escalation.chain_exhausted"
        } else {
            "escalation.tier_selected"
        },
        "policy_id": policy.policy_id,
        "chain_attempt_index": next_ledger.chain_attempt_index,
        "digest_version": "escalation_blocker_digest_v1",
    })
    .to_string();
    let event = EscalationEvent {
        id: uuid::Uuid::new_v4().to_string(),
        escalation_ledger_id: meta.escalation_ledger_id.clone(),
        event_kind_raw: if next_tier.kind == "pause" {
            "escalation.chain_exhausted".into()
        } else {
            "escalation.tier_selected".into()
        },
        tier_id: Some(next_tier.tier_id.clone()),
        tier_kind_raw: Some(next_tier.kind.clone()),
        trigger_raw: Some(trigger_raw.to_string()),
        pause_reason_raw: (next_tier.kind == "pause")
            .then(|| "escalation_chain_exhausted".to_string()),
        payload_json: Some(payload_json),
        redaction_version: Some("redaction_v1".into()),
        created_at: completed_at,
    };
    escalation_repo::insert_event_tx(&mut tx, &event).await?;
    tx.commit().await?;

    Ok(())
}

fn failure_kind_digest_value(failure_kind: Option<&AgentFailureKind>) -> &'static str {
    match failure_kind {
        Some(AgentFailureKind::ProviderQuota) => "provider_quota",
        Some(AgentFailureKind::TransportEpipe) => "transport_epipe",
        Some(AgentFailureKind::TransportProtocolError) => "transport_protocol_error",
        Some(AgentFailureKind::TransportClosed) => "transport_closed",
        Some(AgentFailureKind::McpStartupTimeout) => "mcp_startup_timeout",
        Some(AgentFailureKind::MissingRequiredOutputs) => "missing_required_outputs",
        Some(AgentFailureKind::InvalidOutputContract) => "invalid_output_contract",
        Some(AgentFailureKind::ProviderInternalError) => "provider_internal_error",
        Some(AgentFailureKind::ToolOutputBudgetExceeded) => "tool_output_budget_exceeded",
        Some(AgentFailureKind::ProviderTimeout) => "provider_timeout",
        Some(AgentFailureKind::XcodeHostEnvironmentError) => "xcode_host_environment_error",
        Some(AgentFailureKind::CancelledByOperator) => "cancelled_by_operator",
        Some(AgentFailureKind::SupersededByRetry) => "superseded_by_retry",
        Some(AgentFailureKind::ProviderPermissionRequired) => "provider_permission_required",
        Some(AgentFailureKind::ProviderPermissionRejected) => "provider_permission_rejected",
        Some(AgentFailureKind::HostInterruption) => "host_interruption",
        Some(AgentFailureKind::McpPermissionModalStall) => "mcp_permission_modal_stall",
        Some(AgentFailureKind::Unknown) | None => "unknown",
    }
}

fn output_settlement_state_for_trigger(trigger_raw: &str) -> &'static str {
    match trigger_raw {
        "contract_output_failure" => "invalid_or_missing",
        "stale_no_output" => "missing",
        "provider_quota_exhausted" => "quota_wait",
        "transport_failure" => "transport_failed",
        "repeated_same_blocker_digest" => "no_progress",
        "loop_budget_threshold" => "loop_threshold",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use db::repos::{
        agent_execution_runtime_facts, agent_executions, escalation as escalation_repo, ideas,
        runs, stages,
    };
    use db::writer::{register_shared_writer, DbWriter};
    use domain::agent::AgentFailureKind;
    use domain::agent::{
        AgentExecution, AgentExecutionRuntimeFacts, AgentOutputSettlement, AgentStatus,
    };
    use domain::escalation::{EscalationExecutionMetadata, EscalationLedger};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{IdeaId, RunId, StageExecutionId};
    use domain::run::{Run, RunStatus};
    use domain::stage::{StageExecution, StageStatus};
    use std::sync::Arc;
    use workflow::plan::EscalationTierSnapshot;

    // ── Trigger classifier tests ───────────────────────────────────────────────

    #[test]
    fn p058_shadow_trigger_quota_maps_to_provider_quota_exhausted() {
        assert_eq!(
            classify_trigger_from_failure_kind(Some(&AgentFailureKind::ProviderQuota)),
            Some("provider_quota_exhausted")
        );
    }

    #[test]
    fn p058_shadow_trigger_transport_failures_map_to_transport_failure() {
        for kind in [
            AgentFailureKind::TransportEpipe,
            AgentFailureKind::TransportProtocolError,
            AgentFailureKind::TransportClosed,
            AgentFailureKind::McpStartupTimeout,
        ] {
            assert_eq!(
                classify_trigger_from_failure_kind(Some(&kind)),
                Some("transport_failure"),
                "Expected transport_failure for {kind:?}"
            );
        }
    }

    #[test]
    fn p058_shadow_trigger_contract_failures_map_to_contract_output_failure() {
        for kind in [
            AgentFailureKind::MissingRequiredOutputs,
            AgentFailureKind::InvalidOutputContract,
        ] {
            assert_eq!(
                classify_trigger_from_failure_kind(Some(&kind)),
                Some("contract_output_failure"),
                "Expected contract_output_failure for {kind:?}"
            );
        }
    }

    #[test]
    fn p058_shadow_trigger_provider_errors_map_to_stale_no_output() {
        for kind in [
            AgentFailureKind::ProviderInternalError,
            AgentFailureKind::ProviderTimeout,
            AgentFailureKind::XcodeHostEnvironmentError,
        ] {
            assert_eq!(
                classify_trigger_from_failure_kind(Some(&kind)),
                Some("stale_no_output"),
                "Expected stale_no_output for {kind:?}"
            );
        }
    }

    #[test]
    fn p058_shadow_trigger_non_escalatable_kinds_return_none() {
        for kind in [
            AgentFailureKind::CancelledByOperator,
            AgentFailureKind::SupersededByRetry,
            AgentFailureKind::ProviderPermissionRequired,
            AgentFailureKind::ProviderPermissionRejected,
            AgentFailureKind::HostInterruption,
            AgentFailureKind::McpPermissionModalStall,
            AgentFailureKind::Unknown,
        ] {
            assert_eq!(
                classify_trigger_from_failure_kind(Some(&kind)),
                None,
                "Expected None for non-escalatable {kind:?}"
            );
        }
    }

    #[test]
    fn p058_shadow_trigger_no_failure_kind_returns_none() {
        assert_eq!(classify_trigger_from_failure_kind(None), None);
    }

    // ── Next-tier resolution tests ─────────────────────────────────────────────

    fn make_tier(id: &str, kind: &str) -> EscalationTierSnapshot {
        EscalationTierSnapshot {
            tier_id: id.into(),
            kind: kind.into(),
            backend_profile_id: None,
            max_attempts: None,
        }
    }

    #[test]
    fn p058_shadow_next_tier_returns_second_when_first_is_current() {
        let tiers = vec![
            make_tier("primary_retry", "same_backend_retry"),
            make_tier("frontier_profile", "backend_profile"),
            make_tier("human_pause", "pause"),
        ];
        let next = find_next_tier(&tiers, "primary_retry");
        assert_eq!(
            next.map(|t| t.tier_id.as_str()),
            Some("frontier_profile"),
            "Next tier after primary_retry should be frontier_profile"
        );
    }

    #[test]
    fn p058_shadow_next_tier_returns_none_for_last_tier() {
        let tiers = vec![
            make_tier("primary", "same_backend_retry"),
            make_tier("human_pause", "pause"),
        ];
        assert!(
            find_next_tier(&tiers, "human_pause").is_none(),
            "No next tier after the last tier"
        );
    }

    #[test]
    fn p058_shadow_next_tier_returns_none_for_unknown_tier_id() {
        let tiers = vec![make_tier("primary", "same_backend_retry")];
        assert!(
            find_next_tier(&tiers, "nonexistent_tier").is_none(),
            "Unknown tier_id should return None"
        );
    }

    #[test]
    fn p058_shadow_next_tier_returns_none_for_empty_tier_list() {
        assert!(find_next_tier(&[], "any_tier").is_none());
    }

    #[tokio::test]
    async fn proposal_058_scheduler_selection_writes_would_select_and_advances_ledger() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        register_shared_writer(&pool, Arc::new(DbWriter::new(pool.clone())))
            .await
            .unwrap();

        let now = chrono::DateTime::parse_from_rfc3339("2026-05-25T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let stage_execution_id = StageExecutionId::new();
        let agent_execution_id = AgentExecutionId::new();

        ideas::insert(
            &pool,
            &Idea {
                id: idea_id,
                title: "P058 durable tier advancement".into(),
                body: "runtime scheduler proof".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: now,
                archived_at: None,
            },
        )
        .await
        .unwrap();

        let workflow_json = r#"{
            "initial_state": "impl_state",
            "workflow": {"id": "p058_scheduler_proof"},
            "states": {
                "impl_state": {
                    "label": "Implementation",
                    "owner": "impl_agent",
                    "type": "end"
                }
            }
        }"#;
        let catalog_json = r#"{
            "backend_profiles": {
                "impl_profile": {"provider": "claude"},
                "fallback_profile": {"provider": "codex"},
                "lead_profile": {"provider": "claude"}
            },
            "permission_profiles": {
                "lead_perm": {}
            },
            "contracts": {
                "lead_contract": {"format": "json"}
            },
            "agents": [
                {"id": "impl_agent", "backend_profile": "impl_profile"},
                {
                    "id": "lead_agent",
                    "system_role": "lead",
                    "backend_profile": "lead_profile",
                    "permission_profile": "lead_perm",
                    "lead_resolution_contract": "lead_contract"
                }
            ],
            "escalation_policies": [
                {
                    "policy_id": "impl_escalation",
                    "schema_version": "escalation_policy_v1",
                    "enabled_default": true,
                    "applies_to": {"agent_id": "impl_agent"},
                    "max_chain_attempts": 3,
                    "max_chain_wall_clock_seconds": 1800,
                    "triggers": ["contract_output_failure"],
                    "tiers": [
                        {"tier_id": "retry_tier", "kind": "same_backend_retry", "max_attempts": 1},
                        {"tier_id": "fallback_tier", "kind": "backend_profile", "backend_profile_id": "fallback_profile", "max_attempts": 1}
                    ]
                }
            ]
        }"#;

        runs::insert(
            &pool,
            &Run {
                id: run_id,
                idea_id,
                status: RunStatus::Running,
                workflow_id: "p058_scheduler_proof".into(),
                workflow_title: "P058 scheduler proof".into(),
                workspace_root: "/tmp".into(),
                artifact_root: "/tmp".into(),
                started_at: now,
                completed_at: None,
                cancellation_requested_at: None,
                cancellation_settled_at: None,
                cancellation_settlement_log: None,
                current_state: Some("impl_state".into()),
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
                workflow_snapshot_json: Some(workflow_json.into()),
                catalog_snapshot_json: Some(catalog_json.into()),
                drift_detected_at: None,
                drift_details_json: None,
                chainworks_meta_root: None,
                review_routing_json: None,
                closeout_readiness_mode: None,
            },
        )
        .await
        .unwrap();

        stages::insert(
            &pool,
            &StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "impl_state".into(),
                label: "Implementation".into(),
                status: StageStatus::Running,
                iteration: 1,
                attempt_number: 1,
                settlement_kind: None,
                started_at: now,
                completed_at: None,
                owner_agent: None,
                provider: None,
                model: None,
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();

        agent_executions::insert(
            &pool,
            &AgentExecution {
                id: agent_execution_id,
                stage_execution_id: Some(stage_execution_id),
                agent_id: "impl_agent".into(),
                provider: "claude".into(),
                model: Some("sonnet".into()),
                status: AgentStatus::Failed,
                started_at: now,
                completed_at: Some(now),
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: None,
                session_reset_reason: None,
                backend_profile_id: Some("impl_profile".into()),
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
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
                escalation_policy_id: Some("impl_escalation".into()),
                escalation_policy_hash: Some("sha256:testpolicy".into()),
                escalation_tier_id: Some("retry_tier".into()),
                escalation_tier_kind_raw: Some("same_backend_retry".into()),
                escalation_trigger_raw: None,
                escalation_digest_version: Some("escalation_blocker_digest_v1".into()),
                escalation_ledger_id: Some("ledger-runtime-advance".into()),
            },
        )
        .await
        .unwrap();

        escalation_repo::insert_ledger(
            &pool,
            &EscalationLedger {
                id: "ledger-runtime-advance".into(),
                run_id,
                stage_id: "impl_state".into(),
                agent_id: "impl_agent".into(),
                policy_id: "impl_escalation".into(),
                policy_hash: "sha256:testpolicy".into(),
                status_raw: "active".into(),
                current_tier_id: Some("retry_tier".into()),
                current_tier_kind_raw: Some("same_backend_retry".into()),
                chain_attempt_index: 0,
                trigger_raw: None,
                pause_reason_raw: None,
                operator_action_hint: None,
                runbook_anchor: None,
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        escalation_repo::insert_execution_metadata(
            &pool,
            &EscalationExecutionMetadata {
                agent_execution_id,
                escalation_ledger_id: "ledger-runtime-advance".into(),
                tier_id: "retry_tier".into(),
                tier_kind_raw: "same_backend_retry".into(),
                tier_attempt_index: 0,
                trigger_raw: None,
                digest_version: Some("escalation_blocker_digest_v1".into()),
                capacity_probe_counter: 0,
                created_at: now,
                updated_at: now,
                would_select_tier_id: None,
                would_select_trigger_raw: None,
                would_select_decision_json: None,
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

        write_shadow_escalation_inner(
            &pool,
            agent_execution_id,
            Some(&AgentFailureKind::MissingRequiredOutputs),
            now,
        )
        .await
        .unwrap();

        // Production tier selection advances the durable ledger. The shadow columns remain
        // as compatibility readback, but they are no longer the only source of truth.
        let ledger_after = escalation_repo::find_ledger_by_id(&pool, "ledger-runtime-advance")
            .await
            .unwrap()
            .expect("ledger must still exist");
        assert_eq!(
            ledger_after.current_tier_id.as_deref(),
            Some("fallback_tier"),
            "P058 must advance the durable ledger tier"
        );
        assert_eq!(
            ledger_after.current_tier_kind_raw.as_deref(),
            Some("backend_profile"),
            "P058 must update the durable ledger tier kind"
        );
        assert_eq!(
            ledger_after.trigger_raw.as_deref(),
            Some("contract_output_failure"),
            "P058 must stamp the durable trigger_raw on the ledger"
        );
        assert_eq!(
            ledger_after.chain_attempt_index, 1,
            "P058 must increment chain_attempt_index"
        );
        assert_eq!(ledger_after.status_raw, "active");

        // Shadow columns ARE written.
        let meta = escalation_repo::find_execution_metadata_for_agent(
            &pool,
            &agent_execution_id.to_string(),
        )
        .await
        .unwrap()
        .expect("metadata remains readable");
        assert_eq!(meta.would_select_tier_id.as_deref(), Some("fallback_tier"));
        assert_eq!(
            meta.would_select_trigger_raw.as_deref(),
            Some("contract_output_failure")
        );

        // The event journal now records production tier selection.
        let events = escalation_repo::find_events_by_ledger(&pool, "ledger-runtime-advance")
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].event_kind_raw, "escalation.tier_selected",
            "P058 must emit production tier selection"
        );
        assert_eq!(events[0].tier_id.as_deref(), Some("fallback_tier"));
        let payload: serde_json::Value =
            serde_json::from_str(events[0].payload_json.as_deref().unwrap()).unwrap();
        assert_eq!(
            payload
                .pointer("/digest_inputs/failure_kind")
                .and_then(|value| value.as_str()),
            Some("missing_required_outputs")
        );
        assert_eq!(
            payload
                .pointer("/digest_inputs/output_settlement_state")
                .and_then(|value| value.as_str()),
            Some("invalid_or_missing")
        );
        assert_eq!(
            payload
                .pointer("/redacted_evidence_ref")
                .and_then(|value| value.as_str()),
            Some(format!("ref/escalation/{agent_execution_id}").as_str())
        );
    }
}
