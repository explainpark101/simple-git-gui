//! Git context for AI-assisted commit message generation.
//!
//! Prefers staged (`--cached`) diff; falls back to working-tree diff when the
//! index is empty (Zed-style). Caps patch size so prompts stay bounded.

use std::path::Path;

use crate::shell::{run_git, GitCliError};

const MAX_PATCH_BYTES: usize = 40_000;
const MAX_SUMMARY_BYTES: usize = 8_000;
const RECENT_SUBJECT_COUNT: usize = 8;

#[derive(Debug, Clone)]
pub struct CommitMessageContext {
    pub summary: String,
    pub patch: String,
    pub recent_subjects: Vec<String>,
    pub branch: Option<String>,
    pub user_hint: Option<String>,
    /// True when the patch came from `git diff --cached`.
    pub used_staged: bool,
}

impl CommitMessageContext {
    pub fn is_empty(&self) -> bool {
        self.summary.trim().is_empty() && self.patch.trim().is_empty()
    }
}

/// Build prompt inputs from the repo working tree / index.
pub fn prepare_commit_context(
    repo: &Path,
    user_hint: Option<&str>,
) -> Result<CommitMessageContext, GitCliError> {
    let staged_summary = run_git(repo, &["diff", "--cached", "--name-status"]).unwrap_or_default();
    let staged_patch = run_git(repo, &["diff", "--cached", "--no-color"]).unwrap_or_default();

    let (summary_raw, patch_raw, used_staged) =
        if !staged_summary.trim().is_empty() || !staged_patch.trim().is_empty() {
            (staged_summary, staged_patch, true)
        } else {
            let summary = run_git(repo, &["diff", "--name-status"]).unwrap_or_default();
            let patch = run_git(repo, &["diff", "--no-color"]).unwrap_or_default();
            // Untracked files never appear in `git diff`; append as name-status adds.
            let untracked = run_git(repo, &["ls-files", "--others", "--exclude-standard"])
                .unwrap_or_default();
            let mut summary = summary;
            for path in untracked.lines() {
                let path = path.trim();
                if path.is_empty() {
                    continue;
                }
                if !summary.is_empty() {
                    summary.push('\n');
                }
                summary.push_str("A\t");
                summary.push_str(path);
            }
            (summary, patch, false)
        };

    let recent = run_git(
        repo,
        &[
            "log",
            &format!("-n{RECENT_SUBJECT_COUNT}"),
            "--pretty=format:%s",
        ],
    )
    .unwrap_or_default();
    let recent_subjects = recent
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    let branch = run_git(repo, &["branch", "--show-current"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    Ok(CommitMessageContext {
        summary: truncate_utf8(&summary_raw, MAX_SUMMARY_BYTES),
        patch: truncate_utf8(&patch_raw, MAX_PATCH_BYTES),
        recent_subjects,
        branch,
        user_hint: user_hint
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        used_staged,
    })
}

fn truncate_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = s[..end].to_string();
    out.push_str("\n\n…[truncated]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_respects_char_boundary() {
        let s = "한글테스트메시지";
        let out = truncate_utf8(s, 5);
        assert!(out.contains("truncated") || out.len() <= 5 + "…[truncated]".len());
    }
}
