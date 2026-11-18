//! Git context for AI-assisted commit message generation.
//!
//! - If anything is staged → use **only** the index (`git diff --cached`).
//! - If the index is empty → use **unstaged + untracked** (working tree).
//!
//! Caps patch size so prompts stay bounded. Staging in the GUI uses libgit2;
//! this module reads via `git` CLI and also consults `git status --porcelain`.

use std::fs;
use std::path::Path;

use crate::shell::{run_git, GitCliError};

const MAX_PATCH_BYTES: usize = 40_000;
const MAX_SUMMARY_BYTES: usize = 8_000;
const RECENT_SUBJECT_COUNT: usize = 8;
const MAX_UNTRACKED_FILE_BYTES: u64 = 64_000;

#[derive(Debug, Clone)]
pub struct CommitMessageContext {
    pub summary: String,
    pub patch: String,
    pub recent_subjects: Vec<String>,
    pub branch: Option<String>,
    pub user_hint: Option<String>,
    /// True when the patch came from `git diff --cached` (staged only).
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
    let porcelain = git_ok(repo, &["status", "--porcelain", "-uall"]);
    let (has_staged, has_unstaged, has_untracked) = classify_porcelain(&porcelain);

    let (summary_raw, patch_raw, used_staged) = if has_staged {
        // Staged present → ignore unstaged / untracked entirely.
        let mut summary = git_ok(repo, &["diff", "--cached", "--name-status"]);
        let patch = git_ok(repo, &["diff", "--cached", "--no-color"]);
        if summary.trim().is_empty() {
            summary = summary_from_porcelain(&porcelain, true);
        }
        (summary, patch, true)
    } else {
        // Nothing staged → unstaged tracked changes + untracked files.
        let mut summary = git_ok(repo, &["diff", "--name-status"]);
        let mut patch = git_ok(repo, &["diff", "--no-color"]);
        let untracked = git_ok(repo, &["ls-files", "--others", "--exclude-standard"]);
        for path in untracked.lines() {
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            push_name_status(&mut summary, 'A', path);
        }
        if summary.trim().is_empty() && (has_unstaged || has_untracked) {
            summary = summary_from_porcelain(&porcelain, false);
        }
        append_untracked_patches(repo, &mut patch, &untracked);
        // No HEAD yet: plain `git diff` may be empty; still prefer working-tree porcelain.
        if summary.trim().is_empty() && patch.trim().is_empty() && !porcelain.trim().is_empty() {
            summary = summary_from_porcelain(&porcelain, false);
        }
        (summary, patch, false)
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

/// Append small text untracked files into the patch so the model can read them.
fn append_untracked_patches(repo: &Path, patch: &mut String, untracked: &str) {
    for path in untracked.lines() {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        if patch.len() >= MAX_PATCH_BYTES {
            break;
        }
        let full = repo.join(path);
        let Ok(meta) = fs::metadata(&full) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if !patch.is_empty() && !patch.ends_with('\n') {
            patch.push('\n');
        }
        if meta.len() > MAX_UNTRACKED_FILE_BYTES {
            patch.push_str(&format!(
                "diff --git a/{path} b/{path}\nnew file (skipped, {bytes} bytes)\n",
                bytes = meta.len()
            ));
            continue;
        }
        let Ok(bytes) = fs::read(&full) else {
            continue;
        };
        if bytes.contains(&0) {
            patch.push_str(&format!(
                "diff --git a/{path} b/{path}\nnew file (binary, skipped)\n"
            ));
            continue;
        }
        let text = String::from_utf8_lossy(&bytes);
        patch.push_str(&format!(
            "diff --git a/{path} b/{path}\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/{path}\n"
        ));
        for line in text.lines() {
            if patch.len() >= MAX_PATCH_BYTES {
                patch.push_str("\n…[truncated]\n");
                return;
            }
            patch.push('+');
            patch.push_str(line);
            patch.push('\n');
        }
        if text.ends_with('\n') {
            // already line-split; OK
        } else if !text.is_empty() && !patch.ends_with('\n') {
            patch.push('\n');
        }
    }
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
            // Working-tree view: unstaged letter preferred; skip index-only lines.
            if y != ' ' && y != '?' {
                push_name_status(&mut out, status_letter(y), path);
            }
            // When nothing is staged overall we still shouldn't list index-only rows,
            // but include them only if there is no unstaged side (defensive).
            else if x != ' ' && x != '?' {
                push_name_status(&mut out, status_letter(x), path);
            }
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

    #[test]
    fn summary_staged_only_skips_untracked() {
        let s = summary_from_porcelain("M  a.rs\n?? b.rs\n", true);
        assert!(s.contains("a.rs"));
        assert!(!s.contains("b.rs"));
    }

    #[test]
    fn summary_working_tree_includes_untracked() {
        let s = summary_from_porcelain(" M a.rs\n?? b.rs\n", false);
        assert!(s.contains("a.rs"));
        assert!(s.contains("b.rs"));
    }
}
