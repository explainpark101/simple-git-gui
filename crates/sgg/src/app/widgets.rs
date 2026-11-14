//! Shared UI helpers.
//!
//! Display buttons = left-aligned lists/selection.
//! Action buttons = centered label text + leading icon; rows right-aligned via `action_row`.
//! See `.cursor/rules/sgg-button-alignment.mdc`.

use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{App, Div, ElementId, Styled as _, div, px};

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

/// Display / list / selection button (left-aligned content).
pub fn display_btn(id: impl Into<ElementId>) -> Button {
    btn(id).justify_start()
}

/// Action / command / submit button (centered label, leading icon).
pub fn action_btn(id: impl Into<ElementId>, icon: impl Into<Icon>) -> Button {
    btn(id).justify_center().icon(icon)
}

/// Raised stage-family action: solid secondary surface + shadow so it lifts off the strip.
pub fn raised_action_btn(
    id: impl Into<ElementId>,
    icon: impl Into<Icon>,
    cx: &App,
) -> Button {
    action_btn(id, icon).custom(
        ButtonCustomVariant::new(cx)
            .color(cx.theme().secondary)
            .foreground(cx.theme().secondary_foreground)
            .hover(cx.theme().secondary_hover)
            .active(cx.theme().secondary_active)
            .shadow(true),
    )
}

/// Confirm / OK action: filled primary + shadow with explicit foreground for icon contrast.
pub fn confirm_btn(id: impl Into<ElementId>, icon: impl Into<Icon>, cx: &App) -> Button {
    action_btn(id, icon).custom(
        ButtonCustomVariant::new(cx)
            .color(cx.theme().primary)
            .foreground(cx.theme().primary_foreground)
            .hover(cx.theme().primary_hover)
            .active(cx.theme().primary_active)
            .shadow(true),
    )
}

/// Apply loading spinner when `kind` matches the current busy state.
pub fn with_busy(btn: Button, busy: Option<BusyKind>, kind: BusyKind) -> Button {
    btn.loading(busy == Some(kind))
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
