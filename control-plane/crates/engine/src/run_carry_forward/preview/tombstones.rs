use super::*;
use sha2::{Digest, Sha256};
use std::process::Stdio;
use tokio::io::AsyncReadExt;

struct Blob {
    oid: String,
    mode: u32,
}

/// A deletion has no current file bytes. Bind its presentation hash to an actual
/// historical Git blob; the unchanged workspace tombstone remains authoritative
/// for materialization. Nothing here restores the deleted file or writes Git.
pub(super) async fn bind_preimages(
    workspace: &WorkspacePreview,
    entries: &mut [PlanEntry],
    deadline: Deadline,
) -> anyhow::Result<()> {
    if !entries.iter().any(|entry| {
        entry
            .workspace_entry
            .as_ref()
            .is_some_and(|file| file.deleted && !file.excluded)
    }) {
        return Ok(());
    }
    let head = super::super::git::run(
        &workspace.source,
        &["ls-tree", "-r", "-z", &workspace.head],
        None,
        deadline,
    )
    .await?;
    let index = super::super::git::run(
        &workspace.source,
        &["ls-files", "--stage", "-z"],
        None,
        deadline,
    )
    .await?;
    let head = blobs(&head, true)?;
    let index = blobs(&index, false)?;
    let mut total = entries
        .iter()
        .try_fold(0u64, |sum, entry| sum.checked_add(entry.bytes))
        .context("continuation_budget_exceeded")?;
    for entry in entries {
        if !entry
            .workspace_entry
            .as_ref()
            .is_some_and(|file| file.deleted && !file.excluded)
        {
            continue;
        }
        deadline.check()?;
        let (blob, contract) = if let Some(blob) = head.get(&entry.key.relative_path) {
            (blob, "git_head_blob")
        } else {
            (
                index
                    .get(&entry.key.relative_path)
                    .context("source_changed: deletion has no observable preimage")?,
                "git_index_blob",
            )
        };
        let size: u64 =
            super::super::git::text(&workspace.source, &["cat-file", "-s", &blob.oid], deadline)
                .await?
                .parse()
                .context("source_changed: historical blob size")?;
        ensure!(
            size <= InventoryLimits::default().max_file_bytes,
            "continuation_budget_exceeded: historical blob"
        );
        total = total
            .checked_add(size)
            .context("continuation_budget_exceeded")?;
        ensure!(
            total <= InventoryLimits::default().max_total_bytes,
            "continuation_budget_exceeded: historical preimages"
        );
        entry.sha256 = Some(hash_blob(&workspace.source, &blob.oid, size, deadline).await?);
        entry.bytes = size;
        entry.mode = blob.mode;
        entry.logical_name = Some("workspace_deleted_path".into());
        entry.target_logical_name = Some("workspace_absent_file".into());
        entry.source_contract = Some(contract.into());
        entry.source_schema_version = Some("workspace_deletion_preimage_v1".into());
        entry.reason = "preserve_workspace_deletion".into();
    }
    Ok(())
}

fn blobs(bytes: &[u8], head: bool) -> anyhow::Result<BTreeMap<String, Blob>> {
    let mut result = BTreeMap::new();
    for raw in bytes.split(|byte| *byte == 0).filter(|raw| !raw.is_empty()) {
        let text = std::str::from_utf8(raw).context("unsafe_path: historical path encoding")?;
        let (metadata, path) = text
            .split_once('\t')
            .context("source_changed: Git blob record")?;
        validate_relative(path)?;
        let fields: Vec<_> = metadata.split(' ').collect();
        ensure!(
            fields.len() == 3
                && if head {
                    fields[1] == "blob"
                } else {
                    fields[2] == "0"
                },
            "unsupported_frontier: historical Git entry"
        );
        let oid = fields[if head { 2 } else { 1 }];
        ensure!(
            [40, 64].contains(&oid.len())
                && oid
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                && ["100644", "100755", "120000"].contains(&fields[0]),
            "unsupported_frontier: historical blob identity"
        );
        ensure!(
            result
                .insert(
                    path.into(),
                    Blob {
                        oid: oid.into(),
                        mode: u32::from_str_radix(fields[0], 8)?
                    }
                )
                .is_none(),
            "source_changed: duplicate Git path"
        );
        ensure!(
            result.len() <= 50_000,
            "continuation_budget_exceeded: historical inventory"
        );
    }
    Ok(result)
}

async fn hash_blob(
    root: &Path,
    oid: &str,
    expected_size: u64,
    deadline: Deadline,
) -> anyhow::Result<ContentDigest> {
    let remaining = deadline.remaining()?.min(Duration::from_secs(30));
    // cat-file has no working-tree filters. Disable network hydration, hooks,
    // maintenance, external config and optional locks just as the core Git reader.
    let mut child = tokio::process::Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("LC_ALL", "C")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.file.allow=never",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "cat-file",
            "blob",
            oid,
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("source_changed: historical blob reader")?;
    let mut output = child
        .stdout
        .take()
        .context("source_changed: historical blob output")?;
    let result = tokio::time::timeout(remaining, async {
        let mut buffer = vec![0; 1024 * 1024];
        let mut hasher = Sha256::new();
        let mut bytes = 0u64;
        loop {
            deadline.check()?;
            let count = output.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            bytes += count as u64;
            ensure!(
                bytes <= expected_size,
                "source_changed: historical blob length"
            );
            hasher.update(&buffer[..count]);
        }
        ensure!(
            child.wait().await?.success() && bytes == expected_size,
            "source_changed: historical blob unavailable"
        );
        Ok::<_, anyhow::Error>(ContentDigest::from_sha256_hex(&format!(
            "{:x}",
            hasher.finalize()
        ))?)
    })
    .await;
    match result {
        Ok(Ok(digest)) => Ok(digest),
        failed => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match failed {
                Ok(Err(error)) => Err(error),
                _ => anyhow::bail!("continuation_budget_exceeded: historical blob deadline"),
            }
        }
    }
}
