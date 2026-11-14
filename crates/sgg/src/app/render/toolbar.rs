use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _};
use gpui_kit::*;

use crate::app::page::AppPage;
use crate::app::view::AppView;
use crate::app::widgets::btn;

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();

    div()
        .flex()
        .items_center()
        .gap_1()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            btn("history-back")
                .ghost()
                .small()
                .label(t.history_back)
                .tooltip(t.tip_history_back)
                .disabled(!view.can_go_back())
                .on_click(cx.listener(|this, _, _, cx| this.history_back(cx))),
        )
        .child(
            btn("history-forward")
                .ghost()
                .small()
                .label(t.history_forward)
                .tooltip(t.tip_history_forward)
                .disabled(!view.can_go_forward())
                .on_click(cx.listener(|this, _, _, cx| this.history_forward(cx))),
        )
        .child(
            btn("tab-git")
                .ghost()
                .small()
                .label(t.tab_git)
                .selected(view.page == AppPage::Git)
                .tooltip(t.tip_tab_git)
                .on_click(cx.listener(|this, _, _, cx| this.set_page(AppPage::Git, cx))),
        )
        .child(
            btn("tab-files")
                .ghost()
                .small()
                .label(t.tab_files)
                .selected(view.page == AppPage::Files)
                .tooltip(t.tip_tab_files)
                .on_click(cx.listener(|this, _, _, cx| this.set_page(AppPage::Files, cx))),
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
        )
}