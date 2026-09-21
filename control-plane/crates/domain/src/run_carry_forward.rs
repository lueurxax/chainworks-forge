//! P039 immutable input evidence. No historical artifact grants execution authority.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ids::{ApprovalId, RunId, StageExecutionId};

#[derive(Debug, Error)]
pub enum CarryForwardError {
    #[error("invalid_sha256")]
    InvalidDigest,
    #[error("invalid_evidence: {0}")]
    InvalidEvidence(&'static str),
    #[error("stale_approval_binding")]
    StaleApproval,
    #[error("approval_not_actionable")]
    ApprovalNotActionable,
    #[error("invalid_canonical_json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ContentDigest(String);

impl ContentDigest {
    pub fn of(bytes: &[u8]) -> Self {
        Self(format!("sha256:{:x}", Sha256::digest(bytes)))
    }

    pub fn from_sha256_hex(hex: &str) -> Result<Self, CarryForwardError> {
        Self::try_from(format!("sha256:{hex}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContentDigest {
    type Error = CarryForwardError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let hex = value
            .strip_prefix("sha256:")
            .ok_or(CarryForwardError::InvalidDigest)?;
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(CarryForwardError::InvalidDigest);
        }
        Ok(Self(value))
    }
}

impl From<ContentDigest> for String {
    fn from(value: ContentDigest) -> Self {
        value.0
    }
}

pub fn canonical_digest(value: &impl Serialize) -> Result<ContentDigest, CarryForwardError> {
    Ok(ContentDigest::of(&serde_json_canonicalizer::to_vec(value)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryRole {
    ExecutionSeed,
    ReferenceOnly,
    PreserveOnly,
    Excluded,
}

impl EntryRole {
    pub fn can_seed_input(self) -> bool {
        self == Self::ExecutionSeed
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalEvidenceV1 {
    pub source_run_id: RunId,
    pub successor_run_id: RunId,
    pub manifest_sha256: ContentDigest,
    pub proposal_sha256: ContentDigest,
    pub code_sha256: ContentDigest,
    pub workflow_snapshot_hash: ContentDigest,
    pub catalog_snapshot_hash: ContentDigest,
    pub capability_delta_sha256: ContentDigest,
    pub unresolved_findings_sha256: ContentDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BindingState {
    Pending,
    Granted,
    Consumed,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalBindingV1 {
    approval_id: ApprovalId,
    stage_execution_id: StageExecutionId,
    evidence: ApprovalEvidenceV1,
    state: BindingState,
}

impl ApprovalBindingV1 {
    pub fn pending(
        approval_id: ApprovalId,
        stage_execution_id: StageExecutionId,
        evidence: ApprovalEvidenceV1,
    ) -> Result<Self, CarryForwardError> {
        if evidence.source_run_id == evidence.successor_run_id {
            return Err(CarryForwardError::InvalidEvidence(
                "source_equals_successor",
            ));
        }
        Ok(Self {
            approval_id,
            stage_execution_id,
            evidence,
            state: BindingState::Pending,
        })
    }

    pub fn grant(
        &mut self,
        evidence: &ApprovalEvidenceV1,
        stage: StageExecutionId,
    ) -> Result<(), CarryForwardError> {
        if self.evidence != *evidence || self.stage_execution_id != stage {
            return Err(CarryForwardError::StaleApproval);
        }
        if self.state != BindingState::Pending {
            return Err(CarryForwardError::ApprovalNotActionable);
        }
        self.state = BindingState::Granted;
        Ok(())
    }

    pub fn authorizes(&self, evidence: &ApprovalEvidenceV1, stage: StageExecutionId) -> bool {
        self.state == BindingState::Granted
            && self.stage_execution_id == stage
            && self.evidence == *evidence
    }

    pub fn consume(
        &mut self,
        evidence: &ApprovalEvidenceV1,
        stage: StageExecutionId,
    ) -> Result<(), CarryForwardError> {
        if !self.authorizes(evidence, stage) {
            return Err(CarryForwardError::ApprovalNotActionable);
        }
        self.state = BindingState::Consumed;
        Ok(())
    }

    pub fn supersede(&mut self) {
        self.state = BindingState::Superseded;
    }
}
