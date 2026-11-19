//! GitHub Pull Requests via the `gh` CLI (list / open / merge / create).

use std::path::Path;
use std::process::{Command, Stdio};

use serde::Deserialize;

use super::cli_tools;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrMergeMethod {
    Merge,
    Squash,
    Rebase,
}

impl PrMergeMethod {
    pub const ALL: [Self; 3] = [Self::Merge, Self::Squash, Self::Rebase];

    pub fn as_gh_flag(self) -> &'static str {
        match self {
            Self::Merge => "--merge",
            Self::Squash => "--squash",
            Self::Rebase => "--rebase",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestInfo {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String,
    pub head_ref_name: String,
    #[serde(default)]
    pub is_draft: bool,
    /// `MERGEABLE` | `CONFLICTING` | `UNKNOWN` | empty
    #[serde(default)]
    pub mergeable: String,
}

#[derive(Debug)]
pub enum GhError {
    NotFound(String),
    Failed(String),
}

impl std::fmt::Display for GhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(s) | Self::Failed(s) => write!(f, "{s}"),
        }
    }
}

impl GhError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }
}

fn run_gh(repo: &Path, args: &[&str]) -> Result<String, GhError> {
    let bin = cli_tools::detect_gh_bin().unwrap_or_else(|| "gh".into());
    let mut cmd = Command::new(&bin);
    cmd.arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            GhError::NotFound(
                "GitHub CLI (`gh`) not found. Install it from this app or https://cli.github.com/"
                    .into(),
            )
        } else {
            GhError::Failed(format!("failed to spawn gh: {err}"))
        }
    })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let msg = if stderr.is_empty() { stdout } else { stderr };
        Err(GhError::Failed(if msg.is_empty() {
            format!("gh exited with status {}", output.status.code().unwrap_or(-1))
        } else {
            msg
        }))
    }
}

/// Open PRs for this repository (`gh pr list`).
pub fn list_open_prs(repo: &Path) -> Result<Vec<PullRequestInfo>, GhError> {
    let out = run_gh(
        repo,
        &[
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "30",
            "--json",
            "number,title,url,state,headRefName,isDraft,mergeable",
        ],
    )?;
    if out.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&out).map_err(|e| GhError::Failed(format!("parse gh pr list: {e}")))
}

pub fn merge_pr(repo: &Path, number: u64, method: PrMergeMethod) -> Result<String, GhError> {
    let num = number.to_string();
    run_gh(
        repo,
        &[
            "pr",
            "merge",
            &num,
            method.as_gh_flag(),
            "--delete-branch=false",
        ],
    )
}

/// Open the create-PR flow in the system browser (`gh pr create --web`).
pub fn create_pr_web(repo: &Path) -> Result<String, GhError> {
    run_gh(repo, &["pr", "create", "--web"])
}

/// Open an existing PR in the browser (`gh pr view N --web`).
pub fn view_pr_web(repo: &Path, number: u64) -> Result<String, GhError> {
    let num = number.to_string();
    run_gh(repo, &["pr", "view", &num, "--web"])
}

/// PR for the current checked-out branch, if any.
pub fn pr_for_current_branch(repo: &Path) -> Result<Option<PullRequestInfo>, GhError> {
    let out = run_gh(
        repo,
        &[
            "pr",
            "view",
            "--json",
            "number,title,url,state,headRefName,isDraft,mergeable",
        ],
    );
    match out {
        Ok(json) if !json.is_empty() => {
            let pr: PullRequestInfo = serde_json::from_str(&json)
                .map_err(|e| GhError::Failed(format!("parse gh pr view: {e}")))?;
            Ok(Some(pr))
        }
        Ok(_) => Ok(None),
        Err(GhError::Failed(msg))
            if msg.to_lowercase().contains("no pull requests")
                || msg.to_lowercase().contains("not found") =>
        {
            Ok(None)
        }
        Err(err) => Err(err),
    }
}
