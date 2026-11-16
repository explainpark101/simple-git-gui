//! Core git helpers for simple-git-gui.
//!
//! Reads (status, branches, remotes) prefer `git2`.
//! Network mutations (push/pull) shell out to the `git` CLI so credentials,
//! SSH agents, and hooks behave like a normal developer toolchain.

mod branch;
mod error;
mod find;
mod graph;
mod history;
mod index_ops;
mod line_diff;
mod remote;
mod shell;
mod state;
mod status;
mod types;

pub use error::RepoError;
pub use find::find_git_root;
pub use graph::{
    build_history_graph, is_uncommitted_oid, CommitLine, CommitLineSegment, CurveKind, GraphCommit,
    GraphCommitInput, HistoryGraph, GRAPH_ACCENT_COUNT, UNCOMMITTED_OID,
};
pub use history::{
    blame_file, blame_file_at, diff_file_at_commit, list_history, reset_to, rev_parse, show_commit,
    show_file_at_revision, show_uncommitted, split_file_at_commit, BlameEntry, HistoryCommit,
    HistoryDetail, HistoryFileChange, HistoryRevSpec, ResetMode,
};
pub use line_diff::{
    align_split_from_unified, line_start_offset, lines_byte_range, list_file_line_changes,
    parse_unified_diff_hunks, split_diff_marks_from_unified, unified_diff_line_marks,
    AlignedSplitLine, AlignedSplitView, LineChange, LineChangeKind, SplitDiffMarks,
    UnifiedDiffLineKind,
};
pub use shell::{GitCliError, run_git};
pub use state::RepoState;
pub use types::{
    BranchInfo, ChangeKind, FileChange, RemoteInfo, RepoOperation, RepoSnapshot,
};