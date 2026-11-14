mod changes;
mod files_page;
mod preview_pane;
mod settings;
mod sidebar;
mod status_style;
mod toolbar;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::{h_resizable, resizable_panel};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::page::AppPage;
use super::view::AppView;
use crate::app::{HistoryBack, HistoryForward, SavePreview};

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let changes = self
            .snapshot
            .as_ref()
            .map(|s| s.changes.clone())
            .unwrap_or_default();
        let branches = self
            .snapshot
            .as_ref()
            .map(|s| s.branches.clone())
            .unwrap_or_default();
        let remotes = self
            .snapshot
            .as_ref()
            .map(|s| s.remotes.clone())
            .unwrap_or_default();
        let focus = self.focus_handle(cx);
        let preview_open = self.preview_path.is_some();
        let sidebar_collapsed = self.sidebar_collapsed;

        div()
            .id("sgg-root")
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&focus)
            .key_context("AppView")
            .tab_index(0)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &HistoryBack, window, cx| {
                this.history_back(window, cx);
            }))
            .on_action(cx.listener(|this, _: &HistoryForward, window, cx| {
                this.history_forward(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SavePreview, window, cx| {
                this.save_preview(window, cx);
            }))
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, window, cx| this.history_back(window, cx)),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, window, cx| this.history_forward(window, cx)),
            )
            .child(toolbar::render(self, cx))
            .when_some(self.error.clone(), |this, err| {
                this.child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(cx.theme().danger)
                        .text_color(cx.theme().background)
                        .text_sm()
                        .text_left()
                        .child(err),
                )
            })
            .child(
                div().flex_1().min_h_0().w_full().child(match self.page {
                    AppPage::Git => {
                        // Changes min: wide enough that sort + stage-actions dropdown
                        // do not overlap when the preview pane is open.
                        // Sidebar min: remote header (fetch/pull/push + collapse) stays usable.
                        let work = if preview_open {
                            h_resizable("git-work")
                                .child(
                                    resizable_panel()
                                        .size(px(360.))
                                        .size_range(px(340.)..px(560.))
                                        .child(
                                            changes::render(self, changes, cx)
                                                .into_any_element(),
                                        ),
                                )
                                .child(
                                    resizable_panel().child(
                                        preview_pane::render(self, true, cx)
                                            .into_any_element(),
                                    ),
                                )
                                .into_any_element()
                        } else {
                            changes::render(self, changes, cx).into_any_element()
                        };

                        h_resizable("git-panes")
                            .child(
                                resizable_panel()
                                    .size(if sidebar_collapsed {
                                        px(48.)
                                    } else {
                                        px(300.)
                                    })
                                    .size_range(if sidebar_collapsed {
                                        px(40.)..px(56.)
                                    } else {
                                        px(280.)..px(480.)
                                    })
                                    .child(
                                        sidebar::render(self, branches, remotes, cx),
                                    ),
                            )
                            .child(resizable_panel().child(work))
                            .into_any_element()
                    }
                    AppPage::Files => files_page::render(self, cx).into_any_element(),
                    AppPage::Settings => settings::render(self, cx).into_any_element(),
                }),
            )
    }
}
