use anyhow::{ensure, Context, Result};
use domain::run_carry_forward::{canonical_digest, ContentDigest};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

use super::{
    budget::Deadline,
    git,
    safe_files::{hash_file, identity, validate_relative, SafeRoot},
};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct InventoryLimits {
    pub max_entries: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
}

impl Default for InventoryLimits {
    fn default() -> Self {
        Self {
            max_entries: 50_000,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
            max_file_bytes: 256 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PreviewOptions {
    pub limits: InventoryLimits,
    pub exclusions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkspaceReplacement {
    Directory,
    Ancestor { path: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspaceEntry {
    pub path: String,
    pub sha256: Option<ContentDigest>,
    pub size: u64,
    pub mode: u32,
    pub deleted: bool,
    pub replacement: Option<WorkspaceReplacement>,
    pub excluded: bool,
    pub exclusion_reason: Option<String>,
    pub link_target: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WorkspacePreview {
    pub(super) source: PathBuf,
    pub(super) git_dir: PathBuf,
    pub(super) git_common_dir: PathBuf,
    pub(super) root_identity: (u64, u64),
    pub(super) options: PreviewOptions,
    pub(super) head: String,
    pub(super) entries: Vec<WorkspaceEntry>,
    pub(super) index_sha256: ContentDigest,
    pub(super) staged_sha256: ContentDigest,
    pub(super) unstaged_sha256: ContentDigest,
    pub(super) digest: ContentDigest,
    #[serde(skip)]
    pub(super) index: Vec<u8>,
    #[serde(skip)]
    pub(super) staged: Vec<u8>,
    #[serde(skip)]
    pub(super) unstaged: Vec<u8>,
}

impl WorkspacePreview {
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    pub fn entry(&self, path: &str) -> Option<&WorkspaceEntry> {
        self.entries.iter().find(|entry| entry.path == path)
    }
    pub fn source(&self) -> &Path {
        &self.source
    }
    pub fn head(&self) -> &str {
        &self.head
    }
}

pub struct WorkspacePlanner;

impl WorkspacePlanner {
    /// Read-only local inventory; the caller must keep external writers quiescent during Git access.
    pub async fn preview(source: &Path, options: PreviewOptions) -> Result<WorkspacePreview> {
        Self::preview_with_deadline(source, options, Deadline::after(Duration::from_secs(120)))
            .await
    }

    pub(super) async fn preview_with_deadline(
        source: &Path,
        options: PreviewOptions,
        deadline: Deadline,
    ) -> Result<WorkspacePreview> {
        deadline.check()?;
        let max = InventoryLimits::default();
        ensure!(
            options.limits.max_entries > 0
                && options.limits.max_entries <= max.max_entries
                && options.limits.max_total_bytes > 0
                && options.limits.max_total_bytes <= max.max_total_bytes
                && options.limits.max_file_bytes > 0
                && options.limits.max_file_bytes <= max.max_file_bytes
                && options.exclusions.len() <= 128,
            "continuation_budget_exceeded: limits"
        );
        let source = fs::canonicalize(source)?;
        let root = SafeRoot::open(&source)?;
        let metadata = root.file.metadata()?;
        let root_identity = (metadata.dev(), metadata.ino());
        let config = git::run(&source, &["config", "--null", "--list"], None, deadline).await?;
        for setting in config
            .split(|byte| *byte == 0)
            .filter(|entry| !entry.is_empty())
        {
            let key = setting
                .split(|byte| *byte == b'\n')
                .next()
                .unwrap_or_default();
            let key = String::from_utf8_lossy(key).to_ascii_lowercase();
            ensure!(
                key != "extensions.partialclone"
                    && !key.ends_with(".promisor")
                    && !key.ends_with(".partialclonefilter"),
                "unsupported_frontier: partial clone"
            );
        }
        let top = git::text(&source, &["rev-parse", "--show-toplevel"], deadline).await?;
        ensure!(
            fs::canonicalize(top)? == source,
            "unsafe_path: not repository root"
        );
        let git_dir = fs::canonicalize(
            git::text(&source, &["rev-parse", "--absolute-git-dir"], deadline).await?,
        )?;
        let git_common_dir = fs::canonicalize(
            git::text(
                &source,
                &["rev-parse", "--path-format=absolute", "--git-common-dir"],
                deadline,
            )
            .await?,
        )?;
        let head = git::text(
            &source,
            &["rev-parse", "--verify", "HEAD^{commit}"],
            deadline,
        )
        .await?;
        ensure!(
            [40, 64].contains(&head.len()) && head.bytes().all(|c| c.is_ascii_hexdigit()),
            "unsupported_frontier: missing commit"
        );
        let index_path = PathBuf::from(
            git::text(
                &source,
                &["rev-parse", "--path-format=absolute", "--git-path", "index"],
                deadline,
            )
            .await?,
        );
        ensure!(
            git::text(&source, &["rev-parse", "--shared-index-path"], deadline)
                .await?
                .is_empty(),
            "unsupported_frontier: split index"
        );
        let index = read_index(&index_path)?;
        let flags = git::run(&source, &["ls-files", "-v", "-z"], None, deadline).await?;
        ensure!(
            flags
                .split(|b| *b == 0)
                .filter(|entry| !entry.is_empty())
                .all(|entry| entry.starts_with(b"H ")),
            "unsupported_frontier: sparse or hidden index"
        );
        let tracked_raw = git::run(&source, &["ls-files", "--stage", "-z"], None, deadline).await?;
        let mut tracked = BTreeSet::new();
        for entry in tracked_raw.split(|b| *b == 0).filter(|e| !e.is_empty()) {
            let entry = std::str::from_utf8(entry).context("unsafe_path: non-UTF8 entry")?;
            let (meta, path) = entry.split_once('\t').context("unsafe_path: index entry")?;
            let fields: Vec<_> = meta.split(' ').collect();
            ensure!(
                fields.len() == 3 && fields[2] == "0" && fields[0] != "160000",
                "unsupported_frontier: unmerged index or submodule"
            );
            tracked.insert(path.to_string());
        }
        let mut current_leaves = tracked.clone();
        // The index omits staged deletions and rename origins. Keep their HEAD paths as evidence.
        let tree = git::run(&source, &["ls-tree", "-r", "-z", "HEAD"], None, deadline).await?;
        for entry in tree.split(|b| *b == 0).filter(|entry| !entry.is_empty()) {
            let entry = std::str::from_utf8(entry).context("unsafe_path: non-UTF8 tree entry")?;
            let (meta, path) = entry.split_once('\t').context("unsafe_path: tree entry")?;
            let fields: Vec<_> = meta.split(' ').collect();
            ensure!(
                fields.len() == 3 && fields[1] == "blob" && fields[0] != "160000",
                "unsupported_frontier: submodule"
            );
            tracked.insert(path.to_string());
        }
        let mut paths = tracked.clone();
        let untracked = git::run(
            &source,
            &["ls-files", "--others", "--exclude-standard", "-z"],
            None,
            deadline,
        )
        .await?;
        for path in untracked.split(|b| *b == 0).filter(|p| !p.is_empty()) {
            let path = std::str::from_utf8(path)
                .context("unsafe_path: non-UTF8 entry")?
                .to_string();
            current_leaves.insert(path.clone());
            paths.insert(path);
        }
        for (path, reason) in &options.exclusions {
            validate_relative(path)?;
            ensure!(
                !tracked.contains(path)
                    && machine_path(path)
                    && !reason.trim().is_empty()
                    && reason.len() <= 1024,
                "unsafe_path: invalid machine exclusion"
            );
            paths.insert(path.clone());
        }
        ensure!(
            paths.len() <= options.limits.max_entries,
            "continuation_budget_exceeded: entries"
        );
        let mut input = Vec::new();
        for path in &paths {
            validate_relative(path)?;
            input.extend_from_slice(path.as_bytes());
            input.push(0);
        }
        if !input.is_empty() {
            let attrs = git::run(
                &source,
                &[
                    "check-attr",
                    "-z",
                    "--stdin",
                    "filter",
                    "working-tree-encoding",
                ],
                Some(&input),
                deadline,
            )
            .await?;
            for triple in attrs
                .split(|b| *b == 0)
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .chunks(3)
            {
                ensure!(
                    triple.len() == 3
                        && [b"unspecified".as_slice(), b"unset".as_slice()].contains(&triple[2]),
                    "unsupported_frontier: filter or encoding"
                );
            }
        }
        let mut entries = Vec::new();
        let mut total = index.len() as u64;
        ensure!(
            total <= options.limits.max_total_bytes,
            "continuation_budget_exceeded: preservation"
        );
        for path in paths {
            deadline.check()?;
            let excluded = options.exclusions.contains_key(&path);
            ensure!(
                excluded || !protected_path(&path),
                "unsafe_path: runtime or credential material"
            );
            let mut replacement = None;
            for (separator, _) in path.match_indices('/') {
                let ancestor = &path[..separator];
                if current_leaves.contains(ancestor)
                    && root.inspect(ancestor)?.is_some_and(|info| {
                        [libc::S_IFREG as u32, libc::S_IFLNK as u32]
                            .contains(&(info.mode & libc::S_IFMT as u32))
                    })
                {
                    replacement = Some(WorkspaceReplacement::Ancestor {
                        path: ancestor.into(),
                    });
                    break;
                }
            }
            let info = if replacement.is_some() {
                None
            } else {
                root.inspect(&path)?
            };
            if tracked.contains(&path)
                && info
                    .as_ref()
                    .is_some_and(|info| info.mode & libc::S_IFMT as u32 == libc::S_IFDIR as u32)
            {
                replacement = Some(WorkspaceReplacement::Directory);
            }
            let mut entry = WorkspaceEntry {
                path: path.clone(),
                sha256: None,
                size: 0,
                mode: 0,
                deleted: info.is_none() || replacement.is_some(),
                replacement,
                excluded,
                exclusion_reason: options.exclusions.get(&path).cloned(),
                link_target: None,
            };
            if entry.replacement.is_some() {
                entries.push(entry);
                continue;
            }
            if let Some(ref info) = info {
                if !excluded {
                    total = total
                        .checked_add(info.size)
                        .context("continuation_budget_exceeded: preservation")?;
                    ensure!(
                        total <= options.limits.max_total_bytes,
                        "continuation_budget_exceeded: preservation"
                    );
                }
                entry.mode = info.mode & 0o777;
                entry.size = info.size;
                ensure!(
                    info.mode & 0o7000 == 0,
                    "unsafe_path: special permission bits"
                );
                match info.mode & libc::S_IFMT as u32 {
                    x if x == libc::S_IFLNK as u32 => {
                        let target = root.read_link(&path)?;
                        if !excluded {
                            validate_link(&root, &path, &target)?;
                            entry.sha256 = Some(ContentDigest::of(target.as_bytes()));
                        }
                        entry.link_target = Some(target);
                    }
                    x if x == libc::S_IFREG as u32 => {
                        if !excluded {
                            entry.sha256 = Some(hash_file(
                                root.open_file(&path)?,
                                options.limits.max_file_bytes,
                                deadline,
                            )?);
                        }
                    }
                    _ => anyhow::bail!("unsafe_path: special file or nested repository"),
                }
                ensure!(
                    Some(info) == root.inspect(&path)?.as_ref(),
                    "source_changed: replaced path"
                );
            }
            entries.push(entry);
        }
        let staged = git::run(
            &source,
            &[
                "diff",
                "--cached",
                "--binary",
                "--no-ext-diff",
                "--no-textconv",
                "HEAD",
                "--",
            ],
            None,
            deadline,
        )
        .await?;
        let unstaged = git::run(
            &source,
            &["diff", "--binary", "--no-ext-diff", "--no-textconv", "--"],
            None,
            deadline,
        )
        .await?;
        total += (staged.len() + unstaged.len()) as u64;
        ensure!(
            total <= options.limits.max_total_bytes,
            "continuation_budget_exceeded: preservation"
        );
        ensure!(
            index == read_index(&index_path)?
                && head
                    == git::text(
                        &source,
                        &["rev-parse", "--verify", "HEAD^{commit}"],
                        deadline
                    )
                    .await?,
            "source_changed: Git witness"
        );
        let current = fs::symlink_metadata(&source)?;
        ensure!(
            (current.dev(), current.ino()) == root_identity && current.is_dir(),
            "source_changed: root"
        );
        let index_sha256 = ContentDigest::of(&index);
        let staged_sha256 = ContentDigest::of(&staged);
        let unstaged_sha256 = ContentDigest::of(&unstaged);
        let digest = canonical_digest(
            &serde_json::json!({"schema_version":"run_carry_forward_workspace_v1",
            "source":source,"git_dir":git_dir,"git_common_dir":git_common_dir,
            "root_identity":root_identity,"options":options,"head":head,"entries":entries,
            "index_sha256":index_sha256,"staged_sha256":staged_sha256,"unstaged_sha256":unstaged_sha256}),
        )?;
        deadline.check()?;
        Ok(WorkspacePreview {
            source,
            git_dir,
            git_common_dir,
            root_identity,
            options,
            head,
            entries,
            index_sha256,
            staged_sha256,
            unstaged_sha256,
            digest,
            index,
            staged,
            unstaged,
        })
    }
}

fn read_index(path: &Path) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = file.metadata()?;
    ensure!(
        before.is_file() && before.len() <= git::OUTPUT_LIMIT as u64,
        "continuation_budget_exceeded: index"
    );
    let mut bytes = Vec::new();
    let mut reader = file.take(git::OUTPUT_LIMIT as u64 + 1);
    reader.read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= git::OUTPUT_LIMIT
            && identity(&before) == identity(&reader.get_ref().metadata()?),
        "source_changed: index during read"
    );
    Ok(bytes)
}

fn machine_path(path: &str) -> bool {
    [
        ".antigravitycli/",
        "DerivedData/",
        ".build/",
        "node_modules/",
        "target/",
        ".chainworks/",
    ]
    .iter()
    .any(|prefix| path.starts_with(prefix))
        || path == ".DS_Store"
}

pub(super) fn protected_path(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part,
            ".git" | ".chainworks" | ".codex" | ".claude" | ".forge-codex-acp" | "secrets"
        ) || part.starts_with(".env")
    })
}

fn validate_link(root: &SafeRoot, path: &str, target: &str) -> Result<()> {
    ensure!(
        !target.is_empty() && !target.starts_with('/'),
        "unsafe_path: external link"
    );
    let mut parts: Vec<_> = path.split('/').collect();
    parts.pop();
    for component in target.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                ensure!(parts.pop().is_some(), "unsafe_path: escaping link");
            }
            other => {
                parts.push(other);
                let prefix = parts.join("/");
                ensure!(
                    !protected_path(&prefix) && !machine_path(&format!("{prefix}/")),
                    "unsafe_path: protected link target"
                );
                // Reject every link hop before interpreting subsequent '..' components.
                if let Some(info) = root.inspect(&prefix)? {
                    ensure!(
                        info.mode & libc::S_IFMT as u32 != libc::S_IFLNK as u32,
                        "unsafe_path: chained link"
                    );
                }
            }
        }
    }
    ensure!(
        !parts.is_empty() && !protected_path(&parts.join("/")),
        "unsafe_path: protected link target"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::budget::Deadline;
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn expired_preview_stops_before_any_filesystem_or_git_access() {
        let error = WorkspacePlanner::preview_with_deadline(
            Path::new("/does-not-exist-p039"),
            PreviewOptions::default(),
            Deadline::after(Duration::ZERO),
        )
        .await
        .unwrap_err();
        assert_eq!(error.to_string(), "continuation_budget_exceeded: deadline");
    }
}
