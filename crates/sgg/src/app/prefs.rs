use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::ai_provider::{AiProvider, ACTIVE_AUTO};
use super::editor::ExternalEditor;
use super::i18n::Locale;
use super::sort::ChangeSort;
use super::terminal_shell::TerminalShellPrefs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    System,
}

/// Branch list sync indicator style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BranchSyncDisplay {
    /// Soft traffic-light glow after the name.
    #[default]
    Dot,
    /// Push/pull counts with arrows; dirty working tree as `!n`.
    Counts,
}

impl BranchSyncDisplay {
    pub const ALL: [Self; 2] = [Self::Dot, Self::Counts];
}

/// How `git pull` reconciles divergent local/remote branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PullStrategy {
    /// `git pull --no-rebase` (create a merge commit when histories diverge).
    #[default]
    Merge,
    /// `git pull --rebase` (replay local commits on top of upstream).
    Rebase,
    /// `git pull --ff-only` (fail unless a fast-forward is possible).
    FfOnly,
}

impl PullStrategy {
    pub const ALL: [Self; 3] = [Self::Merge, Self::Rebase, Self::FfOnly];

    pub fn git_flag(self) -> &'static str {
        match self {
            Self::Merge => "--no-rebase",
            Self::Rebase => "--rebase",
            Self::FfOnly => "--ff-only",
        }
    }
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
    /// How local branch sync status is shown (dot vs ↑/↓/! counts).
    #[serde(default)]
    pub branch_sync_display: BranchSyncDisplay,
    /// Ask whether to fetch when opening a repository (unless muted).
    #[serde(default = "default_true")]
    pub ask_fetch_on_open: bool,
    /// Divergent-branch strategy for Pull (`--no-rebase` / `--rebase` / `--ff-only`).
    #[serde(default)]
    pub pull_strategy: PullStrategy,
    /// Absolute repo roots that should skip the fetch-on-open prompt.
    #[serde(default)]
    pub fetch_prompt_muted: Vec<String>,
    /// Recently opened repository roots (most recent first). Unbounded.
    #[serde(default)]
    pub recent_repos: Vec<String>,
    /// Custom colors for git status labels in Changes / Files.
    #[serde(default)]
    pub status_colors: StatusColors,
    /// Terminal font family (CSS-like cascade). Empty uses the built-in default.
    #[serde(default = "default_terminal_font_family")]
    pub terminal_font_family: String,
    /// Terminal font size in pixels.
    #[serde(default = "default_terminal_font_size")]
    pub terminal_font_size: f32,
    /// Terminal line-height multiplier (Zed-style: 1.3 standard, 1.618 comfortable).
    #[serde(default = "default_terminal_line_height")]
    pub terminal_line_height: f32,
    /// Shell used for new terminal tabs (Zed `terminal.shell`).
    #[serde(default)]
    pub terminal_shell: TerminalShellPrefs,
    /// Which History table columns are visible (Description / Date / Author / Commit).
    #[serde(default)]
    pub history_columns: HistoryColumnsPrefs,
    /// Show git blame next to the preview caret line.
    #[serde(default = "default_true")]
    pub show_inline_blame: bool,
    /// Tint changed preview lines with a translucent SCM color (VS Code style).
    #[serde(default = "default_true")]
    pub show_diff_line_background: bool,
    /// UI (sans) font family. Empty uses the theme default.
    #[serde(default)]
    pub ui_font_family: String,
    /// Check GitHub `latest.json` for a newer release on launch.
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
    /// Version the user dismissed with "Later" (silent launch checks skip this tag).
    #[serde(default)]
    pub skipped_update_version: String,
    /// AI commit-message generator (Changes view sparkle button).
    #[serde(default)]
    pub commit_ai: CommitAiPrefs,
    /// Include untracked files when stashing (`git stash push -u`).
    #[serde(default)]
    pub stash_include_untracked: bool,
    /// Per-repository collapse / panel open state (keyed by absolute display path).
    #[serde(default)]
    pub project_ui: HashMap<String, ProjectUiState>,
    /// Settings page: show the right-hand table-of-contents panel.
    #[serde(default = "default_true")]
    pub settings_toc_open: bool,
}

/// Collapsed / expanded chrome remembered per project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectUiState {
    #[serde(default = "default_true")]
    pub tags_panel_open: bool,
    #[serde(default)]
    pub stash_panel_open: bool,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub progress_collapsed: bool,
}

impl Default for ProjectUiState {
    fn default() -> Self {
        Self {
            tags_panel_open: true,
            stash_panel_open: false,
            sidebar_collapsed: false,
            progress_collapsed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommitAiStyle {
    #[default]
    Conventional,
    Free,
}

impl CommitAiStyle {
    pub const ALL: [Self; 2] = [Self::Conventional, Self::Free];

    pub fn label(self) -> &'static str {
        match self {
            Self::Conventional => "Conventional Commits",
            Self::Free => "Free-form",
        }
    }
}

/// How OpenAI-compatible HTTP providers request a commit message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommitAiHttpMode {
    /// SSE `stream: true` (default). Can abort between chunks.
    #[default]
    Streaming,
    /// Single JSON completion response.
    Completion,
}

impl CommitAiHttpMode {
    pub const ALL: [Self; 2] = [Self::Streaming, Self::Completion];

    pub fn label(self) -> &'static str {
        match self {
            Self::Streaming => "Streaming",
            Self::Completion => "Completion",
        }
    }

    pub fn is_streaming(self) -> bool {
        matches!(self, Self::Streaming)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitAiPrefs {
    #[serde(default)]
    pub style: CommitAiStyle,
    /// `auto` or a provider [`AiProvider::id`].
    #[serde(default = "default_active_provider")]
    pub active_provider_id: String,
    /// Registered backends (OpenAI-compatible + CLI). Order = Auto try order.
    /// Always includes built-in Cursor / Codex CLI entries after load.
    #[serde(default)]
    pub providers: Vec<AiProvider>,
    /// Legacy global Cursor CLI path (fallback when a provider has empty `cli_bin`).
    #[serde(default)]
    pub cursor_bin: String,
    /// Legacy global Codex CLI path (fallback when a provider has empty `cli_bin`).
    #[serde(default)]
    pub codex_bin: String,
    /// HTTP/CLI wait limit in seconds. `None` = never time out.
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// OpenAI-compatible request mode (streaming vs completion).
    #[serde(default)]
    pub http_mode: CommitAiHttpMode,
}

fn default_active_provider() -> String {
    ACTIVE_AUTO.into()
}

impl Default for CommitAiPrefs {
    fn default() -> Self {
        let mut prefs = Self {
            style: CommitAiStyle::Conventional,
            active_provider_id: default_active_provider(),
            providers: Vec::new(),
            cursor_bin: String::new(),
            codex_bin: String::new(),
            timeout_secs: None,
            http_mode: CommitAiHttpMode::Streaming,
        };
        prefs.ensure_builtin_cli_providers();
        prefs
    }
}

impl CommitAiPrefs {
    /// Ensure first-class Cursor / Codex CLI providers exist (stable ids).
    pub fn ensure_builtin_cli_providers(&mut self) {
        use super::ai_provider::{AiProvider, AiProviderKind, BUILTIN_CODEX_ID, BUILTIN_CURSOR_ID};

        if !self.providers.iter().any(|p| p.id == BUILTIN_CURSOR_ID) {
            // Prefer promoting an existing Cursor CLI entry rather than duplicating.
            if let Some(existing) = self
                .providers
                .iter_mut()
                .find(|p| p.kind == AiProviderKind::CursorCli && !p.is_builtin())
            {
                existing.id = BUILTIN_CURSOR_ID.into();
                if existing.title.trim().is_empty() {
                    existing.title = AiProvider::builtin_cursor().title;
                }
                if existing.api_key_env.trim().is_empty() {
                    existing.api_key_env = "CURSOR_API_KEY".into();
                }
            } else {
                self.providers.insert(0, AiProvider::builtin_cursor());
            }
        } else if let Some(cursor) = self.providers.iter_mut().find(|p| p.id == BUILTIN_CURSOR_ID)
        {
            if cursor.api_key_env.trim().is_empty() {
                cursor.api_key_env = "CURSOR_API_KEY".into();
            }
        }
        if !self.providers.iter().any(|p| p.id == BUILTIN_CODEX_ID) {
            if let Some(existing) = self
                .providers
                .iter_mut()
                .find(|p| p.kind == AiProviderKind::CodexCli && !p.is_builtin())
            {
                existing.id = BUILTIN_CODEX_ID.into();
                if existing.title.trim().is_empty() {
                    existing.title = AiProvider::builtin_codex().title;
                }
            } else {
                let insert_at = self
                    .providers
                    .iter()
                    .position(|p| p.id == BUILTIN_CURSOR_ID)
                    .map(|i| i + 1)
                    .unwrap_or(0);
                self.providers
                    .insert(insert_at, AiProvider::builtin_codex());
            }
        }
    }

    pub fn is_auto(&self) -> bool {
        let id = self.active_provider_id.trim();
        id.is_empty() || id.eq_ignore_ascii_case(ACTIVE_AUTO)
    }

    pub fn provider_by_id(&self, id: &str) -> Option<&AiProvider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn provider_by_id_mut(&mut self, id: &str) -> Option<&mut AiProvider> {
        self.providers.iter_mut().find(|p| p.id == id)
    }

    pub fn ensure_defaults(&mut self) {
        self.ensure_builtin_cli_providers();
        if self.active_provider_id.trim().is_empty() {
            self.active_provider_id = ACTIVE_AUTO.into();
        }
        if !self.is_auto() && self.provider_by_id(&self.active_provider_id).is_none() {
            self.active_provider_id = ACTIVE_AUTO.into();
        }
    }
}

fn default_true() -> bool {
    true
}

/// Windows PostScript/family name for MesloLGS Nerd Font (Powerlevel10k).
/// GPUI takes a single family — use [`DEFAULT_TERMINAL_FONT_FALLBACKS`] for the rest.
pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = "MesloLGS NF";
/// Fallbacks when the primary Nerd Font is missing (Zed-style `font_fallbacks`).
pub const DEFAULT_TERMINAL_FONT_FALLBACKS: &[&str] = &[
    "MesloLGS Nerd Font",
    "MesloLGSNerdFont",
    "JetBrainsMono NF",
    "JetBrainsMono Nerd Font",
    "FiraCode Nerd Font",
    "Cascadia Mono",
    "Consolas",
    "monospace",
];
pub const DEFAULT_TERMINAL_FONT_SIZE: f32 = 13.0;
/// Matches Zed `TerminalLineHeight::Standard` (good for TUIs / box-drawing).
pub const DEFAULT_TERMINAL_LINE_HEIGHT: f32 = 1.3;
/// Matches Zed `TerminalLineHeight::Comfortable`.
pub const COMFORTABLE_TERMINAL_LINE_HEIGHT: f32 = 1.618;

fn default_terminal_font_family() -> String {
    DEFAULT_TERMINAL_FONT_FAMILY.to_string()
}

fn default_terminal_font_size() -> f32 {
    DEFAULT_TERMINAL_FONT_SIZE
}

fn default_terminal_line_height() -> f32 {
    DEFAULT_TERMINAL_LINE_HEIGHT
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
            branch_sync_display: BranchSyncDisplay::default(),
            ask_fetch_on_open: true,
            pull_strategy: PullStrategy::default(),
            fetch_prompt_muted: Vec::new(),
            recent_repos: Vec::new(),
            status_colors: StatusColors::default(),
            terminal_font_family: default_terminal_font_family(),
            terminal_font_size: DEFAULT_TERMINAL_FONT_SIZE,
            terminal_line_height: DEFAULT_TERMINAL_LINE_HEIGHT,
            terminal_shell: TerminalShellPrefs::default(),
            history_columns: HistoryColumnsPrefs::default(),
            show_inline_blame: true,
            show_diff_line_background: true,
            ui_font_family: String::new(),
            auto_check_updates: true,
            skipped_update_version: String::new(),
            commit_ai: CommitAiPrefs::default(),
            stash_include_untracked: false,
            project_ui: HashMap::new(),
            settings_toc_open: true,
        }
    }
}

/// Visibility of History list columns (Graph is always shown).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryColumnsPrefs {
    #[serde(default = "default_true")]
    pub description: bool,
    #[serde(default = "default_true")]
    pub date: bool,
    #[serde(default = "default_true")]
    pub author: bool,
    #[serde(default = "default_true")]
    pub commit: bool,
}

impl Default for HistoryColumnsPrefs {
    fn default() -> Self {
        Self {
            description: true,
            date: true,
            author: true,
            commit: true,
        }
    }
}

impl HistoryColumnsPrefs {
    /// Fixed-width columns (Date + Author + Commit) plus gap/padding budget.
    pub fn fixed_width_px(&self) -> f32 {
        let mut w = 24.;
        if self.date {
            w += 148.;
        }
        if self.author {
            w += 128.;
        }
        if self.commit {
            w += 72.;
        }
        w
    }

    /// Flex description column minimum when visible.
    pub fn description_min_px(&self) -> f32 {
        if self.description {
            160.
        } else {
            0.
        }
    }
}

pub use super::terminal_shell::TerminalShellKind;

/// Move `path` to the front of the recent-repos list (deduped). No size cap.
/// Returns `true` when the list order/content changed.
pub fn remember_repo(list: &mut Vec<String>, path: &Path) -> bool {
    let key = display_path(path);
    if key.is_empty() {
        return false;
    }
    if list.first().is_some_and(|p| repo_path_eq(p, &key)) {
        return false;
    }
    list.retain(|p| !repo_path_eq(p, &key));
    list.insert(0, key);
    true
}

fn repo_path_eq(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.eq_ignore_ascii_case(b)
    }
    #[cfg(not(windows))]
    {
        a == b
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
        prefs.normalize_terminal_font();
        prefs.terminal_shell.normalize_for_platform();
        prefs.commit_ai.ensure_defaults();
        Ok(prefs)
    }

    /// Clamp / fill terminal font fields after load.
    pub fn normalize_terminal_font(&mut self) {
        let trimmed = self.terminal_font_family.trim();
        if trimmed.is_empty() {
            self.terminal_font_family = DEFAULT_TERMINAL_FONT_FAMILY.to_string();
        } else if trimmed.contains(',') {
            // GPUI needs a single primary family; keep first name from old cascades.
            if let Some(primary) = trimmed.split(',').next().map(str::trim) {
                if !primary.is_empty() {
                    self.terminal_font_family = primary.to_string();
                }
            }
        }
        // Migrate previous default label that Windows does not register.
        if self.terminal_font_family == "MesloLGS Nerd Font" {
            self.terminal_font_family = DEFAULT_TERMINAL_FONT_FAMILY.to_string();
        }
        if !self.terminal_font_size.is_finite() {
            self.terminal_font_size = DEFAULT_TERMINAL_FONT_SIZE;
        }
        self.terminal_font_size = self.terminal_font_size.clamp(9.0, 32.0);
        if !self.terminal_line_height.is_finite() {
            self.terminal_line_height = DEFAULT_TERMINAL_LINE_HEIGHT;
        }
        self.terminal_line_height = self.terminal_line_height.clamp(1.0, 3.0);
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