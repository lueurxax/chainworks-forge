use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use db::repos::{
    agent_executions, escalation, ideas, retry_operator_instructions,
    retry_stage_execution_authorities, runs, stages, work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::commands::{ResumeEscalationChainCmd, ResumeEscalationDeadlineCmd};
use domain::escalation::{EscalationDeadlineWindow, EscalationEvent};
use domain::ids::{AgentExecutionId, StageExecutionId};
use domain::run::RunStatus;
use domain::stage::{StageExecution, StageStatus};
use sha2::{Digest, Sha256};
use sqlx::{Row, Sqlite, Transaction};

#[derive(Clone, Debug)]
pub struct ResumeEscalationDeadlineOutcome {
    pub run_id: domain::ids::RunId,
    pub escalation_ledger_id: String,
    pub deadline_window_id: String,
    pub retry_stage_execution_id: StageExecutionId,
    pub source_stage_execution_id: StageExecutionId,
    pub source_agent_execution_id: AgentExecutionId,
    pub work_item_id: String,
    pub tier_id: String,
    pub backend_profile_id: String,
    pub provider: String,
    pub starts_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub replayed: bool,
}

#[derive(Clone, Copy)]
enum ResumeEscalationRequest<'a> {
    Deadline(&'a ResumeEscalationDeadlineCmd),
    ExhaustedChain(&'a ResumeEscalationChainCmd),
}

impl<'a> ResumeEscalationRequest<'a> {
    fn run_id(self) -> domain::ids::RunId {
        match self {
            Self::Deadline(command) => command.run_id,
            Self::ExhaustedChain(command) => command.run_id,
        }
    }

    fn escalation_ledger_id(self) -> &'a str {
        match self {
            Self::Deadline(command) => &command.escalation_ledger_id,
            Self::ExhaustedChain(command) => &command.escalation_ledger_id,
        }
    }

    fn reason(self) -> &'a str {
        match self {
            Self::Deadline(command) => &command.reason,
            Self::ExhaustedChain(command) => &command.reason,
        }
    }

    fn idempotency_key(self) -> &'a str {
        match self {
            Self::Deadline(command) => &command.idempotency_key,
            Self::ExhaustedChain(command) => &command.idempotency_key,
        }
    }

    fn operator_instruction(self) -> Option<&'a str> {
        match self {
            Self::Deadline(_) => None,
            Self::ExhaustedChain(command) => command.operator_instruction.as_deref(),
        }
    }

    fn target_tier_id(self, ledger: &domain::escalation::EscalationLedger) -> Result<String> {
        match self {
            Self::Deadline(_) => ledger
                .current_tier_id
                .clone()
                .context("P058_RESUME_TIER_MISSING: ledger has no current tier"),
            Self::ExhaustedChain(command) => Ok(command.target_tier_id.clone()),
        }
    }

    fn source_pause_reason(self) -> &'static str {
        match self {
            Self::Deadline(_) => "escalation_deadline_elapsed",
            Self::ExhaustedChain(_) => "escalation_chain_exhausted",
        }
    }

    fn id_prefix(self) -> &'static str {
        match self {
            Self::Deadline(_) => "p058-deadline-resume",
            Self::ExhaustedChain(_) => "p058-chain-resume",
        }
    }

    fn retry_reason_prefix(self) -> &'static str {
        match self {
            Self::Deadline(_) => "p058_deadline_resume",
            Self::ExhaustedChain(_) => "p058_chain_resume",
        }
    }

    fn targeted_retry_reason(self) -> &'static str {
        match self {
            Self::Deadline(_) => "p058_deadline_window_resume",
            Self::ExhaustedChain(_) => "p058_exhausted_chain_resume",
        }
    }

    fn provider_fallback_reason(self) -> &'static str {
        match self {
            Self::Deadline(_) => "p058_operator_deadline_resume",
            Self::ExhaustedChain(_) => "p058_operator_chain_resume",
        }
    }

    fn event_kind(self) -> &'static str {
        match self {
            Self::Deadline(_) => "escalation.deadline_window_resumed",
            Self::ExhaustedChain(_) => "escalation.chain_window_resumed",
        }
    }
}

pub async fn resume_escalation_deadline_tx(
    tx: &mut Transaction<'_, Sqlite>,
    command: &ResumeEscalationDeadlineCmd,
    principal_id: &str,
    command_journal_id: &str,
    now: DateTime<Utc>,
) -> Result<ResumeEscalationDeadlineOutcome> {
    resume_escalation_tx(
        tx,
        ResumeEscalationRequest::Deadline(command),
        principal_id,
        command_journal_id,
        now,
    )
    .await
}

pub async fn resume_escalation_chain_tx(
    tx: &mut Transaction<'_, Sqlite>,
    command: &ResumeEscalationChainCmd,
    principal_id: &str,
    command_journal_id: &str,
    now: DateTime<Utc>,
) -> Result<ResumeEscalationDeadlineOutcome> {
    resume_escalation_tx(
        tx,
        ResumeEscalationRequest::ExhaustedChain(command),
        principal_id,
        command_journal_id,
        now,
    )
    .await
}

async fn resume_escalation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    request: ResumeEscalationRequest<'_>,
    principal_id: &str,
    command_journal_id: &str,
    now: DateTime<Utc>,
) -> Result<ResumeEscalationDeadlineOutcome> {
    validate_request(request)?;
    let validated_instruction = request
        .operator_instruction()
        .map(domain::retry_instruction::validate_operator_instruction)
        .transpose()
        .map_err(|error| anyhow!("operator_instruction validation: {error}"))?;
    let run_id = request.run_id();
    let escalation_ledger_id = request.escalation_ledger_id();
    let idempotency_key = request.idempotency_key();
    let request_hash = resume_request_hash(request);

    if let Some(existing) =
        escalation::find_deadline_window_by_idempotency_key_tx(tx, idempotency_key).await?
    {
        if existing.escalation_ledger_id != escalation_ledger_id
            || existing.resume_request_hash != request_hash
        {
            bail!(
                "P058_RESUME_IDEMPOTENCY_CONFLICT: idempotency key is bound to a different recovery request"
            );
        }
        let ledger = escalation::find_ledger_by_id_tx(tx, &existing.escalation_ledger_id)
            .await?
            .context("P058_RESUME_LEDGER_NOT_FOUND: replay ledger is missing")?;
        if ledger.run_id != run_id {
            bail!("P058_RESUME_IDEMPOTENCY_CONFLICT: replay run_id does not match ledger");
        }
        return Ok(ResumeEscalationDeadlineOutcome {
            run_id: ledger.run_id,
            escalation_ledger_id: ledger.id,
            deadline_window_id: existing.id,
            retry_stage_execution_id: existing
                .retry_stage_execution_id
                .parse()
                .context("P058_RESUME_CORRUPT_WINDOW: retry_stage_execution_id")?,
            source_stage_execution_id: existing
                .source_stage_execution_id
                .parse()
                .context("P058_RESUME_CORRUPT_WINDOW: source stage is invalid")?,
            source_agent_execution_id: existing
                .source_agent_execution_id
                .parse()
                .context("P058_RESUME_CORRUPT_WINDOW: source agent execution is invalid")?,
            work_item_id: existing.work_item_id,
            tier_id: existing.tier_id,
            backend_profile_id: existing.target_backend_profile_id,
            provider: existing.target_provider,
            starts_at: existing.starts_at,
            expires_at: existing.expires_at,
            replayed: true,
        });
    }

    let mut ledger = escalation::find_ledger_by_id_tx(tx, escalation_ledger_id)
        .await?
        .context("P058_RESUME_LEDGER_NOT_FOUND: escalation ledger does not exist")?;
    if ledger.run_id != run_id {
        bail!("P058_RESUME_LEDGER_RUN_MISMATCH: ledger does not belong to run_id");
    }
    match request {
        ResumeEscalationRequest::Deadline(_) => {
            if ledger.status_raw != "paused"
                || ledger.pause_reason_raw.as_deref() != Some("escalation_deadline_elapsed")
            {
                bail!(
                    "P058_RESUME_REASON_NOT_ALLOWED: only a paused escalation_deadline_elapsed ledger may be resumed"
                );
            }
        }
        ResumeEscalationRequest::ExhaustedChain(_) => {
            if ledger.status_raw != "paused"
                || ledger.current_tier_kind_raw.as_deref() != Some("pause")
                || ledger.pause_reason_raw.as_deref() != Some("escalation_chain_exhausted")
            {
                bail!(
                    "P058_CHAIN_RESUME_REASON_NOT_ALLOWED: only a terminal paused escalation_chain_exhausted ledger may be resumed"
                );
            }
        }
    }

    let run = runs::find_by_id_tx(tx, run_id)
        .await?
        .context("P058_RESUME_RUN_NOT_FOUND: run does not exist")?;
    if run.status.is_terminal() || matches!(run.status, RunStatus::Cancelling) {
        bail!("P058_RESUME_RUN_TERMINAL: terminal or cancelling runs cannot be resumed");
    }
    let plan = crate::command_handler::compile_run_plan_from_snapshot(&run)?
        .context("P058_RESUME_FROZEN_PLAN_MISSING: run has no frozen plan snapshot")?;
    let policy = plan
        .escalation_policies
        .iter()
        .find(|policy| policy.policy_id == ledger.policy_id)
        .context("P058_RESUME_POLICY_MISSING: frozen escalation policy is absent")?;
    if policy.policy_hash != ledger.policy_hash {
        bail!("P058_RESUME_POLICY_DRIFT: frozen policy hash does not match ledger");
    }
    if policy.max_chain_wall_clock_seconds == 0 {
        bail!("P058_RESUME_WINDOW_DISABLED: policy has no bounded wall-clock window");
    }
    match request {
        ResumeEscalationRequest::Deadline(_)
            if ledger.chain_attempt_index >= i64::from(policy.max_chain_attempts) =>
        {
            bail!("P058_RESUME_CHAIN_EXHAUSTED: chain attempt budget is exhausted");
        }
        ResumeEscalationRequest::ExhaustedChain(_)
            if ledger.chain_attempt_index < i64::from(policy.max_chain_attempts) =>
        {
            bail!("P058_CHAIN_RESUME_NOT_EXHAUSTED: chain attempt budget is not exhausted");
        }
        _ => {}
    }

    let tier_id = request.target_tier_id(&ledger)?;
    let tier = policy
        .tiers
        .iter()
        .find(|tier| tier.tier_id == tier_id)
        .context("P058_RESUME_TIER_MISSING: current tier is absent from frozen policy")?;
    if matches!(request, ResumeEscalationRequest::Deadline(_))
        && ledger.current_tier_kind_raw.as_deref() != Some(tier.kind.as_str())
    {
        bail!("P058_RESUME_TIER_DRIFT: ledger tier kind does not match frozen policy");
    }
    if tier.kind != "backend_profile" {
        match request {
            ResumeEscalationRequest::Deadline(_) => bail!(
                "P058_RESUME_TIER_NOT_SUPPORTED: elapsed-deadline recovery requires a backend_profile current tier"
            ),
            ResumeEscalationRequest::ExhaustedChain(_) => bail!(
                "P058_CHAIN_RESUME_TIER_NOT_SUPPORTED: exhausted-chain recovery requires an explicit backend_profile tier"
            ),
        }
    }
    let backend_profile_id = tier
        .backend_profile_id
        .clone()
        .context("P058_RESUME_PROFILE_MISSING: current tier has no backend profile")?;

    let catalog: serde_json::Value = serde_json::from_str(
        run.catalog_snapshot_json
            .as_deref()
            .context("P058_RESUME_CATALOG_MISSING: frozen catalog snapshot is absent")?,
    )
    .context("P058_RESUME_CATALOG_INVALID: parse frozen catalog snapshot")?;
    let profile = catalog
        .get("backend_profiles")
        .and_then(|profiles| profiles.get(&backend_profile_id))
        .and_then(serde_json::Value::as_object)
        .context("P058_RESUME_PROFILE_MISSING: backend profile is absent from frozen catalog")?;
    let provider = profile
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .filter(|provider| !provider.trim().is_empty())
        .context("P058_RESUME_PROFILE_INVALID: backend profile has no provider")?
        .to_string();

    let (source_agent_execution_id, source_stage_execution_id, source_agent_id) =
        latest_failed_execution_for_ledger(tx, &ledger.id).await?;
    if source_agent_id != ledger.agent_id {
        bail!("P058_RESUME_EXECUTION_MISMATCH: failed execution agent does not match ledger");
    }
    let source_stage = stages::find_by_id_tx(tx, source_stage_execution_id)
        .await?
        .context("P058_RESUME_STAGE_NOT_FOUND: failed execution stage is missing")?;
    if source_stage.run_id != run_id || source_stage.stage_id != ledger.stage_id {
        bail!("P058_RESUME_STAGE_MISMATCH: failed execution stage does not match ledger");
    }

    let source_work_items = work_items::list_by_run_tx(tx, run_id).await?;
    let source_item = find_source_invoke_work_item(
        &source_work_items,
        source_stage_execution_id,
        &ledger.agent_id,
        source_agent_execution_id,
    )
    .context("P058_RESUME_SOURCE_WORK_MISSING: source InvokeAgent work item is absent")?;
    let mut retry_payload: serde_json::Value = serde_json::from_str(&source_item.payload_json)
        .context("P058_RESUME_SOURCE_WORK_INVALID: source payload is not valid JSON")?;
    let idea = ideas::find_by_id_tx(tx, run.idea_id)
        .await?
        .context("P058_RESUME_SOURCE_WORK_INVALID: durable Idea is missing")?;
    let source_execution = agent_executions::find_by_id_tx(tx, source_agent_execution_id)
        .await?
        .context("P058_RESUME_SOURCE_WORK_INVALID: durable source execution is missing")?;
    let mediation_truth = crate::agent_mission_context::p058_mediation_copy_truth_for_execution(
        &plan,
        &run,
        &ledger,
        &source_execution,
        &retry_payload,
    )?;
    crate::agent_mission_context::validate_persisted_v1_payload_prompt_with_copy_truth(
        &plan,
        &run,
        &idea,
        &retry_payload,
        mediation_truth.as_ref(),
    )
    .context("P058_RESUME_SOURCE_WORK_INVALID: source V1 prompt validation failed")?;
    let source_provider = retry_payload
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let source_backend_profile_id = retry_payload
        .get("backend_profile_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);

    let all_stages = stages::list_by_run_tx(tx, run_id).await?;
    let next_attempt_number = all_stages
        .iter()
        .filter(|stage| stage.stage_id == ledger.stage_id)
        .map(|stage| stage.attempt_number)
        .max()
        .unwrap_or(source_stage.attempt_number)
        + 1;
    let deadline_window_id = uuid::Uuid::now_v7().to_string();
    let retry_stage_execution_id = StageExecutionId::new();
    let work_item_id = format!("{}:{deadline_window_id}", request.id_prefix());
    let retry_authority_id = format!("p091-retry-authority:{retry_stage_execution_id}");
    let retry_stage = StageExecution {
        id: retry_stage_execution_id,
        run_id,
        stage_id: source_stage.stage_id.clone(),
        label: source_stage.label.clone(),
        status: StageStatus::Running,
        iteration: source_stage.iteration,
        attempt_number: next_attempt_number,
        settlement_kind: None,
        started_at: now,
        completed_at: None,
        owner_agent: source_stage.owner_agent.clone(),
        provider: Some(provider.clone()),
        model: profile
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        stage_type: source_stage.stage_type.clone(),
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: Some(format!(
            "{}:{}:{}",
            request.retry_reason_prefix(),
            ledger.id,
            deadline_window_id
        )),
    };

    let object = retry_payload
        .as_object_mut()
        .context("P058_RESUME_SOURCE_WORK_INVALID: source payload must be an object")?;
    for field in [
        "p058_claimed",
        "target_stage_execution_id",
        "source_stage_execution_id",
        "source_agent_execution_id",
        "source_work_item_id",
        "retry_authority_id",
        "operator_retry_instruction",
    ] {
        object.remove(field);
    }
    object.insert("run_id".into(), serde_json::json!(run_id.to_string()));
    object.insert("stage_id".into(), serde_json::json!(ledger.stage_id));
    object.insert(
        "stage_execution_id".into(),
        serde_json::json!(retry_stage_execution_id.to_string()),
    );
    object.insert(
        "target_stage_execution_id".into(),
        serde_json::json!(retry_stage_execution_id.to_string()),
    );
    object.insert(
        "retry_authority_id".into(),
        serde_json::json!(retry_authority_id.clone()),
    );
    object.insert("agent_id".into(), serde_json::json!(ledger.agent_id));
    object.insert("provider".into(), serde_json::json!(provider));
    object.insert(
        "backend_profile_id".into(),
        serde_json::json!(backend_profile_id),
    );
    object.insert(
        "model".into(),
        profile
            .get("model")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    );
    for optional in ["effort", "max_turns", "temperature"] {
        if let Some(value) = profile.get(optional).cloned() {
            object.insert(optional.to_string(), value);
        } else {
            object.remove(optional);
        }
    }
    object.insert(
        "targeted_retry".into(),
        serde_json::json!({
            "source_stage_execution_id": source_stage_execution_id.to_string(),
            "source_agent_execution_id": source_agent_execution_id.to_string(),
            "source_work_item_id": source_item.id,
            "retry_authority_id": retry_authority_id,
            "reason": request.targeted_retry_reason(),
            "escalation": {
                "ledger_id": ledger.id,
                "policy_id": ledger.policy_id,
                "tier_id": tier_id,
                "tier_kind_raw": tier.kind,
                "trigger_raw": ledger.trigger_raw,
                "chain_attempt_index": ledger.chain_attempt_index,
                "deadline_window_id": deadline_window_id,
            },
            "provider_fallback": {
                "reason": request.provider_fallback_reason(),
                "from_backend_profile_id": source_backend_profile_id,
                "from_provider": source_provider,
                "to_backend_profile_id": backend_profile_id,
                "to_provider": provider,
            }
        }),
    );

    crate::orchestrator::append_current_proposal_writer_backlog_context(
        &plan,
        &run,
        &ledger.agent_id,
        &mut retry_payload,
    )?;

    let previous_window =
        escalation::find_latest_deadline_window_by_ledger_tx(tx, &ledger.id).await?;
    let wall_clock_seconds = i64::try_from(policy.max_chain_wall_clock_seconds)
        .map_err(|_| anyhow!("P058_RESUME_WINDOW_INVALID: policy duration is too large"))?;
    let source_deadline_at = match request {
        ResumeEscalationRequest::Deadline(_) => {
            let source_deadline_at = previous_window
                .as_ref()
                .map(|window| window.expires_at)
                .unwrap_or(ledger.created_at + Duration::seconds(wall_clock_seconds));
            if source_deadline_at > now {
                bail!("P058_RESUME_DEADLINE_NOT_ELAPSED: source deadline is still in the future");
            }
            source_deadline_at
        }
        ResumeEscalationRequest::ExhaustedChain(_) => ledger.updated_at.min(now),
    };
    let expires_at = now + Duration::seconds(wall_clock_seconds);

    stages::insert_tx(tx, &retry_stage).await?;
    retry_stage_execution_authorities::create_active_targeted_agent_retry_tx(
        tx,
        run_id,
        &source_stage.stage_id,
        retry_stage_execution_id,
        Some(command_journal_id.to_string()),
        None,
        work_item_id.clone(),
        Some(source_agent_execution_id.to_string()),
        now,
    )
    .await?;
    if let Some(instruction_text) = validated_instruction.as_deref() {
        let binding = retry_operator_instructions::create_for_retry_attempt_tx(
            tx,
            &domain::retry_instruction::RetryInstructionBindingInput {
                journal_id: command_journal_id.to_string(),
                run_id,
                stage_id: source_stage.stage_id.clone(),
                source_stage_execution_id,
                retry_stage_execution_id,
                retry_attempt_number: next_attempt_number,
                target_agent_execution_id: Some(source_agent_execution_id),
                scope_kind: domain::retry_instruction::RetryInstructionScopeKind::TargetedRetry,
                instruction_text: instruction_text.to_string(),
                created_by_principal_id: principal_id.to_string(),
                created_by_principal_class: "operator".to_string(),
            },
        )
        .await?;
        retry_operator_instructions::create_for_work_item_tx(
            tx,
            &binding.binding_id,
            Some(&work_item_id),
            None,
        )
        .await?;
        retry_payload
            .as_object_mut()
            .context("P058_RESUME_SOURCE_WORK_INVALID: source payload must be an object")?
            .insert(
                "operator_retry_instruction".into(),
                serde_json::json!({
                    "binding_id": binding.binding_id,
                    "journal_id": binding.journal_id,
                    "scope_kind": binding.scope_kind.to_string(),
                    "instruction": binding.instruction_text,
                    "instruction_sha256": binding.instruction_sha256,
                }),
            );
    }
    work_items::enqueue_tx(
        tx,
        &WorkItem {
            id: work_item_id.clone(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::to_string(&retry_payload)?,
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some(source_stage.stage_id.clone()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await?;

    let window = EscalationDeadlineWindow {
        id: deadline_window_id.clone(),
        escalation_ledger_id: ledger.id.clone(),
        previous_window_id: previous_window.map(|window| window.id),
        tier_id: tier_id.clone(),
        tier_kind_raw: tier.kind.clone(),
        policy_hash: ledger.policy_hash.clone(),
        source_pause_reason_raw: request.source_pause_reason().into(),
        source_deadline_at,
        opened_by_principal_id: principal_id.to_string(),
        command_journal_id: command_journal_id.to_string(),
        resume_idempotency_key: idempotency_key.to_string(),
        resume_request_hash: request_hash,
        source_stage_execution_id: source_stage_execution_id.to_string(),
        source_agent_execution_id: source_agent_execution_id.to_string(),
        retry_stage_execution_id: retry_stage_execution_id.to_string(),
        work_item_id: work_item_id.clone(),
        target_backend_profile_id: backend_profile_id.clone(),
        target_provider: provider.clone(),
        starts_at: now,
        expires_at,
        created_at: now,
    };
    escalation::insert_deadline_window_tx(tx, &window).await?;

    ledger.status_raw = "active".into();
    ledger.current_tier_id = Some(tier_id.clone());
    ledger.current_tier_kind_raw = Some(tier.kind.clone());
    ledger.pause_reason_raw = None;
    ledger.operator_action_hint = None;
    ledger.runbook_anchor = None;
    ledger.updated_at = now;
    escalation::update_ledger_tx(tx, &ledger).await?;
    escalation::insert_event_tx(
        tx,
        &EscalationEvent {
            id: format!("{}:{deadline_window_id}", request.id_prefix()),
            escalation_ledger_id: ledger.id.clone(),
            event_kind_raw: request.event_kind().into(),
            tier_id: Some(tier_id.clone()),
            tier_kind_raw: Some(tier.kind.clone()),
            trigger_raw: ledger.trigger_raw.clone(),
            pause_reason_raw: Some(request.source_pause_reason().into()),
            payload_json: Some(
                serde_json::json!({
                    "event_kind_raw": request.event_kind(),
                    "deadline_window_id": deadline_window_id,
                    "policy_id": ledger.policy_id,
                    "tier_id": tier_id,
                    "tier_kind_raw": tier.kind,
                    "trigger_raw": ledger.trigger_raw,
                    "pause_reason_raw": request.source_pause_reason(),
                    "chain_attempt_index": ledger.chain_attempt_index,
                })
                .to_string(),
            ),
            redaction_version: Some("redaction_v1".into()),
            created_at: now,
        },
    )
    .await?;
    sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
        .bind(RunStatus::Running.to_string())
        .bind(&source_stage.stage_id)
        .bind(run_id.to_string())
        .execute(&mut **tx)
        .await?;

    Ok(ResumeEscalationDeadlineOutcome {
        run_id,
        escalation_ledger_id: ledger.id,
        deadline_window_id,
        retry_stage_execution_id,
        source_stage_execution_id,
        source_agent_execution_id,
        work_item_id,
        tier_id,
        backend_profile_id,
        provider,
        starts_at: now,
        expires_at,
        replayed: false,
    })
}

fn validate_request(request: ResumeEscalationRequest<'_>) -> Result<()> {
    let parsed = uuid::Uuid::parse_str(request.idempotency_key())
        .context("P058_RESUME_IDEMPOTENCY_INVALID: idempotency_key must be UUIDv7")?;
    if parsed.get_version() != Some(uuid::Version::SortRand) {
        bail!("P058_RESUME_IDEMPOTENCY_INVALID: idempotency_key must be UUIDv7");
    }
    if request.escalation_ledger_id().is_empty() || request.escalation_ledger_id().len() > 256 {
        bail!("P058_RESUME_LEDGER_INVALID: escalation_ledger_id must be 1..256 bytes");
    }
    if request.reason().trim().is_empty() || request.reason().len() > 1024 {
        bail!("P058_RESUME_REASON_INVALID: reason must be 1..1024 bytes");
    }
    if request
        .reason()
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\t' | '\n' | '\r'))
    {
        bail!("P058_RESUME_REASON_INVALID: reason contains a disallowed control character");
    }
    if let ResumeEscalationRequest::ExhaustedChain(command) = request {
        if command.target_tier_id.trim().is_empty() || command.target_tier_id.len() > 256 {
            bail!("P058_CHAIN_RESUME_TIER_INVALID: target_tier_id must be 1..256 bytes");
        }
    }
    Ok(())
}

fn resume_request_hash(request: ResumeEscalationRequest<'_>) -> String {
    let canonical = match request {
        ResumeEscalationRequest::Deadline(command) => serde_json::json!({
            "escalation_ledger_id": command.escalation_ledger_id,
            "reason": command.reason,
            "run_id": command.run_id.to_string(),
        }),
        ResumeEscalationRequest::ExhaustedChain(command) => serde_json::json!({
            "escalation_ledger_id": command.escalation_ledger_id,
            "operator_instruction": command.operator_instruction,
            "reason": command.reason,
            "recovery_kind": "escalation_chain_exhausted",
            "run_id": command.run_id.to_string(),
            "target_tier_id": command.target_tier_id,
        }),
    };
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

async fn latest_failed_execution_for_ledger(
    tx: &mut Transaction<'_, Sqlite>,
    escalation_ledger_id: &str,
) -> Result<(AgentExecutionId, StageExecutionId, String)> {
    let row = sqlx::query(
        r#"SELECT ae.id AS agent_execution_id, ae.stage_execution_id, ae.agent_id
           FROM escalation_execution_metadata em
           JOIN agent_executions ae ON ae.id = em.agent_execution_id
           WHERE em.escalation_ledger_id = ?1
             AND ae.status = 'failed'
             AND ae.stage_execution_id IS NOT NULL
           ORDER BY em.created_at DESC, ae.id DESC
           LIMIT 1"#,
    )
    .bind(escalation_ledger_id)
    .fetch_optional(&mut **tx)
    .await?
    .context("P058_RESUME_FAILED_EXECUTION_MISSING: ledger has no failed execution")?;
    let agent_execution_id: String = row.try_get("agent_execution_id")?;
    let stage_execution_id: String = row.try_get("stage_execution_id")?;
    Ok((
        agent_execution_id
            .parse()
            .context("P058_RESUME_FAILED_EXECUTION_INVALID: agent execution id")?,
        stage_execution_id
            .parse()
            .context("P058_RESUME_FAILED_EXECUTION_INVALID: stage execution id")?,
        row.try_get("agent_id")?,
    ))
}

fn find_source_invoke_work_item<'a>(
    items: &'a [WorkItem],
    stage_execution_id: StageExecutionId,
    agent_id: &str,
    agent_execution_id: AgentExecutionId,
) -> Option<&'a WorkItem> {
    let stage_execution_id = stage_execution_id.to_string();
    let agent_execution_id = agent_execution_id.to_string();
    items
        .iter()
        .filter(|item| item.kind == WorkItemKind::InvokeAgent)
        .filter_map(|item| {
            let payload: serde_json::Value = serde_json::from_str(&item.payload_json).ok()?;
            let claimed = payload
                .pointer("/p058_claimed/agent_execution_id")
                .and_then(serde_json::Value::as_str);
            let stage = payload
                .get("stage_execution_id")
                .and_then(serde_json::Value::as_str);
            let agent = payload.get("agent_id").and_then(serde_json::Value::as_str);
            (claimed == Some(agent_execution_id.as_str())
                || (stage == Some(stage_execution_id.as_str()) && agent == Some(agent_id)))
            .then_some(item)
        })
        .max_by_key(|item| item.created_at)
}
