//! Core git helpers for simple-git-gui.
//!
//! Reads (status, branches, remotes) prefer `git2`.
//! Network mutations (push/pull) shell out to the `git` CLI so credentials,
//! SSH agents, and hooks behave like a normal developer toolchain.

mod branch;
mod error;
mod find;
mod index_ops;
mod remote;
mod shell;
mod state;
mod status;
mod types;

pub use error::RepoError;
pub use find::find_git_root;
pub use shell::{GitCliError, run_git};
pub use state::RepoState;
pub use types::{BranchInfo, ChangeKind, FileChange, RemoteInfo, RepoSnapshot};