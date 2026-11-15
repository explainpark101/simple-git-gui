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
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
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
                        action_btn("toolbar-open-folder", AssetIcon::FolderOpen)
                            .outline()
                            .small()
                            .label(t.open_other_project)
                            .tooltip(t.tip_open_other_project)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_folder(window, cx)
                            })),
                        view.busy,
                        BusyKind::OpenFolder,
                    ),
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
