//! Git context for AI-assisted commit message generation.
//!
//! Prefers staged (`--cached`) diff; falls back to working-tree diff when the
//! index is empty (Zed-style). Caps patch size so prompts stay bounded.
//!
//! Staging in the GUI uses libgit2; this module reads via `git` CLI. We also
//! consult `git status --porcelain` so index writes are not missed when a plain
//! `diff --cached` comes back empty.

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
    // Prefer porcelain as the change detector — matches what the UI shows even
    // when libgit2 staged the index and a bare `diff --cached` is briefly empty.
    let porcelain = git_ok(repo, &["status", "--porcelain", "-uall"]);
    let (has_staged, has_unstaged, has_untracked) = classify_porcelain(&porcelain);

    let staged_summary = git_ok(repo, &["diff", "--cached", "--name-status"]);
    let staged_patch = git_ok(repo, &["diff", "--cached", "--no-color"]);
    let staged_nonempty =
        !staged_summary.trim().is_empty() || !staged_patch.trim().is_empty() || has_staged;

    let (summary_raw, patch_raw, used_staged) = if staged_nonempty {
        let summary = if staged_summary.trim().is_empty() {
            summary_from_porcelain(&porcelain, true)
        } else {
            staged_summary
        };
        (summary, staged_patch, true)
    } else {
        let mut summary = git_ok(repo, &["diff", "--name-status"]);
        let patch = git_ok(repo, &["diff", "--no-color"]);
        let untracked = git_ok(repo, &["ls-files", "--others", "--exclude-standard"]);
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
        if summary.trim().is_empty() && (has_unstaged || has_untracked) {
            summary = summary_from_porcelain(&porcelain, false);
        }
        // No HEAD yet: `git diff` fails; fall back to porcelain / cached.
        if summary.trim().is_empty() && patch.trim().is_empty() && !porcelain.trim().is_empty() {
            let summary = summary_from_porcelain(&porcelain, false);
            let patch = git_ok(repo, &["diff", "--cached", "--no-color"]);
            (summary, patch, has_staged)
        } else {
            (summary, patch, false)
        }
    };

    let recent = git_ok(
        repo,
        &[
            "log",
            &format!("-n{RECENT_SUBJECT_COUNT}"),
            "--pretty=format:%s",
        ],
    );
    let recent_subjects = recent
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    let branch = git_ok(repo, &["branch", "--show-current"]);
    let branch = {
        let t = branch.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    };

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

fn git_ok(repo: &Path, args: &[&str]) -> String {
    run_git(repo, args).unwrap_or_default()
}

/// XY path… lines from `git status --porcelain`.
fn classify_porcelain(porcelain: &str) -> (bool, bool, bool) {
    let mut staged = false;
    let mut unstaged = false;
    let mut untracked = false;
    for line in porcelain.lines() {
        let b = line.as_bytes();
        if b.len() < 3 {
            continue;
        }
        let x = b[0] as char;
        let y = b[1] as char;
        if x == '?' && y == '?' {
            untracked = true;
            continue;
        }
        if x != ' ' && x != '?' {
            staged = true;
        }
        if y != ' ' && y != '?' {
            unstaged = true;
        }
    }
    (staged, unstaged, untracked)
}

fn summary_from_porcelain(porcelain: &str, staged_only: bool) -> String {
    let mut out = String::new();
    for line in porcelain.lines() {
        let b = line.as_bytes();
        if b.len() < 4 {
            continue;
        }
        let x = b[0] as char;
        let y = b[1] as char;
        let path = line[3..].trim();
        if path.is_empty() {
            continue;
        }
        if x == '?' && y == '?' {
            if staged_only {
                continue;
            }
            push_name_status(&mut out, 'A', path);
            continue;
        }
        if staged_only {
            if x == ' ' || x == '?' {
                continue;
            }
            push_name_status(&mut out, status_letter(x), path);
        } else {
            let letter = if y != ' ' && y != '?' {
                status_letter(y)
            } else if x != ' ' && x != '?' {
                status_letter(x)
            } else {
                continue;
            };
            push_name_status(&mut out, letter, path);
        }
    }
    out
}

fn status_letter(c: char) -> char {
    match c {
        'A' | 'C' | 'D' | 'M' | 'R' | 'T' | 'U' => c,
        _ => 'M',
    }
}

fn push_name_status(out: &mut String, letter: char, path: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    out.push(letter);
    out.push('\t');
    out.push_str(path);
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

    #[test]
    fn classifies_staged_line() {
        let (s, u, t) = classify_porcelain("M  src/main.rs\n");
        assert!(s && !u && !t);
    }

    #[test]
    fn classifies_untracked() {
        let (s, u, t) = classify_porcelain("?? new.txt\n");
        assert!(!s && !u && t);
    }
}
