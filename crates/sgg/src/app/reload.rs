//! Background repository load so git status / FS walks stay off the UI thread.

use std::collections::HashSet;
use std::path::PathBuf;

use sgg_core::{RepoSnapshot, RepoState};

use super::files::{list_tree_opts, FsNode};
use super::perf::snapshot_fingerprint;
use super::prefs::absolute_path;

#[derive(Debug)]
pub struct LoadedRepo {
    pub snap: RepoSnapshot,
    pub fingerprint: u64,
    pub files_tree: Vec<FsNode>,
    pub status_line: String,
}

#[derive(Debug)]
pub enum LoadRepoError {
    NoRepo(String),
    Snapshot(String),
}

/// Open nearest repo, take snapshot, optionally walk the files tree.
/// Intended to run on a background executor (git2 is used only on that thread).
pub fn load_repo(
    start: PathBuf,
    expanded_dirs: HashSet<String>,
    build_files_tree: bool,
    check_ignored: bool,
) -> Result<LoadedRepo, LoadRepoError> {
    let repo = RepoState::open_nearest(&start).map_err(|e| LoadRepoError::NoRepo(e.to_string()))?;
    let snap = repo
        .snapshot()
        .map_err(|e| LoadRepoError::Snapshot(e.to_string()))?;
    let root = absolute_path(snap.root.clone());
    let branch = snap
        .head_branch
        .clone()
        .unwrap_or_else(|| "(detached)".into());
    let status_line = format!(
        "{}  ·  {}  ·  {} changes",
        super::prefs::display_path(&root),
        branch,
        snap.changes.len()
    );
    let mut snap = snap;
    snap.root = root.clone();
    let fingerprint = snapshot_fingerprint(&snap);
    let files_tree = if build_files_tree {
        list_tree_opts(&root, &expanded_dirs, check_ignored)
    } else {
        Vec::new()
    };
    Ok(LoadedRepo {
        snap,
        fingerprint,
        files_tree,
        status_line,
    })
}
