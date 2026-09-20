//! Persistence boundary only: journal decisions do not grant effect authority.

use domain::xcode_effect::{
    AttemptRevision, Completion, HistoricalResult, InputError, NormalizedIntent, StoredAttempt,
};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XcodeDispatchDecision {
    /// The durable fence committed. Existing runtime admission is still required.
    Dispatch(StoredAttempt),
    /// Historical or in-flight state; never authorizes another send.
    Existing(StoredAttempt),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XcodeJournalError {
    InvalidInput(InputError),
    ScopeMismatch,
    IntentConflict,
    ProjectHeld,
    RevisionConflict,
    InvalidTransition,
    NotFound,
    CorruptRecord,
    /// May include an ambiguous commit; never implies the effect was not sent.
    StorageUnavailable,
}

impl std::fmt::Display for XcodeJournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::InvalidInput(error) => return write!(f, "{error}"),
            Self::ScopeMismatch => "xcode_effect_scope_mismatch",
            Self::IntentConflict => "xcode_effect_intent_conflict",
            Self::ProjectHeld => "xcode_effect_project_held",
            Self::RevisionConflict => "xcode_effect_revision_conflict",
            Self::InvalidTransition => "xcode_effect_invalid_transition",
            Self::NotFound => "xcode_effect_not_found",
            Self::CorruptRecord => "xcode_effect_corrupt_record",
            Self::StorageUnavailable => "xcode_effect_storage_unavailable",
        };
        f.write_str(code)
    }
}

impl std::error::Error for XcodeJournalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidInput(error) => Some(error),
            _ => None,
        }
    }
}

pub type XcodeJournalResult<T> = Result<T, XcodeJournalError>;

/// Engine-created, fixed run/owner scope. Absence of an implementation disables
/// effectful routes; there is deliberately no successful no-op implementation.
#[async_trait::async_trait]
pub trait XcodeEffectJournal: Send + Sync {
    async fn prepare(&self, intent: &NormalizedIntent) -> XcodeJournalResult<StoredAttempt>;
    async fn dispatch(
        &self,
        nonce: Uuid,
        request_digest: &str,
        expected_revision: i64,
    ) -> XcodeJournalResult<XcodeDispatchDecision>;
    async fn complete(
        &self,
        attempt: AttemptRevision,
        completion: &Completion,
    ) -> XcodeJournalResult<StoredAttempt>;
    async fn cancel(
        &self,
        attempt: AttemptRevision,
        outcome: &HistoricalResult,
    ) -> XcodeJournalResult<StoredAttempt>;
    async fn get(&self, attempt_id: Uuid) -> XcodeJournalResult<Option<StoredAttempt>>;
}
