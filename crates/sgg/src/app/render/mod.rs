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

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
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