use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _};
use gpui_kit::*;
use sgg_core::{BranchInfo, RemoteInfo};

use crate::app::ops::RemoteAction;
use crate::app::view::AppView;
use crate::app::widgets::{action_btn, display_btn};

pub fn render(
    view: &AppView,
    branches: Vec<BranchInfo>,
    remotes: Vec<RemoteInfo>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let selected_remote = view.selected_remote.clone();

    div()
        .flex()
        .flex_col()
        .w(px(320.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .font_semibold()
                        .text_left()
                        .child(format!("{}: `{selected_remote}`", t.remote_label)),
                )
                .child(
                    div()
                        .flex()
                        .justify_center()
                        .gap_2()
                        .child(
                            action_btn("fetch")
                                .outline()
                                .small()
                                .label(t.fetch)
                                .tooltip(t.tip_fetch)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_remote_action(RemoteAction::Fetch, window, cx)
                                })),
                        )
                        .child(
                            action_btn("pull")
                                .outline()
                                .small()
                                .label(t.pull)
                                .tooltip(t.tip_pull)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_remote_action(RemoteAction::Pull, window, cx)
                                })),
                        )
                        .child(
                            action_btn("push")
                                .primary()
                                .small()
                                .label(t.push)
                                .tooltip(t.tip_push)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.confirm_remote_action(RemoteAction::Push, window, cx)
                                })),
                        ),
                ),
        )
        .child(
            div()
                .px_3()
                .pt_2()
                .font_semibold()
                .text_left()
                .child(t.remotes),
        )
        .child(
            div()
                .p_3()
                .pt_1()
                .gap_1()
                .flex()
                .flex_col()
                .items_start()
                .border_b_1()
                .border_color(cx.theme().border)
                .children(remotes.into_iter().map(|r| {
                    let name = r.name.clone();
                    let selected = name == selected_remote;
                    let text = match r.url {
                        Some(url) => format!("{} — {}", r.name, url),
                        None => r.name.clone(),
                    };
                    display_btn(SharedString::from(format!("remote:{}", r.name)))
                        .ghost()
                        .label(text)
                        .tooltip(t.tip_select_remote)
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.select_remote(name.clone(), cx);
                        }))
                        .into_any_element()
                })),
        )
        .child(
            div()
                .px_3()
                .py_2()
                .font_semibold()
                .text_left()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(t.branches),
        )
        .child(
            div()
                .id("branch-list")
                .flex_1()
                .min_h_0()
                .p_2()
                .gap_1()
                .flex()
                .flex_col()
                .items_start()
                .overflow_y_scroll()
                .children(branches.into_iter().map(|b| branch_row(b, t.tip_switch_branch, cx))),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(Input::new(&view.branch_input))
                .child(
                    div().flex().justify_center().child(
                        action_btn("create-branch")
                            .outline()
                            .label(t.create_switch)
                            .tooltip(t.tip_create_branch)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_branch(window, cx)
                            })),
                    ),
                ),
        )
}

fn branch_row(
    branch: BranchInfo,
    tip: &'static str,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let name = branch.name.clone();
    let label = if branch.is_head {
        format!("* {}", branch.name)
    } else if branch.is_remote {
        format!("  remotes/{}", branch.name)
    } else {
        format!("  {}", branch.name)
    };
    let disabled = branch.is_remote || branch.is_head;

    display_btn(SharedString::from(format!("branch:{}", branch.name)))
        .ghost()
        .label(label)
        .tooltip(tip)
        .disabled(disabled)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.switch_branch(name.clone(), cx);
        }))
        .into_any_element()
}