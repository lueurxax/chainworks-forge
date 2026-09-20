use anyhow::{ensure, Context, Result};
use domain::execution_root::{select_execution_root, ExecutionRootKind, ResolvedExecutionRoot};
use std::path::Path;

pub fn resolve_execution_root(
    repository: &str,
    worktree: Option<&str>,
    write_enabled: bool,
    strategy: Option<&str>,
) -> Result<ResolvedExecutionRoot> {
    let selected = select_execution_root(repository, worktree, write_enabled, strategy)?;
    let repository = canonical_directory(repository)?;
    let effective = canonical_directory(selected)?;
    let (device, inode) = directory_identity(Path::new(&effective))?;
    Ok(ResolvedExecutionRoot {
        version: 1,
        kind: if effective == repository {
            ExecutionRootKind::Repository
        } else {
            ExecutionRootKind::Worktree
        },
        repository,
        effective,
        device,
        inode,
        strategy: strategy.map(str::to_owned),
    })
}

pub fn revalidate_execution_root(root: &ResolvedExecutionRoot) -> Result<()> {
    ensure!(root.version == 1, "execution_root_version");
    ensure!(
        canonical_directory(&root.repository)? == root.repository,
        "execution_root_changed"
    );
    ensure!(
        canonical_directory(&root.effective)? == root.effective,
        "execution_root_changed"
    );
    let (device, inode) = directory_identity(Path::new(&root.effective))?;
    ensure!(
        device == root.device && inode == root.inode,
        "execution_root_changed"
    );
    ensure!(
        (root.kind == ExecutionRootKind::Repository) == (root.repository == root.effective),
        "execution_root_kind"
    );
    Ok(())
}

fn canonical_directory(path: &str) -> Result<String> {
    ensure!(Path::new(path).is_absolute(), "execution_root_not_absolute");
    let canonical = std::fs::canonicalize(path).context("missing_execution_root")?;
    ensure!(canonical.is_dir(), "execution_root_not_directory");
    canonical
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow::anyhow!("execution_root_not_utf8"))
}

#[cfg(unix)]
fn directory_identity(path: &Path) -> Result<(String, String)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path).context("execution_root_identity")?;
    ensure!(metadata.is_dir(), "execution_root_not_directory");
    Ok((metadata.dev().to_string(), metadata.ino().to_string()))
}

#[cfg(not(unix))]
fn directory_identity(_path: &Path) -> Result<(String, String)> {
    anyhow::bail!("execution_root_identity_unsupported")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_new_uses_shared_worktree_and_rejects_missing_required_root() {
        let mut req: crate::ExecutionRequest = serde_json::from_value(serde_json::json!({
            "run_id": domain::ids::RunId::new(), "stage_id": "s", "agent_id": "a",
            "provider": "fixture", "workspace_root": "/repo", "prompt": "read",
            "worktree_root": "/tree", "worktree_write_enabled": false,
            "worktree_strategy": "shared_implementation_worktree"
        }))
        .unwrap();
        let config = crate::transport::AcpSessionConfig::default();
        let params = crate::transport::build_session_new_params(&req, &config).unwrap();
        assert_eq!(params["cwd"], "/tree");
        req.worktree_root = None;
        assert!(crate::transport::build_session_new_params(&req, &config).is_err());
    }

    #[test]
    fn pinned_root_detects_replacement_and_missing_worktree() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let tree = temp.path().join("tree");
        std::fs::create_dir(&repo).unwrap();
        std::fs::create_dir(&tree).unwrap();
        let root = resolve_execution_root(
            repo.to_str().unwrap(),
            Some(tree.to_str().unwrap()),
            false,
            Some("dedicated"),
        )
        .unwrap();
        assert_eq!(
            root.effective,
            tree.canonicalize().unwrap().to_str().unwrap()
        );
        revalidate_execution_root(&root).unwrap();
        std::fs::rename(&tree, temp.path().join("old-tree")).unwrap();
        std::fs::create_dir(&tree).unwrap();
        assert!(revalidate_execution_root(&root).is_err());
        assert!(
            resolve_execution_root(repo.to_str().unwrap(), None, false, Some("dedicated")).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn root_canonical_alias_is_an_observation_not_a_second_root() {
        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");
        let alias = temp.path().join("alias");
        std::fs::create_dir(&repo).unwrap();
        std::os::unix::fs::symlink(&repo, &alias).unwrap();
        let root = resolve_execution_root(alias.to_str().unwrap(), None, false, None).unwrap();
        assert_eq!(
            root.effective,
            repo.canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(root.kind, ExecutionRootKind::Repository);
        revalidate_execution_root(&root).unwrap();
    }
}
