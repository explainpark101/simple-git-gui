//! GitHub Actions page (workflows | runs).

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, h_resizable,
    resizable_panel,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::busy::BusyKind;
use crate::app::view::AppView;
use crate::app::widgets::{
    action_btn, display_btn, icon_btn, pane_body, pane_header, with_busy,
};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let selected_wf = view.selected_workflow;
    let selected_run = view.selected_run;
    let list_min = px(260.);
    let list_default = px(320.);

    let workflows = div()
        .id("actions-workflows")
        .size_full()
        .min_h_0()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .py_2()
                .child(
                    div()
                        .font_semibold()
                        .text_sm()
                        .text_left()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(format!(
                            "{} ({})",
                            t.actions_workflows,
                            view.workflows.len()
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(with_busy(
                            icon_btn("actions-refresh", AssetIcon::RefreshCw)
                                .accessibility_label(t.actions_refresh)
                                .tooltip(t.tip_actions_refresh)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_actions(cx);
                                })),
                            view.busy,
                            BusyKind::ActionsList,
                        ))
                        .when(view.pr_gh_missing, |this| {
                            this.child(with_busy(
                                icon_btn("actions-install-gh", AssetIcon::Download)
                                    .accessibility_label(t.pr_install_gh)
                                    .tooltip(t.tip_pr_install_gh)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.install_gh_cli(window, cx);
                                    })),
                                view.busy,
                                BusyKind::InstallGh,
                            ))
                        }),
                ),
        )
        .child(
            pane_body(cx)
                .id("actions-workflows-body")
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1()
                .px_2()
                .py_2()
                .when_some(view.actions_list_error.as_ref(), |this, err| {
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .min_w_0()
                            .px_1()
                            .py_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().danger)
                                    .text_left()
                                    .line_height(relative(1.35))
                                    .child(err.clone()),
                            )
                            .when(view.pr_gh_missing, |row| {
                                row.child(
                                    with_busy(
                                        display_btn("actions-install-gh-inline")
                                            .outline()
                                            .small()
                                            .label(t.pr_install_gh)
                                            .tooltip(t.tip_pr_install_gh)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.install_gh_cli(window, cx);
                                            })),
                                        view.busy,
                                        BusyKind::InstallGh,
                                    )
                                    .into_any_element(),
                                )
                            }),
                    )
                })
                .when(
                    view.actions_list_error.is_none() && view.workflows.is_empty(),
                    |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .text_left()
                                .px_1()
                                .py_1()
                                .child(t.actions_workflows_empty),
                        )
                    },
                )
                .children(view.workflows.iter().map(|wf| {
                    let id = wf.id;
                    let name = wf.name.clone();
                    let path = wf.path.clone();
                    let state = wf.state.clone();
                    let active = wf.is_active();
                    let is_selected = selected_wf == Some(id);
                    display_btn(SharedString::from(format!("actions-wf:{id}")))
                        .ghost()
                        .selected(is_selected)
                        .w_full()
                        .px_2()
                        .py_1()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_workflow(id, cx);
                        }))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .w_full()
                                .min_w_0()
                                .text_left()
                                .child(
                                    div()
                                        .text_sm()
                                        .line_height(relative(1.35))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_left()
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .line_height(relative(1.35))
                                        .text_color(cx.theme().muted_foreground)
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .text_left()
                                        .child(format!(
                                            "{} · {}",
                                            path,
                                            if active {
                                                t.actions_state_active
                                            } else {
                                                state.as_str()
                                            }
                                        )),
                                ),
                        )
                        .into_any_element()
                })),
        );

    let runs = runs_pane(view, selected_wf, selected_run, cx);

    h_resizable("actions-panes")
        .child(
            resizable_panel()
                .size(list_default)
                .size_range(list_min..px(560.))
                .child(workflows),
        )
        .child(resizable_panel().child(runs))
}

fn runs_pane(
    view: &AppView,
    selected_wf: Option<u64>,
    selected_run: Option<u64>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let wf = selected_wf.and_then(|id| view.workflows.iter().find(|w| w.id == id).cloned());
    let run = selected_run
        .and_then(|id| view.workflow_runs.iter().find(|r| r.database_id == id).cloned());

    div()
        .size_full()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(cx.theme().border)
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .child(
                    div()
                        .font_semibold()
                        .text_sm()
                        .text_left()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(match &wf {
                            Some(w) => w.name.clone(),
                            None => t.actions_runs.to_string(),
                        }),
                )
                .when_some(wf.as_ref(), |this, w| {
                    let id = w.id;
                    let active = w.is_active();
                    this.child(with_busy(
                        action_btn("actions-run-wf", AssetIcon::Play)
                            .primary()
                            .small()
                            .label(t.actions_run)
                            .tooltip(t.tip_actions_run)
                            .disabled(!active)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_run_workflow(id, window, cx);
                            })),
                        view.busy,
                        BusyKind::ActionsRun,
                    ))
                    .child(with_busy(
                        action_btn(
                            "actions-toggle-wf",
                            if active {
                                AssetIcon::CirclePause
                            } else {
                                AssetIcon::CirclePlay
                            },
                        )
                        .outline()
                        .small()
                        .label(if active {
                            t.actions_disable
                        } else {
                            t.actions_enable
                        })
                        .tooltip(if active {
                            t.tip_actions_disable
                        } else {
                            t.tip_actions_enable
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_workflow_enabled(id, !active, cx);
                        })),
                        view.busy,
                        BusyKind::ActionsToggle,
                    ))
                }),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    h_resizable("actions-runs-detail")
                        .child(
                            resizable_panel()
                                .size(px(360.))
                                .size_range(px(240.)..px(560.))
                                .child(runs_list(view, selected_run, cx)),
                        )
                        .child(resizable_panel().child(run_detail(view, run.as_ref(), cx))),
                ),
        )
}

fn runs_list(
    view: &AppView,
    selected_run: Option<u64>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    pane_body(cx)
        .id("actions-runs-body")
        .size_full()
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap_1()
        .px_2()
        .py_2()
        .when(view.workflow_runs.is_empty(), |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .text_left()
                    .px_1()
                    .py_1()
                    .child(t.actions_runs_empty),
            )
        })
        .children(view.workflow_runs.iter().map(|r| {
            let id = r.database_id;
            let title = if r.display_title.is_empty() {
                r.name.clone()
            } else {
                r.display_title.clone()
            };
            let meta = format!(
                "{} · {} · {}{}",
                r.head_branch,
                r.short_sha(),
                r.status,
                if r.conclusion.is_empty() {
                    String::new()
                } else {
                    format!(" / {}", r.conclusion)
                }
            );
            let is_selected = selected_run == Some(id);
            display_btn(SharedString::from(format!("actions-run:{id}")))
                .ghost()
                .selected(is_selected)
                .w_full()
                .px_2()
                .py_1()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_workflow_run(id, cx);
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .w_full()
                        .min_w_0()
                        .text_left()
                        .child(
                            div()
                                .text_sm()
                                .line_height(relative(1.35))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .text_left()
                                .child(title),
                        )
                        .child(
                            div()
                                .text_xs()
                                .line_height(relative(1.35))
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .text_left()
                                .child(meta),
                        ),
                )
                .into_any_element()
        }))
}

fn run_detail(
    view: &AppView,
    run: Option<&crate::app::github_actions::WorkflowRunInfo>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    div()
        .size_full()
        .min_h_0()
        .min_w_0()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(cx.theme().border)
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .child(
                    div()
                        .font_semibold()
                        .text_sm()
                        .text_left()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .child(match run {
                            Some(r) if !r.display_title.is_empty() => r.display_title.clone(),
                            Some(r) => r.name.clone(),
                            None => t.actions_run_detail.to_string(),
                        }),
                )
                .when_some(run.cloned(), |this, r| {
                    let id = r.database_id;
                    let in_progress = r.is_in_progress();
                    this.child(with_busy(
                        action_btn("actions-open-run", AssetIcon::ExternalLink)
                            .outline()
                            .small()
                            .label(t.actions_open)
                            .tooltip(t.tip_actions_open)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_run_in_browser(id, cx);
                            })),
                        view.busy,
                        BusyKind::ActionsView,
                    ))
                    .child(with_busy(
                        action_btn("actions-rerun", AssetIcon::RotateCcw)
                            .outline()
                            .small()
                            .label(t.actions_rerun)
                            .tooltip(t.tip_actions_rerun)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_rerun(id, false, window, cx);
                            })),
                        view.busy,
                        BusyKind::ActionsRerun,
                    ))
                    .child(with_busy(
                        action_btn("actions-rerun-failed", AssetIcon::RotateCcw)
                            .outline()
                            .small()
                            .label(t.actions_rerun_failed)
                            .tooltip(t.tip_actions_rerun_failed)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_rerun(id, true, window, cx);
                            })),
                        view.busy,
                        BusyKind::ActionsRerun,
                    ))
                    .child(with_busy(
                        action_btn("actions-cancel-run", AssetIcon::CircleX)
                            .danger()
                            .small()
                            .label(t.actions_cancel)
                            .tooltip(t.tip_actions_cancel)
                            .disabled(!in_progress)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_cancel_run(id, window, cx);
                            })),
                        view.busy,
                        BusyKind::ActionsCancel,
                    ))
                }),
        )
        .child(
            pane_body(cx)
                .id("actions-run-detail-body")
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .px_4()
                .py_3()
                .flex()
                .flex_col()
                .gap_3()
                .when(run.is_none(), |this| {
                    this.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .text_left()
                            .line_height(relative(1.35))
                            .child(t.actions_select_hint),
                    )
                })
                .when_some(run.cloned(), |this, r| {
                    let title = if r.display_title.is_empty() {
                        r.name.clone()
                    } else {
                        r.display_title.clone()
                    };
                    let status = if r.conclusion.is_empty() {
                        r.status.clone()
                    } else {
                        format!("{} / {}", r.status, r.conclusion)
                    };
                    let sha = r.short_sha().to_string();
                    this.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .min_w_0()
                            .child(
                                div()
                                    .text_lg()
                                    .font_semibold()
                                    .line_height(relative(1.35))
                                    .text_left()
                                    .child(title),
                            )
                            .child(detail_row(t.actions_field_status, status, cx))
                            .child(detail_row(
                                t.actions_field_workflow,
                                r.workflow_name.clone(),
                                cx,
                            ))
                            .child(detail_row(
                                t.actions_field_branch,
                                r.head_branch.clone(),
                                cx,
                            ))
                            .child(detail_row(t.actions_field_commit, sha, cx))
                            .child(detail_row(t.actions_field_event, r.event.clone(), cx))
                            .child(detail_row(
                                t.actions_field_created,
                                r.created_at.clone(),
                                cx,
                            ))
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(relative(1.35))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_left()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(r.url.clone()),
                            ),
                    )
                }),
        )
}

fn detail_row(label: &str, value: String, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w_0()
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(cx.theme().muted_foreground)
                .text_left()
                .child(label.to_string()),
        )
        .child(
            div()
                .text_sm()
                .line_height(relative(1.35))
                .text_left()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(value),
        )
}
