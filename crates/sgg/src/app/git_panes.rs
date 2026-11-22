//! Git page child entities so AppView `notify` does not rebuild every pane.

use gpui_kit::*;

use super::render::{changes, preview_pane, sidebar};
use super::view::AppView;

pub struct GitSidebarPane {
    pub app: WeakEntity<AppView>,
}

pub struct ChangesPane {
    pub app: WeakEntity<AppView>,
}

pub struct PreviewPaneHost {
    pub app: WeakEntity<AppView>,
    pub show_close: bool,
    pub show_tabs: bool,
}

pub struct CommitComposer {
    pub app: WeakEntity<AppView>,
}

pub(crate) fn app_click(
    app: Entity<AppView>,
    f: impl Fn(&mut AppView, &mut Context<AppView>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, _, cx| {
        app.update(cx, |this, cx| f(this, cx));
    }
}

pub(crate) fn app_click_window(
    app: Entity<AppView>,
    f: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_, window, cx| {
        app.update(cx, |this, cx| f(this, window, cx));
    }
}

pub(crate) fn app_toggle(
    app: Entity<AppView>,
    f: impl Fn(&mut AppView, bool, &mut Context<AppView>) + 'static,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    move |checked, _, cx| {
        app.update(cx, |this, cx| f(this, *checked, cx));
    }
}

fn upgrade_app(app: &WeakEntity<AppView>) -> Option<Entity<AppView>> {
    app.upgrade()
}

impl Render for GitSidebarPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = upgrade_app(&self.app) else {
            return div().into_any_element();
        };
        sidebar::render(app, cx).into_any_element()
    }
}

impl Render for ChangesPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = upgrade_app(&self.app) else {
            return div().into_any_element();
        };
        let composer = app.read(cx).commit_composer.clone();
        changes::render(app, composer, cx).into_any_element()
    }
}

impl Render for PreviewPaneHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = upgrade_app(&self.app) else {
            return div().into_any_element();
        };
        preview_pane::render(app, self.show_close, self.show_tabs, cx).into_any_element()
    }
}

impl Render for CommitComposer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = upgrade_app(&self.app) else {
            return div().into_any_element();
        };
        changes::render_commit_composer(app, cx).into_any_element()
    }
}