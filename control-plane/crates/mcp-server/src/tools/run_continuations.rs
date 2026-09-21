//! P039 transport boundary. Durable admission, replay and effects belong to the service.

use anyhow::Result;
use async_trait::async_trait;
use auth::{boundary::PolicyDecision, Principal, PrincipalClass};
use domain::run_carry_forward_api::*;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::protocol::{JsonRpcResponse, McpTool};

pub const TOOL_NAMES: [&str; 6] = [
    "runs.continuation_preview",
    "runs.continue_blocked",
    "runs.continuation_get",
    "runs.continuation_activate",
    "runs.continuation_reconcile",
    "runs.continuation_abort",
];

/// Implementations must recheck canonical authority before replay/commit, enforce
/// rollout and source fences, and return the corresponding strict domain output.
/// Expected holds are typed command results; uncertain commits use the error below.
#[async_trait]
pub trait ContinuationService: Send + Sync {
    async fn call(&self, tool: &str, principal: &Principal, arguments: Value) -> Result<Value>;
    fn admission_enabled(&self) -> bool {
        false
    }
}

/// Stable adapter errors, downcast through anyhow (never matched by error text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationServiceError {
    Unauthorized,
    InvalidParams,
    AdmissionUnavailable,
    CommandOutcomeUnknown,
}

impl std::fmt::Display for ContinuationServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unauthorized => "continuation access denied",
            Self::InvalidParams => "invalid continuation parameters",
            Self::AdmissionUnavailable => "continuation admission unavailable",
            Self::CommandOutcomeUnknown => "continuation command outcome unknown",
        })
    }
}
impl std::error::Error for ContinuationServiceError {}

#[async_trait]
impl ContinuationService for engine::run_carry_forward::CarryForwardService {
    fn admission_enabled(&self) -> bool {
        self.enabled()
    }
    async fn call(&self, tool: &str, principal: &Principal, arguments: Value) -> Result<Value> {
        engine::run_carry_forward::CarryForwardService::call(self, tool, principal, arguments)
            .await
            .map_err(map_engine_error)
    }
}

pub(crate) async fn compact_fields(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
    principal: &Principal,
    config: engine::run_carry_forward::readback::ReadbackConfig,
) -> Result<Value> {
    use engine::run_carry_forward::readback;
    if !readback::can_read_run(principal, &run_id.to_string()) {
        return Ok(
            json!({"continued_from_run_id":null,"continued_as_run_id":null,"carry_forward_readback":null}),
        );
    }
    readback::public_fields(&readback::authorized_links(pool, run_id, principal, config).await?)
}

fn map_engine_error(error: anyhow::Error) -> anyhow::Error {
    use engine::run_carry_forward::ServiceError;
    match error.downcast_ref::<ServiceError>() {
        Some(ServiceError::Unauthorized) => ContinuationServiceError::Unauthorized.into(),
        Some(ServiceError::InvalidParams) => ContinuationServiceError::InvalidParams.into(),
        Some(ServiceError::AdmissionUnavailable) => {
            ContinuationServiceError::AdmissionUnavailable.into()
        }
        Some(ServiceError::CommandOutcomeUnknown) => {
            ContinuationServiceError::CommandOutcomeUnknown.into()
        }
        None => error,
    }
}

#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[test]
    fn p039_engine_errors_map_by_type_even_with_context() {
        use engine::run_carry_forward::ServiceError as EngineError;
        for (source, expected) in [
            (
                EngineError::Unauthorized,
                ContinuationServiceError::Unauthorized,
            ),
            (
                EngineError::InvalidParams,
                ContinuationServiceError::InvalidParams,
            ),
            (
                EngineError::AdmissionUnavailable,
                ContinuationServiceError::AdmissionUnavailable,
            ),
            (
                EngineError::CommandOutcomeUnknown,
                ContinuationServiceError::CommandOutcomeUnknown,
            ),
        ] {
            let error = map_engine_error(anyhow::Error::new(source).context("private context"));
            assert_eq!(
                error.downcast_ref::<ContinuationServiceError>(),
                Some(&expected)
            );
        }
        let unrelated = map_engine_error(anyhow::anyhow!("continuation access denied"));
        assert!(unrelated
            .downcast_ref::<ContinuationServiceError>()
            .is_none());
    }

    #[test]
    fn p039_concrete_service_implements_local_trait() {
        fn assert_service<T: ContinuationService>() {}
        assert_service::<engine::run_carry_forward::CarryForwardService>();
    }
}

pub fn is_tool(name: &str) -> bool {
    TOOL_NAMES.contains(&super::canonical_tool_name(name))
}
pub fn is_mutation(name: &str) -> bool {
    matches!(
        super::canonical_tool_name(name),
        "runs.continue_blocked"
            | "runs.continuation_activate"
            | "runs.continuation_reconcile"
            | "runs.continuation_abort"
    )
}

pub(crate) fn authorize(
    tool: &str,
    principal: &Principal,
    policy: Option<&auth::boundary::BoundaryPolicy>,
) -> std::result::Result<Option<String>, ContinuationServiceError> {
    use ContinuationServiceError::Unauthorized;
    let id = super::capability_id_for(tool)
        .filter(|_| is_tool(tool))
        .ok_or(Unauthorized)?;
    if !auth::is_tool_allowed(principal, tool) {
        return Err(Unauthorized);
    }
    if is_mutation(tool) {
        auth::require_p039_mutation(principal, id).map_err(|_| Unauthorized)?;
    }
    // Empty scope is never global; non-operators must have an explicit run scope.
    if principal.run_scope.as_ref().is_some_and(Vec::is_empty)
        || (principal.class != PrincipalClass::Operator && principal.run_scope.is_none())
    {
        return Err(Unauthorized);
    }
    let policy = policy.ok_or(Unauthorized)?;
    let decision = policy.evaluate(
        auth::derive_caller_class_for_mcp(principal).as_str(),
        "mcp_tools_call",
        Some(tool),
    );
    let decision = match decision {
        PolicyDecision::Shadow { matched_decision } => *matched_decision,
        other => other,
    };
    match decision {
        PolicyDecision::Allow { row_id } => Ok(row_id),
        _ => Err(Unauthorized),
    }
}

fn can_read(principal: &Principal, run_id: &str) -> bool {
    if auth::check_p080_run_scope(principal, Some(run_id)).is_err() {
        return false;
    }
    // The shared helper treats Operators as global and does not restrict
    // Observers; P039 additionally enforces every explicit scope for both.
    match &principal.run_scope {
        None => principal.class == PrincipalClass::Operator,
        Some(scope) => scope.iter().any(|id| id == run_id),
    }
}

fn decode<T: DeserializeOwned>(value: &Value) -> Result<T> {
    // Raw transport validation MUST happen first: Value cannot preserve duplicate keys.
    serde_json::from_value(value.clone())
        .map_err(|_| ContinuationServiceError::InvalidParams.into())
}

pub(crate) async fn execute(
    tool: &str,
    arguments: Value,
    pool: &SqlitePool,
    principal: &Principal,
    service: Option<&dyn ContinuationService>,
) -> Result<Value> {
    use ContinuationServiceError::{AdmissionUnavailable, InvalidParams, Unauthorized};
    if serde_json::to_vec(&arguments)?.len() > MAX_REQUEST_BYTES {
        return Err(InvalidParams.into());
    }
    let (mut source, operation, caller) = match tool {
        "runs.continuation_preview" => {
            let r: ContinuationPreviewRequest = decode(&arguments)?;
            (Some(r.run_id.as_uuid().to_string()), None, None)
        }
        "runs.continue_blocked" => {
            let r: ContinueBlockedRequest = decode(&arguments)?;
            (
                Some(r.run_id.as_uuid().to_string()),
                None,
                Some(r.caller_request_id),
            )
        }
        "runs.continuation_get" => {
            let r: ContinuationGetRequest = decode(&arguments)?;
            (None, Some(r.operation_id.as_uuid().to_string()), None)
        }
        "runs.continuation_activate" => {
            let r: ContinuationActivateRequest = decode(&arguments)?;
            (
                None,
                Some(r.operation_id.as_uuid().to_string()),
                Some(r.caller_request_id),
            )
        }
        "runs.continuation_reconcile" => {
            let r: ContinuationReconcileRequest = decode(&arguments)?;
            (
                None,
                Some(r.operation_id.as_uuid().to_string()),
                Some(r.caller_request_id),
            )
        }
        "runs.continuation_abort" => {
            let r: ContinuationAbortRequest = decode(&arguments)?;
            (
                None,
                Some(r.operation_id.as_uuid().to_string()),
                Some(r.caller_request_id),
            )
        }
        _ => return Err(InvalidParams.into()),
    };
    let service = service.ok_or(AdmissionUnavailable)?;
    if let Some(source) = &source {
        if !can_read(principal, source) {
            return Err(Unauthorized.into());
        }
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM runs WHERE id = ?)")
            .bind(source)
            .fetch_one(pool)
            .await?;
        if !exists {
            return Err(Unauthorized.into());
        }
    }
    if let Some(operation) = &operation {
        // Filter routing metadata itself by scope: foreign and missing rows both
        // yield None, before the service can load details or an idempotent replay.
        let scope = principal
            .run_scope
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT source_run_id, successor_run_id FROM run_continuations WHERE operation_id = ?
             AND (? IS NULL OR (source_run_id IN (SELECT value FROM json_each(?))
               AND (successor_run_id IS NULL OR successor_run_id IN (SELECT value FROM json_each(?)))))",
        )
        .bind(operation)
        .bind(scope.as_deref())
        .bind(scope.as_deref())
        .bind(scope.as_deref())
        .fetch_optional(pool)
        .await?;
        let (owner, successor) = row.ok_or(Unauthorized)?;
        if !can_read(principal, &owner)
            || successor
                .as_deref()
                .is_some_and(|id| !can_read(principal, id))
        {
            return Err(Unauthorized.into());
        }
        source = Some(owner);
    }
    if is_mutation(tool) {
        match db::repos::audit_log::audit_budget_requires_safe_mode(pool).await {
            Ok(false) => {}
            _ => return Err(AdmissionUnavailable.into()),
        }
    }
    let page_limit = arguments["limit"].as_u64().unwrap_or(100) as usize;
    let result = service.call(tool, principal, arguments).await?;
    // Validate typed bounds/identity; semantic sanitization remains with the
    // service. Never synthesize acceptance or guess denial after a fault.
    if let Some(caller) = caller {
        let output: RunContinuationCommandResultV1 = serde_json::from_value(result.clone())?;
        anyhow::ensure!(
            output.caller_request_id == caller,
            "continuation result identity mismatch"
        );
        if let (Some(expected), Some(summary)) = (&operation, &output.operation) {
            anyhow::ensure!(
                summary.operation_id.as_uuid().to_string() == *expected,
                "continuation operation mismatch"
            );
        }
    } else if tool == "runs.continuation_get" {
        let output: RunContinuationReadbackV1 = serde_json::from_value(result.clone())?;
        anyhow::ensure!(
            Some(output.operation_id.as_uuid().to_string()) == operation,
            "continuation operation mismatch"
        );
        anyhow::ensure!(
            Some(output.source_run_id.as_uuid().to_string()) == source,
            "continuation source mismatch"
        );
        anyhow::ensure!(
            output.manifest_page.as_slice().len() <= page_limit,
            "continuation page exceeds requested limit"
        );
        if !can_read(principal, &output.source_run_id.as_uuid().to_string())
            || output
                .successor_run_id
                .is_some_and(|id| !can_read(principal, &id.as_uuid().to_string()))
        {
            return Err(Unauthorized.into());
        }
    } else if result["schema_version"] == "run_carry_forward_preview_stale_v1" {
        let _: RunCarryForwardPreviewStaleV1 = serde_json::from_value(result.clone())?;
    } else if result["schema_version"] == "run_carry_forward_preview_hold_v1" {
        let output: RunCarryForwardPreviewHoldV1 = serde_json::from_value(result.clone())?;
        anyhow::ensure!(
            Some(output.source_run_id.as_uuid().to_string()) == source,
            "continuation source mismatch"
        );
    } else {
        let output: RunCarryForwardPreviewPageV1 = serde_json::from_value(result.clone())?;
        anyhow::ensure!(
            output.entries.as_slice().len() <= page_limit,
            "continuation page exceeds requested limit"
        );
        anyhow::ensure!(
            Some(output.plan_summary.source_run_id.as_uuid().to_string()) == source,
            "continuation source mismatch"
        );
    }
    Ok(result)
}

pub(crate) fn error_response(
    id: Option<Value>,
    principal: &Principal,
    error: &anyhow::Error,
) -> JsonRpcResponse {
    match error.downcast_ref::<ContinuationServiceError>() {
        Some(
            ContinuationServiceError::Unauthorized | ContinuationServiceError::AdmissionUnavailable,
        ) => JsonRpcResponse::policy_denial(
            id,
            if matches!(
                error.downcast_ref(),
                Some(ContinuationServiceError::AdmissionUnavailable)
            ) {
                "P039_ADMISSION_UNAVAILABLE"
            } else {
                "CAPABILITY_OUT_OF_SCOPE"
            },
            auth::derive_caller_class_for_mcp(principal).as_str(),
            None,
            "p081-boundary-matrix-v1",
        ),
        Some(ContinuationServiceError::InvalidParams) => {
            JsonRpcResponse::error(id, -32602, "invalid continuation parameters".into())
        }
        Some(ContinuationServiceError::CommandOutcomeUnknown) => JsonRpcResponse::error_with_data(
            id,
            -32603,
            "command outcome unknown",
            json!({"schema_version":"p039_transport_error_v1",
                "code":"command_outcome_unknown","next_action":"replay_same_request"}),
        ),
        None => {
            JsonRpcResponse::error_with_data(id, -32603, "INTERNAL", json!({"code":"INTERNAL"}))
        }
    }
}

fn object(fields: &[(&str, Value)], required: &[&str]) -> Value {
    json!({"type":"object","properties":fields.iter().map(|(k,v)| (k.to_string(),v.clone())).collect::<serde_json::Map<_,_>>(),
        "required":required,"additionalProperties":false})
}
fn all(fields: &[(&str, Value)]) -> Value {
    object(fields, &fields.iter().map(|(k, _)| *k).collect::<Vec<_>>())
}
fn nullable(schema: Value) -> Value {
    json!({"anyOf":[schema,{"type":"null"}]})
}
fn array(items: Value, max: u64) -> Value {
    json!({"type":"array","items":items,"maxItems":max})
}
fn text(max: u64) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max,"pattern":r"^(?![\s]*$)[^\u0000]+$",
        "x-maxUtf8Bytes":max,"description":format!("Nonblank, no NUL, at most {max} UTF-8 bytes.")})
}
fn integer(max: u64) -> Value {
    json!({"type":"integer","minimum":0,"maximum":max})
}
fn uuid() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"})
}
fn caller() -> Value {
    json!({"type":"string","pattern":"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"})
}
fn digest() -> Value {
    json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"})
}
fn version() -> Value {
    json!({"type":"integer","minimum":1,"maximum":i64::MAX})
}
fn limit() -> Value {
    json!({"type":"integer","minimum":1,"maximum":500,"default":100})
}
fn profile() -> Value {
    json!({"type":"string","const":"implementation_restart_v1"})
}
fn relative() -> Value {
    json!({"type":"string","minLength":1,"maxLength":4096,"x-maxUtf8Bytes":4096,
        "pattern":r"^(?!/)(?!.*(?:^|/)\.{1,2}(?:/|$))(?!.*//)[^\\:\u0000-\u001f\u007f-\u009f]+$",
        "description":"Relative path, at most 4096 UTF-8 bytes; no empty/dot segments, controls, colon or backslash. A trailing slash is allowed."})
}
fn selection() -> Value {
    let reference = all(&[
        (
            "kind",
            json!({"type":"string","enum":["artifact","carried_input"]}),
        ),
        ("id", uuid()),
    ]);
    let mut references = array(reference.clone(), 128);
    references["uniqueItems"] = json!(true);
    let mut exclusions = array(all(&[("path", relative()), ("reason", text(4096))]), 128);
    exclusions["uniqueItems"] = json!(true);
    exclusions["description"] = json!("Unique relative paths after removing a trailing slash; duplicate paths with different reasons are also invalid.");
    all(&[
        ("proposal_input", reference),
        ("reference_inputs", references),
        ("include_dirty_work", json!({"type":"boolean","const":true})),
        ("excluded_workspace_paths", exclusions),
    ])
}
fn target(preview: bool) -> Value {
    let delivery = object(
        &[
            ("repo_identifier", text(4096)),
            ("repo_root", text(4096)),
            ("base_branch", text(4096)),
            ("worktree_base_path", text(4096)),
            ("target_branch", text(4096)),
            ("release_target_id", nullable(text(4096))),
            ("release_mode", nullable(text(64))),
        ],
        &[
            "repo_identifier",
            "repo_root",
            "base_branch",
            "worktree_base_path",
            "target_branch",
        ],
    );
    let fields = [
        ("workflow_yaml_path", text(4096)),
        ("agent_catalog_yaml_path", text(4096)),
        (
            "expected_workflow_snapshot_hash",
            if preview {
                nullable(digest())
            } else {
                digest()
            },
        ),
        (
            "expected_catalog_snapshot_hash",
            if preview {
                nullable(digest())
            } else {
                digest()
            },
        ),
        (
            "delivery_configuration_json",
            json!({"type":"string","maxLength":1048576,
            "contentMediaType":"application/json","contentSchema":delivery,"x-maxUtf8Bytes":1048576,
            "description":"Strict delivery JSON string; unknown and duplicate fields are rejected."}),
        ),
    ];
    if preview {
        object(
            &fields,
            &[
                "workflow_yaml_path",
                "agent_catalog_yaml_path",
                "delivery_configuration_json",
            ],
        )
    } else {
        all(&fields)
    }
}

fn input_schema(name: &str) -> Value {
    match name {
        "runs.continuation_preview" | "runs.continue_blocked" => {
            let preview = name == "runs.continuation_preview";
            let mut fields = vec![
                ("run_id", uuid()),
                ("target", target(preview)),
                ("selection", selection()),
                ("profile", profile()),
                ("cursor", nullable(text(4096))),
                ("limit", limit()),
            ];
            let mut required = vec!["run_id", "target", "selection", "profile"];
            if !preview {
                fields.extend([
                    ("expected_plan_sha256", digest()),
                    ("caller_request_id", caller()),
                    ("reason", text(4096)),
                ]);
                required.extend(["expected_plan_sha256", "caller_request_id", "reason"]);
            }
            object(&fields, &required)
        }
        "runs.continuation_get" => object(
            &[
                ("operation_id", uuid()),
                ("cursor", nullable(text(4096))),
                ("limit", limit()),
            ],
            &["operation_id"],
        ),
        _ => {
            let mut fields = vec![
                ("operation_id", uuid()),
                ("expected_version", version()),
                ("caller_request_id", caller()),
            ];
            fields.push(if name == "runs.continuation_activate" {
                ("expected_manifest_sha256", digest())
            } else {
                ("reason", text(4096))
            });
            all(&fields)
        }
    }
}

fn next_action() -> Value {
    json!({"type":"string","enum":["none","inspect","await_worker","refresh_preview","read_operation","explicit_reconcile","resolve_hold"]})
}
fn denial() -> Value {
    all(&[
        (
            "code",
            json!({"type":"string","enum":["source_not_blocked","source_busy","source_continued",
        "continuation_in_progress","unsupported_frontier","target_profile_invalid","target_policy_incompatible",
        "source_changed","prepared_material_changed","artifact_provenance_invalid","unsafe_path","destination_conflict",
        "headless_effect_unresolved","effect_outcome_unknown","historical_binding_unproven","continuation_budget_exceeded",
        "idempotency_conflict","stale_operation_version","continuation_capacity","storage_unavailable","rollout_hold","preview_stale"]}),
        ),
        ("message", text(4096)),
    ])
}
fn command_schema() -> Value {
    let operation = all(&[
        ("operation_id", uuid()),
        (
            "phase",
            json!({"type":"string","enum":["preparing","prepared","aborting","needs_reconciliation","activated","aborted"]}),
        ),
        ("version", version()),
        ("successor_run_id", nullable(uuid())),
    ]);
    let mut s = all(&[
        (
            "schema_version",
            json!({"const":"run_continuation_command_result_v1"}),
        ),
        ("status", json!({"enum":["accepted","denied","replayed"]})),
        ("caller_request_id", caller()),
        ("journal_id", nullable(uuid())),
        ("operation", nullable(operation)),
        ("denial", nullable(denial())),
        ("next_action", next_action()),
        ("replay_of", json!({"enum":[null,"accepted","denied"]})),
    ]);
    s["allOf"] = json!([
        {"if":{"properties":{"status":{"const":"replayed"}}},"then":{"properties":{"replay_of":{"enum":["accepted","denied"]}}},
            "else":{"properties":{"replay_of":{"type":"null"}}}},
        {"if":{"anyOf":[{"properties":{"status":{"const":"accepted"}}},{"properties":{"replay_of":{"const":"accepted"}}}]},
            "then":{"properties":{"journal_id":{"type":"string"},"operation":{"type":"object"},"denial":{"type":"null"}}},
            "else":{"properties":{"denial":{"type":"object"}}}},
        {"if":{"properties":{"operation":{"type":"null"}}},"then":{"properties":{"journal_id":{"type":"null"}}}},
        {"properties":{"operation":{"if":{"type":"object","properties":{"phase":{"const":"activated"}}},
            "then":{"properties":{"successor_run_id":{"type":"string"}}},"else":{"properties":{"successor_run_id":{"type":"null"}}}}}}
    ]);
    s
}

fn entry_schema() -> Value {
    let base = vec![
        ("entry_id", text(256)),
        ("source_namespace", text(256)),
        ("source_artifact_id", nullable(uuid())),
        ("logical_name", text(256)),
        ("source_relative_path", relative()),
        ("reason", text(4096)),
    ];
    let mut excluded = base.clone();
    excluded.push(("role", json!({"const":"excluded"})));
    let mut hashed = base;
    hashed.extend([
        (
            "role",
            json!({"enum":["execution_seed","reference_only","preserve_only"]}),
        ),
        ("sha256", digest()),
        ("bytes", integer(u64::MAX)),
        ("mode", integer(u32::MAX.into())),
        ("source_contract", nullable(text(256))),
        ("source_schema_version", nullable(text(256))),
        ("target_logical_name", text(256)),
        (
            "historical_approval_relation",
            json!({"enum":["bound","historical_binding_unproven","not_applicable"]}),
        ),
    ]);
    json!({"oneOf":[all(&hashed),all(&excluded)]})
}

fn readback_schema() -> Value {
    all(&[
        (
            "schema_version",
            json!({"const":"run_continuation_readback_v1"}),
        ),
        ("operation_id", uuid()),
        ("source_run_id", uuid()),
        ("successor_run_id", nullable(uuid())),
        ("phase", text(256)),
        ("version", version()),
        ("plan_sha256", digest()),
        ("manifest_sha256", nullable(digest())),
        (
            "execution_disposition",
            json!({"enum":["source_reserved","source_historical","successor_pending_review","aborted_source_blocked"]}),
        ),
        (
            "verification_status",
            json!({"enum":["unverified","pending","verified","failed","unknown"]}),
        ),
        ("holds", array(denial(), 128)),
        ("next_actions", array(next_action(), 8)),
        ("journal_id", nullable(uuid())),
        ("updated_at", json!({"type":"string","format":"date-time"})),
        (
            "projection_freshness",
            json!({"enum":["fresh","stale","unknown"]}),
        ),
        (
            "admission",
            all(&[
                (
                    "schema_version",
                    json!({"const":"run_continuation_admission_v1"}),
                ),
                ("enabled", json!({"type":"boolean"})),
                ("reason", nullable(text(4096))),
                ("fences_enforced", json!({"type":"boolean"})),
                ("reconcile_available", json!({"type":"boolean"})),
            ]),
        ),
        ("manifest_page", array(entry_schema(), 500)),
        ("next_cursor", nullable(text(4096))),
    ])
}

fn preview_schema() -> Value {
    let git = json!({"type":"string","pattern":"^(?:[0-9a-f]{40}|[0-9a-f]{64})$"});
    let witness = all(&[
        ("workflow_snapshot_hash", digest()),
        ("catalog_snapshot_hash", digest()),
        ("stage_execution_id", uuid()),
        ("cursor", text(4096)),
        ("journal_ids", array(uuid(), 128)),
        ("approval_ids", array(uuid(), 128)),
        ("artifact_generation_ids", array(uuid(), 128)),
        ("idea_sha256", digest()),
        ("head", git.clone()),
        ("tree", git.clone()),
        ("index_sha256", digest()),
        ("content_sha256", digest()),
        (
            "directory_identities",
            array(
                all(&[
                    ("root", text(256)),
                    ("device", integer(u64::MAX)),
                    ("inode", integer(u64::MAX)),
                ]),
                128,
            ),
        ),
        ("settled_ownership_effect_sha256", digest()),
    ]);
    let workspace = all(&[
        ("base_branch", text(4096)),
        ("base_revision", git.clone()),
        ("head", git),
        ("staged_patch_sha256", digest()),
        ("unstaged_patch_sha256", digest()),
        ("inventory_sha256", digest()),
        ("untracked_count", integer(50000)),
        ("deleted_count", integer(50000)),
        ("link_count", integer(50000)),
        ("target_content_sha256", digest()),
    ]);
    let limits = all(&[
        ("max_entries", integer(50000)),
        ("max_preservation_bytes", integer(2147483648)),
        ("max_file_bytes", integer(268435456)),
        ("max_reference_artifacts", integer(128)),
        ("max_request_bytes", integer(1048576)),
        ("max_summary_bytes", integer(65536)),
        ("default_page_size", limit()),
        ("max_page_size", limit()),
        ("streaming_buffer_bytes", integer(1048576)),
    ]);
    let mut summary = all(&[
        ("source_run_id", uuid()),
        ("source_witness", witness),
        ("target", target(false)),
        ("profile", profile()),
        ("selection", selection()),
        ("workspace_snapshot", workspace),
        (
            "capability_delta",
            all(&[
                ("added", array(text(256), 128)),
                ("removed", array(text(256), 128)),
                ("sha256", digest()),
            ]),
        ),
        ("holds", array(denial(), 128)),
        ("limits", limits),
    ]);
    summary["description"] = json!("Serialized plan summary is bounded to 64 KiB of UTF-8 JSON.");
    let page = all(&[
        (
            "schema_version",
            json!({"const":"run_carry_forward_preview_page_v1"}),
        ),
        ("plan_summary", summary),
        ("entries", array(entry_schema(), 500)),
        ("entry_count", integer(50000)),
        ("plan_sha256", digest()),
        ("next_cursor", nullable(text(4096))),
    ]);
    let stale = all(&[
        (
            "schema_version",
            json!({"const":"run_carry_forward_preview_stale_v1"}),
        ),
        ("code", json!({"const":"preview_stale"})),
        ("next_action", json!({"const":"refresh_preview"})),
    ]);
    let mut holds = array(denial(), 128);
    holds["minItems"] = json!(1);
    let hold = all(&[
        (
            "schema_version",
            json!({"const":"run_carry_forward_preview_hold_v1"}),
        ),
        ("source_run_id", uuid()),
        ("holds", holds),
        (
            "next_action",
            json!({"type":"string","enum":["inspect","resolve_hold","refresh_preview"]}),
        ),
    ]);
    json!({"type":"object","oneOf":[page,stale,hold]})
}

pub fn tool_specs() -> Vec<McpTool> {
    TOOL_NAMES.into_iter().map(|name| McpTool {
        name: name.into(),
        description: match name {
            "runs.continuation_preview" => "Read a bounded carry-forward plan preview without reserving or mutating the source.",
            "runs.continue_blocked" => "Reserve an eligible blocked source and enqueue carry-forward preparation; does not activate a successor.",
            "runs.continuation_get" => "Read canonical continuation state and a bounded manifest page; does not repair.",
            "runs.continuation_activate" => "Verify prepared material and atomically activate one successor.",
            "runs.continuation_reconcile" => "Explicitly inspect interrupted effects and record verification or holds; does not retry effects.",
            _ => "Abort a pre-activation continuation after effects settle, preserving files and leaving the source blocked.",
        }.into(),
        input_schema: input_schema(name),
        output_schema: Some(if is_mutation(name) { command_schema() }
            else if name == "runs.continuation_get" { readback_schema() } else { preview_schema() }),
    }).collect()
}
