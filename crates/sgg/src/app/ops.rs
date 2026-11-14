use std::fs;

use gpui_kit::component::WindowExt as _;
use gpui_kit::*;

use super::i18n::UiText;
use super::page::AppPage;
use super::prefs::absolute_path;
use super::view::AppView;

#[derive(Clone, Copy)]
pub(super) enum RemoteAction {
    Fetch,
    Pull,
    Push,
}

impl RemoteAction {
    fn title(self, t: UiText) -> &'static str {
        match self {
            Self::Fetch => t.confirm_fetch,
            Self::Pull => t.confirm_pull,
            Self::Push => t.confirm_push,
        }
    }

    fn ok_text(self, t: UiText) -> &'static str {
        match self {
            Self::Fetch => t.fetch,
            Self::Pull => t.pull,
            Self::Push => t.push,
        }
    }

    fn description(self, remote: &str, t: UiText) -> String {
        let template = match self {
            Self::Fetch => t.desc_fetch,
            Self::Pull => t.desc_pull,
            Self::Push => t.desc_push,
        };
        template.replace("{remote}", remote)
    }
}

impl AppView {
    pub(super) fn stage_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected.clone();
        if paths.is_empty() {
            self.set_error(self.t().select_files_first.into(), cx);
            return;
        }
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        match self.with_repo(|repo| repo.stage_paths(&refs)) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn unstage_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected.clone();
        if paths.is_empty() {
            self.set_error(self.t().select_files_first.into(), cx);
            return;
        }
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        match self.with_repo(|repo| repo.unstage_paths(&refs)) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn stage_all(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.stage_all()) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn unstage_all(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.unstage_all()) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.commit_input.read(cx).value().to_string();
        match self.with_repo(|repo| repo.commit(&message)) {
            Ok(oid) => {
                self.commit_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
                self.set_ok_status(format!("Committed {}", &oid[..8.min(oid.len())]));
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn confirm_remote_action(
        &mut self,
        action: RemoteAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let remote = self.selected_remote.clone();
        let t = self.t();
        let title = action.title(t);
        let ok = action.ok_text(t);
        let description = action.description(&remote, t);
        let view = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            alert
                .title(title)
                .description(description.clone())
                .confirm()
                .ok_text(ok)
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    view.update(cx, |this, cx| match action {
                        RemoteAction::Fetch => this.fetch(cx),
                        RemoteAction::Pull => this.pull(cx),
                        RemoteAction::Push => this.push(cx),
                    });
                    true
                })
        });
    }

    pub(super) fn push(&mut self, cx: &mut Context<Self>) {
        let remote = self.selected_remote.clone();
        match self.with_repo(|repo| repo.push(Some(&remote))) {
            Ok(out) => {
                let msg = if out.is_empty() {
                    format!("Push completed → {remote}")
                } else {
                    out
                };
                self.set_ok_status(msg);
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn pull(&mut self, cx: &mut Context<Self>) {
        let remote = self.selected_remote.clone();
        match self.with_repo(|repo| repo.pull(Some(&remote))) {
            Ok(out) => {
                let msg = if out.is_empty() {
                    format!("Pull completed ← {remote}")
                } else {
                    out
                };
                self.set_ok_status(msg);
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn fetch(&mut self, cx: &mut Context<Self>) {
        let remote = self.selected_remote.clone();
        match self.with_repo(|repo| repo.fetch(Some(&remote))) {
            Ok(out) => {
                let msg = if out.is_empty() {
                    format!("Fetch completed ← {remote}")
                } else {
                    out
                };
                self.set_ok_status(msg);
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn create_branch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.branch_input.read(cx).value().to_string();
        match self.with_repo(|repo| {
            repo.create_branch(&name)?;
            repo.switch_branch(name.trim())?;
            Ok(())
        }) {
            Ok(()) => {
                self.branch_input.update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn switch_branch(&mut self, name: String, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.switch_branch(&name)) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn open_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open folder".into()),
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

            let _ = window.update(|_, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.start_path = absolute_path(path);
                    this.reload(cx);
                });
            });
        })
        .detach();
    }

    pub(super) fn confirm_revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected.clone();
        if paths.is_empty() {
            self.set_error(self.t().select_files_first.into(), cx);
            return;
        }
        self.confirm_revert_paths(paths, window, cx);
    }

    pub(super) fn confirm_revert_one(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_revert_paths(vec![path], window, cx);
    }

    fn confirm_revert_paths(
        &mut self,
        paths: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preview = paths.join(", ");
        let t = self.t();
        let title = t.confirm_revert;
        let ok = t.revert;
        let description = t.desc_revert.replace("{paths}", &preview);
        let view = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let view = view.clone();
            let paths = paths.clone();
            alert
                .title(title)
                .description(description.clone())
                .confirm()
                .ok_text(ok)
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    view.update(cx, |this, cx| this.revert_paths(&paths, cx));
                    true
                })
        });
    }

    pub(super) fn revert_paths(&mut self, paths: &[String], cx: &mut Context<Self>) {
        let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
        match self.with_repo(|repo| repo.revert_paths(&refs)) {
            Ok(()) => {
                self.error = None;
                if let Some(preview) = &self.preview_path {
                    if paths.iter().any(|p| p == preview) {
                        let path = preview.clone();
                        self.load_preview(&path, cx);
                    }
                }
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn open_project_in_editor(&mut self, cx: &mut Context<Self>) {
        self.sync_custom_command_from_input(cx);
        let root = self.repo_root();
        let editor = self.preferred_editor.clone();
        match editor.open_project(&root) {
            Ok(()) => self.set_ok_status(format!("Opened project in {}", editor.label())),
            Err(err) => self.set_error(err.to_string().into(), cx),
        }
        cx.notify();
    }

    pub(super) fn open_path_in_editor(&mut self, rel_path: &str, cx: &mut Context<Self>) {
        self.sync_custom_command_from_input(cx);
        let root = self.repo_root();
        let editor = self.preferred_editor.clone();
        match editor.open_file(&root, rel_path) {
            Ok(()) => self.set_ok_status(format!("Opened {rel_path} in {}", editor.label())),
            Err(err) => self.set_error(err.to_string().into(), cx),
        }
        cx.notify();
    }

    pub(super) fn preview_file(
        &mut self,
        rel_path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.page = AppPage::Files;
        self.ensure_parents_expanded(&rel_path);
        self.load_preview_into(&rel_path, window, cx);
        self.set_ok_status(format!("Preview {rel_path}"));
        cx.notify();
    }

    fn ensure_parents_expanded(&mut self, rel_path: &str) {
        let parts: Vec<&str> = rel_path.split('/').collect();
        if parts.len() <= 1 {
            return;
        }
        let mut prefix = String::new();
        for part in &parts[..parts.len() - 1] {
            if prefix.is_empty() {
                prefix = (*part).to_string();
            } else {
                prefix = format!("{prefix}/{part}");
            }
            if !self.expanded_dirs.iter().any(|p| p == &prefix) {
                self.expanded_dirs.push(prefix.clone());
            }
        }
    }

    pub(super) fn load_preview(&mut self, rel_path: &str, cx: &mut Context<Self>) {
        let full = self.repo_root().join(rel_path);
        match fs::read(&full) {
            Ok(bytes) => {
                if String::from_utf8(bytes).is_err() {
                    self.set_error(
                        format!("{rel_path} is binary and cannot be previewed").into(),
                        cx,
                    );
                    return;
                }
                self.preview_path = Some(rel_path.to_string());
            }
            Err(err) => {
                self.preview_path = Some(rel_path.to_string());
                self.set_error(format!("Failed to read {rel_path}: {err}").into(), cx);
            }
        }
    }

    pub(super) fn load_preview_into(
        &mut self,
        rel_path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let full = self.repo_root().join(rel_path);
        match fs::read(&full) {
            Ok(bytes) => {
                let content = match String::from_utf8(bytes) {
                    Ok(text) => text,
                    Err(_) => {
                        self.set_error(
                            format!("{rel_path} is binary and cannot be previewed").into(),
                            cx,
                        );
                        return;
                    }
                };
                self.preview_input.update(cx, |input, cx| {
                    input.set_value(&content, window, cx);
                });
                self.preview_path = Some(rel_path.to_string());
                self.error = None;
            }
            Err(err) => {
                self.set_error(format!("Failed to read {rel_path}: {err}").into(), cx);
            }
        }
    }

    pub(super) fn save_preview(&mut self, cx: &mut Context<Self>) {
        let Some(rel) = self.preview_path.clone() else {
            self.set_error("No file open in preview".into(), cx);
            return;
        };
        let content = self.preview_input.read(cx).value().to_string();
        let full = self.repo_root().join(&rel);
        match fs::write(&full, content) {
            Ok(()) => {
                self.set_ok_status(format!("Saved {rel}"));
                self.reload(cx);
            }
            Err(err) => self.set_error(format!("Failed to save {rel}: {err}").into(), cx),
        }
    }

    pub(super) fn save_custom_editor_command(&mut self, cx: &mut Context<Self>) {
        self.persist_prefs(cx);
        self.set_ok_status(self.t().settings_saved);
        cx.notify();
    }
}