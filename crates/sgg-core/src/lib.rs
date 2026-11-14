//! Core git helpers for simple-git-gui.
//!
//! Reads (status, branches, remotes) prefer `git2`.
//! Network mutations (push/pull) shell out to the `git` CLI so credentials,
//! SSH agents, and hooks behave like a normal developer toolchain.

mod repo;
mod shell;

pub use repo::{
    BranchInfo, ChangeKind, FileChange, RemoteInfo, RepoError, RepoSnapshot, RepoState, find_git_root,
};
pub use shell::{GitCliError, run_git};