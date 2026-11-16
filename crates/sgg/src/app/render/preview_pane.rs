use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::diff::Diff;
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, h_resizable,
    resizable_panel,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sgg_core::AlignedSplitView;

use crate::app::busy::BusyKind;
use crate::app::view::{AppView, HistoryDiffLayout, HistoryFileViewMode};
use crate::app::widgets::{
    action_row, confirm_btn, display_btn, icon_btn, menu_item, pane_body, pane_header, with_busy,
};

pub fn render(
    view: &AppView,
    show_close: bool,
    show_tabs: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let has_path = view.preview_path.is_some();
    let history_preview = view.history_file_preview.clone();
    let is_history = history_preview.is_some();
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
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_left()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .when_some(view.preview_path.clone(), |this, path| {
                                    this.child(path)
                                }),
                        )
                        .when_some(history_preview.clone(), |this, hp| {
                            let short: String = hp.oid.chars().take(7).collect();
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{short}")),
                            )
                        }),
                )
                .child(
                    action_row()
                        .when_some(history_preview.clone(), |this, hp| {
                            let mode_label = if hp.mode == HistoryFileViewMode::Diff {
                                t.history_view_diff
                            } else {
                                t.history_view_file
                            };
                            let mode = hp.mode;
                            let entity = cx.entity();
                            this.child(
                                display_btn("preview-hist-mode")
                                    .outline()
                                    .small()
                                    .label(format!("{mode_label}  ▾"))
                                    .tooltip(t.tip_history_view_mode)
                                    .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                                        let entity_diff = entity.clone();
                                        let entity_file = entity.clone();
                                        menu.item(
                                            menu_item(t.history_view_diff, move |_, window, cx| {
                                                entity_diff.update(cx, |this, cx| {
                                                    this.set_history_file_view_mode(
                                                        HistoryFileViewMode::Diff,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .checked(mode == HistoryFileViewMode::Diff),
                                        )
                                        .item(
                                            menu_item(t.history_view_file, move |_, window, cx| {
                                                entity_file.update(cx, |this, cx| {
                                                    this.set_history_file_view_mode(
                                                        HistoryFileViewMode::FileAtCommit,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .checked(mode == HistoryFileViewMode::FileAtCommit),
                                        )
                                    }),
                            )
                            .when(hp.mode == HistoryFileViewMode::Diff, |this| {
                                this.child(diff_layout_pill(hp.diff_layout, t, cx))
                                    .when(
                                        hp.diff_layout == HistoryDiffLayout::Split,
                                        |this| {
                                            this.child(
                                                Switch::new("preview-hist-scroll-sync")
                                                    .small()
                                                    .label(t.history_diff_scroll_sync)
                                                    .checked(view.history_diff_scroll_sync)
                                                    .tooltip(t.tip_history_diff_scroll_sync)
                                                    .on_click(cx.listener(|this, checked, _, cx| {
                                                        this.set_history_diff_scroll_sync(
                                                            *checked, cx,
                                                        );
                                                    })),
                                            )
                                        },
                                    )
                            })
                        })
                        .when(!is_history, |this| {
                            this.child(
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
                        })
                        .when(!is_history, |this| {
                            this.when_some(view.preview_path.clone(), |this, path| {
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
                        })
                        .when(show_close || show_tabs || is_history, |this| {
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
        .child({
            let hist = history_preview.as_ref();
            let unified_diff = hist.is_some_and(|hp| {
                hp.mode == HistoryFileViewMode::Diff
                    && hp.diff_layout == HistoryDiffLayout::Unified
            });
            let split_diff = hist.is_some_and(|hp| {
                hp.mode == HistoryFileViewMode::Diff
                    && hp.diff_layout == HistoryDiffLayout::Split
            });
            pane_body(cx)
                .flex_1()
                .min_h_0()
                .p_2()
                .child(if unified_diff {
                    Diff::new(&view.history_diff)
                        .header_visible(false)
                        .soft_wrap(false)
                        .h(relative(1.))
                        .flex_1()
                        .min_h_0()
                        .into_any_element()
                } else if split_diff {
                    let aligned = view.history_aligned_split.clone();
                    // Two independent editors + drag handle (pane-layout rule).
                    h_resizable("history-diff-split")
                        .child(
                            resizable_panel()
                                .size(px(360.))
                                .size_range(px(160.)..px(900.))
                                .child(split_diff_side(
                                    "old",
                                    t.history_diff_old,
                                    &view.preview_input_old,
                                    aligned.as_ref(),
                                    true,
                                    cx,
                                )),
                        )
                        .child(
                            resizable_panel().child(split_diff_side(
                                "new",
                                t.history_diff_new,
                                &view.preview_input,
                                aligned.as_ref(),
                                false,
                                cx,
                            )),
                        )
                        .into_any_element()
                } else {
                    div()
                        .relative()
                        .size_full()
                        .min_h_0()
                        .child(Editor::new(&view.preview_input).h(relative(1.)))
                        .children(inline_blame_overlay(view, cx))
                        .into_any_element()
                })
        })
}

/// Match `gpui_kit::component::input::editor::EDITOR_LINE_HEIGHT` (private).
const SPLIT_EDITOR_LINE_HEIGHT: f32 = 1.5;

/// One Split Diff pane: custom source-line gutter (not in the text) + Editor.
///
/// The Editor uses `.xsmall()` so vertical padding is 0 — otherwise the
/// gutter (no pad) drifts vs the text by `input_py` and looks worse down-file.
fn split_diff_side(
    id: &'static str,
    title: &'static str,
    editor: &Entity<EditorState>,
    aligned: Option<&AlignedSplitView>,
    old_side: bool,
    cx: &App,
) -> impl IntoElement {
    let scroll_y = editor.read(cx).scroll_offset().y;
    // Prefer the editor's laid-out row height; fall back to theme mono × 1.5.
    let line_height = editor.read(cx).line_height().unwrap_or_else(|| {
        px(f32::from(cx.theme().mono_font_size) * SPLIT_EDITOR_LINE_HEIGHT)
    });
    let digits = aligned.map(AlignedSplitView::line_number_digits).unwrap_or(3);
    // Digit advance + padding; keeps the gutter looking like an editor shell.
    let gutter_w = px(10. + digits as f32 * 8.5);
    let muted = cx.theme().muted_foreground;
    let border = cx.theme().border;
    let gutter_bg = cx.theme().secondary;
    let mono = cx.theme().mono_font_family.clone();
    let mono_size = cx.theme().mono_font_size;

    div()
        .size_full()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .px_2()
                .py_1()
                .text_xs()
                .text_color(muted)
                .child(title),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .child(
                    div()
                        .id(SharedString::from(format!("hist-split-gutter-{id}")))
                        .w(gutter_w)
                        .h_full()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .relative()
                        .border_r_1()
                        .border_color(border)
                        .bg(gutter_bg)
                        .children(aligned.map(|aligned| {
                            let lines = aligned.source_lines(old_side);
                            div()
                                .absolute()
                                .top(scroll_y)
                                .left_0()
                                .right_0()
                                .children(lines.iter().enumerate().map(|(i, line)| {
                                    let label = line
                                        .source_line
                                        .map(|n| format!("{n}"))
                                        .unwrap_or_default();
                                    div()
                                        .id(SharedString::from(format!(
                                            "hist-split-ln-{id}-{i}"
                                        )))
                                        .h(line_height)
                                        .px_1()
                                        .flex()
                                        .items_center()
                                        .justify_end()
                                        .font_family(mono.clone())
                                        .text_size(mono_size)
                                        .line_height(relative(SPLIT_EDITOR_LINE_HEIGHT))
                                        .text_color(muted)
                                        .child(label)
                                }))
                        })),
                )
                .child(
                    // xsmall → py 0 so row tops match the gutter strip.
                    Editor::new(editor)
                        .xsmall()
                        .h(relative(1.))
                        .flex_1()
                        .min_w_0(),
                ),
        )
}

/// Icon-only unified/split radio pill (labels live in tooltips).
fn diff_layout_pill(
    layout: HistoryDiffLayout,
    t: crate::app::i18n::UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .flex()
        .items_stretch()
        .rounded_full()
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .child(
            icon_btn("preview-hist-unified", AssetIcon::FileDiff)
                .selected(layout == HistoryDiffLayout::Unified)
                .accessibility_label(t.history_diff_unified)
                .tooltip(t.tip_history_diff_unified)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.set_history_diff_layout(HistoryDiffLayout::Unified, window, cx);
                })),
        )
        .child(
            div()
                .w(px(1.))
                .self_stretch()
                .bg(cx.theme().border),
        )
        .child(
            icon_btn("preview-hist-split", AssetIcon::Split)
                .selected(layout == HistoryDiffLayout::Split)
                .accessibility_label(t.history_diff_split)
                .tooltip(t.tip_history_diff_split)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.set_history_diff_layout(HistoryDiffLayout::Split, window, cx);
                })),
        )
}

/// Caret-line inline blame (same row as the text under the caret).
fn inline_blame_overlay(view: &AppView, cx: &App) -> Option<AnyElement> {
    if view.history_file_preview.is_some() {
        return None;
    }
    if !view.show_inline_blame || view.preview_path.is_none() {
        return None;
    }

    let editor = view.preview_input.read(cx);
    let line = editor.cursor_position().line as usize;
    let entry = view.blame_lines.get(line)?.as_ref()?;
    let (cursor, line_height) = editor.cursor_layout()?;
    let scroll = editor.scroll_offset();
    let input = editor.input_bounds();

    // cursor_layout Y is unscrolled; painter applies scroll. X already includes it.
    let visible_y = cursor.origin.y + scroll.y;
    let top = visible_y - input.origin.y;
    if top + line_height < px(0.) || top > input.size.height {
        return None;
    }

    let gap = px(16.);
    let after_caret = cursor.origin.x + cursor.size.width + gap - input.origin.x;
    // Keep the label inside the editor: prefer after the caret, else pin right.
    let max_w = (input.size.width * 0.55).max(px(120.));
    let use_right_pin = after_caret + px(80.) > input.size.width;

    let mut label = format!("{} · {} · {}", entry.author, entry.short_oid, entry.when);
    if !entry.summary.is_empty() {
        label.push_str(" · ");
        label.push_str(&entry.summary);
    }

    let blame = div()
        .absolute()
        .top(top)
        .h(line_height)
        .flex()
        .items_center()
        .max_w(max_w)
        .px_2()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .opacity(0.55)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(label);

    let blame = if use_right_pin {
        blame.right(px(10.))
    } else {
        blame.left(after_caret)
    };

    Some(blame.into_any_element())
}
