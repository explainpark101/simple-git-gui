use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::editor::{EditorPreset, ExternalEditor};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPrefs {
    pub theme: ThemePreference,
    #[serde(default)]
    pub locale: Locale,
    pub editor: ExternalEditor,
    pub change_sort: ChangeSort,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            theme: ThemePreference::System,
            locale: Locale::English,
            editor: ExternalEditor {
                preset: EditorPreset::Cursor,
                custom_command: String::new(),
            },
            change_sort: ChangeSort::NameAsc,
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
        let prefs = serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", path.display()))?;
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