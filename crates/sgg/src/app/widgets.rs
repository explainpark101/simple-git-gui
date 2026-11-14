//! Shared UI helpers.

use gpui_kit::component::button::Button;
use gpui_kit::{ElementId, Styled as _};

/// Create a button that always uses the pointing-hand cursor.
pub fn btn(id: impl Into<ElementId>) -> Button {
    Button::new(id).cursor_pointer()
}