mod busy;
mod dialogs;
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

gpui_kit::actions!(sgg, [HistoryBack, HistoryForward, SavePreview]);

pub fn run(start: PathBuf) {
    let start = absolute_path(start);
    // Full Lucide catalog (Pencil, Trash, FolderInput, …). Default `Assets`
    // only embeds the ~100 component icons, so many AssetIcon variants render blank.
    application()
        .with_assets(assets::AllAssets)
        .run(move |cx| {
            init(cx);
            Theme::set_scrollbar_mode(ScrollbarMode::Always, cx);

            cx.bind_keys([
                KeyBinding::new("alt-left", HistoryBack, Some("AppView")),
                KeyBinding::new("alt-right", HistoryForward, Some("AppView")),
                // Browser-like shortcuts on some layouts
                KeyBinding::new("ctrl-[", HistoryBack, Some("AppView")),
                KeyBinding::new("ctrl-]", HistoryForward, Some("AppView")),
                KeyBinding::new("ctrl-s", SavePreview, Some("AppView")),
            ]);

            let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("simple-git-gui".into()),
                    ..Default::default()
                }),
                // Prefer full refresh while resizing / interacting (DirectX GPU + Direct Composition).
                inactive_frame_interval: None,
                low_power_frame_interval: None,
                thermal_frame_interval: None,
                ..Default::default()
            };

            open_window(options, cx, move |window, cx| {
                cx.new(|cx| AppView::new(start.clone(), window, cx))
            })
            .expect("open window");
        });
}