use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{Disableable as _, Selectable as _, Sizable as _};
use gpui_kit::*;

use crate::app::busy::BusyKind;
use crate::app::page::AppPage;
use crate::app::view::AppView;
use crate::app::widgets::{action_btn, btn, chrome_bar, with_busy};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();

    chrome_bar(cx)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    btn("history-back")
                        .ghost()
                        .small()
                        .label(t.history_back)
                        .tooltip(t.tip_history_back)
                        .disabled(!view.can_go_back())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.history_back(window, cx)
                        })),
                )
                .child(
                    btn("history-forward")
                        .ghost()
                        .small()
                        .label(t.history_forward)
                        .tooltip(t.tip_history_forward)
                        .disabled(!view.can_go_forward())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.history_forward(window, cx)
                        })),
                )
                .child(
                    btn("tab-git")
                        .ghost()
                        .small()
                        .label(t.tab_git)
                        .selected(view.page == AppPage::Git)
                        .tooltip(t.tip_tab_git)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_page(AppPage::Git, window, cx)
                        })),
                )
                .child(
                    btn("tab-history")
                        .ghost()
                        .small()
                        .label(t.tab_history)
                        .selected(view.page == AppPage::History)
                        .tooltip(t.tip_tab_history)
                        .disabled(!view.has_repo())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_page(AppPage::History, window, cx);
                            this.reload_history(window, cx);
                        })),
                )
                .child(
                    btn("tab-prs")
                        .ghost()
                        .small()
                        .label(t.tab_prs)
                        .selected(view.page == AppPage::PullRequests)
                        .tooltip(t.tip_tab_prs)
                        .disabled(!view.has_repo())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_page(AppPage::PullRequests, window, cx);
                            this.refresh_pull_requests(cx);
                        })),
                )
                .child(
                    btn("tab-actions")
                        .ghost()
                        .small()
                        .label(t.tab_actions)
                        .selected(view.page == AppPage::Actions)
                        .tooltip(t.tip_tab_actions)
                        .disabled(!view.has_repo())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_page(AppPage::Actions, window, cx);
                            this.refresh_actions(cx);
                        })),
                )
                .child(
                    btn("tab-files")
                        .ghost()
                        .small()
                        .label(t.tab_files)
                        .selected(view.page == AppPage::Files)
                        .tooltip(t.tip_tab_files)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_page(AppPage::Files, window, cx)
                        })),
                )
                .child(
                    btn("tab-settings")
                        .ghost()
                        .small()
                        .label(t.tab_settings)
                        .selected(view.page == AppPage::Settings)
                        .tooltip(t.tip_tab_settings)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_page(AppPage::Settings, window, cx)
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    action_btn("toolbar-close-project", AssetIcon::Close)
                        .danger()
                        .small()
                        .label(t.close_project)
                        .tooltip(t.tip_close_project)
                        .disabled(!view.has_repo())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_project(window, cx)
                        })),
                )
                .child(
                    action_btn("toolbar-terminal", AssetIcon::SquareTerminal)
                        .outline()
                        .small()
                        .label(t.terminal)
                        .tooltip(t.tip_toggle_terminal)
                        .selected(view.terminal.visible)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_terminal(window, cx)
                        })),
                )
                .child(
                    with_busy(
                        action_btn("toolbar-open-editor", AssetIcon::ExternalLink)
                            .outline()
                            .small()
                            .label(format!(
                                "{} {}",
                                t.open_project_in,
                                view.preferred_editor.label()
                            ))
                            .tooltip(t.tip_open_project)
                            .disabled(!view.has_repo())
                            .on_click(cx.listener(|this, _, window, cx| {
                                // Always open the repository root (project), never a single file.
                                this.open_project_in_editor_busy(window, cx)
                            })),
                        view.busy,
                        BusyKind::OpenProject,
                    ),
                ),
        )
}
