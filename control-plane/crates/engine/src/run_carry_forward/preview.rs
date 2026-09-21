//! Read-only P039 evidence. A PreparedPlan is not a reservation or execution grant.

mod generations;
mod historical;
mod legacy;
mod provenance;
mod selection;
mod target_verification;
mod tombstones;
mod wire;
mod witness;

use anyhow::{ensure, Context};
use domain::{
    ids::RunId,
    run::{DeliveryConfiguration, Run, RunStatus},
    run_carry_forward::{canonical_digest, ContentDigest, EntryRole},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;
use workflow::{carry_forward::ContinuationTargetV1, plan::RunPlan};

use super::{
    budget::Deadline,
    safe_files::{hash_file, identity, validate_relative, SafeRoot},
    InventoryLimits, PreviewOptions, WorkspacePlanner, WorkspacePreview,
};

pub use selection::CanonicalSourceInput;
pub use target_verification::verify_current_target;
pub use wire::{wire_manifest_entry, wire_page};
pub use witness::{source_semantic_witness, SourceWitness};

const REQUEST_LIMIT: usize = 1024 * 1024;
const SUMMARY_LIMIT: usize = 64 * 1024;
const CURSOR_LIMIT: usize = 4096;
const PROFILE: &str = "implementation_restart_v1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputReference {
    Artifact { id: Uuid },
    CarriedInput { id: Uuid },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewTarget {
    pub workflow_yaml_path: String,
    pub agent_catalog_yaml_path: String,
    pub expected_workflow_snapshot_hash: Option<ContentDigest>,
    pub expected_catalog_snapshot_hash: Option<ContentDigest>,
    pub delivery_configuration_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewSelection {
    pub proposal_input: InputReference,
    pub reference_inputs: Vec<InputReference>,
    pub include_dirty_work: bool,
    pub excluded_workspace_paths: BTreeMap<String, String>,
}

/// Internal input, not a replacement for the strict boundary request decoder.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewInput {
    pub source_run_id: RunId,
    pub target: PreviewTarget,
    pub selection: PreviewSelection,
    pub profile: String,
    pub cursor: Option<String>,
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HoldCode {
    SourceNotBlocked,
    SourceBusy,
    SourceContinued,
    ContinuationInProgress,
    UnsupportedFrontier,
    TargetProfileInvalid,
    TargetPolicyIncompatible,
    SourceChanged,
    ArtifactProvenanceInvalid,
    UnsafePath,
    HeadlessEffectUnresolved,
    EffectOutcomeUnknown,
    ContinuationBudgetExceeded,
    StorageUnavailable,
}

impl HoldCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceNotBlocked => "source_not_blocked",
            Self::SourceBusy => "source_busy",
            Self::SourceContinued => "source_continued",
            Self::ContinuationInProgress => "continuation_in_progress",
            Self::UnsupportedFrontier => "unsupported_frontier",
            Self::TargetProfileInvalid => "target_profile_invalid",
            Self::TargetPolicyIncompatible => "target_policy_incompatible",
            Self::SourceChanged => "source_changed",
            Self::ArtifactProvenanceInvalid => "artifact_provenance_invalid",
            Self::UnsafePath => "unsafe_path",
            Self::HeadlessEffectUnresolved => "headless_effect_unresolved",
            Self::EffectOutcomeUnknown => "effect_outcome_unknown",
            Self::ContinuationBudgetExceeded => "continuation_budget_exceeded",
            Self::StorageUnavailable => "storage_unavailable",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewHold {
    pub code: HoldCode,
}

#[derive(Debug, thiserror::Error)]
pub enum PreviewError {
    #[error("invalid preview input: {0}")]
    InvalidInput(&'static str),
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStale {
    pub schema_version: &'static str,
    pub code: &'static str,
    pub next_action: &'static str,
}

#[derive(Debug)]
pub enum PreviewOutcome {
    Page {
        page: PreviewPage,
        prepared: PreparedPlan,
    },
    Stale(PreviewStale),
    Held {
        holds: Vec<PreviewHold>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryKey {
    pub namespace: String,
    pub relative_path: String,
    pub stable_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanEntry {
    pub key: EntryKey,
    pub source_artifact_id: Option<Uuid>,
    pub logical_name: Option<String>,
    pub role: EntryRole,
    pub sha256: Option<ContentDigest>,
    pub bytes: u64,
    pub mode: u32,
    pub source_contract: Option<String>,
    pub source_schema_version: Option<String>,
    pub target_logical_name: Option<String>,
    pub reason: String,
    pub historical_approval_relation: Option<Value>,
    #[serde(deserialize_with = "wire::deserialize_workspace_entry")]
    pub workspace_entry: Option<super::WorkspaceEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanLimits {
    pub max_entries: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_references: usize,
    pub max_request_bytes: usize,
    pub max_summary_bytes: usize,
    pub default_page: usize,
    pub max_page: usize,
    pub max_cursor_bytes: usize,
    pub deadline_seconds: u64,
}

impl Default for PlanLimits {
    fn default() -> Self {
        let limits = InventoryLimits::default();
        Self {
            max_entries: limits.max_entries,
            max_total_bytes: limits.max_total_bytes,
            max_file_bytes: limits.max_file_bytes,
            max_references: 128,
            max_request_bytes: REQUEST_LIMIT,
            max_summary_bytes: SUMMARY_LIMIT,
            default_page: 100,
            max_page: 500,
            max_cursor_bytes: CURSOR_LIMIT,
            deadline_seconds: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetSummary {
    pub workflow_yaml_path: String,
    pub agent_catalog_yaml_path: String,
    pub workflow_snapshot_hash: ContentDigest,
    pub catalog_snapshot_hash: ContentDigest,
    pub successor_workflow_snapshot_hash: ContentDigest,
    pub successor_catalog_snapshot_hash: ContentDigest,
    pub compiled_plan_sha256: ContentDigest,
    pub definition_files_sha256: ContentDigest,
    pub delivery_configuration: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanSummary {
    pub schema_version: &'static str,
    pub source_run_id: RunId,
    pub source_witness: SourceWitness,
    pub target: TargetSummary,
    pub profile: String,
    pub selection_sha256: ContentDigest,
    pub workspace_snapshot: Value,
    pub capability_delta: Value,
    pub holds: Vec<PreviewHold>,
    pub pending_checks: Vec<String>,
    pub limits: PlanLimits,
}

#[derive(Debug, Serialize)]
pub struct CarryForwardPlan {
    #[serde(flatten)]
    pub summary: PlanSummary,
    pub selection: PreviewSelection,
    pub entries: Vec<PlanEntry>,
    pub plan_sha256: ContentDigest,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewPage {
    pub schema_version: &'static str,
    pub plan_summary: PlanSummary,
    pub entries: Vec<PlanEntry>,
    pub entry_count: usize,
    pub plan_sha256: ContentDigest,
    pub next_cursor: Option<String>,
}

/// No Deserialize or public constructor: consumers cannot forge a verified filesystem plan.
#[derive(Debug)]
pub struct PreparedPlan {
    plan: CarryForwardPlan,
    workspace: WorkspacePreview,
    target: ContinuationTargetV1,
    inputs: Vec<CanonicalSourceInput>,
}

impl PreparedPlan {
    pub fn plan(&self) -> &CarryForwardPlan {
        &self.plan
    }
    pub fn workspace(&self) -> &WorkspacePreview {
        &self.workspace
    }
    pub fn target(&self) -> &ContinuationTargetV1 {
        &self.target
    }
    pub fn inputs(&self) -> &[CanonicalSourceInput] {
        &self.inputs
    }
    pub fn source_witness(&self) -> &SourceWitness {
        &self.plan.summary.source_witness
    }
    pub fn into_parts(
        self,
    ) -> (
        CarryForwardPlan,
        WorkspacePreview,
        ContinuationTargetV1,
        Vec<CanonicalSourceInput>,
    ) {
        (self.plan, self.workspace, self.target, self.inputs)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    version: u8,
    request: ContentDigest,
    plan: ContentDigest,
    last: EntryKey,
}

fn stale() -> PreviewOutcome {
    PreviewOutcome::Stale(PreviewStale {
        schema_version: "run_carry_forward_preview_stale_v1",
        code: "preview_stale",
        next_action: "refresh_preview",
    })
}

/// Caller must authenticate/source-authorize before calling, and keep external writers quiescent.
/// This function performs only bounded reads; it never opens a writer transaction or a live provider.
pub async fn preview(
    pool: &SqlitePool,
    mut input: PreviewInput,
) -> Result<PreviewOutcome, PreviewError> {
    let encoded = serde_json::to_vec(&input).map_err(|_| PreviewError::InvalidInput("request"))?;
    if encoded.len() > REQUEST_LIMIT {
        return Err(PreviewError::InvalidInput("request exceeds 1 MiB"));
    }
    let limit = usize::from(input.limit.unwrap_or(100));
    if !(1..=500).contains(&limit) {
        return Err(PreviewError::InvalidInput("limit must be 1..=500"));
    }
    let cursor: Option<Cursor> = match input.cursor.as_deref() {
        Some(text) if text.len() <= CURSOR_LIMIT => {
            let cursor: Cursor =
                serde_json::from_str(text).map_err(|_| PreviewError::InvalidInput("cursor"))?;
            if cursor.version != 1
                || validate_relative(&cursor.last.relative_path).is_err()
                || cursor.last.stable_id.is_empty()
            {
                return Err(PreviewError::InvalidInput("cursor"));
            }
            Some(cursor)
        }
        Some(_) => return Err(PreviewError::InvalidInput("cursor exceeds 4 KiB")),
        None => None,
    };
    if input.selection.reference_inputs.len() > 128
        || input.selection.excluded_workspace_paths.len() > 128
    {
        return Err(PreviewError::InvalidInput("selection bounds"));
    }
    let unique: BTreeSet<_> = input.selection.reference_inputs.iter().collect();
    if unique.len() != input.selection.reference_inputs.len()
        || unique.contains(&input.selection.proposal_input)
    {
        return Err(PreviewError::InvalidInput("duplicate selection"));
    }
    input.selection.reference_inputs.sort();
    input.cursor = None;
    input.limit = None;
    let request_digest =
        canonical_digest(&input).map_err(|_| PreviewError::InvalidInput("request digest"))?;
    if cursor.as_ref().is_some_and(|c| c.request != request_digest) {
        return Ok(stale());
    }
    let deadline = Deadline::after(Duration::from_secs(120));
    let result = tokio::time::timeout(Duration::from_secs(120), build(pool, input, deadline)).await;
    let prepared = match result {
        Ok(Ok(plan)) => plan,
        Ok(Err(error)) => {
            let code = classify(&error);
            if cursor.is_some()
                && code != HoldCode::StorageUnavailable
                && code != HoldCode::ContinuationBudgetExceeded
            {
                return Ok(stale());
            }
            return Ok(PreviewOutcome::Held {
                holds: vec![PreviewHold { code }],
            });
        }
        Err(_) => {
            return Ok(PreviewOutcome::Held {
                holds: vec![PreviewHold {
                    code: HoldCode::ContinuationBudgetExceeded,
                }],
            })
        }
    };
    if cursor
        .as_ref()
        .is_some_and(|c| c.plan != prepared.plan.plan_sha256)
    {
        return Ok(stale());
    }
    let start = match cursor {
        Some(cursor) => {
            prepared
                .plan
                .entries
                .binary_search_by(|entry| entry.key.cmp(&cursor.last))
                .map_err(|_| PreviewError::InvalidInput("cursor entry"))?
                + 1
        }
        None => 0,
    };
    let end = (start + limit).min(prepared.plan.entries.len());
    let entries = prepared.plan.entries[start..end].to_vec();
    let next_cursor = if end < prepared.plan.entries.len() {
        let cursor = serde_json::to_string(&Cursor {
            version: 1,
            request: request_digest,
            plan: prepared.plan.plan_sha256.clone(),
            last: prepared.plan.entries[end - 1].key.clone(),
        })
        .map_err(|_| PreviewError::InvalidInput("cursor serialization"))?;
        if cursor.len() > CURSOR_LIMIT {
            return Ok(PreviewOutcome::Held {
                holds: vec![PreviewHold {
                    code: HoldCode::ContinuationBudgetExceeded,
                }],
            });
        }
        Some(cursor)
    } else {
        None
    };
    let page = PreviewPage {
        schema_version: "run_carry_forward_preview_page_v1",
        plan_summary: prepared.plan.summary.clone(),
        entries,
        entry_count: prepared.plan.entries.len(),
        plan_sha256: prepared.plan.plan_sha256.clone(),
        next_cursor,
    };
    Ok(PreviewOutcome::Page { page, prepared })
}

fn classify(error: &anyhow::Error) -> HoldCode {
    let message = format!("{error:#}");
    for code in [
        HoldCode::SourceNotBlocked,
        HoldCode::SourceBusy,
        HoldCode::SourceContinued,
        HoldCode::ContinuationInProgress,
        HoldCode::UnsupportedFrontier,
        HoldCode::TargetProfileInvalid,
        HoldCode::TargetPolicyIncompatible,
        HoldCode::SourceChanged,
        HoldCode::ArtifactProvenanceInvalid,
        HoldCode::UnsafePath,
        HoldCode::HeadlessEffectUnresolved,
        HoldCode::EffectOutcomeUnknown,
        HoldCode::ContinuationBudgetExceeded,
    ] {
        if message
            .split(": ")
            .any(|part| part == code.as_str() || part.starts_with(&format!("{}:", code.as_str())))
        {
            return code;
        }
    }
    HoldCode::StorageUnavailable
}

async fn build(
    pool: &SqlitePool,
    input: PreviewInput,
    deadline: Deadline,
) -> anyhow::Result<PreparedPlan> {
    ensure!(input.profile == PROFILE, "target_profile_invalid");
    ensure!(
        input.selection.include_dirty_work,
        "unsafe_path: dirty work cannot be discarded"
    );
    // A directory prefix is not evidence that its contents are machine-owned.
    // This bounded adapter recognizes only the exact macOS metadata filename;
    // other requested exclusions hold until a generated-file ownership adapter exists.
    ensure!(
        input
            .selection
            .excluded_workspace_paths
            .keys()
            .all(|p| p == ".DS_Store"),
        "unsafe_path: unproven generated ownership"
    );
    let observation = witness::observe(pool, input.source_run_id).await?;
    let source = db::repos::runs::find_by_id(pool, input.source_run_id)
        .await?
        .context("source_not_blocked")?;
    ensure!(source.status == RunStatus::Blocked, "source_not_blocked");
    let semantic_before = observation.semantic_sha256.clone();
    witness::check_admission(pool, &source).await?;
    let roots = Roots::derive(&source)?;
    let source_plan = legacy::compile_source(&source)?;
    let (stage_id, cursor) =
        legacy::validate_frontier(pool, &source, &source_plan, &observation).await?;
    let (target, target_summary) = compile_target(&input.target, &source, &roots, deadline)?;
    let inputs = selection::resolve(
        pool,
        &source,
        &source_plan,
        &roots,
        &input.selection,
        deadline,
    )
    .await?;
    let options = PreviewOptions {
        limits: InventoryLimits::default(),
        exclusions: input.selection.excluded_workspace_paths.clone(),
    };
    let workspace =
        WorkspacePlanner::preview_with_deadline(&roots.checkout, options.clone(), deadline).await?;
    let repository_git = super::git::text(
        &roots.repository,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        deadline,
    )
    .await?;
    ensure!(
        std::fs::canonicalize(repository_git)? == workspace.git_common_dir,
        "unsafe_path: different repository identity"
    );
    let wire_source = wire::source_witness(
        &source,
        &observation,
        stage_id,
        cursor,
        &workspace,
        &roots,
        deadline,
    )
    .await?;
    let snapshot =
        serde_json::to_value(wire::workspace_summary(&source, &workspace, deadline).await?)?;
    let mut entries: Vec<_> = workspace
        .entries
        .iter()
        .map(|entry| PlanEntry {
            key: EntryKey {
                namespace: "workspace".into(),
                relative_path: entry.path.clone(),
                stable_id: entry.path.clone(),
            },
            source_artifact_id: None,
            logical_name: None,
            role: if entry.excluded {
                EntryRole::Excluded
            } else {
                EntryRole::PreserveOnly
            },
            sha256: entry.sha256.clone(),
            bytes: entry.size,
            mode: entry.mode,
            source_contract: None,
            source_schema_version: None,
            target_logical_name: None,
            reason: if entry.excluded {
                "excluded_machine_metadata"
            } else {
                "preserve_workspace"
            }
            .into(),
            historical_approval_relation: None,
            workspace_entry: Some(entry.clone()),
        })
        .collect();
    tombstones::bind_preimages(&workspace, &mut entries, deadline).await?;
    entries.extend(inputs.iter().map(CanonicalSourceInput::entry));
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    ensure!(
        entries.len() <= 50_000,
        "continuation_budget_exceeded: entries"
    );
    let bytes = entries
        .iter()
        .try_fold(0u64, |n, e| n.checked_add(e.bytes))
        .context("continuation_budget_exceeded")?;
    let preserved = bytes
        + workspace.index.len() as u64
        + workspace.staged.len() as u64
        + workspace.unstaged.len() as u64;
    ensure!(
        preserved <= InventoryLimits::default().max_total_bytes,
        "continuation_budget_exceeded: preservation"
    );
    // Re-observe all content, target definitions and DB semantics before publishing any page.
    selection::verify_inputs(&inputs, deadline)?;
    let workspace_after =
        WorkspacePlanner::preview_with_deadline(&roots.checkout, options, deadline).await?;
    ensure!(
        workspace.digest() == workspace_after.digest(),
        "source_changed: workspace"
    );
    let (_, target_after) = compile_target(&input.target, &source, &roots, deadline)?;
    ensure!(
        canonical_digest(&target_summary)? == canonical_digest(&target_after)?,
        "source_changed: target"
    );
    ensure!(
        semantic_before == source_semantic_witness(pool, source.id).await?,
        "source_changed: database"
    );
    roots.verify()?;
    deadline.check()?;
    let source_witness = SourceWitness {
        schema_version: "run_carry_forward_source_witness_v1",
        source_run_id: source.id,
        workflow_snapshot_hash: snapshot_hash(
            source
                .workflow_snapshot_hash
                .as_deref()
                .context("unsupported_frontier")?,
        )?,
        catalog_snapshot_hash: snapshot_hash(
            source
                .catalog_snapshot_hash
                .as_deref()
                .context("unsupported_frontier")?,
        )?,
        current_state: source
            .current_state
            .clone()
            .context("unsupported_frontier")?,
        semantic_sha256: semantic_before,
        workspace_sha256: workspace.digest().clone(),
        selected_inputs_sha256: canonical_digest(&inputs)?,
        directory_identities: roots.identities.clone(),
        wire: wire_source,
    };
    let summary = PlanSummary {
        schema_version: "run_carry_forward_plan_v1",
        source_run_id: source.id,
        source_witness,
        target: target_summary,
        profile: PROFILE.into(),
        selection_sha256: canonical_digest(&input.selection)?,
        workspace_snapshot: snapshot,
        capability_delta: serde_json::to_value(wire::capability_delta(
            &source_plan,
            &target.plan,
        )?)?,
        holds: vec![],
        pending_checks: vec![
            "worktree_base_writable".into(),
            "activation_delivery_preflight".into(),
            "activation_rollout_enforcement".into(),
            "reservation_and_activation_ownership_recheck".into(),
        ],
        limits: PlanLimits::default(),
    };
    ensure!(
        serde_json::to_vec(&summary)?.len() <= SUMMARY_LIMIT,
        "continuation_budget_exceeded: summary"
    );
    let mut body = serde_json::to_value(&summary)?;
    body["selection"] = serde_json::to_value(&input.selection)?;
    body["entries"] = serde_json::to_value(&entries)?;
    let digest = canonical_digest(&body)?;
    let prepared = PreparedPlan {
        plan: CarryForwardPlan {
            summary,
            selection: input.selection,
            entries,
            plan_sha256: digest,
        },
        workspace,
        target,
        inputs,
    };
    wire::validate_plan(&prepared)?;
    Ok(prepared)
}

struct Roots {
    repository: PathBuf,
    checkout: PathBuf,
    metadata: PathBuf,
    held: Vec<(PathBuf, SafeRoot)>,
    identities: BTreeMap<String, (u64, u64)>,
}

impl Roots {
    fn derive(source: &Run) -> anyhow::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let repository = PathBuf::from(&source.workspace_root);
        let checkout = PathBuf::from(
            source
                .worktree_root
                .as_deref()
                .context("unsupported_frontier: no canonical worktree")?,
        );
        let metadata = match source.chainworks_meta_root.as_deref() {
            Some(path) if Path::new(path).is_absolute() => PathBuf::from(path),
            Some(path) => {
                validate_relative(path)?;
                repository.join(path)
            }
            None => PathBuf::from(&source.artifact_root),
        };
        ensure!(
            metadata != repository
                && metadata != checkout
                && !repository.starts_with(&metadata)
                && !checkout.starts_with(&metadata),
            "unsafe_path: metadata overlap"
        );
        let mut held = Vec::new();
        let mut identities = BTreeMap::new();
        for (name, path) in [
            ("repository", &repository),
            ("checkout", &checkout),
            ("metadata", &metadata),
        ] {
            let root = SafeRoot::open(path).context("unsafe_path: canonical root")?;
            let info = root.file.metadata()?;
            identities.insert(name.into(), (info.dev(), info.ino()));
            held.push((path.clone(), root));
        }
        Ok(Self {
            repository,
            checkout,
            metadata,
            held,
            identities,
        })
    }
    fn verify(&self) -> anyhow::Result<()> {
        for (path, root) in &self.held {
            root.verify_path(path)?;
        }
        Ok(())
    }
}

fn snapshot_hash(value: &str) -> anyhow::Result<ContentDigest> {
    // The workflow compiler and canonical artifacts store bare SHA-256; wire digests are prefixed.
    ContentDigest::from_sha256_hex(value)
        .context("artifact_provenance_invalid: bare snapshot hash required")
}

fn relative_definition(repository: &Path, path: &str, prefix: &str) -> anyhow::Result<String> {
    let path = Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(repository)
            .context("unsafe_path: definition root")?
    } else {
        path
    };
    let text = relative
        .to_str()
        .context("unsafe_path: definition encoding")?;
    validate_relative(text)?;
    ensure!(
        text.starts_with(prefix) && text.len() <= 4096,
        "unsafe_path: definition root"
    );
    Ok(text.into())
}

fn read_regular(
    root: &SafeRoot,
    relative: &str,
    max: u64,
    deadline: Deadline,
) -> anyhow::Result<Vec<u8>> {
    let mut file = root
        .open_file(relative)
        .context("unsafe_path: regular nofollow file required")?;
    let before = file.metadata()?;
    ensure!(before.len() <= max, "continuation_budget_exceeded: file");
    let mut bytes = Vec::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        deadline.check()?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        ensure!(
            (bytes.len() + count) as u64 <= max,
            "continuation_budget_exceeded: file"
        );
        bytes.extend_from_slice(&buffer[..count]);
    }
    ensure!(
        identity(&before) == identity(&file.metadata()?),
        "source_changed: file during read"
    );
    let now = root
        .inspect(relative)?
        .context("source_changed: missing file")?;
    use std::os::unix::fs::MetadataExt;
    ensure!(
        (now.dev, now.ino, now.size, now.mode)
            == (before.dev(), before.ino(), before.len(), before.mode()),
        "source_changed: file replaced"
    );
    Ok(bytes)
}

fn compile_target(
    request: &PreviewTarget,
    source: &Run,
    roots: &Roots,
    deadline: Deadline,
) -> anyhow::Result<(ContinuationTargetV1, TargetSummary)> {
    let workflow_path = relative_definition(
        &roots.repository,
        &request.workflow_yaml_path,
        "examples/workflows/",
    )?;
    let catalog_path = relative_definition(
        &roots.repository,
        &request.agent_catalog_yaml_path,
        "examples/agents/",
    )?;
    let root = SafeRoot::open(&roots.repository)?;
    let before = definition_witness(&root, &workflow_path, &catalog_path, deadline)?;
    let current = workflow::compiler::compile_for_new_run_v1(
        roots
            .repository
            .join(&workflow_path)
            .to_str()
            .context("unsafe_path")?,
        roots
            .repository
            .join(&catalog_path)
            .to_str()
            .context("unsafe_path")?,
    )
    .context("target_profile_invalid")?
    .into_plan();
    let workflow_hash = snapshot_hash(&current.workflow_snapshot_hash)?;
    let catalog_hash = snapshot_hash(&current.catalog_snapshot_hash)?;
    let target = workflow::carry_forward::freeze_continuation_target_v1(
        current,
        roots
            .repository
            .join(&catalog_path)
            .to_str()
            .context("unsafe_path")?,
    )
    .context("target_profile_invalid")?;
    deadline.check()?;
    ensure!(
        before == definition_witness(&root, &workflow_path, &catalog_path, deadline)?,
        "source_changed: definitions"
    );
    root.verify_path(&roots.repository)?;
    ensure!(
        request
            .expected_workflow_snapshot_hash
            .as_ref()
            .is_none_or(|h| h == &workflow_hash)
            && request
                .expected_catalog_snapshot_hash
                .as_ref()
                .is_none_or(|h| h == &catalog_hash),
        "source_changed: target hash"
    );
    let delivery: DeliveryConfiguration =
        serde_json::from_str(&request.delivery_configuration_json)
            .context("target_policy_incompatible: delivery")?;
    let old: DeliveryConfiguration = serde_json::from_str(
        source
            .delivery_configuration_json
            .as_deref()
            .context("target_policy_incompatible: missing delivery")?,
    )
    .context("target_policy_incompatible: source delivery")?;
    ensure!(
        delivery.repo_root == source.workspace_root
            && old.repo_root == delivery.repo_root
            && !delivery.repo_identifier.is_empty()
            && old.repo_identifier == delivery.repo_identifier
            && delivery.base_branch == old.base_branch
            && delivery.worktree_base_path == old.worktree_base_path
            && delivery.target_branch == old.target_branch
            && delivery.release_target_id == old.release_target_id
            && delivery.release_mode == old.release_mode,
        "target_policy_incompatible: delivery identity or policy"
    );
    // Destination authority stays server-owned. Even otherwise ignored JSON keys cannot override it.
    let authored: Value = serde_json::from_str(&request.delivery_configuration_json)?;
    ensure!(
        authored.as_object().is_some_and(|m| m.keys().all(|k| [
            "repo_identifier",
            "repo_root",
            "base_branch",
            "worktree_base_path",
            "target_branch",
            "release_target_id",
            "release_mode"
        ]
        .contains(&k.as_str()))),
        "target_policy_incompatible: unknown delivery field"
    );
    let summary = TargetSummary {
        workflow_yaml_path: workflow_path,
        agent_catalog_yaml_path: catalog_path,
        workflow_snapshot_hash: workflow_hash,
        catalog_snapshot_hash: catalog_hash,
        compiled_plan_sha256: canonical_digest(&target)?,
        successor_workflow_snapshot_hash: snapshot_hash(&target.plan.workflow_snapshot_hash)?,
        successor_catalog_snapshot_hash: snapshot_hash(&target.plan.catalog_snapshot_hash)?,
        definition_files_sha256: before,
        delivery_configuration: serde_json::to_value(delivery)?,
    };
    Ok((target, summary))
}

fn definition_witness(
    root: &SafeRoot,
    workflow: &str,
    catalog: &str,
    deadline: Deadline,
) -> anyhow::Result<ContentDigest> {
    let workflow_bytes = read_regular(root, workflow, 4 * 1024 * 1024, deadline)?;
    let catalog_bytes = read_regular(root, catalog, 8 * 1024 * 1024, deadline)?;
    let catalog_value: Value =
        serde_yaml::from_slice(&catalog_bytes).context("target_profile_invalid")?;
    let base = Path::new(catalog).parent().context("unsafe_path")?;
    let mut files = BTreeMap::from([
        (workflow.to_string(), ContentDigest::of(&workflow_bytes)),
        (catalog.to_string(), ContentDigest::of(&catalog_bytes)),
    ]);
    let policy = base.join("codex-model-variant-matrix.v1.json");
    let policy = policy.to_str().context("unsafe_path")?;
    files.insert(
        policy.into(),
        ContentDigest::of(&read_regular(root, policy, 1024 * 1024, deadline)?),
    );
    if let Some(skills) = catalog_value.get("skills").and_then(Value::as_object) {
        for skill in skills.values().filter(|s| s["type"] == "external_skill") {
            let path = skill["path"]
                .as_str()
                .context("target_profile_invalid: skill path")?;
            validate_relative(path)?;
            let skill_path = base.join(path);
            let absolute = root_path_child(root, &skill_path, deadline)?;
            for (path, digest) in absolute {
                files.insert(path, digest);
            }
        }
    }
    deadline.check()?;
    canonical_digest(&files).map_err(Into::into)
}

fn root_path_child(
    root: &SafeRoot,
    path: &Path,
    deadline: Deadline,
) -> anyhow::Result<Vec<(String, ContentDigest)>> {
    // Skill loader embeds bounded SKILL.md bytes; reject undeclared resource directories
    // below using its same entry file, without executing any resource.
    let path = path.join("SKILL.md");
    let path = path.to_str().context("unsafe_path: skill encoding")?;
    let digest = hash_file(
        root.open_file(path).context("unsafe_path: skill")?,
        65536,
        deadline,
    )?;
    Ok(vec![(path.into(), digest)])
}
