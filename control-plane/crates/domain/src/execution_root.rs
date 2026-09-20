use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExecutionRootError {
    #[error("missing_required_worktree: the frozen strategy requires a worktree")]
    MissingRequiredWorktree,
    #[error("missing_execution_root: the selected path is empty")]
    EmptyRoot,
}

pub fn select_execution_root<'a>(
    repository: &'a str,
    worktree: Option<&'a str>,
    write_enabled: bool,
    strategy: Option<&str>,
) -> Result<&'a str, ExecutionRootError> {
    let required = matches!(
        strategy,
        Some("dedicated" | "shared_implementation_worktree")
    );
    let selected = if required {
        worktree
            .filter(|p| !p.is_empty())
            .ok_or(ExecutionRootError::MissingRequiredWorktree)?
    } else if write_enabled {
        worktree.unwrap_or(repository)
    } else {
        repository
    };
    if selected.is_empty() {
        return Err(ExecutionRootError::EmptyRoot);
    }
    Ok(selected)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionRootKind {
    Repository,
    Worktree,
}

/// Filesystem observation, not a permission or verified Xcode workspace binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedExecutionRoot {
    pub version: u32,
    pub repository: String,
    pub effective: String,
    pub device: String,
    pub inode: String,
    pub strategy: Option<String>,
    pub kind: ExecutionRootKind,
}
