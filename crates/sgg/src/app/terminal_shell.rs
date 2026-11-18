//! Preferred shell for new terminal tabs (Zed `terminal.shell`).
//!
//! Defaults and the Settings picker are OS-aware:
//! - Windows → PowerShell (also Cmd / Git Bash when installed)
//! - macOS → zsh
//! - Linux → bash

use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalShellKind {
    PowerShell,
    Cmd,
    GitBash,
    Bash,
    Zsh,
    Custom,
}

impl Default for TerminalShellKind {
    fn default() -> Self {
        Self::platform_default()
    }
}

impl TerminalShellKind {
    pub const ALL: [Self; 6] = [
        Self::PowerShell,
        Self::Cmd,
        Self::GitBash,
        Self::Bash,
        Self::Zsh,
        Self::Custom,
    ];

    /// Default shell for this host OS.
    pub fn platform_default() -> Self {
        #[cfg(windows)]
        {
            Self::PowerShell
        }
        #[cfg(target_os = "macos")]
        {
            Self::Zsh
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            Self::Bash
        }
        #[cfg(not(any(windows, unix)))]
        {
            Self::Custom
        }
    }

    /// Shells the Settings UI may offer on this OS (missing tools omitted).
    pub fn available_kinds() -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|kind| kind.is_available())
            .collect()
    }

    pub fn is_available(self) -> bool {
        match self {
            Self::PowerShell | Self::Cmd => cfg!(windows),
            Self::GitBash => cfg!(windows) && find_git_bash().is_some(),
            Self::Bash => cfg!(unix) && find_unix_shell("bash").is_some(),
            Self::Zsh => cfg!(unix) && find_unix_shell("zsh").is_some(),
            Self::Custom => true,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::PowerShell => "PowerShell",
            Self::Cmd => "Command Prompt",
            Self::GitBash => "Git Bash",
            Self::Bash => "bash",
            Self::Zsh => "zsh",
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
            kind: TerminalShellKind::platform_default(),
            custom_command: String::new(),
        }
    }
}

impl TerminalShellPrefs {
    /// If a saved kind is not usable on this OS, fall back to the platform default.
    pub fn normalize_for_platform(&mut self) {
        if !self.kind.is_available() {
            self.kind = TerminalShellKind::platform_default();
        }
    }

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
                    fallback_command()
                }
            }
            TerminalShellKind::Bash => {
                if let Some(bash) = find_unix_shell("bash") {
                    CommandBuilder::new(bash)
                } else {
                    fallback_command()
                }
            }
            TerminalShellKind::Zsh => {
                if let Some(zsh) = find_unix_shell("zsh") {
                    CommandBuilder::new(zsh)
                } else {
                    fallback_command()
                }
            }
            TerminalShellKind::Custom => {
                parse_custom_command(&self.custom_command).unwrap_or_else(fallback_command)
            }
        }
    }
}

fn fallback_command() -> CommandBuilder {
    match TerminalShellKind::platform_default() {
        TerminalShellKind::PowerShell => {
            let mut cmd = CommandBuilder::new("powershell.exe");
            cmd.arg("-NoLogo");
            cmd
        }
        TerminalShellKind::Bash => CommandBuilder::new(
            find_unix_shell("bash").unwrap_or_else(|| PathBuf::from("/bin/bash")),
        ),
        TerminalShellKind::Zsh => CommandBuilder::new(
            find_unix_shell("zsh").unwrap_or_else(|| PathBuf::from("/bin/zsh")),
        ),
        other => {
            // Should not loop: platform_default is never Custom/Cmd/GitBash.
            let _ = other;
            #[cfg(windows)]
            {
                let mut cmd = CommandBuilder::new("powershell.exe");
                cmd.arg("-NoLogo");
                cmd
            }
            #[cfg(unix)]
            {
                CommandBuilder::new("/bin/sh")
            }
            #[cfg(not(any(windows, unix)))]
            {
                CommandBuilder::new("sh")
            }
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
    if let Ok(root) = std::env::var("GIT_INSTALL_ROOT") {
        let p = Path::new(&root).join("bin").join("bash.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn find_unix_shell(name: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        const PREFIXES: &[&str] = &["/bin", "/usr/bin", "/usr/local/bin"];
        for prefix in PREFIXES {
            let p = Path::new(prefix).join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(':') {
                if dir.is_empty() {
                    continue;
                }
                let p = Path::new(dir).join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        None
    }
}

/// Example custom-command placeholder for the Settings input.
pub fn custom_command_placeholder() -> &'static str {
    #[cfg(windows)]
    {
        r#"e.g. "C:\Program Files\Git\bin\bash.exe" --login"#
    }
    #[cfg(not(windows))]
    {
        "e.g. /bin/bash -l"
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_default_is_available() {
        let kind = TerminalShellKind::platform_default();
        assert!(
            kind.is_available() || kind == TerminalShellKind::Custom,
            "default {kind:?} should be available on this host"
        );
    }

    #[test]
    fn available_kinds_include_custom() {
        assert!(TerminalShellKind::available_kinds().contains(&TerminalShellKind::Custom));
    }

    #[test]
    fn normalize_replaces_unavailable() {
        let mut prefs = TerminalShellPrefs {
            kind: if cfg!(windows) {
                TerminalShellKind::Bash
            } else {
                TerminalShellKind::PowerShell
            },
            custom_command: String::new(),
        };
        prefs.normalize_for_platform();
        assert!(prefs.kind.is_available());
    }
}
