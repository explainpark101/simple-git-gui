use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _, resizable_panel,
    v_resizable,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sgg_core::{BranchInfo, RemoteInfo};

use crate::app::busy::BusyKind;
use crate::app::ops::RemoteAction;
use crate::app::view::{AppView, BranchForm};
use crate::app::widgets::{
    action_btn, action_row, confirm_btn, display_btn, icon_btn, icon_danger_btn, pane_body,
    pane_header, pane_toolbar, with_busy,
};

pub fn render(
    view: &AppView,
    branches: Vec<BranchInfo>,
    remotes: Vec<RemoteInfo>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();

    if view.sidebar_collapsed {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .size_full()
            .bg(cx.theme().sidebar)
            .py_2()
            .child(
                icon_btn("sidebar-expand", AssetIcon::PanelLeftOpen)
                    .accessibility_label(t.tip_expand_sidebar)
                    .tooltip(t.tip_expand_sidebar)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_sidebar_collapsed(cx);
                    })),
            )
            .into_any_element();
    }

    let selected_remote = view.selected_remote.clone();
    let form = view.branch_form.clone();

    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(cx.theme().sidebar)
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_2()
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .min_w_0()
                        .flex_1()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().accent.opacity(0.28))
                        .font_semibold()
                        .text_sm()
                        .text_left()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(selected_remote.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .justify_end()
                        .items_center()
                        .gap_1()
                        .child(
                            with_busy(
                                action_btn("fetch", AssetIcon::CloudDownload)
                                    .outline()
                                    .small()
                                    .label(t.fetch)
                                    .tooltip(t.tip_fetch)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_remote_action(
                                            RemoteAction::Fetch,
                                            window,
                                            cx,
                                        )
                                    })),
                                view.busy,
                                BusyKind::Fetch,
                            ),
                        )
                        .child(
                            with_busy(
                                action_btn("pull", AssetIcon::ArrowDownToLine)
                                    .outline()
                                    .small()
                                    .label(t.pull)
                                    .tooltip(t.tip_pull)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_remote_action(
                                            RemoteAction::Pull,
                                            window,
                                            cx,
                                        )
                                    })),
                                view.busy,
                                BusyKind::Pull,
                            ),
                        )
                        .child(
                            with_busy(
                                action_btn("push", AssetIcon::CloudUpload)
                                    .primary()
                                    .small()
                                    .label(t.push)
                                    .tooltip(t.tip_push)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_remote_action(
                                            RemoteAction::Push,
                                            window,
                                            cx,
                                        )
                                    })),
                                view.busy,
                                BusyKind::Push,
                            ),
                        )
                        .child(
                            icon_btn("sidebar-collapse", AssetIcon::PanelLeftClose)
                                .accessibility_label(t.tip_collapse_sidebar)
                                .tooltip(t.tip_collapse_sidebar)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toggle_sidebar_collapsed(cx);
                                })),
                        ),
                ),
        )
        .child(
            div().flex_1().min_h_0().child(
                v_resizable("sidebar-panes")
                    .child(
                        resizable_panel()
                            .size(px(140.))
                            .size_range(px(80.)..px(360.))
                            .child(remotes_panel(view, remotes, selected_remote, cx)),
                    )
                    .child(
                        resizable_panel().child(branches_panel(view, branches, form, cx)),
                    ),
            ),
        )
        .into_any_element()
}

fn remotes_panel(
    view: &AppView,
    remotes: Vec<RemoteInfo>,
    selected_remote: String,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let t = view.t();
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .font_semibold()
                .text_left()
                .child(t.remotes),
        )
        .child(
            div()
                .id("remote-list")
                .flex_1()
                .min_h_0()
                .px_2()
                .pb_2()
                .gap_1()
                .flex()
                .flex_col()
                .items_start()
                .overflow_y_scroll()
                .children(remotes.into_iter().map(|r| {
                    let name = r.name.clone();
                    let selected = name == selected_remote;
                    let text = match r.url {
                        Some(url) => format!("{} — {}", r.name, url),
                        None => r.name.clone(),
                    };
                    div()
                        .id(SharedString::from(format!("remote-row:{}", r.name)))
                        .w_full()
                        .rounded(cx.theme().radius)
                        .when(selected, |this| this.bg(cx.theme().accent.opacity(0.28)))
                        .child(
                            display_btn(SharedString::from(format!("remote:{}", r.name)))
                                .ghost()
                                .w_full()
                                .label(text)
                                .tooltip(t.tip_select_remote)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_remote(name.clone(), cx);
                                })),
                        )
                        .into_any_element()
                })),
        )
        .into_any_element()
}

fn branches_panel(
    view: &AppView,
    branches: Vec<BranchInfo>,
    form: BranchForm,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let t = view.t();
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .py_2()
                .child(div().font_semibold().text_left().child(t.branches))
                .child(
                    icon_btn("branch-new", IconName::Plus)
                        .accessibility_label(t.create_switch)
                        .tooltip(t.tip_new_branch)
                        .selected(matches!(form, BranchForm::Create))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_create_branch_form(window, cx);
                        })),
                ),
        )
        .when(!matches!(form, BranchForm::Hidden), |this| {
            this.child(branch_form_panel(view, &form, cx))
        })
        .child(
            pane_body(cx)
                .id("branch-list")
                .flex_1()
                .min_h_0()
                .p_2()
                .gap_1()
                .flex()
                .flex_col()
                .items_start()
                .overflow_y_scroll()
                .children(branches.into_iter().map(|b| branch_row(b, view, cx))),
        )
        .into_any_element()
}

fn branch_form_panel(
    view: &AppView,
    form: &BranchForm,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let title = match form {
        BranchForm::Create => t.create_switch,
        BranchForm::Rename { from } => from.as_str(),
        BranchForm::Hidden => "",
    };

    pane_toolbar(cx)
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(title.to_string()),
        )
        .child(Input::new(&view.branch_input))
        .child(
            action_row()
                .child(
                    action_btn("branch-form-cancel", AssetIcon::Close)
                        .ghost()
                        .small()
                        .label(t.branch_cancel)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_branch_form(window, cx);
                        })),
                )
                .child({
                    let busy_kind = match &view.branch_form {
                        BranchForm::Rename { .. } => BusyKind::RenameBranch,
                        _ => BusyKind::CreateBranch,
                    };
                    let icon = match &view.branch_form {
                        BranchForm::Rename { .. } => AssetIcon::Pencil,
                        _ => AssetIcon::GitBranchPlus,
                    };
                    with_busy(
                        confirm_btn("branch-form-confirm", icon, cx)
                            .small()
                            .label(t.branch_confirm)
                            .tooltip(t.tip_create_branch)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_branch_form(window, cx);
                            })),
                        view.busy,
                        busy_kind,
                    )
                }),
        )
}

fn branch_row(
    branch: BranchInfo,
    view: &AppView,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let t = view.t();
    let name = branch.name.clone();
    let name_switch = name.clone();
    let name_rename = name.clone();
    let name_delete = name.clone();
    let label = if branch.is_head {
        format!("* {}", branch.name)
    } else if branch.is_remote {
        format!("  remotes/{}", branch.name)
    } else {
        format!("  {}", branch.name)
    };
    let can_switch = !branch.is_remote && !branch.is_head;
    let can_rename = !branch.is_remote;
    let can_delete = !branch.is_remote && !branch.is_head;

    div()
        .id(SharedString::from(format!("branch-row:{}", branch.name)))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_1()
        .child(
            display_btn(SharedString::from(format!("branch:{}", branch.name)))
                .ghost()
                .flex_1()
                .min_w_0()
                .justify_start()
                .label(label)
                .tooltip(t.tip_switch_branch)
                .disabled(!can_switch)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.confirm_switch_branch(name_switch.clone(), window, cx);
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_0()
                .when(can_rename, |this| {
                    this.child(
                        icon_btn(
                            SharedString::from(format!("branch-ren:{}", name)),
                            AssetIcon::Pencil,
                        )
                        .accessibility_label(t.tip_rename_branch)
                        .tooltip(t.tip_rename_branch)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.start_rename_branch(name_rename.clone(), window, cx);
                        })),
                    )
                })
                .when(can_delete, |this| {
                    this.child(
                        icon_danger_btn(
                            SharedString::from(format!("branch-del:{}", name)),
                            AssetIcon::Trash,
                        )
                        .accessibility_label(t.tip_delete_branch)
                        .tooltip(t.tip_delete_branch)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_delete_branch(name_delete.clone(), window, cx);
                        })),
                    )
                }),
        )
        .into_any_element()
}
