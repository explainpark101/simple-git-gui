use std::path::PathBuf;

use gpui_kit::component::input::{InputState, TextareaState};
use gpui_kit::*;

use sgg_core::{RepoSnapshot, RepoState};

use super::editor::{EditorPreset, ExternalEditor};
use super::i18n::{Locale, UiText};
use super::page::AppPage;
use super::prefs::{AppPrefs, ThemePreference, absolute_path, display_path};
use super::sort::ChangeSort;

pub struct AppView {
    pub(super) start_path: PathBuf,
    pub(super) snapshot: Option<RepoSnapshot>,
    pub(super) selected: Vec<String>,
    pub(super) selected_remote: String,
    pub(super) status_line: SharedString,
    pub(super) commit_input: Entity<TextareaState>,
    pub(super) branch_input: Entity<InputState>,
    pub(super) custom_editor_input: Entity<InputState>,
    pub(super) preview_input: Entity<TextareaState>,
    pub(super) preview_path: Option<String>,
    pub(super) error: Option<SharedString>,
    pub(super) page: AppPage,
    pub(super) change_sort: ChangeSort,
    pub(super) preferred_editor: ExternalEditor,
    pub(super) theme_pref: ThemePreference,
    pub(super) locale: Locale,
    pub(super) editor_select_open: bool,
    pub(super) expanded_dirs: Vec<String>,
}

impl AppView {
    pub fn new(start_path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let prefs = AppPrefs::load();
        let start_path = absolute_path(start_path);
        let t = prefs.locale.text();

        let commit_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .rows(4)
                .placeholder(t.commit_placeholder)
        });
        let branch_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t.branch_placeholder));
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
            custom_editor_input,
            preview_input,
            preview_path: None,
            error: None,
            page: AppPage::Git,
            change_sort: prefs.change_sort,
            preferred_editor: prefs.editor,
            theme_pref: prefs.theme,
            locale: prefs.locale,
            editor_select_open: false,
            expanded_dirs: Vec::new(),
        };
        view.apply_theme(cx);
        view.reload(cx);
        view
    }

    pub(super) fn t(&self) -> UiText {
        self.locale.text()
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
                    self.snapshot = Some(snap);
                    self.error = None;
                    self.selected.clear();
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
        self.page = page;
        cx.notify();
    }

    pub(super) fn set_change_sort(&mut self, sort: ChangeSort, cx: &mut Context<Self>) {
        self.change_sort = sort;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn set_editor_preset(&mut self, preset: EditorPreset, cx: &mut Context<Self>) {
        self.preferred_editor.preset = preset;
        self.editor_select_open = false;
        self.sync_custom_command_from_input(cx);
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn toggle_editor_select(&mut self, cx: &mut Context<Self>) {
        self.editor_select_open = !self.editor_select_open;
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
        self.branch_input.update(cx, |input, cx| {
            input.set_placeholder(t.branch_placeholder, window, cx);
        });
        self.preview_input.update(cx, |input, cx| {
            input.set_placeholder(t.preview_placeholder, window, cx);
        });
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

    pub(super) fn persist_prefs(&mut self, cx: &mut Context<Self>) {
        self.sync_custom_command_from_input(cx);
        let prefs = AppPrefs {
            theme: self.theme_pref,
            locale: self.locale,
            editor: self.preferred_editor.clone(),
            change_sort: self.change_sort,
        };
        if let Err(err) = prefs.save() {
            self.error = Some(format!("Failed to save settings: {err:#}").into());
        }
    }

    pub(super) fn toggle_dir(&mut self, rel_path: String, cx: &mut Context<Self>) {
        if let Some(idx) = self.expanded_dirs.iter().position(|p| p == &rel_path) {
            self.expanded_dirs.remove(idx);
        } else {
            self.expanded_dirs.push(rel_path);
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