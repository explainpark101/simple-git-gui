use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::*;

use sgg_core::{RepoSnapshot, RepoState};

use super::busy::BusyKind;
use super::editor::{EditorPreset, ExternalEditor};
use super::i18n::{Locale, UiText};
use super::page::{AppPage, NavEntry};
use super::prefs::{AppPrefs, ThemePreference, absolute_path, display_path};
use super::sort::ChangeSort;

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
}

pub struct AppView {
    pub(super) start_path: PathBuf,
    pub(super) snapshot: Option<RepoSnapshot>,
    pub(super) selected: Vec<String>,
    pub(super) selected_remote: String,
    pub(super) status_line: SharedString,
    pub(super) commit_input: Entity<TextareaState>,
    pub(super) branch_input: Entity<InputState>,
    pub(super) file_op_input: Entity<InputState>,
    pub(super) custom_editor_input: Entity<InputState>,
    pub(super) preview_input: Entity<TextareaState>,
    pub(super) preview_path: Option<String>,
    pub(super) error: Option<SharedString>,
    pub(super) page: AppPage,
    pub(super) change_sort: ChangeSort,
    pub(super) group_by_status: bool,
    pub(super) preferred_editor: ExternalEditor,
    pub(super) theme_pref: ThemePreference,
    pub(super) locale: Locale,
    pub(super) changes_as_tree: bool,
    pub(super) auto_refresh: bool,
    pub(super) ask_fetch_on_open: bool,
    pub(super) fetch_prompt_muted: Vec<String>,
    pub(super) expanded_dirs: Vec<String>,
    pub(super) expanded_change_dirs: Vec<String>,
    pub(super) nav_history: Vec<NavEntry>,
    pub(super) history_index: usize,
    /// Suppresses history pushes while applying back/forward.
    navigating_history: bool,
    pub(super) branch_form: BranchForm,
    pub(super) file_op_form: FileOpForm,
    pub(super) sidebar_collapsed: bool,
    pub(super) busy: Option<BusyKind>,
    /// When the stage action strip is too narrow, collapse actions into a dropdown.
    pub(super) stage_bar_compact: bool,
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
        let custom_editor_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(r#"e.g. "C:\path\to\editor.exe" --wait"#)
        });
        let preview_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(20)
                .placeholder(t.preview_placeholder)
        });

        custom_editor_input.update(cx, |input, cx| {
            input.set_value(&prefs.editor.custom_command, window, cx);
        });

        let mut view = Self {
            start_path,
            snapshot: None,
            selected: Vec::new(),
            selected_remote: "origin".into(),
            status_line: t.loading.into(),
            commit_input,
            branch_input,
            file_op_input,
            custom_editor_input,
            preview_input,
            preview_path: None,
            error: None,
            page: AppPage::Git,
            change_sort: prefs.change_sort.normalized(),
            group_by_status: prefs.group_by_status,
            preferred_editor: prefs.editor,
            theme_pref: prefs.theme,
            locale: prefs.locale,
            changes_as_tree: prefs.changes_as_tree,
            auto_refresh: prefs.auto_refresh,
            ask_fetch_on_open: prefs.ask_fetch_on_open,
            fetch_prompt_muted: prefs.fetch_prompt_muted,
            expanded_dirs: Vec::new(),
            expanded_change_dirs: Vec::new(),
            nav_history: vec![NavEntry::page_only(AppPage::Git)],
            history_index: 0,
            navigating_history: false,
            branch_form: BranchForm::Hidden,
            file_op_form: FileOpForm::Hidden,
            sidebar_collapsed: false,
            busy: None,
            stage_bar_compact: false,
            fetch_dont_ask: false,
            fetch_prompted_root: None,
            focus_handle: focus_handle.clone(),
            refresh_task: None,
        };
        view.apply_theme(cx);
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
                    self.status_line = format!(
                        "{}  ·  {}  ·  {} changes",
                        display_path(&root),
                        branch,
                        snap.changes.len()
                    )
                    .into();
                    let mut snap = snap;
                    snap.root = root;
                    self.sync_selected_remote(&snap);
                    if clear_selection {
                        self.selected.clear();
                    } else {
                        let paths: std::collections::HashSet<_> =
                            snap.changes.iter().map(|c| c.path.as_str()).collect();
                        self.selected.retain(|p| paths.contains(p.as_str()));
                    }
                    self.snapshot = Some(snap);
                    self.error = None;
                }
                Err(err) => {
                    self.error = Some(err.to_string().into());
                    self.status_line = "Failed to read repository".into();
                }
            },
            Err(err) => {
                self.snapshot = None;
                self.error = Some(err.to_string().into());
                self.status_line =
                    format!("No git repo near {}", display_path(&self.start_path)).into();
            }
        }
        cx.notify();
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

    pub(super) fn toggle_select(&mut self, path: String, cx: &mut Context<Self>) {
        if let Some(idx) = self.selected.iter().position(|p| p == &path) {
            self.selected.remove(idx);
        } else {
            self.selected.push(path);
        }
        cx.notify();
    }

    pub(super) fn set_selected(&mut self, path: String, selected: bool, cx: &mut Context<Self>) {
        if selected {
            if !self.selected.iter().any(|p| p == &path) {
                self.selected.push(path);
            }
        } else {
            self.selected.retain(|p| p != &path);
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

    pub(super) fn close_preview(&mut self, cx: &mut Context<Self>) {
        if self.preview_path.is_none() {
            return;
        }
        self.preview_path = None;
        self.push_nav_entry(self.current_nav_entry());
        cx.notify();
    }

    pub(super) fn set_group_by_status(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.group_by_status = enabled;
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
        cx: &mut Context<Self>,
    ) {
        self.theme_pref = pref;
        self.apply_theme(cx);
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
        };
        if let Err(err) = prefs.save() {
            self.error = Some(format!("Failed to save settings: {err:#}").into());
        }
    }

    pub(super) fn set_stage_bar_compact(&mut self, compact: bool, cx: &mut Context<Self>) {
        if self.stage_bar_compact != compact {
            self.stage_bar_compact = compact;
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
        if let Some(idx) = self.expanded_dirs.iter().position(|p| p == &rel_path) {
            self.expanded_dirs.remove(idx);
        } else {
            self.expanded_dirs.push(rel_path);
        }
        cx.notify();
    }

    pub(super) fn toggle_change_dir(&mut self, key: String, cx: &mut Context<Self>) {
        if let Some(idx) = self.expanded_change_dirs.iter().position(|p| p == &key) {
            self.expanded_change_dirs.remove(idx);
        } else {
            self.expanded_change_dirs.push(key);
        }
        cx.notify();
    }

    pub(super) fn set_ok_status(&mut self, message: impl Into<SharedString>) {
        self.status_line = message.into();
        self.error = None;
    }

    pub(super) fn set_error(&mut self, err: SharedString, cx: &mut Context<Self>) {
        self.error = Some(err);
        cx.notify();
    }
}