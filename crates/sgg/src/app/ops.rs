use std::fs;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::{DialogFooter, DialogHeader, DialogTitle};
use gpui_kit::*;

use super::busy::BusyKind;
use super::dialogs::{open_busy_confirm, open_confirm, BusyConfirmOk, FetchPromptSkip};
use super::i18n::UiText;
use super::prefs::absolute_path;
use super::remote_url::remote_browse_url;
use super::view::{AppView, RemoteForm};

/// Soft cap so the editor rope / display_map stay responsive. Larger files
/// should be opened externally (the Editor already paints only the visible
/// line range; soft_wrap is off on the preview state).
const PREVIEW_MAX_BYTES: usize = 1_048_576;
const PREVIEW_MAX_LINES: usize = 40_000;

fn read_preview_text(rel_path: &str, full: &std::path::Path) -> Result<String, String> {
    let meta = fs::metadata(full).map_err(|err| format!("Failed to read {rel_path}: {err}"))?;
    if meta.len() as usize > PREVIEW_MAX_BYTES {
        return Err(format!(
            "{rel_path} is too large to preview ({} KiB). Open externally.",
            meta.len() / 1024
        ));
    }
    let bytes = fs::read(full).map_err(|err| format!("Failed to read {rel_path}: {err}"))?;
    if bytes.len() > PREVIEW_MAX_BYTES {
        return Err(format!(
            "{rel_path} is too large to preview ({} KiB). Open externally.",
            bytes.len() / 1024
        ));
    }
    let text = String::from_utf8(bytes).map_err(|_| {
        format!("{rel_path} is binary and cannot be previewed")
    })?;
    let line_count = text.bytes().filter(|&b| b == b'\n').count().saturating_add(1);
    if line_count > PREVIEW_MAX_LINES {
        return Err(format!(
            "{rel_path} has too many lines ({line_count}). Open externally."
        ));
    }
    Ok(text)
}

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

    fn ok_icon(self) -> gpui_kit::assets::IconName {
        match self {
            Self::Fetch => gpui_kit::assets::IconName::CloudDownload,
            Self::Pull => gpui_kit::assets::IconName::ArrowDownToLine,
            Self::Push => gpui_kit::assets::IconName::CloudUpload,
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

    fn busy_kind(self) -> BusyKind {
        match self {
            Self::Fetch => BusyKind::Fetch,
            Self::Pull => BusyKind::Pull,
            Self::Push => BusyKind::Push,
        }
    }
}

impl AppView {
    /// Run work after painting a loading spinner on matching action buttons.
    pub(super) fn run_busy(
        &mut self,
        kind: BusyKind,
        window: &mut Window,
        cx: &mut Context<Self>,
        work: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if self.busy.is_some() {
            return;
        }
        self.busy = Some(kind);
        self.begin_progress(kind);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(30))
                .await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    work(this, window, cx);
                    this.busy = None;
                    // If work never called set_ok_status / set_error, close the Running line.
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
        })
        .detach();
    }

    pub(super) fn maybe_prompt_fetch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ask_fetch_on_open || self.snapshot.is_none() {
            return;
        }
        let root = self.repo_root();
        if self.fetch_prompted_root.as_ref() == Some(&root) {
            return;
        }
        let root_key = crate::app::prefs::display_path(&root);
        if self.fetch_prompt_muted.iter().any(|p| p == &root_key) {
            return;
        }
        self.fetch_prompted_root = Some(root.clone());
        self.fetch_dont_ask = false;
        let t = self.t();
        let title = t.confirm_fetch_on_open;
        let desc = t.desc_fetch_on_open.replace("{path}", &root_key);
        let fetch_label = t.fetch;
        let skip_label = t.skip_fetch;
        let dont_ask_label = t.dont_ask_again;
        let view = cx.entity();

        let fetch_ok = BusyConfirmOk::mount(
            cx,
            view.clone(),
            BusyKind::Fetch,
            fetch_label.into(),
            gpui_kit::assets::IconName::CloudDownload,
            Rc::new(|this, _window, cx| {
                this.apply_fetch_prompt_prefs(cx);
                this.fetch_now(cx);
            }),
        );
        let fetch_skip = FetchPromptSkip::mount(cx, view.clone(), skip_label.into());

        window.open_dialog(cx, move |dialog, _, cx| {
            let view_check = view.clone();
            let checked = view.read(cx).fetch_dont_ask;
            dialog
                .child(
                    DialogHeader::new().child(DialogTitle::new().child(title)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_4()
                        .child(
                            div()
                                .text_sm()
                                .text_left()
                                .child(desc.clone()),
                        )
                        .child(
                            Checkbox::new("fetch-dont-ask")
                                .label(dont_ask_label)
                                .checked(checked)
                                .on_click(move |checked, _, cx| {
                                    view_check.update(cx, |this, cx| {
                                        this.set_fetch_dont_ask(*checked, cx);
                                    });
                                }),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .child(fetch_skip.clone())
                        .child(fetch_ok.clone()),
                )
        });
    }

    pub(super) fn apply_fetch_prompt_prefs(&mut self, cx: &mut Context<Self>) {
        if self.fetch_dont_ask {
            self.mute_current_fetch_prompt(cx);
        }
        self.fetch_dont_ask = false;
        cx.notify();
    }

    pub(super) fn stage_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths: Vec<String> = self.selected.iter().cloned().collect();
        if paths.is_empty() {
            self.set_error(self.t().select_files_first.into(), cx);
            return;
        }
        self.run_busy(BusyKind::Stage, window, cx, move |this, _, cx| {
            let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
            match this.with_repo(|repo| repo.stage_paths(&refs)) {
                Ok(()) => {
                    this.error = None;
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
    }

    pub(super) fn unstage_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths: Vec<String> = self.selected.iter().cloned().collect();
        if paths.is_empty() {
            self.set_error(self.t().select_files_first.into(), cx);
            return;
        }
        self.run_busy(BusyKind::Unstage, window, cx, move |this, _, cx| {
            let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
            match this.with_repo(|repo| repo.unstage_paths(&refs)) {
                Ok(()) => {
                    this.error = None;
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
    }

    pub(super) fn stage_one(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.run_busy(BusyKind::Stage, window, cx, move |this, _, cx| {
            match this.with_repo(|repo| repo.stage_paths(&[path.as_str()])) {
                Ok(()) => {
                    this.error = None;
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
    }

    pub(super) fn unstage_one(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.run_busy(BusyKind::Unstage, window, cx, move |this, _, cx| {
            match this.with_repo(|repo| repo.unstage_paths(&[path.as_str()])) {
                Ok(()) => {
                    this.error = None;
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
    }

    pub(super) fn stage_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.run_busy(BusyKind::StageAll, window, cx, |this, _, cx| {
            match this.with_repo(|repo| repo.stage_all()) {
                Ok(()) => {
                    this.error = None;
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
    }

    /// Stage every untracked path currently listed in the snapshot.
    pub(super) fn stage_all_untracked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.paths_with_kind(sgg_core::ChangeKind::Untracked);
        if paths.is_empty() {
            self.set_error(self.t().select_files_first.into(), cx);
            return;
        }
        self.run_busy(BusyKind::StageAll, window, cx, move |this, _, cx| {
            let refs: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
            match this.with_repo(|repo| repo.stage_paths(&refs)) {
                Ok(()) => {
                    this.error = None;
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
    }

    fn paths_with_kind(&self, kind: sgg_core::ChangeKind) -> Vec<String> {
        self.snapshot
            .as_ref()
            .map(|s| {
                s.changes
                    .iter()
                    .filter(|c| c.kind == kind)
                    .map(|c| c.path.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn unstage_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.run_busy(BusyKind::UnstageAll, window, cx, |this, _, cx| {
            match this.with_repo(|repo| repo.unstage_all()) {
                Ok(()) => {
                    this.error = None;
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
    }

    pub(super) fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let message = self.commit_input.read(cx).value().to_string();
        self.run_busy(BusyKind::Commit, window, cx, move |this, window, cx| {
            match this.with_repo(|repo| repo.commit(&message)) {
                Ok(oid) => {
                    this.commit_input.update(cx, |input, cx| {
                        input.set_value("", window, cx);
                    });
                    this.set_ok_status(format!("Committed {}", &oid[..8.min(oid.len())]));
                    this.reload(cx);
                }
                Err(err) => this.set_error(err, cx),
            }
        });
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
        let cancel = t.branch_cancel;
        let description = action.description(&remote, t);
        let view = cx.entity();
        let kind = action.busy_kind();
        open_busy_confirm(
            window,
            cx,
            view,
            kind,
            title,
            description,
            ok,
            cancel,
            action.ok_icon(),
            move |this, _, cx| match action {
                RemoteAction::Fetch => this.fetch_now(cx),
                RemoteAction::Pull => this.pull_now(cx),
                RemoteAction::Push => this.push_now(cx),
            },
        );
    }

    pub(super) fn push_now(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn pull_now(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn fetch_now(&mut self, cx: &mut Context<Self>) {
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

    pub(super) fn confirm_switch_branch(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = t.confirm_switch_branch;
        let ok = t.branch_confirm;
        let cancel = t.branch_cancel;
        let description = t.desc_switch_branch.replace("{name}", &name);
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            title,
            description,
            ok,
            cancel,
            false,
            move |_, window, cx| {
                let name = name.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::SwitchBranch, window, cx, move |this, _, cx| {
                        this.switch_branch_now(name, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn confirm_branch_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = self.branch_input.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.set_error("branch name is empty".into(), cx);
            return;
        }
        match self.branch_form.clone() {
            super::view::BranchForm::Create => {
                let t = self.t();
                let title = t.confirm_create_branch;
                let ok = t.create_switch;
                let cancel = t.branch_cancel;
                let description = t.desc_create_branch.replace("{name}", &name);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let name = name.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(BusyKind::CreateBranch, window, cx, move |this, window, cx| {
                                this.create_branch_now(&name, window, cx);
                            });
                        });
                        true
                    },
                );
            }
            super::view::BranchForm::Rename { from } => {
                let t = self.t();
                let title = t.confirm_rename_branch;
                let ok = t.branch_confirm;
                let cancel = t.branch_cancel;
                let description = t
                    .desc_rename_branch
                    .replace("{old}", &from)
                    .replace("{new}", &name);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let from = from.clone();
                        let name = name.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(BusyKind::RenameBranch, window, cx, move |this, window, cx| {
                                this.rename_branch_now(&from, &name, window, cx);
                            });
                        });
                        true
                    },
                );
            }
            super::view::BranchForm::Hidden => {}
        }
    }

    pub(super) fn confirm_remote_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = self.remote_name_input.read(cx).value().trim().to_string();
        let url = self.remote_url_input.read(cx).value().trim().to_string();
        if name.is_empty() || url.is_empty() {
            self.set_error(self.t().remote_name_url_required.into(), cx);
            return;
        }
        match self.remote_form.clone() {
            RemoteForm::Add => {
                let t = self.t();
                let title = t.confirm_add_remote;
                let ok = t.branch_confirm;
                let cancel = t.branch_cancel;
                let description = t
                    .desc_add_remote
                    .replace("{name}", &name)
                    .replace("{url}", &url);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let name = name.clone();
                        let url = url.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(BusyKind::AddRemote, window, cx, move |this, window, cx| {
                                match this.with_repo(|repo| repo.add_remote(&name, &url)) {
                                    Ok(()) => {
                                        this.selected_remote = name.clone();
                                        this.close_remote_form(window, cx);
                                        this.set_ok_status(format!("Added remote {name}"));
                                        this.reload(cx);
                                    }
                                    Err(err) => this.set_error(err, cx),
                                }
                            });
                        });
                        true
                    },
                );
            }
            RemoteForm::Edit { name: old } => {
                let t = self.t();
                let title = t.confirm_edit_remote;
                let ok = t.branch_confirm;
                let cancel = t.branch_cancel;
                let description = t
                    .desc_edit_remote
                    .replace("{old}", &old)
                    .replace("{name}", &name)
                    .replace("{url}", &url);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let old = old.clone();
                        let name = name.clone();
                        let url = url.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(BusyKind::EditRemote, window, cx, move |this, window, cx| {
                                let result = this.with_repo(|repo| {
                                    repo.rename_remote(&old, &name)?;
                                    repo.set_remote_url(&name, &url)?;
                                    Ok(())
                                });
                                match result {
                                    Ok(()) => {
                                        if this.selected_remote == old {
                                            this.selected_remote = name.clone();
                                        }
                                        this.close_remote_form(window, cx);
                                        this.set_ok_status(format!("Updated remote {name}"));
                                        this.reload(cx);
                                    }
                                    Err(err) => this.set_error(err, cx),
                                }
                            });
                        });
                        true
                    },
                );
            }
            RemoteForm::Hidden => {}
        }
    }

    pub(super) fn confirm_delete_remote(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = t.confirm_delete_remote;
        let ok = t.branch_confirm;
        let cancel = t.branch_cancel;
        let description = t.desc_delete_remote.replace("{name}", &name);
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            title,
            description,
            ok,
            cancel,
            true,
            move |_, window, cx| {
                let name = name.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::DeleteRemote, window, cx, move |this, _, cx| {
                        match this.with_repo(|repo| repo.remove_remote(&name)) {
                            Ok(()) => {
                                if this.selected_remote == name {
                                    this.selected_remote = "origin".into();
                                }
                                this.set_ok_status(format!("Removed remote {name}"));
                                this.reload(cx);
                            }
                            Err(err) => this.set_error(err, cx),
                        }
                    });
                });
                true
            },
        );
    }

    pub(super) fn open_remote_in_browser(&mut self, url: &str, cx: &mut Context<Self>) {
        match remote_browse_url(url) {
            Some(browse) => {
                cx.open_url(&browse);
                self.set_ok_status(format!("Opened {browse}"));
                cx.notify();
            }
            None => {
                self.set_error(
                    format!("Could not convert remote URL to a browser link: {url}").into(),
                    cx,
                );
            }
        }
    }

    pub(super) fn confirm_delete_branch(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = t.confirm_delete_branch;
        let ok = t.branch_confirm;
        let cancel = t.branch_cancel;
        let description = t.desc_delete_branch.replace("{name}", &name);
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            title,
            description,
            ok,
            cancel,
            true,
            move |_, window, cx| {
                let name = name.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::DeleteBranch, window, cx, move |this, _, cx| {
                        this.delete_branch_now(&name, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn create_branch_now(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.with_repo(|repo| {
            repo.create_branch(name)?;
            repo.switch_branch(name.trim())?;
            Ok(())
        }) {
            Ok(()) => {
                self.close_branch_form(window, cx);
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn switch_branch_now(&mut self, name: String, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.switch_branch(&name)) {
            Ok(()) => {
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn rename_branch_now(
        &mut self,
        old: &str,
        new: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.with_repo(|repo| repo.rename_branch(old, new)) {
            Ok(()) => {
                self.close_branch_form(window, cx);
                self.error = None;
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn delete_branch_now(&mut self, name: &str, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.delete_branch(name)) {
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

        self.busy = Some(BusyKind::OpenFolder);
        cx.notify();

        cx.spawn_in(window, async move |this, window| {
            let Ok(result) = paths.await else {
                let _ = window.update(|_, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.busy = None;
                        cx.notify();
                    });
                });
                return;
            };
            let Ok(Some(selected)) = result else {
                let _ = window.update(|_, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.busy = None;
                        cx.notify();
                    });
                });
                return;
            };
            let Some(path) = selected.into_iter().next() else {
                let _ = window.update(|_, cx| {
                    let _ = this.update(cx, |this, cx| {
                        this.busy = None;
                        cx.notify();
                    });
                });
                return;
            };

            let _ = window.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.busy = None;
                    this.fetch_prompted_root = None;
                    this.start_path = absolute_path(path);
                    this.reload(cx);
                    this.maybe_prompt_fetch(window, cx);
                });
            });
        })
        .detach();
    }

    pub(super) fn confirm_revert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths: Vec<String> = self.selected.iter().cloned().collect();
        if paths.is_empty() {
            self.set_error(self.t().select_files_first.into(), cx);
            return;
        }
        self.confirm_revert_paths(paths, window, cx);
    }

    /// Confirm discarding / deleting every untracked path in the snapshot.
    pub(super) fn confirm_revert_all_untracked(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.paths_with_kind(sgg_core::ChangeKind::Untracked);
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
        let cancel = t.branch_cancel;
        let description = t.desc_revert.replace("{paths}", &preview);
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            title,
            description,
            ok,
            cancel,
            true,
            move |_, window, cx| {
                let paths = paths.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::Revert, window, cx, move |this, _, cx| {
                        this.revert_paths_now(&paths, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn revert_paths_now(&mut self, paths: &[String], cx: &mut Context<Self>) {
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
        use super::page::AppPage;
        use super::view::PreviewTab;

        self.ensure_parents_expanded(&rel_path);

        if self.page == AppPage::Files {
            self.stash_active_preview_tab(cx);
            if let Some(idx) = self
                .preview_tabs
                .iter()
                .position(|t| t.path == rel_path)
            {
                self.select_preview_tab(idx, window, cx);
                self.set_ok_status(format!("Preview {rel_path}"));
                return;
            }
            // Load from disk into a new tab.
            let full = self.repo_root().join(&rel_path);
            let content = match read_preview_text(&rel_path, &full) {
                Ok(text) => text,
                Err(err) => {
                    self.set_error(err.into(), cx);
                    return;
                }
            };
            self.preview_tabs.push(PreviewTab {
                path: rel_path.clone(),
                content: content.clone(),
            });
            let idx = self.preview_tabs.len() - 1;
            self.preview_tab_index = idx;
            self.preview_input.update(cx, |input, cx| {
                input.set_value(&content, window, cx);
            });
            self.preview_path = Some(rel_path.clone());
            self.error = None;
            self.push_nav_entry(self.current_nav_entry());
            self.set_ok_status(format!("Preview {rel_path}"));
            cx.notify();
            return;
        }

        // Git page: single preview pane (no tab strip).
        if self.preview_path.as_deref() == Some(rel_path.as_str()) {
            return;
        }
        self.load_preview_into(&rel_path, window, cx);
        self.push_nav_entry(self.current_nav_entry());
        self.set_ok_status(format!("Preview {rel_path}"));
        cx.notify();
    }

    pub(super) fn confirm_delete_path(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = t.confirm_delete_file;
        let ok = t.delete_file;
        let cancel = t.branch_cancel;
        let description = t.desc_delete_file.replace("{path}", &path);
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            title,
            description,
            ok,
            cancel,
            true,
            move |_, window, cx| {
                let path = path.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::FileDelete, window, cx, move |this, _, cx| {
                        this.delete_path_now(&path, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn confirm_file_op_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = self.file_op_input.read(cx).value().trim().to_string();
        if value.is_empty() {
            self.set_error("path is empty".into(), cx);
            return;
        }
        match self.file_op_form.clone() {
            super::view::FileOpForm::Rename { from } => {
                let parent = from.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
                let new_path = if parent.is_empty() {
                    value.clone()
                } else {
                    format!("{parent}/{value}")
                };
                let t = self.t();
                let title = t.confirm_rename_file;
                let ok = t.rename_file;
                let cancel = t.branch_cancel;
                let description = t
                    .desc_rename_file
                    .replace("{old}", &from)
                    .replace("{new}", &new_path);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let from = from.clone();
                        let new_path = new_path.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(BusyKind::FileRename, window, cx, move |this, window, cx| {
                                this.rename_path_now(&from, &new_path, window, cx);
                            });
                        });
                        true
                    },
                );
            }
            super::view::FileOpForm::Move { from } => {
                let t = self.t();
                let title = t.confirm_move_file;
                let ok = t.move_file;
                let cancel = t.branch_cancel;
                let description = t
                    .desc_move_file
                    .replace("{old}", &from)
                    .replace("{new}", &value);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let from = from.clone();
                        let value = value.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(BusyKind::FileMove, window, cx, move |this, window, cx| {
                                this.rename_path_now(&from, &value, window, cx);
                            });
                        });
                        true
                    },
                );
            }
            super::view::FileOpForm::NewFile { parent } => {
                if let Err(err) = validate_new_entry_name(&value) {
                    self.set_error(err.into(), cx);
                    return;
                }
                let path = join_parent_name(&parent, &value);
                let t = self.t();
                let title = t.confirm_new_file;
                let ok = t.new_file;
                let cancel = t.branch_cancel;
                let description = t.desc_new_file.replace("{path}", &path);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let path = path.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(BusyKind::FileCreate, window, cx, move |this, window, cx| {
                                this.create_file_now(&path, window, cx);
                            });
                        });
                        true
                    },
                );
            }
            super::view::FileOpForm::NewFolder { parent } => {
                if let Err(err) = validate_new_entry_name(&value) {
                    self.set_error(err.into(), cx);
                    return;
                }
                let path = join_parent_name(&parent, &value);
                let t = self.t();
                let title = t.confirm_new_folder;
                let ok = t.new_folder;
                let cancel = t.branch_cancel;
                let description = t.desc_new_folder.replace("{path}", &path);
                let view = cx.entity();
                open_confirm(
                    window,
                    cx,
                    title,
                    description,
                    ok,
                    cancel,
                    false,
                    move |_, window, cx| {
                        let path = path.clone();
                        view.update(cx, |this, cx| {
                            this.run_busy(
                                BusyKind::FolderCreate,
                                window,
                                cx,
                                move |this, window, cx| {
                                    this.create_folder_now(&path, window, cx);
                                },
                            );
                        });
                        true
                    },
                );
            }
            super::view::FileOpForm::Hidden => {}
        }
    }

    pub(super) fn create_file_now(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let full = self.repo_root().join(path);
        if full.exists() {
            self.set_error(format!("Already exists: {path}").into(), cx);
            return;
        }
        if let Some(parent) = full.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.set_error(format!("Failed to create parent: {err}").into(), cx);
                return;
            }
        }
        match fs::write(&full, b"") {
            Ok(()) => {
                self.ensure_parents_expanded(path);
                self.close_file_op_form(window, cx);
                self.set_ok_status(format!("Created file {path}"));
                self.reload(cx);
                self.preview_file(path.to_string(), window, cx);
            }
            Err(err) => self.set_error(format!("Failed to create {path}: {err}").into(), cx),
        }
    }

    pub(super) fn create_folder_now(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let full = self.repo_root().join(path);
        if full.exists() {
            self.set_error(format!("Already exists: {path}").into(), cx);
            return;
        }
        match fs::create_dir_all(&full) {
            Ok(()) => {
                self.ensure_parents_expanded(&format!("{path}/_"));
                self.expanded_dirs.insert(path.to_string());
                self.close_file_op_form(window, cx);
                self.set_ok_status(format!("Created folder {path}"));
                self.reload(cx);
            }
            Err(err) => self.set_error(format!("Failed to create {path}: {err}").into(), cx),
        }
    }

    pub(super) fn rename_path_now(
        &mut self,
        from: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let root = self.repo_root();
        let src = root.join(from);
        let dst = root.join(to);
        if let Some(parent) = dst.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                self.set_error(format!("Failed to create parent: {err}").into(), cx);
                return;
            }
        }
        match fs::rename(&src, &dst) {
            Ok(()) => {
                if self.preview_path.as_deref() == Some(from) {
                    self.preview_path = Some(to.to_string());
                }
                for tab in &mut self.preview_tabs {
                    if tab.path == from {
                        tab.path = to.to_string();
                    }
                }
                self.close_file_op_form(window, cx);
                self.set_ok_status(format!("Moved {from} → {to}"));
                self.reload(cx);
            }
            Err(err) => self.set_error(format!("Failed to move {from}: {err}").into(), cx),
        }
    }

    pub(super) fn delete_path_now(&mut self, path: &str, cx: &mut Context<Self>) {
        let full = self.repo_root().join(path);
        let result = if full.is_dir() {
            fs::remove_dir_all(&full)
        } else {
            fs::remove_file(&full)
        };
        match result {
            Ok(()) => {
                self.preview_tabs.retain(|t| t.path != path);
                if self.preview_tab_index >= self.preview_tabs.len()
                    && !self.preview_tabs.is_empty()
                {
                    self.preview_tab_index = self.preview_tabs.len() - 1;
                }
                if self.preview_path.as_deref() == Some(path) {
                    if let Some(tab) = self.preview_tabs.get(self.preview_tab_index) {
                        self.preview_path = Some(tab.path.clone());
                    } else {
                        self.preview_path = None;
                        self.preview_tab_index = 0;
                    }
                }
                self.set_ok_status(format!("Deleted {path}"));
                self.reload(cx);
            }
            Err(err) => self.set_error(format!("Failed to delete {path}: {err}").into(), cx),
        }
    }

    pub(super) fn ensure_parents_expanded(&mut self, rel_path: &str) {
        let parts: Vec<&str> = rel_path.split('/').collect();
        if parts.len() <= 1 {
            return;
        }
        let mut changed = false;
        let mut prefix = String::new();
        for part in &parts[..parts.len() - 1] {
            if prefix.is_empty() {
                prefix = (*part).to_string();
            } else {
                prefix = format!("{prefix}/{part}");
            }
            changed |= self.expanded_dirs.insert(prefix.clone());
        }
        if changed {
            self.rebuild_files_tree();
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
        match read_preview_text(rel_path, &full) {
            Ok(content) => {
                self.preview_input.update(cx, |input, cx| {
                    input.set_value(&content, window, cx);
                });
                self.preview_path = Some(rel_path.to_string());
                self.error = None;
            }
            Err(err) => {
                self.set_error(err.into(), cx);
            }
        }
    }

    pub(super) fn save_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rel) = self.preview_path.clone() else {
            self.set_error("No file open in preview".into(), cx);
            return;
        };
        let content = self.preview_input.read(cx).value().to_string();
        if let Some(tab) = self.preview_tabs.iter_mut().find(|t| t.path == rel) {
            tab.content = content.clone();
        }
        self.run_busy(BusyKind::SavePreview, window, cx, move |this, _, cx| {
            let full = this.repo_root().join(&rel);
            match fs::write(&full, &content) {
                Ok(()) => {
                    this.set_ok_status(format!("Saved {rel}"));
                    this.reload(cx);
                }
                Err(err) => this.set_error(format!("Failed to save {rel}: {err}").into(), cx),
            }
        });
    }

    pub(super) fn save_custom_editor_command(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_busy(BusyKind::SaveEditor, window, cx, |this, _, cx| {
            this.persist_prefs(cx);
            this.set_ok_status(this.t().settings_saved);
            cx.notify();
        });
    }

    pub(super) fn open_project_in_editor_busy(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_busy(BusyKind::OpenProject, window, cx, |this, _, cx| {
            this.open_project_in_editor(cx);
        });
    }

    pub(super) fn mute_current_fetch_prompt(&mut self, cx: &mut Context<Self>) {
        let root = self.repo_root();
        let key = crate::app::prefs::display_path(&root);
        if !self.fetch_prompt_muted.iter().any(|p| p == &key) {
            self.fetch_prompt_muted.push(key);
            self.persist_prefs(cx);
            cx.notify();
        }
    }
}

fn join_parent_name(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

fn validate_new_entry_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("name is empty");
    }
    if name.contains('/') || name.contains('\\') {
        return Err("name must not contain path separators");
    }
    if name == "." || name == ".." {
        return Err("invalid name");
    }
    Ok(())
}