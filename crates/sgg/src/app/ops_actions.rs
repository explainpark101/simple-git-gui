//! GitHub Actions ops (`gh workflow` / `gh run`).

use gpui_kit::*;

use super::busy::BusyKind;
use super::dialogs::open_confirm;
use super::github_actions;
use super::github_pr::GhError;
use super::view::AppView;

impl AppView {
    pub(super) fn select_workflow(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.selected_workflow == Some(id) {
            return;
        }
        self.selected_workflow = Some(id);
        self.selected_run = None;
        self.refresh_workflow_runs(cx);
        cx.notify();
    }

    pub(super) fn select_workflow_run(&mut self, id: u64, cx: &mut Context<Self>) {
        self.selected_run = Some(id);
        cx.notify();
    }

    pub(super) fn refresh_actions(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        if root.as_os_str().is_empty() {
            self.set_error(self.t().no_folder_open.into(), cx);
            return;
        }
        let selected_workflow = self.selected_workflow;
        self.busy = Some(BusyKind::ActionsList);
        self.begin_progress(BusyKind::ActionsList);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let workflows = github_actions::list_workflows(&root)?;
                    // Cross-workflow recent runs → in-progress dots on the workflow list.
                    let activity = github_actions::list_runs(&root, None, 50)?;
                    let filter = selected_workflow
                        .or_else(|| workflows.first().map(|w| w.id))
                        .and_then(|id| {
                            workflows
                                .iter()
                                .find(|w| w.id == id)
                                .map(|w| w.id.to_string())
                        });
                    let runs = if let Some(ref f) = filter {
                        github_actions::list_runs(&root, Some(f.as_str()), 30)?
                    } else {
                        activity.clone()
                    };
                    Ok::<_, GhError>((workflows, activity, runs, selected_workflow))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok((workflows, activity, runs, selected_hint)) => {
                        let keep_wf = this
                            .selected_workflow
                            .or(selected_hint)
                            .filter(|id| workflows.iter().any(|w| w.id == *id));
                        this.workflows = workflows;
                        this.selected_workflow =
                            keep_wf.or_else(|| this.workflows.first().map(|w| w.id));
                        this.set_workflows_in_progress_from_runs(&activity);
                        let keep_run = this
                            .selected_run
                            .filter(|id| runs.iter().any(|r| r.database_id == *id));
                        this.workflow_runs = runs;
                        this.selected_run = keep_run.or_else(|| {
                            this.workflow_runs.first().map(|r| r.database_id)
                        });
                        this.actions_list_error = None;
                        this.pr_gh_missing = false;
                        this.set_ok_status(format!(
                            "Loaded {} workflow(s), {} recent run(s)",
                            this.workflows.len(),
                            this.workflow_runs.len()
                        ));
                    }
                    Err(err) => {
                        this.workflows.clear();
                        this.workflow_runs.clear();
                        this.workflows_in_progress.clear();
                        this.selected_workflow = None;
                        this.selected_run = None;
                        this.pr_gh_missing = err.is_not_found();
                        this.actions_list_error = Some(err.to_string());
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

    fn run_workflow_label(run: &super::github_actions::WorkflowRunInfo) -> String {
        if run.workflow_name.is_empty() {
            run.name.clone()
        } else {
            run.workflow_name.clone()
        }
    }

    fn set_workflows_in_progress_from_runs(
        &mut self,
        runs: &[super::github_actions::WorkflowRunInfo],
    ) {
        self.workflows_in_progress = runs
            .iter()
            .filter(|r| r.is_in_progress())
            .map(Self::run_workflow_label)
            .filter(|n| !n.is_empty())
            .collect();
    }

    /// Update the in-progress flag for one workflow from its own run list.
    fn sync_workflow_in_progress_flag(&mut self, workflow_name: &str, runs: &[super::github_actions::WorkflowRunInfo]) {
        if workflow_name.is_empty() {
            return;
        }
        if runs.iter().any(|r| r.is_in_progress()) {
            self.workflows_in_progress.insert(workflow_name.to_string());
        } else {
            self.workflows_in_progress.remove(workflow_name);
        }
    }

    pub(super) fn refresh_workflow_runs(&mut self, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        if root.as_os_str().is_empty() {
            return;
        }
        let filter = self.selected_workflow.map(|id| id.to_string());
        let workflow_name = self
            .selected_workflow
            .and_then(|id| self.workflows.iter().find(|w| w.id == id))
            .map(|w| w.name.clone())
            .unwrap_or_default();
        self.busy = Some(BusyKind::ActionsList);
        self.begin_progress(BusyKind::ActionsList);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    github_actions::list_runs(&root, filter.as_deref(), 30)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(runs) => {
                        this.sync_workflow_in_progress_flag(&workflow_name, &runs);
                        let keep = this
                            .selected_run
                            .filter(|id| runs.iter().any(|r| r.database_id == *id));
                        this.workflow_runs = runs;
                        this.selected_run =
                            keep.or_else(|| this.workflow_runs.first().map(|r| r.database_id));
                        this.actions_list_error = None;
                        this.pr_gh_missing = false;
                        this.set_ok_status(format!(
                            "Loaded {} run(s)",
                            this.workflow_runs.len()
                        ));
                    }
                    Err(err) => {
                        this.workflow_runs.clear();
                        this.selected_run = None;
                        this.pr_gh_missing = err.is_not_found();
                        this.actions_list_error = Some(err.to_string());
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

    pub(super) fn confirm_run_workflow(
        &mut self,
        workflow_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = self
            .workflows
            .iter()
            .find(|w| w.id == workflow_id)
            .map(|w| w.name.clone())
            .unwrap_or_else(|| workflow_id.to_string());
        let branch = self
            .snapshot
            .as_ref()
            .and_then(|s| s.branches.iter().find(|b| b.is_head).map(|b| b.name.clone()))
            .unwrap_or_default();
        let t = self.t();
        let description = t
            .desc_actions_run
            .replace("{name}", &name)
            .replace("{ref}", if branch.is_empty() { "HEAD" } else { &branch });
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_actions_run,
            description,
            t.actions_run,
            t.branch_cancel,
            false,
            move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.run_workflow_now(workflow_id, cx);
                });
                true
            },
        );
    }

    pub(super) fn run_workflow_now(&mut self, workflow_id: u64, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        let git_ref = self
            .snapshot
            .as_ref()
            .and_then(|s| s.branches.iter().find(|b| b.is_head).map(|b| b.name.clone()));
        self.busy = Some(BusyKind::ActionsRun);
        self.begin_progress(BusyKind::ActionsRun);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    github_actions::run_workflow(&root, workflow_id, git_ref.as_deref())
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(out) => {
                        this.set_ok_status(if out.is_empty() {
                            this.t().actions_run_started.to_string()
                        } else {
                            out
                        });
                        this.refresh_workflow_runs(cx);
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

    pub(super) fn toggle_workflow_enabled(
        &mut self,
        workflow_id: u64,
        enable: bool,
        cx: &mut Context<Self>,
    ) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        self.busy = Some(BusyKind::ActionsToggle);
        self.begin_progress(BusyKind::ActionsToggle);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if enable {
                        github_actions::enable_workflow(&root, workflow_id)
                    } else {
                        github_actions::disable_workflow(&root, workflow_id)
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(_) => {
                        this.set_ok_status(if enable {
                            this.t().actions_enabled
                        } else {
                            this.t().actions_disabled
                        });
                        this.refresh_actions(cx);
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

    pub(super) fn open_run_in_browser(&mut self, run_id: u64, cx: &mut Context<Self>) {
        if let Some(run) = self.workflow_runs.iter().find(|r| r.database_id == run_id) {
            cx.open_url(&run.url);
            self.set_ok_status(format!("Opened run #{run_id}"));
            cx.notify();
            return;
        }
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        self.busy = Some(BusyKind::ActionsView);
        self.begin_progress(BusyKind::ActionsView);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { github_actions::view_run_web(&root, run_id) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(_) => this.set_ok_status(format!("Opened run #{run_id}")),
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

    pub(super) fn confirm_rerun(
        &mut self,
        run_id: u64,
        failed_only: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let title = if failed_only {
            t.confirm_actions_rerun_failed
        } else {
            t.confirm_actions_rerun
        };
        let ok = if failed_only {
            t.actions_rerun_failed
        } else {
            t.actions_rerun
        };
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            title,
            t.desc_actions_rerun.replace("{id}", &run_id.to_string()),
            ok,
            t.branch_cancel,
            false,
            move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.rerun_now(run_id, failed_only, cx);
                });
                true
            },
        );
    }

    pub(super) fn rerun_now(
        &mut self,
        run_id: u64,
        failed_only: bool,
        cx: &mut Context<Self>,
    ) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        self.busy = Some(BusyKind::ActionsRerun);
        self.begin_progress(BusyKind::ActionsRerun);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    github_actions::rerun_run(&root, run_id, failed_only)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(_) => {
                        this.set_ok_status(format!("Reran run #{run_id}"));
                        this.refresh_workflow_runs(cx);
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

    pub(super) fn confirm_cancel_run(
        &mut self,
        run_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let t = self.t();
        let view = cx.entity();
        open_confirm(
            window,
            cx,
            t.confirm_actions_cancel,
            t.desc_actions_cancel.replace("{id}", &run_id.to_string()),
            t.actions_cancel,
            t.branch_cancel,
            true,
            move |_, _, cx| {
                view.update(cx, |this, cx| {
                    this.cancel_run_now(run_id, cx);
                });
                true
            },
        );
    }

    pub(super) fn cancel_run_now(&mut self, run_id: u64, cx: &mut Context<Self>) {
        if self.busy.is_some() || self.network_task.is_some() {
            return;
        }
        let root = self.repo_root();
        self.busy = Some(BusyKind::ActionsCancel);
        self.begin_progress(BusyKind::ActionsCancel);
        cx.notify();
        self.network_task = Some(cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { github_actions::cancel_run(&root, run_id) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.network_task = None;
                this.busy = None;
                match result {
                    Ok(_) => {
                        this.set_ok_status(format!("Cancelled run #{run_id}"));
                        this.refresh_workflow_runs(cx);
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
}
