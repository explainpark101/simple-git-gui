use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Editor;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::busy::BusyKind;
use crate::app::view::AppView;
use crate::app::widgets::{action_row, confirm_btn, icon_btn, pane_body, pane_header, with_busy};

pub fn render(
    view: &AppView,
    show_close: bool,
    show_tabs: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let has_path = view.preview_path.is_some();
    let selected_tab = view.preview_tab_index;
    let tab_count = view.preview_tabs.len();

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_w_0()
        .min_h_0()
        .when(show_tabs && tab_count > 0, |this| {
            // Labels only — do not clone tab file buffers every paint.
            let tab_labels: Vec<(usize, String)> = view
                .preview_tabs
                .iter()
                .enumerate()
                .map(|(idx, tab)| {
                    let label = tab
                        .path
                        .rsplit('/')
                        .next()
                        .unwrap_or(tab.path.as_str())
                        .to_string();
                    (idx, label)
                })
                .collect();
            this.child(
                TabBar::new("preview-tabs")
                    .underline()
                    .w_full()
                    .px_2()
                    .selected_index(selected_tab)
                    .on_click(cx.listener(|this, index: &usize, window, cx| {
                        this.select_preview_tab(*index, window, cx);
                    }))
                    .children(tab_labels.into_iter().map(|(idx, label)| {
                        Tab::new()
                            .label(label)
                            .selected(idx == selected_tab)
                            .on_mouse_down(
                                MouseButton::Middle,
                                cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.close_preview_tab(idx, window, cx);
                                }),
                            )
                            .suffix(
                                icon_btn(
                                    SharedString::from(format!("preview-tab-close:{idx}")),
                                    AssetIcon::Close,
                                )
                                .small()
                                .compact()
                                .ghost()
                                .accessibility_label(t.close_preview)
                                .tooltip(t.tip_close_preview)
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.close_preview_tab(idx, window, cx);
                                })),
                            )
                    })),
            )
        })
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .py_2()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .when_some(view.preview_path.clone(), |this, path| this.child(path)),
                )
                .child(
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
                        .when(show_close || show_tabs, |this| {
                            this.child(
                                icon_btn("preview-close", AssetIcon::Close)
                                    .accessibility_label(t.close_preview)
                                    .tooltip(t.tip_close_preview)
                                    .disabled(!has_path)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.close_preview(window, cx)
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
                .child(Editor::new(&view.preview_input).h(relative(1.))),
        )
}
