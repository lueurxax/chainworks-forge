//! Invocation-scoped authority joining the engine, broker, and canonical gate.

use crate::{
    xcode_coordinator::{
        AccessMode, FixtureJournalAuthority, JournalAuthority, OwnerToken,
        WorkspaceAccessCoordinator, WorkspacePermit,
    },
    xcode_headless::{HeadlessWorkspaceController, PreparedWorkspace, WorkspaceOpenRequest},
    xcode_headless_host::TrustedProject,
    ExecutionRequest, XcodeEffectJournal,
};
use anyhow::{bail, ensure, Result};
use domain::{
    execution_root::ResolvedExecutionRoot,
    ids::{AgentExecutionId, RunId},
    xcode_contract as contract,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::time::{timeout_at, Instant};

#[derive(Debug)]
pub enum HeadlessRuntimeError {
    PermissionDenied,
    InactiveInvocation,
    ProjectHeld,
    OutcomeUnknown,
    AlreadyRecorded,
}
impl std::fmt::Display for HeadlessRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::PermissionDenied => "headless_permission_denied",
            Self::InactiveInvocation => "headless_invocation_inactive",
            Self::ProjectHeld => "headless_project_held",
            Self::OutcomeUnknown => "headless_outcome_unknown",
            Self::AlreadyRecorded => "headless_effect_already_recorded",
        })
    }
}
impl std::error::Error for HeadlessRuntimeError {}

fn coordinator_error(error: crate::xcode_coordinator::CoordinatorError) -> anyhow::Error {
    match error {
        crate::xcode_coordinator::CoordinatorError::ProjectHeld => {
            HeadlessRuntimeError::ProjectHeld.into()
        }
        error => error.into(),
    }
}

#[async_trait::async_trait]
pub trait HeadlessProjectTrust: Send + Sync {
    /// Checks a separately operator-admitted project and returns its record digest.
    async fn check(&self, project: &TrustedProject) -> Result<String>;
}

pub struct HeadlessPreparationInput {
    pub run_id: RunId,
    pub invocation_id: AgentExecutionId,
    pub owner_lineage: String,
    pub root: ResolvedExecutionRoot,
    pub project_selector: Option<String>,
    pub permission_policy: Value,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
struct Capabilities {
    read: bool,
    gates: BTreeSet<String>,
    digest: String,
}

impl Capabilities {
    fn evaluate(policy: &Value) -> Result<Self> {
        let caps = policy
            .get("xcode_headless")
            .and_then(Value::as_object)
            .ok_or_else(|| anyhow::anyhow!("headless_permission_required"))?;
        ensure!(
            caps.keys()
                .all(|key| matches!(key.as_str(), "read" | "gates")),
            "headless_permission_invalid"
        );
        let read = caps
            .get("read")
            .and_then(Value::as_bool)
            .ok_or_else(|| anyhow::anyhow!("headless_permission_invalid"))?;
        let raw = caps
            .get("gates")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow::anyhow!("headless_permission_invalid"))?;
        ensure!(raw.len() <= 128, "headless_permission_invalid");
        let mut gates = BTreeSet::new();
        for gate in raw {
            let gate = gate
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("headless_permission_invalid"))?;
            ensure!(
                !gate.is_empty()
                    && gate.len() <= 80
                    && gate
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "headless_permission_invalid"
            );
            ensure!(gates.insert(gate.to_owned()), "headless_permission_invalid");
        }
        ensure!(read || !gates.is_empty(), "headless_permission_required");
        Ok(Self {
            read,
            gates,
            digest: contract::canonical_digest("cw.xcode.permission.v1", policy)?,
        })
    }
}

enum Authority {
    Live(Arc<JournalAuthority>),
    Fixture(Arc<FixtureJournalAuthority>),
}
impl Authority {
    fn check(&self) -> Result<()> {
        match self {
            Self::Live(a) => a.check(),
            Self::Fixture(a) => a.check(),
        }
        .map_err(|_| anyhow::anyhow!("headless_journal_authority_unavailable"))
    }
}

pub struct HeadlessRuntime {
    controller: Arc<HeadlessWorkspaceController>,
    coordinator: Arc<WorkspaceAccessCoordinator>,
    trust: Arc<dyn HeadlessProjectTrust>,
    authority: Authority,
    invocations: Mutex<HashMap<AgentExecutionId, Weak<Invocation>>>,
}

struct Invocation {
    input: HeadlessPreparationInput,
    project: Arc<TrustedProject>,
    capabilities: Capabilities,
    trust_digest: String,
    binding_digest: String,
    workspace: tokio::sync::Mutex<PreparedWorkspace>,
    journal: Arc<dyn XcodeEffectJournal>,
    owner: OwnerToken,
    permit: WorkspacePermit,
    phase: Mutex<Phase>,
    deadline: Instant,
    active: tokio::sync::watch::Sender<bool>,
    operations: tokio::sync::Mutex<()>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Phase {
    Prepared,
    Committed(Option<String>),
    Active(Option<String>),
    Finished,
}

pub struct PreparedXcodeInvocation {
    inner: Arc<Invocation>,
}
impl PreparedXcodeInvocation {
    pub fn binding_digest(&self) -> &str {
        &self.inner.binding_digest
    }
}
impl Drop for PreparedXcodeInvocation {
    fn drop(&mut self) {
        *self.inner.phase.lock().expect("headless phase") = Phase::Finished;
        self.inner.owner.revoke();
        self.inner.active.send_replace(false);
    }
}
pub struct ActiveXcodePrompt {
    inner: Arc<Invocation>,
}
impl Drop for ActiveXcodePrompt {
    fn drop(&mut self) {
        *self.inner.phase.lock().expect("headless phase") = Phase::Finished;
        self.inner.owner.revoke();
        self.inner.active.send_replace(false);
    }
}

impl HeadlessRuntime {
    pub fn new(
        controller: Arc<HeadlessWorkspaceController>,
        coordinator: Arc<WorkspaceAccessCoordinator>,
        trust: Arc<dyn HeadlessProjectTrust>,
        authority: Arc<JournalAuthority>,
    ) -> Arc<Self> {
        Arc::new(Self {
            controller,
            coordinator,
            trust,
            authority: Authority::Live(authority),
            invocations: Mutex::new(HashMap::new()),
        })
    }
    pub fn new_fixture(
        controller: Arc<HeadlessWorkspaceController>,
        coordinator: Arc<WorkspaceAccessCoordinator>,
        trust: Arc<dyn HeadlessProjectTrust>,
        authority: Arc<FixtureJournalAuthority>,
    ) -> Arc<Self> {
        Arc::new(Self {
            controller,
            coordinator,
            trust,
            authority: Authority::Fixture(authority),
            invocations: Mutex::new(HashMap::new()),
        })
    }
    pub async fn prepare(
        &self,
        input: HeadlessPreparationInput,
        journal: Arc<dyn XcodeEffectJournal>,
    ) -> Result<PreparedXcodeInvocation> {
        self.authority.check()?;
        ensure!(
            !input.timeout.is_zero() && input.timeout <= Duration::from_secs(24 * 60 * 60),
            "headless_deadline_invalid"
        );
        let capabilities = Capabilities::evaluate(&input.permission_policy)?;
        let deadline = Instant::now() + input.timeout;
        let preparation_deadline = deadline.min(Instant::now() + Duration::from_secs(720));
        let project = Arc::new(TrustedProject::resolve(
            input.root.clone(),
            input.project_selector.as_deref(),
            crate::current_process_uid(),
        )?);
        ensure!(
            self.coordinator
                .project_key(std::path::Path::new(&project.key().canonical_path))?
                == *project.key(),
            "headless_project_identity_changed"
        );
        let trust_digest = timeout_at(preparation_deadline, self.trust.check(&project))
            .await
            .map_err(|_| anyhow::anyhow!("headless_trust_timeout"))??;
        domain::xcode_effect::validate_digest("trust_record", &trust_digest)?;
        let owner = self.coordinator.new_owner(deadline);
        // Hold one exclusive owner across the whole invocation. Reentrant gates
        // borrow it; unrelated readers/writers queue without a lock upgrade.
        let permit = timeout_at(
            preparation_deadline,
            self.coordinator
                .acquire(&owner, project.key(), AccessMode::Mutate),
        )
        .await
        .map_err(|_| anyhow::anyhow!("headless_preparation_timeout"))?
        .map_err(coordinator_error)?;
        self.authority.check()?;
        permit.check_dispatch().await?;
        let permission_policy_digest = contract::canonical_digest(
            "cw.xcode.admission.v1",
            &json!({"permissions":capabilities.digest,"trust_record":trust_digest}),
        )?;
        let workspace = self
            .controller
            .prepare_for_invocation(
                project.clone(),
                WorkspaceOpenRequest {
                    run_id: input.run_id.inner(),
                    owner_lineage: input.owner_lineage.clone(),
                    invocation_id: input.invocation_id.inner(),
                    operation_key: input.invocation_id.inner(),
                    permission_policy_digest,
                    trust_policy_id: contract::TRUST_POLICY_ID.into(),
                    timeout: preparation_deadline.saturating_duration_since(Instant::now()),
                },
                journal.clone(),
                deadline,
            )
            .await?;
        permit.check_dispatch().await?;
        let binding_digest = workspace.binding_digest().to_owned();
        let invocation_id = input.invocation_id;
        let inner = Arc::new(Invocation {
            input,
            project,
            capabilities,
            trust_digest,
            binding_digest,
            workspace: tokio::sync::Mutex::new(workspace),
            journal,
            owner,
            permit,
            phase: Mutex::new(Phase::Prepared),
            deadline,
            active: tokio::sync::watch::channel(false).0,
            operations: tokio::sync::Mutex::new(()),
        });
        let mut invocations = self.invocations.lock().expect("headless invocations");
        invocations.retain(|_, value| value.strong_count() > 0);
        ensure!(invocations.len() < 256, "headless_invocation_capacity");
        ensure!(
            !invocations.contains_key(&invocation_id),
            "headless_invocation_conflict"
        );
        invocations.insert(invocation_id, Arc::downgrade(&inner));
        Ok(PreparedXcodeInvocation { inner })
    }

    fn invocation(&self, id: AgentExecutionId) -> Result<Arc<Invocation>> {
        self.authority.check()?;
        let invocation = self
            .invocations
            .lock()
            .expect("headless invocations")
            .get(&id)
            .and_then(Weak::upgrade)
            .ok_or(HeadlessRuntimeError::InactiveInvocation)?;
        ensure!(
            Instant::now() < invocation.deadline,
            HeadlessRuntimeError::InactiveInvocation
        );
        ensure!(
            *invocation.phase.lock().expect("headless phase") != Phase::Finished,
            HeadlessRuntimeError::InactiveInvocation
        );
        Ok(invocation)
    }

    fn match_request(&self, invocation: &Invocation, req: &ExecutionRequest) -> Result<()> {
        ensure!(
            req.run_id == invocation.input.run_id
                && req.agent_execution_id == Some(invocation.input.invocation_id),
            "headless_invocation_mismatch"
        );
        let root = crate::execution_root::resolve_execution_root(
            &req.workspace_root,
            req.worktree_root.as_deref(),
            req.worktree_write_enabled,
            req.worktree_strategy.as_deref(),
        )?;
        ensure!(
            root == invocation.input.root,
            "headless_execution_root_mismatch"
        );
        invocation.project.revalidate()?;
        Ok(())
    }

    pub fn commit(&self, prepared: &PreparedXcodeInvocation, req: &ExecutionRequest) -> Result<()> {
        let invocation = self.invocation(prepared.inner.input.invocation_id)?;
        ensure!(
            Arc::ptr_eq(&invocation, &prepared.inner),
            "headless_foreign_preparation"
        );
        self.match_request(&invocation, req)?;
        let mut phase = invocation.phase.lock().expect("headless phase");
        ensure!(*phase == Phase::Prepared, "headless_already_committed");
        *phase = Phase::Committed(req.session_generation_id.clone());
        Ok(())
    }

    pub fn validate_committed(&self, req: &ExecutionRequest) -> Result<()> {
        let invocation = self.invocation(
            req.agent_execution_id
                .ok_or_else(|| anyhow::anyhow!("headless_invocation_required"))?,
        )?;
        self.match_request(&invocation, req)?;
        let phase = invocation.phase.lock().expect("headless phase");
        ensure!(
            matches!(&*phase,Phase::Committed(generation)|Phase::Active(generation) if generation == &req.session_generation_id),
            "headless_generation_mismatch"
        );
        Ok(())
    }

    pub fn begin_prompt(&self, req: &ExecutionRequest) -> Result<ActiveXcodePrompt> {
        self.validate_committed(req)?;
        let inner = self.invocation(req.agent_execution_id.unwrap())?;
        {
            let mut phase = inner.phase.lock().expect("headless phase");
            ensure!(
                *phase == Phase::Committed(req.session_generation_id.clone()),
                "headless_prompt_already_started"
            );
            *phase = Phase::Active(req.session_generation_id.clone());
            inner.active.send_replace(true);
        }
        Ok(ActiveXcodePrompt { inner })
    }

    pub fn binding_digest_for(&self, id: AgentExecutionId) -> Result<String> {
        Ok(self.invocation(id)?.binding_digest.clone())
    }

    async fn check_invocation(&self, invocation: &Invocation) -> Result<()> {
        self.authority.check()?;
        ensure!(
            Instant::now() < invocation.deadline,
            "headless_invocation_expired"
        );
        invocation.project.revalidate()?;
        ensure!(
            timeout_at(invocation.deadline, self.trust.check(&invocation.project))
                .await
                .map_err(|_| anyhow::anyhow!("headless_trust_timeout"))??
                == invocation.trust_digest,
            "headless_project_trust_changed"
        );
        invocation
            .permit
            .check_dispatch()
            .await
            .map_err(coordinator_error)?;
        Ok(())
    }

    pub async fn revalidate(&self, id: AgentExecutionId) -> Result<()> {
        let invocation = self.invocation(id)?;
        self.check_invocation(&invocation).await?;
        timeout_at(invocation.deadline, async {
            invocation.workspace.lock().await.revalidate().await
        })
        .await
        .map_err(|_| anyhow::anyhow!("headless_binding_timeout"))?
    }

    pub async fn forward(&self, id: AgentExecutionId, request: Value) -> Result<Value> {
        let invocation = self.invocation(id)?;
        ensure!(
            *invocation.active.borrow(),
            HeadlessRuntimeError::InactiveInvocation
        );
        let _operation = timeout_at(invocation.deadline, invocation.operations.lock())
            .await
            .map_err(|_| anyhow::anyhow!("headless_operation_timeout"))?;
        self.check_invocation(&invocation).await?;
        ensure!(
            *invocation.active.borrow(),
            HeadlessRuntimeError::InactiveInvocation
        );
        let result = match request.get("method").and_then(Value::as_str) {
            Some("initialize") => {
                json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"chainworks-xcode-headless","version":"1"}})
            }
            Some("ping") => json!({}),
            Some(
                "notifications/initialized" | "notifications/cancelled" | "notifications/progress",
            ) => return Ok(Value::Null),
            Some("tools/list") => {
                json!({"tools":if invocation.capabilities.read {vec![contract::read_tool_definition()]} else {vec![]}})
            }
            Some("tools/call") => {
                ensure!(
                    invocation.capabilities.read && request["params"]["name"] == "XcodeRead",
                    HeadlessRuntimeError::PermissionDenied
                );
                let args = request["params"]["arguments"].clone();
                let mut workspace = timeout_at(invocation.deadline, invocation.workspace.lock())
                    .await
                    .map_err(|_| anyhow::anyhow!("headless_operation_timeout"))?;
                let mut cancellation = prompt_cancellation(&invocation.active)?;
                let value = tokio::select! {
                    result = workspace.read(args) => result?,
                    _ = cancellation.changed() => bail!("headless_prompt_cancelled"),
                };
                ensure!(
                    *invocation.active.borrow(),
                    "headless_active_prompt_required"
                );
                json!({"isError":false,"content":[{"type":"text","text":serde_json::to_string(&value)?}],"structuredContent":value})
            }
            _ => bail!("headless_method_not_admitted"),
        };
        Ok(json!({"jsonrpc":"2.0","id":request["id"],"result":result}))
    }
}

#[async_trait::async_trait]
impl crate::XcodeShimDispatchAuthority for HeadlessRuntime {
    async fn dispatch(
        &self,
        invocation_id: AgentExecutionId,
        operation_key: uuid::Uuid,
        mut request: crate::XcodeShimDispatchRequest,
        config: &crate::XcodeHostExecutorProcessConfig,
        observation_sink: &dyn crate::XcodeRuntimeObservationSink,
    ) -> Result<crate::XcodeShimDispatchOutcome> {
        use domain::xcode_effect::*;
        let invocation = self.invocation(invocation_id)?;
        ensure!(
            *invocation.active.borrow(),
            "headless_active_prompt_required"
        );
        ensure!(!operation_key.is_nil(), "headless_operation_key_required");
        let _operation = timeout_at(invocation.deadline, invocation.operations.lock())
            .await
            .map_err(|_| anyhow::anyhow!("headless_operation_timeout"))?;
        self.check_invocation(&invocation).await?;
        let nested = invocation.permit.borrow_nested(AccessMode::Mutate).await?;
        let mut workspace = timeout_at(invocation.deadline, invocation.workspace.lock())
            .await
            .map_err(|_| anyhow::anyhow!("headless_operation_timeout"))?;
        workspace.revalidate().await?;
        ensure!(
            request.plan_input.invoked_tool == "chainworks-test-gate",
            "headless_use_canonical_gate"
        );
        let identity = crate::pin_headless_canonical_gate(
            std::path::Path::new(&invocation.input.root.effective),
            &request.plan_input.args,
        )?;
        ensure!(
            invocation.capabilities.gates.contains(&identity.gate),
            HeadlessRuntimeError::PermissionDenied
        );
        let host = workspace.host_snapshot();
        request.agent_execution_id = Some(invocation_id);
        request.plan_input.cwd = invocation.input.root.effective.clone();
        request.plan_input.workspace_root = invocation.input.root.effective.clone();
        // No client environment becomes host authority. In particular provider
        // PATH, test-root overrides, credentials and shim recursion are discarded.
        request.plan_input.provider_env = std::collections::BTreeMap::from([
            (
                "HOME".into(),
                host.operator_home.to_string_lossy().into_owned(),
            ),
            ("USER".into(), host.account_name.clone()),
            ("LOGNAME".into(), host.account_name.clone()),
            (
                "TMPDIR".into(),
                host.darwin_tmpdir.to_string_lossy().into_owned(),
            ),
            (
                "DEVELOPER_DIR".into(),
                host.generation.developer_dir.to_string_lossy().into_owned(),
            ),
            (
                "PATH".into(),
                format!(
                    "{}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin",
                    host.operator_home.display()
                ),
            ),
            ("CHAINWORKS_AUTO_CACHE_CLEANUP".into(), "0".into()),
        ]);
        request.plan_input.simulator_candidates.clear();
        request.plan_input.toolchain_mapping_root = None;
        request.attempt.now_epoch_ms = chrono::Utc::now().timestamp_millis();
        ensure!(
            request.grant.authorize(&request.attempt).allowed,
            "headless_shim_authorization_rejected"
        );
        let schema_value = json!({"version":1,"name":"canonical_gate_result","fields":["gate","exit_status","process_group_settled"],"historical":"bounded_summary"});
        let schema = SchemaIdentity {
            contract_id: "chainworks.canonical-gate".into(),
            version: 1,
            manifest_digest: contract::canonical_digest(
                "cw.xcode.gate.manifest.v1",
                &schema_value,
            )?,
            schema_digest: contract::canonical_digest("cw.xcode.gate.schema.v1", &schema_value)?,
        };
        let intent = NormalizedIntent {
            run_id: invocation.input.run_id.inner(),
            owner_lineage: invocation.input.owner_lineage.clone(),
            invocation_id: invocation_id.inner(),
            project_key: invocation.project.key().clone(),
            operation_key,
            operation: "canonical_gate".into(),
            target_digest: invocation.binding_digest.clone(),
            binding_digest: Some(invocation.binding_digest.clone()),
            request_digest: contract::canonical_digest(
                "cw.xcode.gate.request.v1",
                &serde_json::to_value(&identity)?,
            )?,
            origin: EffectOrigin::Shim,
            effect_class: EffectClass::Execute,
            result_schema: schema.clone(),
        };
        let prepared = timeout_at(invocation.deadline, invocation.journal.prepare(&intent))
            .await
            .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??;
        ensure!(
            prepared.state == AttemptState::Prepared,
            HeadlessRuntimeError::AlreadyRecorded
        );
        nested.check_dispatch().await?;
        ensure!(
            *invocation.active.borrow(),
            "headless_active_prompt_required"
        );
        // Arm before CAS: a lost acknowledgement may still leave a durable
        // dispatched row. Recovery must never classify that as safe to resend.
        let mut settlement = GateSettlement {
            invocation: invocation.clone(),
            attempt: Some(AttemptRevision {
                attempt_id: prepared.attempt_id,
                revision: prepared.revision + 1,
            }),
        };
        let dispatched = match timeout_at(
            invocation.deadline,
            invocation
                .journal
                .dispatch(prepared.nonce, &intent.request_digest, prepared.revision),
        )
        .await
        .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??
        {
            crate::XcodeDispatchDecision::Dispatch(attempt) => attempt,
            crate::XcodeDispatchDecision::Existing(_) => {
                bail!(HeadlessRuntimeError::AlreadyRecorded)
            }
        };
        settlement.attempt = Some(AttemptRevision {
            attempt_id: dispatched.attempt_id,
            revision: dispatched.revision,
        });
        // The durable hold is now this operation's fence; do not check it as
        // though it belonged to a different owner after dispatch.
        self.authority.check()?;
        invocation.project.revalidate()?;
        ensure!(
            *invocation.active.borrow(),
            "headless_active_prompt_required"
        );
        let bounded = crate::XcodeHostExecutorProcessConfig {
            tool_paths: Default::default(),
            timeout: config.timeout.min(
                invocation
                    .deadline
                    .saturating_duration_since(Instant::now()),
            ),
        };
        let mut cancellation = prompt_cancellation(&invocation.active)?;
        let outcome = tokio::select! {
            result = crate::dispatch_headless_canonical_gate(request,&identity,&bounded,observation_sink) => result?,
            _ = cancellation.changed() => bail!("headless_gate_cancelled"),
        };
        let settled = outcome
            .process_output
            .as_ref()
            .is_some_and(|output| output.process_group_settled);
        let completion = if settled {
            let value = json!({"gate":identity.gate,"exit_status":outcome.exit_status,"process_group_settled":true});
            let digest = contract::canonical_digest("cw.xcode.gate.result.v1", &value)?;
            let summary = RedactedText::new(&format!(
                "Canonical gate {} exited with status {}",
                identity.gate, outcome.exit_status
            ))?;
            let historical = HistoricalResult {
                schema_version: 1,
                schema,
                payload: if outcome.exit_status == 0 {
                    HistoricalPayload::Result { summary }
                } else {
                    HistoricalPayload::Error {
                        code: EffectErrorCode::TerminalFailure,
                        message: summary,
                    }
                },
                result_digest: digest.clone(),
            };
            let proof = SettlementProof {
                evidence_ref: format!("xcode/gate-result/{digest}"),
                active_operation_check_ref: format!(
                    "xcode/process-group-settled/{}",
                    dispatched.attempt_id
                ),
                no_operation_in_flight: true,
            };
            if outcome.exit_status == 0 {
                Completion::Succeeded {
                    outcome: historical,
                    proof,
                }
            } else {
                Completion::Failed {
                    outcome: historical,
                    proof,
                }
            }
        } else {
            Completion::Unknown {
                reason: UncertaintyReason::AsyncInFlight,
            }
        };
        tokio::time::timeout(
            Duration::from_secs(10),
            invocation
                .journal
                .complete(settlement.attempt.unwrap(), &completion),
        )
        .await
        .map_err(|_| anyhow::anyhow!("headless_journal_timeout"))??;
        settlement.attempt = None;
        if !settled {
            invocation.owner.revoke();
            invocation.active.send_replace(false);
            *invocation.phase.lock().expect("headless phase") = Phase::Finished;
        }
        // Hold the live bridge and nested owner through durable settlement.
        drop(workspace);
        drop(nested);
        Ok(outcome)
    }
}

struct GateSettlement {
    invocation: Arc<Invocation>,
    attempt: Option<domain::xcode_effect::AttemptRevision>,
}

fn prompt_cancellation(
    active: &tokio::sync::watch::Sender<bool>,
) -> Result<tokio::sync::watch::Receiver<bool>> {
    let receiver = active.subscribe();
    ensure!(*receiver.borrow(), HeadlessRuntimeError::InactiveInvocation);
    Ok(receiver)
}
impl Drop for GateSettlement {
    fn drop(&mut self) {
        if let Some(attempt) = self.attempt.take() {
            self.invocation.owner.revoke();
            self.invocation.active.send_replace(false);
            *self.invocation.phase.lock().expect("headless phase") = Phase::Finished;
            let invocation = self.invocation.clone();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(
                    Duration::from_secs(10),
                    invocation.journal.complete(
                        attempt,
                        &domain::xcode_effect::Completion::Unknown {
                            reason: domain::xcode_effect::UncertaintyReason::CancelledAfterDispatch,
                        },
                    ),
                )
                .await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subscription_after_prompt_end_cannot_admit_a_gate() {
        let active = tokio::sync::watch::channel(true).0;
        active.send_replace(false);
        assert!(prompt_cancellation(&active).is_err());
    }
    #[test]
    fn frozen_capabilities_require_explicit_values_not_profile_labels() {
        for policy in [
            json!("CODE_WRITE"),
            json!({}),
            json!({"xcode_headless":{"read":"true"}}),
            json!({"xcode_headless":{"read":true,"gates":["build;rm"]}}),
        ] {
            assert!(Capabilities::evaluate(&policy).is_err());
        }
        let caps = Capabilities::evaluate(
            &json!({"xcode_headless":{"read":true,"gates":["build","fast"]}}),
        )
        .unwrap();
        assert!(caps.read);
        assert_eq!(
            caps.gates,
            BTreeSet::from(["build".to_owned(), "fast".to_owned()])
        );
        assert_eq!(caps.digest.len(), 64);
    }
    #[test]
    fn frozen_policy_changes_binding_digest_even_for_same_profile_name() {
        let a = Capabilities::evaluate(
            &json!({"xcode_headless":{"read":true,"gates":[]},"filesystem":{"deny":["a"]}}),
        )
        .unwrap();
        let b = Capabilities::evaluate(
            &json!({"xcode_headless":{"read":true,"gates":[]},"filesystem":{"deny":["b"]}}),
        )
        .unwrap();
        assert_ne!(a.digest, b.digest);
    }
}
