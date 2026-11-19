mod ai_provider;
mod busy;
mod cli_tools;
mod commit_ai;
mod dialogs;
mod editor;
mod files;
mod i18n;
mod ops;
mod ops_vcs;
mod page;
mod perf;
mod prefs;
mod progress;
mod reload;
mod remote_url;
mod render;
mod sort;
mod t3_bridge;
mod terminal;
mod terminal_shell;
pub(crate) mod update;
mod view;
mod widgets;

use std::path::PathBuf;

use gpui_kit::component::{Theme, scroll::ScrollbarMode};
use gpui_kit::*;

pub use view::AppView;

use prefs::absolute_path;

gpui_kit::actions!(
    sgg,
    [
        HistoryBack,
        HistoryForward,
        SavePreview,
        ClosePreview,
        ToggleTerminal,
        NewTerminalTab,
    ]
);

pub fn run(start: Option<PathBuf>) {
    let start = start.map(absolute_path);
    // DMG / Applications install: register `sgg` on PATH (symlink) when launched
    // from a .app bundle. Failures are non-fatal so the GUI still opens.
    #[cfg(target_os = "macos")]
    {
        if let Err(err) = crate::shell::ensure_cli_path_from_bundle() {
            eprintln!("sgg: could not register CLI on PATH: {err:#}");
        }
    }
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
                KeyBinding::new("ctrl-w", ClosePreview, Some("AppView")),
                // Terminal: Ctrl+` / Ctrl+~ toggle; Ctrl+Shift+` / Ctrl+Shift+~ new tab
                // (same physical key as VS Code / ROADMAP ctrl ~ / ctrl shift ~).
                KeyBinding::new("ctrl-`", ToggleTerminal, Some("AppView")),
                KeyBinding::new("ctrl-~", ToggleTerminal, Some("AppView")),
                KeyBinding::new("ctrl-shift-`", NewTerminalTab, Some("AppView")),
                KeyBinding::new("ctrl-shift-~", NewTerminalTab, Some("AppView")),
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