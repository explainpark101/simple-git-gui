use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const ZED_DOWNLOAD_URL: &str = "https://zed.dev/download";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EditorPreset {
    #[default]
    Zed,
    Cursor,
    VsCode,
    Notepad,
    Custom,
}

impl EditorPreset {
    pub const ALL: [Self; 5] = [
        Self::Zed,
        Self::Cursor,
        Self::VsCode,
        Self::Notepad,
        Self::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Zed => "Zed",
            Self::Cursor => "Cursor",
            Self::VsCode => "VS Code",
            Self::Notepad => "Notepad",
            Self::Custom => "Custom",
        }
    }

    fn candidates(self) -> &'static [&'static str] {
        match self {
            Self::Zed => &["zed", "zed.exe"],
            Self::Cursor => &["cursor", "cursor.cmd", "Cursor.exe"],
            Self::VsCode => &["code", "code.cmd", "Code.exe"],
            Self::Notepad => &["notepad", "notepad.exe"],
            Self::Custom => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalEditor {
    pub preset: EditorPreset,
    #[serde(default)]
    pub custom_command: String,
}

impl Default for ExternalEditor {
    fn default() -> Self {
        Self {
            preset: EditorPreset::Zed,
            custom_command: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenKind {
    /// Repository / folder open (project).
    Project,
    /// Single file open from Changes / Files.
    File,
}

impl ExternalEditor {
    pub fn label(&self) -> String {
        if self.preset == EditorPreset::Custom {
            let cmd = self.custom_command.trim();
            if cmd.is_empty() {
                "Custom".into()
            } else {
                format!("Custom ({cmd})")
            }
        } else {
            self.preset.label().into()
        }
    }

    pub fn open_project(&self, root: &Path) -> Result<()> {
        self.launch(root, OpenKind::Project)
    }

    pub fn open_file(&self, root: &Path, rel: &str) -> Result<()> {
        self.launch(&root.join(rel), OpenKind::File)
    }

    fn launch(&self, path: &Path, _kind: OpenKind) -> Result<()> {
        let path = path
            .canonicalize()
            .with_context(|| format!("canonicalize {}", path.display()))
            .unwrap_or_else(|_| path.to_path_buf());

        if self.preset == EditorPreset::Notepad && path.is_dir() {
            bail!("Notepad can only open files, not folders");
        }

        if self.preset == EditorPreset::Custom {
            return self.open_with_custom(&path);
        }

        let mut last_err = None;
        for bin in self.preset.candidates() {
            let mut command = Command::new(bin);
            // Prefer an existing Zed window for both project folders and files.
            if self.preset == EditorPreset::Zed {
                command.arg("--existing");
            }
            command.arg(&path);
            match command.spawn() {
                Ok(_) => return Ok(()),
                Err(err) => last_err = Some(err),
            }
        }

        Err(match last_err {
            Some(err) => err.into(),
            None => anyhow::anyhow!("failed to launch {}", self.preset.label()),
        })
    }

    fn open_with_custom(&self, path: &Path) -> Result<()> {
        let cmd = self.custom_command.trim();
        if cmd.is_empty() {
            bail!("custom editor command is empty");
        }
        let mut parts = shell_split(cmd);
        if parts.is_empty() {
            bail!("custom editor command is empty");
        }
        let program = parts.remove(0);
        let mut command = Command::new(program);
        command.args(parts).arg(path);
        command
            .spawn()
            .with_context(|| format!("spawn custom editor `{cmd}`"))?;
        Ok(())
    }
}

fn shell_split(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in input.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
