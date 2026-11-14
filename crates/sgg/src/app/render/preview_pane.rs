use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::busy::BusyKind;
use crate::app::view::AppView;
use crate::app::widgets::{action_row, confirm_btn, icon_btn, pane_body, pane_header, with_busy};

pub fn render(
    view: &AppView,
    show_close: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let has_path = view.preview_path.is_some();

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_w_0()
        .min_h_0()
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .px_3()
                .py_2()
                .child(
                    // All preview actions: right-aligned, order Save → External → Close.
                    action_row()
                        .child(
                            with_busy(
                                confirm_btn("preview-save", AssetIcon::Save, cx)
                                    .small()
                                    .label(t.save)
                                    .tooltip(t.tip_save_preview)
                                    .disabled(!has_path)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.save_preview(window, cx)
                                    })),
                                view.busy,
                                BusyKind::SavePreview,
                            ),
                        )
                        .when_some(view.preview_path.clone(), |this, path| {
                            this.child(
                                icon_btn("preview-open-ext", AssetIcon::ExternalLink)
                                    .outline()
                                    .accessibility_label(t.open_external)
                                    .tooltip(t.tip_open_external)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_path_in_editor(&path, cx);
                                    })),
                            )
                        })
                        .when(show_close, |this| {
                            this.child(
                                icon_btn("preview-close", AssetIcon::Close)
                                    .accessibility_label(t.close_preview)
                                    .tooltip(t.tip_close_preview)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_preview(cx)
                                    })),
                            )
                        }),
                ),
        )
        .child(
            pane_body(cx)
                .flex_1()
                .min_h_0()
                .p_2()
                .child(Textarea::new(&view.preview_input).h(relative(1.))),
        )
}
