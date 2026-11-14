use std::path::Path;
use std::process::Command;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitCliError {
    #[error("failed to spawn git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git exited with status {status}: {stderr}")]
    Failed { status: i32, stderr: String },
}

/// Run `git -C <repo> <args...>` and return combined stdout on success.
pub fn run_git(repo: &Path, args: &[&str]) -> Result<String, GitCliError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let status = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(GitCliError::Failed { status, stderr })
    }
}