use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChangeKind {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    pub status_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
    /// Tip commit oid (`None` if the ref does not resolve).
    #[serde(default)]
    pub tip_oid: Option<String>,
    /// Configured or inferred upstream (e.g. `origin/main`). `None` if no remote branch.
    pub upstream: Option<String>,
    /// Commits on this branch not in upstream (needs push). Meaningful when `upstream` is set.
    pub ahead: usize,
    /// Commits on upstream not in this branch (needs pull). Meaningful when `upstream` is set.
    pub behind: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: Option<String>,
}

/// In-progress git operation (rebase / merge / …), if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RepoOperation {
    #[default]
    Clean,
    Merge,
    Rebase,
    CherryPick,
    Revert,
    Bisect,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub root: PathBuf,
    pub head_branch: Option<String>,
    pub changes: Vec<FileChange>,
    pub branches: Vec<BranchInfo>,
    pub remotes: Vec<RemoteInfo>,
    pub operation: RepoOperation,
}