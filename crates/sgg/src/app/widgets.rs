//! Shared UI helpers.
//!
//! Display buttons = left-aligned lists/selection.
//! Action buttons = centered command/submit controls.
//! See `.cursor/rules/sgg-button-alignment.mdc`.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{IconName, Sizable as _};
use gpui_kit::{ElementId, Styled as _};

/// Create a button that always uses the pointing-hand cursor.
pub fn btn(id: impl Into<ElementId>) -> Button {
    Button::new(id).cursor_pointer()
}

/// Display / list / selection button (left-aligned content).
pub fn display_btn(id: impl Into<ElementId>) -> Button {
    btn(id).justify_start()
}

/// Action / command / submit button (centered content).
pub fn action_btn(id: impl Into<ElementId>) -> Button {
    btn(id).justify_center()
}

/// Compact icon-only ghost button (tooltip should describe the action).
pub fn icon_btn(id: impl Into<ElementId>, icon: IconName) -> Button {
    btn(id).ghost().small().compact().icon(icon)
}

/// Compact icon-only danger button.
pub fn icon_danger_btn(id: impl Into<ElementId>, icon: IconName) -> Button {
    btn(id).danger().small().compact().icon(icon)
}