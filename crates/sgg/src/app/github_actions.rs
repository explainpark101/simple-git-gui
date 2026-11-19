//! GitHub Actions via the `gh` CLI (workflows + runs).

use std::path::Path;

use serde::Deserialize;

use super::github_pr::{run_gh, GhError};

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowInfo {
    pub id: u64,
    pub name: String,
    pub path: String,
    /// `active` | `disabled_manually` | `disabled_inactivity` | …
    pub state: String,
}

impl WorkflowInfo {
    pub fn is_active(&self) -> bool {
        self.state.eq_ignore_ascii_case("active")
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunInfo {
    pub database_id: u64,
    #[serde(default)]
    pub display_title: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub workflow_name: String,
    #[serde(default)]
    pub head_branch: String,
    #[serde(default)]
    pub head_sha: String,
    pub status: String,
    #[serde(default)]
    pub conclusion: String,
    #[serde(default)]
    pub event: String,
    pub url: String,
    #[serde(default)]
    pub created_at: String,
}

impl WorkflowRunInfo {
    pub fn is_in_progress(&self) -> bool {
        matches!(
            self.status.to_ascii_lowercase().as_str(),
            "queued" | "in_progress" | "requested" | "waiting" | "pending"
        )
    }

    pub fn short_sha(&self) -> &str {
        let s = self.head_sha.as_str();
        if s.len() >= 7 {
            &s[..7]
        } else {
            s
        }
    }
}

/// List workflows, including disabled (`gh workflow list --all`).
pub fn list_workflows(repo: &Path) -> Result<Vec<WorkflowInfo>, GhError> {
    let out = run_gh(
        repo,
        &[
            "workflow",
            "list",
            "--all",
            "--limit",
            "50",
            "--json",
            "id,name,path,state",
        ],
    )?;
    if out.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&out).map_err(|e| GhError::Failed(format!("parse gh workflow list: {e}")))
}

/// Recent runs; optionally filter by workflow id or name.
pub fn list_runs(
    repo: &Path,
    workflow: Option<&str>,
    limit: u32,
) -> Result<Vec<WorkflowRunInfo>, GhError> {
    let limit_s = limit.max(1).to_string();
    let mut args = vec![
        "run",
        "list",
        "--limit",
        limit_s.as_str(),
        "--json",
        "databaseId,displayTitle,name,workflowName,headBranch,headSha,status,conclusion,event,url,createdAt",
    ];
    if let Some(w) = workflow {
        if !w.is_empty() {
            args.push("--workflow");
            args.push(w);
        }
    }
    let out = run_gh(repo, &args)?;
    if out.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&out).map_err(|e| GhError::Failed(format!("parse gh run list: {e}")))
}

/// Trigger `workflow_dispatch` for a workflow (`gh workflow run`).
pub fn run_workflow(
    repo: &Path,
    workflow_id: u64,
    git_ref: Option<&str>,
) -> Result<String, GhError> {
    let id = workflow_id.to_string();
    let mut args = vec!["workflow", "run", id.as_str()];
    let ref_owned;
    if let Some(r) = git_ref.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--ref");
        ref_owned = r.to_string();
        args.push(ref_owned.as_str());
    }
    run_gh(repo, &args)
}

pub fn enable_workflow(repo: &Path, workflow_id: u64) -> Result<String, GhError> {
    let id = workflow_id.to_string();
    run_gh(repo, &["workflow", "enable", &id])
}

pub fn disable_workflow(repo: &Path, workflow_id: u64) -> Result<String, GhError> {
    let id = workflow_id.to_string();
    run_gh(repo, &["workflow", "disable", &id])
}

pub fn rerun_run(repo: &Path, run_id: u64, failed_only: bool) -> Result<String, GhError> {
    let id = run_id.to_string();
    if failed_only {
        run_gh(repo, &["run", "rerun", &id, "--failed"])
    } else {
        run_gh(repo, &["run", "rerun", &id])
    }
}

pub fn cancel_run(repo: &Path, run_id: u64) -> Result<String, GhError> {
    let id = run_id.to_string();
    run_gh(repo, &["run", "cancel", &id])
}

pub fn view_run_web(repo: &Path, run_id: u64) -> Result<String, GhError> {
    let id = run_id.to_string();
    run_gh(repo, &["run", "view", &id, "--web"])
}
