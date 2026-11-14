mod changes;
mod files_page;
mod settings;
mod sidebar;
mod status_style;
mod toolbar;

use gpui_kit::component::ActiveTheme as _;
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
            .on_action(cx.listener(|this, _: &HistoryBack, _, cx| {
                this.history_back(cx);
            }))
            .on_action(cx.listener(|this, _: &HistoryForward, _, cx| {
                this.history_forward(cx);
            }))
            .on_action(cx.listener(|this, _: &SavePreview, _, cx| {
                this.save_preview(cx);
            }))
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, _, cx| this.history_back(cx)),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, _, cx| this.history_forward(cx)),
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
            .child(match self.page {
                AppPage::Git => div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(changes::render(self, changes, cx))
                    .child(sidebar::render(self, branches, remotes, cx))
                    .into_any_element(),
                AppPage::Files => files_page::render(self, cx).into_any_element(),
                AppPage::Settings => settings::render(self, cx).into_any_element(),
            })
    }
}