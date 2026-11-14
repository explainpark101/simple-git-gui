mod editor;
mod files;
mod i18n;
mod ops;
mod page;
mod prefs;
mod render;
mod sort;
mod view;
mod widgets;

use std::path::PathBuf;

use gpui_kit::component::{Theme, scroll::ScrollbarMode};
use gpui_kit::*;

pub use view::AppView;

use prefs::absolute_path;

pub fn run(start: PathBuf) {
    let start = absolute_path(start);
    application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            init(cx);
            Theme::set_scrollbar_mode(ScrollbarMode::Always, cx);

            let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("simple-git-gui".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            open_window(options, cx, move |window, cx| {
                cx.new(|cx| AppView::new(start.clone(), window, cx))
            })
            .expect("open window");
        });
}