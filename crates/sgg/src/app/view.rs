use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{EditorState, InputEvent, InputState, TextareaState};
use gpui_kit::*;

use sgg_core::{RepoSnapshot, RepoState};

use super::busy::BusyKind;
use super::editor::{EditorPreset, ExternalEditor};
use super::files::{list_tree, FsNode};
use super::i18n::{Locale, UiText};
use super::page::{AppPage, NavEntry};
use super::prefs::{
    absolute_path, display_path, remember_repo, AppPrefs, StatusColors, ThemePreference,
    DEFAULT_TERMINAL_FONT_FAMILY, DEFAULT_TERMINAL_FONT_SIZE, DEFAULT_TERMINAL_LINE_HEIGHT,
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
    pub(super) preview_input: Entity<EditorState>,
    pub(super) terminal_font_family: String,
    pub(super) terminal_font_size: f32,
    pub(super) terminal_line_height: f32,
    pub(super) terminal_shell: TerminalShellPrefs,
    pub(super) terminal_shell_input: Entity<InputState>,
    /// Blinking caret phase for the focused terminal.
    pub(super) terminal_caret_on: bool,
    terminal_caret_ticks: u32,
    pub(super) preview_path: Option<String>,
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
    reload_task: Option<Task<()>>,
    pub(super) preferred_editor: ExternalEditor,
    pub(super) theme_pref: ThemePreference,
    pub(super) locale: Locale,
    pub(super) changes_as_tree: bool,
    pub(super) auto_refresh: bool,
    pub(super) show_remote_branches: bool,
    pub(super) ask_fetch_on_open: bool,
    pub(super) fetch_prompt_muted: Vec<String>,
    /// Absolute repo roots, most recent first (persisted, unbounded).
    pub(super) recent_repos: Vec<String>,
    pub(super) status_colors: StatusColors,
    pub(super) status_color_pickers: StatusColorPickers,
    _status_color_subs: Vec<Subscription>,
    _branch_search_sub: Option<Subscription>,
    _commit_input_sub: Option<Subscription>,
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
            InputState::new(window, cx).placeholder(DEFAULT_TERMINAL_FONT_FAMILY)
        });
        let terminal_shell_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(r#"e.g. "C:\Program Files\Git\bin\bash.exe" --login"#)
        });
        // Code editor: line numbers + Ctrl/Cmd-F. soft_wrap(false) keeps the
        // gpui-base display_map on buffer lines only (visible-range paint), which
        // avoids wrap-map thrash on large files. Syntax stays plaintext — tree-sitter
        // is not enabled in this crate's gpui-kit features.
        let preview_input = cx.new(|cx| {
            EditorState::new(window, cx)
                .placeholder(t.preview_placeholder)
                .language("plaintext")
                .soft_wrap(false)
        });

        custom_editor_input.update(cx, |input, cx| {
            input.set_value(&prefs.editor.custom_command, window, cx);
        });
        terminal_font_input.update(cx, |input, cx| {
            input.set_value(&prefs.terminal_font_family, window, cx);
        });
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
            preview_input,
            terminal_font_family: prefs.terminal_font_family.clone(),
            terminal_font_size: prefs.terminal_font_size,
            terminal_line_height: prefs.terminal_line_height,
            terminal_shell: prefs.terminal_shell.clone(),
            terminal_shell_input,
            terminal_caret_on: true,
            terminal_caret_ticks: 0,
            preview_path: None,
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
            show_remote_branches: prefs.show_remote_branches,
            ask_fetch_on_open: prefs.ask_fetch_on_open,
            fetch_prompt_muted: prefs.fetch_prompt_muted,
            recent_repos: prefs.recent_repos,
            status_colors,
            status_color_pickers,
            _status_color_subs: Vec::new(),
            _branch_search_sub: None,
            _commit_input_sub: None,
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
        view.wire_commit_input(window, cx);
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
            preview: self.preview_path.clone(),
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
        self.page = entry.page;
        match entry.preview {
            Some(path) => {
                self.ensure_parents_expanded(&path);
                self.load_preview_into(&path, window, cx);
            }
            None => {
                self.preview_path = None;
                if self.page != AppPage::Files {
                    // Leave Files tabs alone when history clears a Git-only preview.
                } else {
                    self.preview_tabs.clear();
                    self.preview_tab_index = 0;
                }
            }
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

    pub(super) fn select_remote(&mut self, name: String, cx: &mut Context<Self>) {
        self.selected_remote = name;
        cx.notify();
    }

    pub(super) fn set_page(&mut self, page: AppPage, cx: &mut Context<Self>) {
        if page == self.page {
            return;
        }
        self.page = page;
        // Auto-refresh may skip the files tree while on Git; rebuild when opening Files.
        if page == AppPage::Files && self.has_repo() {
            self.rebuild_files_tree();
        }
        self.push_nav_entry(self.current_nav_entry());
        cx.notify();
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
        if self.page == AppPage::Files && !self.preview_tabs.is_empty() {
            self.close_preview_tab(self.preview_tab_index, window, cx);
            return;
        }
        if self.preview_path.is_none() {
            return;
        }
        self.preview_path = None;
        self.push_nav_entry(self.current_nav_entry());
        cx.notify();
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
        self.terminal.maybe_resize_from_px(
            width,
            self.terminal_font_size,
            self.terminal_line_height,
        );
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
        self.terminal_font_family = family.clone();
        self.terminal_font_input.update(cx, |input, cx| {
            input.set_value(&family, window, cx);
        });
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