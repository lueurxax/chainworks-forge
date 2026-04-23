use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ids::{RunId, StageExecutionId};

const DEFAULT_EXACT_PATH_READ_CAP_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_PRE_PROMPT_MAX_EXPECTED_OUTPUT_SPECS: usize = 200;
const DEFAULT_PRE_PROMPT_AGGREGATE_DIGEST_READ_BUDGET_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_PRE_PROMPT_METADATA_TIMEOUT: Duration = Duration::from_secs(5);
const BOUNDED_META_ROOT_MAX_FILES: usize = 500;
const BOUNDED_META_ROOT_MAX_BYTES_PER_FILE: u64 = 1024 * 1024;
const BOUNDED_META_ROOT_MAX_TOTAL_BYTES: u64 = 10 * 1024 * 1024;
const LEGACY_BROAD_DISCOVERY_MAX_FILES: usize = 1000;
const LEGACY_BROAD_DISCOVERY_MAX_BYTES_PER_FILE: u64 = 1024 * 1024;
const LEGACY_BROAD_DISCOVERY_MAX_TOTAL_BYTES: u64 = 10 * 1024 * 1024;
const LEGACY_BROAD_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryOperationKind {
    SnapshotFiles,
    LegacyBroadDiscovery,
    CaptureExpectedPathBaseline,
    CapturePrePromptMetadata,
    BoundedPrePromptMetadata,
    BoundedMetaRootDiscovery,
    ReadDir,
    SymlinkMetadata,
    Canonicalize,
    ReadFile,
    GeneratedStateDirSkipped,
    GeneratedStateFileSkipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryOperation {
    pub kind: DiscoveryOperationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_name: Option<String>,
    #[serde(default)]
    pub details: BTreeMap<String, String>,
}

impl DiscoveryOperation {
    pub fn new(kind: DiscoveryOperationKind) -> Self {
        Self {
            kind,
            path: None,
            output_name: None,
            details: BTreeMap::new(),
        }
    }

    pub fn path(kind: DiscoveryOperationKind, path: impl AsRef<Path>) -> Self {
        let mut operation = Self::new(kind);
        operation.path = Some(path.as_ref().to_string_lossy().into_owned());
        operation
    }

    pub fn output(kind: DiscoveryOperationKind, spec: &ExpectedOutputSpec) -> Self {
        let mut operation = Self::path(kind, Path::new(&spec.target_path));
        operation.output_name = Some(spec.output_name.clone());
        operation
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

pub trait DiscoveryOperationRecorder: Send + Sync {
    fn record(&self, operation: DiscoveryOperation);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopDiscoveryOperationRecorder;

impl DiscoveryOperationRecorder for NoopDiscoveryOperationRecorder {
    fn record(&self, _operation: DiscoveryOperation) {}
}

#[derive(Debug, Clone, Default)]
pub struct RecordingDiscoveryOperationRecorder {
    operations: Arc<Mutex<Vec<DiscoveryOperation>>>,
}

impl RecordingDiscoveryOperationRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn operations(&self) -> Vec<DiscoveryOperation> {
        self.operations
            .lock()
            .map(|operations| operations.clone())
            .unwrap_or_default()
    }
}

impl DiscoveryOperationRecorder for RecordingDiscoveryOperationRecorder {
    fn record(&self, operation: DiscoveryOperation) {
        if let Ok(mut operations) = self.operations.lock() {
            operations.push(operation);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedOutputRole {
    Machine,
    Companion,
    ControlPlane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputReusePolicy {
    MustProduce,
    AllowUnchangedExisting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputRootClass {
    Worktree,
    Workspace,
    ChainworksMetaRoot,
    ControlPlaneGenerated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceGenerationOwner {
    Agent,
    ControlPlane,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyBroadDiscoveryPolicy {
    #[default]
    Disabled,
    WorkflowOptIn,
}

impl LegacyBroadDiscoveryPolicy {
    pub fn allows_broad_discovery(self) -> bool {
        matches!(self, LegacyBroadDiscoveryPolicy::WorkflowOptIn)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyDiscoveryOverrideStatus {
    Pending,
    Consumed,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyDiscoveryOverrideInput {
    pub run_id: RunId,
    pub stage_id: String,
    pub workflow_id: String,
    pub target_stage_execution_id: StageExecutionId,
    pub target_attempt_number: i64,
    pub actor_id: String,
    pub reason: String,
    pub requested_policy: LegacyBroadDiscoveryPolicy,
    pub from_policy: LegacyBroadDiscoveryPolicy,
    pub approval_source: String,
    pub journal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyDiscoveryOverride {
    pub override_id: String,
    pub run_id: RunId,
    pub stage_id: String,
    pub workflow_id: String,
    pub target_stage_execution_id: StageExecutionId,
    pub target_attempt_number: i64,
    pub actor_id: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub expires_at_attempt: i64,
    pub requested_policy: LegacyBroadDiscoveryPolicy,
    pub from_policy: LegacyBroadDiscoveryPolicy,
    pub approval_source: String,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_at: Option<DateTime<Utc>>,
    pub status: LegacyDiscoveryOverrideStatus,
    pub journal_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedRoot {
    pub root_class: OutputRootClass,
    pub root_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedOutputSpec {
    pub output_name: String,
    pub output_role: ExpectedOutputRole,
    pub target_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_of: Option<String>,
    pub display_label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    pub required: bool,
    pub reuse_policy: OutputReusePolicy,
    pub max_bytes: u64,
    pub aggregate_acceptance_cap_bytes: u64,
    pub authorized_roots: Vec<AuthorizedRoot>,
    pub source_generation_owner: SourceGenerationOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedPathBaselineStatus {
    Absent,
    #[serde(rename = "regular_digest_captured", alias = "regular_content_captured")]
    RegularContentCaptured,
    Oversized,
    Unreadable,
    NotRegularFile,
    SymlinkEscape,
    UnauthorizedRoot,
    MetadataTimeout,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedPathBaseline {
    pub target_path: String,
    pub status: ExpectedPathBaselineStatus,
    pub content: Option<Vec<u8>>,
}

impl ExpectedPathBaseline {
    pub fn absent(path: impl AsRef<Path>) -> Self {
        Self {
            target_path: path.as_ref().to_string_lossy().into_owned(),
            status: ExpectedPathBaselineStatus::Absent,
            content: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrePromptExpectedOutputMetadata {
    pub output_name: String,
    pub target_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    pub root_class: OutputRootClass,
    pub existed: bool,
    pub file_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime_ns: Option<i128>,
    pub baseline_status: ExpectedPathBaselineStatus,
    pub agent_execution_id: String,
    pub stage_execution_id: String,
    pub attempt_number: u32,
    pub session_generation_id: String,
    pub prompt_turn_id: String,
    pub discovery_generation_id: String,
}

impl PrePromptExpectedOutputMetadata {
    pub fn absent(spec: &ExpectedOutputSpec, context: &PrePromptExpectedOutputContext) -> Self {
        Self {
            output_name: spec.output_name.clone(),
            target_path: spec.target_path.clone(),
            canonical_path: None,
            root_class: spec
                .authorized_roots
                .first()
                .map(|root| root.root_class)
                .unwrap_or(OutputRootClass::Workspace),
            existed: false,
            file_type: "absent".to_string(),
            size_bytes: None,
            content_digest: None,
            mtime_ns: None,
            baseline_status: ExpectedPathBaselineStatus::Absent,
            agent_execution_id: context.agent_execution_id.clone(),
            stage_execution_id: context.stage_execution_id.clone(),
            attempt_number: context.attempt_number,
            session_generation_id: context.session_generation_id.clone(),
            prompt_turn_id: context.prompt_turn_id.clone(),
            discovery_generation_id: context.discovery_generation_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrePromptExpectedOutputContext {
    pub agent_execution_id: String,
    pub stage_execution_id: String,
    pub attempt_number: u32,
    pub session_generation_id: String,
    pub prompt_turn_id: String,
    pub discovery_generation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrePromptMetadataBounds {
    pub max_expected_output_specs: usize,
    pub aggregate_digest_read_budget_bytes: u64,
    pub timeout: Duration,
}

impl Default for PrePromptMetadataBounds {
    fn default() -> Self {
        Self {
            max_expected_output_specs: DEFAULT_PRE_PROMPT_MAX_EXPECTED_OUTPUT_SPECS,
            aggregate_digest_read_budget_bytes:
                DEFAULT_PRE_PROMPT_AGGREGATE_DIGEST_READ_BUDGET_BYTES,
            timeout: DEFAULT_PRE_PROMPT_METADATA_TIMEOUT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputDiscoveryStatus {
    Accepted,
    Missing,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputDiscoveryReason {
    ProviderEnvelope,
    ProviderEnvelopeOversized,
    ChainworksOutputOversized,
    ExactPathNew,
    ExactPathChanged,
    DeclaredReusePolicy,
    ControlPlaneGenerated,
    MissingAfterPrompt,
    StaleExpectedOutput,
    BaselineUncertain,
    BaselineUnreadable,
    MetadataTimeout,
    Oversized,
    AggregateExactOutputCap,
    ProviderEnvelopeParseError,
    SymlinkEscape,
    UnauthorizedRoot,
    WrongRunMetaRoot,
    NotRegularFile,
    ContractInvalid,
    ReadError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputDiscoveryProvenance {
    ProviderEnvelope,
    ExactPath,
    DeclaredReusePolicy,
    ControlPlaneGenerated,
    BoundedMetaRoot,
    LegacyBroadDiscovery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputDiscoveryDecision {
    pub output_name: String,
    pub output_role: ExpectedOutputRole,
    pub target_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_of: Option<String>,
    pub status: OutputDiscoveryStatus,
    pub reason: OutputDiscoveryReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<OutputDiscoveryProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_class: Option<OutputRootClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_status: Option<ExpectedPathBaselineStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes_applied: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate_bytes_after_acceptance: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_payload_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_bytes_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    #[serde(default)]
    pub diagnostics: BTreeMap<String, String>,
    pub decision_at: DateTime<Utc>,
}

impl OutputDiscoveryDecision {
    pub fn is_accepted(&self) -> bool {
        self.status == OutputDiscoveryStatus::Accepted
    }
}

pub const DISCOVERY_DIAGNOSTICS_V1_SCHEMA_VERSION: &str = "discovery_diagnostics_v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryDiagnosticsV1 {
    pub schema_version: String,
    pub agent_execution_id: String,
    #[serde(default)]
    pub decisions: Vec<OutputDiscoveryDecision>,
    #[serde(default)]
    pub pre_prompt_expected_outputs: Vec<PrePromptExpectedOutputMetadata>,
    pub legacy_broad_discovery_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounded_meta_root_discovery: Option<BoundedMetaRootDiscovery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_manifest_status: Option<String>,
    #[serde(default)]
    pub resume_warnings: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub generated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_pre_initialize_local_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_initialize_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_session_new_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_prompt_duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_pre_prompt_metadata_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_pre_prompt_metadata_timeout: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_pre_prompt_metadata_digest_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_expected_output_spec_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_control_plane_manifest_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_exact_output_acceptance_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_meta_root_discovery_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_git_changed_files_latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_expected_outputs_found_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_expected_outputs_missing_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_expected_outputs_stale_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_expected_outputs_rejected_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_meta_discovery_truncated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_meta_discovery_truncation_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_legacy_broad_discovery_policy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_legacy_broad_discovery_used: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_git_manifest_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_resume_discovery_warning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_discovery_schema_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_discovery_override_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_missing_required_output_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_rejected_output_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_stale_output_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_exact_output_acceptance_timeout: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_exact_output_aggregate_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_exact_output_aggregate_cap_hit: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_cap_validation_sample_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_cap_validation_p90_output_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_cap_validation_p90_aggregate_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_legacy_broad_discovery_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_legacy_broad_discovery_truncation_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acp_reconciliation_pending: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentExecutionDiscoveryDiagnostics {
    pub agent_execution_id: String,
    pub payload: DiscoveryDiagnosticsV1,
    pub discovery_schema_version: String,
    pub legacy_broad_discovery_used: bool,
    pub missing_required_output_count: i64,
    pub rejected_output_count: i64,
    pub stale_output_count: i64,
    pub meta_discovery_truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_manifest_status: Option<String>,
    pub resume_warning_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentExecutionDiscoveryDiagnostics {
    pub fn from_payload(mut payload: DiscoveryDiagnosticsV1, now: DateTime<Utc>) -> Self {
        let missing_required_output_count = payload
            .decisions
            .iter()
            .filter(|decision| decision.status == OutputDiscoveryStatus::Missing)
            .count() as i64;
        let rejected_output_count = payload
            .decisions
            .iter()
            .filter(|decision| decision.status == OutputDiscoveryStatus::Rejected)
            .count() as i64;
        let stale_output_count = payload
            .decisions
            .iter()
            .filter(|decision| decision.reason == OutputDiscoveryReason::StaleExpectedOutput)
            .count() as i64;
        let meta_discovery_truncated =
            payload
                .bounded_meta_root_discovery
                .as_ref()
                .is_some_and(|discovery| {
                    discovery.truncated_by_file_cap
                        || discovery.truncated_by_file_size
                        || discovery.truncated_by_total_bytes
                });
        payload
            .acp_expected_output_spec_count
            .get_or_insert_with(|| payload.pre_prompt_expected_outputs.len() as u64);
        payload
            .acp_expected_outputs_found_count
            .get_or_insert_with(|| {
                payload
                    .decisions
                    .iter()
                    .filter(|decision| decision.status == OutputDiscoveryStatus::Accepted)
                    .count() as i64
            });
        payload
            .acp_expected_outputs_missing_count
            .get_or_insert(missing_required_output_count);
        payload
            .acp_expected_outputs_rejected_count
            .get_or_insert(rejected_output_count);
        payload
            .acp_expected_outputs_stale_count
            .get_or_insert(stale_output_count);
        payload
            .acp_meta_discovery_truncated
            .get_or_insert(meta_discovery_truncated);
        payload.acp_meta_discovery_truncation_reason =
            payload.acp_meta_discovery_truncation_reason.or_else(|| {
                payload
                    .bounded_meta_root_discovery
                    .as_ref()
                    .and_then(bounded_meta_root_truncation_reason)
            });
        payload
            .acp_legacy_broad_discovery_used
            .get_or_insert(payload.legacy_broad_discovery_used);
        payload.acp_git_manifest_status = payload
            .acp_git_manifest_status
            .clone()
            .or_else(|| payload.git_manifest_status.clone());
        payload.acp_resume_discovery_warning = payload
            .acp_resume_discovery_warning
            .clone()
            .or_else(|| payload.resume_warnings.first().cloned());
        payload.acp_discovery_schema_version = payload
            .acp_discovery_schema_version
            .clone()
            .or_else(|| Some(payload.schema_version.clone()));
        payload
            .acp_missing_required_output_count
            .get_or_insert(missing_required_output_count);
        payload
            .acp_rejected_output_count
            .get_or_insert(rejected_output_count);
        payload
            .acp_stale_output_count
            .get_or_insert(stale_output_count);
        payload
            .acp_exact_output_acceptance_timeout
            .get_or_insert(false);
        payload.acp_exact_output_aggregate_bytes =
            payload.acp_exact_output_aggregate_bytes.or_else(|| {
                payload
                    .decisions
                    .iter()
                    .filter_map(|decision| decision.aggregate_bytes_after_acceptance)
                    .max()
            });
        payload
            .acp_exact_output_aggregate_cap_hit
            .get_or_insert_with(|| {
                payload.decisions.iter().any(|decision| {
                    decision.reason == OutputDiscoveryReason::AggregateExactOutputCap
                })
            });
        payload
            .acp_legacy_broad_discovery_timeout_ms
            .get_or_insert(LEGACY_BROAD_DISCOVERY_TIMEOUT.as_millis() as u64);
        payload.acp_legacy_broad_discovery_truncation_reason = payload
            .acp_legacy_broad_discovery_truncation_reason
            .clone()
            .or_else(|| {
                payload.warnings.iter().find_map(|warning| {
                    warning
                        .strip_prefix("legacy_broad_discovery_truncated:")
                        .map(str::to_string)
                })
            });
        Self {
            agent_execution_id: payload.agent_execution_id.clone(),
            discovery_schema_version: payload.schema_version.clone(),
            legacy_broad_discovery_used: payload.legacy_broad_discovery_used,
            missing_required_output_count,
            rejected_output_count,
            stale_output_count,
            meta_discovery_truncated,
            git_manifest_status: payload.git_manifest_status.clone(),
            resume_warning_count: payload.resume_warnings.len() as i64,
            payload,
            created_at: now,
            updated_at: now,
        }
    }
}

fn bounded_meta_root_truncation_reason(discovery: &BoundedMetaRootDiscovery) -> Option<String> {
    if discovery.truncated_by_file_cap {
        Some("file_cap".to_string())
    } else if discovery.truncated_by_file_size {
        Some("per_file_bytes".to_string())
    } else if discovery.truncated_by_total_bytes {
        Some("total_bytes".to_string())
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedMetaRootDiscovery {
    pub root_path: String,
    pub artifact_paths: Vec<String>,
    pub files_visited: usize,
    pub total_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    pub truncated_by_file_cap: bool,
    pub truncated_by_file_size: bool,
    pub truncated_by_total_bytes: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyBroadDiscoverySnapshot {
    pub root_path: String,
    pub files: HashSet<String>,
    pub files_visited: usize,
    pub total_bytes: u64,
    pub truncated_by_file_cap: bool,
    pub truncated_by_file_size: bool,
    pub truncated_by_total_bytes: bool,
    pub timed_out: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl LegacyBroadDiscoverySnapshot {
    fn skipped(root_path: String, warning: impl Into<String>) -> Self {
        Self {
            root_path,
            files: HashSet::new(),
            files_visited: 0,
            total_bytes: 0,
            truncated_by_file_cap: false,
            truncated_by_file_size: false,
            truncated_by_total_bytes: false,
            timed_out: false,
            warnings: vec![warning.into()],
        }
    }

    pub fn was_truncated(&self) -> bool {
        self.truncated_by_file_cap
            || self.truncated_by_file_size
            || self.truncated_by_total_bytes
            || self.timed_out
    }
}

impl BoundedMetaRootDiscovery {
    fn skipped(root_path: String, warning: impl Into<String>) -> Self {
        Self {
            root_path,
            artifact_paths: Vec::new(),
            files_visited: 0,
            total_bytes: 0,
            latency_ms: None,
            truncated_by_file_cap: false,
            truncated_by_file_size: false,
            truncated_by_total_bytes: false,
            warnings: vec![warning.into()],
        }
    }
}

pub trait DiscoveryFilesystem: Send + Sync {
    fn capture_expected_path_baseline_with_recorder(
        &self,
        path: &Path,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> ExpectedPathBaseline;

    fn capture_pre_prompt_expected_output_metadata_with_recorder(
        &self,
        spec: &ExpectedOutputSpec,
        context: &PrePromptExpectedOutputContext,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> PrePromptExpectedOutputMetadata;

    fn discover_bounded_meta_root_artifacts_with_recorder(
        &self,
        root: &Path,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> BoundedMetaRootDiscovery;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StdDiscoveryFilesystem;

impl DiscoveryFilesystem for StdDiscoveryFilesystem {
    fn capture_expected_path_baseline_with_recorder(
        &self,
        path: &Path,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> ExpectedPathBaseline {
        StdDiscoveryFilesystem::capture_expected_path_baseline_with_recorder(path, recorder)
    }

    fn capture_pre_prompt_expected_output_metadata_with_recorder(
        &self,
        spec: &ExpectedOutputSpec,
        context: &PrePromptExpectedOutputContext,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> PrePromptExpectedOutputMetadata {
        StdDiscoveryFilesystem::capture_pre_prompt_expected_output_metadata_with_recorder(
            spec, context, recorder,
        )
    }

    fn discover_bounded_meta_root_artifacts_with_recorder(
        &self,
        root: &Path,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> BoundedMetaRootDiscovery {
        StdDiscoveryFilesystem::discover_bounded_meta_root_artifacts_with_recorder(root, recorder)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FakeDiscoveryFilesystem {
    expected_path_baselines: BTreeMap<String, ExpectedPathBaseline>,
    pre_prompt_metadata: BTreeMap<String, PrePromptExpectedOutputMetadata>,
    bounded_meta_root_discoveries: BTreeMap<String, BoundedMetaRootDiscovery>,
}

impl FakeDiscoveryFilesystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_expected_path_baseline(
        mut self,
        path: impl Into<String>,
        baseline: ExpectedPathBaseline,
    ) -> Self {
        self.expected_path_baselines.insert(path.into(), baseline);
        self
    }

    pub fn with_pre_prompt_metadata(
        mut self,
        output_name: impl Into<String>,
        metadata: PrePromptExpectedOutputMetadata,
    ) -> Self {
        self.pre_prompt_metadata
            .insert(output_name.into(), metadata);
        self
    }

    pub fn with_bounded_meta_root_discovery(
        mut self,
        root: impl Into<String>,
        discovery: BoundedMetaRootDiscovery,
    ) -> Self {
        self.bounded_meta_root_discoveries
            .insert(root.into(), discovery);
        self
    }
}

impl DiscoveryFilesystem for FakeDiscoveryFilesystem {
    fn capture_expected_path_baseline_with_recorder(
        &self,
        path: &Path,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> ExpectedPathBaseline {
        recorder.record(DiscoveryOperation::path(
            DiscoveryOperationKind::CaptureExpectedPathBaseline,
            path,
        ));
        self.expected_path_baselines
            .get(&path.to_string_lossy().into_owned())
            .cloned()
            .unwrap_or_else(|| ExpectedPathBaseline::absent(path))
    }

    fn capture_pre_prompt_expected_output_metadata_with_recorder(
        &self,
        spec: &ExpectedOutputSpec,
        _context: &PrePromptExpectedOutputContext,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> PrePromptExpectedOutputMetadata {
        recorder.record(DiscoveryOperation::output(
            DiscoveryOperationKind::CapturePrePromptMetadata,
            spec,
        ));
        self.pre_prompt_metadata
            .get(&spec.output_name)
            .cloned()
            .unwrap_or_else(|| PrePromptExpectedOutputMetadata::absent(spec, _context))
    }

    fn discover_bounded_meta_root_artifacts_with_recorder(
        &self,
        root: &Path,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> BoundedMetaRootDiscovery {
        recorder.record(DiscoveryOperation::path(
            DiscoveryOperationKind::BoundedMetaRootDiscovery,
            root,
        ));
        self.bounded_meta_root_discoveries
            .get(&root.to_string_lossy().into_owned())
            .cloned()
            .unwrap_or_else(|| {
                BoundedMetaRootDiscovery::skipped(
                    root.to_string_lossy().into_owned(),
                    "fake_discovery_not_configured",
                )
            })
    }
}

impl StdDiscoveryFilesystem {
    pub fn snapshot_files(root: impl AsRef<Path>) -> HashSet<String> {
        let recorder = NoopDiscoveryOperationRecorder;
        Self::snapshot_files_with_recorder(root, &recorder)
    }

    pub fn snapshot_files_with_recorder(
        root: impl AsRef<Path>,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> HashSet<String> {
        let mut files = HashSet::new();
        recorder.record(DiscoveryOperation::path(
            DiscoveryOperationKind::SnapshotFiles,
            root.as_ref(),
        ));
        collect_files(root.as_ref(), &mut files, recorder);
        files
    }

    pub fn snapshot_legacy_broad_discovery(root: impl AsRef<Path>) -> LegacyBroadDiscoverySnapshot {
        let recorder = NoopDiscoveryOperationRecorder;
        Self::snapshot_legacy_broad_discovery_with_recorder(root, &recorder)
    }

    pub fn snapshot_legacy_broad_discovery_with_recorder(
        root: impl AsRef<Path>,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> LegacyBroadDiscoverySnapshot {
        snapshot_legacy_broad_discovery(root.as_ref(), recorder)
    }

    pub fn capture_expected_path_baseline(path: impl AsRef<Path>) -> ExpectedPathBaseline {
        let recorder = NoopDiscoveryOperationRecorder;
        Self::capture_expected_path_baseline_with_recorder(path, &recorder)
    }

    pub fn capture_expected_path_baseline_with_recorder(
        path: impl AsRef<Path>,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> ExpectedPathBaseline {
        capture_expected_path_baseline(path.as_ref(), DEFAULT_EXACT_PATH_READ_CAP_BYTES, recorder)
    }

    pub fn capture_pre_prompt_expected_output_metadata(
        spec: &ExpectedOutputSpec,
        context: &PrePromptExpectedOutputContext,
    ) -> PrePromptExpectedOutputMetadata {
        let recorder = NoopDiscoveryOperationRecorder;
        Self::capture_pre_prompt_expected_output_metadata_with_recorder(spec, context, &recorder)
    }

    pub fn capture_pre_prompt_expected_output_metadata_with_recorder(
        spec: &ExpectedOutputSpec,
        context: &PrePromptExpectedOutputContext,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> PrePromptExpectedOutputMetadata {
        capture_pre_prompt_expected_output_metadata(spec, context, None, None, recorder)
    }

    pub fn capture_bounded_pre_prompt_expected_output_metadata(
        specs: &[ExpectedOutputSpec],
        context: &PrePromptExpectedOutputContext,
    ) -> Vec<PrePromptExpectedOutputMetadata> {
        Self::capture_bounded_pre_prompt_expected_output_metadata_with_bounds(
            specs,
            context,
            PrePromptMetadataBounds::default(),
        )
    }

    pub fn capture_bounded_pre_prompt_expected_output_metadata_with_bounds(
        specs: &[ExpectedOutputSpec],
        context: &PrePromptExpectedOutputContext,
        bounds: PrePromptMetadataBounds,
    ) -> Vec<PrePromptExpectedOutputMetadata> {
        let recorder = NoopDiscoveryOperationRecorder;
        Self::capture_bounded_pre_prompt_expected_output_metadata_with_bounds_and_recorder(
            specs, context, bounds, &recorder,
        )
    }

    pub fn capture_bounded_pre_prompt_expected_output_metadata_with_bounds_and_recorder(
        specs: &[ExpectedOutputSpec],
        context: &PrePromptExpectedOutputContext,
        bounds: PrePromptMetadataBounds,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> Vec<PrePromptExpectedOutputMetadata> {
        capture_bounded_pre_prompt_expected_output_metadata(specs, context, bounds, recorder)
    }

    pub fn expected_path_has_current_content(baseline: &ExpectedPathBaseline) -> bool {
        let path = Path::new(&baseline.target_path);
        let recorder = NoopDiscoveryOperationRecorder;
        let current =
            capture_expected_path_baseline(path, DEFAULT_EXACT_PATH_READ_CAP_BYTES, &recorder);
        match (&baseline.status, &current.status) {
            (
                ExpectedPathBaselineStatus::Absent,
                ExpectedPathBaselineStatus::RegularContentCaptured,
            ) => true,
            (
                ExpectedPathBaselineStatus::RegularContentCaptured,
                ExpectedPathBaselineStatus::RegularContentCaptured,
            ) => baseline.content != current.content,
            _ => false,
        }
    }

    pub fn expected_output_has_current_content(
        spec: &ExpectedOutputSpec,
        baseline: &PrePromptExpectedOutputMetadata,
        context: &PrePromptExpectedOutputContext,
    ) -> bool {
        if !matches!(
            baseline.baseline_status,
            ExpectedPathBaselineStatus::Absent | ExpectedPathBaselineStatus::RegularContentCaptured
        ) {
            return false;
        }
        let recorder = NoopDiscoveryOperationRecorder;
        let current =
            capture_pre_prompt_expected_output_metadata(spec, context, None, None, &recorder);
        match (&baseline.baseline_status, &current.baseline_status) {
            (
                ExpectedPathBaselineStatus::Absent,
                ExpectedPathBaselineStatus::RegularContentCaptured,
            ) => true,
            (
                ExpectedPathBaselineStatus::RegularContentCaptured,
                ExpectedPathBaselineStatus::RegularContentCaptured,
            ) => baseline.content_digest != current.content_digest,
            _ => false,
        }
    }

    pub fn should_skip_generated_state_dir(path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        if ends_with_components(path, &[".chainworks", "worktrees"])
            || ends_with_components(path, &[".chainworks", "backups"])
            || ends_with_components(path, &[".claude", "worktrees"])
            || ends_with_components(path, &[".git", "objects"])
        {
            return true;
        }

        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                matches!(
                    name,
                    ".forge-codex-acp" | "target" | "DerivedData" | ".build" | "node_modules"
                )
            })
    }

    pub fn should_skip_generated_state_file(path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        let in_chainworks = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some(".chainworks");

        in_chainworks && (file_name.contains(".db.backup-") || file_name.ends_with(".sqlite"))
    }

    pub fn discover_bounded_meta_root_artifacts(
        root: impl AsRef<Path>,
    ) -> BoundedMetaRootDiscovery {
        let recorder = NoopDiscoveryOperationRecorder;
        Self::discover_bounded_meta_root_artifacts_with_recorder(root, &recorder)
    }

    pub fn discover_bounded_meta_root_artifacts_with_recorder(
        root: impl AsRef<Path>,
        recorder: &dyn DiscoveryOperationRecorder,
    ) -> BoundedMetaRootDiscovery {
        discover_bounded_meta_root_artifacts(root.as_ref(), recorder)
    }
}

fn capture_pre_prompt_expected_output_metadata(
    spec: &ExpectedOutputSpec,
    context: &PrePromptExpectedOutputContext,
    digest_budget_remaining: Option<u64>,
    deadline: Option<Instant>,
    recorder: &dyn DiscoveryOperationRecorder,
) -> PrePromptExpectedOutputMetadata {
    let target = Path::new(&spec.target_path);
    recorder.record(DiscoveryOperation::output(
        DiscoveryOperationKind::CapturePrePromptMetadata,
        spec,
    ));
    let mut metadata = PrePromptExpectedOutputMetadata {
        output_name: spec.output_name.clone(),
        target_path: spec.target_path.clone(),
        canonical_path: None,
        root_class: spec
            .authorized_roots
            .first()
            .map(|root| root.root_class)
            .unwrap_or(OutputRootClass::Workspace),
        existed: false,
        file_type: "absent".to_string(),
        size_bytes: None,
        content_digest: None,
        mtime_ns: None,
        baseline_status: ExpectedPathBaselineStatus::Absent,
        agent_execution_id: context.agent_execution_id.clone(),
        stage_execution_id: context.stage_execution_id.clone(),
        attempt_number: context.attempt_number,
        session_generation_id: context.session_generation_id.clone(),
        prompt_turn_id: context.prompt_turn_id.clone(),
        discovery_generation_id: context.discovery_generation_id.clone(),
    };

    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::SymlinkMetadata,
        target,
    ));
    let Ok(link_metadata) = std::fs::symlink_metadata(target) else {
        return metadata;
    };
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        metadata.baseline_status = ExpectedPathBaselineStatus::MetadataTimeout;
        return metadata;
    }
    metadata.existed = true;
    metadata.file_type = if link_metadata.file_type().is_symlink() {
        "symlink".to_string()
    } else if link_metadata.is_file() {
        "regular".to_string()
    } else if link_metadata.is_dir() {
        "directory".to_string()
    } else {
        "other".to_string()
    };

    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::Canonicalize,
        target,
    ));
    let canonical = match std::fs::canonicalize(target) {
        Ok(path) => path,
        Err(_) => {
            metadata.baseline_status = ExpectedPathBaselineStatus::Unreadable;
            return metadata;
        }
    };
    metadata.canonical_path = Some(canonical.to_string_lossy().into_owned());

    let Some(root_class) = authorized_root_class_for_canonical_path(spec, &canonical) else {
        metadata.baseline_status = if link_metadata.file_type().is_symlink() {
            ExpectedPathBaselineStatus::SymlinkEscape
        } else {
            ExpectedPathBaselineStatus::UnauthorizedRoot
        };
        return metadata;
    };
    metadata.root_class = root_class;

    let Ok(file_metadata) = std::fs::metadata(&canonical) else {
        metadata.baseline_status = ExpectedPathBaselineStatus::Unreadable;
        return metadata;
    };
    metadata.mtime_ns = metadata_mtime_ns(&file_metadata);
    if !file_metadata.is_file() {
        metadata.file_type = if file_metadata.is_dir() {
            "directory".to_string()
        } else {
            "other".to_string()
        };
        metadata.baseline_status = ExpectedPathBaselineStatus::NotRegularFile;
        return metadata;
    }

    metadata.file_type = "regular".to_string();
    metadata.size_bytes = Some(file_metadata.len());
    if file_metadata.len() > spec.max_bytes {
        metadata.baseline_status = ExpectedPathBaselineStatus::Oversized;
        return metadata;
    }
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        metadata.baseline_status = ExpectedPathBaselineStatus::MetadataTimeout;
        return metadata;
    }
    if digest_budget_remaining.is_some_and(|remaining| file_metadata.len() > remaining) {
        metadata.baseline_status = ExpectedPathBaselineStatus::Uncertain;
        return metadata;
    }

    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::ReadFile,
        &canonical,
    ));
    match std::fs::read(&canonical) {
        Ok(content) => {
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                metadata.baseline_status = ExpectedPathBaselineStatus::MetadataTimeout;
            } else {
                metadata.content_digest = Some(sha256_digest(&content));
                metadata.baseline_status = ExpectedPathBaselineStatus::RegularContentCaptured;
            }
        }
        Err(_) => {
            metadata.baseline_status = ExpectedPathBaselineStatus::Unreadable;
        }
    }

    metadata
}

fn capture_bounded_pre_prompt_expected_output_metadata(
    specs: &[ExpectedOutputSpec],
    context: &PrePromptExpectedOutputContext,
    bounds: PrePromptMetadataBounds,
    recorder: &dyn DiscoveryOperationRecorder,
) -> Vec<PrePromptExpectedOutputMetadata> {
    recorder.record(
        DiscoveryOperation::new(DiscoveryOperationKind::BoundedPrePromptMetadata)
            .with_detail("spec_count", specs.len().to_string())
            .with_detail(
                "max_expected_output_specs",
                bounds.max_expected_output_specs.to_string(),
            )
            .with_detail(
                "aggregate_digest_read_budget_bytes",
                bounds.aggregate_digest_read_budget_bytes.to_string(),
            ),
    );
    let deadline = Instant::now()
        .checked_add(bounds.timeout)
        .unwrap_or_else(Instant::now);
    let mut digest_budget_remaining = bounds.aggregate_digest_read_budget_bytes;
    let mut captured = Vec::with_capacity(specs.len());

    for (index, spec) in specs.iter().enumerate() {
        if index >= bounds.max_expected_output_specs || Instant::now() >= deadline {
            captured.push(pre_prompt_metadata_with_status(
                spec,
                context,
                ExpectedPathBaselineStatus::MetadataTimeout,
            ));
            continue;
        }

        let metadata = capture_pre_prompt_expected_output_metadata(
            spec,
            context,
            Some(digest_budget_remaining),
            Some(deadline),
            recorder,
        );
        if metadata.baseline_status == ExpectedPathBaselineStatus::RegularContentCaptured {
            if let Some(size_bytes) = metadata.size_bytes {
                digest_budget_remaining = digest_budget_remaining.saturating_sub(size_bytes);
            }
        }
        captured.push(metadata);
    }

    captured
}

fn pre_prompt_metadata_with_status(
    spec: &ExpectedOutputSpec,
    context: &PrePromptExpectedOutputContext,
    status: ExpectedPathBaselineStatus,
) -> PrePromptExpectedOutputMetadata {
    PrePromptExpectedOutputMetadata {
        output_name: spec.output_name.clone(),
        target_path: spec.target_path.clone(),
        canonical_path: None,
        root_class: spec
            .authorized_roots
            .first()
            .map(|root| root.root_class)
            .unwrap_or(OutputRootClass::Workspace),
        existed: false,
        file_type: "unknown".to_string(),
        size_bytes: None,
        content_digest: None,
        mtime_ns: None,
        baseline_status: status,
        agent_execution_id: context.agent_execution_id.clone(),
        stage_execution_id: context.stage_execution_id.clone(),
        attempt_number: context.attempt_number,
        session_generation_id: context.session_generation_id.clone(),
        prompt_turn_id: context.prompt_turn_id.clone(),
        discovery_generation_id: context.discovery_generation_id.clone(),
    }
}

fn authorized_root_class_for_canonical_path(
    spec: &ExpectedOutputSpec,
    canonical_path: &Path,
) -> Option<OutputRootClass> {
    spec.authorized_roots.iter().find_map(|root| {
        let root_path = Path::new(&root.root_path);
        let canonical_root = std::fs::canonicalize(root_path).unwrap_or_else(|_| {
            if root_path.is_absolute() {
                root_path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(root_path))
                    .unwrap_or_else(|_| root_path.to_path_buf())
            }
        });
        canonical_path
            .starts_with(&canonical_root)
            .then_some(root.root_class)
    })
}

fn metadata_mtime_ns(metadata: &std::fs::Metadata) -> Option<i128> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as i128)
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn discover_bounded_meta_root_artifacts(
    root: &Path,
    recorder: &dyn DiscoveryOperationRecorder,
) -> BoundedMetaRootDiscovery {
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::BoundedMetaRootDiscovery,
        root,
    ));
    let root_path = root.to_string_lossy().into_owned();
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::SymlinkMetadata,
        root,
    ));
    let Ok(root_metadata) = std::fs::symlink_metadata(root) else {
        return BoundedMetaRootDiscovery::skipped(root_path, "chainworks_meta_root_absent");
    };
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return BoundedMetaRootDiscovery::skipped(root_path, "chainworks_meta_root_not_directory");
    }
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::Canonicalize,
        root,
    ));
    let Ok(canonical_root) = std::fs::canonicalize(root) else {
        return BoundedMetaRootDiscovery::skipped(root_path, "chainworks_meta_root_unreadable");
    };

    let mut result = BoundedMetaRootDiscovery {
        root_path,
        artifact_paths: Vec::new(),
        files_visited: 0,
        total_bytes: 0,
        latency_ms: None,
        truncated_by_file_cap: false,
        truncated_by_file_size: false,
        truncated_by_total_bytes: false,
        warnings: Vec::new(),
    };
    let mut stack = vec![canonical_root.clone()];

    while let Some(dir) = stack.pop() {
        recorder.record(DiscoveryOperation::path(
            DiscoveryOperationKind::ReadDir,
            &dir,
        ));
        let Ok(entries) = std::fs::read_dir(&dir) else {
            result
                .warnings
                .push(format!("unreadable_dir:{}", dir.display()));
            continue;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();

        for path in entries {
            recorder.record(DiscoveryOperation::path(
                DiscoveryOperationKind::SymlinkMetadata,
                &path,
            ));
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                result
                    .warnings
                    .push(format!("unreadable_path:{}", path.display()));
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }

            if metadata.is_dir() {
                if should_skip_bounded_meta_root_dir(&path) {
                    recorder.record(DiscoveryOperation::path(
                        DiscoveryOperationKind::GeneratedStateDirSkipped,
                        &path,
                    ));
                    continue;
                }
                recorder.record(DiscoveryOperation::path(
                    DiscoveryOperationKind::Canonicalize,
                    &path,
                ));
                let Ok(canonical_dir) = std::fs::canonicalize(&path) else {
                    result
                        .warnings
                        .push(format!("unreadable_dir:{}", path.display()));
                    continue;
                };
                if canonical_dir.starts_with(&canonical_root) {
                    stack.push(canonical_dir);
                }
                continue;
            }

            if !metadata.is_file() {
                continue;
            }
            result.files_visited += 1;
            if result.files_visited > BOUNDED_META_ROOT_MAX_FILES {
                result.truncated_by_file_cap = true;
                return result;
            }
            if metadata.len() > BOUNDED_META_ROOT_MAX_BYTES_PER_FILE {
                result.truncated_by_file_size = true;
                continue;
            }
            if result.total_bytes.saturating_add(metadata.len()) > BOUNDED_META_ROOT_MAX_TOTAL_BYTES
            {
                result.truncated_by_total_bytes = true;
                return result;
            }
            recorder.record(DiscoveryOperation::path(
                DiscoveryOperationKind::Canonicalize,
                &path,
            ));
            let Ok(canonical_file) = std::fs::canonicalize(&path) else {
                result
                    .warnings
                    .push(format!("unreadable_file:{}", path.display()));
                continue;
            };
            if !canonical_file.starts_with(&canonical_root) {
                continue;
            }
            result.total_bytes += metadata.len();
            result
                .artifact_paths
                .push(canonical_file.to_string_lossy().into_owned());
        }
    }

    result.artifact_paths.sort();
    result
}

fn snapshot_legacy_broad_discovery(
    root: &Path,
    recorder: &dyn DiscoveryOperationRecorder,
) -> LegacyBroadDiscoverySnapshot {
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::LegacyBroadDiscovery,
        root,
    ));
    let root_path = root.to_string_lossy().into_owned();
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::SymlinkMetadata,
        root,
    ));
    let Ok(root_metadata) = std::fs::symlink_metadata(root) else {
        return LegacyBroadDiscoverySnapshot::skipped(root_path, "legacy_root_absent");
    };
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return LegacyBroadDiscoverySnapshot::skipped(root_path, "legacy_root_not_directory");
    }
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::Canonicalize,
        root,
    ));
    let Ok(canonical_root) = std::fs::canonicalize(root) else {
        return LegacyBroadDiscoverySnapshot::skipped(root_path, "legacy_root_unreadable");
    };

    let mut result = LegacyBroadDiscoverySnapshot {
        root_path,
        files: HashSet::new(),
        files_visited: 0,
        total_bytes: 0,
        truncated_by_file_cap: false,
        truncated_by_file_size: false,
        truncated_by_total_bytes: false,
        timed_out: false,
        warnings: Vec::new(),
    };
    let deadline = Instant::now()
        .checked_add(LEGACY_BROAD_DISCOVERY_TIMEOUT)
        .unwrap_or_else(Instant::now);
    let mut stack = vec![canonical_root.clone()];

    while let Some(dir) = stack.pop() {
        if Instant::now() >= deadline {
            result.timed_out = true;
            return result;
        }
        recorder.record(DiscoveryOperation::path(
            DiscoveryOperationKind::ReadDir,
            &dir,
        ));
        let Ok(entries) = std::fs::read_dir(&dir) else {
            result
                .warnings
                .push(format!("unreadable_dir:{}", dir.display()));
            continue;
        };
        let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();

        for path in entries {
            if Instant::now() >= deadline {
                result.timed_out = true;
                return result;
            }
            recorder.record(DiscoveryOperation::path(
                DiscoveryOperationKind::SymlinkMetadata,
                &path,
            ));
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                result
                    .warnings
                    .push(format!("unreadable_path:{}", path.display()));
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }

            if metadata.is_dir() {
                if StdDiscoveryFilesystem::should_skip_generated_state_dir(&path) {
                    recorder.record(DiscoveryOperation::path(
                        DiscoveryOperationKind::GeneratedStateDirSkipped,
                        &path,
                    ));
                    continue;
                }
                recorder.record(DiscoveryOperation::path(
                    DiscoveryOperationKind::Canonicalize,
                    &path,
                ));
                let Ok(canonical_dir) = std::fs::canonicalize(&path) else {
                    result
                        .warnings
                        .push(format!("unreadable_dir:{}", path.display()));
                    continue;
                };
                if canonical_dir.starts_with(&canonical_root) {
                    stack.push(canonical_dir);
                }
                continue;
            }

            if !metadata.is_file() {
                continue;
            }
            if StdDiscoveryFilesystem::should_skip_generated_state_file(&path) {
                recorder.record(DiscoveryOperation::path(
                    DiscoveryOperationKind::GeneratedStateFileSkipped,
                    &path,
                ));
                continue;
            }
            result.files_visited += 1;
            if result.files_visited > LEGACY_BROAD_DISCOVERY_MAX_FILES {
                result.truncated_by_file_cap = true;
                return result;
            }
            if metadata.len() > LEGACY_BROAD_DISCOVERY_MAX_BYTES_PER_FILE {
                result.truncated_by_file_size = true;
                continue;
            }
            if result.total_bytes.saturating_add(metadata.len())
                > LEGACY_BROAD_DISCOVERY_MAX_TOTAL_BYTES
            {
                result.truncated_by_total_bytes = true;
                return result;
            }
            recorder.record(DiscoveryOperation::path(
                DiscoveryOperationKind::Canonicalize,
                &path,
            ));
            let Ok(canonical_file) = std::fs::canonicalize(&path) else {
                result
                    .warnings
                    .push(format!("unreadable_file:{}", path.display()));
                continue;
            };
            if !canonical_file.starts_with(&canonical_root) {
                continue;
            }
            result.total_bytes += metadata.len();
            result
                .files
                .insert(canonical_file.to_string_lossy().into_owned());
        }
    }

    result
}

fn should_skip_bounded_meta_root_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git"
                    | ".hg"
                    | ".svn"
                    | ".claude"
                    | ".codex"
                    | ".gemini"
                    | "node_modules"
                    | "target"
                    | "DerivedData"
                    | ".build"
                    | "dist"
                    | "build"
                    | "tmp"
                    | "temp"
                    | "cache"
            )
        })
}

fn capture_expected_path_baseline(
    path: &Path,
    max_bytes: u64,
    recorder: &dyn DiscoveryOperationRecorder,
) -> ExpectedPathBaseline {
    let target_path = path.to_string_lossy().into_owned();
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::CaptureExpectedPathBaseline,
        path,
    ));
    let Ok(metadata) = std::fs::metadata(path) else {
        return ExpectedPathBaseline {
            target_path,
            status: ExpectedPathBaselineStatus::Absent,
            content: None,
        };
    };
    if !metadata.is_file() {
        return ExpectedPathBaseline {
            target_path,
            status: ExpectedPathBaselineStatus::NotRegularFile,
            content: None,
        };
    }
    if metadata.len() > max_bytes {
        return ExpectedPathBaseline {
            target_path,
            status: ExpectedPathBaselineStatus::Oversized,
            content: None,
        };
    }
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::ReadFile,
        path,
    ));
    match std::fs::read(path) {
        Ok(content) => ExpectedPathBaseline {
            target_path,
            status: ExpectedPathBaselineStatus::RegularContentCaptured,
            content: Some(content),
        },
        Err(_) => ExpectedPathBaseline {
            target_path,
            status: ExpectedPathBaselineStatus::Unreadable,
            content: None,
        },
    }
}

fn collect_files(dir: &Path, out: &mut HashSet<String>, recorder: &dyn DiscoveryOperationRecorder) {
    recorder.record(DiscoveryOperation::path(
        DiscoveryOperationKind::ReadDir,
        dir,
    ));
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() {
            continue;
        }
        if path.is_dir() {
            if StdDiscoveryFilesystem::should_skip_generated_state_dir(&path) {
                recorder.record(DiscoveryOperation::path(
                    DiscoveryOperationKind::GeneratedStateDirSkipped,
                    &path,
                ));
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') {
                continue;
            }
            collect_files(&path, out, recorder);
        } else if path.is_file() {
            if StdDiscoveryFilesystem::should_skip_generated_state_file(&path) {
                recorder.record(DiscoveryOperation::path(
                    DiscoveryOperationKind::GeneratedStateFileSkipped,
                    &path,
                ));
                continue;
            }
            if let Some(s) = path.to_str() {
                out.insert(s.to_string());
            }
        }
    }
}

fn ends_with_components(path: &Path, expected: &[&str]) -> bool {
    let components: Vec<&str> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();

    components.ends_with(expected)
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorizedRoot, BoundedMetaRootDiscovery, DiscoveryFilesystem, DiscoveryOperationKind,
        ExpectedOutputRole, ExpectedOutputSpec, ExpectedPathBaseline, ExpectedPathBaselineStatus,
        FakeDiscoveryFilesystem, OutputDiscoveryDecision, OutputDiscoveryProvenance,
        OutputDiscoveryReason, OutputDiscoveryStatus, OutputReusePolicy, OutputRootClass,
        PrePromptExpectedOutputContext, PrePromptMetadataBounds,
        RecordingDiscoveryOperationRecorder, SourceGenerationOwner, StdDiscoveryFilesystem,
    };
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    fn temp_test_dir(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("chainworks-domain-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp test dir");
        path
    }

    #[test]
    fn generated_state_denylist_matches_p053_roots() {
        for path in [
            ".chainworks/worktrees",
            ".chainworks/backups",
            ".forge-codex-acp",
            ".claude/worktrees",
            ".git/objects",
            "control-plane/target",
            "nested/target",
            "DerivedData",
            ".build",
            "node_modules",
        ] {
            assert!(
                StdDiscoveryFilesystem::should_skip_generated_state_dir(Path::new(path)),
                "{path} should be denied"
            );
        }
    }

    #[test]
    fn generated_state_file_denylist_matches_db_backups() {
        assert!(StdDiscoveryFilesystem::should_skip_generated_state_file(
            Path::new(".chainworks/app.db.backup-20260422")
        ));
        assert!(StdDiscoveryFilesystem::should_skip_generated_state_file(
            Path::new(".chainworks/app.sqlite")
        ));
        assert!(!StdDiscoveryFilesystem::should_skip_generated_state_file(
            Path::new("logs/app.sqlite")
        ));
    }

    #[test]
    fn proposal_053_operation_recorder_observes_bounded_discovery_without_generated_state_reads() {
        let root = temp_test_dir("operation-recorder");
        let logs = root.join("logs");
        let target = root.join("target");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(logs.join("report.json"), br#"{"status":"ok"}"#).unwrap();
        std::fs::write(target.join("build.log"), b"skip").unwrap();
        let recorder = RecordingDiscoveryOperationRecorder::new();

        let discovery = StdDiscoveryFilesystem::discover_bounded_meta_root_artifacts_with_recorder(
            &root, &recorder,
        );

        assert_eq!(discovery.artifact_paths.len(), 1);
        let operations = recorder.operations();
        assert!(operations
            .iter()
            .any(|op| op.kind == DiscoveryOperationKind::BoundedMetaRootDiscovery));
        assert!(operations.iter().any(|op| {
            op.kind == DiscoveryOperationKind::GeneratedStateDirSkipped
                && op
                    .path
                    .as_deref()
                    .is_some_and(|path| path.ends_with("target"))
        }));
        assert!(!operations.iter().any(|op| {
            op.kind == DiscoveryOperationKind::ReadDir
                && op
                    .path
                    .as_deref()
                    .is_some_and(|path| path.ends_with("target"))
        }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proposal_053_gate_uses_discovery_filesystem_trait_fake() {
        let root = Path::new("/fake/meta-root");
        let recorder = RecordingDiscoveryOperationRecorder::new();
        let fake = FakeDiscoveryFilesystem::new()
            .with_expected_path_baseline(
                "/fake/output.json",
                ExpectedPathBaseline {
                    target_path: "/fake/output.json".into(),
                    status: ExpectedPathBaselineStatus::RegularContentCaptured,
                    content: Some(br#"{"ok":true}"#.to_vec()),
                },
            )
            .with_bounded_meta_root_discovery(
                root.to_string_lossy(),
                BoundedMetaRootDiscovery {
                    root_path: root.to_string_lossy().into_owned(),
                    artifact_paths: vec!["/fake/meta-root/report.json".into()],
                    files_visited: 1,
                    total_bytes: 17,
                    latency_ms: Some(0),
                    truncated_by_file_cap: false,
                    truncated_by_file_size: false,
                    truncated_by_total_bytes: false,
                    warnings: Vec::new(),
                },
            );
        let filesystem: &dyn DiscoveryFilesystem = &fake;

        let baseline = filesystem.capture_expected_path_baseline_with_recorder(
            Path::new("/fake/output.json"),
            &recorder,
        );
        let discovery =
            filesystem.discover_bounded_meta_root_artifacts_with_recorder(root, &recorder);

        assert_eq!(
            baseline.status,
            ExpectedPathBaselineStatus::RegularContentCaptured
        );
        assert_eq!(
            discovery.artifact_paths,
            vec!["/fake/meta-root/report.json"]
        );
        let operations = recorder.operations();
        assert!(operations.iter().any(|op| {
            op.kind == DiscoveryOperationKind::CaptureExpectedPathBaseline
                && op.path.as_deref() == Some("/fake/output.json")
        }));
        assert!(operations
            .iter()
            .any(|op| op.kind == DiscoveryOperationKind::BoundedMetaRootDiscovery));
    }

    #[test]
    fn proposal_053_operation_recorder_orders_metadata_before_file_read() {
        let root = temp_test_dir("operation-recorder-metadata");
        let output = root.join("proposal_review.json");
        std::fs::write(&output, br#"{"status":"green"}"#).unwrap();
        let spec = ExpectedOutputSpec {
            output_name: "proposal_review".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: output.to_string_lossy().into_owned(),
            companion_of: None,
            display_label: "Proposal review".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: OutputReusePolicy::MustProduce,
            max_bytes: 10 * 1024 * 1024,
            aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
            authorized_roots: vec![AuthorizedRoot {
                root_class: OutputRootClass::ChainworksMetaRoot,
                root_path: root.to_string_lossy().into_owned(),
            }],
            source_generation_owner: SourceGenerationOwner::Agent,
        };
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "turn-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };
        let recorder = RecordingDiscoveryOperationRecorder::new();

        let metadata =
            StdDiscoveryFilesystem::capture_pre_prompt_expected_output_metadata_with_recorder(
                &spec, &context, &recorder,
            );

        assert_eq!(
            metadata.baseline_status,
            ExpectedPathBaselineStatus::RegularContentCaptured
        );
        let operations = recorder.operations();
        let metadata_index = operations
            .iter()
            .position(|op| op.kind == DiscoveryOperationKind::CapturePrePromptMetadata)
            .expect("metadata operation recorded");
        let read_index = operations
            .iter()
            .position(|op| op.kind == DiscoveryOperationKind::ReadFile)
            .expect("file read operation recorded");
        assert!(
            metadata_index < read_index,
            "metadata operation must be recorded before file read"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn baseline_status_uses_regular_digest_captured_wire_value() {
        let encoded = serde_json::to_string(&ExpectedPathBaselineStatus::RegularContentCaptured)
            .expect("baseline status should serialize");
        assert_eq!(encoded, "\"regular_digest_captured\"");

        let decoded: ExpectedPathBaselineStatus =
            serde_json::from_str("\"regular_content_captured\"")
                .expect("legacy baseline status alias should deserialize");
        assert_eq!(decoded, ExpectedPathBaselineStatus::RegularContentCaptured);
    }

    #[test]
    fn proposal_053_legacy_broad_discovery_caps_and_skips_generated_state() {
        let root = temp_test_dir("legacy-broad-discovery-caps");
        let src = root.join("src");
        let target = root.join("control-plane").join("target");
        let chainworks = root.join(".chainworks");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::create_dir_all(&chainworks).unwrap();
        let small = src.join("result.json");
        let oversized = src.join("large.json");
        let skipped_build_output = target.join("build.log");
        let skipped_backup = chainworks.join("run.db.backup-20260422");
        std::fs::write(&small, br#"{"status":"ok"}"#).unwrap();
        std::fs::write(&oversized, vec![b'x'; 1024 * 1024 + 1]).unwrap();
        std::fs::write(&skipped_build_output, b"skip").unwrap();
        std::fs::write(&skipped_backup, b"skip").unwrap();

        let snapshot = StdDiscoveryFilesystem::snapshot_legacy_broad_discovery(&root);

        assert!(snapshot
            .files
            .iter()
            .any(|path| path.ends_with("src/result.json")));
        assert!(!snapshot
            .files
            .iter()
            .any(|path| path.ends_with("src/large.json")));
        assert!(!snapshot
            .files
            .iter()
            .any(|path| path.ends_with("control-plane/target/build.log")));
        assert!(!snapshot
            .files
            .iter()
            .any(|path| path.ends_with(".chainworks/run.db.backup-20260422")));
        assert!(snapshot.truncated_by_file_size);
        assert_eq!(snapshot.total_bytes, br#"{"status":"ok"}"#.len() as u64);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn proposal_053_bounded_meta_root_discovers_only_small_regular_files() {
        let root = temp_test_dir("bounded-meta-root");
        let logs = root.join("logs");
        let target = root.join("target");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        let report = logs.join("report.json");
        let skipped_build_output = target.join("build.log");
        let oversized = root.join("large.bin");
        std::fs::write(&report, br#"{"status":"ok"}"#).unwrap();
        std::fs::write(&skipped_build_output, b"skip").unwrap();
        std::fs::write(&oversized, vec![b'x'; 1024 * 1024 + 1]).unwrap();

        let discovery = StdDiscoveryFilesystem::discover_bounded_meta_root_artifacts(&root);

        assert!(discovery
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("logs/report.json")));
        assert!(!discovery
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("target/build.log")));
        assert!(!discovery
            .artifact_paths
            .iter()
            .any(|path| path.ends_with("large.bin")));
        assert!(discovery.truncated_by_file_size);
        assert_eq!(discovery.total_bytes, br#"{"status":"ok"}"#.len() as u64);

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn proposal_053_bounded_meta_root_never_follows_symlinks() {
        let root = temp_test_dir("bounded-meta-root-symlink");
        let outside = temp_test_dir("bounded-meta-root-outside");
        let outside_file = outside.join("outside.json");
        let linked_file = root.join("linked.json");
        std::fs::write(&outside_file, br#"{"outside":true}"#).unwrap();
        std::os::unix::fs::symlink(&outside_file, &linked_file).unwrap();

        let discovery = StdDiscoveryFilesystem::discover_bounded_meta_root_artifacts(&root);

        assert!(discovery.artifact_paths.is_empty());
        assert_eq!(discovery.total_bytes, 0);

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn expected_output_spec_serializes_p053_policy_fields() {
        let spec = ExpectedOutputSpec {
            output_name: "implementation_self_assessment".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: "/tmp/run/implementation/self-assessment.json".to_string(),
            companion_of: None,
            display_label: "Implementation self-assessment".to_string(),
            contract_id: Some("implementation_self_assessment_v2".to_string()),
            required: true,
            reuse_policy: OutputReusePolicy::MustProduce,
            max_bytes: 10 * 1024 * 1024,
            aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
            authorized_roots: vec![AuthorizedRoot {
                root_class: OutputRootClass::ChainworksMetaRoot,
                root_path: "/tmp/run".to_string(),
            }],
            source_generation_owner: SourceGenerationOwner::Agent,
        };

        let json = serde_json::to_value(&spec).expect("spec serializes");
        assert_eq!(json["output_role"], "machine");
        assert_eq!(json["reuse_policy"], "must_produce");
        assert_eq!(
            json["authorized_roots"][0]["root_class"],
            "chainworks_meta_root"
        );

        let round_trip: ExpectedOutputSpec =
            serde_json::from_value(json).expect("spec deserializes");
        assert_eq!(round_trip, spec);
    }

    #[test]
    fn pre_prompt_expected_output_metadata_captures_digest_and_root() {
        let root = temp_test_dir("pre-prompt-metadata");
        let output = root.join("proposal_review.json");
        std::fs::write(&output, br#"{"summary":"ok","status":"green"}"#).unwrap();
        let spec = ExpectedOutputSpec {
            output_name: "proposal_review".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: output.to_string_lossy().into_owned(),
            companion_of: None,
            display_label: "Proposal review".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: OutputReusePolicy::AllowUnchangedExisting,
            max_bytes: 10 * 1024 * 1024,
            aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
            authorized_roots: vec![AuthorizedRoot {
                root_class: OutputRootClass::ChainworksMetaRoot,
                root_path: root.to_string_lossy().into_owned(),
            }],
            source_generation_owner: SourceGenerationOwner::Agent,
        };
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 2,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "turn-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };

        let metadata =
            StdDiscoveryFilesystem::capture_pre_prompt_expected_output_metadata(&spec, &context);

        assert!(metadata.existed);
        assert_eq!(
            metadata.baseline_status,
            ExpectedPathBaselineStatus::RegularContentCaptured
        );
        assert_eq!(metadata.root_class, OutputRootClass::ChainworksMetaRoot);
        assert_eq!(metadata.file_type, "regular");
        assert_eq!(
            metadata.size_bytes,
            Some(br#"{"summary":"ok","status":"green"}"#.len() as u64)
        );
        assert!(metadata.content_digest.as_deref().is_some_and(|digest| {
            digest.starts_with("sha256:") && digest.len() == "sha256:".len() + 64
        }));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_pre_prompt_metadata_enforces_spec_count_limit() {
        let root = temp_test_dir("pre-prompt-metadata-spec-count");
        let specs: Vec<_> = (0..3)
            .map(|index| {
                let output = root.join(format!("output-{index}.json"));
                std::fs::write(&output, br#"{"status":"green"}"#).unwrap();
                ExpectedOutputSpec {
                    output_name: format!("output_{index}"),
                    output_role: ExpectedOutputRole::Machine,
                    target_path: output.to_string_lossy().into_owned(),
                    companion_of: None,
                    display_label: format!("Output {index}"),
                    contract_id: None,
                    required: true,
                    reuse_policy: OutputReusePolicy::AllowUnchangedExisting,
                    max_bytes: 10 * 1024 * 1024,
                    aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
                    authorized_roots: vec![AuthorizedRoot {
                        root_class: OutputRootClass::ChainworksMetaRoot,
                        root_path: root.to_string_lossy().into_owned(),
                    }],
                    source_generation_owner: SourceGenerationOwner::Agent,
                }
            })
            .collect();
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "turn-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };

        let metadata =
            StdDiscoveryFilesystem::capture_bounded_pre_prompt_expected_output_metadata_with_bounds(
                &specs,
                &context,
                PrePromptMetadataBounds {
                    max_expected_output_specs: 2,
                    aggregate_digest_read_budget_bytes: 64 * 1024 * 1024,
                    timeout: Duration::from_secs(5),
                },
            );

        assert_eq!(metadata.len(), 3);
        assert_eq!(
            metadata[0].baseline_status,
            ExpectedPathBaselineStatus::RegularContentCaptured
        );
        assert_eq!(
            metadata[1].baseline_status,
            ExpectedPathBaselineStatus::RegularContentCaptured
        );
        assert_eq!(
            metadata[2].baseline_status,
            ExpectedPathBaselineStatus::MetadataTimeout
        );
        assert_eq!(metadata[2].content_digest, None);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_pre_prompt_metadata_enforces_aggregate_digest_budget() {
        let root = temp_test_dir("pre-prompt-metadata-aggregate-budget");
        let first = root.join("first.json");
        let second = root.join("second.json");
        std::fs::write(&first, b"1234").unwrap();
        std::fs::write(&second, b"5678").unwrap();
        let specs: Vec<_> = [("first", &first), ("second", &second)]
            .into_iter()
            .map(|(name, output)| ExpectedOutputSpec {
                output_name: name.to_string(),
                output_role: ExpectedOutputRole::Machine,
                target_path: output.to_string_lossy().into_owned(),
                companion_of: None,
                display_label: name.to_string(),
                contract_id: None,
                required: true,
                reuse_policy: OutputReusePolicy::AllowUnchangedExisting,
                max_bytes: 10 * 1024 * 1024,
                aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
                authorized_roots: vec![AuthorizedRoot {
                    root_class: OutputRootClass::ChainworksMetaRoot,
                    root_path: root.to_string_lossy().into_owned(),
                }],
                source_generation_owner: SourceGenerationOwner::Agent,
            })
            .collect();
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "turn-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };

        let metadata =
            StdDiscoveryFilesystem::capture_bounded_pre_prompt_expected_output_metadata_with_bounds(
                &specs,
                &context,
                PrePromptMetadataBounds {
                    max_expected_output_specs: 200,
                    aggregate_digest_read_budget_bytes: 4,
                    timeout: Duration::from_secs(5),
                },
            );

        assert_eq!(
            metadata[0].baseline_status,
            ExpectedPathBaselineStatus::RegularContentCaptured
        );
        assert!(metadata[0].content_digest.is_some());
        assert_eq!(
            metadata[1].baseline_status,
            ExpectedPathBaselineStatus::Uncertain
        );
        assert_eq!(metadata[1].size_bytes, Some(4));
        assert_eq!(metadata[1].content_digest, None);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bounded_pre_prompt_metadata_marks_remaining_specs_on_timeout() {
        let root = temp_test_dir("pre-prompt-metadata-timeout");
        let output = root.join("proposal_review.json");
        std::fs::write(&output, br#"{"status":"green"}"#).unwrap();
        let spec = ExpectedOutputSpec {
            output_name: "proposal_review".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: output.to_string_lossy().into_owned(),
            companion_of: None,
            display_label: "Proposal review".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: OutputReusePolicy::AllowUnchangedExisting,
            max_bytes: 10 * 1024 * 1024,
            aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
            authorized_roots: vec![AuthorizedRoot {
                root_class: OutputRootClass::ChainworksMetaRoot,
                root_path: root.to_string_lossy().into_owned(),
            }],
            source_generation_owner: SourceGenerationOwner::Agent,
        };
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "turn-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };

        let metadata =
            StdDiscoveryFilesystem::capture_bounded_pre_prompt_expected_output_metadata_with_bounds(
                &[spec],
                &context,
                PrePromptMetadataBounds {
                    max_expected_output_specs: 200,
                    aggregate_digest_read_budget_bytes: 64 * 1024 * 1024,
                    timeout: Duration::ZERO,
                },
            );

        assert_eq!(
            metadata[0].baseline_status,
            ExpectedPathBaselineStatus::MetadataTimeout
        );
        assert_eq!(metadata[0].content_digest, None);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pre_prompt_expected_output_metadata_rejects_unauthorized_root() {
        let root = temp_test_dir("pre-prompt-metadata-authorized");
        let outside = temp_test_dir("pre-prompt-metadata-outside");
        let output = outside.join("proposal_review.json");
        std::fs::write(&output, br#"{"summary":"ok","status":"green"}"#).unwrap();
        let spec = ExpectedOutputSpec {
            output_name: "proposal_review".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: output.to_string_lossy().into_owned(),
            companion_of: None,
            display_label: "Proposal review".to_string(),
            contract_id: None,
            required: true,
            reuse_policy: OutputReusePolicy::AllowUnchangedExisting,
            max_bytes: 10 * 1024 * 1024,
            aggregate_acceptance_cap_bytes: 64 * 1024 * 1024,
            authorized_roots: vec![AuthorizedRoot {
                root_class: OutputRootClass::ChainworksMetaRoot,
                root_path: root.to_string_lossy().into_owned(),
            }],
            source_generation_owner: SourceGenerationOwner::Agent,
        };
        let context = PrePromptExpectedOutputContext {
            agent_execution_id: "agent-exec-1".to_string(),
            stage_execution_id: "stage-exec-1".to_string(),
            attempt_number: 1,
            session_generation_id: "session-1".to_string(),
            prompt_turn_id: "turn-1".to_string(),
            discovery_generation_id: "discovery-1".to_string(),
        };

        let metadata =
            StdDiscoveryFilesystem::capture_pre_prompt_expected_output_metadata(&spec, &context);

        assert_eq!(
            metadata.baseline_status,
            ExpectedPathBaselineStatus::UnauthorizedRoot
        );
        assert_eq!(metadata.content_digest, None);

        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside);
    }

    #[test]
    fn output_discovery_decision_names_rejection_reasons() {
        let decision = OutputDiscoveryDecision {
            output_name: "proposal_review".to_string(),
            output_role: ExpectedOutputRole::Machine,
            target_path: "/tmp/proposal_review.json".to_string(),
            companion_of: None,
            status: OutputDiscoveryStatus::Rejected,
            reason: OutputDiscoveryReason::StaleExpectedOutput,
            provenance: Some(OutputDiscoveryProvenance::ExactPath),
            canonical_path: Some("/tmp/proposal_review.json".to_string()),
            root_class: Some(OutputRootClass::ChainworksMetaRoot),
            baseline_status: Some(ExpectedPathBaselineStatus::RegularContentCaptured),
            size_bytes: Some(18),
            content_digest: Some("sha256:before".to_string()),
            max_bytes_applied: Some(10 * 1024 * 1024),
            aggregate_bytes_after_acceptance: None,
            accepted_payload_ref: None,
            accepted_bytes_sha256: None,
            generated_by: None,
            diagnostics: [("freshness".to_string(), "digest_unchanged".to_string())].into(),
            decision_at: chrono::Utc::now(),
        };

        assert!(!decision.is_accepted());
        let json = serde_json::to_value(&decision).expect("decision serializes");
        assert_eq!(json["status"], "rejected");
        assert_eq!(json["reason"], "stale_expected_output");
        assert_eq!(json["provenance"], "exact_path");
        assert!(json.get("accepted_payload_ref").is_none());
    }
}
