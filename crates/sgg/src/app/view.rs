use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::diff::{DiffMode, DiffState};
use gpui_kit::component::input::{
    EditorState, GutterDecoration, GutterDecorationCollection, InputEvent, InputState,
    OverviewRulerDecoration, OverviewRulerDecorationCollection, OverviewRulerLane,
    RangeDecoration, RangeDecorationCollection, RangeDecorationStyle, TextareaState,
};
use gpui_kit::*;

use sgg_core::{
    line_start_offset, lines_byte_range, list_file_line_changes, AlignedSplitView, BlameEntry,
    HistoryCommit, HistoryDetail, HistoryGraph, LineChange, LineChangeKind, RepoSnapshot,
    RepoState,
};

use super::busy::BusyKind;
use super::editor::{EditorPreset, ExternalEditor};
use super::files::{list_tree, FsNode};
use super::i18n::{Locale, UiText};
use super::page::{AppPage, NavEntry};
use super::prefs::{
    absolute_path, display_path, remember_repo, AppPrefs, CommitAiPrefs, StatusColors,
    ThemePreference, DEFAULT_TERMINAL_FONT_FAMILY, DEFAULT_TERMINAL_FONT_SIZE,
    DEFAULT_TERMINAL_LINE_HEIGHT,
};
use super::progress::{
    ProgressEntry, ProgressState, PROGRESS_DEFAULT_HEIGHT, PROGRESS_LOG_LIMIT,
};
use super::terminal::TerminalDock;
use super::terminal_shell::{TerminalShellKind, TerminalShellPrefs};
use super::reload::{load_repo, LoadRepoError, LoadedRepo};
use super::render::status_style::{hsla_to_hex, theme_status_color, TreeStatusIndex};
use super::sort::{group_and_sort, section_flat_rows, ChangeFlatRow, ChangeSection, ChangeSort};

#[derive(Clone)]
pub(super) struct StatusColorPickers {
    pub modified: Entity<ColorPickerState>,
    pub added: Entity<ColorPickerState>,
    pub deleted: Entity<ColorPickerState>,
    pub renamed: Entity<ColorPickerState>,
    pub untracked: Entity<ColorPickerState>,
    pub conflict: Entity<ColorPickerState>,
}

impl StatusColorPickers {
    pub fn get(&self, status: &str) -> Option<&Entity<ColorPickerState>> {
        match status {
            "modified" => Some(&self.modified),
            "added" => Some(&self.added),
            "deleted" => Some(&self.deleted),
            "renamed" => Some(&self.renamed),
            "untracked" => Some(&self.untracked),
            "conflict" => Some(&self.conflict),
            _ => None,
        }
    }
}

/// Default polling interval when auto-refresh is enabled.
pub(super) const AUTO_REFRESH_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum BranchForm {
    #[default]
    Hidden,
    Create,
    Rename {
        from: String,
    },
}

/// How a History-tab file is shown in the preview pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum HistoryFileViewMode {
    #[default]
    Diff,
    FileAtCommit,
}

/// How a Git Changes preview is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ChangesFileViewMode {
    /// Editable working-tree file with SCM gutter markers.
    #[default]
    File,
    /// Uncommitted patch (Unified / Split like History).
    Diff,
}

/// Diff presentation when Diff mode is active (History or Changes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum HistoryDiffLayout {
    #[default]
    Unified,
    Split,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct HistoryFilePreview {
    pub oid: String,
    pub path: String,
    pub mode: HistoryFileViewMode,
    pub diff_layout: HistoryDiffLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum FileOpForm {
    #[default]
    Hidden,
    Rename {
        from: String,
    },
    Move {
        from: String,
    },
    /// Create a file under `parent` (empty = repo root).
    NewFile {
        parent: String,
    },
    /// Create a folder under `parent` (empty = repo root).
    NewFolder {
        parent: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum RemoteForm {
    #[default]
    Hidden,
    Add,
    Edit {
        name: String,
    },
}

/// One open file tab in the Files preview pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PreviewTab {
    pub path: String,
    pub content: String,
}

pub struct AppView {
    /// Working folder used to find the nearest git repo. `None` = nothing open yet.
    pub(super) start_path: Option<PathBuf>,
    pub(super) snapshot: Option<RepoSnapshot>,
    /// Last applied snapshot fingerprint (skip notify when auto-refresh is a no-op).
    snapshot_fp: u64,
    pub(super) selected: HashSet<String>,
    pub(super) selected_remote: String,
    pub(super) status_line: SharedString,
    pub(super) commit_input: Entity<TextareaState>,
    pub(super) branch_input: Entity<InputState>,
    pub(super) branch_search_input: Entity<InputState>,
    pub(super) file_op_input: Entity<InputState>,
    pub(super) remote_name_input: Entity<InputState>,
    pub(super) remote_url_input: Entity<InputState>,
    pub(super) custom_editor_input: Entity<InputState>,
    pub(super) terminal_font_input: Entity<InputState>,
    pub(super) ui_font_input: Entity<InputState>,
    /// Cached `TextSystem::all_font_names()` for Settings font pickers.
    pub(super) system_font_names: Option<Vec<String>>,
    pub(super) preview_input: Entity<EditorState>,
    /// Left pane for History split-diff (old file / parent blob).
    pub(super) preview_input_old: Entity<EditorState>,
    /// History Diff viewer (unified / split with empty counterpart cells).
    pub(super) history_diff: Entity<DiffState>,
    /// Aligned old|new rows for History Split Diff (plain text + gutter metadata).
    pub(super) history_aligned_split: Option<AlignedSplitView>,
    /// Sync Split Diff scroll by shared context / aligned display rows.
    pub(super) history_diff_scroll_sync: bool,
    /// Outer scroll for the old Split Diff pane (gutter + editor move together).
    pub(super) history_split_scroll_old: ScrollHandle,
    /// Outer scroll for the new Split Diff pane.
    pub(super) history_split_scroll_new: ScrollHandle,
    /// Shared handle when scroll-sync is on (both panes track this).
    pub(super) history_split_scroll_shared: ScrollHandle,
    /// Settings page vertical scroll (so we can jump to Commit AI).
    pub(super) settings_scroll: ScrollHandle,
    /// Anchor on the Commit AI settings card.
    pub(super) settings_commit_ai_anchor: ScrollAnchor,
    /// VS Code-style SCM bars beside the preview gutter (line numbers).
    preview_diff_gutter: GutterDecorationCollection,
    preview_diff_ruler: OverviewRulerDecorationCollection,
    /// Translucent whole-line fills matching SCM gutter colors.
    preview_diff_line_bg: RangeDecorationCollection,
    preview_diff_gutter_old: GutterDecorationCollection,
    preview_diff_line_bg_old: RangeDecorationCollection,
    pub(super) terminal_font_family: String,
    /// Empty = theme default sans font.
    pub(super) ui_font_family: String,
    pub(super) show_inline_blame: bool,
    pub(super) show_diff_line_background: bool,
    /// Commit history list (History tab).
    pub(super) history_commits: Vec<HistoryCommit>,
    /// Lane layout for the History list graph gutter (Zed-style).
    pub(super) history_graph: HistoryGraph,
    /// Full oid of `HEAD` (parent of the uncommitted WIP graph row).
    pub(super) history_head_oid: Option<String>,
    pub(super) history_selected: Option<String>,
    pub(super) history_detail: Option<HistoryDetail>,
    /// Optional revision tip for History (`None` = all selected branches).
    pub(super) history_rev: Option<String>,
    /// Branch names hidden from the History graph. Empty = all branches (default).
    pub(super) history_graph_excluded: HashSet<String>,
    /// Search box in the History graph branch-filter panel.
    pub(super) history_branch_filter_input: Entity<InputState>,
    _history_branch_filter_sub: Option<Subscription>,
    /// Cog panel open: pick which branches appear in the graph.
    pub(super) history_branch_filter_open: bool,
    /// History file opened in the preview pane (diff or blob at commit).
    pub(super) history_file_preview: Option<HistoryFilePreview>,
    /// Git Changes preview: working-tree file vs Diff.
    pub(super) changes_file_view_mode: ChangesFileViewMode,
    /// Unified / Split when [`ChangesFileViewMode::Diff`] is active.
    pub(super) changes_diff_layout: HistoryDiffLayout,
    /// Commit detail changed-files: folder tree vs flat list (session).
    pub(super) history_files_as_tree: bool,
    /// Dir keys collapsed in History changed-files tree mode.
    pub(super) collapsed_history_dirs: HashSet<String>,
    /// Blame cache for the current preview file (0-based lines).
    pub(super) blame_path: Option<String>,
    /// Revision used for [`Self::blame_lines`] (`None` = working tree).
    blame_rev: Option<String>,
    pub(super) blame_lines: Vec<Option<BlameEntry>>,
    blame_cursor_line: Option<usize>,
    /// Last editor scroll Y used to keep inline blame glued to the caret row.
    blame_scroll_y: Option<f32>,
    pub(super) terminal_font_size: f32,
    pub(super) terminal_line_height: f32,
    pub(super) terminal_shell: TerminalShellPrefs,
    pub(super) terminal_shell_input: Entity<InputState>,
    /// Blinking caret phase for the focused terminal.
    pub(super) terminal_caret_on: bool,
    terminal_caret_ticks: u32,
    /// Path currently loaded into the shared preview editors (active page only).
    pub(super) preview_path: Option<String>,
    /// Git page preview path (kept when switching away so History/Files cannot wipe it).
    pub(super) git_preview_path: Option<String>,
    /// Open preview tabs on the Files page (Git uses a single `preview_path`).
    pub(super) preview_tabs: Vec<PreviewTab>,
    pub(super) preview_tab_index: usize,
    pub(super) error: Option<SharedString>,
    pub(super) page: AppPage,
    pub(super) change_sort: ChangeSort,
    pub(super) group_by_status: bool,
    /// Grouped/sorted Changes rows; rebuilt on snapshot or sort/group prefs.
    pub(super) changes_sections: Vec<ChangeSection>,
    /// Precomputed virtualized body rows per section (same order as `changes_sections`).
    pub(super) changes_flat_rows: Vec<Vec<ChangeFlatRow>>,
    /// Coalesce in-flight background reloads.
    pub(super) reload_task: Option<Task<()>>,
    pub(super) preferred_editor: ExternalEditor,
    pub(super) theme_pref: ThemePreference,
    pub(super) locale: Locale,
    pub(super) changes_as_tree: bool,
    pub(super) auto_refresh: bool,
    pub(super) auto_check_updates: bool,
    /// Version dismissed via "Later" (silent launch checks skip re-prompting it).
    pub(super) skipped_update_version: String,
    pub(super) commit_ai: CommitAiPrefs,
    /// Add/Edit form; closed until the user opens it.
    pub(super) commit_ai_form: super::ai_provider::CommitAiFormMode,
    pub(super) commit_ai_form_auth: super::ai_provider::AiAuthKind,
    pub(super) commit_ai_title_input: Entity<InputState>,
    pub(super) commit_ai_docs_input: Entity<InputState>,
    pub(super) commit_ai_base_input: Entity<InputState>,
    pub(super) commit_ai_path_input: Entity<InputState>,
    pub(super) commit_ai_model_input: Entity<InputState>,
    pub(super) commit_ai_key_input: Entity<InputState>,
    pub(super) commit_ai_auth_header_input: Entity<InputState>,
    pub(super) commit_ai_auth_query_input: Entity<InputState>,
    pub(super) commit_ai_key_env_input: Entity<InputState>,
    pub(super) commit_ai_cli_bin_input: Entity<InputState>,
    /// Seconds; empty = no timeout (see [`CommitAiPrefs::timeout_secs`]).
    pub(super) commit_ai_timeout_input: Entity<InputState>,
    /// Quick-set field on the built-in Cursor card (T3-style CURSOR_API_KEY paste).
    pub(super) cursor_api_key_input: Entity<InputState>,
    /// Set while AI commit generation runs; flipped to abort in-flight work.
    generate_commit_abort: Option<Arc<AtomicBool>>,
    /// Last T3 import summary shown under the Cursor card.
    pub(super) t3_import_status: Option<SharedString>,
    pub(super) t3_available: bool,
    pub(super) commit_ai_models: Vec<String>,
    pub(super) commit_ai_models_error: Option<String>,
    pub(super) commit_ai_models_task: Option<Task<()>>,
    commit_ai_install_cli_task: Option<Task<()>>,
    /// 0.0..=1.0 while [`BusyKind::InstallCli`] runs (bottom-border progress).
    pub(super) cli_install_progress: Option<f32>,
    /// Cached CLI probe results for Settings (never refresh on the UI thread).
    pub(super) cli_status_cursor: Option<super::cli_tools::CliStatus>,
    pub(super) cli_status_codex: Option<super::cli_tools::CliStatus>,
    pub(super) cli_status_task: Option<Task<()>>,
    generate_commit_task: Option<Task<()>>,
    /// Latest release from `latest.json` when newer than this build.
    pub(super) update_available: Option<super::update::LatestInfo>,
    update_task: Option<Task<()>>,
    pub(super) show_remote_branches: bool,
    pub(super) ask_fetch_on_open: bool,
    pub(super) fetch_prompt_muted: Vec<String>,
    /// Absolute repo roots, most recent first (persisted, unbounded).
    pub(super) recent_repos: Vec<String>,
    pub(super) status_colors: StatusColors,
    pub(super) status_color_pickers: StatusColorPickers,
    _status_color_subs: Vec<Subscription>,
    _branch_search_sub: Option<Subscription>,
    _font_search_subs: Vec<Subscription>,
    _commit_input_sub: Option<Subscription>,
    _preview_blame_sub: Option<Subscription>,
    /// Re-render when Unified Diff line selection changes (blame strip).
    _preview_diff_blame_sub: Option<Subscription>,
    _window_bounds_sub: Option<Subscription>,
    pub(super) expanded_dirs: HashSet<String>,
    /// Cached Files tree rows; rebuilt on expand/collapse/reload/file ops.
    pub(super) files_tree_nodes: Vec<FsNode>,
    /// Cached path→status for Files tree badges; rebuilt with snapshot.
    pub(super) files_status_index: TreeStatusIndex,
    /// Dir keys the user collapsed in Changes tree mode (all others stay expanded).
    pub(super) collapsed_change_dirs: HashSet<String>,
    /// Status section titles collapsed in Changes (default: all expanded).
    pub(super) collapsed_change_sections: HashSet<String>,
    pub(super) nav_history: Vec<NavEntry>,
    pub(super) history_index: usize,
    /// Suppresses history pushes while applying back/forward.
    navigating_history: bool,
    pub(super) branch_form: BranchForm,
    pub(super) file_op_form: FileOpForm,
    pub(super) remote_form: RemoteForm,
    pub(super) sidebar_collapsed: bool,
    pub(super) busy: Option<BusyKind>,
    /// Bottom Progress dock: activity lines for fetch/pull/push and other busy work.
    pub(super) progress_log: Vec<ProgressEntry>,
    /// When true, only the Progress header is shown (stuck to the sidebar bottom).
    pub(super) progress_collapsed: bool,
    /// Expanded Progress panel height (px); adjusted via the resizable split.
    pub(super) progress_panel_height: f32,
    /// Bottom PTY terminal dock (portable-pty + alacritty VTE).
    pub(super) terminal: TerminalDock,
    pub(super) terminal_focus: FocusHandle,
    pub(super) terminal_focused: bool,
    terminal_poll_task: Option<Task<()>>,
    /// Narrow Changes list: letter status + action buttons only on row hover.
    pub(super) changes_list_compact: bool,
    /// Per Changes section title: header action buttons icon-only (no labels).
    pub(super) changes_section_hide_labels: HashMap<String, bool>,
    /// Wide Changes pane: Unstaged/Untracked left, Staged right.
    pub(super) changes_split_view: bool,
    /// Narrow sidebar remote header: Fetch/Pull/Push show icons only.
    pub(super) remote_actions_compact: bool,
    /// Narrow Branches header: search collapses to a magnifier + dropdown.
    pub(super) branch_search_compact: bool,
    /// Compact branch search dropdown open under the Branches header.
    pub(super) branch_search_open: bool,
    /// Narrow Files tree: action buttons only on row hover.
    pub(super) files_tree_compact: bool,
    /// Checkbox state for the fetch-on-open prompt.
    pub(super) fetch_dont_ask: bool,
    /// Repo root we already prompted for this session (avoid repeat).
    pub(super) fetch_prompted_root: Option<PathBuf>,
    pub(super) focus_handle: FocusHandle,
    refresh_task: Option<Task<()>>,
}

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AppView {
    pub fn new(start_path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = AppPrefs::load();
        let start_path = start_path.map(absolute_path);
        let t = prefs.locale.text();
        let focus_handle = cx.focus_handle();

        let commit_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder(t.commit_placeholder)
        });
        let branch_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.branch_placeholder));
        let branch_search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.branch_search_placeholder));
        let history_branch_filter_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t.history_branch_filter_placeholder)
        });
        let file_op_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.file_op_placeholder));
        let remote_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.remote_name_placeholder));
        let remote_url_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.remote_url_placeholder));
        let custom_editor_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(r#"e.g. "C:\path\to\editor.exe" --wait"#)
        });
        let terminal_font_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t.font_search_placeholder)
        });
        let ui_font_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t.font_search_placeholder)
        });
        let commit_ai_title_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Provider title"));
        let commit_ai_docs_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("https://… docs / console"));
        let commit_ai_base_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("https://api.openai.com/v1")
        });
        let commit_ai_path_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("/chat/completions"));
        let commit_ai_model_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("model id or pick from /models"));
        let commit_ai_key_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("sk-… or leave empty"));
        let commit_ai_auth_header_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("x-api-key"));
        let commit_ai_auth_query_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("key"));
        let commit_ai_key_env_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("OPENAI_API_KEY"));
        let commit_ai_cli_bin_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("CLI path"));
        let commit_ai_timeout_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("seconds (empty = no timeout)")
        });
        let cursor_api_key_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("CURSOR_API_KEY (paste from Dashboard)")
        });
        let terminal_shell_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(super::terminal_shell::custom_command_placeholder())
        });
        // Code editor: line numbers + Ctrl/Cmd-F. soft_wrap(false) keeps the
        // gpui-base display_map on buffer lines only (visible-range paint), which
        // avoids wrap-map thrash on large files. Syntax stays plaintext — tree-sitter
        // is not enabled in this crate's gpui-kit features.
        // Reserve a 3px SCM gutter strip (empty collection keeps the width stable).
        let preview_input = cx.new(|cx| {
            EditorState::new(window, cx)
                .placeholder(t.preview_placeholder)
                .language("plaintext")
                .soft_wrap(false)
                .gutter_decorations_width(px(3.))
        });
        let preview_input_old = cx.new(|cx| {
            EditorState::new(window, cx)
                .placeholder(t.preview_placeholder)
                .language("plaintext")
                .soft_wrap(false)
                .gutter_decorations_width(px(3.))
        });
        // History Diff uses gpui-kit Diff (split pairs empty cells opposite
        // pure adds/deletes; line numbers stay on the source side only).
        let history_diff = cx.new(|cx| {
            DiffState::new([], cx)
                .with_context_lines(None)
                .with_mode(DiffMode::Split)
        });
        let preview_diff_gutter = preview_input.update(cx, |state, cx| {
            state.create_gutter_decorations_collection(Vec::new(), cx)
        });
        let preview_diff_ruler = preview_input.update(cx, |state, cx| {
            state.create_overview_ruler_decorations_collection(Vec::new(), cx)
        });
        let preview_diff_line_bg = preview_input.update(cx, |state, cx| {
            state.create_range_decorations_collection(Vec::new(), cx)
        });
        let preview_diff_gutter_old = preview_input_old.update(cx, |state, cx| {
            state.create_gutter_decorations_collection(Vec::new(), cx)
        });
        let preview_diff_line_bg_old = preview_input_old.update(cx, |state, cx| {
            state.create_range_decorations_collection(Vec::new(), cx)
        });

        custom_editor_input.update(cx, |input, cx| {
            input.set_value(&prefs.editor.custom_command, window, cx);
        });
        // UI / terminal font inputs are search filters only (start empty).
        terminal_shell_input.update(cx, |input, cx| {
            input.set_value(&prefs.terminal_shell.custom_command, window, cx);
        });

        let status_colors = prefs.status_colors.clone();
        // Placeholder pickers; values synced after theme apply.
        let status_color_pickers = StatusColorPickers {
            modified: cx.new(|cx| ColorPickerState::new(window, cx)),
            added: cx.new(|cx| ColorPickerState::new(window, cx)),
            deleted: cx.new(|cx| ColorPickerState::new(window, cx)),
            renamed: cx.new(|cx| ColorPickerState::new(window, cx)),
            untracked: cx.new(|cx| ColorPickerState::new(window, cx)),
            conflict: cx.new(|cx| ColorPickerState::new(window, cx)),
        };

        let initial_status = if start_path.is_some() {
            t.loading.into()
        } else {
            t.no_folder_open.into()
        };
        let settings_scroll = ScrollHandle::new();
        let settings_commit_ai_anchor = ScrollAnchor::for_handle(settings_scroll.clone());
        let mut view = Self {
            start_path,
            snapshot: None,
            snapshot_fp: 0,
            selected: HashSet::new(),
            selected_remote: "origin".into(),
            status_line: initial_status,
            commit_input,
            branch_input,
            branch_search_input,
            file_op_input,
            remote_name_input,
            remote_url_input,
            custom_editor_input,
            terminal_font_input,
            ui_font_input,
            system_font_names: None,
            preview_input,
            preview_input_old,
            history_diff,
            history_aligned_split: None,
            history_diff_scroll_sync: true,
            history_split_scroll_old: ScrollHandle::new(),
            history_split_scroll_new: ScrollHandle::new(),
            history_split_scroll_shared: ScrollHandle::new(),
            settings_scroll,
            settings_commit_ai_anchor,
            preview_diff_gutter,
            preview_diff_ruler,
            preview_diff_line_bg,
            preview_diff_gutter_old,
            preview_diff_line_bg_old,
            terminal_font_family: prefs.terminal_font_family.clone(),
            ui_font_family: prefs.ui_font_family.clone(),
            show_inline_blame: prefs.show_inline_blame,
            show_diff_line_background: prefs.show_diff_line_background,
            history_commits: Vec::new(),
            history_graph: HistoryGraph::default(),
            history_head_oid: None,
            history_selected: None,
            history_detail: None,
            history_rev: None,
            history_graph_excluded: HashSet::new(),
            history_branch_filter_input,
            _history_branch_filter_sub: None,
            history_branch_filter_open: false,
            history_file_preview: None,
            changes_file_view_mode: ChangesFileViewMode::default(),
            changes_diff_layout: HistoryDiffLayout::default(),
            history_files_as_tree: false,
            collapsed_history_dirs: HashSet::new(),
            blame_path: None,
            blame_rev: None,
            blame_lines: Vec::new(),
            blame_cursor_line: None,
            blame_scroll_y: None,
            terminal_font_size: prefs.terminal_font_size,
            terminal_line_height: prefs.terminal_line_height,
            terminal_shell: prefs.terminal_shell.clone(),
            terminal_shell_input,
            terminal_caret_on: true,
            terminal_caret_ticks: 0,
            preview_path: None,
            git_preview_path: None,
            preview_tabs: Vec::new(),
            preview_tab_index: 0,
            error: None,
            page: AppPage::Git,
            change_sort: prefs.change_sort.normalized(),
            group_by_status: prefs.group_by_status,
            changes_sections: Vec::new(),
            changes_flat_rows: Vec::new(),
            reload_task: None,
            preferred_editor: prefs.editor,
            theme_pref: prefs.theme,
            locale: prefs.locale,
            changes_as_tree: prefs.changes_as_tree,
            auto_refresh: prefs.auto_refresh,
            auto_check_updates: prefs.auto_check_updates,
            skipped_update_version: prefs.skipped_update_version,
            commit_ai: prefs.commit_ai.clone(),
            commit_ai_form: super::ai_provider::CommitAiFormMode::Closed,
            commit_ai_form_auth: super::ai_provider::AiAuthKind::Bearer,
            commit_ai_title_input,
            commit_ai_docs_input,
            commit_ai_base_input,
            commit_ai_path_input,
            commit_ai_model_input,
            commit_ai_key_input,
            commit_ai_auth_header_input,
            commit_ai_auth_query_input,
            commit_ai_key_env_input,
            commit_ai_cli_bin_input,
            commit_ai_timeout_input,
            cursor_api_key_input,
            generate_commit_abort: None,
            t3_import_status: None,
            t3_available: super::t3_bridge::t3_available(),
            commit_ai_models: Vec::new(),
            commit_ai_models_error: None,
            commit_ai_models_task: None,
            commit_ai_install_cli_task: None,
            cli_install_progress: None,
            cli_status_cursor: None,
            cli_status_codex: None,
            cli_status_task: None,
            generate_commit_task: None,
            update_available: None,
            update_task: None,
            show_remote_branches: prefs.show_remote_branches,
            ask_fetch_on_open: prefs.ask_fetch_on_open,
            fetch_prompt_muted: prefs.fetch_prompt_muted,
            recent_repos: prefs.recent_repos,
            status_colors,
            status_color_pickers,
            _status_color_subs: Vec::new(),
            _branch_search_sub: None,
            _font_search_subs: Vec::new(),
            _commit_input_sub: None,
            _preview_blame_sub: None,
            _preview_diff_blame_sub: None,
            _window_bounds_sub: None,
            expanded_dirs: HashSet::new(),
            files_tree_nodes: Vec::new(),
            files_status_index: TreeStatusIndex::default(),
            collapsed_change_dirs: HashSet::new(),
            collapsed_change_sections: HashSet::new(),
            nav_history: vec![NavEntry::page_only(AppPage::Git)],
            history_index: 0,
            navigating_history: false,
            branch_form: BranchForm::Hidden,
            file_op_form: FileOpForm::Hidden,
            remote_form: RemoteForm::Hidden,
            sidebar_collapsed: false,
            busy: None,
            progress_log: Vec::new(),
            progress_collapsed: false,
            progress_panel_height: PROGRESS_DEFAULT_HEIGHT,
            terminal: TerminalDock::default(),
            terminal_focus: cx.focus_handle(),
            terminal_focused: false,
            terminal_poll_task: None,
            changes_list_compact: false,
            changes_section_hide_labels: HashMap::new(),
            changes_split_view: false,
            remote_actions_compact: false,
            branch_search_compact: false,
            branch_search_open: false,
            files_tree_compact: false,
            fetch_dont_ask: false,
            fetch_prompted_root: None,
            focus_handle: focus_handle.clone(),
            refresh_task: None,
        };
        view.apply_theme(cx);
        view.wire_status_color_pickers(window, cx);
        view.wire_branch_search(cx);
        view.wire_history_branch_filter(cx);
        view.wire_font_search_inputs(cx);
        view.ensure_system_fonts(cx);
        view.wire_commit_input(window, cx);
        view.wire_preview_blame(cx);
        view.wire_window_bounds(window, cx);
        view.sync_cursor_api_key_input(window, cx);
        view.sync_commit_ai_timeout_input(window, cx);
        view.start_terminal_poll(cx);
        if view.start_path.is_some() {
            view.reload(cx);
            view.sync_auto_refresh(cx);
            // Root (dialogs/sheets) is wrapped around this view after construction.
            // Defer the fetch prompt until the window host is ready.
            cx.defer_in(window, |this, window, cx| {
                this.maybe_prompt_fetch(window, cx);
            });
        } else {
            view.sync_auto_refresh(cx);
        }
        if view.auto_check_updates {
            cx.defer_in(window, |this, window, cx| {
                this.check_for_updates(true, window, cx);
            });
        }
        window.focus(&focus_handle, cx);
        view
    }

    pub(super) fn has_repo(&self) -> bool {
        self.snapshot.is_some()
    }

    pub(super) fn t(&self) -> UiText {
        self.locale.text()
    }

    pub(super) fn can_go_back(&self) -> bool {
        self.history_index > 0
    }

    pub(super) fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.nav_history.len()
    }

    pub(super) fn current_nav_entry(&self) -> NavEntry {
        NavEntry {
            page: self.page,
            preview: match self.page {
                AppPage::Git => self.git_preview_path.clone(),
                AppPage::Files => self.preview_path.clone(),
                AppPage::History => self
                    .history_file_preview
                    .as_ref()
                    .map(|p| p.path.clone()),
                AppPage::Settings => None,
            },
        }
    }

    pub(super) fn push_nav_entry(&mut self, entry: NavEntry) {
        if self.navigating_history {
            return;
        }
        if self
            .nav_history
            .get(self.history_index)
            .is_some_and(|cur| cur == &entry)
        {
            return;
        }
        self.nav_history.truncate(self.history_index + 1);
        self.nav_history.push(entry);
        self.history_index = self.nav_history.len() - 1;
    }

    fn apply_nav_entry(
        &mut self,
        entry: NavEntry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigating_history = true;
        if self.page == AppPage::Files {
            self.stash_active_preview_tab(cx);
        }
        self.page = entry.page;
        match entry.page {
            AppPage::Git => match entry.preview {
                Some(path) => {
                    self.git_preview_path = Some(path.clone());
                    self.ensure_parents_expanded(&path);
                    self.load_preview_into(&path, window, cx);
                }
                None => {
                    self.git_preview_path = None;
                    self.preview_path = None;
                    self.clear_shared_preview_buffers(window, cx);
                }
            },
            AppPage::Files => match entry.preview {
                Some(path) => {
                    self.ensure_parents_expanded(&path);
                    if let Some(idx) = self.preview_tabs.iter().position(|t| t.path == path) {
                        self.select_preview_tab(idx, window, cx);
                    } else {
                        self.preview_file(path, window, cx);
                    }
                }
                None => {
                    self.preview_tabs.clear();
                    self.preview_tab_index = 0;
                    self.preview_path = None;
                    self.clear_shared_preview_buffers(window, cx);
                }
            },
            AppPage::History => {
                // Nav only stores the path; restore the retained History session if any.
                self.restore_page_preview(window, cx);
            }
            AppPage::Settings => {}
        }
        self.navigating_history = false;
    }

    pub(super) fn repo_root(&self) -> PathBuf {
        self.snapshot
            .as_ref()
            .map(|s| s.root.clone())
            .or_else(|| self.start_path.clone())
            .unwrap_or_default()
    }

    pub(super) fn repo_root_display(&self) -> String {
        if self.start_path.is_none() && self.snapshot.is_none() {
            return self.t().no_folder_open.to_string();
        }
        display_path(&self.repo_root())
    }

    pub(super) fn with_repo<F, T>(&self, f: F) -> Result<T, SharedString>
    where
        F: FnOnce(&RepoState) -> Result<T, sgg_core::RepoError>,
    {
        let start = self
            .snapshot
            .as_ref()
            .map(|s| s.root.clone())
            .or_else(|| self.start_path.clone())
            .ok_or_else(|| SharedString::from(self.t().no_folder_open))?;
        let repo = RepoState::open_nearest(&start).map_err(|e| SharedString::from(e.to_string()))?;
        f(&repo).map_err(|e| SharedString::from(e.to_string()))
    }

    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        self.queue_reload(true, true, cx);
    }

    pub(super) fn reload_auto(&mut self, cx: &mut Context<Self>) {
        // Skip if a reload is already in flight (coalesce mouse-time refreshes).
        if self.reload_task.is_some() {
            return;
        }
        self.queue_reload(false, self.page == AppPage::Files, cx);
    }

    fn queue_reload(
        &mut self,
        clear_selection: bool,
        build_files_tree: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(start) = self.start_path.clone() else {
            self.snapshot = None;
            self.snapshot_fp = 0;
            self.selected.clear();
            self.files_tree_nodes.clear();
            self.changes_sections.clear();
            self.changes_flat_rows.clear();
            self.files_status_index = TreeStatusIndex::default();
            self.clear_history_state();
            self.clear_blame_state();
            self.error = None;
            self.status_line = self.t().no_folder_open.into();
            cx.notify();
            return;
        };

        let expanded = self.expanded_dirs.clone();
        // Full ignore scan only on Files page / manual reload.
        let check_ignored = build_files_tree && (clear_selection || self.page == AppPage::Files);
        self.reload_task = Some(cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_spawn(async move {
                    load_repo(start, expanded, build_files_tree, check_ignored)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.reload_task = None;
                this.apply_loaded_repo(loaded, clear_selection, build_files_tree, cx);
            });
        }));
    }

    fn apply_loaded_repo(
        &mut self,
        loaded: Result<LoadedRepo, LoadRepoError>,
        clear_selection: bool,
        build_files_tree: bool,
        cx: &mut Context<Self>,
    ) {
        match loaded {
            Ok(loaded) => {
                if !clear_selection && self.error.is_none() && self.snapshot_fp == loaded.fingerprint
                {
                    return;
                }
                self.sync_selected_remote(&loaded.snap);
                if clear_selection {
                    self.selected.clear();
                } else {
                    let paths: HashSet<_> =
                        loaded.snap.changes.iter().map(|c| c.path.as_str()).collect();
                    self.selected.retain(|p| paths.contains(p.as_str()));
                }
                self.snapshot_fp = loaded.fingerprint;
                self.status_line = loaded.status_line.into();
                self.error = None;
                if remember_repo(&mut self.recent_repos, &loaded.snap.root) {
                    self.persist_prefs(cx);
                }
                self.snapshot = Some(loaded.snap);
                self.rebuild_changes_sections();
                self.rebuild_files_status_index();
                if build_files_tree {
                    self.files_tree_nodes = loaded.files_tree;
                }
                self.refresh_preview_diff_gutter(cx);
                // WIP row appears/disappears with the dirty working tree.
                if self.page == AppPage::History {
                    self.rebuild_history_graph();
                    if self
                        .history_selected
                        .as_deref()
                        .is_some_and(sgg_core::is_uncommitted_oid)
                    {
                        if self.history_has_uncommitted() {
                            let root = self.repo_root();
                            if let Ok(detail) = sgg_core::show_uncommitted(&root) {
                                self.history_detail = Some(detail);
                            }
                        } else {
                            self.history_selected = None;
                            self.history_detail = None;
                        }
                    }
                }
            }
            Err(LoadRepoError::Snapshot(err)) => {
                self.error = Some(err.into());
                self.status_line = "Failed to read repository".into();
                self.snapshot_fp = 0;
                self.changes_sections.clear();
                self.changes_flat_rows.clear();
                self.files_status_index = TreeStatusIndex::default();
            }
            Err(LoadRepoError::NoRepo(err)) => {
                self.snapshot = None;
                self.snapshot_fp = 0;
                self.files_tree_nodes.clear();
                self.changes_sections.clear();
                self.changes_flat_rows.clear();
                self.files_status_index = TreeStatusIndex::default();
                self.error = Some(err.into());
                self.status_line = self.t().no_folder_open.into();
            }
        }
        cx.notify();
    }

    pub(super) fn rebuild_changes_sections(&mut self) {
        let changes = self
            .snapshot
            .as_ref()
            .map(|s| s.changes.as_slice())
            .unwrap_or(&[]);
        self.changes_sections =
            group_and_sort(changes, self.change_sort, self.group_by_status);
        self.rebuild_changes_flat_rows();
    }

    pub(super) fn rebuild_changes_flat_rows(&mut self) {
        let as_tree = self.changes_as_tree;
        let collapsed = &self.collapsed_change_dirs;
        let group_by_file_status = self.group_by_status;
        self.changes_flat_rows = self
            .changes_sections
            .iter()
            .map(|section| {
                section_flat_rows(
                    section.title,
                    &section.items,
                    as_tree,
                    group_by_file_status,
                    collapsed,
                )
            })
            .collect();
    }

    pub(super) fn rebuild_files_status_index(&mut self) {
        let changes = self
            .snapshot
            .as_ref()
            .map(|s| s.changes.as_slice())
            .unwrap_or(&[]);
        self.files_status_index = TreeStatusIndex::from_changes(changes);
    }

    pub(super) fn rebuild_files_tree(&mut self) {
        let root = self.repo_root();
        if root.as_os_str().is_empty() {
            self.files_tree_nodes.clear();
            return;
        }
        self.files_tree_nodes = list_tree(&root, &self.expanded_dirs);
    }

    fn sync_selected_remote(&mut self, snap: &sgg_core::RepoSnapshot) {
        if snap
            .remotes
            .iter()
            .any(|r| r.name == self.selected_remote)
        {
            return;
        }
        self.selected_remote = snap
            .remotes
            .first()
            .map(|r| r.name.clone())
            .unwrap_or_else(|| "origin".into());
    }

    pub(super) fn set_selected(&mut self, path: String, selected: bool, cx: &mut Context<Self>) {
        if selected {
            self.selected.insert(path);
        } else {
            self.selected.remove(&path);
        }
        cx.notify();
    }

    /// Select or deselect every path in `paths` (used by Changes folder checkboxes).
    pub(super) fn set_selected_many(
        &mut self,
        paths: &[String],
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            return;
        }
        if selected {
            self.selected.extend(paths.iter().cloned());
        } else {
            for path in paths {
                self.selected.remove(path);
            }
        }
        cx.notify();
    }

    /// Paths listed under a Changes section title (`Staged` / `Unstaged` / …).
    pub(super) fn paths_in_section(&self, title: &str) -> Vec<String> {
        self.changes_sections
            .iter()
            .find(|s| s.title == title)
            .map(|s| s.items.iter().map(|c| c.path.clone()).collect())
            .unwrap_or_default()
    }

    pub(super) fn section_all_selected(&self, title: &str) -> bool {
        let paths = self.paths_in_section(title);
        !paths.is_empty() && paths.iter().all(|p| self.selected.contains(p))
    }

    /// Toggle checkbox selection for every file in a Changes section.
    pub(super) fn toggle_select_section(&mut self, title: impl AsRef<str>, cx: &mut Context<Self>) {
        let title = title.as_ref();
        let paths = self.paths_in_section(title);
        if paths.is_empty() {
            return;
        }
        let select = !self.section_all_selected(title);
        self.set_selected_many(&paths, select, cx);
    }

    pub(super) fn select_remote(&mut self, name: String, cx: &mut Context<Self>) {
        self.selected_remote = name;
        cx.notify();
    }

    pub(super) fn set_page(
        &mut self,
        page: AppPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if page == self.page {
            return;
        }
        if self.page == AppPage::Files {
            self.stash_active_preview_tab(cx);
        }
        self.page = page;
        // Auto-refresh may skip the files tree while on Git; rebuild when opening Files.
        if page == AppPage::Files && self.has_repo() {
            self.rebuild_files_tree();
        }
        if page == AppPage::Settings {
            self.ensure_system_fonts(cx);
            self.refresh_cli_status(cx);
        }
        self.restore_page_preview(window, cx);
        self.push_nav_entry(self.current_nav_entry());
        cx.notify();
    }

    /// Open Settings and scroll to the Commit AI card.
    pub(super) fn open_commit_ai_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page != AppPage::Settings {
            self.set_page(AppPage::Settings, window, cx);
        }
        cx.defer_in(window, |this, window, cx| {
            this.settings_commit_ai_anchor.scroll_to(window, cx);
            cx.notify();
        });
    }

    /// Probe Cursor/Codex CLI on a background thread; Settings reads the cache only.
    pub(super) fn refresh_cli_status(&mut self, cx: &mut Context<Self>) {
        if self.cli_status_task.is_some() {
            return;
        }
        self.cli_status_task = Some(cx.spawn(async move |this, cx| {
            let (cursor, codex) = cx
                .background_spawn(async {
                    (
                        super::cli_tools::status(super::cli_tools::CliToolKind::Cursor),
                        super::cli_tools::status(super::cli_tools::CliToolKind::Codex),
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.cli_status_cursor = Some(cursor);
                this.cli_status_codex = Some(codex);
                this.cli_status_task = None;
                cx.notify();
            });
        }));
    }

    /// Load installed font family names once (Settings font pickers).
    pub(super) fn ensure_system_fonts(&mut self, cx: &App) {
        if self.system_font_names.is_some() {
            return;
        }
        self.system_font_names = Some(cx.text_system().all_font_names());
    }

    pub(super) fn history_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_go_back() {
            return;
        }
        self.history_index -= 1;
        let entry = self.nav_history[self.history_index].clone();
        self.apply_nav_entry(entry, window, cx);
        cx.notify();
    }

    pub(super) fn history_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_go_forward() {
            return;
        }
        self.history_index += 1;
        let entry = self.nav_history[self.history_index].clone();
        self.apply_nav_entry(entry, window, cx);
        cx.notify();
    }

    pub(super) fn set_change_sort(&mut self, sort: ChangeSort, cx: &mut Context<Self>) {
        self.change_sort = sort.normalized();
        self.rebuild_changes_sections();
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn toggle_change_sort_name(&mut self, cx: &mut Context<Self>) {
        let next = match self.change_sort.normalized() {
            ChangeSort::NameAsc => ChangeSort::NameDesc,
            ChangeSort::NameDesc => ChangeSort::NameAsc,
            ChangeSort::Legacy => ChangeSort::NameDesc,
        };
        self.set_change_sort(next, cx);
    }

    pub(super) fn toggle_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    pub(super) fn close_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.page {
            AppPage::History => {
                self.close_history_file_preview(window, cx);
            }
            AppPage::Files => {
                if !self.preview_tabs.is_empty() {
                    self.close_preview_tab(self.preview_tab_index, window, cx);
                }
            }
            AppPage::Git => {
                if self.git_preview_path.is_none() {
                    return;
                }
                self.git_preview_path = None;
                self.preview_path = None;
                self.clear_shared_preview_buffers(window, cx);
                self.push_nav_entry(self.current_nav_entry());
                cx.notify();
            }
            AppPage::Settings => {}
        }
    }

    pub(super) fn stash_active_preview_tab(&mut self, cx: &App) {
        let Some(path) = self.preview_path.clone() else {
            return;
        };
        let content = self.preview_input.read(cx).value().to_string();
        if let Some(tab) = self.preview_tabs.iter_mut().find(|t| t.path == path) {
            tab.content = content;
        }
    }

    pub(super) fn select_preview_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.preview_tabs.len() {
            return;
        }
        if index != self.preview_tab_index {
            self.stash_active_preview_tab(cx);
        }
        self.preview_tab_index = index;
        let path = self.preview_tabs[index].path.clone();
        let content = self.preview_tabs[index].content.clone();
        self.preview_input.update(cx, |input, cx| {
            input.set_value(&content, window, cx);
        });
        self.preview_path = Some(path.clone());
        self.ensure_parents_expanded(&path);
        self.clear_blame_state();
        self.refresh_inline_blame(cx);
        self.refresh_preview_diff_gutter(cx);
        self.push_nav_entry(self.current_nav_entry());
        cx.notify();
    }

    pub(super) fn close_preview_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.preview_tabs.len() {
            return;
        }
        self.stash_active_preview_tab(cx);
        self.preview_tabs.remove(index);
        if self.preview_tabs.is_empty() {
            self.preview_tab_index = 0;
            self.preview_path = None;
            self.clear_blame_state();
            self.clear_preview_diff_gutter(cx);
            self.preview_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            self.push_nav_entry(self.current_nav_entry());
            cx.notify();
            return;
        }
        let next = index.min(self.preview_tabs.len() - 1);
        self.preview_tab_index = next;
        let path = self.preview_tabs[next].path.clone();
        let content = self.preview_tabs[next].content.clone();
        self.preview_input.update(cx, |input, cx| {
            input.set_value(&content, window, cx);
        });
        self.preview_path = Some(path);
        self.clear_blame_state();
        self.refresh_inline_blame(cx);
        self.refresh_preview_diff_gutter(cx);
        self.push_nav_entry(self.current_nav_entry());
        cx.notify();
    }

    pub(super) fn set_group_by_status(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.group_by_status = enabled;
        self.rebuild_changes_sections();
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_show_remote_branches(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.show_remote_branches = enabled;
        self.persist_prefs(cx);
        cx.notify();
    }

    /// Open a repository from a remembered path (welcome / settings).
    pub(super) fn open_repo_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.fetch_prompted_root = None;
        self.start_path = Some(absolute_path(path));
        self.page = AppPage::Git;
        self.reload(cx);
        self.sync_auto_refresh(cx);
        cx.notify();
    }

    pub(super) fn set_editor_preset(&mut self, preset: EditorPreset, cx: &mut Context<Self>) {
        self.preferred_editor.preset = preset;
        self.sync_custom_command_from_input(cx);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_theme_preference(
        &mut self,
        pref: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_pref = pref;
        self.apply_theme(cx);
        // Refresh pickers that still follow the theme (overrides keep their hex).
        self.sync_status_color_pickers(window, cx);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_locale(&mut self, locale: Locale, window: &mut Window, cx: &mut Context<Self>) {
        self.locale = locale;
        self.apply_locale_placeholders(window, cx);
        self.persist_prefs(cx);
        cx.notify();
    }

    fn apply_locale_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let t = self.t();
        self.commit_input.update(cx, |input, cx| {
            input.set_placeholder(t.commit_placeholder, window, cx);
        });
        let branch_ph = match &self.branch_form {
            BranchForm::Rename { .. } => t.rename_placeholder,
            _ => t.branch_placeholder,
        };
        self.branch_input.update(cx, |input, cx| {
            input.set_placeholder(branch_ph, window, cx);
        });
        self.branch_search_input.update(cx, |input, cx| {
            input.set_placeholder(t.branch_search_placeholder, window, cx);
        });
        self.history_branch_filter_input.update(cx, |input, cx| {
            input.set_placeholder(t.history_branch_filter_placeholder, window, cx);
        });
        self.preview_input.update(cx, |input, cx| {
            input.set_placeholder(t.preview_placeholder, window, cx);
        });
        self.remote_name_input.update(cx, |input, cx| {
            input.set_placeholder(t.remote_name_placeholder, window, cx);
        });
        self.remote_url_input.update(cx, |input, cx| {
            input.set_placeholder(t.remote_url_placeholder, window, cx);
        });
    }

    fn wire_branch_search(&mut self, cx: &mut Context<Self>) {
        let input = self.branch_search_input.clone();
        self._branch_search_sub = Some(cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        }));
    }

    fn wire_history_branch_filter(&mut self, cx: &mut Context<Self>) {
        let input = self.history_branch_filter_input.clone();
        self._history_branch_filter_sub =
            Some(cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
    }

    fn wire_font_search_inputs(&mut self, cx: &mut Context<Self>) {
        self._font_search_subs.clear();
        for input in [
            self.ui_font_input.clone(),
            self.terminal_font_input.clone(),
        ] {
            let sub = cx.subscribe(&input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
            self._font_search_subs.push(sub);
        }
    }

    fn wire_commit_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.commit_input.clone();
        // Ctrl/Cmd+Enter (secondary) commits; plain Enter inserts a newline in the textarea.
        self._commit_input_sub = Some(cx.subscribe_in(
            &input,
            window,
            |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter {
                    secondary: true, ..
                } = event
                {
                    this.commit(window, cx);
                }
            },
        ));
    }

    fn wire_window_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._window_bounds_sub = Some(cx.observe_window_bounds(window, |this, window, cx| {
            if !this.terminal.visible {
                return;
            }
            let width: f32 = window.bounds().size.width.into();
            if this.terminal.maybe_resize_from_px(
                width,
                this.terminal.height(),
                this.terminal_font_size,
                this.terminal_line_height,
            ) {
                this.terminal.refresh_lines();
                cx.notify();
            }
        }));
    }

    fn wire_preview_blame(&mut self, cx: &mut Context<Self>) {
        let input = self.preview_input.clone();
        self._preview_blame_sub = Some(cx.observe(&input, |this, input, cx| {
            if !this.show_inline_blame || this.preview_path.is_none() {
                return;
            }
            let editor = input.read(cx);
            let line = editor.cursor_position().line as usize;
            // Re-render when the caret line or scroll moves so blame stays on-row.
            let scroll_y = f32::from(editor.scroll_offset().y);
            let prev_scroll = this.blame_scroll_y;
            let line_changed = this.blame_cursor_line != Some(line);
            let scroll_changed = prev_scroll.is_none_or(|y| (y - scroll_y).abs() > 0.5);
            if line_changed || scroll_changed {
                this.blame_cursor_line = Some(line);
                this.blame_scroll_y = Some(scroll_y);
                cx.notify();
            }
        }));
        let diff = self.history_diff.clone();
        self._preview_diff_blame_sub = Some(cx.observe(&diff, |this, _, cx| {
            if !this.show_inline_blame || this.preview_path.is_none() {
                return;
            }
            if this.preview_diff_layout() != Some(HistoryDiffLayout::Unified) {
                return;
            }
            cx.notify();
        }));
    }

    /// Revision for blame of the open preview (`None` = working tree).
    pub(super) fn blame_revision(&self) -> Option<String> {
        if self.page != AppPage::History {
            return None;
        }
        let hp = self.history_file_preview.as_ref()?;
        if sgg_core::is_uncommitted_oid(&hp.oid) {
            None
        } else {
            Some(hp.oid.clone())
        }
    }

    /// Toggle Split Diff scroll sync. When on, both panes share one ScrollHandle
    /// so gutters and text stay locked with zero lag between sides.
    pub(super) fn set_history_diff_scroll_sync(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.history_diff_scroll_sync == enabled {
            return;
        }
        if enabled {
            let offset = self.history_split_scroll_new.offset();
            self.history_split_scroll_shared.set_offset(offset);
        } else {
            let offset = self.history_split_scroll_shared.offset();
            self.history_split_scroll_old.set_offset(offset);
            self.history_split_scroll_new.set_offset(offset);
        }
        self.history_diff_scroll_sync = enabled;
        cx.notify();
    }

    /// Scroll handle for one Split Diff pane (shared when sync is on).
    pub(super) fn history_split_scroll_handle(&self, old_side: bool) -> &ScrollHandle {
        if self.history_diff_scroll_sync {
            &self.history_split_scroll_shared
        } else if old_side {
            &self.history_split_scroll_old
        } else {
            &self.history_split_scroll_new
        }
    }

    pub(super) fn set_show_inline_blame(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.show_inline_blame = enabled;
        if enabled {
            self.refresh_inline_blame(cx);
        } else {
            self.clear_blame_state();
        }
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_show_diff_line_background(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        self.show_diff_line_background = enabled;
        self.refresh_preview_diff_gutter(cx);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_ui_font_family(
        &mut self,
        family: impl Into<String>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let family = family.into().trim().to_string();
        self.ui_font_family = family;
        self.apply_theme(cx);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn apply_ui_font_from_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let family = self.ui_font_input.read(cx).value().to_string();
        self.set_ui_font_family(family, window, cx);
    }

    pub(super) fn reset_ui_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui_font_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.set_ui_font_family(String::new(), window, cx);
    }

    pub(super) fn clear_history_state(&mut self) {
        self.history_commits.clear();
        self.history_graph = HistoryGraph::default();
        self.history_head_oid = None;
        self.history_selected = None;
        self.history_detail = None;
        self.history_rev = None;
        self.history_graph_excluded.clear();
        self.history_branch_filter_open = false;
        self.history_file_preview = None;
    }

    /// Working tree / index has local changes → show the gray WIP tip on History.
    pub(super) fn history_has_uncommitted(&self) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|s| !s.changes.is_empty())
    }

    /// Whether `branch` is included in the History graph (default: yes).
    pub(super) fn history_graph_branch_visible(&self, branch: &str) -> bool {
        !self.history_graph_excluded.contains(branch)
    }

    pub(super) fn toggle_history_branch_filter(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.history_branch_filter_open = !self.history_branch_filter_open;
        if self.history_branch_filter_open {
            self.history_branch_filter_input.update(cx, |input, cx| {
                input.focus(window, cx);
            });
        }
        cx.notify();
    }

    pub(super) fn is_history_preview(&self) -> bool {
        self.page == AppPage::History && self.history_file_preview.is_some()
    }

    pub(super) fn rebuild_history_graph(&mut self) {
        use sgg_core::{
            build_history_graph, GraphCommitInput, GRAPH_ACCENT_COUNT, UNCOMMITTED_OID,
        };

        let mut inputs: Vec<GraphCommitInput> = Vec::with_capacity(self.history_commits.len() + 1);
        // Tip of the graph: gray uncommitted node parented at HEAD when dirty.
        if self.history_has_uncommitted() {
            let parents = self
                .history_head_oid
                .clone()
                .into_iter()
                .collect::<Vec<_>>();
            inputs.push(GraphCommitInput {
                oid: UNCOMMITTED_OID.to_string(),
                parents,
            });
        }
        inputs.extend(self.history_commits.iter().map(|c| GraphCommitInput {
            oid: c.full_oid.clone(),
            parents: c.parents.clone(),
        }));
        self.history_graph = build_history_graph(&inputs, GRAPH_ACCENT_COUNT);
    }

    pub(super) fn clear_blame_state(&mut self) {
        self.blame_path = None;
        self.blame_rev = None;
        self.blame_lines.clear();
        self.blame_cursor_line = None;
        self.blame_scroll_y = None;
    }

    pub(super) fn clear_preview_diff_gutter(&mut self, cx: &mut Context<Self>) {
        self.preview_diff_gutter.clear(cx);
        self.preview_diff_ruler.clear(cx);
        self.preview_diff_line_bg.clear(cx);
        self.preview_diff_gutter_old.clear(cx);
        self.preview_diff_line_bg_old.clear(cx);
    }

    /// Active Split Diff target `(oid, path)`, if the preview is showing one.
    pub(super) fn split_diff_target(&self) -> Option<(String, String)> {
        if self.page == AppPage::History {
            let preview = self.history_file_preview.as_ref()?;
            if preview.mode == HistoryFileViewMode::Diff
                && preview.diff_layout == HistoryDiffLayout::Split
            {
                return Some((preview.oid.clone(), preview.path.clone()));
            }
            return None;
        }
        if self.page == AppPage::Git
            && self.changes_file_view_mode == ChangesFileViewMode::Diff
            && self.changes_diff_layout == HistoryDiffLayout::Split
        {
            return self
                .git_preview_path
                .clone()
                .or_else(|| self.preview_path.clone())
                .map(|path| (sgg_core::UNCOMMITTED_OID.to_string(), path));
        }
        None
    }

    /// Whether the preview body should render Unified / Split Diff (not the file editor).
    pub(super) fn preview_diff_layout(&self) -> Option<HistoryDiffLayout> {
        if self.page == AppPage::History {
            let preview = self.history_file_preview.as_ref()?;
            return if preview.mode == HistoryFileViewMode::Diff {
                Some(preview.diff_layout)
            } else {
                None
            };
        }
        if self.page == AppPage::Git && self.changes_file_view_mode == ChangesFileViewMode::Diff {
            return Some(self.changes_diff_layout);
        }
        None
    }

    /// VS Code-style SCM bars: added / modified lines + deletion pills in the gutter.
    pub(super) fn refresh_preview_diff_gutter(&mut self, cx: &mut Context<Self>) {
        if self.page == AppPage::History && self.history_file_preview.is_some() {
            self.refresh_history_diff_decorations(cx);
            return;
        }
        if self.page == AppPage::Git && self.changes_file_view_mode == ChangesFileViewMode::Diff {
            self.refresh_history_diff_decorations(cx);
            return;
        }
        let Some(path) = self.preview_path.clone() else {
            self.clear_preview_diff_gutter(cx);
            return;
        };
        if !self.has_repo() {
            self.clear_preview_diff_gutter(cx);
            return;
        }

        let root = self.repo_root();
        let text = self.preview_input.read(cx).value().to_string();
        let changes = match list_file_line_changes(&root, &path) {
            Ok(c) => c,
            Err(_) => {
                self.clear_preview_diff_gutter(cx);
                return;
            }
        };

        let added = self.effective_status_color("added", cx);
        let modified = self.effective_status_color("modified", cx);
        let deleted = self.effective_status_color("deleted", cx);
        let line_bg = self.show_diff_line_background;
        // Soft tint over the full line (gutter bar keeps full-strength color).
        const LINE_BG_OPACITY: f32 = 0.18;

        let mut gutter = Vec::with_capacity(changes.len());
        let mut ruler = Vec::with_capacity(changes.len());
        let mut fills = Vec::new();
        for change in changes {
            match change {
                LineChange::Lines { kind, start, end } if end > start => {
                    let range = lines_byte_range(&text, start, end);
                    if range.start >= range.end && !text.is_empty() {
                        continue;
                    }
                    let color = match kind {
                        LineChangeKind::Added => added,
                        LineChangeKind::Modified => modified,
                        LineChangeKind::Deleted => deleted,
                    };
                    gutter.push(GutterDecoration::new(range.clone()).with_color(color));
                    ruler.push(
                        OverviewRulerDecoration::new(range.clone())
                            .with_lane(OverviewRulerLane::Left)
                            .with_color(color.opacity(0.7)),
                    );
                    if line_bg && matches!(kind, LineChangeKind::Added | LineChangeKind::Modified)
                    {
                        fills.push(
                            RangeDecoration::new(range)
                                .with_style(RangeDecorationStyle::Fill)
                                .with_color(color.opacity(LINE_BG_OPACITY)),
                        );
                    }
                }
                LineChange::DeletedBefore { before } => {
                    let offset = line_start_offset(&text, before);
                    let range = offset..offset;
                    gutter.push(GutterDecoration::new(range.clone()).with_color(deleted));
                    ruler.push(
                        OverviewRulerDecoration::new(range)
                            .with_lane(OverviewRulerLane::Left)
                            .with_color(deleted.opacity(0.7)),
                    );
                }
                LineChange::Lines { .. } => {}
            }
        }

        self.preview_diff_gutter.set(gutter, cx);
        self.preview_diff_ruler.set(ruler, cx);
        self.preview_diff_line_bg.set(fills, cx);
    }

    /// Gutter + fills for Split Diff (aligned dual editors: History or Changes).
    pub(super) fn refresh_history_diff_decorations(&mut self, cx: &mut Context<Self>) {
        let Some((oid, path)) = self.split_diff_target() else {
            self.clear_preview_diff_gutter(cx);
            return;
        };

        let root = self.repo_root();
        let patch = sgg_core::diff_file_at_commit(&root, &oid, &path).unwrap_or_default();
        let Some(aligned) = sgg_core::align_split_from_unified(&patch) else {
            self.clear_preview_diff_gutter(cx);
            return;
        };

        let old_text = self.preview_input_old.read(cx).value().to_string();
        let new_text = self.preview_input.read(cx).value().to_string();
        let added = self.effective_status_color("added", cx);
        let modified = self.effective_status_color("modified", cx);
        let deleted = self.effective_status_color("deleted", cx);
        const LINE_BG_OPACITY: f32 = 0.18;
        let line_bg = self.show_diff_line_background;

        let mut gutter_old = Vec::new();
        let mut fills_old = Vec::new();
        for (kind, start, end) in aligned.old_marks() {
            if end <= start {
                continue;
            }
            let range = lines_byte_range(&old_text, start, end);
            let color = match kind {
                LineChangeKind::Deleted => deleted,
                LineChangeKind::Modified => modified,
                LineChangeKind::Added => added,
            };
            gutter_old.push(GutterDecoration::new(range.clone()).with_color(color));
            if line_bg {
                fills_old.push(
                    RangeDecoration::new(range)
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(color.opacity(LINE_BG_OPACITY)),
                );
            }
        }

        let mut gutter_new = Vec::new();
        let mut fills_new = Vec::new();
        let mut ruler = Vec::new();
        for (kind, start, end) in aligned.new_marks() {
            if end <= start {
                continue;
            }
            let range = lines_byte_range(&new_text, start, end);
            let color = match kind {
                LineChangeKind::Added => added,
                LineChangeKind::Modified => modified,
                LineChangeKind::Deleted => deleted,
            };
            gutter_new.push(GutterDecoration::new(range.clone()).with_color(color));
            ruler.push(
                OverviewRulerDecoration::new(range.clone())
                    .with_lane(OverviewRulerLane::Left)
                    .with_color(color.opacity(0.7)),
            );
            if line_bg {
                fills_new.push(
                    RangeDecoration::new(range)
                        .with_style(RangeDecorationStyle::Fill)
                        .with_color(color.opacity(LINE_BG_OPACITY)),
                );
            }
        }

        self.preview_diff_gutter_old.set(gutter_old, cx);
        self.preview_diff_line_bg_old.set(fills_old, cx);
        self.preview_diff_gutter.set(gutter_new, cx);
        self.preview_diff_line_bg.set(fills_new, cx);
        self.preview_diff_ruler.set(ruler, cx);
    }

    pub(super) fn refresh_inline_blame(&mut self, cx: &mut Context<Self>) {
        if !self.show_inline_blame {
            self.clear_blame_state();
            return;
        }
        let Some(path) = self.preview_path.clone() else {
            self.clear_blame_state();
            return;
        };
        let rev = self.blame_revision();
        if self.blame_path.as_deref() == Some(path.as_str())
            && self.blame_rev == rev
            && !self.blame_lines.is_empty()
        {
            return;
        }
        let root = self.repo_root();
        self.blame_path = Some(path.clone());
        self.blame_rev = rev.clone();
        self.blame_lines.clear();
        self.blame_cursor_line = None;
        cx.spawn(async move |this, cx| {
            let blamed_path = path.clone();
            let blamed_rev = rev.clone();
            let rev_for_git = blamed_rev.clone();
            let result = cx
                .background_spawn(async move {
                    sgg_core::blame_file_at(&root, rev_for_git.as_deref(), &path)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.preview_path.as_deref() != Some(blamed_path.as_str()) {
                    return;
                }
                if this.blame_rev != blamed_rev {
                    return;
                }
                match result {
                    Ok(lines) => {
                        this.blame_lines = lines;
                        this.blame_path = Some(blamed_path);
                        this.blame_rev = blamed_rev;
                        this.blame_cursor_line = Some(
                            this.preview_input.read(cx).cursor_position().line as usize,
                        );
                        cx.notify();
                    }
                    Err(_) => {
                        this.blame_lines.clear();
                        this.blame_path = Some(blamed_path);
                        this.blame_rev = blamed_rev;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn set_branch_search_compact(&mut self, compact: bool, cx: &mut Context<Self>) {
        if self.branch_search_compact == compact {
            return;
        }
        self.branch_search_compact = compact;
        if !compact {
            self.branch_search_open = false;
        }
        cx.notify();
    }

    pub(super) fn toggle_branch_search_open(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.branch_search_open = !self.branch_search_open;
        if self.branch_search_open {
            self.branch_search_input.update(cx, |input, cx| {
                input.focus(window, cx);
            });
        }
        cx.notify();
    }

    pub(super) fn toggle_add_remote_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.remote_form, RemoteForm::Add) {
            self.close_remote_form(window, cx);
            return;
        }
        self.remote_form = RemoteForm::Add;
        self.remote_name_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.remote_url_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        cx.notify();
    }

    pub(super) fn start_edit_remote(
        &mut self,
        name: String,
        url: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.remote_form = RemoteForm::Edit {
            name: name.clone(),
        };
        self.remote_name_input.update(cx, |input, cx| {
            input.set_value(&name, window, cx);
        });
        self.remote_url_input.update(cx, |input, cx| {
            input.set_value(url.as_deref().unwrap_or(""), window, cx);
        });
        cx.notify();
    }

    pub(super) fn close_remote_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.remote_form = RemoteForm::Hidden;
        self.remote_name_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.remote_url_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        cx.notify();
    }

    pub(super) fn toggle_create_branch_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.branch_form, BranchForm::Create) {
            self.close_branch_form(window, cx);
            return;
        }
        self.branch_form = BranchForm::Create;
        let t = self.t();
        self.branch_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(t.branch_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn start_rename_branch(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.branch_form = BranchForm::Rename {
            from: name.clone(),
        };
        let t = self.t();
        self.branch_input.update(cx, |input, cx| {
            input.set_value(&name, window, cx);
            input.set_placeholder(t.rename_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn close_branch_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.branch_form = BranchForm::Hidden;
        let t = self.t();
        self.branch_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(t.branch_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn start_file_rename(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or(path.as_str())
            .to_string();
        self.file_op_form = FileOpForm::Rename { from: path };
        let t = self.t();
        self.file_op_input.update(cx, |input, cx| {
            input.set_value(&name, window, cx);
            input.set_placeholder(t.file_op_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn start_file_move(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.file_op_form = FileOpForm::Move { from: path.clone() };
        let t = self.t();
        self.file_op_input.update(cx, |input, cx| {
            input.set_value(&path, window, cx);
            input.set_placeholder(t.move_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn start_new_file(
        &mut self,
        parent: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !parent.is_empty() {
            self.ensure_parents_expanded(&format!("{parent}/_"));
            self.expanded_dirs.insert(parent.clone());
        }
        self.file_op_form = FileOpForm::NewFile {
            parent: parent.clone(),
        };
        let t = self.t();
        self.file_op_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(t.new_file_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn start_new_folder(
        &mut self,
        parent: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !parent.is_empty() {
            self.ensure_parents_expanded(&format!("{parent}/_"));
            self.expanded_dirs.insert(parent.clone());
        }
        self.file_op_form = FileOpForm::NewFolder {
            parent: parent.clone(),
        };
        let t = self.t();
        self.file_op_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(t.new_folder_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn close_file_op_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.file_op_form = FileOpForm::Hidden;
        let t = self.t();
        self.file_op_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.set_placeholder(t.file_op_placeholder, window, cx);
        });
        cx.notify();
    }

    pub(super) fn apply_theme(&self, cx: &mut App) {
        use gpui_kit::component::{Theme, ThemeMode};

        let mode = match self.theme_pref {
            ThemePreference::Light => ThemeMode::Light,
            ThemePreference::Dark => ThemeMode::Dark,
            ThemePreference::System => ThemeMode::from(cx.window_appearance()),
        };
        Theme::change(mode, None, cx);
        self.apply_ui_font_to_theme(cx);
    }

    pub(super) fn apply_ui_font_to_theme(&self, cx: &mut App) {
        use gpui_kit::component::Theme;

        let family = self.ui_font_family.trim();
        if family.is_empty() {
            return;
        }
        let family = SharedString::from(family.to_string());
        Theme::update(cx, |theme| {
            theme.font_family = family;
        });
    }

    pub(super) fn sync_custom_command_from_input(&mut self, cx: &App) {
        self.preferred_editor.custom_command =
            self.custom_editor_input.read(cx).value().to_string();
    }

    pub(super) fn set_changes_as_tree(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.changes_as_tree = enabled;
        self.rebuild_changes_flat_rows();
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_history_files_as_tree(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.history_files_as_tree = enabled;
        cx.notify();
    }

    pub(super) fn toggle_history_file_dir(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.collapsed_history_dirs.remove(&key) {
            self.collapsed_history_dirs.insert(key);
        }
        cx.notify();
    }

    pub(super) fn set_auto_check_updates(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.auto_check_updates = enabled;
        self.persist_prefs(cx);
        cx.notify();
    }

    /// Fetch GitHub `latest.json`. When `silent`, only prompt if a newer release exists.
    pub(super) fn check_for_updates(
        &mut self,
        silent: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.update_task.is_some()
            || self.busy == Some(BusyKind::CheckUpdate)
            || self.busy == Some(BusyKind::DownloadUpdate)
        {
            return;
        }
        if !silent {
            self.busy = Some(BusyKind::CheckUpdate);
            self.begin_progress(BusyKind::CheckUpdate);
            cx.notify();
        }
        self.update_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { super::update::fetch_latest() })
                .await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    this.update_task = None;
                    if !silent {
                        this.busy = None;
                    }
                    match result {
                        Ok(info) => {
                            let current = super::update::current_version();
                            if super::update::is_newer(&info.version, current) {
                                this.update_available = Some(info.clone());
                                let skipped = !this.skipped_update_version.is_empty()
                                    && this.skipped_update_version == info.version;
                                if !silent || !skipped {
                                    this.prompt_update_available(info, window, cx);
                                }
                            } else {
                                this.update_available = None;
                                if !silent {
                                    let msg = this
                                        .t()
                                        .update_up_to_date
                                        .replace("{version}", current);
                                    this.set_ok_status(msg);
                                }
                            }
                        }
                        Err(err) => {
                            this.update_available = None;
                            if !silent {
                                let msg = this
                                    .t()
                                    .update_check_failed
                                    .replace("{error}", &format!("{err:#}"));
                                this.set_error(msg.into(), cx);
                            }
                        }
                    }
                    if !silent
                        && this
                            .progress_log
                            .last()
                            .is_some_and(|e| e.state == super::progress::ProgressState::Running)
                    {
                        let done = this.status_line.clone();
                        this.finish_progress(this.error.is_none(), done);
                    }
                    cx.notify();
                })
            });
        }));
    }

    fn prompt_update_available(
        &mut self,
        info: super::update::LatestInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = t.update_available;
        let desc = t
            .update_available_desc
            .replace("{latest}", &info.version)
            .replace("{current}", super::update::current_version());
        let ok = t.download_update;
        let cancel = t.later;
        let view = cx.entity();
        let skip_version = info.version.clone();
        super::dialogs::open_update_confirm(
            window,
            cx,
            view,
            title,
            desc,
            ok,
            cancel,
            skip_version,
        );
    }

    pub(super) fn skip_update_version(&mut self, version: String, cx: &mut Context<Self>) {
        self.skipped_update_version = version;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn download_available_update(&mut self, cx: &mut Context<Self>) {
        if self.update_task.is_some() || self.busy == Some(BusyKind::DownloadUpdate) {
            return;
        }
        let Some(info) = self.update_available.clone() else {
            self.set_error(self.t().update_no_asset.into(), cx);
            return;
        };
        if info.asset_for_this_platform().is_none() {
            self.set_error(self.t().update_no_asset.into(), cx);
            return;
        }
        self.busy = Some(BusyKind::DownloadUpdate);
        self.begin_progress(BusyKind::DownloadUpdate);
        cx.notify();
        self.update_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { super::update::download_and_open_latest(&info) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.update_task = None;
                this.busy = None;
                match result {
                    Ok(launch) => {
                        this.skipped_update_version.clear();
                        this.persist_prefs(cx);
                        if launch.should_quit {
                            this.set_ok_status(this.t().update_installing_restart);
                            if this
                                .progress_log
                                .last()
                                .is_some_and(|e| e.state == super::progress::ProgressState::Running)
                            {
                                let done = this.status_line.clone();
                                this.finish_progress(true, done);
                            }
                            cx.quit();
                            return;
                        }
                        let msg = this
                            .t()
                            .update_downloaded
                            .replace("{path}", &launch.path.display().to_string());
                        this.set_ok_status(msg);
                    }
                    Err(err) => {
                        let msg = this
                            .t()
                            .update_check_failed
                            .replace("{error}", &format!("{err:#}"));
                        this.set_error(msg.into(), cx);
                    }
                }
                if this
                    .progress_log
                    .last()
                    .is_some_and(|e| e.state == super::progress::ProgressState::Running)
                {
                    let done = this.status_line.clone();
                    this.finish_progress(this.error.is_none(), done);
                }
                cx.notify();
            });
        }));
    }

    pub(super) fn set_auto_refresh(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.auto_refresh = enabled;
        self.persist_prefs(cx);
        self.sync_auto_refresh(cx);
        cx.notify();
    }

    pub(super) fn sync_auto_refresh(&mut self, cx: &mut Context<Self>) {
        // Dropping the previous task cancels the loop.
        self.refresh_task = None;
        if !self.auto_refresh {
            return;
        }

        let interval = Duration::from_secs(AUTO_REFRESH_SECS);
        self.refresh_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let should = this.read_with(cx, |this, _| this.busy.is_none());
                match should {
                    Ok(true) => {}
                    Ok(false) => continue,
                    Err(_) => break,
                }
                if this
                    .update(cx, |this, cx| {
                        this.reload_auto(cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    fn clear_commit_ai_form_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [
            &self.commit_ai_title_input,
            &self.commit_ai_docs_input,
            &self.commit_ai_base_input,
            &self.commit_ai_path_input,
            &self.commit_ai_model_input,
            &self.commit_ai_key_input,
            &self.commit_ai_auth_header_input,
            &self.commit_ai_auth_query_input,
            &self.commit_ai_key_env_input,
            &self.commit_ai_cli_bin_input,
        ] {
            input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
        }
        self.commit_ai_path_input.update(cx, |input, cx| {
            input.set_value("/chat/completions", window, cx);
        });
        self.commit_ai_auth_header_input.update(cx, |input, cx| {
            input.set_value("x-api-key", window, cx);
        });
        self.commit_ai_auth_query_input.update(cx, |input, cx| {
            input.set_value("key", window, cx);
        });
        self.commit_ai_form_auth = super::ai_provider::AiAuthKind::Bearer;
        self.commit_ai_models.clear();
        self.commit_ai_models_error = None;
    }

    fn fill_commit_ai_form_from_provider(
        &mut self,
        provider: &super::ai_provider::AiProvider,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_ai_title_input.update(cx, |input, cx| {
            input.set_value(&provider.title, window, cx);
        });
        self.commit_ai_docs_input.update(cx, |input, cx| {
            input.set_value(&provider.docs_url, window, cx);
        });
        self.commit_ai_base_input.update(cx, |input, cx| {
            input.set_value(&provider.base_url, window, cx);
        });
        self.commit_ai_path_input.update(cx, |input, cx| {
            input.set_value(&provider.chat_path, window, cx);
        });
        self.commit_ai_model_input.update(cx, |input, cx| {
            input.set_value(&provider.model, window, cx);
        });
        self.commit_ai_key_input.update(cx, |input, cx| {
            input.set_value(&provider.api_key, window, cx);
        });
        self.commit_ai_auth_header_input.update(cx, |input, cx| {
            input.set_value(&provider.auth_header, window, cx);
        });
        self.commit_ai_auth_query_input.update(cx, |input, cx| {
            input.set_value(&provider.auth_query_param, window, cx);
        });
        self.commit_ai_key_env_input.update(cx, |input, cx| {
            input.set_value(&provider.api_key_env, window, cx);
        });
        self.commit_ai_cli_bin_input.update(cx, |input, cx| {
            input.set_value(&provider.cli_bin, window, cx);
        });
        self.commit_ai_form_auth = provider.auth;
        self.commit_ai_models.clear();
        self.commit_ai_models_error = None;
    }

    fn draft_provider_from_form(&self, cx: &App) -> super::ai_provider::AiProvider {
        use super::ai_provider::{AiProvider, AiProviderKind, CommitAiFormMode};
        let mut provider = match &self.commit_ai_form {
            CommitAiFormMode::AddCursor => AiProvider::blank_cursor(),
            CommitAiFormMode::AddCodex => AiProvider::blank_codex(),
            CommitAiFormMode::Edit(id) => self
                .commit_ai
                .provider_by_id(id)
                .cloned()
                .unwrap_or_else(AiProvider::blank_http),
            _ => AiProvider::blank_http(),
        };
        if matches!(self.commit_ai_form, CommitAiFormMode::AddHttp) {
            provider.kind = AiProviderKind::OpenAiCompatible;
            provider.id = AiProvider::new_id();
        }
        if matches!(
            self.commit_ai_form,
            CommitAiFormMode::AddCursor | CommitAiFormMode::AddCodex
        ) {
            provider.id = AiProvider::new_id();
        }
        provider.title = self.commit_ai_title_input.read(cx).value().to_string();
        provider.docs_url = self.commit_ai_docs_input.read(cx).value().to_string();
        provider.base_url = self.commit_ai_base_input.read(cx).value().to_string();
        let chat_path = self.commit_ai_path_input.read(cx).value().to_string();
        provider.chat_path = if chat_path.trim().is_empty() {
            "/chat/completions".into()
        } else {
            chat_path
        };
        provider.model = self.commit_ai_model_input.read(cx).value().to_string();
        provider.api_key = self.commit_ai_key_input.read(cx).value().to_string();
        provider.auth = self.commit_ai_form_auth;
        let auth_header = self.commit_ai_auth_header_input.read(cx).value().to_string();
        provider.auth_header = if auth_header.trim().is_empty() {
            "x-api-key".into()
        } else {
            auth_header
        };
        let auth_query = self.commit_ai_auth_query_input.read(cx).value().to_string();
        provider.auth_query_param = if auth_query.trim().is_empty() {
            "key".into()
        } else {
            auth_query
        };
        provider.api_key_env = self.commit_ai_key_env_input.read(cx).value().to_string();
        provider.cli_bin = self.commit_ai_cli_bin_input.read(cx).value().to_string();
        if provider.title.trim().is_empty() {
            provider.title = match provider.kind {
                AiProviderKind::CursorCli => "Cursor CLI".into(),
                AiProviderKind::CodexCli => "Codex CLI".into(),
                AiProviderKind::OpenAiCompatible => "OpenAI-compatible".into(),
            };
        }
        provider
    }

    pub(super) fn set_commit_ai_style(
        &mut self,
        style: super::prefs::CommitAiStyle,
        cx: &mut Context<Self>,
    ) {
        self.commit_ai.style = style;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_commit_ai_http_mode(
        &mut self,
        mode: super::prefs::CommitAiHttpMode,
        cx: &mut Context<Self>,
    ) {
        self.commit_ai.http_mode = mode;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn sync_commit_ai_timeout_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = self
            .commit_ai
            .timeout_secs
            .map(|s| s.to_string())
            .unwrap_or_default();
        self.commit_ai_timeout_input.update(cx, |input, cx| {
            input.set_value(&value, window, cx);
        });
    }

    pub(super) fn apply_commit_ai_timeout_from_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let raw = self.commit_ai_timeout_input.read(cx).value().to_string();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            self.commit_ai.timeout_secs = None;
        } else {
            match trimmed.parse::<u64>() {
                Ok(0) => self.commit_ai.timeout_secs = None,
                Ok(secs) => self.commit_ai.timeout_secs = Some(secs),
                Err(_) => {
                    self.set_error(self.t().commit_ai_timeout_invalid.into(), cx);
                    self.sync_commit_ai_timeout_input(window, cx);
                    return;
                }
            }
        }
        self.persist_prefs(cx);
        self.set_ok_status(self.t().commit_ai_timeout_saved);
        cx.notify();
    }

    pub(super) fn set_commit_ai_chat_path(
        &mut self,
        path: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = path.into();
        self.commit_ai_path_input.update(cx, |input, cx| {
            input.set_value(&path, window, cx);
        });
        cx.notify();
    }

    pub(super) fn set_commit_ai_active_provider(
        &mut self,
        id: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.commit_ai.active_provider_id = id.into();
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn open_commit_ai_add_http(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_commit_ai_form_inputs(window, cx);
        self.commit_ai_form = super::ai_provider::CommitAiFormMode::AddHttp;
        cx.notify();
    }

    pub(super) fn open_commit_ai_add_cursor(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let template = super::ai_provider::AiProvider::blank_cursor();
        self.fill_commit_ai_form_from_provider(&template, window, cx);
        if let Some(detected) = super::commit_ai::detect_cursor_bin() {
            self.commit_ai_cli_bin_input.update(cx, |input, cx| {
                input.set_value(&detected, window, cx);
            });
        }
        self.commit_ai_form = super::ai_provider::CommitAiFormMode::AddCursor;
        cx.notify();
    }

    pub(super) fn open_commit_ai_add_codex(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let template = super::ai_provider::AiProvider::blank_codex();
        self.fill_commit_ai_form_from_provider(&template, window, cx);
        if let Some(detected) = super::commit_ai::detect_codex_bin() {
            self.commit_ai_cli_bin_input.update(cx, |input, cx| {
                input.set_value(&detected, window, cx);
            });
        }
        self.commit_ai_form = super::ai_provider::CommitAiFormMode::AddCodex;
        cx.notify();
    }

    pub(super) fn open_commit_ai_edit(
        &mut self,
        id: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = id.into();
        let Some(provider) = self.commit_ai.provider_by_id(&id).cloned() else {
            return;
        };
        self.fill_commit_ai_form_from_provider(&provider, window, cx);
        if provider.cli_bin.trim().is_empty() {
            let detected = match provider.kind {
                super::ai_provider::AiProviderKind::CursorCli => {
                    super::commit_ai::detect_cursor_bin()
                }
                super::ai_provider::AiProviderKind::CodexCli => {
                    super::commit_ai::detect_codex_bin()
                }
                _ => None,
            };
            if let Some(path) = detected {
                self.commit_ai_cli_bin_input.update(cx, |input, cx| {
                    input.set_value(&path, window, cx);
                });
            }
        }
        self.commit_ai_form = super::ai_provider::CommitAiFormMode::Edit(id);
        cx.notify();
    }

    pub(super) fn cancel_commit_ai_form(&mut self, cx: &mut Context<Self>) {
        self.commit_ai_form = super::ai_provider::CommitAiFormMode::Closed;
        self.commit_ai_models.clear();
        self.commit_ai_models_error = None;
        self.commit_ai_models_task = None;
        cx.notify();
    }

    pub(super) fn apply_commit_ai_http_preset(
        &mut self,
        preset: super::ai_provider::AiProviderPreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(
            self.commit_ai_form,
            super::ai_provider::CommitAiFormMode::AddHttp
                | super::ai_provider::CommitAiFormMode::Edit(_)
        ) {
            self.commit_ai_form = super::ai_provider::CommitAiFormMode::AddHttp;
        }
        let template = preset.fill_template();
        // Keep API key already typed in the form.
        let existing_key = self.commit_ai_key_input.read(cx).value().to_string();
        self.fill_commit_ai_form_from_provider(&template, window, cx);
        if !existing_key.trim().is_empty() {
            self.commit_ai_key_input.update(cx, |input, cx| {
                input.set_value(&existing_key, window, cx);
            });
        }
        cx.notify();
    }

    pub(super) fn apply_commit_ai_local_preset(
        &mut self,
        preset: super::ai_provider::LocalLlmPreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use super::ai_provider::{AiAuthKind, CommitAiFormMode, LocalLlmPreset};
        if !matches!(
            self.commit_ai_form,
            CommitAiFormMode::AddHttp | CommitAiFormMode::Edit(_)
        ) {
            self.commit_ai_form = CommitAiFormMode::AddHttp;
        }
        let title = match preset {
            LocalLlmPreset::Custom => "Custom local LLM".to_string(),
            other => format!("{} (local)", other.label()),
        };
        self.commit_ai_title_input.update(cx, |input, cx| {
            input.set_value(&title, window, cx);
        });
        self.commit_ai_base_input.update(cx, |input, cx| {
            input.set_value(preset.base_url(), window, cx);
        });
        self.commit_ai_docs_input.update(cx, |input, cx| {
            input.set_value(preset.docs_url(), window, cx);
        });
        self.commit_ai_path_input.update(cx, |input, cx| {
            input.set_value("/chat/completions", window, cx);
        });
        self.commit_ai_form_auth = AiAuthKind::None;
        self.commit_ai_models.clear();
        self.commit_ai_models_error = None;
        cx.notify();
    }

    pub(super) fn set_commit_ai_form_auth(
        &mut self,
        auth: super::ai_provider::AiAuthKind,
        cx: &mut Context<Self>,
    ) {
        self.commit_ai_form_auth = auth;
        cx.notify();
    }

    pub(super) fn set_commit_ai_model(
        &mut self,
        model: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let model = model.into();
        self.commit_ai_model_input.update(cx, |input, cx| {
            input.set_value(&model, window, cx);
        });
        cx.notify();
    }

    pub(super) fn refresh_commit_ai_models(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.commit_ai_models_task.is_some() {
            return;
        }
        let draft = self.draft_provider_from_form(cx);
        self.commit_ai_models_error = None;
        cx.notify();
        self.commit_ai_models_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result =
                cx.background_spawn(async move { super::commit_ai::fetch_models(&draft) })
                    .await;
            let _ = cx.update(|_, cx| {
                this.update(cx, |this, cx| {
                    this.commit_ai_models_task = None;
                    match result {
                        Ok(models) => {
                            this.commit_ai_models = models;
                            this.commit_ai_models_error = None;
                        }
                        Err(err) => {
                            this.commit_ai_models.clear();
                            this.commit_ai_models_error = Some(format!("{err:#}"));
                        }
                    }
                    cx.notify();
                })
            });
        }));
    }

    pub(super) fn detect_commit_ai_cli_bin(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use super::ai_provider::CommitAiFormMode;
        let detected = match &self.commit_ai_form {
            CommitAiFormMode::AddCursor => super::commit_ai::detect_cursor_bin(),
            CommitAiFormMode::AddCodex => super::commit_ai::detect_codex_bin(),
            CommitAiFormMode::Edit(id) => match self.commit_ai.provider_by_id(id).map(|p| p.kind)
            {
                Some(super::ai_provider::AiProviderKind::CursorCli) => {
                    super::commit_ai::detect_cursor_bin()
                }
                Some(super::ai_provider::AiProviderKind::CodexCli) => {
                    super::commit_ai::detect_codex_bin()
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(path) = detected {
            self.commit_ai_cli_bin_input.update(cx, |input, cx| {
                input.set_value(&path, window, cx);
            });
            cx.notify();
        } else {
            self.set_error(self.t().commit_ai_cli_not_found.into(), cx);
        }
    }

    pub(super) fn install_commit_ai_cli_bin(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use super::ai_provider::{AiProviderKind, CommitAiFormMode};
        use super::cli_tools::CliToolKind;

        if self.commit_ai_install_cli_task.is_some()
            || self.busy == Some(BusyKind::InstallCli)
        {
            return;
        }
        let kind = match &self.commit_ai_form {
            CommitAiFormMode::AddCursor => Some(CliToolKind::Cursor),
            CommitAiFormMode::AddCodex => Some(CliToolKind::Codex),
            CommitAiFormMode::Edit(id) => match self.commit_ai.provider_by_id(id).map(|p| p.kind)
            {
                Some(AiProviderKind::CursorCli) => Some(CliToolKind::Cursor),
                Some(AiProviderKind::CodexCli) => Some(CliToolKind::Codex),
                _ => None,
            },
            _ => None,
        };
        let Some(kind) = kind else {
            return;
        };

        self.start_cli_install(
            kind,
            window,
            cx,
            |this, path, window, cx| {
                this.commit_ai_cli_bin_input.update(cx, |input, cx| {
                    input.set_value(&path, window, cx);
                });
            },
        );
    }

    pub(super) fn browse_commit_ai_cli_bin(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(self.t().commit_ai_browse_cli.into()),
        });
        cx.spawn_in(window, async move |this, window| {
            let Ok(result) = paths.await else {
                return;
            };
            let Ok(Some(selected)) = result else {
                return;
            };
            let Some(path) = selected.into_iter().next() else {
                return;
            };
            let _ = window.update(|window, cx| {
                this.update(cx, |this, cx| {
                    this.commit_ai_cli_bin_input.update(cx, |input, cx| {
                        input.set_value(path.to_string_lossy().as_ref(), window, cx);
                    });
                    cx.notify();
                })
            });
        })
        .detach();
    }

    pub(super) fn save_commit_ai_form(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use super::ai_provider::CommitAiFormMode;
        let draft = self.draft_provider_from_form(cx);
        match &self.commit_ai_form {
            CommitAiFormMode::AddHttp | CommitAiFormMode::AddCursor | CommitAiFormMode::AddCodex => {
                self.commit_ai.providers.push(draft);
            }
            CommitAiFormMode::Edit(id) => {
                if let Some(provider) = self.commit_ai.provider_by_id_mut(id) {
                    let enabled = provider.enabled;
                    let keep_id = provider.id.clone();
                    *provider = draft;
                    provider.id = keep_id;
                    provider.enabled = enabled;
                }
            }
            CommitAiFormMode::Closed => return,
        }
        self.commit_ai_form = CommitAiFormMode::Closed;
        self.commit_ai_models.clear();
        self.commit_ai_models_error = None;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_commit_ai_provider_enabled(
        &mut self,
        id: impl AsRef<str>,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let id = id.as_ref();
        if let Some(provider) = self.commit_ai.provider_by_id_mut(id) {
            provider.enabled = enabled;
            self.persist_prefs(cx);
            cx.notify();
        }
    }

    pub(super) fn remove_commit_ai_provider(
        &mut self,
        id: impl AsRef<str>,
        cx: &mut Context<Self>,
    ) {
        let id = id.as_ref();
        if super::ai_provider::is_builtin_cli_id(id) {
            // Built-ins stay registered; disable instead of remove.
            if let Some(p) = self.commit_ai.provider_by_id_mut(id) {
                p.enabled = false;
            }
            self.persist_prefs(cx);
            cx.notify();
            return;
        }
        self.commit_ai.providers.retain(|p| p.id != id);
        if self.commit_ai.active_provider_id == id {
            self.commit_ai.active_provider_id = super::ai_provider::ACTIVE_AUTO.into();
        }
        if self.commit_ai_form.edit_id() == Some(id) {
            self.commit_ai_form = super::ai_provider::CommitAiFormMode::Closed;
        }
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn sync_cursor_api_key_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_ai.ensure_builtin_cli_providers();
        let key = self
            .commit_ai
            .provider_by_id(super::ai_provider::BUILTIN_CURSOR_ID)
            .map(|p| p.api_key.clone())
            .unwrap_or_default();
        self.cursor_api_key_input.update(cx, |input, cx| {
            input.set_value(&key, window, cx);
        });
    }

    pub(super) fn save_cursor_api_key(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_ai.ensure_builtin_cli_providers();
        let key = self
            .cursor_api_key_input
            .read(cx)
            .value()
            .to_string()
            .trim()
            .to_string();
        if let Some(p) = self
            .commit_ai
            .provider_by_id_mut(super::ai_provider::BUILTIN_CURSOR_ID)
        {
            p.api_key = key.clone();
            if p.api_key_env.trim().is_empty() {
                p.api_key_env = "CURSOR_API_KEY".into();
            }
            p.enabled = true;
        }
        // Keep the edit-form key field in sync when that form is open on Cursor.
        self.commit_ai_key_input.update(cx, |input, cx| {
            input.set_value(&key, window, cx);
        });
        self.persist_prefs(cx);
        if key.is_empty() {
            self.set_ok_status(self.t().commit_ai_cursor_api_key_cleared);
        } else {
            self.set_ok_status(self.t().commit_ai_cursor_api_key_saved);
        }
        cx.notify();
    }

    pub(super) fn open_cursor_api_keys_dashboard(&mut self, cx: &mut Context<Self>) {
        cx.open_url(super::t3_bridge::CURSOR_API_KEYS_URL);
    }

    pub(super) fn import_t3_agents(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.t3_available = super::t3_bridge::t3_available();
        if !self.t3_available {
            self.set_error(self.t().commit_ai_t3_not_found.into(), cx);
            cx.notify();
            return;
        }
        let result = super::t3_bridge::import_agents(&mut self.commit_ai);
        if !result.cursor_models.is_empty() {
            self.commit_ai_models = result.cursor_models.clone();
            self.commit_ai_models_error = None;
        }
        if let Some(model) = result.preferred_cursor_model.clone() {
            if model != "default" {
                self.commit_ai_model_input.update(cx, |input, cx| {
                    if input.value().trim().is_empty() {
                        input.set_value(&model, window, cx);
                    }
                });
            }
        }
        // Prefer env key already present under T3; otherwise leave the paste field as-is.
        if result.used_env_cursor_key {
            // Ensure env fallback is wired; do not overwrite a user-pasted key.
            if let Some(p) = self
                .commit_ai
                .provider_by_id_mut(super::ai_provider::BUILTIN_CURSOR_ID)
            {
                if p.api_key_env.trim().is_empty() {
                    p.api_key_env = "CURSOR_API_KEY".into();
                }
            }
        }
        self.sync_cursor_api_key_input(window, cx);
        self.t3_import_status = Some(SharedString::from(result.summary.clone()));
        self.persist_prefs(cx);
        let msg = self
            .t()
            .commit_ai_t3_import_done
            .replace("{summary}", &result.summary);
        self.set_ok_status(msg);
        self.refresh_cli_status(cx);
        cx.notify();
    }

    pub(super) fn login_commit_ai_cli(
        &mut self,
        kind: super::cli_tools::CliToolKind,
        cx: &mut Context<Self>,
    ) {
        match super::cli_tools::open_login(kind) {
            Ok(()) => {
                let msg = self
                    .t()
                    .commit_ai_cli_login_started
                    .replace("{name}", kind.label());
                self.set_ok_status(msg);
                self.refresh_cli_status(cx);
            }
            Err(err) => {
                let msg = self
                    .t()
                    .commit_ai_cli_login_failed
                    .replace("{error}", &format!("{err:#}"));
                self.set_error(msg.into(), cx);
            }
        }
        cx.notify();
    }

    pub(super) fn install_builtin_cli(
        &mut self,
        kind: super::cli_tools::CliToolKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.commit_ai_install_cli_task.is_some()
            || self.busy == Some(BusyKind::InstallCli)
        {
            return;
        }
        self.start_cli_install(
            kind,
            window,
            cx,
            move |this, path, _window, cx| {
                let id = match kind {
                    super::cli_tools::CliToolKind::Cursor => {
                        super::ai_provider::BUILTIN_CURSOR_ID
                    }
                    super::cli_tools::CliToolKind::Codex => super::ai_provider::BUILTIN_CODEX_ID,
                };
                this.commit_ai.ensure_builtin_cli_providers();
                if let Some(p) = this.commit_ai.provider_by_id_mut(id) {
                    p.cli_bin = path.clone();
                    p.enabled = true;
                }
                this.persist_prefs(cx);
            },
        );
    }

    fn start_cli_install(
        &mut self,
        kind: super::cli_tools::CliToolKind,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_ok: impl FnOnce(&mut Self, String, &mut Window, &mut Context<Self>) + 'static,
    ) {
        self.busy = Some(BusyKind::InstallCli);
        self.cli_install_progress = Some(0.0);
        self.begin_progress(BusyKind::InstallCli);
        cx.notify();

        let millipct = Arc::new(AtomicU32::new(0));
        let millipct_bg = millipct.clone();
        let progress_cb: super::cli_tools::InstallProgressFn = Arc::new(move |frac| {
            let v = (frac.clamp(0.0, 1.0) * 1000.0).round() as u32;
            millipct_bg.store(v, Ordering::Relaxed);
        });

        self.commit_ai_install_cli_task = Some(cx.spawn_in(window, async move |this, cx| {
            // Poll install progress into UI ~10×/s while the download runs.
            let millipct_poll = millipct.clone();
            let this_poll = this.clone();
            let poll = cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let v = millipct_poll.load(Ordering::Relaxed) as f32 / 1000.0;
                    let cont = this_poll
                        .update(cx, |this, cx| {
                            if this.busy == Some(BusyKind::InstallCli) {
                                this.cli_install_progress = Some(v);
                                cx.notify();
                                true
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    if !cont {
                        break;
                    }
                }
            });

            let result = cx
                .background_spawn(async move {
                    super::commit_ai::install_cli_bin_with_progress(kind, progress_cb)
                })
                .await;
            // Dropping the poll task cancels the UI heartbeat loop.
            drop(poll);

            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    this.commit_ai_install_cli_task = None;
                    this.busy = None;
                    this.cli_install_progress = None;
                    match result {
                        Ok(path) => {
                            on_ok(this, path.clone(), window, cx);
                            let msg = this
                                .t()
                                .commit_ai_install_cli_done
                                .replace("{path}", &path);
                            this.set_ok_status(msg);
                            this.refresh_cli_status(cx);
                        }
                        Err(err) => {
                            let msg = this
                                .t()
                                .commit_ai_install_cli_failed
                                .replace("{error}", &format!("{err:#}"));
                            this.set_error(msg.into(), cx);
                        }
                    }
                    if this
                        .progress_log
                        .last()
                        .is_some_and(|e| e.state == super::progress::ProgressState::Running)
                    {
                        let done = this.status_line.clone();
                        this.finish_progress(this.error.is_none(), done);
                    }
                    cx.notify();
                })
            });
        }));
    }

    pub(super) fn move_commit_ai_provider(
        &mut self,
        id: impl AsRef<str>,
        delta: isize,
        cx: &mut Context<Self>,
    ) {
        let id = id.as_ref();
        let Some(idx) = self.commit_ai.providers.iter().position(|p| p.id == id) else {
            return;
        };
        let target = idx as isize + delta;
        if target < 0 || target as usize >= self.commit_ai.providers.len() {
            return;
        }
        self.commit_ai.providers.swap(idx, target as usize);
        self.persist_prefs(cx);
        cx.notify();
    }

    /// True while an AI commit-message task is in flight (does not use [`Self::busy`],
    /// so Stage / Unstage stay available).
    pub(super) fn is_generating_commit_message(&self) -> bool {
        self.generate_commit_task.is_some()
    }

    /// Fill the commit textarea from AI (staged-only when staged exists; otherwise
    /// unstaged + untracked). Never commits. Does **not** take [`Self::busy`] so
    /// staging/unstaging can continue while generation runs.
    ///
    /// Re-clicking while a run is active opens a confirm dialog, then aborts.
    pub(super) fn generate_commit_message(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.generate_commit_task.is_some() {
            self.confirm_abort_generate_commit_message(window, cx);
            return;
        }
        self.start_generate_commit_message(window, cx);
    }

    pub(super) fn confirm_abort_generate_commit_message(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = t.confirm_abort_generate_commit;
        let description = t.desc_abort_generate_commit;
        let ok = t.abort_generate_commit;
        let cancel = t.cancel;
        let view = cx.entity();
        super::dialogs::open_confirm(
            window,
            cx,
            title,
            description,
            ok,
            cancel,
            true,
            move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.abort_generate_commit_message(cx);
                });
                true
            },
        );
    }

    pub(super) fn abort_generate_commit_message(&mut self, cx: &mut Context<Self>) {
        if let Some(flag) = self.generate_commit_abort.take() {
            flag.store(true, Ordering::SeqCst);
        }
        self.generate_commit_task = None;
        if self
            .progress_log
            .last()
            .is_some_and(|e| e.state == super::progress::ProgressState::Running)
        {
            self.finish_progress(false, self.t().generate_commit_aborted.into());
        } else {
            self.set_ok_status(self.t().generate_commit_aborted);
        }
        cx.notify();
    }

    fn start_generate_commit_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.generate_commit_task.is_some() {
            return;
        }
        if !self.has_repo() {
            self.set_error(self.t().no_folder_open.into(), cx);
            return;
        }
        if self.commit_ai.providers.is_empty() {
            self.set_error(self.t().commit_ai_no_providers.into(), cx);
            return;
        }
        let root = self.repo_root();
        let hint = {
            let current = self.commit_input.read(cx).value().to_string();
            let trimmed = current.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        };
        let prefs = self.commit_ai.clone();
        let abort = Arc::new(AtomicBool::new(false));
        self.generate_commit_abort = Some(abort.clone());
        // Progress only — leave `busy` free for Stage / Unstage / Commit.
        self.begin_progress(BusyKind::GenerateCommitMessage);
        cx.notify();

        enum GenEvent {
            Partial(String),
            Done(Result<super::commit_ai::GeneratedCommitMessage, String>),
        }

        let (tx, rx) = mpsc::channel::<GenEvent>();
        let abort_bg = abort.clone();
        self.generate_commit_task = Some(cx.spawn_in(window, async move |this, cx| {
            let worker = cx.background_spawn(async move {
                let tx_partial = tx.clone();
                let result = super::commit_ai::generate_for_repo(
                    &root,
                    hint.as_deref(),
                    &prefs,
                    Some(&abort_bg),
                    |partial| {
                        let _ = tx_partial.send(GenEvent::Partial(partial.to_string()));
                    },
                );
                let _ = tx.send(GenEvent::Done(result.map_err(|e| format!("{e:#}"))));
            });

            loop {
                match rx.try_recv() {
                    Ok(GenEvent::Partial(text)) => {
                        let _ = cx.update(|window, cx| {
                            this.update(cx, |this, cx| {
                                if this.generate_commit_abort.as_ref().is_some_and(|a| {
                                    a.load(Ordering::Relaxed)
                                }) {
                                    return;
                                }
                                this.commit_input.update(cx, |input, cx| {
                                    input.set_value(&text, window, cx);
                                });
                                cx.notify();
                            })
                        });
                    }
                    Ok(GenEvent::Done(result)) => {
                        let _ = worker.await;
                        let _ = cx.update(|window, cx| {
                            this.update(cx, |this, cx| {
                                this.generate_commit_task = None;
                                this.generate_commit_abort = None;
                                match result {
                                    Ok(msg) => {
                                        let text = msg.combined();
                                        this.commit_input.update(cx, |input, cx| {
                                            input.set_value(&text, window, cx);
                                        });
                                        this.set_ok_status(this.t().generate_commit_done);
                                    }
                                    Err(err) if err.contains("aborted") => {
                                        this.set_ok_status(this.t().generate_commit_aborted);
                                    }
                                    Err(err) => {
                                        let msg = this
                                            .t()
                                            .generate_commit_failed
                                            .replace("{error}", &err);
                                        this.set_error(msg.into(), cx);
                                    }
                                }
                                if this.progress_log.last().is_some_and(|e| {
                                    e.state == super::progress::ProgressState::Running
                                }) {
                                    let done = this.status_line.clone();
                                    this.finish_progress(this.error.is_none(), done);
                                }
                                cx.notify();
                            })
                        });
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(40))
                            .await;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        let _ = worker.await;
                        let _ = cx.update(|_, cx| {
                            this.update(cx, |this, cx| {
                                this.generate_commit_task = None;
                                this.generate_commit_abort = None;
                                if this.progress_log.last().is_some_and(|e| {
                                    e.state == super::progress::ProgressState::Running
                                }) {
                                    this.finish_progress(
                                        false,
                                        this.t().generate_commit_aborted.into(),
                                    );
                                }
                                cx.notify();
                            })
                        });
                        break;
                    }
                }
            }
        }));
    }

    pub(super) fn persist_prefs(&mut self, cx: &mut Context<Self>) {
        self.sync_custom_command_from_input(cx);
        let prefs = AppPrefs {
            theme: self.theme_pref,
            locale: self.locale,
            editor: self.preferred_editor.clone(),
            change_sort: self.change_sort.normalized(),
            group_by_status: self.group_by_status,
            changes_as_tree: self.changes_as_tree,
            auto_refresh: self.auto_refresh,
            auto_check_updates: self.auto_check_updates,
            skipped_update_version: self.skipped_update_version.clone(),
            show_remote_branches: self.show_remote_branches,
            ask_fetch_on_open: self.ask_fetch_on_open,
            fetch_prompt_muted: self.fetch_prompt_muted.clone(),
            recent_repos: self.recent_repos.clone(),
            status_colors: self.status_colors.clone(),
            terminal_font_family: self.terminal_font_family.clone(),
            terminal_font_size: self.terminal_font_size,
            terminal_line_height: self.terminal_line_height,
            terminal_shell: {
                self.sync_terminal_shell_from_input(cx);
                self.terminal_shell.clone()
            },
            show_inline_blame: self.show_inline_blame,
            show_diff_line_background: self.show_diff_line_background,
            ui_font_family: self.ui_font_family.clone(),
            commit_ai: self.commit_ai.clone(),
        };
        if let Err(err) = prefs.save() {
            self.error = Some(format!("Failed to save settings: {err:#}").into());
        }
    }

    pub(super) fn effective_status_color(&self, status: &str, cx: &App) -> Hsla {
        super::render::status_style::status_color(status, &self.status_colors, cx)
    }

    fn wire_status_color_pickers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_status_color_pickers(window, cx);
        let mut subs = Vec::new();
        for key in StatusColors::KEYS {
            let Some(picker) = self.status_color_pickers.get(key).cloned() else {
                continue;
            };
            let status = key.to_string();
            subs.push(cx.subscribe(
                &picker,
                move |this, _picker, event: &ColorPickerEvent, cx| {
                    if let ColorPickerEvent::Change(Some(color)) = event {
                        this.status_colors
                            .set(&status, Some(hsla_to_hex(*color)));
                        this.persist_prefs(cx);
                        this.refresh_preview_diff_gutter(cx);
                        cx.notify();
                    }
                },
            ));
        }
        self._status_color_subs = subs;
    }

    pub(super) fn sync_status_color_pickers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for key in StatusColors::KEYS {
            let color = self.effective_status_color(key, cx);
            if let Some(picker) = self.status_color_pickers.get(key).cloned() {
                picker.update(cx, |state, cx| {
                    state.set_value(color, window, cx);
                });
            }
        }
    }

    pub(super) fn reset_status_colors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.status_colors.clear();
        self.sync_status_color_pickers(window, cx);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn reset_status_color(
        &mut self,
        status: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status_colors.set(status, None);
        let color = theme_status_color(status, cx);
        if let Some(picker) = self.status_color_pickers.get(status).cloned() {
            picker.update(cx, |state, cx| {
                state.set_value(color, window, cx);
            });
        }
        self.persist_prefs(cx);
        cx.notify();
    }

    /// Progressive density for the Changes pane (widest → narrowest):
    /// hover-only row actions → single column. Section header icon-only
    /// mode is measured per section via [`set_section_hide_labels`].
    pub(super) fn set_changes_density(
        &mut self,
        compact: bool,
        split: bool,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;
        if self.changes_list_compact != compact {
            self.changes_list_compact = compact;
            changed = true;
        }
        if self.changes_split_view != split {
            self.changes_split_view = split;
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    pub(super) fn set_section_hide_labels(
        &mut self,
        title: &str,
        hide: bool,
        cx: &mut Context<Self>,
    ) {
        let prev = self
            .changes_section_hide_labels
            .get(title)
            .copied()
            .unwrap_or(false);
        if prev == hide {
            return;
        }
        self.changes_section_hide_labels
            .insert(title.to_string(), hide);
        cx.notify();
    }

    pub(super) fn set_remote_actions_compact(&mut self, compact: bool, cx: &mut Context<Self>) {
        if self.remote_actions_compact != compact {
            self.remote_actions_compact = compact;
            cx.notify();
        }
    }

    pub(super) fn set_files_tree_compact(&mut self, compact: bool, cx: &mut Context<Self>) {
        if self.files_tree_compact != compact {
            self.files_tree_compact = compact;
            cx.notify();
        }
    }

    pub(super) fn set_ask_fetch_on_open(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.ask_fetch_on_open = enabled;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn unmute_fetch_prompt(&mut self, root: String, cx: &mut Context<Self>) {
        self.fetch_prompt_muted.retain(|p| p != &root);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_fetch_dont_ask(&mut self, checked: bool, cx: &mut Context<Self>) {
        self.fetch_dont_ask = checked;
        cx.notify();
    }

    pub(super) fn toggle_dir(&mut self, rel_path: String, cx: &mut Context<Self>) {
        if !self.expanded_dirs.remove(&rel_path) {
            self.expanded_dirs.insert(rel_path);
        }
        self.rebuild_files_tree();
        cx.notify();
    }

    pub(super) fn toggle_change_dir(&mut self, key: String, cx: &mut Context<Self>) {
        if !self.collapsed_change_dirs.remove(&key) {
            self.collapsed_change_dirs.insert(key);
        }
        self.rebuild_changes_flat_rows();
        cx.notify();
    }

    pub(super) fn toggle_change_section(&mut self, title: String, cx: &mut Context<Self>) {
        if !self.collapsed_change_sections.remove(&title) {
            self.collapsed_change_sections.insert(title);
        }
        cx.notify();
    }

    pub(super) fn set_ok_status(&mut self, message: impl Into<SharedString>) {
        self.status_line = message.into();
        self.error = None;
        self.finish_progress(true, self.status_line.clone());
    }

    pub(super) fn set_error(&mut self, err: SharedString, cx: &mut Context<Self>) {
        self.error = Some(err.clone());
        self.finish_progress(false, err);
        cx.notify();
    }

    pub(super) fn clear_error(&mut self, cx: &mut Context<Self>) {
        if self.error.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn copy_error(&mut self, cx: &mut Context<Self>) {
        let Some(err) = self.error.clone() else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(err.to_string()));
        self.set_ok_status(self.t().copy_error_done);
        cx.notify();
    }

    pub(super) fn copy_progress_entry(
        &mut self,
        title: &str,
        detail: &str,
        cx: &mut Context<Self>,
    ) {
        let text = if detail.trim().is_empty() {
            title.to_string()
        } else {
            format!("{title}\n{detail}")
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.set_ok_status(self.t().copy_progress_done);
        cx.notify();
    }

    pub(super) fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.has_repo().then(|| self.repo_root());
        self.sync_terminal_shell_from_input(cx);
        let shell = self.terminal_shell.clone();
        match self.terminal.toggle(cwd.as_deref(), &shell) {
            Ok(()) => {
                if self.terminal.visible {
                    self.resize_terminal_to_window(window);
                    self.focus_terminal(window, cx);
                } else {
                    self.terminal_focused = false;
                }
                cx.notify();
            }
            Err(err) => self.set_error(format!("Terminal: {err:#}").into(), cx),
        }
    }

    pub(super) fn new_terminal_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.has_repo().then(|| self.repo_root());
        self.sync_terminal_shell_from_input(cx);
        let shell = self.terminal_shell.clone();
        match self.terminal.spawn_session(cwd.as_deref(), &shell) {
            Ok(()) => {
                self.resize_terminal_to_window(window);
                self.focus_terminal(window, cx);
                cx.notify();
            }
            Err(err) => self.set_error(format!("Terminal: {err:#}").into(), cx),
        }
    }

    pub(super) fn sync_terminal_shell_from_input(&mut self, cx: &App) {
        self.terminal_shell.custom_command =
            self.terminal_shell_input.read(cx).value().to_string();
    }

    pub(super) fn set_terminal_shell_kind(
        &mut self,
        kind: TerminalShellKind,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !kind.is_available() {
            return;
        }
        self.terminal_shell.kind = kind;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn apply_terminal_shell_custom(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_terminal_shell_from_input(cx);
        self.terminal_shell.kind = TerminalShellKind::Custom;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn terminal_line_px(&self) -> f32 {
        super::terminal::terminal_line_px(self.terminal_font_size, self.terminal_line_height)
    }

    pub(super) fn resize_terminal_to_window(&mut self, window: &Window) {
        let width: f32 = window.bounds().size.width.into();
        let _ = self.terminal.maybe_resize_from_px(
            width,
            self.terminal.height(),
            self.terminal_font_size,
            self.terminal_line_height,
        );
    }

    /// Sync PTY cols/rows from the measured terminal dock bounds (window/pane resize).
    pub(super) fn sync_terminal_grid_from_bounds(
        &mut self,
        width_px: f32,
        height_px: f32,
        cx: &mut Context<Self>,
    ) {
        if !self.terminal.visible {
            return;
        }
        if self.terminal.maybe_resize_from_px(
            width_px,
            height_px,
            self.terminal_font_size,
            self.terminal_line_height,
        ) {
            self.terminal.refresh_lines();
            cx.notify();
        }
    }

    pub(super) fn set_terminal_height(
        &mut self,
        height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prev = self.terminal.height();
        self.terminal.set_height(height);
        if (self.terminal.height() - prev).abs() < 0.5 {
            return;
        }
        self.resize_terminal_to_window(window);
        self.terminal.refresh_lines();
        cx.notify();
    }

    pub(super) fn set_terminal_font_family(
        &mut self,
        family: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut family = family.into();
        if family.trim().is_empty() {
            family = DEFAULT_TERMINAL_FONT_FAMILY.to_string();
        }
        self.terminal_font_family = family;
        self.resize_terminal_to_window(window);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn apply_terminal_font_from_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let family = self.terminal_font_input.read(cx).value().to_string();
        self.set_terminal_font_family(family, window, cx);
    }

    pub(super) fn set_terminal_font_size(
        &mut self,
        size: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal_font_size = size.clamp(9.0, 32.0);
        self.resize_terminal_to_window(window);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn bump_terminal_font_size(
        &mut self,
        delta: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_terminal_font_size(self.terminal_font_size + delta, window, cx);
    }

    pub(super) fn set_terminal_line_height(
        &mut self,
        line_height: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal_line_height = line_height.clamp(1.0, 3.0);
        self.resize_terminal_to_window(window);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn reset_terminal_font(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal_font_size = DEFAULT_TERMINAL_FONT_SIZE;
        self.terminal_line_height = DEFAULT_TERMINAL_LINE_HEIGHT;
        self.terminal_font_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.set_terminal_font_family(DEFAULT_TERMINAL_FONT_FAMILY, window, cx);
    }

    pub(super) fn focus_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_focused = true;
        self.terminal_caret_on = true;
        self.terminal_caret_ticks = 0;
        window.focus(&self.terminal_focus, cx);
        cx.notify();
    }

    pub(super) fn close_terminal_tab(
        &mut self,
        idx: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.terminal.close_at(idx);
        if self.terminal.visible {
            self.focus_terminal(window, cx);
        } else {
            self.terminal_focused = false;
        }
        cx.notify();
    }

    pub(super) fn close_active_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.close_active();
        if self.terminal.visible {
            self.focus_terminal(window, cx);
        } else {
            self.terminal_focused = false;
        }
        cx.notify();
    }

    pub(super) fn handle_terminal_key(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.terminal.visible {
            return;
        }
        // Ensure focus flag stays true while the pane is receiving keys.
        self.terminal_focused = true;
        if let Some(bytes) = super::terminal::key_event_to_bytes(event) {
            self.terminal.write_input(&bytes);
            // Restart blink so the caret stays visible while typing.
            self.terminal_caret_on = true;
            self.terminal_caret_ticks = 0;
            cx.stop_propagation();
        }
    }

    fn start_terminal_poll(&mut self, cx: &mut Context<Self>) {
        self.terminal_poll_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let cont = this
                    .update(cx, |this, cx| {
                        let mut dirty_ui = false;
                        if this.terminal.visible && this.terminal.take_dirty() {
                            this.terminal.refresh_lines();
                            dirty_ui = true;
                        }
                        // ~530ms blink while the terminal pane is focused.
                        if this.terminal.visible && this.terminal_focused {
                            this.terminal_caret_ticks = this.terminal_caret_ticks.wrapping_add(1);
                            if this.terminal_caret_ticks >= 16 {
                                this.terminal_caret_ticks = 0;
                                this.terminal_caret_on = !this.terminal_caret_on;
                                dirty_ui = true;
                            }
                        }
                        if dirty_ui {
                            cx.notify();
                        }
                        true
                    })
                    .unwrap_or(false);
                if !cont {
                    break;
                }
            }
        }));
    }

    pub(super) fn toggle_progress_collapsed(&mut self, cx: &mut Context<Self>) {
        self.progress_collapsed = !self.progress_collapsed;
        cx.notify();
    }

    pub(super) fn begin_progress(&mut self, kind: BusyKind) {
        let t = self.t();
        // Git CLI stdout/stderr are captured in-process (no external cmd window)
        // and written into this Progress dock when the op finishes.
        self.progress_log.push(ProgressEntry {
            title: kind.progress_label(t).into(),
            detail: t.progress_running.into(),
            state: ProgressState::Running,
        });
        while self.progress_log.len() > PROGRESS_LOG_LIMIT {
            self.progress_log.remove(0);
        }
        // Show the dock while work is in flight.
        self.progress_collapsed = false;
    }

    pub(super) fn finish_progress(&mut self, ok: bool, detail: SharedString) {
        if let Some(entry) = self
            .progress_log
            .iter_mut()
            .rev()
            .find(|e| e.state == ProgressState::Running)
        {
            entry.state = if ok {
                ProgressState::Done
            } else {
                ProgressState::Failed
            };
            entry.detail = detail;
        }
    }
}