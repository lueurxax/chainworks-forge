use async_graphql::*;
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::sync::{Arc, OnceLock};

use db::repos::sessions::{SessionEventPage, SessionGenerationPage, SessionLineagePage};
use domain::session::{
    SessionEvent, SessionEventType, SessionGeneration, SessionGenerationStatus, SessionLineage,
};

// ── P046 config stored in schema context ─────────────────────────────────────

#[derive(Clone, Debug)]
pub struct P046Config {
    pub enabled: bool,
    /// Capacity of the per-subscriber mpsc channel for sessionStatusChanged.
    /// Default is 64 in production; smaller values allow testing slow-consumer disconnect.
    pub subscription_channel_capacity: usize,
}

/// Live mutable principal table handle for P046 subscription per-emission auth.
///
/// The schema stores a snapshot `auth::PrincipalTable` for synchronous per-request
/// auth. For long-lived subscriptions, per-emission rechecks must observe the most
/// current revocation state. This handle wraps an `Arc<RwLock<Option<…>>>` where
/// `None` represents an unavailable/unloadable auth source (deny-all, fail-closed):
/// - The daemon can write a refreshed table when principals.json changes.
/// - On principals.json reload failure the daemon calls `mark_unavailable` so
///   existing subscribers see `auth_ok == false` and terminate rather than
///   continuing under stale grants.
/// - Tests can write a revoked table or mark unavailable to verify termination.
/// - Subscriptions read-lock the current value on every emission check.
#[derive(Clone, Debug)]
pub struct P046LivePrincipalHandle(pub Arc<tokio::sync::RwLock<Option<auth::PrincipalTable>>>);

#[derive(Clone, Debug)]
pub struct P046LiveCredential {
    pub principal_id: String,
    pub token_fingerprint: String,
}

impl P046LivePrincipalHandle {
    pub fn new(table: auth::PrincipalTable) -> Self {
        Self(Arc::new(tokio::sync::RwLock::new(Some(table))))
    }

    /// Replace the current table (for daemon live-reload or test revocation).
    pub async fn update(&self, new_table: auth::PrincipalTable) {
        let mut w = self.0.write().await;
        *w = Some(new_table);
    }

    /// Mark the auth source unavailable (fail-closed). Called on principals.json
    /// reload failure so that per-emission auth_ok returns false and running
    /// subscriptions terminate with authorization_recheck_failed rather than
    /// continuing under stale grants.
    pub async fn mark_unavailable(&self) {
        let mut w = self.0.write().await;
        *w = None;
    }

    /// Returns true iff the live auth source is available AND the principal has
    /// current operator-read permission. Returns false when the source is unavailable
    /// (fail-closed) or when the principal is missing/revoked/downgraded.
    pub async fn auth_ok(&self, principal_id: &str) -> bool {
        let guard = self.0.read().await;
        let table = match guard.as_ref() {
            Some(t) => t,
            // Auth source unavailable (reload failed) → deny-all fail-closed.
            None => return false,
        };
        match auth::find_principal_by_id(table, principal_id) {
            Some(p) if p.class == auth::PrincipalClass::Operator => {
                match auth::is_subscription_allowed_by_surface_policy(table, principal_id) {
                    Some(true) => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Returns true iff the same principal id is still authorized and its bearer
    /// credential fingerprint still matches the live principal table.
    pub async fn auth_ok_for_credential(&self, credential: &P046LiveCredential) -> bool {
        let guard = self.0.read().await;
        let table = match guard.as_ref() {
            Some(t) => t,
            None => return false,
        };
        let Some(current_fingerprint) =
            auth::principal_token_fingerprint_by_id(table, &credential.principal_id)
        else {
            return false;
        };
        if current_fingerprint != credential.token_fingerprint {
            return false;
        }
        match auth::find_principal_by_id(table, &credential.principal_id) {
            Some(p) if p.class == auth::PrincipalClass::Operator => matches!(
                auth::is_subscription_allowed_by_surface_policy(table, &credential.principal_id),
                Some(true)
            ),
            _ => false,
        }
    }
}

pub fn p046_visible(ctx: &Context<'_>) -> bool {
    ctx.data::<P046Config>()
        .map(|config| config.enabled)
        .unwrap_or(false)
}

// ── Enums ─────────────────────────────────────────────────────────────────────

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SessionGenerationStatus")]
pub enum GqlSessionGenerationStatus {
    Active,
    Closed,
    Invalidated,
    Reset,
    Failed,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

impl From<&SessionGenerationStatus> for GqlSessionGenerationStatus {
    fn from(s: &SessionGenerationStatus) -> Self {
        match s {
            SessionGenerationStatus::Active => Self::Active,
            SessionGenerationStatus::Closed => Self::Closed,
            SessionGenerationStatus::Invalidated => Self::Invalidated,
            SessionGenerationStatus::Reset => Self::Reset,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SessionEventType")]
pub enum GqlSessionEventType {
    LineageCreated,
    GenerationStarted,
    SessionReused,
    GenerationClosed,
    GenerationInvalidated,
    OperatorResetRecorded,
    CheckpointRehydrated,
    RepairAttempted,
    RepairFailed,
    ContextWindowObserved,
    #[graphql(name = "UNKNOWN_EVENT_SHAPE")]
    UnknownEventShape,
}

impl From<&SessionEventType> for GqlSessionEventType {
    fn from(t: &SessionEventType) -> Self {
        match t {
            SessionEventType::Created => Self::GenerationStarted,
            SessionEventType::Reused => Self::SessionReused,
            SessionEventType::Invalidated => Self::GenerationInvalidated,
            SessionEventType::Closed => Self::GenerationClosed,
            SessionEventType::OperatorReset => Self::OperatorResetRecorded,
            SessionEventType::OutputContractRepairStarted => Self::RepairAttempted,
            SessionEventType::OutputContractRepairFailed => Self::RepairFailed,
            SessionEventType::OutputContractRepairSucceeded => Self::RepairAttempted,
            SessionEventType::OutputContractRepairSkipped => Self::RepairAttempted,
            SessionEventType::BudgetExceeded => Self::ContextWindowObserved,
            SessionEventType::Compacted => Self::UnknownEventShape,
            SessionEventType::CodeWriterCompletionStarted => Self::UnknownEventShape,
            SessionEventType::CodeWriterCompletionSucceeded => Self::UnknownEventShape,
            SessionEventType::CodeWriterCompletionFailed => Self::UnknownEventShape,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SessionHealthState")]
pub enum GqlSessionHealthState {
    Healthy,
    Warning,
    Critical,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SessionHealthSeverity")]
pub enum GqlSessionHealthSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SessionStatusChangedStatus")]
pub enum GqlSessionStatusChangedStatus {
    Active,
    Closed,
    Invalidated,
    Reset,
    Failed,
    #[graphql(name = "UNKNOWN")]
    Unknown,
    ResyncRequired,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SessionInvocationOwnerKind")]
pub enum GqlSessionInvocationOwnerKind {
    Stage,
    Agent,
    System,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

// ── SessionEndReason ──────────────────────────────────────────────────────────

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SessionEndReason")]
pub enum GqlSessionEndReason {
    Completed,
    Failed,
    OperatorReset,
    Invalidated,
    ContextPressure,
    TransportError,
    Timeout,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

impl GqlSessionEndReason {
    pub fn from_str(s: &str) -> Self {
        match s {
            "COMPLETED" | "completed" => Self::Completed,
            "FAILED" | "failed" => Self::Failed,
            "OPERATOR_RESET" | "operator_reset" => Self::OperatorReset,
            "INVALIDATED" | "invalidated" => Self::Invalidated,
            "CONTEXT_PRESSURE" | "context_pressure" => Self::ContextPressure,
            "TRANSPORT_ERROR" | "transport_error" => Self::TransportError,
            "TIMEOUT" | "timeout" => Self::Timeout,
            _ => Self::Unknown,
        }
    }
}

// ── Connection infrastructure ─────────────────────────────────────────────────

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "PageInfo")]
pub struct GqlPageInfo {
    pub has_next_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

// ── SessionLineage ────────────────────────────────────────────────────────────

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionLineage")]
pub struct GqlSessionLineage {
    pub id: ID,
    pub run_id: String,
    pub agent_id: String,
    pub lineage_key: String,
    pub session_reuse_scope: String,
    pub session_family_id: Option<String>,
    pub active_generation_id: Option<String>,
    pub created_at: String,
    pub closed_at: Option<String>,
    pub health_state: GqlSessionHealthState,
    /// Total number of generations for this lineage.
    pub generation_count: i64,
    /// ISO-8601 timestamp of the most recent session event for this lineage, if any.
    pub latest_event_at: Option<String>,
    /// The currently active generation for this lineage, if any and if loaded.
    pub active_generation: Option<GqlSessionGeneration>,
}

impl GqlSessionLineage {
    pub fn from_lineage_with_stats(
        l: SessionLineage,
        stats: Option<&db::repos::sessions::LineageStats>,
    ) -> Self {
        let health_state = compute_lineage_health_state(&l, stats);
        Self {
            id: ID::from(l.id.clone()),
            run_id: l.run_id,
            agent_id: l.agent_id,
            lineage_key: l.lineage_id,
            session_reuse_scope: l.session_reuse_scope,
            session_family_id: l.session_family_id,
            active_generation_id: l.active_generation_id,
            created_at: l.created_at.to_rfc3339(),
            closed_at: l.closed_at.map(|t| t.to_rfc3339()),
            health_state,
            generation_count: stats.map(|s| s.generation_count).unwrap_or(0),
            latest_event_at: stats
                .and_then(|s| s.latest_event_at)
                .map(|t| t.to_rfc3339()),
            active_generation: None,
        }
    }
}

/// Compute a per-lineage health state from lineage row data and aggregated stats.
/// Uses the active generation status from stats (loaded by aggregate_lineage_stats_for_page)
/// to determine:
/// - HEALTHY: lineage has an active generation that is currently active.
/// - WARNING: active_generation_id points to an invalidated/reset generation
///   (invalidated_active_generation threshold).
///   Or active_generation_id is set but no generation row found (active_generation_missing).
/// - UNKNOWN: no active generation, lineage is initializing/closed, or insufficient data.
fn compute_lineage_health_state(
    l: &SessionLineage,
    stats: Option<&db::repos::sessions::LineageStats>,
) -> GqlSessionHealthState {
    use domain::session::SessionGenerationStatus;
    let Some(active_gen_id) = &l.active_generation_id else {
        // No active generation: lineage is initializing or normally closed.
        return GqlSessionHealthState::Unknown;
    };
    let _ = active_gen_id;
    let Some(s) = stats else {
        // No stats loaded → cannot determine health.
        return GqlSessionHealthState::Unknown;
    };
    match &s.active_generation_status {
        None => {
            // active_generation_id is set but the generation row was not found.
            GqlSessionHealthState::Warning
        }
        Some(SessionGenerationStatus::Active) => GqlSessionHealthState::Healthy,
        Some(SessionGenerationStatus::Invalidated) | Some(SessionGenerationStatus::Reset) => {
            GqlSessionHealthState::Warning
        }
        Some(SessionGenerationStatus::Closed) => {
            // Active generation is closed; lineage may be in a transitional state.
            GqlSessionHealthState::Unknown
        }
    }
}

impl From<SessionLineage> for GqlSessionLineage {
    fn from(l: SessionLineage) -> Self {
        Self::from_lineage_with_stats(l, None)
    }
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionLineageEdge")]
pub struct GqlSessionLineageEdge {
    pub cursor: String,
    pub node: GqlSessionLineage,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionLineageConnection")]
pub struct GqlSessionLineageConnection {
    pub edges: Vec<GqlSessionLineageEdge>,
    pub nodes: Vec<GqlSessionLineage>,
    pub page_info: GqlPageInfo,
}

impl GqlSessionLineageConnection {
    pub fn from_page(page: SessionLineagePage) -> Self {
        Self::from_page_with_stats(page, None)
    }

    pub fn from_page_with_stats(
        page: SessionLineagePage,
        stats: Option<&std::collections::HashMap<String, db::repos::sessions::LineageStats>>,
    ) -> Self {
        let edges: Vec<GqlSessionLineageEdge> = page
            .items
            .iter()
            .map(|l| GqlSessionLineageEdge {
                cursor: db::repos::sessions::encode_session_lineage_cursor(l),
                node: GqlSessionLineage::from_lineage_with_stats(
                    l.clone(),
                    stats.and_then(|m| m.get(&l.id)),
                ),
            })
            .collect();
        let nodes: Vec<GqlSessionLineage> = page
            .items
            .into_iter()
            .map(|l| {
                let s = stats.and_then(|m| m.get(&l.id));
                GqlSessionLineage::from_lineage_with_stats(l, s)
            })
            .collect();
        Self {
            edges,
            nodes,
            page_info: GqlPageInfo {
                has_next_page: page.has_next_page,
                start_cursor: page.start_cursor,
                end_cursor: page.end_cursor,
            },
        }
    }
}

// ── SessionGeneration ─────────────────────────────────────────────────────────

// Per-process instance salt: generated once at startup using a CSPRNG (UUID v4 via getrandom).
// Changes on every daemon restart, ensuring derived references are not stable
// across control-plane instances as required by the proposal. Using a CSPRNG-backed
// UUID ensures the salt is non-guessable even when daemon PID and start-time are observable.
static P046_INSTANCE_SALT: OnceLock<[u8; 16]> = OnceLock::new();

fn p046_instance_salt() -> &'static [u8; 16] {
    P046_INSTANCE_SALT.get_or_init(|| *uuid::Uuid::new_v4().as_bytes())
}

/// Derive a non-secret, non-reversible, instance-scoped reference for a sensitive session field.
/// Uses SHA-256 over "p046_v1|{tag}|{run_id}|{instance_salt}|{raw}" — non-reversible for
/// operator-visible GraphQL fields, not stable across control-plane instances.
pub fn derive_scoped_ref(run_id: &str, raw: &str, tag: &str) -> String {
    derive_scoped_ref_with_salt(run_id, raw, tag, p046_instance_salt())
}

/// Same as `derive_scoped_ref` but accepts an explicit salt — used in tests to prove
/// different salts produce different references for the same input (cross-instance property).
pub fn derive_scoped_ref_with_salt(run_id: &str, raw: &str, tag: &str, salt: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"p046_v1|");
    hasher.update(tag.as_bytes());
    hasher.update(b"|");
    hasher.update(run_id.as_bytes());
    hasher.update(b"|");
    hasher.update(salt);
    hasher.update(b"|");
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    digest[..16].iter().map(|b| format!("{b:02x}")).collect()
}

pub fn redact_working_directory(raw: &str) -> String {
    if raw.is_empty() {
        return "<redacted>".to_string();
    }
    "<outside-workspace redacted>".to_string()
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionGeneration")]
pub struct GqlSessionGeneration {
    pub id: ID,
    pub lineage_id: String,
    pub generation: i64,
    pub has_provider_session: bool,
    pub provider_session_ref: Option<ID>,
    pub binding_profile_ref: ID,
    pub invocation_owner_kind: GqlSessionInvocationOwnerKind,
    pub invocation_owner_ref: ID,
    pub workspace_mode: String,
    pub working_directory_display: String,
    pub runtime_provider: String,
    pub runtime_model: String,
    pub status: GqlSessionGenerationStatus,
    pub turn_count: i64,
    pub estimated_input_tokens: i64,
    pub latest_cached_input_tokens: Option<i64>,
    pub latest_output_tokens: Option<i64>,
    pub latest_model_context_window: Option<i64>,
    pub cumulative_prompt_tokens: i64,
    pub cumulative_cost_cents: i64,
    pub created_at: String,
    pub last_activity_at: Option<String>,
    pub ended_at: Option<String>,
    pub end_reason: Option<GqlSessionEndReason>,
    pub is_active: bool,
}

impl GqlSessionGeneration {
    pub fn from_domain(g: SessionGeneration, run_id: &str) -> Self {
        let has_provider_session = g.provider_session_id.is_some();
        let provider_session_ref = g
            .provider_session_id
            .as_deref()
            .map(|raw| ID::from(derive_scoped_ref(run_id, raw, "psr")));
        let binding_profile_ref =
            ID::from(derive_scoped_ref(run_id, &g.binding_fingerprint, "bpr"));
        let invocation_owner_ref =
            ID::from(derive_scoped_ref(run_id, &g.invocation_owner_key, "ior"));
        let working_directory_display = redact_working_directory(&g.working_directory);
        let is_active = matches!(g.status, SessionGenerationStatus::Active);
        let invocation_owner_kind = if g.invocation_owner_key.starts_with("stage:") {
            GqlSessionInvocationOwnerKind::Stage
        } else if g.invocation_owner_key.starts_with("agent:") {
            GqlSessionInvocationOwnerKind::Agent
        } else if g.invocation_owner_key.starts_with("system:") {
            GqlSessionInvocationOwnerKind::System
        } else {
            GqlSessionInvocationOwnerKind::Unknown
        };
        Self {
            id: ID::from(g.id),
            lineage_id: g.lineage_id,
            generation: g.generation,
            has_provider_session,
            provider_session_ref,
            binding_profile_ref,
            invocation_owner_kind,
            invocation_owner_ref,
            workspace_mode: g.workspace_mode,
            working_directory_display,
            runtime_provider: g.runtime_provider,
            runtime_model: g.runtime_model,
            status: GqlSessionGenerationStatus::from(&g.status),
            turn_count: g.turn_count,
            estimated_input_tokens: g.estimated_input_tokens,
            latest_cached_input_tokens: g.latest_cached_input_tokens,
            latest_output_tokens: g.latest_output_tokens,
            latest_model_context_window: g.latest_model_context_window,
            cumulative_prompt_tokens: g.cumulative_prompt_tokens,
            cumulative_cost_cents: g.cumulative_cost_cents,
            created_at: g.created_at.to_rfc3339(),
            last_activity_at: g.last_activity_at.map(|t| t.to_rfc3339()),
            ended_at: g.ended_at.map(|t| t.to_rfc3339()),
            end_reason: g.end_reason.as_deref().map(GqlSessionEndReason::from_str),
            is_active,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionGenerationEdge")]
pub struct GqlSessionGenerationEdge {
    pub cursor: String,
    pub node: GqlSessionGeneration,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionGenerationConnection")]
pub struct GqlSessionGenerationConnection {
    pub edges: Vec<GqlSessionGenerationEdge>,
    pub nodes: Vec<GqlSessionGeneration>,
    pub page_info: GqlPageInfo,
}

impl GqlSessionGenerationConnection {
    pub fn from_page(page: SessionGenerationPage, run_id: &str) -> Self {
        let edges: Vec<GqlSessionGenerationEdge> = page
            .items
            .iter()
            .map(|g| GqlSessionGenerationEdge {
                cursor: db::repos::sessions::encode_session_generation_cursor(g),
                node: GqlSessionGeneration::from_domain(g.clone(), run_id),
            })
            .collect();
        let nodes: Vec<GqlSessionGeneration> = page
            .items
            .into_iter()
            .map(|g| GqlSessionGeneration::from_domain(g, run_id))
            .collect();
        Self {
            edges,
            nodes,
            page_info: GqlPageInfo {
                has_next_page: page.has_next_page,
                start_cursor: page.start_cursor,
                end_cursor: page.end_cursor,
            },
        }
    }
}

// ── SessionEvent ──────────────────────────────────────────────────────────────

const P046_REDACTION_ALLOWED_KEYS: &[&str] = &[
    "schemaVersion",
    "summaryCode",
    "providerKind",
    "modelFamily",
    "reuseDisposition",
    "resetReason",
    "endReason",
    "tokenEstimateBucket",
    "contextWindowPressureBucket",
    "checkpointPresent",
    "repairAttemptCount",
    "safeDiagnosticCode",
];
const P046_EVENT_DETAILS_REDACTION_SCHEMA_VERSION: &str = "p046_event_details_redaction_v1";
// Maximum length for individual string scalar values.
const P046_REDACTION_VALUE_MAX_BYTES: usize = 256;
// Operator-safe string content: only bounded identifier/code characters.
// Rejects paths (slashes), prompts (spaces/newlines), credentials (colons/quotes), etc.
const P046_SAFE_STRING_CONTENT_MAX_BYTES: usize = 128;

/// Typed redacted event details. All fields are optional; absent means the source
/// JSON did not contain that key. Nested objects or arrays in source JSON always
/// fail closed (detailsJsonRedacted = null with a bounded warning).
#[derive(SimpleObject, Clone, Debug, Default)]
#[graphql(name = "SessionEventDetailsRedacted")]
pub struct GqlSessionEventDetailsRedacted {
    pub schema_version: Option<String>,
    pub summary_code: Option<String>,
    pub provider_kind: Option<String>,
    pub model_family: Option<String>,
    pub reuse_disposition: Option<String>,
    pub reset_reason: Option<String>,
    pub end_reason: Option<String>,
    pub token_estimate_bucket: Option<String>,
    pub context_window_pressure_bucket: Option<String>,
    pub checkpoint_present: Option<bool>,
    pub repair_attempt_count: Option<i64>,
    pub safe_diagnostic_code: Option<String>,
}

pub fn redact_event_details(
    details_json: Option<&str>,
) -> (Option<GqlSessionEventDetailsRedacted>, Vec<String>) {
    let Some(json) = details_json else {
        return (None, vec![]);
    };
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (None, vec!["redaction_unknown_details_shape".to_string()]),
    };
    let obj = match &value {
        serde_json::Value::Object(m) => m,
        _ => return (None, vec!["redaction_unknown_details_shape".to_string()]),
    };
    if obj
        .keys()
        .any(|k| !P046_REDACTION_ALLOWED_KEYS.contains(&k.as_str()))
    {
        return (None, vec!["redaction_unknown_details_shape".to_string()]);
    }
    if obj.get("schemaVersion").and_then(|v| v.as_str())
        != Some(P046_EVENT_DETAILS_REDACTION_SCHEMA_VERSION)
    {
        return (None, vec!["redaction_unknown_schema_version".to_string()]);
    }
    // Known bearer-like token prefixes. Strings starting with these are rejected even
    // if they pass the alphanumeric charset check — they are structurally credential-shaped.
    const CREDENTIAL_DENY_PREFIXES: &[&str] = &[
        "ghp_",
        "ghs_",
        "gho_",
        "github_pat_", // GitHub
        "sk-",         // OpenAI / Anthropic / generic secret keys
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-", // Slack
        "ya29.", // Google OAuth access tokens
        "eyJ",   // JWT (base64 header prefix)
        // AWS IAM access key ID prefixes (access keys, session keys, batch operation,
        // certificate, and credential group prefixes — all structurally credential-shaped).
        "AKIA",
        "ASIA",
        "ABIA",
        "ACCA",
        // Google API key prefix.
        "AIza",
    ];

    // Closed-vocabulary sets for enum-like fields. These fields have a finite expected
    // value set; values outside the set fail closed rather than passing through
    // as arbitrary identifier strings (which could carry high-entropy tokens).
    const PROVIDER_KIND_VOCAB: &[&str] = &[
        "claude", "codex", "gemini", "auggie", "junie", "goose", "unknown",
    ];
    const REUSE_DISPOSITION_VOCAB: &[&str] = &["new", "reused", "reset", "invalidated", "unknown"];
    const RESET_REASON_VOCAB: &[&str] = &[
        "operator_reset",
        "context_pressure",
        "transport_error",
        "timeout",
        "invalidated",
        "output_contract_repair",
        "unknown",
    ];
    const END_REASON_VOCAB: &[&str] = &[
        "completed",
        "failed",
        "operator_reset",
        "invalidated",
        "context_pressure",
        "transport_error",
        "timeout",
        "unknown",
    ];
    const TOKEN_ESTIMATE_BUCKET_VOCAB: &[&str] = &[
        "none",
        "tiny",
        "small",
        "medium",
        "large",
        "very_large",
        "unknown",
    ];
    const CONTEXT_WINDOW_PRESSURE_BUCKET_VOCAB: &[&str] =
        &["none", "low", "medium", "high", "critical", "unknown"];
    // Closed vocabulary for summaryCode: lifecycle/outcome codes produced by control-plane
    // session management. Secret-shaped tokens (AWS AKIA, Google AIza, high-entropy base64)
    // cannot appear in this set, making the field safe even without prefix checks.
    const SUMMARY_CODE_VOCAB: &[&str] = &[
        "created",
        "active",
        "reused",
        "closed",
        "reset",
        "invalidated",
        "failed",
        "completed",
        "context_pressure",
        "checkpoint",
        "repair",
        "operator_reset",
        "transport_error",
        "timeout",
        "unknown",
    ];
    // Closed vocabulary for modelFamily: known provider model family identifiers.
    const MODEL_FAMILY_VOCAB: &[&str] = &[
        "claude", "gpt", "gemini", "codex", "llama", "mistral", "palm", "unknown",
    ];
    // Closed vocabulary for safeDiagnosticCode: bounded reason codes from
    // p046_session_graphql_vocab_v1. Matches the proposal's reason_codes list exactly.
    const SAFE_DIAGNOSTIC_CODE_VOCAB: &[&str] = &[
        "stale_active_generation",
        "active_generation_missing",
        "generation_without_lineage",
        "repeated_operator_reset",
        "invalidated_active_generation",
        "context_window_pressure",
        "repair_failure_recent",
        "no_session_data",
        "transient_db_unavailable",
        "redaction_unknown_event_type",
        "redaction_unknown_schema_version",
        "redaction_unknown_details_shape",
        "redaction_size_limit_exceeded",
        "subscription_resync_required",
        "slow_consumer_disconnected",
        "authorization_recheck_failed",
        "sqlite_retry_exhausted",
        "unknown",
    ];

    // Value-type safety: all values must be scalars (string, number, bool, null).
    // Nested objects or arrays are forbidden even under allowed keys.
    // String values must be bounded-length identifier/code-like content only:
    // alphanumeric plus '_', '-', '.' — this blocks paths (slashes), prompt/transcript
    // text (spaces, newlines), credentials (colons, quotes), and diagnostics.
    for (key, v) in obj.iter() {
        match v {
            serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                return (None, vec!["redaction_unknown_details_shape".to_string()]);
            }
            serde_json::Value::String(s) if s.len() > P046_REDACTION_VALUE_MAX_BYTES => {
                return (None, vec!["redaction_size_limit_exceeded".to_string()]);
            }
            serde_json::Value::String(s)
                if s.len() > P046_SAFE_STRING_CONTENT_MAX_BYTES
                    || !s
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') =>
            {
                return (None, vec!["redaction_unknown_details_shape".to_string()]);
            }
            serde_json::Value::String(s)
                if CREDENTIAL_DENY_PREFIXES.iter().any(|p| s.starts_with(p)) =>
            {
                return (None, vec!["redaction_unknown_details_shape".to_string()]);
            }
            // Reject all-hex strings of >= 16 chars: these are UUID/SHA-shaped tokens
            // that should not appear in the operator-safe enum/code fields.
            serde_json::Value::String(s)
                if s.len() >= 16 && s.chars().all(|c| c.is_ascii_hexdigit()) =>
            {
                return (None, vec!["redaction_unknown_details_shape".to_string()]);
            }
            // Per-field closed vocabulary for enum-like fields. All string-valued
            // fields must have a closed vocabulary to prevent secret-shaped tokens
            // (e.g. AWS AKIA keys, Google AIza keys, high-entropy base58/base64)
            // from passing through alphanumeric charset checks.
            serde_json::Value::String(s) => {
                let vocab: Option<&[&str]> = match key.as_str() {
                    "providerKind" => Some(PROVIDER_KIND_VOCAB),
                    "reuseDisposition" => Some(REUSE_DISPOSITION_VOCAB),
                    "resetReason" => Some(RESET_REASON_VOCAB),
                    "endReason" => Some(END_REASON_VOCAB),
                    "tokenEstimateBucket" => Some(TOKEN_ESTIMATE_BUCKET_VOCAB),
                    "contextWindowPressureBucket" => Some(CONTEXT_WINDOW_PRESSURE_BUCKET_VOCAB),
                    "summaryCode" => Some(SUMMARY_CODE_VOCAB),
                    "modelFamily" => Some(MODEL_FAMILY_VOCAB),
                    "safeDiagnosticCode" => Some(SAFE_DIAGNOSTIC_CODE_VOCAB),
                    // schemaVersion is checked above (must equal the pinned version string).
                    _ => None,
                };
                if let Some(allowed) = vocab {
                    let s_lower = s.to_lowercase();
                    if !allowed.contains(&s_lower.as_str()) {
                        return (None, vec!["redaction_unknown_details_shape".to_string()]);
                    }
                }
            }
            _ => {}
        }
    }
    // Total serialized size guard.
    let total_len = serde_json::to_string(&value)
        .map(|s| s.len())
        .unwrap_or(usize::MAX);
    if total_len > 4096 {
        return (None, vec!["redaction_size_limit_exceeded".to_string()]);
    }

    let get_str = |key: &str| obj.get(key).and_then(|v| v.as_str()).map(str::to_string);

    let redacted = GqlSessionEventDetailsRedacted {
        schema_version: get_str("schemaVersion"),
        summary_code: get_str("summaryCode"),
        provider_kind: get_str("providerKind"),
        model_family: get_str("modelFamily"),
        reuse_disposition: get_str("reuseDisposition"),
        reset_reason: get_str("resetReason"),
        end_reason: get_str("endReason"),
        token_estimate_bucket: get_str("tokenEstimateBucket"),
        context_window_pressure_bucket: get_str("contextWindowPressureBucket"),
        checkpoint_present: obj.get("checkpointPresent").and_then(|v| v.as_bool()),
        repair_attempt_count: obj.get("repairAttemptCount").and_then(|v| v.as_i64()),
        safe_diagnostic_code: get_str("safeDiagnosticCode"),
    };
    (Some(redacted), vec![])
}

/// Per-event safe typed fields extracted from details_json.
/// Each field is populated only when its value passes the operator-safe charset, length,
/// and credential-prefix checks. Unknown keys in the source JSON are silently ignored,
/// so typedDetails may be returned even when detailsJsonRedacted is null (partial-safe).
/// detailsJsonRedacted is returned only when the ENTIRE object conforms to the allowlist.
#[derive(SimpleObject, Clone, Debug, Default)]
#[graphql(name = "SessionEventTypedDetails")]
pub struct GqlSessionEventTypedDetails {
    pub schema_version: Option<String>,
    pub summary_code: Option<String>,
    pub provider_kind: Option<String>,
    pub model_family: Option<String>,
    pub reuse_disposition: Option<String>,
    pub reset_reason: Option<String>,
    pub end_reason: Option<String>,
    pub token_estimate_bucket: Option<String>,
    pub context_window_pressure_bucket: Option<String>,
    pub checkpoint_present: Option<bool>,
    pub repair_attempt_count: Option<i64>,
    pub safe_diagnostic_code: Option<String>,
}

/// Extract typed safe fields from event details JSON, ignoring unknown keys.
/// Applies the same per-value safety checks as redact_event_details.
/// Returns None when JSON is absent, unparseable, or non-object, or when no
/// allowed key is present in the object.
pub fn extract_typed_details(details_json: Option<&str>) -> Option<GqlSessionEventTypedDetails> {
    let json_str = details_json?;
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let obj = value.as_object()?;
    if obj.get("schemaVersion").and_then(|v| v.as_str())
        != Some(P046_EVENT_DETAILS_REDACTION_SCHEMA_VERSION)
    {
        return None;
    }

    const CREDENTIAL_DENY_PREFIXES: &[&str] = &[
        "ghp_",
        "ghs_",
        "gho_",
        "github_pat_",
        "sk-",
        "xoxb-",
        "xoxp-",
        "xoxa-",
        "xoxr-",
        "ya29.",
        "eyJ",
        // AWS IAM access key ID prefixes.
        "AKIA",
        "ASIA",
        "ABIA",
        "ACCA",
        // Google API key prefix.
        "AIza",
    ];
    // Closed vocabularies for fields that are not already enum-restricted by redact_event_details.
    // Applying them here ensures typedDetails also rejects secret-shaped tokens under these keys.
    const SUMMARY_CODE_VOCAB: &[&str] = &[
        "created",
        "active",
        "reused",
        "closed",
        "reset",
        "invalidated",
        "failed",
        "completed",
        "context_pressure",
        "checkpoint",
        "repair",
        "operator_reset",
        "transport_error",
        "timeout",
        "unknown",
    ];
    const MODEL_FAMILY_VOCAB: &[&str] = &[
        "claude", "gpt", "gemini", "codex", "llama", "mistral", "palm", "unknown",
    ];
    const PROVIDER_KIND_VOCAB: &[&str] = &[
        "claude", "codex", "gemini", "auggie", "junie", "goose", "unknown",
    ];
    const REUSE_DISPOSITION_VOCAB: &[&str] = &["new", "reused", "reset", "invalidated", "unknown"];
    const RESET_REASON_VOCAB: &[&str] = &[
        "operator_reset",
        "context_pressure",
        "transport_error",
        "timeout",
        "invalidated",
        "output_contract_repair",
        "unknown",
    ];
    const END_REASON_VOCAB: &[&str] = &[
        "completed",
        "failed",
        "operator_reset",
        "invalidated",
        "context_pressure",
        "transport_error",
        "timeout",
        "unknown",
    ];
    const TOKEN_ESTIMATE_BUCKET_VOCAB: &[&str] = &[
        "none",
        "tiny",
        "small",
        "medium",
        "large",
        "very_large",
        "unknown",
    ];
    const CONTEXT_WINDOW_PRESSURE_BUCKET_VOCAB: &[&str] =
        &["none", "low", "medium", "high", "critical", "unknown"];
    const SAFE_DIAGNOSTIC_CODE_VOCAB: &[&str] = &[
        "stale_active_generation",
        "active_generation_missing",
        "generation_without_lineage",
        "repeated_operator_reset",
        "invalidated_active_generation",
        "context_window_pressure",
        "repair_failure_recent",
        "no_session_data",
        "transient_db_unavailable",
        "redaction_unknown_event_type",
        "redaction_unknown_schema_version",
        "redaction_unknown_details_shape",
        "redaction_size_limit_exceeded",
        "subscription_resync_required",
        "slow_consumer_disconnected",
        "authorization_recheck_failed",
        "sqlite_retry_exhausted",
        "unknown",
    ];

    // Helper: check if a string value is operator-safe per the same rules used by
    // redact_event_details. Unsafe values are silently omitted rather than failing.
    let safe_str = |v: &serde_json::Value| -> Option<String> {
        let s = v.as_str()?;
        if s.len() > P046_REDACTION_VALUE_MAX_BYTES {
            return None;
        }
        if s.len() > P046_SAFE_STRING_CONTENT_MAX_BYTES {
            return None;
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return None;
        }
        if CREDENTIAL_DENY_PREFIXES.iter().any(|p| s.starts_with(p)) {
            return None;
        }
        if s.len() >= 16 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(s.to_string())
    };
    // Vocabulary-constrained helper: silently omits value when not in the closed set.
    let safe_vocab = |v: &serde_json::Value, vocab: &[&str]| -> Option<String> {
        let s = safe_str(v)?;
        let s_lower = s.to_lowercase();
        if vocab.contains(&s_lower.as_str()) {
            Some(s)
        } else {
            None
        }
    };

    let typed = GqlSessionEventTypedDetails {
        schema_version: obj.get("schemaVersion").and_then(safe_str),
        summary_code: obj
            .get("summaryCode")
            .and_then(|v| safe_vocab(v, SUMMARY_CODE_VOCAB)),
        provider_kind: obj
            .get("providerKind")
            .and_then(|v| safe_vocab(v, PROVIDER_KIND_VOCAB)),
        model_family: obj
            .get("modelFamily")
            .and_then(|v| safe_vocab(v, MODEL_FAMILY_VOCAB)),
        reuse_disposition: obj
            .get("reuseDisposition")
            .and_then(|v| safe_vocab(v, REUSE_DISPOSITION_VOCAB)),
        reset_reason: obj
            .get("resetReason")
            .and_then(|v| safe_vocab(v, RESET_REASON_VOCAB)),
        end_reason: obj
            .get("endReason")
            .and_then(|v| safe_vocab(v, END_REASON_VOCAB)),
        token_estimate_bucket: obj
            .get("tokenEstimateBucket")
            .and_then(|v| safe_vocab(v, TOKEN_ESTIMATE_BUCKET_VOCAB)),
        context_window_pressure_bucket: obj
            .get("contextWindowPressureBucket")
            .and_then(|v| safe_vocab(v, CONTEXT_WINDOW_PRESSURE_BUCKET_VOCAB)),
        checkpoint_present: obj.get("checkpointPresent").and_then(|v| v.as_bool()),
        repair_attempt_count: obj.get("repairAttemptCount").and_then(|v| v.as_i64()),
        safe_diagnostic_code: obj
            .get("safeDiagnosticCode")
            .and_then(|v| safe_vocab(v, SAFE_DIAGNOSTIC_CODE_VOCAB)),
    };

    // Return None if no safe field was found (fully unknown/unsafe payload).
    let has_any = typed.schema_version.is_some()
        || typed.summary_code.is_some()
        || typed.provider_kind.is_some()
        || typed.model_family.is_some()
        || typed.reuse_disposition.is_some()
        || typed.reset_reason.is_some()
        || typed.end_reason.is_some()
        || typed.token_estimate_bucket.is_some()
        || typed.context_window_pressure_bucket.is_some()
        || typed.checkpoint_present.is_some()
        || typed.repair_attempt_count.is_some()
        || typed.safe_diagnostic_code.is_some();

    if has_any {
        Some(typed)
    } else {
        None
    }
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionEvent")]
pub struct GqlSessionEvent {
    pub id: ID,
    pub lineage_id: String,
    pub generation_id: String,
    pub event_id: String,
    pub event_type: GqlSessionEventType,
    pub recorded_at: String,
    pub typed_details: Option<GqlSessionEventTypedDetails>,
    pub details_json_redacted: Option<GqlSessionEventDetailsRedacted>,
    pub redaction_warnings: Vec<String>,
}

impl From<SessionEvent> for GqlSessionEvent {
    fn from(e: SessionEvent) -> Self {
        let gql_event_type = GqlSessionEventType::from(&e.event_type);
        // Unknown event shapes must fail closed: no details leak regardless of JSON content.
        let (redacted, warnings) =
            if matches!(gql_event_type, GqlSessionEventType::UnknownEventShape) {
                db::metrics::increment_counter_with_label(
                    "session_event_redaction_total",
                    "redaction_unknown_event_type:redacted",
                );
                (None, vec!["redaction_unknown_event_type".to_string()])
            } else {
                let result = redact_event_details(e.details_json.as_deref());
                // Emit redaction metrics for any warning codes produced.
                for w in &result.1 {
                    db::metrics::increment_counter_with_label(
                        "session_event_redaction_total",
                        &format!("{w}:redacted"),
                    );
                }
                if result.1.is_empty() && result.0.is_some() {
                    db::metrics::increment_counter_with_label(
                        "session_event_redaction_total",
                        "none:ok",
                    );
                }
                result
            };
        // Typed details: extract safe fields from source JSON, ignoring unknown keys.
        // For unknown event shapes, fail closed to None (same as detailsJsonRedacted).
        let typed_details = if matches!(gql_event_type, GqlSessionEventType::UnknownEventShape) {
            None
        } else {
            extract_typed_details(e.details_json.as_deref())
        };
        Self {
            event_id: e.id.clone(),
            id: ID::from(e.id),
            lineage_id: e.lineage_id,
            generation_id: e.generation_id,
            event_type: gql_event_type,
            recorded_at: e.recorded_at.to_rfc3339(),
            typed_details,
            details_json_redacted: redacted,
            redaction_warnings: warnings,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionEventEdge")]
pub struct GqlSessionEventEdge {
    pub cursor: String,
    pub node: GqlSessionEvent,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionEventConnection")]
pub struct GqlSessionEventConnection {
    pub edges: Vec<GqlSessionEventEdge>,
    pub nodes: Vec<GqlSessionEvent>,
    pub page_info: GqlPageInfo,
}

impl GqlSessionEventConnection {
    pub fn from_page(page: SessionEventPage) -> Self {
        let gen_id_filter = page.gen_id_filter.clone();
        let edges: Vec<GqlSessionEventEdge> = page
            .items
            .iter()
            .map(|e| GqlSessionEventEdge {
                cursor: db::repos::sessions::encode_session_cursor(
                    &e.lineage_id,
                    &gen_id_filter,
                    &e.recorded_at.to_rfc3339(),
                    &e.id,
                ),
                node: GqlSessionEvent::from(e.clone()),
            })
            .collect();
        let nodes: Vec<GqlSessionEvent> = page.items.into_iter().map(Into::into).collect();
        Self {
            edges,
            nodes,
            page_info: GqlPageInfo {
                has_next_page: page.has_next_page,
                start_cursor: page.start_cursor,
                end_cursor: page.end_cursor,
            },
        }
    }
}

// ── SessionKpiSummary ─────────────────────────────────────────────────────────

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionKpiSummary")]
pub struct GqlSessionKpiSummary {
    pub run_id: String,
    pub lineage_count: i64,
    pub generation_count: i64,
    pub active_generation_count: i64,
    pub closed_generation_count: i64,
    pub reset_generation_count: i64,
    pub invalidated_generation_count: i64,
    pub reuse_event_count: i64,
    pub operator_reset_event_count: i64,
    pub total_turn_count: i64,
    pub total_prompt_tokens: i64,
    pub total_cost_cents: i64,
    pub latest_activity_at: Option<String>,
    pub stale_active_generation_count: i64,
}

// ── SessionHealth ─────────────────────────────────────────────────────────────

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionHealthWarning")]
pub struct GqlSessionHealthWarning {
    pub reason_code: String,
    pub severity: GqlSessionHealthSeverity,
    pub lineage_id: Option<String>,
    pub generation_id: Option<String>,
    pub message: String,
    pub suggested_mcp_action: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionHealthReport")]
pub struct GqlSessionHealthReport {
    pub run_id: String,
    pub state: GqlSessionHealthState,
    pub warnings: Vec<GqlSessionHealthWarning>,
    pub checked_at: String,
    pub thresholds_version: String,
}

pub const P046_HEALTH_THRESHOLDS_VERSION: &str = "p046_session_health_thresholds_v1";
pub const STALE_ACTIVE_GENERATION_MINUTES: i64 = 15;
pub const REPEATED_RESET_WINDOW_MINUTES: i64 = 30;
pub const REPEATED_RESET_THRESHOLD: usize = 2;
pub const REPAIR_FAILURE_WINDOW_MINUTES: i64 = 30;
pub const REPAIR_FAILURE_THRESHOLD: usize = 2;
pub const CONTEXT_PRESSURE_THRESHOLD_PCT: f64 = 0.85;

pub fn compute_session_health(
    data: &db::repos::sessions::SessionHealthData,
    run_id: &str,
    run_is_terminal: bool,
) -> GqlSessionHealthReport {
    let mut warnings: Vec<GqlSessionHealthWarning> = Vec::new();
    let now = Utc::now();

    if data.lineages.is_empty() {
        // Orphan generations take precedence: if generation rows exist for this run but their
        // lineage rows are absent, that is a CRITICAL integrity violation, not merely missing data.
        if data.orphan_generation_count > 0 {
            db::metrics::increment_counter_with_label(
                "session_health_warning_total",
                "generation_without_lineage:CRITICAL",
            );
            return GqlSessionHealthReport {
                run_id: run_id.to_string(),
                state: GqlSessionHealthState::Critical,
                warnings: vec![GqlSessionHealthWarning {
                    reason_code: "generation_without_lineage".to_string(),
                    severity: GqlSessionHealthSeverity::Critical,
                    lineage_id: None,
                    generation_id: None,
                    message: "One or more session generation rows reference a missing lineage."
                        .to_string(),
                    suggested_mcp_action: None,
                }],
                checked_at: now.to_rfc3339(),
                thresholds_version: P046_HEALTH_THRESHOLDS_VERSION.to_string(),
            };
        }
        db::metrics::increment_counter_with_label(
            "session_health_warning_total",
            "no_session_data:WARNING",
        );
        return GqlSessionHealthReport {
            run_id: run_id.to_string(),
            state: GqlSessionHealthState::Unknown,
            warnings: vec![GqlSessionHealthWarning {
                reason_code: "no_session_data".to_string(),
                severity: GqlSessionHealthSeverity::Warning,
                lineage_id: None,
                generation_id: None,
                message: "No session data available for this run.".to_string(),
                suggested_mcp_action: None,
            }],
            checked_at: now.to_rfc3339(),
            thresholds_version: P046_HEALTH_THRESHOLDS_VERSION.to_string(),
        };
    }

    for lineage in &data.lineages {
        // Check for stale active generation
        if let Some(gen) = data
            .active_generations
            .iter()
            .find(|g| Some(&g.id) == lineage.active_generation_id.as_ref())
        {
            // Check if active_generation_id points to an INVALIDATED or RESET generation.
            if matches!(
                gen.status,
                SessionGenerationStatus::Invalidated | SessionGenerationStatus::Reset
            ) {
                warnings.push(GqlSessionHealthWarning {
                    reason_code: "invalidated_active_generation".to_string(),
                    severity: GqlSessionHealthSeverity::Warning,
                    lineage_id: Some(lineage.id.clone()),
                    generation_id: Some(gen.id.clone()),
                    message: "Lineage active generation pointer is invalidated or reset."
                        .to_string(),
                    suggested_mcp_action: Some("use the MCP session reset capability".to_string()),
                });
            } else {
                // Only flag stale active generation while the run is non-terminal.
                if !run_is_terminal {
                    if let Some(last_activity) = gen.last_activity_at {
                        let stale_threshold =
                            now - chrono::Duration::minutes(STALE_ACTIVE_GENERATION_MINUTES);
                        if last_activity < stale_threshold {
                            warnings.push(GqlSessionHealthWarning {
                                reason_code: "stale_active_generation".to_string(),
                                severity: GqlSessionHealthSeverity::Warning,
                                lineage_id: Some(lineage.id.clone()),
                                generation_id: Some(gen.id.clone()),
                                message:
                                    "Active generation has had no activity for more than 15 minutes."
                                        .to_string(),
                                suggested_mcp_action: Some(
                                    "use the MCP session reset capability".to_string(),
                                ),
                            });
                        }
                    }
                }
                // Check context window pressure
                let pressure = if let (Some(latest_ctx), Some(latest_cached), Some(latest_out)) = (
                    gen.latest_model_context_window,
                    gen.latest_cached_input_tokens,
                    gen.latest_output_tokens,
                ) {
                    if latest_ctx > 0 {
                        Some((latest_cached + latest_out) as f64 / latest_ctx as f64)
                    } else {
                        None
                    }
                } else if gen
                    .latest_model_context_window
                    .map(|c| c > 0)
                    .unwrap_or(false)
                {
                    let ctx = gen.latest_model_context_window.unwrap();
                    Some(gen.estimated_input_tokens as f64 / ctx as f64)
                } else {
                    None
                };
                if let Some(p) = pressure {
                    if p >= CONTEXT_PRESSURE_THRESHOLD_PCT {
                        warnings.push(GqlSessionHealthWarning {
                            reason_code: "context_window_pressure".to_string(),
                            severity: GqlSessionHealthSeverity::Warning,
                            lineage_id: Some(lineage.id.clone()),
                            generation_id: Some(gen.id.clone()),
                            message: "Session context window usage is at or above 85%.".to_string(),
                            suggested_mcp_action: Some(
                                "use the MCP session reset capability".to_string(),
                            ),
                        });
                    }
                }
            }
        } else if lineage.active_generation_id.is_some() {
            // active_generation_id points to a missing generation
            warnings.push(GqlSessionHealthWarning {
                reason_code: "active_generation_missing".to_string(),
                severity: GqlSessionHealthSeverity::Critical,
                lineage_id: Some(lineage.id.clone()),
                generation_id: None,
                message: "Lineage references an active generation that cannot be found."
                    .to_string(),
                suggested_mcp_action: Some("use the MCP session reset capability".to_string()),
            });
        }

        // Repeated operator reset: ≥2 in last 30 minutes OR ≥3 in last 24 hours.
        let reset_count_30m = data
            .recent_operator_reset_events
            .iter()
            .filter(|e| e.lineage_id == lineage.id)
            .count();
        let reset_count_24h = data
            .operator_reset_events_24h
            .iter()
            .filter(|e| e.lineage_id == lineage.id)
            .count();
        if reset_count_30m >= REPEATED_RESET_THRESHOLD || reset_count_24h >= 3 {
            warnings.push(GqlSessionHealthWarning {
                reason_code: "repeated_operator_reset".to_string(),
                severity: GqlSessionHealthSeverity::Warning,
                lineage_id: Some(lineage.id.clone()),
                generation_id: None,
                message: "Multiple operator resets recorded in the last 30 minutes.".to_string(),
                suggested_mcp_action: None,
            });
        }

        // Repair failures
        let repair_fail_count = data
            .recent_repair_failed_events
            .iter()
            .filter(|e| e.lineage_id == lineage.id)
            .count();
        if repair_fail_count >= REPAIR_FAILURE_THRESHOLD {
            warnings.push(GqlSessionHealthWarning {
                reason_code: "repair_failure_recent".to_string(),
                severity: GqlSessionHealthSeverity::Warning,
                lineage_id: Some(lineage.id.clone()),
                generation_id: None,
                message: "Multiple repair failures recorded in the last 30 minutes.".to_string(),
                suggested_mcp_action: None,
            });
        }
    }

    // generation_without_lineage: any generation row references a missing lineage (data integrity).
    if data.orphan_generation_count > 0 {
        warnings.push(GqlSessionHealthWarning {
            reason_code: "generation_without_lineage".to_string(),
            severity: GqlSessionHealthSeverity::Critical,
            lineage_id: None,
            generation_id: None,
            message: "One or more session generation rows reference a missing lineage.".to_string(),
            suggested_mcp_action: None,
        });
    }

    let state = if warnings
        .iter()
        .any(|w| matches!(w.severity, GqlSessionHealthSeverity::Critical))
    {
        GqlSessionHealthState::Critical
    } else if warnings.is_empty() {
        GqlSessionHealthState::Healthy
    } else {
        GqlSessionHealthState::Warning
    };

    // Emit bounded health warning metrics.
    for w in &warnings {
        let severity_label = match w.severity {
            GqlSessionHealthSeverity::Info => "INFO",
            GqlSessionHealthSeverity::Warning => "WARNING",
            GqlSessionHealthSeverity::Critical => "CRITICAL",
        };
        db::metrics::increment_counter_with_label(
            "session_health_warning_total",
            &format!("{}:{severity_label}", w.reason_code),
        );
    }

    GqlSessionHealthReport {
        run_id: run_id.to_string(),
        state,
        warnings,
        checked_at: now.to_rfc3339(),
        thresholds_version: P046_HEALTH_THRESHOLDS_VERSION.to_string(),
    }
}

/// Returns UNKNOWN/transient_db_unavailable health after SQLite retry exhaustion.
pub fn transient_db_unavailable_health(run_id: &str) -> GqlSessionHealthReport {
    db::metrics::increment_counter_with_label(
        "session_health_warning_total",
        "transient_db_unavailable:WARNING",
    );
    GqlSessionHealthReport {
        run_id: run_id.to_string(),
        state: GqlSessionHealthState::Unknown,
        warnings: vec![GqlSessionHealthWarning {
            reason_code: "transient_db_unavailable".to_string(),
            severity: GqlSessionHealthSeverity::Warning,
            lineage_id: None,
            generation_id: None,
            message: "Session health data is temporarily unavailable. Retry after a moment."
                .to_string(),
            suggested_mcp_action: None,
        }],
        checked_at: Utc::now().to_rfc3339(),
        thresholds_version: P046_HEALTH_THRESHOLDS_VERSION.to_string(),
    }
}

// ── SessionStatusChangedEvent ─────────────────────────────────────────────────

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SessionStatusChangedEvent")]
pub struct GqlSessionStatusChangedEvent {
    pub run_id: String,
    pub lineage_id: Option<String>,
    pub generation_id: Option<String>,
    pub event_id: Option<String>,
    pub status: GqlSessionStatusChangedStatus,
    pub event_type: GqlSessionEventType,
    pub recorded_at: String,
    pub health_state: GqlSessionHealthState,
    pub resync_required: bool,
}

/// Derive the session status from the most recent event type.
fn status_from_event_type(
    event_type: &domain::session::SessionEventType,
) -> GqlSessionStatusChangedStatus {
    use domain::session::SessionEventType::*;
    match event_type {
        Created
        | OutputContractRepairStarted
        | OutputContractRepairSucceeded
        | OutputContractRepairSkipped
        | CodeWriterCompletionStarted
        | BudgetExceeded => GqlSessionStatusChangedStatus::Active,
        Reused => GqlSessionStatusChangedStatus::Active,
        Closed => GqlSessionStatusChangedStatus::Closed,
        Invalidated => GqlSessionStatusChangedStatus::Invalidated,
        OperatorReset => GqlSessionStatusChangedStatus::Reset,
        OutputContractRepairFailed | CodeWriterCompletionFailed => {
            GqlSessionStatusChangedStatus::Failed
        }
        Compacted | CodeWriterCompletionSucceeded => GqlSessionStatusChangedStatus::Unknown,
    }
}

pub fn session_event_to_status_changed(
    event: &SessionEvent,
    run_id: &str,
) -> GqlSessionStatusChangedEvent {
    GqlSessionStatusChangedEvent {
        run_id: run_id.to_string(),
        lineage_id: Some(event.lineage_id.clone()),
        generation_id: Some(event.generation_id.clone()),
        event_id: Some(event.id.clone()),
        status: status_from_event_type(&event.event_type),
        event_type: GqlSessionEventType::from(&event.event_type),
        recorded_at: event.recorded_at.to_rfc3339(),
        health_state: GqlSessionHealthState::Unknown,
        resync_required: false,
    }
}

pub fn resync_event(run_id: &str) -> GqlSessionStatusChangedEvent {
    let synthetic_id = format!("resync:{run_id}:{}", uuid::Uuid::new_v4());
    GqlSessionStatusChangedEvent {
        run_id: run_id.to_string(),
        lineage_id: Some(synthetic_id.clone()),
        generation_id: Some(synthetic_id.clone()),
        event_id: Some(synthetic_id),
        status: GqlSessionStatusChangedStatus::ResyncRequired,
        event_type: GqlSessionEventType::UnknownEventShape,
        recorded_at: Utc::now().to_rfc3339(),
        health_state: GqlSessionHealthState::Unknown,
        resync_required: true,
    }
}

// ── Pagination limit constants ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::session::{SessionEvent, SessionEventType};

    #[test]
    fn p046_created_generation_events_render_as_generation_started() {
        let event = SessionEvent {
            id: "event-created-generation".to_string(),
            lineage_id: "lineage-1".to_string(),
            generation_id: "generation-1".to_string(),
            event_type: SessionEventType::Created,
            recorded_at: Utc::now(),
            details_json: None,
        };

        let gql_event = GqlSessionEvent::from(event.clone());
        assert_eq!(gql_event.event_type, GqlSessionEventType::GenerationStarted);

        let status_event = session_event_to_status_changed(&event, "run-1");
        assert_eq!(
            status_event.event_type,
            GqlSessionEventType::GenerationStarted
        );
    }

    #[test]
    fn p046_resync_status_event_keeps_contract_identity_fields() {
        let event = resync_event("run-1");

        assert_eq!(event.run_id, "run-1");
        assert_eq!(event.status, GqlSessionStatusChangedStatus::ResyncRequired);
        assert!(event.resync_required);
        assert!(
            event
                .event_id
                .as_deref()
                .is_some_and(|id| id.starts_with("resync:run-1:")),
            "resync payload must carry a stable synthetic event identity: {event:?}"
        );
        assert!(
            event
                .lineage_id
                .as_deref()
                .is_some_and(|id| id.starts_with("resync:run-1:")),
            "resync payload must carry a synthetic lineage identity: {event:?}"
        );
        assert!(
            event
                .generation_id
                .as_deref()
                .is_some_and(|id| id.starts_with("resync:run-1:")),
            "resync payload must carry a synthetic generation identity: {event:?}"
        );
    }
}

pub const SESSION_LINEAGES_DEFAULT_FIRST: i64 = 100;
pub const SESSION_LINEAGES_MAX_FIRST: i64 = 500;
pub const SESSION_GENERATIONS_DEFAULT_FIRST: i64 = 100;
pub const SESSION_GENERATIONS_MAX_FIRST: i64 = 500;
pub const SESSION_EVENTS_DEFAULT_FIRST: i64 = 200;
pub const SESSION_EVENTS_MAX_FIRST: i64 = 1000;
