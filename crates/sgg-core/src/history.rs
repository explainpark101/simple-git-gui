//! Commit history and blame helpers (git CLI).

use std::path::Path;

use crate::graph::{is_uncommitted_oid, UNCOMMITTED_OID};
use crate::shell::{run_git, GitCliError};

const HISTORY_LIMIT: usize = 200;

#[derive(Debug, Clone)]
pub struct HistoryCommit {
    pub full_oid: String,
    pub short_oid: String,
    pub subject: String,
    pub author: String,
    pub email: String,
    pub when: String,
    /// Parent full OIDs (first parent first). Used for the commit graph.
    pub parents: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct HistoryFileChange {
    pub status: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct HistoryDetail {
    pub commit: HistoryCommit,
    pub body: String,
    pub parents: Vec<String>,
    pub files: Vec<HistoryFileChange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetMode {
    Soft,
    Mixed,
    Hard,
}

impl ResetMode {
    pub fn as_git_arg(self) -> &'static str {
        match self {
            Self::Soft => "--soft",
            Self::Mixed => "--mixed",
            Self::Hard => "--hard",
        }
    }
}

/// One line of `git blame` for a working-tree file (1-based line → index `line - 1`).
#[derive(Debug, Clone)]
pub struct BlameEntry {
    pub short_oid: String,
    pub author: String,
    pub summary: String,
    pub when: String,
}

/// Resolve a revision to a full oid (`git rev-parse`).
pub fn rev_parse(repo: &Path, rev: &str) -> Result<String, GitCliError> {
    let out = run_git(repo, &["rev-parse", rev])?;
    Ok(out.trim().to_string())
}

/// Which tips feed [`list_history`].
#[derive(Debug, Clone, Copy)]
pub enum HistoryRevSpec<'a> {
    /// Every ref (`git log --all`) — local and remote branches.
    All,
    /// One or more branch / ref names (e.g. `main`, `origin/main`).
    Tips(&'a [String]),
}

/// Topo-ordered history (newest first), including parents for the graph.
///
/// Remote-tracking names work as tips. Empty [`HistoryRevSpec::Tips`] yields
/// an empty list (caller chose no branches).
pub fn list_history(
    repo: &Path,
    rev: HistoryRevSpec<'_>,
) -> Result<Vec<HistoryCommit>, GitCliError> {
    let limit = format!("-n{HISTORY_LIMIT}");
    let pretty = "--pretty=format:%H%x00%h%x00%s%x00%an%x00%ae%x00%cr%x00%P";
    let mut args: Vec<&str> = vec!["log"];
    match rev {
        HistoryRevSpec::All => {
            args.push("--all");
        }
        HistoryRevSpec::Tips(tips) => {
            if tips.is_empty() {
                return Ok(Vec::new());
            }
            for tip in tips {
                args.push(tip.as_str());
            }
        }
    }
    args.push("--topo-order");
    args.push(&limit);
    args.push(pretty);
    let raw = run_git(repo, &args)?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for line in raw.split('\n') {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(7, '\0');
        let Some(full_oid) = parts.next() else { continue };
        let Some(short_oid) = parts.next() else { continue };
        let Some(subject) = parts.next() else { continue };
        let Some(author) = parts.next() else { continue };
        let Some(email) = parts.next() else { continue };
        let when = parts.next().unwrap_or("").to_string();
        let parents_raw = parts.next().unwrap_or("");
        let parents: Vec<String> = parents_raw
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        out.push(HistoryCommit {
            full_oid: full_oid.to_string(),
            short_oid: short_oid.to_string(),
            subject: subject.to_string(),
            author: author.to_string(),
            email: email.to_string(),
            when,
            parents,
        });
    }
    Ok(out)
}

pub fn show_commit(repo: &Path, oid: &str) -> Result<HistoryDetail, GitCliError> {
    if is_uncommitted_oid(oid) {
        return show_uncommitted(repo);
    }
    let meta = run_git(
        repo,
        &[
            "show",
            "-s",
            "--pretty=format:%H%x00%h%x00%s%x00%an%x00%ae%x00%cr%x00%P%x00%b",
            oid,
        ],
    )?;
    let mut parts = meta.splitn(8, '\0');
    let full_oid = parts.next().unwrap_or(oid).to_string();
    let short_oid = parts.next().unwrap_or("").to_string();
    let subject = parts.next().unwrap_or("").to_string();
    let author = parts.next().unwrap_or("").to_string();
    let email = parts.next().unwrap_or("").to_string();
    let when = parts.next().unwrap_or("").to_string();
    let parents_raw = parts.next().unwrap_or("").to_string();
    let body = parts.next().unwrap_or("").trim().to_string();
    let parents: Vec<String> = parents_raw
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    let files_raw = run_git(
        repo,
        &["diff-tree", "--no-commit-id", "--name-status", "-r", "-z", oid],
    )?;
    let files = parse_name_status_z(&files_raw);

    Ok(HistoryDetail {
        commit: HistoryCommit {
            full_oid,
            short_oid,
            subject,
            author,
            email,
            when,
            parents: parents.clone(),
        },
        body,
        parents,
        files,
    })
}

/// Working-tree / index changes as a History detail (synthetic WIP commit).
pub fn show_uncommitted(repo: &Path) -> Result<HistoryDetail, GitCliError> {
    let head = rev_parse(repo, "HEAD").ok();
    let mut files = match run_git(repo, &["diff", "--name-status", "-z", "HEAD"]) {
        Ok(raw) => parse_name_status_z(&raw),
        Err(_) => Vec::new(),
    };
    // Untracked files are not in `git diff HEAD`; list them as adds.
    if let Ok(raw) = run_git(repo, &["ls-files", "-o", "--exclude-standard", "-z"]) {
        for path in raw.split('\0').filter(|s| !s.is_empty()) {
            if !files.iter().any(|f| f.path == path) {
                files.push(HistoryFileChange {
                    status: "A".to_string(),
                    path: path.to_string(),
                });
            }
        }
    }
    // No HEAD yet: fall back to porcelain status letters.
    if head.is_none() && files.is_empty() {
        if let Ok(raw) = run_git(repo, &["status", "--porcelain", "-z"]) {
            files = parse_porcelain_z(&raw);
        }
    }

    let parents = head.into_iter().collect::<Vec<_>>();
    Ok(HistoryDetail {
        commit: HistoryCommit {
            full_oid: UNCOMMITTED_OID.to_string(),
            short_oid: "WIP".to_string(),
            subject: "Uncommitted changes".to_string(),
            author: String::new(),
            email: String::new(),
            when: "working tree".to_string(),
            parents: parents.clone(),
        },
        body: String::new(),
        parents,
        files,
    })
}

/// `git status --porcelain -z` → name-status style entries (best-effort).
fn parse_porcelain_z(raw: &str) -> Vec<HistoryFileChange> {
    let mut out = Vec::new();
    let mut parts = raw.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = parts.next() {
        if entry.len() < 3 {
            continue;
        }
        let xy = &entry[..2];
        let path = entry[3..].to_string();
        // Rename lines are followed by the old path in a separate NUL field.
        if xy.contains('R') || xy.contains('C') {
            let _old = parts.next();
        }
        let status = if xy.contains('?') || xy.contains('A') {
            "A"
        } else if xy.contains('D') {
            "D"
        } else if xy.contains('R') {
            "R"
        } else {
            "M"
        };
        out.push(HistoryFileChange {
            status: status.to_string(),
            path,
        });
    }
    out
}

fn parse_name_status_z(raw: &str) -> Vec<HistoryFileChange> {
    let mut out = Vec::new();
    let mut parts = raw.split('\0').filter(|s| !s.is_empty());
    while let Some(status) = parts.next() {
        let Some(path) = parts.next() else { break };
        // Rename/copy: status like R100, then old path, then new path.
        if status.starts_with('R') || status.starts_with('C') {
            let _old = path;
            let Some(new_path) = parts.next() else { break };
            out.push(HistoryFileChange {
                status: status.to_string(),
                path: new_path.to_string(),
            });
        } else {
            out.push(HistoryFileChange {
                status: status.to_string(),
                path: path.to_string(),
            });
        }
    }
    out
}

pub fn reset_to(repo: &Path, oid: &str, mode: ResetMode) -> Result<String, GitCliError> {
    run_git(repo, &["reset", mode.as_git_arg(), oid])
}

/// File contents at `revision` (`git show rev:path`).
///
/// For the synthetic uncommitted oid, reads the working-tree file.
pub fn show_file_at_revision(
    repo: &Path,
    revision: &str,
    rel_path: &str,
) -> Result<String, GitCliError> {
    if is_uncommitted_oid(revision) {
        return read_worktree_file(repo, rel_path);
    }
    let spec = format!("{revision}:{rel_path}");
    run_git(repo, &["show", "--textconv", &spec])
}

fn read_worktree_file(repo: &Path, rel_path: &str) -> Result<String, GitCliError> {
    let full = repo.join(rel_path);
    if !full.exists() {
        // Deleted in the working tree.
        return Ok(String::new());
    }
    std::fs::read_to_string(&full).map_err(|err| GitCliError::Failed {
        status: -1,
        stderr: format!("Failed to read {rel_path}: {err}"),
    })
}

/// Unified diff for `rel_path` introduced by `oid` (`git show --patch`).
///
/// Uses a very large context (`-U999999`) so split viewers can pair every
/// line and leave empty cells opposite pure adds/deletes.
///
/// Uncommitted oid → `git diff HEAD -- path` (or a synthetic add for untracked).
pub fn diff_file_at_commit(
    repo: &Path,
    oid: &str,
    rel_path: &str,
) -> Result<String, GitCliError> {
    if is_uncommitted_oid(oid) {
        return diff_file_uncommitted(repo, rel_path);
    }
    run_git(
        repo,
        &[
            "show",
            "--format=",
            "--patch",
            "-U999999",
            "--find-renames",
            oid,
            "--",
            rel_path,
        ],
    )
}

fn diff_file_uncommitted(repo: &Path, rel_path: &str) -> Result<String, GitCliError> {
    match run_git(
        repo,
        &[
            "diff",
            "-U999999",
            "--find-renames",
            "HEAD",
            "--",
            rel_path,
        ],
    ) {
        Ok(patch) if !patch.trim().is_empty() => Ok(patch),
        Ok(_) | Err(_) => {
            // Untracked (or empty HEAD): synthesize an all-added patch.
            match read_worktree_file(repo, rel_path) {
                Ok(text) => Ok(synthesize_added_patch(rel_path, &text)),
                Err(err) => Err(err),
            }
        }
    }
}

fn synthesize_added_patch(rel_path: &str, text: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("diff --git a/{rel_path} b/{rel_path}\n"));
    out.push_str("new file mode 100644\n");
    out.push_str("--- /dev/null\n");
    out.push_str(&format!("+++ b/{rel_path}\n"));
    let line_count = if text.is_empty() {
        0
    } else {
        let mut n = text.bytes().filter(|&b| b == b'\n').count();
        if !text.ends_with('\n') {
            n += 1;
        }
        n
    };
    if line_count == 0 {
        out.push_str("@@ -0,0 +0,0 @@\n");
        return out;
    }
    out.push_str(&format!("@@ -0,0 +1,{line_count} @@\n"));
    for line in text.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        out.push('+');
        out.push_str(body);
        out.push('\n');
    }
    out
}

/// Old and new file blobs for a path at `oid` (parent vs commit) for split diff.
///
/// Missing sides (added / deleted / root commit) become empty strings.
/// Uncommitted oid → HEAD blob vs working tree.
pub fn split_file_at_commit(
    repo: &Path,
    oid: &str,
    rel_path: &str,
) -> Result<(String, String), GitCliError> {
    if is_uncommitted_oid(oid) {
        let new_text = read_worktree_file(repo, rel_path).unwrap_or_default();
        let old_text = match show_file_at_revision(repo, "HEAD", rel_path) {
            Ok(t) => t,
            Err(_) => String::new(),
        };
        return Ok((old_text, new_text));
    }
    let new_text = match show_file_at_revision(repo, oid, rel_path) {
        Ok(t) => t,
        Err(_) => String::new(),
    };
    let parent = format!("{oid}^");
    let old_text = match show_file_at_revision(repo, &parent, rel_path) {
        Ok(t) => t,
        Err(_) => String::new(),
    };
    Ok((old_text, new_text))
}

/// Blame every line of `rel_path` (repo-relative). Index is 0-based line number.
pub fn blame_file(repo: &Path, rel_path: &str) -> Result<Vec<Option<BlameEntry>>, GitCliError> {
    let raw = run_git(repo, &["blame", "--line-porcelain", "--", rel_path])?;
    Ok(parse_blame_porcelain(&raw))
}

fn parse_blame_porcelain(raw: &str) -> Vec<Option<BlameEntry>> {
    let mut lines: Vec<Option<BlameEntry>> = Vec::new();
    let mut pending_oid = String::new();
    let mut author = String::new();
    let mut summary = String::new();
    let mut when = String::new();
    let mut author_time = String::new();

    for line in raw.lines() {
        if line.starts_with('\t') {
            // Content line ends a hunk; map to next file line.
            let entry = if pending_oid.is_empty() {
                None
            } else {
                let short = pending_oid.chars().take(7).collect::<String>();
                Some(BlameEntry {
                    short_oid: short,
                    author: author.clone(),
                    summary: summary.clone(),
                    when: if when.is_empty() {
                        author_time.clone()
                    } else {
                        when.clone()
                    },
                })
            };
            lines.push(entry);
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("author ") {
            author = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            author_time = format_unix_time(rest);
        } else if let Some(rest) = line.strip_prefix("summary ") {
            summary = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("filename ") {
            let _ = rest;
        } else {
            // Header: <40-hex> <orig> <final> [<num>]
            let mut bits = line.split_whitespace();
            if let Some(oid) = bits.next() {
                if oid.len() >= 7 && oid.chars().all(|c| c.is_ascii_hexdigit()) {
                    pending_oid = oid.to_string();
                    when.clear();
                }
            }
        }
    }
    lines
}

fn format_unix_time(raw: &str) -> String {
    let Ok(secs) = raw.trim().parse::<i64>() else {
        return raw.to_string();
    };
    // Keep a compact absolute stamp; locale-free for stable UI.
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let Some(time) = UNIX_EPOCH.checked_add(Duration::from_secs(secs.max(0) as u64)) else {
        return raw.to_string();
    };
    let Ok(elapsed) = SystemTime::now().duration_since(time) else {
        return format!("{secs}");
    };
    let days = elapsed.as_secs() / 86_400;
    if days == 0 {
        "today".into()
    } else if days == 1 {
        "1 day ago".into()
    } else if days < 30 {
        format!("{days} days ago")
    } else if days < 365 {
        format!("{} months ago", days / 30)
    } else {
        format!("{} years ago", days / 365)
    }
}
