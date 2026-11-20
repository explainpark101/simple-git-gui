//! Rebase / merge / cherry-pick / tag / stash ops (ROADMAP).

use gpui_kit::*;
use sgg_core::RepoOperation;

use super::busy::BusyKind;
use super::dialogs::open_confirm;
use super::view::{AppView, TagForm};

impl AppView {
    pub(super) fn repo_operation(&self) -> RepoOperation {
        self.snapshot
            .as_ref()
            .map(|s| s.operation)
            .unwrap_or(RepoOperation::Clean)
    }

    // ── Rebase ──────────────────────────────────────────────────────────

    pub(super) fn confirm_rebase_onto(
        &mut self,
        onto: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = t.confirm_rebase_onto;
        let ok = t.rebase_onto;
        let cancel = t.branch_cancel;
        let description = t.desc_rebase_onto.replace("{onto}", &onto);
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
                let onto = onto.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::Rebase, window, cx, move |this, _, cx| {
                        this.rebase_onto_now(onto, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn rebase_onto_now(&mut self, onto: String, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.rebase_onto(&onto)) {
            Ok(out) => {
                let msg = if out.is_empty() {
                    format!("Rebase onto {onto} completed")
                } else {
                    out
                };
                self.set_ok_status(msg);
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_rebase_continue(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_busy(BusyKind::RebaseContinue, window, cx, |this, _, cx| {
            this.rebase_continue_now(cx);
        });
    }

    pub(super) fn rebase_continue_now(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.rebase_continue()) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Rebase continued".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_rebase_abort(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_rebase_abort,
            t.desc_rebase_abort,
            t.rebase_abort,
            t.branch_cancel,
            true,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::RebaseAbort, window, cx, |this, _, cx| {
                        this.rebase_abort_now(cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn rebase_abort_now(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.rebase_abort()) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Rebase aborted".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn confirm_rebase_skip(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_rebase_skip,
            t.desc_rebase_skip,
            t.rebase_skip,
            t.branch_cancel,
            false,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::RebaseSkip, window, cx, |this, _, cx| {
                        this.rebase_skip_now(cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn rebase_skip_now(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.rebase_skip()) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Rebase skipped commit".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    // ── Merge ───────────────────────────────────────────────────────────

    pub(super) fn confirm_merge_branch(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_merge,
            t.desc_merge.replace("{name}", &name),
            t.merge_branch,
            t.branch_cancel,
            false,
            move |_, window, cx| {
                let name = name.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::Merge, window, cx, move |this, _, cx| {
                        this.merge_branch_now(name, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn merge_branch_now(&mut self, name: String, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.merge_branch(&name)) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    format!("Merged {name}")
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_merge_continue(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_busy(BusyKind::MergeContinue, window, cx, |this, _, cx| {
            this.merge_continue_now(cx);
        });
    }

    pub(super) fn merge_continue_now(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.merge_continue()) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Merge continued".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_merge_abort(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_merge_abort,
            t.desc_merge_abort,
            t.merge_abort,
            t.branch_cancel,
            true,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::MergeAbort, window, cx, |this, _, cx| {
                        this.merge_abort_now(cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn merge_abort_now(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.merge_abort()) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Merge aborted".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    // ── Cherry-pick ─────────────────────────────────────────────────────

    pub(super) fn confirm_cherry_pick(
        &mut self,
        oid: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let short = oid.chars().take(7).collect::<String>();
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_cherry_pick,
            t.desc_cherry_pick.replace("{oid}", &short),
            t.cherry_pick,
            t.branch_cancel,
            false,
            move |_, window, cx| {
                let oid = oid.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::CherryPick, window, cx, move |this, _, cx| {
                        this.cherry_pick_now(oid, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn cherry_pick_now(&mut self, oid: String, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.cherry_pick(&oid)) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    format!("Cherry-picked {oid}")
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_cherry_pick_continue(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.run_busy(BusyKind::CherryPickContinue, window, cx, |this, _, cx| {
            this.cherry_pick_continue_now(cx);
        });
    }

    pub(super) fn cherry_pick_continue_now(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.cherry_pick_continue()) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Cherry-pick continued".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_cherry_pick_abort(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_cherry_pick_abort,
            t.desc_cherry_pick_abort,
            t.cherry_pick_abort,
            t.branch_cancel,
            true,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::CherryPickAbort, window, cx, |this, _, cx| {
                        this.cherry_pick_abort_now(cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn cherry_pick_abort_now(&mut self, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.cherry_pick_abort()) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Cherry-pick aborted".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    // ── Tags ────────────────────────────────────────────────────────────

    pub(super) fn start_create_tag(
        &mut self,
        target: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Tag form lives in the Git sidebar Tags panel.
        if self.page != super::page::AppPage::Git {
            self.set_page(super::page::AppPage::Git, window, cx);
        }
        self.tag_form = TagForm::Create { target };
        self.tags_panel_open = true;
        self.tag_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub(super) fn close_tag_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tag_form = TagForm::Hidden;
        self.tag_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        cx.notify();
    }

    pub(super) fn confirm_tag_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let TagForm::Create { target } = self.tag_form.clone() else {
            return;
        };
        let name = self.tag_input.read(cx).value().to_string();
        let name = name.trim().to_string();
        if name.is_empty() {
            self.set_error(self.t().tag_name_required.into(), cx);
            return;
        }
        let t = self.t();
        let desc = match target.as_deref() {
            Some(oid) => t
                .desc_create_tag
                .replace("{name}", &name)
                .replace("{oid}", &oid.chars().take(7).collect::<String>()),
            None => t
                .desc_create_tag_head
                .replace("{name}", &name),
        };
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_create_tag,
            desc,
            t.create_tag,
            t.branch_cancel,
            false,
            move |_, window, cx| {
                let name = name.clone();
                let target = target.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::TagCreate, window, cx, move |this, window, cx| {
                        this.create_tag_now(name, target, window, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn create_tag_now(
        &mut self,
        name: String,
        target: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.with_repo(|repo| repo.create_tag(&name, target.as_deref(), None)) {
            Ok(()) => {
                self.close_tag_form(window, cx);
                self.set_ok_status(format!("Created tag {name}"));
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn confirm_delete_tag(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_delete_tag,
            t.desc_delete_tag.replace("{name}", &name),
            t.delete_tag,
            t.branch_cancel,
            true,
            move |_, window, cx| {
                let name = name.clone();
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::TagDelete, window, cx, move |this, _, cx| {
                        this.delete_tag_now(name, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn delete_tag_now(&mut self, name: String, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.delete_tag(&name)) {
            Ok(()) => {
                self.set_ok_status(format!("Deleted tag {name}"));
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn confirm_push_tag(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let remote = self.selected_remote.clone();
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_push_tag,
            t.desc_push_tag
                .replace("{name}", &name)
                .replace("{remote}", &remote),
            t.push_tag,
            t.branch_cancel,
            false,
            move |_, _, cx| {
                let name = name.clone();
                let remote = remote.clone();
                view.update(cx, |this, cx| {
                    this.push_tag_now(remote, name, cx);
                });
                true
            },
        );
    }

    pub(super) fn push_tag_now(
        &mut self,
        remote: String,
        name: String,
        cx: &mut Context<Self>,
    ) {
        // Tag push is network-bound; run off the UI thread (same as push/pull/fetch).
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        if root.as_os_str().is_empty() {
            self.set_error(self.t().no_folder_open.into(), cx);
            return;
        }
        let remote_ok = remote.clone();
        let name_ok = name.clone();
        self.busy = Some(BusyKind::TagPush);
        self.begin_progress(BusyKind::TagPush);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let repo = sgg_core::RepoState::open_nearest(&root)
                        .map_err(|e| e.to_string())?;
                    repo.push_tag(&remote, &name).map_err(|e| e.to_string())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(out) => {
                        this.set_ok_status(if out.is_empty() {
                            format!("Pushed tag {name_ok} → {remote_ok}")
                        } else {
                            out
                        });
                        this.reload(cx);
                    }
                    Err(err) => this.set_error(err.into(), cx),
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

    pub(super) fn toggle_tags_panel(&mut self, cx: &mut Context<Self>) {
        self.tags_panel_open = !self.tags_panel_open;
        self.persist_project_ui(cx);
        cx.notify();
    }

    // ── Stash ───────────────────────────────────────────────────────────

    pub(super) fn set_stash_include_untracked(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        self.stash_include_untracked = enabled;
        self.persist_prefs(cx);
        cx.notify();
    }

    pub(super) fn toggle_stash_panel(&mut self, cx: &mut Context<Self>) {
        self.stash_panel_open = !self.stash_panel_open;
        self.persist_project_ui(cx);
        cx.notify();
    }

    pub(super) fn confirm_stash_push(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let include = self.stash_include_untracked;
        let t = self.t();
        let description = if include {
            t.desc_stash_push_untracked
        } else {
            t.desc_stash_push
        };
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_stash_push,
            description,
            t.stash_push,
            t.branch_cancel,
            false,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::StashPush, window, cx, move |this, _, cx| {
                        this.stash_push_now(None, include, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn stash_push_now(
        &mut self,
        message: Option<String>,
        include_untracked: bool,
        cx: &mut Context<Self>,
    ) {
        match self.with_repo(|repo| {
            repo.stash_push(message.as_deref(), include_untracked)
        }) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Stashed working tree".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    pub(super) fn confirm_stash_pop(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let description = match index {
            Some(i) => t.desc_stash_pop_n.replace("{n}", &i.to_string()),
            None => t.desc_stash_pop.to_string(),
        };
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_stash_pop,
            description,
            t.stash_pop,
            t.branch_cancel,
            false,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::StashPop, window, cx, move |this, _, cx| {
                        this.stash_pop_now(index, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn stash_pop_now(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.stash_pop(index)) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Stash popped".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_stash_apply(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let description = match index {
            Some(i) => t.desc_stash_apply_n.replace("{n}", &i.to_string()),
            None => t.desc_stash_apply.to_string(),
        };
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_stash_apply,
            description,
            t.stash_apply,
            t.branch_cancel,
            false,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::StashApply, window, cx, move |this, _, cx| {
                        this.stash_apply_now(index, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn stash_apply_now(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.stash_apply(index)) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    "Stash applied".into()
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => {
                self.set_error(err, cx);
                self.reload(cx);
            }
        }
    }

    pub(super) fn confirm_stash_drop(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_stash_drop,
            t.desc_stash_drop.replace("{n}", &index.to_string()),
            t.stash_drop,
            t.branch_cancel,
            true,
            move |_, window, cx| {
                view.update(cx, |this, cx| {
                    this.run_busy(BusyKind::StashDrop, window, cx, move |this, _, cx| {
                        this.stash_drop_now(index, cx);
                    });
                });
                true
            },
        );
    }

    pub(super) fn stash_drop_now(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.with_repo(|repo| repo.stash_drop(index)) {
            Ok(out) => {
                self.set_ok_status(if out.is_empty() {
                    format!("Dropped stash@{{{index}}}")
                } else {
                    out
                });
                self.reload(cx);
            }
            Err(err) => self.set_error(err, cx),
        }
    }

    /// Shared continue/abort dispatcher for the in-progress banner.
    pub(super) fn confirm_continue_operation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.repo_operation() {
            RepoOperation::Rebase => self.confirm_rebase_continue(window, cx),
            RepoOperation::Merge => self.confirm_merge_continue(window, cx),
            RepoOperation::CherryPick => self.confirm_cherry_pick_continue(window, cx),
            _ => {}
        }
    }

    pub(super) fn confirm_abort_operation(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.repo_operation() {
            RepoOperation::Rebase => self.confirm_rebase_abort(window, cx),
            RepoOperation::Merge => self.confirm_merge_abort(window, cx),
            RepoOperation::CherryPick => self.confirm_cherry_pick_abort(window, cx),
            _ => {}
        }
    }
}
