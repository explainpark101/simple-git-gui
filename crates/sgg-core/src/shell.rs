use std::path::Path;
use std::process::{Command, Stdio};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GitCliError {
    #[error("failed to spawn git: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("git exited with status {status}: {stderr}")]
    Failed { status: i32, stderr: String },
}

/// Run `git -C <repo> <args...>` and return combined stdout on success.
///
/// On Windows the GUI binary uses `windows_subsystem = "windows"`. Without
/// `CREATE_NO_WINDOW`, spawning the console `git.exe` briefly flashes a cmd
/// window. Output is captured and surfaced in the in-app Progress panel
/// instead of an external console.
pub fn run_git(repo: &Path, args: &[&str]) -> Result<String, GitCliError> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Prefer GUI credential helpers over a TTY prompt (no console attached).
        .env("GIT_TERMINAL_PROMPT", "0");

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // Some git commands (fetch) print progress on stderr even on success.
        Ok(merge_git_output(&stdout, &stderr))
    } else {
        let status = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = if stderr.is_empty() {
            stdout
        } else if stdout.is_empty() {
            stderr
        } else {
            format!("{stderr}\n{stdout}")
        };
        Err(GitCliError::Failed { status, stderr })
    }
}

fn merge_git_output(stdout: &str, stderr: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n{stderr}"),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (true, true) => String::new(),
    }
}
