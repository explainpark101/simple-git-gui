use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Hsla};

/// Human-readable status label shown in the Changes list.
pub fn display_label(status: &str) -> &'static str {
    match status {
        "modified" => "Updated",
        "added" => "Added",
        "deleted" => "Deleted",
        "renamed" => "Renamed",
        "untracked" => "Untracked",
        "conflict" => "Conflict",
        _ => "Changed",
    }
}

pub fn status_color(status: &str, cx: &App) -> Hsla {
    let theme = cx.theme();
    match status {
        "added" | "untracked" => theme.success,
        "modified" => theme.warning,
        "deleted" | "conflict" => theme.danger,
        "renamed" => theme.info,
        _ => theme.muted_foreground,
    }
}

pub fn area_label(kind: sgg_core::ChangeKind) -> &'static str {
    match kind {
        sgg_core::ChangeKind::Staged => "Staged",
        sgg_core::ChangeKind::Unstaged => "Unstaged",
        sgg_core::ChangeKind::Untracked => "Untracked",
        sgg_core::ChangeKind::Conflicted => "Conflict",
    }
}