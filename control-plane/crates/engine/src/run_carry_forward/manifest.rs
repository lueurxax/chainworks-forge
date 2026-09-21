//! Immutable, private preparation evidence. Readback is not activation authority:
//! the service must still check the current fence, source witness and admission.

use anyhow::{ensure, Context, Result};
use db::repos::{run_continuation_inputs::PreparedInput, run_continuations::Continuation};
use domain::{
    ids::{IdeaId, RunId},
    run::Run,
    run_carry_forward::{canonical_digest, ContentDigest, EntryRole},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;
use workflow::carry_forward::ContinuationTargetV1;

use super::{
    budget::Deadline,
    finalize::{build_successor, seed_path},
    git,
    safe_files::{identity, validate_relative, SafeRoot},
    PreparationContext, PreviewOptions, WorkspacePlanner,
};

const MAX_FILE: u64 = 256 * 1024 * 1024;
const MAX_TOTAL: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestV1 {
    pub schema_version: String,
    pub operation_id: Uuid,
    pub source_run_id: RunId,
    pub reserved_successor_run_id: RunId,
    pub idea_id: IdeaId,
    pub plan_sha256: ContentDigest,
    /// Full composite witness; the DB reservation separately pins semantic_sha256.
    pub source_witness_sha256: ContentDigest,
    pub source_semantic_sha256: ContentDigest,
    /// Full original plan, including every inventory entry and exclusion.
    pub plan: Value,
    pub target: ContinuationTargetV1,
    pub target_sha256: ContentDigest,
    pub original_workflow_sha256: ContentDigest,
    pub original_catalog_sha256: ContentDigest,
    pub definition_files_sha256: ContentDigest,
    pub capability_delta_sha256: ContentDigest,
    /// Hashes whole historical reference evidence, without adjudicating findings.
    pub unresolved_findings_sha256: ContentDigest,
    pub preserved_bytes: u64,
    pub workspace: WorkspaceMaterialV1,
    pub files: Vec<FileBindingV1>,
    /// Pending descriptors only: activation installs these in its transaction.
    pub inputs: Vec<PreparedInput>,
    pub input_provenance: Vec<InputProvenanceV1>,
    /// Pending blueprint only; no run or approval has been created.
    pub successor: Run,
    pub verified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceMaterialV1 {
    pub operation_root: PathBuf,
    pub source_root: PathBuf,
    pub checkout_root: PathBuf,
    pub preservation_root: PathBuf,
    pub metadata_root: PathBuf,
    pub head: String,
    pub pin_ref: String,
    /// Prepared clean index only; not a restriction on legitimate successor work.
    pub checkout_index_sha256: ContentDigest,
    pub source_witness_sha256: ContentDigest,
    pub preservation_manifest_sha256: ContentDigest,
    pub directory_identities: BTreeMap<String, (u64, u64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileBindingV1 {
    /// Relative to workspace.operation_root, never a source pointer.
    pub relative_path: String,
    pub sha256: ContentDigest,
    pub bytes: u64,
}

impl FileBindingV1 {
    pub(super) fn new(path: &str, bytes: &[u8]) -> Self {
        Self {
            relative_path: path.into(),
            sha256: ContentDigest::of(bytes),
            bytes: bytes.len() as u64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputProvenanceV1 {
    pub input_id: Uuid,
    /// Historical locator only; never used as authority or opened on restart.
    pub source_root: PathBuf,
    pub source_relative_path: String,
    pub logical_name: String,
    pub source_contract: String,
    pub original_run_id: RunId,
    pub ancestor_manifest_sha256: Vec<ContentDigest>,
    pub content_sha256: ContentDigest,
    pub bytes: u64,
    pub source_mode: u32,
    pub installed_mode: u32,
    pub mandatory: bool,
}

/// Read-only, 120-second verification with blocking I/O isolation. Only the
/// canonical operation's manifest_ref/hash are trusted; no path-only overload.
/// No source repair, no effect replay, no DB writes or reconstructed worker jobs.
pub async fn read_verified_manifest(operation: &Continuation) -> Result<ManifestV1> {
    read(operation, true).await
}

/// Immutable evidence reader for fresh successor approval binding. Frozen files,
/// source preservation and historical references remain hash-pinned. Successor
/// product files, proposal seed and new outputs may legitimately evolve; this
/// function does not attest their current contents or authorize activation.
pub async fn read_manifest(operation: &Continuation) -> Result<ManifestV1> {
    read(operation, false).await
}

async fn read(operation: &Continuation, verify_successor: bool) -> Result<ManifestV1> {
    let operation = operation.clone();
    let runtime = tokio::runtime::Handle::current();
    let deadline = Deadline::after(Duration::from_secs(120));
    tokio::task::spawn_blocking(move || {
        runtime.block_on(async move {
            let path = Path::new(
                operation
                    .manifest_ref
                    .as_deref()
                    .context("prepared_material_changed: missing manifest")?,
            );
            let expected = db_digest(
                operation
                    .manifest_sha256
                    .as_deref()
                    .context("prepared_material_changed: missing manifest digest")?,
            )?;
            ensure!(
                path.file_name().is_some_and(|n| n == "manifest.json"),
                "unsafe_path: manifest name"
            );
            let root_path = path.parent().context("unsafe_path: manifest root")?;
            ensure!(
                root_path.file_name().and_then(|n| n.to_str())
                    == Some(operation.operation_id.as_str()),
                "unsafe_path: operation root"
            );
            let root = SafeRoot::open(root_path)?;
            let (actual, bytes) =
                read_file(&root, root_path, "manifest.json", true, deadline, None).await?;
            ensure!(
                actual == expected,
                "prepared_material_changed: manifest digest"
            );
            let manifest: ManifestV1 = serde_json::from_slice(&bytes)
                .context("prepared_material_changed: manifest schema")?;
            ensure!(
                manifest.workspace.operation_root == root_path,
                "unsafe_path: manifest ownership"
            );
            verify(&manifest, &operation, deadline, None, verify_successor).await?;
            // Close the read/verification window over the authoritative manifest.
            let (again, _) =
                read_file(&root, root_path, "manifest.json", false, deadline, None).await?;
            ensure!(
                again == expected,
                "prepared_material_changed: manifest digest"
            );
            Ok(manifest)
        })
    })
    .await
    .context("manifest verification task failed")?
}

pub(super) fn db_digest(raw: &str) -> Result<ContentDigest> {
    Ok(ContentDigest::from_sha256_hex(raw)?)
}

pub(super) fn bounded_preservation_bytes(
    entries: impl IntoIterator<Item = u64>,
    patches: impl IntoIterator<Item = u64>,
) -> Result<u64> {
    let mut total = 0u64;
    for bytes in entries.into_iter().chain(patches) {
        ensure!(bytes <= MAX_FILE, "continuation_budget_exceeded: file");
        total = total
            .checked_add(bytes)
            .context("continuation_budget_exceeded: total overflow")?;
        ensure!(
            total <= MAX_TOTAL,
            "continuation_budget_exceeded: preservation including inputs"
        );
    }
    Ok(total)
}

pub(super) fn findings_digest(
    inputs: &[PreparedInput],
    provenance: &[InputProvenanceV1],
) -> Result<ContentDigest> {
    let evidence: Vec<_> = inputs
        .iter()
        .zip(provenance)
        .filter(|(i, _)| i.role == EntryRole::ReferenceOnly)
        .collect();
    Ok(canonical_digest(&evidence)?)
}

async fn guard(deadline: Deadline, context: Option<&PreparationContext>) -> Result<()> {
    deadline.check()?;
    if let Some(context) = context {
        context.check_current().await?;
    }
    Ok(())
}

async fn read_file(
    root: &SafeRoot,
    root_path: &Path,
    relative: &str,
    capture: bool,
    deadline: Deadline,
    context: Option<&PreparationContext>,
) -> Result<(ContentDigest, Vec<u8>)> {
    guard(deadline, context).await?;
    let mut file = root.open_file(relative)?;
    let before = file.metadata()?;
    ensure!(
        before.len() <= MAX_FILE,
        "continuation_budget_exceeded: file"
    );
    let mut bytes = Vec::new();
    let mut total = 0;
    let mut buffer = vec![0; 1024 * 1024];
    let mut digest = Sha256::new();
    loop {
        guard(deadline, context).await?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(
            total <= MAX_FILE,
            "continuation_budget_exceeded: growing file"
        );
        digest.update(&buffer[..n]);
        if capture {
            bytes.extend_from_slice(&buffer[..n]);
        }
    }
    guard(deadline, context).await?;
    root.verify_path(root_path)?;
    ensure!(
        identity(&before) == identity(&file.metadata()?)
            && identity(&before) == identity(&root.open_file(relative)?.metadata()?),
        "prepared_material_changed: file identity"
    );
    Ok((
        ContentDigest::from_sha256_hex(&format!("{:x}", digest.finalize()))?,
        bytes,
    ))
}

pub(super) async fn clean_checkout_index_digest(
    checkout: &Path,
    deadline: Deadline,
    context: Option<&PreparationContext>,
) -> Result<ContentDigest> {
    async {
        guard(deadline, context).await?;
        let path = PathBuf::from(
            git::text(
                checkout,
                &["rev-parse", "--path-format=absolute", "--git-path", "index"],
                deadline,
            )
            .await?,
        );
        let parent = path.parent().context("missing index parent")?;
        ensure!(
            path.file_name().is_some_and(|name| name == "index"),
            "invalid index path"
        );
        let root = SafeRoot::open(parent)?;
        let info = root.inspect("index")?.context("missing index")?;
        ensure!(
            info.size <= git::OUTPUT_LIMIT as u64,
            "continuation_budget_exceeded: index"
        );
        let (before, _) = read_file(&root, parent, "index", false, deadline, context).await?;
        let flags = git::run(checkout, &["ls-files", "-v", "-z"], None, deadline).await?;
        ensure!(
            flags
                .split(|b| *b == 0)
                .filter(|entry| !entry.is_empty())
                .all(|entry| entry.starts_with(b"H ")),
            "hidden index entries"
        );
        guard(deadline, context).await?;
        let staged = git::run(
            checkout,
            &[
                "diff",
                "--cached",
                "--raw",
                "--no-ext-diff",
                "--no-textconv",
                "HEAD",
                "--",
            ],
            None,
            deadline,
        )
        .await?;
        ensure!(staged.is_empty(), "index differs from HEAD");
        let (after, _) = read_file(&root, parent, "index", false, deadline, context).await?;
        ensure!(before == after, "index changed during verification");
        Ok(before)
    }
    .await
    .context("prepared_material_changed: checkout index")
}

pub(super) async fn verify_material(
    m: &ManifestV1,
    op: &Continuation,
    deadline: Deadline,
    context: Option<&PreparationContext>,
) -> Result<()> {
    verify(m, op, deadline, context, true).await
}

async fn verify(
    m: &ManifestV1,
    op: &Continuation,
    deadline: Deadline,
    context: Option<&PreparationContext>,
    verify_successor: bool,
) -> Result<()> {
    guard(deadline, context).await?;
    validate_bindings(m, op)?;
    let workspace = &m.workspace;
    let root_path = &workspace.operation_root;
    let root = SafeRoot::open(root_path)?;
    for (relative, expected) in &workspace.directory_identities {
        guard(deadline, context).await?;
        let stat = SafeRoot::open(&root_path.join(relative))?.file.metadata()?;
        ensure!(
            (stat.dev(), stat.ino()) == *expected && stat.mode() & 0o777 == 0o700,
            "prepared_material_changed: directory identity or privacy"
        );
    }
    let expected_files = [
        "plan.json",
        "target.json",
        "workflow.snapshot.json",
        "catalog.snapshot.json",
        "preservation/index",
        "preservation/staged.patch",
        "preservation/unstaged.patch",
        "preservation/manifest.json",
        "verified-receipt.json",
        "owner.json",
    ];
    ensure!(
        m.files.len() == expected_files.len(),
        "prepared_material_changed: file set"
    );
    for name in expected_files {
        let file = m
            .files
            .iter()
            .find(|f| f.relative_path == name)
            .context("prepared_material_changed: missing binding")?;
        let info = root
            .inspect(name)?
            .context("prepared_material_changed: missing file")?;
        ensure!(
            info.size == file.bytes && info.mode & 0o777 == 0o600,
            "prepared_material_changed: file size or privacy"
        );
        let (digest, _) = read_file(&root, root_path, name, false, deadline, context).await?;
        ensure!(
            digest == file.sha256,
            "prepared_material_changed: file digest {name}"
        );
    }
    let meta = SafeRoot::open(&workspace.metadata_root)?;
    let mut meta_paths = BTreeSet::new();
    for (input, provenance) in m.inputs.iter().zip(&m.input_provenance) {
        if !verify_successor && input.role == EntryRole::ExecutionSeed {
            continue;
        }
        let path = &input.target_relative_path;
        meta_paths.insert(path.clone());
        let info = meta
            .inspect(path)?
            .context("prepared_material_changed: missing selected input")?;
        ensure!(
            info.size == provenance.bytes && info.mode & 0o777 == provenance.installed_mode,
            "prepared_material_changed: selected input metadata"
        );
        let (digest, _) = read_file(
            &meta,
            &workspace.metadata_root,
            path,
            false,
            deadline,
            context,
        )
        .await?;
        ensure!(
            digest == input.content_sha256,
            "prepared_material_changed: selected input digest"
        );
    }
    if verify_successor {
        verify_tree(
            &workspace.metadata_root,
            &meta_paths,
            false,
            deadline,
            context,
        )
        .await?;
    }
    let entries = m.plan["entries"]
        .as_array()
        .context("prepared_material_changed: entries")?;
    let mut expected_paths = BTreeSet::new();
    for entry in entries {
        if entry["workspace_entry"].is_null() {
            continue;
        }
        let file: WorkspaceFile = serde_json::from_value(entry["workspace_entry"].clone())?;
        validate_relative(&file.path)?;
        if !file.deleted && !file.excluded {
            expected_paths.insert(file.path.clone());
        }
        for path in [
            &workspace.checkout_root,
            &workspace.preservation_root.join("workspace"),
        ] {
            if !verify_successor && path == &workspace.checkout_root {
                continue;
            }
            guard(deadline, context).await?;
            let directory = SafeRoot::open(path)?;
            if let Some(replacement) = &file.replacement {
                match replacement {
                    Replacement::Directory => ensure!(
                        directory
                            .inspect(&file.path)?
                            .is_none_or(|i| i.mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32),
                        "prepared_material_changed: replacement directory"
                    ),
                    Replacement::Ancestor { path } => ensure!(
                        directory.inspect(path)?.is_some_and(|i| [
                            libc::S_IFREG as u32,
                            libc::S_IFLNK as u32
                        ]
                        .contains(&(i.mode & libc::S_IFMT as u32))),
                        "prepared_material_changed: replacement ancestor"
                    ),
                }
            } else if file.deleted || file.excluded {
                ensure!(
                    directory.inspect(&file.path)?.is_none(),
                    "prepared_material_changed: excluded or deleted path"
                );
            } else if let Some(link) = &file.link_target {
                ensure!(
                    directory.read_link(&file.path)? == *link,
                    "prepared_material_changed: link target"
                );
            } else {
                let info = directory
                    .inspect(&file.path)?
                    .context("prepared_material_changed: missing workspace file")?;
                ensure!(
                    info.size == file.size && info.mode & 0o777 == file.mode,
                    "prepared_material_changed: workspace metadata"
                );
                let (digest, _) =
                    read_file(&directory, path, &file.path, false, deadline, context).await?;
                ensure!(
                    file.sha256.as_ref() == Some(&digest),
                    "prepared_material_changed: workspace content"
                );
            }
        }
    }
    if verify_successor {
        verify_tree(
            &workspace.checkout_root,
            &expected_paths,
            true,
            deadline,
            context,
        )
        .await?;
    }
    verify_tree(
        &workspace.preservation_root.join("workspace"),
        &expected_paths,
        false,
        deadline,
        context,
    )
    .await?;
    if verify_successor {
        ensure!(
            clean_checkout_index_digest(&workspace.checkout_root, deadline, context).await?
                == workspace.checkout_index_sha256,
            "prepared_material_changed: checkout index digest"
        );
        ensure!(
            git::text(
                &workspace.checkout_root,
                &["rev-parse", "--verify", "HEAD^{commit}"],
                deadline
            )
            .await?
                == workspace.head,
            "prepared_material_changed: checkout revision"
        );
        guard(deadline, context).await?;
        ensure!(
            git::text(
                &workspace.checkout_root,
                &["symbolic-ref", "HEAD"],
                deadline
            )
            .await?
                == workspace.pin_ref,
            "prepared_material_changed: checkout pin"
        );
        guard(deadline, context).await?;
    }
    if verify_successor {
        verify_sources(m, deadline, context).await?;
    }
    root.verify_path(root_path)?;
    Ok(())
}

fn validate_bindings(m: &ManifestV1, op: &Continuation) -> Result<()> {
    ensure!(
        m.schema_version == "run_carry_forward_manifest_v1"
            && m.operation_id.to_string() == op.operation_id
            && m.source_run_id.to_string() == op.source_run_id
            && m.reserved_successor_run_id.to_string() == op.reserved_successor_run_id
            && m.idea_id.to_string() == op.idea_id
            && m.source_run_id != m.reserved_successor_run_id
            && m.plan_sha256 == db_digest(&op.plan_sha256)?
            && m.source_semantic_sha256 == db_digest(&op.source_witness_sha256)?,
        "prepared_material_changed: operation binding"
    );
    let root = &m.workspace.operation_root;
    ensure!(
        root.file_name().and_then(|n| n.to_str()) == Some(op.operation_id.as_str())
            && m.workspace.checkout_root == root.join("checkout")
            && m.workspace.metadata_root == root.join("metadata")
            && m.workspace.preservation_root == root.join("preservation")
            && !root.starts_with(&m.workspace.source_root)
            && m.workspace.pin_ref == format!("refs/heads/carry-forward-{}", m.operation_id),
        "unsafe_path: owned roots"
    );
    let expected_dirs: BTreeSet<_> = [
        "",
        "checkout",
        "preservation",
        "preservation/workspace",
        "metadata",
    ]
    .into_iter()
    .collect();
    ensure!(
        m.workspace
            .directory_identities
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            == expected_dirs,
        "prepared_material_changed: directory bindings"
    );
    for (actual, name) in [(&op.plan_ref, "plan.json"), (&op.target_ref, "target.json")] {
        ensure!(
            actual == name || Path::new(actual) == root.join(name),
            "unsafe_path: reserved file reference"
        );
    }
    let mut body = m.plan.clone();
    ensure!(
        body["schema_version"] == "run_carry_forward_plan_v1"
            && body["source_run_id"] == m.source_run_id.to_string()
            && body["plan_sha256"] == serde_json::to_value(&m.plan_sha256)?,
        "prepared_material_changed: plan schema"
    );
    body.as_object_mut()
        .context("prepared_material_changed: plan")?
        .remove("plan_sha256");
    ensure!(
        canonical_digest(&body)? == m.plan_sha256
            && canonical_digest(&m.plan["source_witness"])? == m.source_witness_sha256
            && m.plan["source_witness"]["semantic_sha256"]
                == serde_json::to_value(&m.source_semantic_sha256)?
            && m.plan["source_witness"]["workspace_sha256"]
                == serde_json::to_value(&m.workspace.source_witness_sha256)?
            && canonical_digest(&m.target)? == m.target_sha256
            && m.plan["target"]["compiled_plan_sha256"] == serde_json::to_value(&m.target_sha256)?
            && m.plan["target"]["workflow_snapshot_hash"]
                == serde_json::to_value(&m.original_workflow_sha256)?
            && m.plan["target"]["catalog_snapshot_hash"]
                == serde_json::to_value(&m.original_catalog_sha256)?
            && m.plan["target"]["definition_files_sha256"]
                == serde_json::to_value(&m.definition_files_sha256)?
            && canonical_digest(&m.plan["capability_delta"])? == m.capability_delta_sha256,
        "prepared_material_changed: evidence binding"
    );
    let seed = seed_path(&m.target)?;
    ensure!(
        m.inputs.len() == m.input_provenance.len()
            && !m.inputs.is_empty()
            && m.inputs.len() <= 129
            && m.inputs
                .iter()
                .filter(|i| i.role == EntryRole::ExecutionSeed)
                .count()
                == 1,
        "artifact_provenance_invalid: inputs"
    );
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for (ordinal, (i, p)) in m.inputs.iter().zip(&m.input_provenance).enumerate() {
        validate_relative(&i.target_relative_path)?;
        validate_relative(&p.source_relative_path)?;
        ensure!(
            i.ordinal == ordinal
                && i.input_id == p.input_id
                && ids.insert(i.input_id)
                && paths.insert(&i.target_relative_path)
                && i.source_run_id == m.source_run_id
                && i.content_sha256 == p.content_sha256
                && i.source_schema == p.source_contract
                && p.ancestor_manifest_sha256.len() <= 32
                && p.bytes <= MAX_FILE
                && p.installed_mode == 0o600
                && ["artifact", "carried_input"].contains(&i.source_kind.as_str()),
            "artifact_provenance_invalid: input binding"
        );
        match i.role {
            EntryRole::ExecutionSeed => ensure!(
                i.target_logical_name == "proposal_current"
                    && p.logical_name == "proposal_current"
                    && i.source_schema == "proposal_current"
                    && i.target_relative_path == seed,
                "artifact_provenance_invalid: execution seed"
            ),
            EntryRole::ReferenceOnly => ensure!(
                i.target_relative_path
                    == format!("carry-forward/references/{}/content", i.input_id),
                "unsafe_path: reference namespace"
            ),
            _ => anyhow::bail!("artifact_provenance_invalid: input role"),
        }
    }
    ensure!(
        findings_digest(&m.inputs, &m.input_provenance)? == m.unresolved_findings_sha256,
        "prepared_material_changed: findings binding"
    );
    ensure!(
        serde_json::to_value(&m.successor)?
            == serde_json::to_value(build_successor(op, &m.workspace, &m.target, &m.plan)?)?,
        "prepared_material_changed: successor blueprint"
    );
    let entries = m.plan["entries"]
        .as_array()
        .context("prepared_material_changed: entries")?;
    ensure!(
        entries.len() <= 50_000,
        "continuation_budget_exceeded: entries"
    );
    let bytes: Vec<_> = entries
        .iter()
        .map(|e| {
            e["bytes"]
                .as_u64()
                .context("prepared_material_changed: entry bytes")
        })
        .collect::<Result<_>>()?;
    let patches: Vec<_> = [
        "preservation/index",
        "preservation/staged.patch",
        "preservation/unstaged.patch",
    ]
    .iter()
    .map(|path| {
        m.files
            .iter()
            .find(|f| f.relative_path == *path)
            .map(|f| f.bytes)
            .context("prepared_material_changed: patches")
    })
    .collect::<Result<_>>()?;
    ensure!(
        bounded_preservation_bytes(bytes, patches)? == m.preserved_bytes,
        "prepared_material_changed: preservation budget"
    );
    for (name, bytes) in [
        ("plan.json", serde_json::to_vec(&m.plan)?),
        (
            "target.json",
            serde_json::to_vec(&serde_json::to_value(&m.target)?)?,
        ),
        (
            "workflow.snapshot.json",
            m.target.plan.workflow_snapshot_json.as_bytes().to_vec(),
        ),
        (
            "catalog.snapshot.json",
            m.target.plan.catalog_snapshot_json.as_bytes().to_vec(),
        ),
    ] {
        ensure!(
            m.files.iter().any(|f| f.relative_path == name
                && f.sha256 == ContentDigest::of(&bytes)
                && f.bytes == bytes.len() as u64),
            "prepared_material_changed: frozen file binding"
        );
    }
    ensure!(
        ContentDigest::of(m.target.plan.workflow_snapshot_json.as_bytes())
            == db_digest(&m.target.plan.workflow_snapshot_hash)?
            && ContentDigest::of(m.target.plan.catalog_snapshot_json.as_bytes())
                == db_digest(&m.target.plan.catalog_snapshot_hash)?
            && m.files
                .iter()
                .any(|f| f.relative_path == "preservation/manifest.json"
                    && f.sha256 == m.workspace.preservation_manifest_sha256),
        "prepared_material_changed: snapshot binding"
    );
    Ok(())
}

async fn verify_sources(
    m: &ManifestV1,
    deadline: Deadline,
    context: Option<&PreparationContext>,
) -> Result<()> {
    let options = PreviewOptions {
        exclusions: serde_json::from_value(
            m.plan["selection"]["excluded_workspace_paths"].clone(),
        )?,
        ..PreviewOptions::default()
    };
    guard(deadline, context).await?;
    let workspace =
        WorkspacePlanner::preview_with_deadline(&m.workspace.source_root, options, deadline)
            .await?;
    guard(deadline, context).await?;
    ensure!(
        *workspace.digest() == m.workspace.source_witness_sha256,
        "source_changed: workspace witness"
    );
    let expected: (u64, u64) = serde_json::from_value(
        m.plan["source_witness"]["directory_identities"]["metadata"].clone(),
    )?;
    for p in &m.input_provenance {
        guard(deadline, context).await?;
        let root = SafeRoot::open(&p.source_root)?;
        let directory = root.file.metadata()?;
        ensure!(
            (directory.dev(), directory.ino()) == expected,
            "source_changed: selected input root"
        );
        let info = root
            .inspect(&p.source_relative_path)?
            .context("source_changed: missing selected input")?;
        ensure!(
            info.size == p.bytes && info.mode == p.source_mode,
            "source_changed: selected input metadata"
        );
        let (digest, _) = read_file(
            &root,
            &p.source_root,
            &p.source_relative_path,
            false,
            deadline,
            context,
        )
        .await
        .context("source_changed: selected input read")?;
        ensure!(
            digest == p.content_sha256,
            "source_changed: selected input digest"
        );
    }
    guard(deadline, context).await
}

#[derive(Deserialize)]
struct WorkspaceFile {
    path: String,
    sha256: Option<ContentDigest>,
    size: u64,
    mode: u32,
    deleted: bool,
    replacement: Option<Replacement>,
    excluded: bool,
    link_target: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Replacement {
    Directory,
    Ancestor { path: String },
}

async fn verify_tree(
    root_path: &Path,
    expected: &BTreeSet<String>,
    checkout: bool,
    deadline: Deadline,
    context: Option<&PreparationContext>,
) -> Result<()> {
    let root = SafeRoot::open(root_path)?;
    let mut pending = vec![PathBuf::new()];
    let mut seen = BTreeSet::new();
    let mut count = 0;
    while let Some(relative) = pending.pop() {
        guard(deadline, context).await?;
        SafeRoot::open(&root_path.join(&relative))?;
        for item in fs::read_dir(root_path.join(&relative))? {
            guard(deadline, context).await?;
            count += 1;
            ensure!(
                count <= 100_000,
                "continuation_budget_exceeded: verification tree"
            );
            let item = item?;
            let path = relative.join(item.file_name());
            let name = path.to_str().context("unsafe_path: non-UTF8 tree")?;
            if checkout && name == ".git" {
                continue;
            }
            let stat = root
                .inspect(name)?
                .context("prepared_material_changed: tree entry")?;
            if stat.mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32 {
                pending.push(path);
            } else {
                ensure!(
                    expected.contains(name),
                    "prepared_material_changed: unexpected file"
                );
                seen.insert(name.to_owned());
            }
        }
    }
    ensure!(&seen == expected, "prepared_material_changed: file set");
    root.verify_path(root_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_inputs_share_the_workspace_preservation_ceiling() {
        assert_eq!(
            bounded_preservation_bytes([MAX_FILE; 8], []).unwrap(),
            MAX_TOTAL
        );
        assert!(bounded_preservation_bytes([MAX_FILE; 8], [1]).is_err());
        assert!(bounded_preservation_bytes([MAX_FILE + 1], []).is_err());
    }
}
