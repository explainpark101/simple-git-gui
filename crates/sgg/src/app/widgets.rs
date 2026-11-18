//! Shared UI helpers.
//!
//! Display buttons = left-aligned lists/selection.
//! Action buttons = centered label text + leading icon; rows right-aligned via `action_row`.
//! See `.cursor/rules/sgg-button-alignment.mdc`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, ClickEvent, Div, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Styled as _, Window, div, px, relative,
};

use super::busy::BusyKind;

/// App top chrome (tab bar): title-bar fill + thicker edge.
pub fn chrome_bar(cx: &App) -> Div {
    div()
        .bg(cx.theme().title_bar)
        .border_b(px(2.))
        .border_color(cx.theme().title_bar_border)
}

/// Panel section header (title row): list-head fill + thicker edge.
pub fn pane_header(cx: &App) -> Div {
    div()
        .bg(cx.theme().list_head)
        .border_b(px(2.))
        .border_color(cx.theme().sidebar_border)
}

/// Secondary strip under a header (commit box, action buttons, filters).
pub fn pane_toolbar(cx: &App) -> Div {
    div()
        .bg(cx.theme().secondary)
        .border_b_1()
        .border_color(cx.theme().border)
}

/// Scrollable list / content body.
pub fn pane_body(cx: &App) -> Div {
    // `Theme::list` is ListSettings; the list fill lives on `colors`.
    div().bg(cx.theme().colors.list)
}

/// Create a button that always uses the pointing-hand cursor.
pub fn btn(id: impl Into<ElementId>) -> Button {
    Button::new(id).cursor_pointer()
}

/// Display / list / selection button (left-aligned outer flex).
///
/// Note: gpui-kit `Button` still centers its inner label row. For full-width
/// list rows prefer [`list_row_btn`], which left-aligns via a full-width child.
pub fn display_btn(id: impl Into<ElementId>) -> Button {
    btn(id).justify_start()
}

/// Full-width (or flex) list/selection row with left-aligned text.
///
/// `Button` paints an inner `h_flex().size_full().justify_center()` around
/// `.label(...)`, so labels stay centered even after `.justify_start()`. A
/// full-width child with `text_left` is the reliable left-align path.
pub fn list_row_btn(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    let label = label.into();
    btn(id)
        .ghost()
        .justify_start()
        .accessibility_label(label.clone())
        .child(
            div()
                .w_full()
                .min_w_0()
                .text_left()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(label),
        )
}

/// Action / command / submit button (centered label, leading icon).
pub fn action_btn(id: impl Into<ElementId>, icon: impl Into<Icon>) -> Button {
    btn(id).justify_center().icon(icon)
}

/// Raised stage-family action: solid secondary fill (not Custom — Custom idle is dimmed).
pub fn raised_action_btn(
    id: impl Into<ElementId>,
    icon: impl Into<Icon>,
    _cx: &App,
) -> Button {
    action_btn(id, icon).secondary()
}

/// Confirm / OK action: solid primary fill.
///
/// Do not use `ButtonCustomVariant` for filled primaries: gpui-kit mixes the
/// custom idle background with transparent (`mix_oklab(..., 0.2)`), so the
/// button looks dimmed until hover (which uses the full `hover` color).
pub fn confirm_btn(id: impl Into<ElementId>, icon: impl Into<Icon>, _cx: &App) -> Button {
    action_btn(id, icon).primary()
}

/// Apply loading spinner when `kind` matches the current busy state.
pub fn with_busy(btn: Button, busy: Option<BusyKind>, kind: BusyKind) -> Button {
    btn.loading(busy == Some(kind))
}

/// Busy button with a bottom-edge progress fill (0.0..=1.0).
///
/// gpui-kit `Button` has no built-in progress API — only a spinner via
/// [`with_busy`]. This wraps the button so the bottom border fills as
/// `progress` advances (e.g. CLI install download). When busy but
/// `progress` is `None`, shows an indeterminate mid-width pulse bar.
pub fn with_busy_bottom_progress(
    btn: Button,
    busy: Option<BusyKind>,
    kind: BusyKind,
    progress: Option<f32>,
    cx: &App,
) -> Div {
    let is_busy = busy == Some(kind);
    let btn = btn.loading(is_busy);
    let fill = if is_busy {
        progress.map(|p| p.clamp(0.0, 1.0)).unwrap_or(0.35)
    } else {
        0.0
    };
    let track = cx.theme().border;
    let bar = cx.theme().primary;
    div()
        .relative()
        .child(btn)
        .when(is_busy, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(2.))
                    .bg(track)
                    .child(div().h_full().w(relative(fill)).bg(bar)),
            )
        })
}

/// Container for action buttons: right-aligned row.
pub fn action_row() -> Div {
    div().flex().items_center().justify_end().gap_2()
}

/// Compact icon-only ghost button (tooltip should describe the action).
pub fn icon_btn(id: impl Into<ElementId>, icon: impl Into<Icon>) -> Button {
    btn(id).ghost().small().compact().icon(icon)
}

/// Compact icon-only danger button.
pub fn icon_danger_btn(id: impl Into<ElementId>, icon: impl Into<Icon>) -> Button {
    btn(id).danger().small().compact().icon(icon)
}

/// Wrap a [`Switch`] so hover fills the track+label row and the cursor is a
/// pointer. Upstream gpui-kit Switch omits both.
pub fn hoverable_switch(
    key: impl Into<SharedString>,
    switch: Switch,
    cx: &App,
) -> impl IntoElement {
    let key = key.into();
    let hover_bg = cx.theme().muted.opacity(0.4);
    div()
        .id(SharedString::from(format!("sw-hover:{key}")))
        .flex()
        .items_center()
        .rounded(cx.theme().radius)
        .px_1()
        .py_0p5()
        .cursor_pointer()
        .hover(move |this| this.bg(hover_bg))
        .child(switch)
}

/// Popup / context-menu item with an explicit pointing-hand cursor.
///
/// Upstream `MenuItemElement` does not set `cursor_pointer` on clickable rows,
/// so plain `PopupMenuItem::new` keeps the default arrow. Wrap the label in an
/// element that opts into the pointer cursor.
pub fn menu_item(
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> PopupMenuItem {
    let label = label.into();
    let label_el = label.clone();
    PopupMenuItem::element(move |_, _| {
        div()
            .w_full()
            .cursor_pointer()
            .child(label_el.clone())
    })
    .on_click(on_click)
}

/// Like [`menu_item`], with a leading icon inside the pointer hit target.
pub fn menu_item_with_icon(
    label: impl Into<SharedString>,
    icon: impl Into<Icon>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> PopupMenuItem {
    let label = label.into();
    let icon = icon.into();
    let label_el = label.clone();
    let icon_el = icon.clone();
    PopupMenuItem::element(move |_, _| {
        h_flex()
            .w_full()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .child(icon_el.clone())
            .child(label_el.clone())
    })
    .on_click(on_click)
}
