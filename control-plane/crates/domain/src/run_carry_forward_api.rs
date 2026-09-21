//! P039 wire contracts, not execution or caller authority.
//!
//! Deserialize raw JSON into these DTOs to retain duplicate-field detection.
//! Deserializing a `serde_json::Value` cannot detect keys its parser discarded.
//! Root ownership, provenance, policy, access and cursor binding require fresh
//! service checks; syntactically valid paths or IDs never establish those facts.

use serde::{de, Deserialize, Deserializer, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

use crate::run_carry_forward::ContentDigest;

pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_SUMMARY_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid P039 wire value: {0}")]
pub struct WireError(pub &'static str);

/// Apply the transport byte ceiling before deserializing (not after a Value).
pub fn parse_request<T: de::DeserializeOwned>(bytes: &[u8]) -> Result<T, serde_json::Error> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(de::Error::custom("P039 request exceeds 1 MiB"));
    }
    serde_json::from_slice(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CanonicalUuid(Uuid);

impl TryFrom<String> for CanonicalUuid {
    type Error = WireError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let id = Uuid::parse_str(&value).map_err(|_| WireError("UUID"))?;
        if id.to_string() != value {
            return Err(WireError("canonical lowercase UUID required"));
        }
        Ok(Self(id))
    }
}

impl CanonicalUuid {
    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for CanonicalUuid {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<CanonicalUuid> for String {
    fn from(value: CanonicalUuid) -> Self {
        value.0.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CanonicalUuid", into = "CanonicalUuid")]
pub struct CallerRequestId(CanonicalUuid);

impl TryFrom<CanonicalUuid> for CallerRequestId {
    type Error = WireError;
    fn try_from(value: CanonicalUuid) -> Result<Self, Self::Error> {
        if value.0.get_version() != Some(uuid::Version::Random)
            || value.0.get_variant() != uuid::Variant::RFC4122
        {
            return Err(WireError("caller_request_id must be UUIDv4"));
        }
        Ok(Self(value))
    }
}

impl CallerRequestId {
    pub fn as_uuid(self) -> Uuid {
        self.0.as_uuid()
    }
}

impl From<CallerRequestId> for CanonicalUuid {
    fn from(value: CallerRequestId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BoundedText<const N: usize>(String);

impl<const N: usize> TryFrom<String> for BoundedText<N> {
    type Error = WireError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.trim().is_empty() || value.len() > N || value.contains('\0') {
            return Err(WireError("empty, oversized or NUL-containing string"));
        }
        Ok(Self(value))
    }
}

impl<const N: usize> BoundedText<N> {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<const N: usize> From<BoundedText<N>> for String {
    fn from(value: BoundedText<N>) -> Self {
        value.0
    }
}

pub type Reason = BoundedText<4096>;
pub type DefinitionPath = BoundedText<4096>;
pub type PageCursor = BoundedText<4096>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelativePath(String);

impl TryFrom<String> for RelativePath {
    type Error = WireError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let path = value.strip_suffix('/').unwrap_or(&value);
        if value.len() > 4096
            || value.chars().any(char::is_control)
            || value.contains(['\\', ':'])
            || path
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(WireError("normalized relative path required"));
        }
        Ok(Self(value))
    }
}

impl RelativePath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<RelativePath> for String {
    fn from(value: RelativePath) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u16", into = "u16")]
pub struct PageLimit(u16);

impl TryFrom<u16> for PageLimit {
    type Error = WireError;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        if !(1..=500).contains(&value) {
            return Err(WireError("limit must be 1..=500"));
        }
        Ok(Self(value))
    }
}

impl Default for PageLimit {
    fn default() -> Self {
        Self(100)
    }
}

impl PageLimit {
    pub fn get(self) -> u16 {
        self.0
    }
}

impl From<PageLimit> for u16 {
    fn from(value: PageLimit) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct OperationVersion(u64);

impl TryFrom<u64> for OperationVersion {
    type Error = WireError;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value == 0 || value > i64::MAX as u64 {
            return Err(WireError("positive i64 version required"));
        }
        Ok(Self(value))
    }
}

impl OperationVersion {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<OperationVersion> for u64 {
    fn from(value: OperationVersion) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct BoundedList<T, const N: usize>(Vec<T>);

impl<T, const N: usize> TryFrom<Vec<T>> for BoundedList<T, N> {
    type Error = WireError;
    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        if value.len() > N {
            return Err(WireError("too many entries"));
        }
        Ok(Self(value))
    }
}

impl<T, const N: usize> BoundedList<T, N> {
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}

impl<'de, T: Deserialize<'de>, const N: usize> Deserialize<'de> for BoundedList<T, N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor<T, const N: usize>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>, const N: usize> de::Visitor<'de> for Visitor<T, N> {
            type Value = BoundedList<T, N>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "at most {N} entries")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element()? {
                    if values.len() == N {
                        return Err(de::Error::custom("too many entries"));
                    }
                    values.push(value);
                }
                Ok(BoundedList(values))
            }
        }
        deserializer.deserialize_seq(Visitor::<T, N>(std::marker::PhantomData))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarryForwardProfile {
    #[serde(rename = "implementation_restart_v1")]
    ImplementationRestartV1,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputReference {
    Artifact { id: CanonicalUuid },
    CarriedInput { id: CanonicalUuid },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "bool", into = "bool")]
pub struct IncludeDirtyWork;

impl TryFrom<bool> for IncludeDirtyWork {
    type Error = WireError;
    fn try_from(value: bool) -> Result<Self, Self::Error> {
        if value {
            Ok(Self)
        } else {
            Err(WireError("dirty work cannot be discarded"))
        }
    }
}

impl From<IncludeDirtyWork> for bool {
    fn from(_: IncludeDirtyWork) -> Self {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedWorkspacePath {
    pub path: RelativePath,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarryForwardSelection {
    pub proposal_input: InputReference,
    #[serde(deserialize_with = "unique_references")]
    pub reference_inputs: BoundedList<InputReference, 128>,
    pub include_dirty_work: IncludeDirtyWork,
    #[serde(deserialize_with = "unique_exclusions")]
    pub excluded_workspace_paths: BoundedList<ExcludedWorkspacePath, 128>,
}

fn unique_references<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BoundedList<InputReference, 128>, D::Error> {
    let values = BoundedList::<InputReference, 128>::deserialize(d)?;
    if values.0.iter().collect::<BTreeSet<_>>().len() != values.0.len() {
        return Err(de::Error::custom("duplicate reference"));
    }
    Ok(values)
}

fn unique_exclusions<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BoundedList<ExcludedWorkspacePath, 128>, D::Error> {
    let values = BoundedList::<ExcludedWorkspacePath, 128>::deserialize(d)?;
    if values
        .0
        .iter()
        .map(|e| e.path.as_str().trim_end_matches('/'))
        .collect::<BTreeSet<_>>()
        .len()
        != values.0.len()
    {
        return Err(de::Error::custom("duplicate exclusion"));
    }
    Ok(values)
}

/// Strict JSON-string payload matching the existing StartRun delivery shape.
/// The service must bind its repository and generated destinations to the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeliveryConfigurationJson(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarryForwardDeliveryConfiguration {
    pub repo_identifier: BoundedText<4096>,
    pub repo_root: DefinitionPath,
    pub base_branch: BoundedText<4096>,
    pub worktree_base_path: DefinitionPath,
    pub target_branch: BoundedText<4096>,
    pub release_target_id: Option<BoundedText<4096>>,
    pub release_mode: Option<BoundedText<64>>,
}

impl TryFrom<String> for DeliveryConfigurationJson {
    type Error = WireError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > MAX_REQUEST_BYTES {
            return Err(WireError("delivery configuration too large"));
        }
        serde_json::from_str::<CarryForwardDeliveryConfiguration>(&value)
            .map_err(|_| WireError("invalid delivery configuration"))?;
        Ok(Self(value))
    }
}

impl DeliveryConfigurationJson {
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn decode(&self) -> CarryForwardDeliveryConfiguration {
        serde_json::from_str(&self.0).expect("validated immutable delivery configuration")
    }
}

impl From<DeliveryConfigurationJson> for String {
    fn from(value: DeliveryConfigurationJson) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewTarget {
    pub workflow_yaml_path: DefinitionPath,
    pub agent_catalog_yaml_path: DefinitionPath,
    pub expected_workflow_snapshot_hash: Option<ContentDigest>,
    pub expected_catalog_snapshot_hash: Option<ContentDigest>,
    pub delivery_configuration_json: DeliveryConfigurationJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarryForwardTarget {
    pub workflow_yaml_path: DefinitionPath,
    pub agent_catalog_yaml_path: DefinitionPath,
    pub expected_workflow_snapshot_hash: ContentDigest,
    pub expected_catalog_snapshot_hash: ContentDigest,
    pub delivery_configuration_json: DeliveryConfigurationJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationPreviewRequest {
    pub run_id: CanonicalUuid,
    pub target: PreviewTarget,
    pub selection: CarryForwardSelection,
    pub profile: CarryForwardProfile,
    pub cursor: Option<PageCursor>,
    #[serde(default)]
    pub limit: PageLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinueBlockedRequest {
    pub run_id: CanonicalUuid,
    pub target: CarryForwardTarget,
    pub selection: CarryForwardSelection,
    pub profile: CarryForwardProfile,
    pub cursor: Option<PageCursor>,
    #[serde(default)]
    pub limit: PageLimit,
    pub expected_plan_sha256: ContentDigest,
    pub caller_request_id: CallerRequestId,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationGetRequest {
    pub operation_id: CanonicalUuid,
    pub cursor: Option<PageCursor>,
    #[serde(default)]
    pub limit: PageLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationActivateRequest {
    pub operation_id: CanonicalUuid,
    pub expected_version: OperationVersion,
    pub expected_manifest_sha256: ContentDigest,
    pub caller_request_id: CallerRequestId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationReconcileRequest {
    pub operation_id: CanonicalUuid,
    pub expected_version: OperationVersion,
    pub caller_request_id: CallerRequestId,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationAbortRequest {
    pub operation_id: CanonicalUuid,
    pub expected_version: OperationVersion,
    pub caller_request_id: CallerRequestId,
    pub reason: Reason,
}

macro_rules! wire_literal {
    ($name:ident, $value:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            #[serde(rename = $value)]
            V1,
        }
    };
}

wire_literal!(CommandResultSchema, "run_continuation_command_result_v1");
wire_literal!(ReadbackSchema, "run_continuation_readback_v1");
wire_literal!(AdmissionSchema, "run_continuation_admission_v1");
wire_literal!(LinksSchema, "run_carry_forward_links_v1");
wire_literal!(PreviewPageSchema, "run_carry_forward_preview_page_v1");
wire_literal!(PreviewHoldSchema, "run_carry_forward_preview_hold_v1");
wire_literal!(PreviewStaleSchema, "run_carry_forward_preview_stale_v1");
wire_literal!(PreviewStaleCode, "preview_stale");
wire_literal!(RefreshPreviewAction, "refresh_preview");
wire_literal!(TransportErrorSchema, "p039_transport_error_v1");
wire_literal!(CommandOutcomeUnknownCode, "command_outcome_unknown");
wire_literal!(ReplaySameRequestAction, "replay_same_request");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationPhase {
    Preparing,
    Prepared,
    Aborting,
    NeedsReconciliation,
    Activated,
    Aborted,
}

/// Unknown phases are retained only for diagnostics, never for commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "BoundedText<256>", into = "BoundedText<256>")]
pub struct ReadbackPhase(BoundedText<256>);

impl ReadbackPhase {
    pub fn known(&self) -> Option<ContinuationPhase> {
        match self.0.as_str() {
            "preparing" => Some(ContinuationPhase::Preparing),
            "prepared" => Some(ContinuationPhase::Prepared),
            "aborting" => Some(ContinuationPhase::Aborting),
            "needs_reconciliation" => Some(ContinuationPhase::NeedsReconciliation),
            "activated" => Some(ContinuationPhase::Activated),
            "aborted" => Some(ContinuationPhase::Aborted),
            _ => None,
        }
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<BoundedText<256>> for ReadbackPhase {
    fn from(value: BoundedText<256>) -> Self {
        Self(value)
    }
}

impl From<ReadbackPhase> for BoundedText<256> {
    fn from(value: ReadbackPhase) -> Self {
        value.0
    }
}

impl From<ContinuationPhase> for ReadbackPhase {
    fn from(value: ContinuationPhase) -> Self {
        let text = match value {
            ContinuationPhase::Preparing => "preparing",
            ContinuationPhase::Prepared => "prepared",
            ContinuationPhase::Aborting => "aborting",
            ContinuationPhase::NeedsReconciliation => "needs_reconciliation",
            ContinuationPhase::Activated => "activated",
            ContinuationPhase::Aborted => "aborted",
        };
        Self(BoundedText(text.into()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationReasonCode {
    SourceNotBlocked,
    SourceBusy,
    SourceContinued,
    ContinuationInProgress,
    UnsupportedFrontier,
    TargetProfileInvalid,
    TargetPolicyIncompatible,
    SourceChanged,
    PreparedMaterialChanged,
    ArtifactProvenanceInvalid,
    UnsafePath,
    DestinationConflict,
    HeadlessEffectUnresolved,
    EffectOutcomeUnknown,
    HistoricalBindingUnproven,
    ContinuationBudgetExceeded,
    IdempotencyConflict,
    StaleOperationVersion,
    ContinuationCapacity,
    StorageUnavailable,
    RolloutHold,
    PreviewStale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationNextAction {
    None,
    Inspect,
    AwaitWorker,
    RefreshPreview,
    ReadOperation,
    ExplicitReconcile,
    ResolveHold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationCommandStatus {
    Accepted,
    Denied,
    Replayed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayOf {
    Accepted,
    Denied,
}

// A deserialize_with field is required even for Option<T>: missing is not null.
fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

fn required_field<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    T::deserialize(d)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationOperationSummary {
    pub operation_id: CanonicalUuid,
    pub phase: ContinuationPhase,
    pub version: OperationVersion,
    #[serde(deserialize_with = "required_nullable")]
    pub successor_run_id: Option<CanonicalUuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationDenial {
    pub code: ContinuationReasonCode,
    pub message: Reason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CommandResultWire")]
pub struct RunContinuationCommandResultV1 {
    pub schema_version: CommandResultSchema,
    pub status: ContinuationCommandStatus,
    pub caller_request_id: CallerRequestId,
    pub journal_id: Option<CanonicalUuid>,
    pub operation: Option<ContinuationOperationSummary>,
    pub denial: Option<ContinuationDenial>,
    pub next_action: ContinuationNextAction,
    pub replay_of: Option<ReplayOf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandResultWire {
    schema_version: CommandResultSchema,
    status: ContinuationCommandStatus,
    caller_request_id: CallerRequestId,
    #[serde(deserialize_with = "required_nullable")]
    journal_id: Option<CanonicalUuid>,
    #[serde(deserialize_with = "required_nullable")]
    operation: Option<ContinuationOperationSummary>,
    #[serde(deserialize_with = "required_nullable")]
    denial: Option<ContinuationDenial>,
    next_action: ContinuationNextAction,
    #[serde(deserialize_with = "required_nullable")]
    replay_of: Option<ReplayOf>,
}

impl TryFrom<CommandResultWire> for RunContinuationCommandResultV1 {
    type Error = WireError;
    fn try_from(v: CommandResultWire) -> Result<Self, Self::Error> {
        let result = Self {
            schema_version: v.schema_version,
            status: v.status,
            caller_request_id: v.caller_request_id,
            journal_id: v.journal_id,
            operation: v.operation,
            denial: v.denial,
            next_action: v.next_action,
            replay_of: v.replay_of,
        };
        result.validate()?;
        Ok(result)
    }
}

impl RunContinuationCommandResultV1 {
    pub fn validate(&self) -> Result<(), WireError> {
        let original = match (self.status, self.replay_of) {
            (ContinuationCommandStatus::Accepted, None) => ReplayOf::Accepted,
            (ContinuationCommandStatus::Denied, None) => ReplayOf::Denied,
            (ContinuationCommandStatus::Replayed, Some(original)) => original,
            _ => return Err(WireError("inconsistent replay status")),
        };
        match original {
            ReplayOf::Accepted
                if self.operation.is_none()
                    || self.journal_id.is_none()
                    || self.denial.is_some() =>
            {
                return Err(WireError(
                    "accepted requires operation and journal, without denial",
                ))
            }
            ReplayOf::Denied if self.denial.is_none() => {
                return Err(WireError("denied requires denial"))
            }
            _ => {}
        }
        if let Some(operation) = &self.operation {
            if (operation.phase == ContinuationPhase::Activated)
                != operation.successor_run_id.is_some()
            {
                return Err(WireError("successor is present only after activation"));
            }
        } else if self.journal_id.is_some() {
            return Err(WireError(
                "no operation before reservation implies no journal",
            ));
        }
        Ok(())
    }

    /// Formatting only: the service must authenticate before revealing this replay.
    pub fn replayed(&self) -> Result<Self, WireError> {
        self.validate()?;
        let mut replay = self.clone();
        replay.replay_of = Some(match self.status {
            ContinuationCommandStatus::Accepted => ReplayOf::Accepted,
            ContinuationCommandStatus::Denied => ReplayOf::Denied,
            ContinuationCommandStatus::Replayed => self.replay_of.expect("validated replay"),
        });
        replay.status = ContinuationCommandStatus::Replayed;
        Ok(replay)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCarryForwardPreviewStaleV1 {
    pub schema_version: PreviewStaleSchema,
    pub code: PreviewStaleCode,
    pub next_action: RefreshPreviewAction,
}

/// A known preview hold before a complete source witness can be established.
/// It carries no fabricated witness, digest, inventory or operation identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PreviewHoldWire")]
pub struct RunCarryForwardPreviewHoldV1 {
    pub schema_version: PreviewHoldSchema,
    pub source_run_id: CanonicalUuid,
    pub holds: BoundedList<ContinuationDenial, 128>,
    pub next_action: ContinuationNextAction,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviewHoldWire {
    schema_version: PreviewHoldSchema,
    source_run_id: CanonicalUuid,
    holds: BoundedList<ContinuationDenial, 128>,
    next_action: ContinuationNextAction,
}

impl TryFrom<PreviewHoldWire> for RunCarryForwardPreviewHoldV1 {
    type Error = WireError;
    fn try_from(value: PreviewHoldWire) -> Result<Self, Self::Error> {
        let result = Self {
            schema_version: value.schema_version,
            source_run_id: value.source_run_id,
            holds: value.holds,
            next_action: value.next_action,
        };
        result.validate()?;
        Ok(result)
    }
}

impl RunCarryForwardPreviewHoldV1 {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.holds.as_slice().is_empty() {
            return Err(WireError("preview hold requires at least one hold"));
        }
        if !matches!(
            self.next_action,
            ContinuationNextAction::Inspect
                | ContinuationNextAction::ResolveHold
                | ContinuationNextAction::RefreshPreview
        ) {
            return Err(WireError("invalid preview hold next action"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct P039TransportErrorV1 {
    pub schema_version: TransportErrorSchema,
    pub code: CommandOutcomeUnknownCode,
    pub next_action: ReplaySameRequestAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionDisposition {
    SourceReserved,
    SourceHistorical,
    SuccessorPendingReview,
    AbortedSourceBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Unverified,
    Pending,
    Verified,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionFreshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunContinuationAdmissionV1 {
    pub schema_version: AdmissionSchema,
    pub enabled: bool,
    #[serde(deserialize_with = "required_nullable")]
    pub reason: Option<Reason>,
    pub fences_enforced: bool,
    pub reconcile_available: bool,
}

// Generate the same strict compact/detail fields without serde flatten, which
// weakens unknown/duplicate-field handling at security-sensitive boundaries.
macro_rules! readback_type {
    ($name:ident $(, $extra:ident: $ty:ty)*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub schema_version: ReadbackSchema,
            pub operation_id: CanonicalUuid,
            pub source_run_id: CanonicalUuid,
            #[serde(deserialize_with = "required_nullable")]
            pub successor_run_id: Option<CanonicalUuid>,
            pub phase: ReadbackPhase,
            pub version: OperationVersion,
            pub plan_sha256: ContentDigest,
            #[serde(deserialize_with = "required_nullable")]
            pub manifest_sha256: Option<ContentDigest>,
            pub execution_disposition: ExecutionDisposition,
            pub verification_status: VerificationStatus,
            pub holds: BoundedList<ContinuationDenial, 128>,
            pub next_actions: BoundedList<ContinuationNextAction, 8>,
            #[serde(deserialize_with = "required_nullable")]
            pub journal_id: Option<CanonicalUuid>,
            pub updated_at: chrono::DateTime<chrono::Utc>,
            pub projection_freshness: ProjectionFreshness,
            pub admission: RunContinuationAdmissionV1,
            $(#[serde(deserialize_with = "required_field")] pub $extra: $ty,)*
        }
        impl $name {
            /// Presentation eligibility only; never authorizes a mutation.
            pub fn actionable_next_actions(&self) -> &[ContinuationNextAction] {
                if self.phase.known().is_none() || self.projection_freshness != ProjectionFreshness::Fresh {
                    &[]
                } else { self.next_actions.as_slice() }
            }
        }
    };
}

readback_type!(RunContinuationCompactReadbackV1);
readback_type!(RunContinuationReadbackV1,
    manifest_page: BoundedList<CarryForwardEntryV1, 500>, next_cursor: Option<PageCursor>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCarryForwardLinksV1 {
    pub schema_version: LinksSchema,
    #[serde(deserialize_with = "required_nullable")]
    pub incoming: Option<RunContinuationCompactReadbackV1>,
    #[serde(deserialize_with = "required_nullable")]
    pub outgoing: Option<RunContinuationCompactReadbackV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalApprovalRelation {
    Bound,
    HistoricalBindingUnproven,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarryForwardHashedEntryV1 {
    pub entry_id: BoundedText<256>,
    pub source_namespace: BoundedText<256>,
    #[serde(deserialize_with = "required_nullable")]
    pub source_artifact_id: Option<CanonicalUuid>,
    pub logical_name: BoundedText<256>,
    pub source_relative_path: RelativePath,
    pub sha256: ContentDigest,
    pub bytes: u64,
    pub mode: u32,
    #[serde(deserialize_with = "required_nullable")]
    pub source_contract: Option<BoundedText<256>>,
    #[serde(deserialize_with = "required_nullable")]
    pub source_schema_version: Option<BoundedText<256>>,
    pub target_logical_name: BoundedText<256>,
    pub reason: Reason,
    pub historical_approval_relation: HistoricalApprovalRelation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarryForwardExcludedEntryV1 {
    pub entry_id: BoundedText<256>,
    pub source_namespace: BoundedText<256>,
    #[serde(deserialize_with = "required_nullable")]
    pub source_artifact_id: Option<CanonicalUuid>,
    pub logical_name: BoundedText<256>,
    pub source_relative_path: RelativePath,
    pub reason: Reason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum CarryForwardEntryV1 {
    ExecutionSeed(CarryForwardHashedEntryV1),
    ReferenceOnly(CarryForwardHashedEntryV1),
    PreserveOnly(CarryForwardHashedEntryV1),
    Excluded(CarryForwardExcludedEntryV1),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GitObjectId(String);

impl TryFrom<String> for GitObjectId {
    type Error = WireError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !matches!(value.len(), 40 | 64)
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(WireError("canonical Git object ID required"));
        }
        Ok(Self(value))
    }
}

impl GitObjectId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<GitObjectId> for String {
    fn from(value: GitObjectId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct BoundedCount<const MAX: u64>(u64);

impl<const MAX: u64> TryFrom<u64> for BoundedCount<MAX> {
    type Error = WireError;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value > MAX {
            return Err(WireError("count exceeds ceiling"));
        }
        Ok(Self(value))
    }
}

impl<const MAX: u64> BoundedCount<MAX> {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl<const MAX: u64> From<BoundedCount<MAX>> for u64 {
    fn from(value: BoundedCount<MAX>) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryIdentityV1 {
    pub root: BoundedText<256>,
    pub device: u64,
    pub inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceWitnessV1 {
    pub workflow_snapshot_hash: ContentDigest,
    pub catalog_snapshot_hash: ContentDigest,
    pub stage_execution_id: CanonicalUuid,
    pub cursor: BoundedText<4096>,
    pub journal_ids: BoundedList<CanonicalUuid, 128>,
    pub approval_ids: BoundedList<CanonicalUuid, 128>,
    pub artifact_generation_ids: BoundedList<CanonicalUuid, 128>,
    pub idea_sha256: ContentDigest,
    pub head: GitObjectId,
    pub tree: GitObjectId,
    pub index_sha256: ContentDigest,
    pub content_sha256: ContentDigest,
    pub directory_identities: BoundedList<DirectoryIdentityV1, 128>,
    pub settled_ownership_effect_sha256: ContentDigest,
}

/// Detailed inventories remain in the canonical plan/manifest, not this summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceSnapshotSummaryV1 {
    pub base_branch: BoundedText<4096>,
    pub base_revision: GitObjectId,
    pub head: GitObjectId,
    pub staged_patch_sha256: ContentDigest,
    pub unstaged_patch_sha256: ContentDigest,
    pub inventory_sha256: ContentDigest,
    pub untracked_count: BoundedCount<50000>,
    pub deleted_count: BoundedCount<50000>,
    pub link_count: BoundedCount<50000>,
    pub target_content_sha256: ContentDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDeltaV1 {
    pub added: BoundedList<BoundedText<256>, 128>,
    pub removed: BoundedList<BoundedText<256>, 128>,
    pub sha256: ContentDigest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarryForwardLimitsV1 {
    pub max_entries: BoundedCount<50000>,
    pub max_preservation_bytes: BoundedCount<2147483648>,
    pub max_file_bytes: BoundedCount<268435456>,
    pub max_reference_artifacts: BoundedCount<128>,
    pub max_request_bytes: BoundedCount<1048576>,
    pub max_summary_bytes: BoundedCount<65536>,
    pub default_page_size: PageLimit,
    pub max_page_size: PageLimit,
    pub streaming_buffer_bytes: BoundedCount<1048576>,
}

impl Default for CarryForwardLimitsV1 {
    fn default() -> Self {
        Self {
            max_entries: BoundedCount(50000),
            max_preservation_bytes: BoundedCount(2147483648),
            max_file_bytes: BoundedCount(268435456),
            max_reference_artifacts: BoundedCount(128),
            max_request_bytes: BoundedCount(1048576),
            max_summary_bytes: BoundedCount(65536),
            default_page_size: PageLimit(100),
            max_page_size: PageLimit(500),
            streaming_buffer_bytes: BoundedCount(1048576),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCarryForwardPlanSummaryV1 {
    pub source_run_id: CanonicalUuid,
    pub source_witness: SourceWitnessV1,
    pub target: CarryForwardTarget,
    pub profile: CarryForwardProfile,
    pub selection: CarryForwardSelection,
    pub workspace_snapshot: WorkspaceSnapshotSummaryV1,
    pub capability_delta: CapabilityDeltaV1,
    pub holds: BoundedList<ContinuationDenial, 128>,
    pub limits: CarryForwardLimitsV1,
}

fn bounded_summary<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<RunCarryForwardPlanSummaryV1, D::Error> {
    let summary = RunCarryForwardPlanSummaryV1::deserialize(d)?;
    let bytes = serde_json::to_vec(&summary).map_err(de::Error::custom)?;
    if bytes.len() > MAX_SUMMARY_BYTES {
        return Err(de::Error::custom("summary exceeds 64 KiB"));
    }
    Ok(summary)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunCarryForwardPreviewPageV1 {
    pub schema_version: PreviewPageSchema,
    #[serde(deserialize_with = "bounded_summary")]
    pub plan_summary: RunCarryForwardPlanSummaryV1,
    pub entries: BoundedList<CarryForwardEntryV1, 500>,
    pub entry_count: BoundedCount<50000>,
    /// Digest of the entire canonical plan, never the page or cursor.
    pub plan_sha256: ContentDigest,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<PageCursor>,
}
