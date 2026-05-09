use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentExecutionId, RunId, StageExecutionId};

// ── IDs ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SideEffectId(String);

impl SideEffectId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_str(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl Default for SideEffectId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SideEffectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SideEffectId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SideEffectAttemptId(String);

impl SideEffectAttemptId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn from_str(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl Default for SideEffectAttemptId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SideEffectAttemptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SideEffectAttemptId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SideEffectSettlementId(String);

impl SideEffectSettlementId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for SideEffectSettlementId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SideEffectSettlementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SideEffectSettlementId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}


// ── EffectKind ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    GitCommit,
    GitPush,
    BuildArchive,
    ConnectUpload,
    TagCreate,
    ArtifactPublish,
}

impl EffectKind {
    pub fn is_wired(&self) -> bool {
        matches!(
            self,
            EffectKind::GitCommit
                | EffectKind::GitPush
                | EffectKind::BuildArchive
                | EffectKind::ConnectUpload
        )
    }

    pub fn lease_ttl_seconds(&self) -> i64 {
        match self {
            EffectKind::GitCommit | EffectKind::TagCreate => 30,
            EffectKind::GitPush | EffectKind::ArtifactPublish => 30,
            EffectKind::BuildArchive => 60,
            EffectKind::ConnectUpload => 60,
        }
    }

    pub fn deadline_seconds(&self) -> i64 {
        match self {
            EffectKind::GitCommit => 60,
            EffectKind::GitPush => 120,
            EffectKind::BuildArchive => 7200,
            EffectKind::ConnectUpload => 3600,
            EffectKind::TagCreate => 60,
            EffectKind::ArtifactPublish => 120,
        }
    }
}

impl std::fmt::Display for EffectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectKind::GitCommit => write!(f, "git_commit"),
            EffectKind::GitPush => write!(f, "git_push"),
            EffectKind::BuildArchive => write!(f, "build_archive"),
            EffectKind::ConnectUpload => write!(f, "connect_upload"),
            EffectKind::TagCreate => write!(f, "tag_create"),
            EffectKind::ArtifactPublish => write!(f, "artifact_publish"),
        }
    }
}

impl std::str::FromStr for EffectKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "git_commit" => Ok(EffectKind::GitCommit),
            "git_push" => Ok(EffectKind::GitPush),
            "build_archive" => Ok(EffectKind::BuildArchive),
            "connect_upload" => Ok(EffectKind::ConnectUpload),
            "tag_create" => Ok(EffectKind::TagCreate),
            "artifact_publish" => Ok(EffectKind::ArtifactPublish),
            other => Err(format!("unknown EffectKind: {other}")),
        }
    }
}

// ── SideEffectStatus ─────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectStatus {
    Prepared,
    Executing,
    ExternallyObserved,
    NeedsReconciliation,
    Settled,
    Reconciled,
    Conflict,
    Unrecoverable,
}

impl SideEffectStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SideEffectStatus::Settled
                | SideEffectStatus::Reconciled
                | SideEffectStatus::Conflict
                | SideEffectStatus::Unrecoverable
        )
    }

    pub fn is_unresolved(&self) -> bool {
        matches!(
            self,
            SideEffectStatus::Prepared
                | SideEffectStatus::Executing
                | SideEffectStatus::ExternallyObserved
                | SideEffectStatus::NeedsReconciliation
                | SideEffectStatus::Conflict
                | SideEffectStatus::Unrecoverable
        )
    }

    /// Legal transitions: returns true when moving from self → next is allowed.
    pub fn can_transition_to(&self, next: &SideEffectStatus) -> bool {
        match (self, next) {
            (SideEffectStatus::Prepared, SideEffectStatus::Executing) => true,
            (SideEffectStatus::Prepared, SideEffectStatus::NeedsReconciliation) => true,
            (SideEffectStatus::Executing, SideEffectStatus::ExternallyObserved) => true,
            (SideEffectStatus::Executing, SideEffectStatus::NeedsReconciliation) => true,
            (SideEffectStatus::Executing, SideEffectStatus::Settled) => true,
            (SideEffectStatus::Executing, SideEffectStatus::Conflict) => true,
            (SideEffectStatus::Executing, SideEffectStatus::Unrecoverable) => true,
            (SideEffectStatus::ExternallyObserved, SideEffectStatus::Settled) => true,
            (SideEffectStatus::ExternallyObserved, SideEffectStatus::NeedsReconciliation) => true,
            (SideEffectStatus::NeedsReconciliation, SideEffectStatus::Reconciled) => true,
            (SideEffectStatus::NeedsReconciliation, SideEffectStatus::Conflict) => true,
            (SideEffectStatus::NeedsReconciliation, SideEffectStatus::Unrecoverable) => true,
            (SideEffectStatus::NeedsReconciliation, SideEffectStatus::Settled) => true,
            (SideEffectStatus::Conflict, SideEffectStatus::Reconciled) => true,
            (SideEffectStatus::Conflict, SideEffectStatus::Unrecoverable) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for SideEffectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SideEffectStatus::Prepared => write!(f, "prepared"),
            SideEffectStatus::Executing => write!(f, "executing"),
            SideEffectStatus::ExternallyObserved => write!(f, "externally_observed"),
            SideEffectStatus::NeedsReconciliation => write!(f, "needs_reconciliation"),
            SideEffectStatus::Settled => write!(f, "settled"),
            SideEffectStatus::Reconciled => write!(f, "reconciled"),
            SideEffectStatus::Conflict => write!(f, "conflict"),
            SideEffectStatus::Unrecoverable => write!(f, "unrecoverable"),
        }
    }
}

impl std::str::FromStr for SideEffectStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "prepared" => Ok(SideEffectStatus::Prepared),
            "executing" => Ok(SideEffectStatus::Executing),
            "externally_observed" => Ok(SideEffectStatus::ExternallyObserved),
            "needs_reconciliation" => Ok(SideEffectStatus::NeedsReconciliation),
            "settled" => Ok(SideEffectStatus::Settled),
            "reconciled" => Ok(SideEffectStatus::Reconciled),
            "conflict" => Ok(SideEffectStatus::Conflict),
            "unrecoverable" => Ok(SideEffectStatus::Unrecoverable),
            other => Err(format!("unknown SideEffectStatus: {other}")),
        }
    }
}

// ── SideEffect row ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SideEffect {
    pub id: SideEffectId,
    pub run_id: RunId,
    pub stage_execution_id: StageExecutionId,
    pub agent_execution_id: Option<AgentExecutionId>,
    pub effect_kind: EffectKind,
    pub target_key: String,
    pub idempotency_key: String,
    pub idempotency_key_version: i64,
    pub request_fingerprint: String,
    pub request_fingerprint_version: i64,
    pub status: SideEffectStatus,
    pub owner_instance_id: Option<String>,
    pub lease_acquired_at: Option<DateTime<Utc>>,
    pub lease_renewed_at: Option<DateTime<Utc>>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub external_write_started_at: Option<DateTime<Utc>>,
    pub external_write_attempted: bool,
    pub attempt_budget_remaining: i64,
    pub expected_evidence_json: Option<String>,
    pub observed_evidence_summary_json: Option<String>,
    pub evidence_root: Option<String>,
    pub last_error_kind: Option<String>,
    pub last_error: Option<String>,
    pub settlement_txn_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── SideEffectAttempt row ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SideEffectAttempt {
    pub id: SideEffectAttemptId,
    pub side_effect_id: SideEffectId,
    pub attempt_number: i64,
    pub attempt_kind: String,
    pub owner_instance_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub exit_status: Option<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub trace_path: Option<String>,
    pub observed_evidence_summary_json: Option<String>,
    pub error_kind: Option<String>,
    pub error: Option<String>,
}

// ── SideEffectSettlement row ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SideEffectSettlement {
    pub id: SideEffectSettlementId,
    pub side_effect_id: SideEffectId,
    pub settlement_status: SideEffectStatus,
    pub settlement_source: String,
    pub receipt_artifact_id: Option<String>,
    pub reconciliation_report_artifact_id: Option<String>,
    pub applied_at: DateTime<Utc>,
    pub applied_by: Option<String>,
    pub disposition_id: Option<String>,
    pub settlement_attempt_id: Option<SideEffectAttemptId>,
    pub decision_json: Option<String>,
    pub decision_json_hash: Option<String>,
}

// ── Prepare intent ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PrepareEffectIntent {
    pub run_id: RunId,
    pub stage_execution_id: StageExecutionId,
    pub agent_execution_id: Option<AgentExecutionId>,
    pub effect_kind: EffectKind,
    pub target_key: String,
    pub idempotency_key: String,
    pub idempotency_key_version: i64,
    pub request_fingerprint: String,
    pub request_fingerprint_version: i64,
    pub expected_evidence_json: Option<String>,
    pub evidence_root: Option<String>,
    pub deadline_at: Option<DateTime<Utc>>,
}

/// Compute the idempotency key for a side effect.
/// sha256('p078:v1:' || effect_kind || ':' || run_id || ':' || stage_execution_id
///        || ':' || agent_execution_id_or_empty || ':' || target_key || ':' || intent_version)
pub fn derive_idempotency_key(
    effect_kind: &EffectKind,
    run_id: &RunId,
    stage_execution_id: &StageExecutionId,
    agent_execution_id: Option<&AgentExecutionId>,
    target_key: &str,
    intent_version: u32,
) -> String {
    use sha2::{Digest, Sha256};
    let agent_part = agent_execution_id
        .map(|a| a.to_string())
        .unwrap_or_default();
    let input = format!(
        "p078:v1:{}:{}:{}:{}:{}:{}",
        effect_kind,
        run_id,
        stage_execution_id,
        agent_part,
        target_key,
        intent_version,
    );
    let hash_bytes = Sha256::digest(input.as_bytes());
    let hex: String = hash_bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("p078:v{intent_version}:{hex}")
}

// ── Retry block envelope ─────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequiresEffectReconciliationEnvelope {
    pub schema_version: String,
    pub error: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: Option<String>,
    pub effect_ids: Vec<String>,
    pub reason: ReconciliationBlockReason,
    pub retry_forbidden: bool,
    pub recommended_mcp_tool: String,
    pub projection_updated_at: Option<String>,
}

impl RequiresEffectReconciliationEnvelope {
    pub fn new(
        run_id: &RunId,
        stage_execution_id: &StageExecutionId,
        agent_execution_id: Option<&AgentExecutionId>,
        effect_ids: Vec<String>,
        reason: ReconciliationBlockReason,
    ) -> Self {
        Self {
            schema_version: "requires_effect_reconciliation_v1".into(),
            error: "requires_effect_reconciliation".into(),
            run_id: run_id.to_string(),
            stage_execution_id: stage_execution_id.to_string(),
            agent_execution_id: agent_execution_id.map(|a| a.to_string()),
            effect_ids,
            reason,
            retry_forbidden: true,
            recommended_mcp_tool: "effects.inspect or effects.reconcile".into(),
            projection_updated_at: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationBlockReason {
    UnresolvedPrepared,
    UnresolvedExecuting,
    ExternallyObservedPending,
    NeedsReconciliation,
    Conflict,
    Unrecoverable,
    LedgerReadbackError,
}

impl std::fmt::Display for ReconciliationBlockReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedPrepared => write!(f, "unresolved_prepared"),
            Self::UnresolvedExecuting => write!(f, "unresolved_executing"),
            Self::ExternallyObservedPending => write!(f, "externally_observed_pending"),
            Self::NeedsReconciliation => write!(f, "needs_reconciliation"),
            Self::Conflict => write!(f, "conflict"),
            Self::Unrecoverable => write!(f, "unrecoverable"),
            Self::LedgerReadbackError => write!(f, "ledger_readback_error"),
        }
    }
}

// ── Feature flag keys ─────────────────────────────────────────────────────────

pub const FEATURE_RELEASE_SIDE_EFFECTS_ENABLED: &str = "CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED";
pub const FEATURE_P078_HEURISTIC_RETRY_GUARD_ENABLED: &str =
    "CHAINWORKS_P078_HEURISTIC_RETRY_GUARD_ENABLED";

// ── MCP error codes ───────────────────────────────────────────────────────────

pub const MCP_ERROR_EFFECT_NOT_FOUND: &str = "effect_not_found";
pub const MCP_ERROR_UNAUTHORIZED: &str = "unauthorized";
pub const MCP_ERROR_INVALID_STATUS_TRANSITION: &str = "invalid_status_transition";
pub const MCP_ERROR_INVALID_DISPOSITION: &str = "invalid_disposition";
pub const MCP_ERROR_DISPOSITION_PAYLOAD_MISMATCH: &str = "disposition_payload_mismatch";
pub const MCP_ERROR_REQUIRES_RECONCILIATION: &str = "requires_effect_reconciliation";
pub const MCP_ERROR_INSUFFICIENT_EVIDENCE: &str = "insufficient_evidence";
pub const MCP_ERROR_CONFLICT: &str = "conflict";
pub const MCP_ERROR_READBACK_UNAVAILABLE: &str = "readback_unavailable";
pub const MCP_ERROR_DEADLINE_EXCEEDED: &str = "deadline_exceeded";
pub const MCP_ERROR_ATTEMPT_BUDGET_EXHAUSTED: &str = "attempt_budget_exhausted";
pub const MCP_ERROR_LEDGER_READBACK_ERROR: &str = "ledger_readback_error";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_078_effect_kind_roundtrip() {
        for (s, k) in [
            ("git_commit", EffectKind::GitCommit),
            ("git_push", EffectKind::GitPush),
            ("build_archive", EffectKind::BuildArchive),
            ("connect_upload", EffectKind::ConnectUpload),
            ("tag_create", EffectKind::TagCreate),
            ("artifact_publish", EffectKind::ArtifactPublish),
        ] {
            assert_eq!(s.parse::<EffectKind>().unwrap(), k);
            assert_eq!(k.to_string(), s);
        }
        assert!("unknown_kind".parse::<EffectKind>().is_err());
    }

    #[test]
    fn proposal_078_status_roundtrip() {
        for (s, st) in [
            ("prepared", SideEffectStatus::Prepared),
            ("executing", SideEffectStatus::Executing),
            ("externally_observed", SideEffectStatus::ExternallyObserved),
            ("needs_reconciliation", SideEffectStatus::NeedsReconciliation),
            ("settled", SideEffectStatus::Settled),
            ("reconciled", SideEffectStatus::Reconciled),
            ("conflict", SideEffectStatus::Conflict),
            ("unrecoverable", SideEffectStatus::Unrecoverable),
        ] {
            assert_eq!(s.parse::<SideEffectStatus>().unwrap(), st);
            assert_eq!(st.to_string(), s);
        }
        assert!("unknown_status".parse::<SideEffectStatus>().is_err());
    }

    #[test]
    fn proposal_078_terminal_statuses() {
        assert!(!SideEffectStatus::Prepared.is_terminal());
        assert!(!SideEffectStatus::Executing.is_terminal());
        assert!(!SideEffectStatus::ExternallyObserved.is_terminal());
        assert!(!SideEffectStatus::NeedsReconciliation.is_terminal());
        assert!(SideEffectStatus::Settled.is_terminal());
        assert!(SideEffectStatus::Reconciled.is_terminal());
        assert!(SideEffectStatus::Conflict.is_terminal());
        assert!(SideEffectStatus::Unrecoverable.is_terminal());
    }

    #[test]
    fn proposal_078_unresolved_statuses() {
        assert!(SideEffectStatus::Prepared.is_unresolved());
        assert!(SideEffectStatus::Executing.is_unresolved());
        assert!(SideEffectStatus::ExternallyObserved.is_unresolved());
        assert!(SideEffectStatus::NeedsReconciliation.is_unresolved());
        assert!(!SideEffectStatus::Settled.is_unresolved());
        assert!(!SideEffectStatus::Reconciled.is_unresolved());
        // Conflict and Unrecoverable are both unresolved (operator action needed)
        assert!(SideEffectStatus::Conflict.is_unresolved());
        assert!(SideEffectStatus::Unrecoverable.is_unresolved());
    }

    #[test]
    fn proposal_078_legal_transitions() {
        assert!(SideEffectStatus::Prepared.can_transition_to(&SideEffectStatus::Executing));
        assert!(SideEffectStatus::Prepared.can_transition_to(&SideEffectStatus::NeedsReconciliation));
        assert!(!SideEffectStatus::Prepared.can_transition_to(&SideEffectStatus::Settled));
        assert!(SideEffectStatus::Executing.can_transition_to(&SideEffectStatus::Settled));
        assert!(SideEffectStatus::Executing.can_transition_to(&SideEffectStatus::NeedsReconciliation));
        assert!(!SideEffectStatus::Settled.can_transition_to(&SideEffectStatus::Executing));
    }

    #[test]
    fn proposal_078_wired_kinds() {
        assert!(EffectKind::GitCommit.is_wired());
        assert!(EffectKind::GitPush.is_wired());
        assert!(EffectKind::BuildArchive.is_wired());
        assert!(EffectKind::ConnectUpload.is_wired());
        assert!(!EffectKind::TagCreate.is_wired());
        assert!(!EffectKind::ArtifactPublish.is_wired());
    }

    #[test]
    fn proposal_078_idempotency_key_is_stable() {
        use crate::ids::{RunId, StageExecutionId};
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let k1 = derive_idempotency_key(
            &EffectKind::GitPush,
            &run_id,
            &stage_id,
            None,
            "refs/heads/main",
            1,
        );
        let k2 = derive_idempotency_key(
            &EffectKind::GitPush,
            &run_id,
            &stage_id,
            None,
            "refs/heads/main",
            1,
        );
        assert_eq!(k1, k2, "idempotency key must be stable for the same inputs");
    }

    #[test]
    fn proposal_078_idempotency_key_uses_sha256_format() {
        // Regression guard: SHA-256 produces 64 hex chars; DefaultHasher only 16.
        // Format must be: "p078:v{version}:{64-char-sha256-hex}"
        use crate::ids::{RunId, StageExecutionId};
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let k = derive_idempotency_key(
            &EffectKind::GitCommit,
            &run_id,
            &stage_id,
            None,
            "refs/heads/main",
            1,
        );
        let mut parts = k.splitn(3, ':');
        assert_eq!(parts.next(), Some("p078"));
        assert_eq!(parts.next(), Some("v1"));
        let hex_part = parts.next().expect("must have hex part");
        assert_eq!(
            hex_part.len(),
            64,
            "SHA-256 hex must be 64 chars, got {}; key={k}",
            hex_part.len()
        );
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit()),
            "hex part must be lowercase hex: {hex_part}"
        );
    }

    #[test]
    fn proposal_078_idempotency_key_differs_by_kind() {
        use crate::ids::{RunId, StageExecutionId};
        let run_id = RunId::new();
        let stage_id = StageExecutionId::new();
        let k_commit = derive_idempotency_key(
            &EffectKind::GitCommit,
            &run_id,
            &stage_id,
            None,
            "target",
            1,
        );
        let k_push = derive_idempotency_key(
            &EffectKind::GitPush,
            &run_id,
            &stage_id,
            None,
            "target",
            1,
        );
        assert_ne!(k_commit, k_push);
    }

    #[test]
    fn proposal_078_reconciliation_envelope_schema_version() {
        use crate::ids::{RunId, StageExecutionId};
        let env = RequiresEffectReconciliationEnvelope::new(
            &RunId::new(),
            &StageExecutionId::new(),
            None,
            vec!["eff-1".into()],
            ReconciliationBlockReason::UnresolvedExecuting,
        );
        assert_eq!(env.schema_version, "requires_effect_reconciliation_v1");
        assert!(env.retry_forbidden);
    }
}
