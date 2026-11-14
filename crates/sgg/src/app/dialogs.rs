//! Confirm dialogs with pointer cursors on footer buttons.

use std::rc::Rc;

use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::*;

use super::widgets::btn;

/// Open a confirm alert whose OK/Cancel buttons use `cursor: pointer`.
pub fn open_confirm(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    cancel_text: impl Into<SharedString>,
    danger: bool,
    on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
) {
    let title = title.into();
    let description = description.into();
    let ok_text = ok_text.into();
    let cancel_text = cancel_text.into();
    let on_ok = Rc::new(on_ok);

    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_ok = on_ok.clone();
        let mut ok = btn("alert-ok").label(ok_text.clone());
        ok = if danger {
            ok.danger()
        } else {
            ok.primary()
        };
        ok = ok.on_click(move |ev, window, cx| {
            if on_ok(ev, window, cx) {
                window.close_dialog(cx);
            }
        });

        alert
            .title(title.clone())
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().trigger(|b| {
                            b.outline()
                                .cursor_pointer()
                                .label(cancel_text.clone())
                        }),
                    )
                    .child(ok),
            )
    });
}
