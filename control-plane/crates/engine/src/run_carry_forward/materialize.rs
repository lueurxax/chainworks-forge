use anyhow::{ensure, Context, Result};
use domain::run_carry_forward::ContentDigest;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs,
    io::{self, Read, Write},
    os::fd::AsRawFd,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

use super::{
    budget::Deadline,
    git,
    safe_files::{hash_file, identity, SafeRoot},
    PreparationContext, WorkspaceEntry, WorkspacePlanner, WorkspacePreview, WorkspaceReplacement,
};

#[derive(Debug, Serialize)]
pub struct MaterializationReceiptV1 {
    pub schema_version: &'static str,
    pub operation_id: Uuid,
    pub operation_root: PathBuf,
    pub checkout_root: PathBuf,
    pub preservation_root: PathBuf,
    pub pin_ref: String,
    pub source_witness: ContentDigest,
    pub manifest_sha256: ContentDigest,
}

/// I1 local core. Production callers must supply I2's journalled intent/fence before wiring this.
/// An existing destination is never resumed or deleted, even after an interrupted attempt.
/// External writers must be quiescent over source, destination and their ancestors: Git is
/// pathname-based, so identity checks do not sandbox active same-UID path replacement.
/// Any error after effects retains evidence (possibly a receipt) and never authorizes replay.
pub async fn materialize(
    preview: &WorkspacePreview,
    owned_parent: &Path,
    operation_id: Uuid,
) -> Result<MaterializationReceiptV1> {
    let deadline = Deadline::after(Duration::from_secs(15 * 60));
    materialize_with_context(preview, owned_parent, operation_id, deadline, None).await
}

pub(super) async fn materialize_with_context(
    preview: &WorkspacePreview,
    owned_parent: &Path,
    operation_id: Uuid,
    deadline: Deadline,
    context: Option<&PreparationContext>,
) -> Result<MaterializationReceiptV1> {
    checkpoint(context, deadline, "workspace:validate").await?;
    let parent = fs::canonicalize(owned_parent)?;
    let parent_guard = SafeRoot::open(&parent)?;
    ensure!(
        !parent.starts_with(&preview.git_dir) && !parent.starts_with(&preview.git_common_dir),
        "unsafe_path: Git metadata destination"
    );
    ensure!(
        !parent.starts_with(&preview.source),
        "unsafe_path: overlapping source destination"
    );
    let current = WorkspacePlanner::preview_with_deadline(
        &preview.source,
        preview.options.clone(),
        deadline.within(Duration::from_secs(120)),
    )
    .await?;
    checkpoint(context, deadline, "workspace:validated").await?;
    ensure!(
        current.digest() == preview.digest(),
        "source_changed: preview witness"
    );
    let operation_root = parent.join(operation_id.to_string());
    begin_phase(
        context,
        "preservation",
        serde_json::json!({
            "source_witness":preview.digest(), "head":preview.head,
            "parent_identity":identity(&parent_guard.file.metadata()?),
            "files":"preview-bound workspace, index, staged.patch, unstaged.patch, manifest.json",
        }),
        &operation_root,
    )
    .await?;
    checkpoint(context, deadline, "create:operation").await?;
    parent_guard.verify_path(&parent)?;
    let operation = parent_guard
        .create_directory(&operation_id.to_string())
        .context("destination_conflict: operation root")?;
    operation.verify_path(&operation_root)?;
    publish_checked(
        &operation,
        &operation_root,
        "owner.json",
        &serde_json::to_vec(&serde_json::json!({
            "schema_version":"run_carry_forward_owner_v1", "operation_id":operation_id,
            "source_witness":preview.digest(),
        }))?,
        deadline,
        context,
    )
    .await?;
    let preservation_root = operation_root.join("preservation");
    let workspace_archive = preservation_root.join("workspace");
    let checkout_root = operation_root.join("checkout");
    checkpoint(context, deadline, "create:preservation").await?;
    let preservation = operation.create_directory("preservation")?;
    checkpoint(context, deadline, "create:archive").await?;
    let archive = preservation.create_directory("workspace")?;
    let source = SafeRoot::open(&preview.source)?;
    copy_entries(
        &source,
        &archive,
        &preview.entries,
        preview.options.limits.max_file_bytes,
        deadline,
        context,
        "archive",
    )
    .await?;
    publish_checked(
        &preservation,
        &preservation_root,
        "index",
        &preview.index,
        deadline,
        context,
    )
    .await?;
    publish_checked(
        &preservation,
        &preservation_root,
        "staged.patch",
        &preview.staged,
        deadline,
        context,
    )
    .await?;
    publish_checked(
        &preservation,
        &preservation_root,
        "unstaged.patch",
        &preview.unstaged,
        deadline,
        context,
    )
    .await?;
    let manifest = serde_json::to_vec(preview)?;
    publish_checked(
        &preservation,
        &preservation_root,
        "manifest.json",
        &manifest,
        deadline,
        context,
    )
    .await?;
    verify_entries(
        &archive,
        &preview.entries,
        preview.options.limits.max_file_bytes,
        deadline,
    )?;
    finish_phase(context, "preservation", &ContentDigest::of(&manifest)).await?;

    let branch = format!("carry-forward-{operation_id}");
    let pin_ref = format!("refs/heads/{branch}");
    let checkout = checkout_root
        .to_str()
        .context("unsafe_path: non-UTF8 destination")?;
    begin_phase(
        context,
        "git_worktree",
        serde_json::json!({
            "head":preview.head, "pin_ref":pin_ref, "source_witness":preview.digest(),
            "operation_identity":identity(&operation.file.metadata()?),
        }),
        &checkout_root,
    )
    .await?;
    checkpoint(context, deadline, "git:worktree_add").await?;
    parent_guard.verify_path(&parent)?;
    operation.verify_path(&operation_root)?;
    archive.verify_path(&workspace_archive)?;
    git::run(
        &preview.source,
        &[
            "worktree",
            "add",
            "--no-checkout",
            "-b",
            &branch,
            checkout,
            &preview.head,
        ],
        None,
        deadline,
    )
    .await?;
    checkpoint(context, deadline, "git:worktree_added").await?;
    operation.verify_path(&operation_root)?;
    finish_phase(
        context,
        "git_worktree",
        &ContentDigest::of(&serde_json::to_vec(&serde_json::json!({
            "head":preview.head, "pin_ref":pin_ref, "checkout":checkout_root,
        }))?),
    )
    .await?;
    begin_phase(
        context,
        "git_index",
        serde_json::json!({"head":preview.head, "command":"read-tree HEAD"}),
        &checkout_root,
    )
    .await?;
    checkpoint(context, deadline, "git:read_tree").await?;
    git::run(&checkout_root, &["read-tree", "HEAD"], None, deadline).await?;
    checkpoint(context, deadline, "git:read_tree_completed").await?;
    finish_phase(
        context,
        "git_index",
        &ContentDigest::of(preview.head.as_bytes()),
    )
    .await?;
    let destination = SafeRoot::open(&checkout_root)?;
    begin_phase(context, "workspace_overlay", serde_json::json!({"source_witness":preview.digest(),
        "checkout_identity":identity(&destination.file.metadata()?), "files":"all non-excluded preview entries"}), &checkout_root).await?;
    copy_entries(
        &archive,
        &destination,
        &preview.entries,
        preview.options.limits.max_file_bytes,
        deadline,
        context,
        "checkout",
    )
    .await?;
    verify_entries(
        &destination,
        &preview.entries,
        preview.options.limits.max_file_bytes,
        deadline,
    )?;
    verify_entries(
        &archive,
        &preview.entries,
        preview.options.limits.max_file_bytes,
        deadline,
    )?;
    let current = WorkspacePlanner::preview_with_deadline(
        &preview.source,
        preview.options.clone(),
        deadline.within(Duration::from_secs(120)),
    )
    .await?;
    checkpoint(context, deadline, "workspace:reverified").await?;
    ensure!(
        current.digest() == preview.digest(),
        "source_changed: materialization witness"
    );
    ensure!(
        git::text(
            &checkout_root,
            &["rev-parse", "--verify", "HEAD^{commit}"],
            deadline
        )
        .await?
            == preview.head,
        "prepared_material_changed: checkout revision"
    );
    checkpoint(context, deadline, "workspace:head_verified").await?;
    finish_phase(context, "workspace_overlay", preview.digest()).await?;
    let receipt = MaterializationReceiptV1 {
        schema_version: "run_carry_forward_materialization_v1",
        operation_id,
        operation_root,
        checkout_root,
        preservation_root,
        pin_ref,
        source_witness: preview.digest().clone(),
        manifest_sha256: ContentDigest::of(&manifest),
    };
    parent_guard.verify_path(&parent)?;
    operation.verify_path(&receipt.operation_root)?;
    preservation.verify_path(&receipt.preservation_root)?;
    archive.verify_path(&workspace_archive)?;
    destination.verify_path(&receipt.checkout_root)?;
    begin_phase(
        context,
        "workspace_receipt",
        serde_json::json!({"source_witness":preview.digest(),
        "receipt":"verified-receipt.json"}),
        &receipt.operation_root,
    )
    .await?;
    publish_checked(
        &operation,
        &receipt.operation_root,
        "verified-receipt.json",
        &serde_json::to_vec(&receipt)?,
        deadline,
        context,
    )
    .await?;
    finish_phase(
        context,
        "workspace_receipt",
        &ContentDigest::of(&serde_json::to_vec(&receipt)?),
    )
    .await?;
    Ok(receipt)
}

async fn copy_entries(
    source: &SafeRoot,
    destination: &SafeRoot,
    entries: &[WorkspaceEntry],
    limit: u64,
    deadline: Deadline,
    context: Option<&PreparationContext>,
    phase: &str,
) -> Result<()> {
    for entry in entries {
        checkpoint(context, deadline, &format!("copy:{phase}:{}", entry.path)).await?;
        if entry.deleted || entry.excluded {
            continue;
        }
        if let Some(target) = &entry.link_target {
            ensure!(
                source.read_link(&entry.path)? == *target,
                "source_changed: link"
            );
            checkpoint(context, deadline, "copy:link").await?;
            destination.create_link(&entry.path, target)?;
            checkpoint(
                context,
                deadline,
                &format!("copy_done:{phase}:{}", entry.path),
            )
            .await?;
            continue;
        }
        let mut input = source.open_file(&entry.path)?;
        let before = input.metadata()?;
        ensure!(
            before.len() == entry.size && before.len() <= limit,
            "source_changed: size"
        );
        checkpoint(context, deadline, "copy:create_file").await?;
        let mut output = destination.create_file(&entry.path)?;
        let mut digest = Sha256::new();
        let mut total = 0u64;
        let mut buffer = vec![0u8; 1024 * 1024];
        loop {
            check_current(context, deadline).await?;
            let n = input.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            ensure!(
                total <= entry.size && total <= limit,
                "source_changed: growing copy"
            );
            check_current(context, deadline).await?;
            output.write_all(&buffer[..n])?;
            digest.update(&buffer[..n]);
        }
        let digest = ContentDigest::from_sha256_hex(&format!("{:x}", digest.finalize()))?;
        ensure!(
            entry.sha256.as_ref() == Some(&digest)
                && total == entry.size
                && identity(&before) == identity(&input.metadata()?),
            "source_changed: copy digest"
        );
        check_current(context, deadline).await?;
        output.set_permissions(fs::Permissions::from_mode(entry.mode))?;
        check_current(context, deadline).await?;
        output.sync_all()?;
        checkpoint(
            context,
            deadline,
            &format!("copy_done:{phase}:{}", entry.path),
        )
        .await?;
    }
    check_current(context, deadline).await?;
    destination.file.sync_all()?;
    Ok(())
}

async fn check_current(context: Option<&PreparationContext>, deadline: Deadline) -> Result<()> {
    deadline.check()?;
    if let Some(context) = context {
        context.check_current().await?;
    }
    Ok(())
}

async fn checkpoint(
    context: Option<&PreparationContext>,
    deadline: Deadline,
    label: &str,
) -> Result<()> {
    deadline.check()?;
    if let Some(context) = context {
        context.checkpoint(label).await?;
    }
    Ok(())
}

async fn begin_phase(
    context: Option<&PreparationContext>,
    key: &str,
    intent: serde_json::Value,
    destination: &Path,
) -> Result<()> {
    if let Some(context) = context {
        context.begin_step(key, &intent, destination).await?;
    }
    Ok(())
}

async fn finish_phase(
    context: Option<&PreparationContext>,
    key: &str,
    receipt: &ContentDigest,
) -> Result<()> {
    if let Some(context) = context {
        context.finish_step(key, receipt).await?;
    }
    Ok(())
}

/// Keep the final link (authoritative publication) behind a fresh generation check,
/// including when cancellation happens during a large write or fsync.
pub(super) async fn publish_checked(
    root: &SafeRoot,
    path: &Path,
    name: &str,
    bytes: &[u8],
    deadline: Deadline,
    context: Option<&PreparationContext>,
) -> Result<()> {
    super::safe_files::validate_relative(name)?;
    ensure!(!name.contains('/'), "unsafe_path: publication name");
    if context.is_none() {
        return root.publish(name, bytes, deadline);
    }
    checkpoint(context, deadline, &format!("publish:{name}")).await?;
    root.verify_path(path)?;
    let pending = format!(".pending-{}", Uuid::new_v4());
    let mut file = root.create_file(&pending)?;
    for chunk in bytes.chunks(1024 * 1024) {
        check_current(context, deadline).await?;
        file.write_all(chunk)?;
    }
    check_current(context, deadline).await?;
    file.sync_all()?;
    checkpoint(context, deadline, &format!("publish_ready:{name}")).await?;
    root.verify_path(path)?;
    let from = CString::new(pending)?;
    let to = CString::new(name)?;
    let result = unsafe {
        libc::linkat(
            root.file.as_raw_fd(),
            from.as_ptr(),
            root.file.as_raw_fd(),
            to.as_ptr(),
            0,
        )
    };
    if result < 0 {
        return Err(io::Error::last_os_error()).context("destination_conflict: publication");
    }
    root.file.sync_all()?;
    check_current(context, deadline).await?;
    let result = unsafe { libc::unlinkat(root.file.as_raw_fd(), from.as_ptr(), 0) };
    if result < 0 {
        return Err(io::Error::last_os_error().into());
    }
    root.file.sync_all()?;
    checkpoint(context, deadline, &format!("published:{name}")).await
}

fn verify_entries(
    root: &SafeRoot,
    entries: &[WorkspaceEntry],
    limit: u64,
    deadline: Deadline,
) -> Result<()> {
    for entry in entries {
        deadline.check()?;
        if let Some(replacement) = &entry.replacement {
            match replacement {
                WorkspaceReplacement::Directory => {
                    ensure!(
                        root.inspect(&entry.path)?.is_none_or(|info| info.mode
                            & libc::S_IFMT as u32
                            == libc::S_IFDIR as u32),
                        "prepared_material_changed: historical file became a leaf"
                    );
                }
                WorkspaceReplacement::Ancestor { path } => {
                    ensure!(
                        root.inspect(path)?.is_some_and(|info| [
                            libc::S_IFREG as u32,
                            libc::S_IFLNK as u32
                        ]
                        .contains(&(info.mode & libc::S_IFMT as u32))),
                        "prepared_material_changed: missing replacement ancestor"
                    );
                }
            }
            continue;
        }
        let info = root.inspect(&entry.path)?;
        if entry.deleted || entry.excluded {
            ensure!(
                info.is_none(),
                "prepared_material_changed: excluded or deleted path"
            );
        } else if let Some(target) = &entry.link_target {
            ensure!(
                root.read_link(&entry.path)? == *target,
                "prepared_material_changed: link"
            );
        } else {
            let info = info.context("prepared_material_changed: missing file")?;
            ensure!(
                info.mode & 0o777 == entry.mode
                    && Some(hash_file(root.open_file(&entry.path)?, limit, deadline)?)
                        == entry.sha256,
                "prepared_material_changed: content or mode"
            );
        }
    }
    Ok(())
}

/// Journalled finalizer copy: bounded memory, no-follow source and parents, and no
/// authoritative destination until the complete digest has matched. Partial bytes
/// remain private evidence after cancellation; this function never retries.
#[allow(clippy::too_many_arguments)]
pub(super) async fn copy_verified_input(
    source_path: &Path,
    source_relative: &str,
    destination_path: &Path,
    destination_relative: &str,
    expected: &ContentDigest,
    size: u64,
    mode: u32,
    context: &PreparationContext,
) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    super::safe_files::validate_relative(destination_relative)?;
    ensure!(
        size <= 256 * 1024 * 1024,
        "continuation_budget_exceeded: selected input"
    );
    context.checkpoint("input:open").await?;
    let source = SafeRoot::open(source_path)?;
    let mut input = source.open_file(source_relative)?;
    let before = input.metadata()?;
    ensure!(
        before.len() == size && before.mode() == mode,
        "source_changed: selected input metadata"
    );
    let destination = SafeRoot::open(destination_path)?;
    let mut parent_path = destination_path.to_path_buf();
    let mut parent = SafeRoot::open(&parent_path)?;
    let mut components: Vec<_> = destination_relative.split('/').collect();
    let name = components
        .pop()
        .context("unsafe_path: selected input destination")?;
    for component in components {
        context.checkpoint("input:create_parent").await?;
        destination.verify_path(destination_path)?;
        parent.verify_path(&parent_path)?;
        if parent.inspect(component)?.is_none() {
            parent.create_directory(component)?;
        }
        parent_path.push(component);
        parent = SafeRoot::open(&parent_path)?;
    }
    context.checkpoint("input:create_pending").await?;
    parent.verify_path(&parent_path)?;
    let pending = format!(".pending-{}", Uuid::new_v4());
    let mut output = parent.create_file(&pending)?;
    let mut buffer = vec![0; 1024 * 1024];
    let mut digest = Sha256::new();
    let mut total = 0;
    loop {
        context.check_current().await?;
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        ensure!(total <= size, "source_changed: growing selected input");
        context.checkpoint("input:copy_chunk").await?;
        output.write_all(&buffer[..n])?;
        digest.update(&buffer[..n]);
    }
    ensure!(
        total == size
            && *expected == ContentDigest::from_sha256_hex(&format!("{:x}", digest.finalize()))?
            && identity(&before) == identity(&input.metadata()?),
        "source_changed: selected input digest"
    );
    context.check_current().await?;
    output.sync_all()?;
    context.checkpoint("input:publish_ready").await?;
    source.verify_path(source_path)?;
    ensure!(
        identity(&before) == identity(&source.open_file(source_relative)?.metadata()?),
        "source_changed: selected input replaced"
    );
    destination.verify_path(destination_path)?;
    parent.verify_path(&parent_path)?;
    let from = CString::new(pending)?;
    let to = CString::new(name)?;
    if unsafe {
        libc::linkat(
            parent.file.as_raw_fd(),
            from.as_ptr(),
            parent.file.as_raw_fd(),
            to.as_ptr(),
            0,
        )
    } < 0
    {
        return Err(io::Error::last_os_error())
            .context("destination_conflict: selected input publication");
    }
    parent.file.sync_all()?;
    context.check_current().await?;
    if unsafe { libc::unlinkat(parent.file.as_raw_fd(), from.as_ptr(), 0) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    parent.file.sync_all()?;
    context.checkpoint("input:published").await
}
