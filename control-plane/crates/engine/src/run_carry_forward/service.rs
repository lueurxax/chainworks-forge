//! Authenticated command composition. Files never substitute for durable authority.
#[cfg(test)]
#[path = "../../tests/p039_fixture/service_reliability.rs"]
mod reliability_tests;

use super::{preview, readback, CarryForwardFinalizer, PreparationJob, PreparationLane};
use anyhow::{Context, Result};
use auth::{Principal, PrincipalClass};
use db::repos::{run_continuation_commands as commands, run_continuations as operations};
use domain::{
    ids::RunId,
    run_carry_forward::{canonical_digest, ContentDigest},
    run_carry_forward_api as api, CapabilityToolId,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub enabled: bool,
    /// Existing, private, server-configured storage, never a request parameter.
    pub owned_parent: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ServiceError {
    #[error("continuation access denied")]
    Unauthorized,
    #[error("invalid continuation parameters")]
    InvalidParams,
    #[error("continuation admission unavailable")]
    AdmissionUnavailable,
    #[error("continuation command outcome unknown")]
    CommandOutcomeUnknown,
}

pub struct CarryForwardService {
    pool: SqlitePool,
    config: ServiceConfig,
    lane: PreparationLane,
    // Reserve + enqueue is one local admission sequence. Durable DB constraints
    // separately serialize all service instances and survive process loss.
    admission: tokio::sync::Mutex<()>,
}

impl CarryForwardService {
    pub fn new(pool: SqlitePool, config: ServiceConfig) -> Self {
        Self {
            lane: PreparationLane::new(pool.clone()),
            pool,
            config,
            admission: tokio::sync::Mutex::new(()),
        }
    }
    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    /// The boundary must freshly authenticate and evaluate its BoundaryPolicy on
    /// every call. This second capability/scope check also protects direct callers.
    pub async fn call(&self, tool: &str, principal: &Principal, arguments: Value) -> Result<Value> {
        let result = self.call_inner(tool, principal, arguments).await;
        if matches!(tool, "runs.continue_blocked" | "runs.continuation_activate") {
            let (status, reason) = match &result {
                Ok(value) => (
                    value["status"].as_str().unwrap_or("internal_error"),
                    serde_json::from_value::<api::ContinuationReasonCode>(
                        value["denial"]["code"].clone(),
                    )
                    .ok(),
                ),
                Err(error) => (
                    match error.downcast_ref::<ServiceError>() {
                        Some(ServiceError::Unauthorized) => "unauthorized",
                        Some(ServiceError::InvalidParams) => "invalid_params",
                        Some(ServiceError::CommandOutcomeUnknown) => "outcome_unknown",
                        _ => "internal_error",
                    },
                    None,
                ),
            };
            db::metrics::record_continuation_command(
                tool == "runs.continuation_activate",
                status,
                reason,
            );
        }
        result
    }

    async fn call_inner(
        &self,
        tool: &str,
        principal: &Principal,
        arguments: Value,
    ) -> Result<Value> {
        authorize(tool, principal)?;
        if serde_json::to_vec(&arguments)?.len() > api::MAX_REQUEST_BYTES {
            return Err(ServiceError::InvalidParams.into());
        }
        match tool {
            "runs.continuation_preview" => {
                let request: api::ContinuationPreviewRequest = decode(arguments)?;
                self.read_source(principal, request.run_id.as_uuid().to_string())
                    .await?;
                let source = request.run_id;
                match preview::preview(&self.pool, preview_input(request)?)
                    .await
                    .map_err(|_| ServiceError::InvalidParams)?
                {
                    preview::PreviewOutcome::Page { page, prepared } => {
                        Ok(serde_json::to_value(preview::wire_page(&page, &prepared)?)?)
                    }
                    preview::PreviewOutcome::Stale(stale) => Ok(serde_json::to_value(stale)?),
                    preview::PreviewOutcome::Held { holds } => {
                        let result: api::RunCarryForwardPreviewHoldV1 = serde_json::from_value(
                            json!({
                            "schema_version":"run_carry_forward_preview_hold_v1","source_run_id":source,
                            "holds":holds.iter().map(|h|json!({"code":h.code.as_str(),"message":h.code.as_str()})).collect::<Vec<_>>(),
                            "next_action":"resolve_hold"}),
                        )?;
                        Ok(serde_json::to_value(result)?)
                    }
                }
            }
            "runs.continuation_get" => {
                let request: api::ContinuationGetRequest = decode(arguments)?;
                let operation = self
                    .read_operation(principal, &request.operation_id.as_uuid().to_string())
                    .await?;
                self.get(operation, request).await
            }
            "runs.continue_blocked" => {
                let request: api::ContinueBlockedRequest = decode(arguments.clone())?;
                let source = request.run_id.as_uuid().to_string();
                self.read_source(principal, source.clone()).await?;
                let identity =
                    Identity::new(principal, tool, request.caller_request_id, &arguments)?;
                if let Some(replay) = self.replay(&identity).await? {
                    return Ok(replay);
                }
                if !self.config.enabled {
                    return self
                        .deny(
                            &identity,
                            &source,
                            None,
                            api::ContinuationReasonCode::RolloutHold,
                        )
                        .await;
                }
                self.prepare(request, &identity).await
            }
            "runs.continuation_activate"
            | "runs.continuation_abort"
            | "runs.continuation_reconcile" => {
                let (id, version, caller, manifest) = match tool {
                    "runs.continuation_activate" => {
                        let r: api::ContinuationActivateRequest = decode(arguments.clone())?;
                        (
                            r.operation_id,
                            r.expected_version,
                            r.caller_request_id,
                            Some(r.expected_manifest_sha256),
                        )
                    }
                    "runs.continuation_abort" => {
                        let r: api::ContinuationAbortRequest = decode(arguments.clone())?;
                        (
                            r.operation_id,
                            r.expected_version,
                            r.caller_request_id,
                            None,
                        )
                    }
                    _ => {
                        let r: api::ContinuationReconcileRequest = decode(arguments.clone())?;
                        (
                            r.operation_id,
                            r.expected_version,
                            r.caller_request_id,
                            None,
                        )
                    }
                };
                let op = self
                    .read_operation(principal, &id.as_uuid().to_string())
                    .await?;
                let identity = Identity::new(principal, tool, caller, &arguments)?;
                if let Some(replay) = self.replay(&identity).await? {
                    return Ok(replay);
                }
                if version.get() != op.version as u64 {
                    return self
                        .deny(
                            &identity,
                            &op.source_run_id,
                            Some(&op),
                            api::ContinuationReasonCode::StaleOperationVersion,
                        )
                        .await;
                }
                if tool == "runs.continuation_activate" {
                    if !self.config.enabled {
                        return self
                            .deny(
                                &identity,
                                &op.source_run_id,
                                Some(&op),
                                api::ContinuationReasonCode::RolloutHold,
                            )
                            .await;
                    }
                    self.activate(op, manifest.expect("typed activate"), &identity)
                        .await
                } else if tool == "runs.continuation_abort" {
                    let result = operations::abort_command(
                        &self.pool,
                        &op.operation_id,
                        op.version,
                        &identity.borrow(),
                    )
                    .await;
                    self.command_result(result, &identity, &op.source_run_id, Some(&op))
                        .await
                } else {
                    self.reconcile(op, &identity).await
                }
            }
            _ => Err(ServiceError::InvalidParams.into()),
        }
    }

    async fn read_source(&self, principal: &Principal, source: String) -> Result<()> {
        if !can_read(principal, &source) {
            return Err(ServiceError::Unauthorized.into());
        }
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs WHERE id=?)")
            .bind(source)
            .fetch_one(&self.pool)
            .await?;
        if !exists {
            return Err(ServiceError::Unauthorized.into());
        }
        Ok(())
    }

    async fn read_operation(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<operations::Continuation> {
        let op = operations::find(&self.pool, id)
            .await?
            .ok_or(ServiceError::Unauthorized)?;
        if !can_read(principal, &op.source_run_id)
            || op
                .successor_run_id
                .as_ref()
                .is_some_and(|id| !can_read(principal, id))
        {
            return Err(ServiceError::Unauthorized.into());
        }
        Ok(op)
    }

    async fn replay(&self, id: &Identity) -> Result<Option<Value>> {
        match commands::find(&self.pool, &id.caller, &id.request, &id.tool, &id.intent).await {
            Ok(Some(original)) => Ok(Some(serde_json::to_value(original.replayed()?)?)),
            Ok(None) => Ok(None),
            Err(error)
                if known_reason(&error)
                    == Some(api::ContinuationReasonCode::IdempotencyConflict) =>
            {
                Ok(Some(serde_json::to_value(conflict(&id.request)?)?))
            }
            Err(error) => Err(error),
        }
    }

    async fn deny(
        &self,
        id: &Identity,
        source: &str,
        op: Option<&operations::Continuation>,
        code: api::ContinuationReasonCode,
    ) -> Result<Value> {
        let result = commands::record_denial(&self.pool, &id.borrow(), source, op, code)
            .await
            .map_err(commit_error)?;
        Ok(serde_json::to_value(result)?)
    }

    async fn command_result(
        &self,
        result: Result<api::RunContinuationCommandResultV1>,
        id: &Identity,
        source: &str,
        op: Option<&operations::Continuation>,
    ) -> Result<Value> {
        match result {
            Ok(value) => Ok(serde_json::to_value(value)?),
            Err(error)
                if error
                    .downcast_ref::<commands::CommitOutcomeUnknown>()
                    .is_some() =>
            {
                Err(ServiceError::CommandOutcomeUnknown.into())
            }
            Err(error) => match known_reason(&error) {
                Some(api::ContinuationReasonCode::IdempotencyConflict) => {
                    Ok(serde_json::to_value(conflict(&id.request)?)?)
                }
                Some(code) => self.deny(id, source, op, code).await,
                None => Err(error),
            },
        }
    }

    async fn prepare(&self, request: api::ContinueBlockedRequest, id: &Identity) -> Result<Value> {
        let source = request.run_id.as_uuid().to_string();
        let mut value = serde_json::to_value(&request)?;
        for field in ["expected_plan_sha256", "caller_request_id", "reason"] {
            value.as_object_mut().expect("object").remove(field);
        }
        let input = preview_input(decode(value)?)?;
        let prepared = match preview::preview(&self.pool, input)
            .await
            .map_err(|_| ServiceError::InvalidParams)?
        {
            preview::PreviewOutcome::Page { prepared, .. } => prepared,
            preview::PreviewOutcome::Stale(_) => {
                return self
                    .deny(id, &source, None, api::ContinuationReasonCode::PreviewStale)
                    .await
            }
            preview::PreviewOutcome::Held { holds } => {
                let code = serde_json::from_value(json!(holds
                    .first()
                    .context("empty preview holds")?
                    .code
                    .as_str()))?;
                return self.deny(id, &source, None, code).await;
            }
        };
        if prepared.plan().plan_sha256 != request.expected_plan_sha256 {
            return self
                .deny(
                    id,
                    &source,
                    None,
                    api::ContinuationReasonCode::SourceChanged,
                )
                .await;
        }
        if super::safe_files::SafeRoot::open(&self.config.owned_parent).is_err() {
            return self
                .deny(id, &source, None, api::ContinuationReasonCode::UnsafePath)
                .await;
        }
        let witness = bare(&prepared.source_witness().semantic_sha256).to_owned();
        let finalizer = Arc::new(CarryForwardFinalizer::new(prepared)?);
        let guard = self.admission.lock().await;
        // A request waiting for this lock may have been accepted by its twin.
        if let Some(replay) = self.replay(id).await? {
            return Ok(replay);
        }
        let permit = match self.lane.try_reserve() {
            Ok(permit) => permit,
            Err(error) => {
                drop(guard);
                return self.command_result(Err(error), id, &source, None).await;
            }
        };
        let reserved = operations::reserve_checked(
            &self.pool,
            &operations::Reservation {
                source_run_id: source.clone(),
                caller_fingerprint: id.caller.clone(),
                caller_request_id: id.request.clone(),
                intent_sha256: id.intent.clone(),
                plan_ref: String::new(),
                target_ref: String::new(),
                plan_sha256: bare(&request.expected_plan_sha256).into(),
                source_witness_sha256: witness,
                reason: request.reason.as_str().into(),
            },
            &self.config.owned_parent,
        )
        .await;
        let op = match reserved {
            Ok(op) => op,
            Err(error) => {
                drop(permit);
                drop(guard);
                return self.command_result(Err(error), id, &source, None).await;
            }
        };
        let job = PreparationJob::new(
            uuid::Uuid::parse_str(&op.operation_id)?,
            finalizer.workspace(),
            self.config.owned_parent.clone(),
            finalizer,
        );
        if let Err(error) = self.lane.try_submit_reserved(job, permit) {
            // Reservation already committed. Never turn this into a no-operation
            // denial or resubmit an effect. Persist the unclaimed hold before reply.
            tracing::error!(operation_id=%op.operation_id,error=%error,"continuation reserved but preparation not submitted");
            operations::hold_queued_preparation(
                &self.pool,
                &op.operation_id,
                op.generation,
                "effect_outcome_unknown",
            )
            .await
            .map_err(|_| ServiceError::CommandOutcomeUnknown)?;
        }
        drop(guard);
        let original = commands::find(&self.pool, &id.caller, &id.request, &id.tool, &id.intent)
            .await?
            .ok_or(ServiceError::CommandOutcomeUnknown)?;
        Ok(serde_json::to_value(original)?)
    }

    async fn activate(
        &self,
        op: operations::Continuation,
        expected: ContentDigest,
        id: &Identity,
    ) -> Result<Value> {
        if op.phase != "prepared" || op.manifest_sha256.as_deref() != Some(bare(&expected)) {
            return self
                .deny(
                    id,
                    &op.source_run_id,
                    Some(&op),
                    api::ContinuationReasonCode::PreparedMaterialChanged,
                )
                .await;
        }
        let verified = tokio::time::timeout(std::time::Duration::from_secs(120), async {
            let manifest = super::read_verified_manifest(&op).await?;
            preview::verify_current_target(&self.pool, &manifest).await?;
            verify_rollout(&manifest).await?;
            // Verification above is read-only and may race external writers. The
            // final material check plus transaction-local canonical witness is the
            // last admission check; operator-owned external writers must be quiet.
            let current = super::read_verified_manifest(&op).await?;
            anyhow::ensure!(
                canonical_digest(&manifest)? == canonical_digest(&current)?,
                "prepared_material_changed"
            );
            Ok::<_, anyhow::Error>(manifest)
        })
        .await
        .unwrap_or_else(|_| {
            Err(anyhow::anyhow!(
                "continuation_budget_exceeded: activation verification"
            ))
        });
        let manifest = match verified {
            Ok(value) => value,
            Err(error) => {
                return self
                    .command_result(Err(error), id, &op.source_run_id, Some(&op))
                    .await
            }
        };
        let result = operations::activate_checked_command(
            &self.pool,
            &op.operation_id,
            op.version,
            bare(&expected),
            &manifest.successor,
            &manifest.inputs,
            &id.borrow(),
        )
        .await;
        self.command_result(result, id, &op.source_run_id, Some(&op))
            .await
    }

    async fn reconcile(&self, op: operations::Continuation, id: &Identity) -> Result<Value> {
        if op.phase == "aborting" {
            let result = operations::reconcile_abort_command(
                &self.pool,
                &op.operation_id,
                op.version,
                &id.borrow(),
            )
            .await;
            return self
                .command_result(result, id, &op.source_run_id, Some(&op))
                .await;
        }
        if op.phase == "needs_reconciliation" && !op.worker_claimed {
            match self.verified_reconciliation_candidate(&op).await {
                Ok(candidate) => {
                    let result = operations::reconcile_prepared_command(
                        &self.pool,
                        &op.operation_id,
                        op.version,
                        candidate
                            .manifest_ref
                            .as_deref()
                            .expect("verified manifest path"),
                        candidate
                            .manifest_sha256
                            .as_deref()
                            .expect("verified manifest digest"),
                        &id.borrow(),
                    )
                    .await;
                    return self
                        .command_result(result, id, &op.source_run_id, Some(&op))
                        .await;
                }
                Err(error) => {
                    let code = match known_reason(&error) {
                        Some(api::ContinuationReasonCode::SourceChanged) => {
                            api::ContinuationReasonCode::SourceChanged
                        }
                        Some(api::ContinuationReasonCode::EffectOutcomeUnknown) => {
                            api::ContinuationReasonCode::EffectOutcomeUnknown
                        }
                        _ => api::ContinuationReasonCode::PreparedMaterialChanged,
                    };
                    let result = operations::reconcile_command(
                        &self.pool,
                        &op.operation_id,
                        op.version,
                        Some(code),
                        &id.borrow(),
                    )
                    .await;
                    return self
                        .command_result(result, id, &op.source_run_id, Some(&op))
                        .await;
                }
            }
        }
        // No effect is retried. A worker claim/unknown step remains unresolved
        // unless its tracked owner positively settles it; absence of a PID is not proof.
        let verified = if op.phase == "prepared" && !op.worker_claimed {
            match super::read_verified_manifest(&op).await {
                Ok(_) => None,
                Err(_) => Some(api::ContinuationReasonCode::PreparedMaterialChanged),
            }
        } else {
            Some(api::ContinuationReasonCode::EffectOutcomeUnknown)
        };
        let result = operations::reconcile_command(
            &self.pool,
            &op.operation_id,
            op.version,
            verified,
            &id.borrow(),
        )
        .await;
        self.command_result(result, id, &op.source_run_id, Some(&op))
            .await
    }

    async fn verified_reconciliation_candidate(
        &self,
        op: &operations::Continuation,
    ) -> Result<operations::Continuation> {
        let (count, unverified):(i64,i64) = sqlx::query_as("SELECT COUNT(*),COALESCE(SUM(status<>'verified' OR generation<>?2),0) FROM run_continuation_steps WHERE operation_id=?1")
            .bind(&op.operation_id).bind(op.generation).fetch_one(&self.pool).await?;
        anyhow::ensure!(count > 0 && unverified == 0, "effect_outcome_unknown");
        let receipt:Option<String> = sqlx::query_scalar("SELECT receipt_sha256 FROM run_continuation_steps WHERE operation_id=? AND step_key='selected_inputs_and_target' AND status='verified'")
            .bind(&op.operation_id).fetch_optional(&self.pool).await?.flatten();
        let receipt = receipt.context("effect_outcome_unknown")?;
        let root = self.config.owned_parent.join(&op.operation_id);
        anyhow::ensure!(
            std::path::Path::new(&op.plan_ref).parent() == Some(root.as_path())
                && std::path::Path::new(&op.target_ref).parent() == Some(root.as_path()),
            "unsafe_path"
        );
        if let Some(stored) = &op.manifest_sha256 {
            anyhow::ensure!(*stored == receipt, "prepared_material_changed");
        }
        let mut candidate = op.clone();
        candidate.manifest_ref = Some(
            root.join("manifest.json")
                .to_str()
                .context("unsafe_path")?
                .into(),
        );
        candidate.manifest_sha256 = Some(receipt);
        tokio::time::timeout(std::time::Duration::from_secs(120), async {
            let manifest = super::read_verified_manifest(&candidate).await?;
            preview::verify_current_target(&self.pool, &manifest).await?;
            verify_rollout(&manifest).await?;
            super::read_verified_manifest(&candidate).await?;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("continuation_budget_exceeded")??;
        Ok(candidate)
    }

    async fn get(
        &self,
        op: operations::Continuation,
        request: api::ContinuationGetRequest,
    ) -> Result<Value> {
        let mut value = serde_json::to_value(readback::compact(&op, self.config.enabled)?)?;
        value["admission"]["reconcile_available"] = json!(true);
        value["manifest_page"] = json!([]);
        value["next_cursor"] = Value::Null;
        if op.manifest_sha256.is_some() {
            let manifest = match super::read_manifest(&op).await {
                Ok(manifest) => manifest,
                Err(error) => {
                    let code = known_reason(&error)
                        .unwrap_or(api::ContinuationReasonCode::PreparedMaterialChanged);
                    let message = serde_json::to_value(code)?;
                    value["verification_status"] = json!("failed");
                    value["holds"] = json!([{"code":code,"message":message}]);
                    value["next_actions"] = json!(["resolve_hold"]);
                    let typed: api::RunContinuationReadbackV1 = serde_json::from_value(value)?;
                    return Ok(serde_json::to_value(typed)?);
                }
            };
            let entries = manifest.plan["entries"]
                .as_array()
                .context("manifest entries missing")?;
            let digest =
                ContentDigest::from_sha256_hex(op.manifest_sha256.as_deref().expect("manifest"))?;
            let start = match request.cursor {
                Some(cursor) => {
                    let c: ManifestCursor = serde_json::from_str(cursor.as_str())
                        .map_err(|_| ServiceError::InvalidParams)?;
                    if c.version != 1
                        || c.operation != op.operation_id
                        || c.manifest != digest
                        || c.offset >= entries.len()
                    {
                        return Err(ServiceError::InvalidParams.into());
                    }
                    c.offset
                }
                None => 0,
            };
            let end = (start + usize::from(request.limit.get())).min(entries.len());
            let page = entries[start..end]
                .iter()
                .map(preview::wire_manifest_entry)
                .collect::<Result<Vec<_>>>()?;
            value["manifest_page"] = serde_json::to_value(page)?;
            if end < entries.len() {
                value["next_cursor"] = json!(serde_json::to_string(&ManifestCursor {
                    version: 1,
                    operation: op.operation_id.clone(),
                    manifest: digest,
                    offset: end
                })?);
            }
        } else if request.cursor.is_some() {
            return Err(ServiceError::InvalidParams.into());
        }
        let typed: api::RunContinuationReadbackV1 = serde_json::from_value(value)?;
        Ok(serde_json::to_value(typed)?)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestCursor {
    version: u8,
    operation: String,
    manifest: ContentDigest,
    offset: usize,
}

struct Identity {
    caller: String,
    request: String,
    tool: String,
    intent: String,
}
impl Identity {
    fn new(
        principal: &Principal,
        tool: &str,
        request: api::CallerRequestId,
        args: &Value,
    ) -> Result<Self> {
        let mut intent = args.clone();
        // Paging is observation, not mutation intent, as in the preview plan hash.
        if tool == "runs.continue_blocked" {
            for field in ["cursor", "limit"] {
                intent.as_object_mut().context("request")?.remove(field);
            }
        }
        Ok(Self {
            caller: bare(&canonical_digest(
                &json!({"schema":"p039_caller_v1","id":principal.id,"class":principal.class}),
            )?)
            .into(),
            request: request.as_uuid().to_string(),
            tool: tool.into(),
            intent: bare(&canonical_digest(&json!({"tool":tool,"arguments":intent}))?).into(),
        })
    }
    fn borrow(&self) -> commands::CommandIdentity<'_> {
        commands::CommandIdentity {
            caller_fingerprint: &self.caller,
            caller_request_id: &self.request,
            command: &self.tool,
            intent_sha256: &self.intent,
        }
    }
}

fn bare(digest: &ContentDigest) -> &str {
    digest
        .as_str()
        .strip_prefix("sha256:")
        .expect("ContentDigest")
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|_| ServiceError::InvalidParams.into())
}
fn can_read(principal: &Principal, run: &str) -> bool {
    match &principal.run_scope {
        None => principal.class == PrincipalClass::Operator,
        Some(scope) => scope.iter().any(|id| id == run),
    }
}
fn authorize(tool: &str, principal: &Principal) -> Result<()> {
    use CapabilityToolId::*;
    let capability = match tool {
        "runs.continuation_preview" => RunsContinuationPreview,
        "runs.continue_blocked" => RunsContinueBlocked,
        "runs.continuation_get" => RunsContinuationGet,
        "runs.continuation_activate" => RunsContinuationActivate,
        "runs.continuation_abort" => RunsContinuationAbort,
        "runs.continuation_reconcile" => RunsContinuationReconcile,
        _ => return Err(ServiceError::InvalidParams.into()),
    };
    if !auth::is_tool_allowed(principal, tool) || !principal.tool_capabilities.contains(&capability)
    {
        return Err(ServiceError::Unauthorized.into());
    }
    if !matches!(capability, RunsContinuationPreview | RunsContinuationGet) {
        auth::require_p039_mutation(principal, capability)
            .map_err(|_| ServiceError::Unauthorized)?;
    }
    Ok(())
}
fn preview_input(request: api::ContinuationPreviewRequest) -> Result<preview::PreviewInput> {
    Ok(preview::PreviewInput {
        source_run_id: RunId(request.run_id.as_uuid()),
        target: serde_json::from_value(serde_json::to_value(request.target)?)?,
        selection: preview::PreviewSelection {
            proposal_input: serde_json::from_value(serde_json::to_value(
                request.selection.proposal_input,
            )?)?,
            reference_inputs: serde_json::from_value(serde_json::to_value(
                request.selection.reference_inputs,
            )?)?,
            include_dirty_work: true,
            excluded_workspace_paths: request
                .selection
                .excluded_workspace_paths
                .into_vec()
                .into_iter()
                .map(|e| (e.path.as_str().into(), e.reason.as_str().into()))
                .collect(),
        },
        profile: "implementation_restart_v1".into(),
        cursor: request.cursor.map(|c| c.as_str().into()),
        limit: Some(request.limit.get()),
    })
}
fn conflict(request: &str) -> Result<api::RunContinuationCommandResultV1> {
    Ok(serde_json::from_value(
        json!({"schema_version":"run_continuation_command_result_v1","status":"denied","caller_request_id":request,
        "journal_id":null,"operation":null,"denial":{"code":"idempotency_conflict","message":"idempotency_conflict"},"next_action":"resolve_hold","replay_of":null}),
    )?)
}
fn known_reason(error: &anyhow::Error) -> Option<api::ContinuationReasonCode> {
    // Legacy repository helpers carry closed reason identifiers in anyhow. Only
    // exact leading identifiers are admitted; no raw error string crosses the API.
    if let Some(sqlx::Error::Database(error)) = error.downcast_ref::<sqlx::Error>() {
        if let Ok(code) =
            serde_json::from_value::<api::ContinuationReasonCode>(json!(error.message()))
        {
            return Some(code);
        }
    }
    error.chain().find_map(|cause| {
        serde_json::from_value::<api::ContinuationReasonCode>(json!(cause
            .to_string()
            .split(':')
            .next()?
            .trim()))
        .ok()
    })
}
fn commit_error(error: anyhow::Error) -> anyhow::Error {
    if error
        .downcast_ref::<commands::CommitOutcomeUnknown>()
        .is_some()
    {
        ServiceError::CommandOutcomeUnknown.into()
    } else {
        error
    }
}
async fn verify_rollout(manifest: &super::ManifestV1) -> Result<()> {
    let m = manifest.clone();
    tokio::task::spawn_blocking(move || {
        let input = m
            .inputs
            .iter()
            .find(|i| i.role == domain::run_carry_forward::EntryRole::ExecutionSeed)
            .context("artifact_provenance_invalid")?;
        let root = super::safe_files::SafeRoot::open(&m.workspace.metadata_root)?;
        let file = root.open_file(&input.target_relative_path)?;
        anyhow::ensure!(
            file.metadata()?.len() <= 256 * 1024 * 1024,
            "continuation_budget_exceeded"
        );
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(
            &mut std::io::Read::take(file, 256 * 1024 * 1024 + 1),
            &mut bytes,
        )?;
        anyhow::ensure!(
            bytes.len() <= 256 * 1024 * 1024,
            "continuation_budget_exceeded"
        );
        anyhow::ensure!(
            ContentDigest::of(&bytes) == input.content_sha256,
            "prepared_material_changed"
        );
        let failures =
            crate::rollout_contract_preflight::approved_proposal_rollout_contract_lint_failures(
                &bytes,
                &m.workspace.metadata_root.join(&input.target_relative_path),
                m.workspace.checkout_root.to_str().context("unsafe_path")?,
            )?;
        anyhow::ensure!(failures.is_empty(), "rollout_hold");
        Ok(())
    })
    .await?
}
