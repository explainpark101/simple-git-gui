use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_kit::component::input::{EditorState, InputState, TextareaState};
use gpui_kit::*;

use sgg_core::{RepoSnapshot, RepoState};

use super::busy::BusyKind;
use super::editor::{EditorPreset, ExternalEditor};
use super::files::{list_tree, FsNode};
use super::i18n::{Locale, UiText};
use super::page::{AppPage, NavEntry};
use super::perf::snapshot_fingerprint;
use super::prefs::{AppPrefs, StatusColors, ThemePreference, absolute_path, display_path};
use super::progress::{
    ProgressEntry, ProgressState, PROGRESS_DEFAULT_HEIGHT, PROGRESS_LOG_LIMIT,
};
use super::render::status_style::{hsla_to_hex, theme_status_color, TreeStatusIndex};
use super::sort::{group_and_sort, ChangeSection, ChangeSort};

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
pub(super) const AUTO_REFRESH_SECS: u64 = 3;

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
    pub(super) start_path: PathBuf,
    pub(super) snapshot: Option<RepoSnapshot>,
    /// Last applied snapshot fingerprint (skip notify when auto-refresh is a no-op).
    snapshot_fp: u64,
    pub(super) selected: HashSet<String>,
    pub(super) selected_remote: String,
    pub(super) status_line: SharedString,
    pub(super) commit_input: Entity<TextareaState>,
    pub(super) branch_input: Entity<InputState>,
    pub(super) file_op_input: Entity<InputState>,
    pub(super) remote_name_input: Entity<InputState>,
    pub(super) remote_url_input: Entity<InputState>,
    pub(super) custom_editor_input: Entity<InputState>,
    pub(super) preview_input: Entity<EditorState>,
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
    pub(super) preferred_editor: ExternalEditor,
    pub(super) theme_pref: ThemePreference,
    pub(super) locale: Locale,
    pub(super) changes_as_tree: bool,
    pub(super) auto_refresh: bool,
    pub(super) ask_fetch_on_open: bool,
    pub(super) fetch_prompt_muted: Vec<String>,
    pub(super) status_colors: StatusColors,
    pub(super) status_color_pickers: StatusColorPickers,
    _status_color_subs: Vec<Subscription>,
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
    /// Narrow Changes list: letter status + action buttons only on row hover.
    pub(super) changes_list_compact: bool,
    /// Mid-width Changes: section header buttons show icons only (no labels).
    pub(super) changes_hide_labels: bool,
    /// Wide Changes pane: Unstaged/Untracked left, Staged right.
    pub(super) changes_split_view: bool,
    /// Narrow sidebar remote header: Fetch/Pull/Push show icons only.
    pub(super) remote_actions_compact: bool,
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
    pub fn new(start_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = AppPrefs::load();
        let start_path = absolute_path(start_path);
        let t = prefs.locale.text();
        let focus_handle = cx.focus_handle();

        let commit_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder(t.commit_placeholder)
        });
        let branch_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.branch_placeholder));
        let file_op_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.file_op_placeholder));
        let remote_name_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.remote_name_placeholder));
        let remote_url_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.remote_url_placeholder));
        let custom_editor_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(r#"e.g. "C:\path\to\editor.exe" --wait"#)
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

        let mut view = Self {
            start_path,
            snapshot: None,
            snapshot_fp: 0,
            selected: HashSet::new(),
            selected_remote: "origin".into(),
            status_line: t.loading.into(),
            commit_input,
            branch_input,
            file_op_input,
            remote_name_input,
            remote_url_input,
            custom_editor_input,
            preview_input,
            preview_path: None,
            preview_tabs: Vec::new(),
            preview_tab_index: 0,
            error: None,
            page: AppPage::Git,
            change_sort: prefs.change_sort.normalized(),
            group_by_status: prefs.group_by_status,
            changes_sections: Vec::new(),
            preferred_editor: prefs.editor,
            theme_pref: prefs.theme,
            locale: prefs.locale,
            changes_as_tree: prefs.changes_as_tree,
            auto_refresh: prefs.auto_refresh,
            ask_fetch_on_open: prefs.ask_fetch_on_open,
            fetch_prompt_muted: prefs.fetch_prompt_muted,
            status_colors,
            status_color_pickers,
            _status_color_subs: Vec::new(),
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
            changes_list_compact: false,
            changes_hide_labels: false,
            changes_split_view: false,
            remote_actions_compact: false,
            files_tree_compact: false,
            fetch_dont_ask: false,
            fetch_prompted_root: None,
            focus_handle: focus_handle.clone(),
            refresh_task: None,
        };
        view.apply_theme(cx);
        view.wire_status_color_pickers(window, cx);
        view.reload(cx);
        view.sync_auto_refresh(cx);
        // Root (dialogs/sheets) is wrapped around this view after construction.
        // Defer the fetch prompt until the window host is ready.
        cx.defer_in(window, |this, window, cx| {
            this.maybe_prompt_fetch(window, cx);
        });
        window.focus(&focus_handle, cx);
        view
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
            .unwrap_or_else(|| self.start_path.clone())
    }

    pub(super) fn repo_root_display(&self) -> String {
        display_path(&self.repo_root())
    }

    pub(super) fn with_repo<F, T>(&self, f: F) -> Result<T, SharedString>
    where
        F: FnOnce(&RepoState) -> Result<T, sgg_core::RepoError>,
    {
        let start = self.repo_root();
        let repo = RepoState::open_nearest(&start).map_err(|e| SharedString::from(e.to_string()))?;
        f(&repo).map_err(|e| SharedString::from(e.to_string()))
    }

    pub(super) fn reload(&mut self, cx: &mut Context<Self>) {
        self.reload_inner(true, cx);
    }

    pub(super) fn reload_auto(&mut self, cx: &mut Context<Self>) {
        self.reload_inner(false, cx);
    }

    fn reload_inner(&mut self, clear_selection: bool, cx: &mut Context<Self>) {
        match RepoState::open_nearest(&self.start_path) {
            Ok(repo) => match repo.snapshot() {
                Ok(snap) => {
                    let root = absolute_path(snap.root.clone());
                    let branch = snap
                        .head_branch
                        .clone()
                        .unwrap_or_else(|| "(detached)".into());
                    let status = SharedString::from(format!(
                        "{}  ·  {}  ·  {} changes",
                        display_path(&root),
                        branch,
                        snap.changes.len()
                    ));
                    let mut snap = snap;
                    snap.root = root;
                    let fp = snapshot_fingerprint(&snap);
                    // Auto-refresh: skip full UI rebuild when nothing meaningful changed.
                    if !clear_selection && self.error.is_none() && self.snapshot_fp == fp {
                        return;
                    }
                    self.sync_selected_remote(&snap);
                    if clear_selection {
                        self.selected.clear();
                    } else {
                        let paths: HashSet<_> =
                            snap.changes.iter().map(|c| c.path.as_str()).collect();
                        self.selected.retain(|p| paths.contains(p.as_str()));
                    }
                    self.snapshot_fp = fp;
                    self.snapshot = Some(snap);
                    self.status_line = status;
                    self.error = None;
                    self.rebuild_changes_sections();
                    self.rebuild_files_status_index();
                    self.rebuild_files_tree();
                }
                Err(err) => {
                    self.error = Some(err.to_string().into());
                    self.status_line = "Failed to read repository".into();
                    self.snapshot_fp = 0;
                    self.changes_sections.clear();
                    self.files_status_index = TreeStatusIndex::default();
                }
            },
            Err(err) => {
                self.snapshot = None;
                self.snapshot_fp = 0;
                self.files_tree_nodes.clear();
                self.changes_sections.clear();
                self.files_status_index = TreeStatusIndex::default();
                self.error = Some(err.to_string().into());
                self.status_line =
                    format!("No git repo near {}", display_path(&self.start_path)).into();
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
            ask_fetch_on_open: self.ask_fetch_on_open,
            fetch_prompt_muted: self.fetch_prompt_muted.clone(),
            status_colors: self.status_colors.clone(),
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
    /// labels → icon-only headers → hover-only row actions → single column.
    pub(super) fn set_changes_density(
        &mut self,
        hide_labels: bool,
        compact: bool,
        split: bool,
        cx: &mut Context<Self>,
    ) {
        let mut changed = false;
        if self.changes_hide_labels != hide_labels {
            self.changes_hide_labels = hide_labels;
            changed = true;
        }
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

    pub(super) fn toggle_progress_collapsed(&mut self, cx: &mut Context<Self>) {
        self.progress_collapsed = !self.progress_collapsed;
        cx.notify();
    }

    pub(super) fn begin_progress(&mut self, kind: BusyKind) {
        let t = self.t();
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