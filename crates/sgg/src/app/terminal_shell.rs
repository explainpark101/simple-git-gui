//! Preferred shell for new terminal tabs (Zed `terminal.shell`).

use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalShellKind {
    #[default]
    PowerShell,
    Cmd,
    GitBash,
    Custom,
}

impl TerminalShellKind {
    pub const ALL: [Self; 4] = [Self::PowerShell, Self::Cmd, Self::GitBash, Self::Custom];

    pub fn label(self) -> &'static str {
        match self {
            Self::PowerShell => "PowerShell",
            Self::Cmd => "Command Prompt",
            Self::GitBash => "Git Bash",
            Self::Custom => "Custom",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalShellPrefs {
    #[serde(default)]
    pub kind: TerminalShellKind,
    /// Executable + optional args for [`TerminalShellKind::Custom`].
    #[serde(default)]
    pub custom_command: String,
}

impl Default for TerminalShellPrefs {
    fn default() -> Self {
        Self {
            kind: TerminalShellKind::PowerShell,
            custom_command: String::new(),
        }
    }
}

impl TerminalShellPrefs {
    pub fn build_command(&self) -> CommandBuilder {
        match self.kind {
            TerminalShellKind::PowerShell => {
                let mut cmd = CommandBuilder::new("powershell.exe");
                cmd.arg("-NoLogo");
                cmd
            }
            TerminalShellKind::Cmd => {
                let mut cmd = CommandBuilder::new(std::env::var("COMSPEC").unwrap_or_else(|_| {
                    r"C:\Windows\System32\cmd.exe".into()
                }));
                cmd.arg("/Q");
                cmd
            }
            TerminalShellKind::GitBash => {
                if let Some(bash) = find_git_bash() {
                    CommandBuilder::new(bash)
                } else {
                    // Fall back to PowerShell if Git Bash is missing.
                    let mut cmd = CommandBuilder::new("powershell.exe");
                    cmd.arg("-NoLogo");
                    cmd
                }
            }
            TerminalShellKind::Custom => parse_custom_command(&self.custom_command)
                .unwrap_or_else(|| {
                    let mut cmd = CommandBuilder::new("powershell.exe");
                    cmd.arg("-NoLogo");
                    cmd
                }),
        }
    }
}

fn find_git_bash() -> Option<PathBuf> {
    const CANDIDATES: &[&str] = &[
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files\Git\usr\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];
    for path in CANDIDATES {
        let p = Path::new(path);
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }
    // Also honor GIT_INSTALL_ROOT / ProgramFiles.
    if let Ok(root) = std::env::var("GIT_INSTALL_ROOT") {
        let p = Path::new(&root).join("bin").join("bash.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Very small argv splitter: supports `"quoted paths"` and plain tokens.
fn parse_custom_command(raw: &str) -> Option<CommandBuilder> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in raw.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    parts.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    let mut iter = parts.into_iter();
    let prog = iter.next()?;
    let mut cmd = CommandBuilder::new(prog);
    for arg in iter {
        cmd.arg(arg);
    }
    Some(cmd)
}
