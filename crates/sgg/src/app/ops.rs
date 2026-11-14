use std::fs;
use std::time::Duration;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::{DialogFooter, DialogHeader, DialogTitle};
use gpui_kit::*;

use super::busy::BusyKind;
use super::dialogs::open_confirm;
use super::i18n::UiText;
use super::prefs::absolute_path;
use super::view::AppView;
use super::widgets::btn;

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
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(30))
                .await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    work(this, window, cx);
                    this.busy = None;
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

        window.open_dialog(cx, move |dialog, _, cx| {
            let view_cb = view.clone();
            let view_skip = view.clone();
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
                        .child(
                            btn("fetch-skip")
                                .outline()
                                .label(skip_label)
                                .on_click(move |_, window, cx| {
                                    view_skip.update(cx, |this, cx| {
                                        this.finish_fetch_prompt(false, window, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        )
                        .child(
                            crate::app::widgets::confirm_btn(
                                "fetch-do",
                                gpui_kit::assets::IconName::CloudDownload,
                                cx,
                            )
                                .label(fetch_label)
                                .on_click(move |_, window, cx| {
                                    view_cb.update(cx, |this, cx| {
                                        this.finish_fetch_prompt(true, window, cx);
                                    });
                                    window.close_dialog(cx);
                                }),
                        ),
                )
        });
    }

    pub(super) fn finish_fetch_prompt(
        &mut self,
        do_fetch: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.fetch_dont_ask {
            self.mute_current_fetch_prompt(cx);
        }
        self.fetch_dont_ask = false;
        if do_fetch {
            self.run_busy(BusyKind::Fetch, window, cx, |this, _, cx| this.fetch_now(cx));
        }
        cx.notify();
    }

    pub(super) fn stage_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected.clone();
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
        let paths = self.selected.clone();
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
        open_confirm(
            window,
            cx,
            title,
            description,
            ok,
            cancel,
            false,
            move |_, window, cx| {
                view.update(cx, |this, cx| match action {
                    RemoteAction::Fetch => {
                        this.run_busy(BusyKind::Fetch, window, cx, |this, _, cx| {
                            this.fetch_now(cx)
                        })
                    }
                    RemoteAction::Pull => {
                        this.run_busy(BusyKind::Pull, window, cx, |this, _, cx| {
                            this.pull_now(cx)
                        })
                    }
                    RemoteAction::Push => {
                        this.run_busy(BusyKind::Push, window, cx, |this, _, cx| {
                            this.push_now(cx)
                        })
                    }
                });
                true
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
        // Keep the current page (Git shows Changes + preview; Files stays on Files).
        if self.preview_path.as_deref() == Some(rel_path.as_str()) {
            return;
        }
        self.ensure_parents_expanded(&rel_path);
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
            super::view::FileOpForm::Hidden => {}
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
                if self.preview_path.as_deref() == Some(path) {
                    self.preview_path = None;
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

    pub(super) fn save_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rel) = self.preview_path.clone() else {
            self.set_error("No file open in preview".into(), cx);
            return;
        };
        let content = self.preview_input.read(cx).value().to_string();
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