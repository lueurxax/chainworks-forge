//! Fixed-scope engine implementation of the ACP journal boundary.

use std::sync::Arc;

use acp::xcode_effect_journal::{
    XcodeDispatchDecision, XcodeEffectJournal, XcodeJournalError, XcodeJournalResult,
};
use chrono::Utc;
use db::repos::xcode_effect_attempts::{self as journal, DispatchDecision, JournalError};
use db::write_class::WriteLane;
use db::writer::{class_a_operation, DbWriter, QueuedTransaction};
use domain::xcode_effect::{
    AttemptRevision, Completion, HistoricalResult, NormalizedIntent, ProjectKey, StoredAttempt,
};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

pub struct DbXcodeProjectHoldCheck(pub SqlitePool);

#[async_trait::async_trait]
impl acp::xcode_coordinator::ProjectHoldCheck for DbXcodeProjectHoldCheck {
    async fn is_held(
        &self,
        project: &ProjectKey,
    ) -> Result<bool, acp::xcode_coordinator::CoordinatorError> {
        journal::project_is_held(&self.0, project)
            .await
            .map_err(|_| acp::xcode_coordinator::CoordinatorError::HoldCheckUnavailable)
    }
}

pub fn frozen_xcode_permission_policy(
    catalog: Option<&str>,
    profile: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let catalog: workflow::catalog::AgentCatalogFile = serde_json::from_str(
        catalog.ok_or_else(|| anyhow::anyhow!("headless_frozen_catalog_required"))?,
    )?;
    let profiles = serde_json::to_value(
        catalog
            .permission_profiles
            .ok_or_else(|| anyhow::anyhow!("headless_frozen_permissions_required"))?,
    )?;
    profiles
        .get(profile.ok_or_else(|| anyhow::anyhow!("headless_permission_profile_required"))?)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("headless_permission_profile_missing"))
}

pub struct DbXcodeEffectJournal {
    pool: SqlitePool,
    writer: Arc<DbWriter>,
    run_id: Uuid,
    owner_lineage: String,
}

impl DbXcodeEffectJournal {
    /// The engine supplies the canonical pool and its existing shared writer.
    /// This persistence adapter does not establish live runtime authority.
    pub fn new(
        pool: SqlitePool,
        writer: Arc<DbWriter>,
        run_id: Uuid,
        owner_lineage: String,
    ) -> Self {
        Self {
            pool,
            writer,
            run_id,
            owner_lineage,
        }
    }

    fn contains(&self, intent: &NormalizedIntent) -> bool {
        intent.run_id == self.run_id && intent.owner_lineage == self.owner_lineage
    }

    async fn require_scope(&self, attempt_id: Uuid) -> XcodeJournalResult<()> {
        // The repository makes intent identity immutable; only state/revision
        // may change between this read and the writer-admitted CAS below.
        self.get(attempt_id)
            .await?
            .ok_or(XcodeJournalError::NotFound)?;
        Ok(())
    }

    fn revision_key(&self, attempt: AttemptRevision) -> String {
        format!(
            "{}/{}/{}/{}",
            self.run_id, self.owner_lineage, attempt.attempt_id, attempt.revision
        )
    }

    async fn begin(
        &self,
        operation: &'static str,
        key: String,
    ) -> XcodeJournalResult<QueuedTransaction> {
        self.writer
            .begin_immediate_transaction(
                class_a_operation(operation, WriteLane::CriticalBarrier, key),
                operation,
            )
            .await
            .map_err(|_| XcodeJournalError::StorageUnavailable)
    }
}

#[async_trait::async_trait]
impl XcodeEffectJournal for DbXcodeEffectJournal {
    async fn prepare(&self, intent: &NormalizedIntent) -> XcodeJournalResult<StoredAttempt> {
        if !self.contains(intent) {
            return Err(XcodeJournalError::ScopeMismatch);
        }
        intent.validate().map_err(XcodeJournalError::InvalidInput)?;
        // This hash bounds the writer's bookkeeping key, not an authority digest.
        let project = serde_json::to_vec(&intent.project_key)
            .map_err(|_| XcodeJournalError::CorruptRecord)?;
        let project_hash = format!("{:x}", Sha256::digest(project));
        let key = format!(
            "{}/{}/{}/{}",
            self.run_id, self.owner_lineage, project_hash, intent.operation_key
        );
        let tx = self.begin("xcode_effect.prepare", key).await?;
        journal::prepare(tx, intent, Utc::now())
            .await
            .map_err(map_error)
    }

    async fn dispatch(
        &self,
        nonce: Uuid,
        request_digest: &str,
        expected_revision: i64,
    ) -> XcodeJournalResult<XcodeDispatchDecision> {
        self.require_scope(nonce).await?;
        let key = self.revision_key(AttemptRevision {
            attempt_id: nonce,
            revision: expected_revision,
        });
        let tx = self.begin("xcode_effect.dispatch", key).await?;
        match journal::dispatch(
            tx,
            &self.owner_lineage,
            nonce,
            request_digest,
            expected_revision,
            Utc::now(),
        )
        .await
        .map_err(map_error)?
        {
            DispatchDecision::Dispatch(attempt) => Ok(XcodeDispatchDecision::Dispatch(attempt)),
            DispatchDecision::Existing(attempt) => Ok(XcodeDispatchDecision::Existing(attempt)),
        }
    }

    async fn complete(
        &self,
        attempt: AttemptRevision,
        completion: &Completion,
    ) -> XcodeJournalResult<StoredAttempt> {
        self.require_scope(attempt.attempt_id).await?;
        let tx = self
            .begin("xcode_effect.complete", self.revision_key(attempt))
            .await?;
        journal::complete(
            tx,
            &self.owner_lineage,
            attempt.attempt_id,
            attempt.revision,
            completion,
            Utc::now(),
        )
        .await
        .map_err(map_error)
    }

    async fn cancel(
        &self,
        attempt: AttemptRevision,
        outcome: &HistoricalResult,
    ) -> XcodeJournalResult<StoredAttempt> {
        self.require_scope(attempt.attempt_id).await?;
        let tx = self
            .begin("xcode_effect.cancel", self.revision_key(attempt))
            .await?;
        journal::cancel(
            tx,
            &self.owner_lineage,
            attempt.attempt_id,
            attempt.revision,
            outcome,
            Utc::now(),
        )
        .await
        .map_err(map_error)
    }

    async fn get(&self, attempt_id: Uuid) -> XcodeJournalResult<Option<StoredAttempt>> {
        let attempt = journal::get(&self.pool, &self.owner_lineage, attempt_id)
            .await
            .map_err(map_error)?;
        Ok(attempt.filter(|attempt| self.contains(&attempt.intent)))
    }
}

fn map_error(error: JournalError) -> XcodeJournalError {
    match error {
        JournalError::InvalidInput(error) => XcodeJournalError::InvalidInput(error),
        JournalError::IntentConflict => XcodeJournalError::IntentConflict,
        JournalError::ProjectHeld => XcodeJournalError::ProjectHeld,
        JournalError::RevisionConflict => XcodeJournalError::RevisionConflict,
        JournalError::InvalidTransition => XcodeJournalError::InvalidTransition,
        JournalError::NotFound => XcodeJournalError::NotFound,
        JournalError::CorruptRecord => XcodeJournalError::CorruptRecord,
        JournalError::Storage(_) => XcodeJournalError::StorageUnavailable,
    }
}
