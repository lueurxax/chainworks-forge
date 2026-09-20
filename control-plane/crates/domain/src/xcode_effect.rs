//! Standalone journal data, not permission grants or an admitted tool facade.
//! Project identities and authority digests must already have been resolved by
//! the engine/domain adapter. Validation here cannot attest filesystem identity,
//! tool completion, authorization or JCS digest provenance.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptState {
    Prepared,
    Dispatched,
    Succeeded,
    Failed,
    Unknown,
    Reconciled,
}

impl AttemptState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Dispatched => "dispatched",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
            Self::Reconciled => "reconciled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectOrigin {
    Lifecycle,
    Mcp,
    Shim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    Mutate,
    Execute,
}

/// A frozen identity supplied by the future coordinator, not a verified binding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectKey {
    pub version: u32,
    pub uid: u32,
    pub canonical_path: String,
    pub device: String,
    pub inode: String,
}

impl ProjectKey {
    pub fn validate(&self) -> Result<(), InputError> {
        let path = &self.canonical_path;
        let decimal = |s: &str| {
            !s.is_empty()
                && s.len() <= 20
                && s.bytes().all(|c| c.is_ascii_digit())
                && (s == "0" || !s.starts_with('0'))
                && s.parse::<u64>().is_ok()
        };
        if self.version != 1
            || path.len() > 2048
            || !path.starts_with('/')
            || path.ends_with('/')
            || path.chars().any(char::is_control)
            || path[1..]
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || !decimal(&self.device)
            || !decimal(&self.inode)
        {
            return Err(InputError::new(
                "project_key",
                InputReasonCode::InvalidProjectKey,
            ));
        }
        Ok(())
    }
}

/// Immutable local contract identity is retained even if future admission changes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaIdentity {
    pub contract_id: String,
    pub version: u32,
    pub manifest_digest: String,
    pub schema_digest: String,
}

impl SchemaIdentity {
    pub fn validate(&self) -> Result<(), InputError> {
        validate_text("contract_id", &self.contract_id, 128)?;
        if self.version == 0 {
            return Err(InputError::new(
                "schema_version",
                InputReasonCode::InvalidSchema,
            ));
        }
        validate_digest("manifest_digest", &self.manifest_digest)?;
        validate_digest("schema_digest", &self.schema_digest)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizedIntent {
    pub run_id: Uuid,
    pub owner_lineage: String,
    pub invocation_id: Uuid,
    pub project_key: ProjectKey,
    pub operation_key: Uuid,
    pub operation: String,
    pub target_digest: String,
    pub binding_digest: Option<String>,
    pub request_digest: String,
    pub origin: EffectOrigin,
    pub effect_class: EffectClass,
    pub result_schema: SchemaIdentity,
}

impl NormalizedIntent {
    pub fn validate(&self) -> Result<(), InputError> {
        validate_uuid("run_id", self.run_id)?;
        validate_uuid("invocation_id", self.invocation_id)?;
        validate_uuid("operation_key", self.operation_key)?;
        validate_text("owner_lineage", &self.owner_lineage, 256)?;
        validate_text("operation", &self.operation, 128)?;
        self.project_key.validate()?;
        validate_digest("target_digest", &self.target_digest)?;
        validate_digest("request_digest", &self.request_digest)?;
        match &self.binding_digest {
            Some(digest) => validate_digest("binding_digest", digest)?,
            None if self.origin == EffectOrigin::Lifecycle
                && self.effect_class == EffectClass::Mutate
                && self.operation == "workspace_open" => {}
            None => {
                return Err(InputError::new(
                    "binding_digest",
                    InputReasonCode::BindingRequired,
                ))
            }
        }
        self.result_schema.validate()
    }

    /// Invocation is provenance, not retry identity. Keep the original invocation.
    pub fn same_operation(&self, other: &Self) -> bool {
        let mut retry = other.clone();
        retry.invocation_id = self.invocation_id;
        self == &retry
    }
}

/// Only bounded summaries enter this journal; no arbitrary JSON or provider bytes.
/// Adapters must redact domain-specific secrets too; generic sanitization is not
/// proof that arbitrary upstream text is safe to persist.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RedactedText(String);

impl RedactedText {
    pub fn new(value: &str) -> Result<Self, InputError> {
        validate_text("redacted_text", value, 512)?;
        let redacted = crate::error_sanitizer::strip_credentials_from_error(value);
        validate_text("redacted_text", &redacted, 512)?;
        Ok(Self(redacted))
    }
}

impl TryFrom<String> for RedactedText {
    type Error = InputError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<RedactedText> for String {
    fn from(value: RedactedText) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectErrorCode {
    CancelledBeforeDispatch,
    TerminalFailure,
    OutcomeUnknown,
    ReconciledNotApplied,
    ReconciledPartial,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum HistoricalPayload {
    Result {
        summary: RedactedText,
    },
    Error {
        code: EffectErrorCode,
        message: RedactedText,
    },
}

/// A journal readback envelope, not the future manifest-generated tool result.
/// The digest is supplied by the domain adapter; this module does not invent a
/// substitute canonical JSON algorithm or derive authority from serialization.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalResult {
    pub schema_version: u32,
    pub schema: SchemaIdentity,
    pub payload: HistoricalPayload,
    pub result_digest: String,
}

impl HistoricalResult {
    pub fn validate(&self, schema: &SchemaIdentity) -> Result<(), InputError> {
        if self.schema_version != 1 || &self.schema != schema {
            return Err(InputError::new(
                "outcome_schema",
                InputReasonCode::InvalidSchema,
            ));
        }
        self.schema.validate()?;
        validate_digest("result_digest", &self.result_digest)
    }

    pub fn require_error(&self, expected: EffectErrorCode) -> Result<(), InputError> {
        if matches!(&self.payload, HistoricalPayload::Error { code, .. } if *code == expected) {
            Ok(())
        } else {
            Err(InputError::new("outcome", InputReasonCode::InvalidOutcome))
        }
    }

    pub fn require_result(&self) -> Result<(), InputError> {
        if matches!(&self.payload, HistoricalPayload::Result { .. }) {
            Ok(())
        } else {
            Err(InputError::new("outcome", InputReasonCode::InvalidOutcome))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyReason {
    Restart,
    TransportLost,
    PartialEffect,
    AsyncInFlight,
    CancelledAfterDispatch,
}

/// Evidence assertions from a trusted adapter. DB validation is not verification
/// of the referenced evidence and is never an authorization endpoint.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettlementProof {
    pub evidence_ref: String,
    pub active_operation_check_ref: String,
    pub no_operation_in_flight: bool,
}

impl SettlementProof {
    pub fn validate(&self) -> Result<(), InputError> {
        validate_evidence_ref("evidence_ref", &self.evidence_ref)?;
        validate_evidence_ref(
            "active_operation_check_ref",
            &self.active_operation_check_ref,
        )?;
        if !self.no_operation_in_flight {
            return Err(InputError::new(
                "no_operation_in_flight",
                InputReasonCode::UnsettledOperation,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Completion {
    Succeeded {
        outcome: HistoricalResult,
        proof: SettlementProof,
    },
    Failed {
        outcome: HistoricalResult,
        proof: SettlementProof,
    },
    Unknown {
        reason: UncertaintyReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconciliationDisposition {
    Applied,
    NotApplied,
    Partial,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reconciliation {
    pub operator_id: String,
    pub inspected_state: RedactedText,
    pub proof: SettlementProof,
    pub disposition: ReconciliationDisposition,
    pub outcome: HistoricalResult,
}

impl Reconciliation {
    pub fn validate(&self, schema: &SchemaIdentity) -> Result<(), InputError> {
        validate_text("operator_id", &self.operator_id, 256)?;
        self.proof.validate()?;
        self.outcome.validate(schema)?;
        match self.disposition {
            ReconciliationDisposition::Applied => self.outcome.require_result(),
            ReconciliationDisposition::NotApplied => self
                .outcome
                .require_error(EffectErrorCode::ReconciledNotApplied),
            ReconciliationDisposition::Partial => self
                .outcome
                .require_error(EffectErrorCode::ReconciledPartial),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredAttempt {
    pub sequence: i64,
    pub attempt_id: Uuid,
    pub nonce: Uuid,
    pub intent: NormalizedIntent,
    pub state: AttemptState,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub outcome: Option<HistoricalResult>,
    pub result_digest: Option<String>,
    pub uncertainty_reason: Option<UncertaintyReason>,
    pub settlement_proof: Option<SettlementProof>,
    pub reconciliation: Option<Reconciliation>,
    pub reconciliation_evidence_ref: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptRevision {
    pub attempt_id: Uuid,
    pub revision: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputReasonCode {
    InvalidText,
    InvalidUuid,
    InvalidDigest,
    InvalidProjectKey,
    BindingRequired,
    InvalidSchema,
    InvalidTimestamp,
    InvalidRevision,
    InvalidOutcome,
    InvalidEvidenceReference,
    UnsettledOperation,
    InvalidLimit,
    InvalidCursor,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid {field}: {code:?}")]
pub struct InputError {
    pub field: &'static str,
    pub code: InputReasonCode,
}

impl InputError {
    pub fn new(field: &'static str, code: InputReasonCode) -> Self {
        Self { field, code }
    }
}

pub fn validate_text(field: &'static str, value: &str, max_bytes: usize) -> Result<(), InputError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        Err(InputError::new(field, InputReasonCode::InvalidText))
    } else {
        Ok(())
    }
}

pub fn validate_digest(field: &'static str, value: &str) -> Result<(), InputError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        Err(InputError::new(field, InputReasonCode::InvalidDigest))
    } else {
        Ok(())
    }
}

pub fn validate_uuid(field: &'static str, value: Uuid) -> Result<(), InputError> {
    if value.is_nil() || value.is_max() {
        Err(InputError::new(field, InputReasonCode::InvalidUuid))
    } else {
        Ok(())
    }
}

pub fn validate_revision(revision: i64) -> Result<(), InputError> {
    if !(0..i64::MAX).contains(&revision) {
        Err(InputError::new(
            "revision",
            InputReasonCode::InvalidRevision,
        ))
    } else {
        Ok(())
    }
}

pub fn validate_time(
    now: DateTime<Utc>,
    previous: Option<DateTime<Utc>>,
) -> Result<(), InputError> {
    if now.timestamp() < 0 || previous.is_some_and(|previous| now < previous) {
        Err(InputError::new(
            "timestamp",
            InputReasonCode::InvalidTimestamp,
        ))
    } else {
        Ok(())
    }
}

pub fn validate_evidence_ref(field: &'static str, value: &str) -> Result<(), InputError> {
    // Evidence is an opaque local identifier, never a URL, raw payload or credential.
    if value.is_empty()
        || value.len() > 256
        || value.starts_with('/')
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-/.".contains(&c))
        || value
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
    {
        Err(InputError::new(
            field,
            InputReasonCode::InvalidEvidenceReference,
        ))
    } else {
        Ok(())
    }
}
