//! GitHub Pull Request ops (`gh` CLI) — list / open / merge / create.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::*;

use super::busy::BusyKind;
use super::cli_tools;
use super::dialogs::open_confirm;
use super::github_pr::{self, PrMergeMethod};
use super::view::AppView;

impl AppView {
    pub(super) fn toggle_pr_panel(&mut self, cx: &mut Context<Self>) {
        self.pr_panel_open = !self.pr_panel_open;
        if self.pr_panel_open && self.pull_requests.is_empty() && self.pr_list_error.is_none() {
            self.refresh_pull_requests(cx);
        }
        cx.notify();
    }

    pub(super) fn set_pr_merge_method(
        &mut self,
        method: PrMergeMethod,
        cx: &mut Context<Self>,
    ) {
        self.pr_merge_method = method;
        cx.notify();
    }

    pub(super) fn refresh_pull_requests(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        if root.as_os_str().is_empty() {
            self.set_error(self.t().no_folder_open.into(), cx);
            return;
        }
        self.busy = Some(BusyKind::PrList);
        self.begin_progress(BusyKind::PrList);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { github_pr::list_open_prs(&root) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(prs) => {
                        this.pull_requests = prs;
                        this.pr_list_error = None;
                        this.pr_gh_missing = false;
                        let n = this.pull_requests.len();
                        this.set_ok_status(format!("Loaded {n} open pull request(s)"));
                    }
                    Err(err) => {
                        this.pull_requests.clear();
                        this.pr_gh_missing = err.is_not_found();
                        this.pr_list_error = Some(err.to_string());
                        this.set_error(err.to_string().into(), cx);
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

    pub(super) fn open_pr_in_browser(
        &mut self,
        number: u64,
        cx: &mut Context<Self>,
    ) {
        if let Some(pr) = self.pull_requests.iter().find(|p| p.number == number) {
            cx.open_url(&pr.url);
            self.set_ok_status(format!("Opened PR #{number}"));
            cx.notify();
            return;
        }
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        self.busy = Some(BusyKind::PrView);
        self.begin_progress(BusyKind::PrView);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { github_pr::view_pr_web(&root, number) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(_) => this.set_ok_status(format!("Opened PR #{number}")),
                    Err(err) => this.set_error(err.to_string().into(), cx),
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

    pub(super) fn confirm_merge_pr(
        &mut self,
        number: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let method = self.pr_merge_method;
        let title_pr = self
            .pull_requests
            .iter()
            .find(|p| p.number == number)
            .map(|p| p.title.clone())
            .unwrap_or_default();
        let t = self.t();
        let method_label = match method {
            PrMergeMethod::Merge => t.pr_merge_method_merge,
            PrMergeMethod::Squash => t.pr_merge_method_squash,
            PrMergeMethod::Rebase => t.pr_merge_method_rebase,
        };
        let description = t
            .desc_pr_merge
            .replace("{n}", &number.to_string())
            .replace("{title}", &title_pr)
            .replace("{method}", method_label);
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_pr_merge,
            description,
            t.pr_merge,
            t.branch_cancel,
            false,
            move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.merge_pr_now(number, method, cx);
                });
                true
            },
        );
    }

    pub(super) fn merge_pr_now(
        &mut self,
        number: u64,
        method: PrMergeMethod,
        cx: &mut Context<Self>,
    ) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        self.busy = Some(BusyKind::PrMerge);
        self.begin_progress(BusyKind::PrMerge);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    github_pr::merge_pr(&root, number, method)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(out) => {
                        this.set_ok_status(if out.is_empty() {
                            format!("Merged PR #{number}")
                        } else {
                            out
                        });
                        this.reload(cx);
                        this.refresh_pull_requests(cx);
                    }
                    Err(err) => {
                        if err.is_not_found() {
                            this.pr_gh_missing = true;
                        }
                        this.set_error(err.to_string().into(), cx);
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

    pub(super) fn create_pr_web(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        self.busy = Some(BusyKind::PrCreate);
        self.begin_progress(BusyKind::PrCreate);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { github_pr::create_pr_web(&root) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(_) => this.set_ok_status(this.t().pr_create_opened),
                    Err(err) => {
                        if err.is_not_found() {
                            this.pr_gh_missing = true;
                        }
                        this.set_error(err.to_string().into(), cx);
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

    /// Download portable GitHub CLI into the app-managed tools directory.
    pub(super) fn install_gh_cli(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.commit_ai_install_cli_task.is_some()
            || self.busy == Some(BusyKind::InstallCli)
            || self.busy == Some(BusyKind::InstallGh)
        {
            return;
        }
        self.busy = Some(BusyKind::InstallGh);
        self.cli_install_progress = Some(0.0);
        self.begin_progress(BusyKind::InstallGh);
        cx.notify();

        let millipct = Arc::new(AtomicU32::new(0));
        let millipct_bg = millipct.clone();
        let progress_cb: cli_tools::InstallProgressFn = Arc::new(move |frac| {
            let v = (frac.clamp(0.0, 1.0) * 1000.0).round() as u32;
            millipct_bg.store(v, Ordering::Relaxed);
        });

        self.commit_ai_install_cli_task = Some(cx.spawn_in(window, async move |this, cx| {
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
                            if this.busy == Some(BusyKind::InstallGh) {
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
                .background_spawn(async move { cli_tools::install_gh(Some(progress_cb)) })
                .await;
            drop(poll);

            let _ = cx.update(|_window, cx| {
                this.update(cx, |this, cx| {
                    this.commit_ai_install_cli_task = None;
                    this.busy = None;
                    this.cli_install_progress = None;
                    match result {
                        Ok(path) => {
                            this.pr_gh_missing = false;
                            let msg = this
                                .t()
                                .pr_install_gh_done
                                .replace("{path}", &path);
                            this.set_ok_status(msg);
                            this.refresh_pull_requests(cx);
                        }
                        Err(err) => {
                            let msg = this
                                .t()
                                .pr_install_gh_failed
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
}
