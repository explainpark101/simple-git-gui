//! GitHub Pull Requests page (list | detail).

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
use crate::app::github_pr::PrMergeMethod;
use crate::app::view::AppView;
use crate::app::widgets::{
    action_btn, display_btn, icon_btn, pane_body, pane_header, with_busy,
};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let method = view.pr_merge_method;
    let selected = view.selected_pr;
    let list_min = px(280.);
    let list_default = px(360.);

    let list = div()
        .id("pr-page-list")
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
                        .child(format!("{} ({})", t.tab_prs, view.pull_requests.len())),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(with_busy(
                            icon_btn("pr-page-refresh", AssetIcon::RefreshCw)
                                .accessibility_label(t.pr_refresh)
                                .tooltip(t.tip_pr_refresh)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_pull_requests(cx);
                                })),
                            view.busy,
                            BusyKind::PrList,
                        ))
                        .when(view.pr_gh_missing, |this| {
                            this.child(with_busy(
                                icon_btn("pr-page-install-gh", AssetIcon::Download)
                                    .accessibility_label(t.pr_install_gh)
                                    .tooltip(t.tip_pr_install_gh)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.install_gh_cli(window, cx);
                                    })),
                                view.busy,
                                BusyKind::InstallGh,
                            ))
                        })
                        .child(with_busy(
                            icon_btn("pr-page-create", AssetIcon::GitPullRequestCreateArrow)
                                .accessibility_label(t.pr_create)
                                .tooltip(t.tip_pr_create)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.create_pr_web(cx);
                                })),
                            view.busy,
                            BusyKind::PrCreate,
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .children(PrMergeMethod::ALL.into_iter().map(|m| {
                    let label = match m {
                        PrMergeMethod::Merge => t.pr_merge_method_merge,
                        PrMergeMethod::Squash => t.pr_merge_method_squash,
                        PrMergeMethod::Rebase => t.pr_merge_method_rebase,
                    };
                    display_btn(SharedString::from(format!("pr-page-method-{:?}", m)))
                        .outline()
                        .small()
                        .label(label)
                        .tooltip(t.tip_pr_merge_method)
                        .selected(method == m)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_pr_merge_method(m, cx);
                        }))
                        .into_any_element()
                })),
        )
        .child(
            pane_body(cx)
                .id("pr-page-list-body")
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_1()
                .px_2()
                .py_2()
                .when_some(view.pr_list_error.as_ref(), |this, err| {
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
                                        display_btn("pr-page-install-gh-inline")
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
                    view.pr_list_error.is_none() && view.pull_requests.is_empty(),
                    |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .text_left()
                                .px_1()
                                .py_1()
                                .child(t.pr_empty),
                        )
                    },
                )
                .children(view.pull_requests.iter().map(|pr| {
                    let number = pr.number;
                    let title = pr.title.clone();
                    let head = pr.head_ref_name.clone();
                    let draft = pr.is_draft;
                    let conflict = pr.mergeable.eq_ignore_ascii_case("CONFLICTING");
                    let is_selected = selected == Some(number);
                    display_btn(SharedString::from(format!("pr-row:{number}")))
                        .ghost()
                        .selected(is_selected)
                        .w_full()
                        .px_2()
                        .py_1()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_pull_request(number, cx);
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
                                        .child(format!("#{number} {title}")),
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
                                        .child({
                                            let mut meta = head;
                                            if draft {
                                                meta = format!("{meta} · {}", t.pr_draft);
                                            }
                                            if conflict {
                                                meta = format!("{meta} · {}", t.pr_conflict);
                                            }
                                            meta
                                        }),
                                ),
                        )
                        .into_any_element()
                })),
        );

    let detail = detail_pane(view, cx);

    h_resizable("pr-panes")
        .child(
            resizable_panel()
                .size(list_default)
                .size_range(list_min..px(640.))
                .child(list),
        )
        .child(resizable_panel().child(detail))
}

fn detail_pane(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let selected = view
        .selected_pr
        .and_then(|n| view.pull_requests.iter().find(|p| p.number == n).cloned());

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
                        .child(match &selected {
                            Some(pr) => format!("#{} {}", pr.number, pr.title),
                            None => t.pr_section.to_string(),
                        }),
                )
                .when_some(selected.as_ref(), |this, pr| {
                    let number = pr.number;
                    let draft = pr.is_draft;
                    let conflict = pr.mergeable.eq_ignore_ascii_case("CONFLICTING");
                    this.child(
                        with_busy(
                            action_btn("pr-detail-open", AssetIcon::ExternalLink)
                                .outline()
                                .small()
                                .label(t.pr_open)
                                .tooltip(t.tip_pr_open)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_pr_in_browser(number, cx);
                                })),
                            view.busy,
                            BusyKind::PrView,
                        ),
                    )
                    .child(with_busy(
                        action_btn("pr-detail-merge", AssetIcon::GitMerge)
                            .primary()
                            .small()
                            .label(t.pr_merge)
                            .tooltip(t.tip_pr_merge)
                            .disabled(draft || conflict)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.confirm_merge_pr(number, window, cx);
                            })),
                        view.busy,
                        BusyKind::PrMerge,
                    ))
                }),
        )
        .child(
            pane_body(cx)
                .id("pr-page-detail-body")
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .px_4()
                .py_3()
                .flex()
                .flex_col()
                .gap_3()
                .when(selected.is_none(), |this| {
                    this.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .text_left()
                            .line_height(relative(1.35))
                            .child(t.pr_select_hint),
                    )
                })
                .when_some(selected, |this, pr| {
                    let draft = pr.is_draft;
                    let conflict = pr.mergeable.eq_ignore_ascii_case("CONFLICTING");
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
                                    .child(format!("#{} {}", pr.number, pr.title)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .line_height(relative(1.35))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_left()
                                    .child({
                                        let mut meta = pr.head_ref_name.clone();
                                        if draft {
                                            meta = format!("{meta} · {}", t.pr_draft);
                                        }
                                        if conflict {
                                            meta = format!("{meta} · {}", t.pr_conflict);
                                        }
                                        if !pr.state.is_empty() {
                                            meta = format!("{meta} · {}", pr.state);
                                        }
                                        meta
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .line_height(relative(1.35))
                                    .text_color(cx.theme().muted_foreground)
                                    .text_left()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .child(pr.url.clone()),
                            ),
                    )
                }),
        )
}
