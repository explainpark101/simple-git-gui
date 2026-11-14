use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::editor::ExternalEditor;
use super::i18n::Locale;
use super::sort::ChangeSort;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

/// Per-status color overrides as `#rrggbb` / `#rrggbbaa` hex (None = theme default).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusColors {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untracked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict: Option<String>,
}

impl StatusColors {
    pub const KEYS: [&'static str; 6] = [
        "modified",
        "added",
        "deleted",
        "renamed",
        "untracked",
        "conflict",
    ];

    pub fn get(&self, status: &str) -> Option<&str> {
        match status {
            "modified" => self.modified.as_deref(),
            "added" => self.added.as_deref(),
            "deleted" => self.deleted.as_deref(),
            "renamed" => self.renamed.as_deref(),
            "untracked" => self.untracked.as_deref(),
            "conflict" => self.conflict.as_deref(),
            _ => None,
        }
    }

    pub fn set(&mut self, status: &str, hex: Option<String>) {
        let slot = match status {
            "modified" => &mut self.modified,
            "added" => &mut self.added,
            "deleted" => &mut self.deleted,
            "renamed" => &mut self.renamed,
            "untracked" => &mut self.untracked,
            "conflict" => &mut self.conflict,
            _ => return,
        };
        *slot = hex;
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPrefs {
    pub theme: ThemePreference,
    #[serde(default)]
    pub locale: Locale,
    pub editor: ExternalEditor,
    pub change_sort: ChangeSort,
    /// Group Changes into status sections (Staged / Unstaged / …).
    #[serde(default = "default_true")]
    pub group_by_status: bool,
    /// Show Changes list as a folder tree (editor-style).
    #[serde(default)]
    pub changes_as_tree: bool,
    /// Periodically reload repository status / changes.
    #[serde(default = "default_true")]
    pub auto_refresh: bool,
    /// Include remote-tracking branches in the Branches list.
    #[serde(default = "default_true")]
    pub show_remote_branches: bool,
    /// Ask whether to fetch when opening a repository (unless muted).
    #[serde(default = "default_true")]
    pub ask_fetch_on_open: bool,
    /// Absolute repo roots that should skip the fetch-on-open prompt.
    #[serde(default)]
    pub fetch_prompt_muted: Vec<String>,
    /// Custom colors for git status labels in Changes / Files.
    #[serde(default)]
    pub status_colors: StatusColors,
}

fn default_true() -> bool {
    true
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            locale: Locale::English,
            editor: ExternalEditor::default(),
            change_sort: ChangeSort::NameAsc,
            group_by_status: true,
            changes_as_tree: false,
            auto_refresh: true,
            show_remote_branches: true,
            ask_fetch_on_open: true,
            fetch_prompt_muted: Vec::new(),
            status_colors: StatusColors::default(),
        }
    }
}

impl AppPrefs {
    pub fn load() -> Self {
        match Self::load_from_disk() {
            Ok(prefs) => prefs,
            Err(err) => {
                eprintln!("sgg: failed to load settings, using defaults: {err:#}");
                Self::default()
            }
        }
    }

    fn load_from_disk() -> Result<Self> {
        let path = settings_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        let mut prefs: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", path.display()))?;
        prefs.change_sort = prefs.change_sort.normalized();
        Ok(prefs)
    }

    pub fn save(&self) -> Result<()> {
        let path = settings_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }
}

pub fn settings_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("no config directory available")?;
    Ok(base.join("sgg").join("settings.json"))
}

/// Resolve `.` / relative paths to a stable absolute display path.
pub fn absolute_path(path: PathBuf) -> PathBuf {
    let joined = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    match joined.canonicalize() {
        Ok(canon) => strip_verbatim(canon),
        Err(_) => joined,
    }
}

pub fn display_path(path: &std::path::Path) -> String {
    strip_verbatim(path.to_path_buf()).display().to_string()
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    let _ = s;
    path
}