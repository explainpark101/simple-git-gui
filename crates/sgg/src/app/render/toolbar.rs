use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _};
use gpui_kit::*;

use crate::app::ops::RemoteAction;
use crate::app::page::AppPage;
use crate::app::view::AppView;
use crate::app::widgets::btn;

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();

    div()
        .flex()
        .flex_col()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .px_3()
                .pt_2()
                .pb_1()
                .child(
                    btn("tab-git")
                        .ghost()
                        .small()
                        .label(t.tab_git)
                        .selected(view.page == AppPage::Git)
                        .tooltip(t.tip_tab_git)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_page(AppPage::Git, cx)
                        })),
                )
                .child(
                    btn("tab-files")
                        .ghost()
                        .small()
                        .label(t.tab_files)
                        .selected(view.page == AppPage::Files)
                        .tooltip(t.tip_tab_files)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_page(AppPage::Files, cx)
                        })),
                )
                .child(
                    btn("tab-settings")
                        .ghost()
                        .small()
                        .label(t.tab_settings)
                        .selected(view.page == AppPage::Settings)
                        .tooltip(t.tip_tab_settings)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_page(AppPage::Settings, cx)
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_4()
                .py_2()
                .child(div().text_sm().font_semibold().child("sgg"))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .text_left()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(view.status_line.clone()),
                )
                .child(
                    btn("refresh")
                        .outline()
                        .small()
                        .label(t.refresh)
                        .tooltip(t.tip_refresh)
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                )
                .child(
                    btn("fetch")
                        .outline()
                        .small()
                        .label(t.fetch)
                        .tooltip(t.tip_fetch)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_remote_action(RemoteAction::Fetch, window, cx)
                        })),
                )
                .child(
                    btn("pull")
                        .outline()
                        .small()
                        .label(t.pull)
                        .tooltip(t.tip_pull)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_remote_action(RemoteAction::Pull, window, cx)
                        })),
                )
                .child(
                    btn("push")
                        .primary()
                        .small()
                        .label(t.push)
                        .tooltip(t.tip_push)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_remote_action(RemoteAction::Push, window, cx)
                        })),
                ),
        )
}