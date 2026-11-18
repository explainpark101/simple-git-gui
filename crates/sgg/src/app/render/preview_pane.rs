use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::ScrollableMask;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::diff::{Diff, DiffSide};
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, Size, h_resizable,
    resizable_panel,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sgg_core::AlignedSplitView;

use crate::app::busy::BusyKind;
use crate::app::page::AppPage;
use crate::app::view::{
    AppView, ChangesFileViewMode, HistoryDiffLayout, HistoryFileViewMode,
};
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
    let history_preview = if view.page == AppPage::History {
        view.history_file_preview.clone()
    } else {
        None
    };
    let is_history = history_preview.is_some();
    let is_changes = view.page == AppPage::Git;
    let changes_mode = view.changes_file_view_mode;
    let changes_layout = view.changes_diff_layout;
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
                            let oid = hp.oid.clone();
                            let short = if sgg_core::is_uncommitted_oid(&oid) {
                                "WIP".to_string()
                            } else {
                                oid.chars().take(7).collect()
                            };
                            this.child(
                                display_btn("preview-hist-oid")
                                    .ghost()
                                    .xsmall()
                                    .label(short)
                                    .tooltip(t.tip_history_open_commit)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.show_history_commit(oid.clone(), window, cx);
                                    })),
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
                                this.child(diff_layout_pill(hp.diff_layout, false, t, cx))
                                    .when(
                                        hp.diff_layout == HistoryDiffLayout::Split,
                                        |this| {
                                            this.child(diff_scroll_sync_switch(view, cx))
                                        },
                                    )
                            })
                        })
                        .when(is_changes && has_path, |this| {
                            let mode_label = if changes_mode == ChangesFileViewMode::Diff {
                                t.history_view_diff
                            } else {
                                t.changes_view_file
                            };
                            let entity = cx.entity();
                            this.child(
                                display_btn("preview-changes-mode")
                                    .outline()
                                    .small()
                                    .label(format!("{mode_label}  ▾"))
                                    .tooltip(t.tip_changes_view_mode)
                                    .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                                        let entity_file = entity.clone();
                                        let entity_diff = entity.clone();
                                        menu.item(
                                            menu_item(t.changes_view_file, move |_, window, cx| {
                                                entity_file.update(cx, |this, cx| {
                                                    this.set_changes_file_view_mode(
                                                        ChangesFileViewMode::File,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .checked(changes_mode == ChangesFileViewMode::File),
                                        )
                                        .item(
                                            menu_item(t.history_view_diff, move |_, window, cx| {
                                                entity_diff.update(cx, |this, cx| {
                                                    this.set_changes_file_view_mode(
                                                        ChangesFileViewMode::Diff,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            })
                                            .checked(changes_mode == ChangesFileViewMode::Diff),
                                        )
                                    }),
                            )
                            .when(changes_mode == ChangesFileViewMode::Diff, |this| {
                                this.child(diff_layout_pill(changes_layout, true, t, cx))
                                    .when(
                                        changes_layout == HistoryDiffLayout::Split,
                                        |this| {
                                            this.child(diff_scroll_sync_switch(view, cx))
                                        },
                                    )
                            })
                        })
                        .when(!is_history && !(is_changes && changes_mode == ChangesFileViewMode::Diff), |this| {
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
            let layout = view.preview_diff_layout();
            let unified_diff = layout == Some(HistoryDiffLayout::Unified);
            let split_diff = layout == Some(HistoryDiffLayout::Split);
            // Split id must differ per page so resizable state does not clash.
            let split_id = if is_history {
                "history-diff-split"
            } else {
                "changes-diff-split"
            };
            pane_body(cx)
                .flex_1()
                .min_h_0()
                .p_2()
                .child(if unified_diff {
                    div()
                        .size_full()
                        .min_h_0()
                        .flex()
                        .flex_col()
                        .children(unified_diff_blame_bar(view, cx))
                        .child(
                            Diff::new(&view.history_diff)
                                .header_visible(false)
                                .soft_wrap(false)
                                .h(relative(1.))
                                .flex_1()
                                .min_h_0(),
                        )
                        .into_any_element()
                } else if split_diff {
                    let aligned = view.history_aligned_split.clone();
                    let scroll_old = view.history_split_scroll_handle(true).clone();
                    let scroll_new = view.history_split_scroll_handle(false).clone();
                    h_resizable(split_id)
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
                                    scroll_old,
                                    None,
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
                                scroll_new,
                                Some(view),
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
                        .children(inline_blame_overlay(
                            view,
                            &view.preview_input,
                            None,
                            // Default Editor size is Medium (py=8); coords must
                            // include that inset vs the outer relative parent.
                            Size::Medium,
                            cx,
                        ))
                        .into_any_element()
                })
        })
}

/// Match `gpui_kit::component::input::editor::EDITOR_LINE_HEIGHT` (private).
const SPLIT_EDITOR_LINE_HEIGHT: f32 = 1.5;

/// One Split Diff pane: gutter + editor share one outer ScrollHandle so line
/// numbers never lag behind the text (no observe→notify gutter chase).
fn split_diff_side(
    id: &'static str,
    title: &'static str,
    editor: &Entity<EditorState>,
    aligned: Option<&AlignedSplitView>,
    old_side: bool,
    scroll: ScrollHandle,
    // When set (new side), draw caret-line blame over the editor.
    blame_view: Option<&AppView>,
    cx: &App,
) -> impl IntoElement {
    let line_height = editor.read(cx).line_height().unwrap_or_else(|| {
        px(f32::from(cx.theme().mono_font_size) * SPLIT_EDITOR_LINE_HEIGHT)
    });
    let rows = aligned
        .map(|a| a.source_lines(old_side).len())
        .unwrap_or(1)
        .max(1);
    let content_h = line_height * rows as f32;
    let digits = aligned.map(AlignedSplitView::line_number_digits).unwrap_or(3);
    let gutter_w = px(10. + digits as f32 * 8.5);
    let muted = cx.theme().muted_foreground;
    let border = cx.theme().border;
    let gutter_bg = cx.theme().secondary;
    let mono = cx.theme().mono_font_family.clone();
    let mono_size = cx.theme().mono_font_size;

    let gutter = div()
        .id(SharedString::from(format!("hist-split-gutter-{id}")))
        .w(gutter_w)
        .h(content_h)
        .flex_shrink_0()
        .border_r_1()
        .border_color(border)
        .bg(gutter_bg)
        .children(aligned.map(|aligned| {
            let lines = aligned.source_lines(old_side);
            div().children(lines.iter().enumerate().map(|(i, line)| {
                let label = line
                    .source_line
                    .map(|n| format!("{n}"))
                    .unwrap_or_default();
                div()
                    .id(SharedString::from(format!("hist-split-ln-{id}-{i}")))
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
        }));

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
                .id(SharedString::from(format!("hist-split-scroll-{id}")))
                .relative()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(
                    div()
                        .id(SharedString::from(format!("hist-split-body-{id}")))
                        .size_full()
                        .overflow_hidden()
                        .track_scroll(&scroll)
                        .child(
                            div()
                                .flex()
                                .w_full()
                                .h(content_h)
                                .child(gutter)
                                // Full content height → no internal editor scroll;
                                // the outer handle moves gutter + text together.
                                // Blame sits inside the scrolled body so it stays
                                // glued to the caret row without lag.
                                .child(
                                    div()
                                        .relative()
                                        .flex_1()
                                        .min_w_0()
                                        .h(content_h)
                                        .child(
                                            Editor::new(editor)
                                                .xsmall()
                                                .h(content_h)
                                                .w_full(),
                                        )
                                        .children(blame_view.and_then(|view| {
                                            inline_blame_overlay(
                                                view,
                                                editor,
                                                aligned.filter(|_| !old_side),
                                                // Split editors use .xsmall() (py=0).
                                                Size::XSmall,
                                                cx,
                                            )
                                        })),
                                ),
                        ),
                )
                // Capture wheel over the editor and drive the shared handle.
                .child(
                    ScrollableMask::new(Axis::Vertical, &scroll)
                        .id(SharedString::from(format!("hist-split-mask-{id}"))),
                ),
        )
}

fn diff_scroll_sync_switch(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    Switch::new("preview-diff-scroll-sync")
        .small()
        .label(t.history_diff_scroll_sync)
        .checked(view.history_diff_scroll_sync)
        .tooltip(t.tip_history_diff_scroll_sync)
        .on_click(cx.listener(|this, checked, _, cx| {
            this.set_history_diff_scroll_sync(*checked, cx);
        }))
}

/// Icon-only unified/split radio pill (labels live in tooltips).
fn diff_layout_pill(
    layout: HistoryDiffLayout,
    for_changes: bool,
    t: crate::app::i18n::UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let unified_id = if for_changes {
        "preview-changes-unified"
    } else {
        "preview-hist-unified"
    };
    let split_id = if for_changes {
        "preview-changes-split"
    } else {
        "preview-hist-split"
    };
    div()
        .flex()
        .items_stretch()
        .rounded_full()
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .child(
            icon_btn(unified_id, AssetIcon::FileDiff)
                .selected(layout == HistoryDiffLayout::Unified)
                .accessibility_label(t.history_diff_unified)
                .tooltip(t.tip_history_diff_unified)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if for_changes {
                        this.set_changes_diff_layout(HistoryDiffLayout::Unified, window, cx);
                    } else {
                        this.set_history_diff_layout(HistoryDiffLayout::Unified, window, cx);
                    }
                })),
        )
        .child(
            div()
                .w(px(1.))
                .self_stretch()
                .bg(cx.theme().border),
        )
        .child(
            icon_btn(split_id, AssetIcon::Split)
                .selected(layout == HistoryDiffLayout::Split)
                .accessibility_label(t.history_diff_split)
                .tooltip(t.tip_history_diff_split)
                .on_click(cx.listener(move |this, _, window, cx| {
                    if for_changes {
                        this.set_changes_diff_layout(HistoryDiffLayout::Split, window, cx);
                    } else {
                        this.set_history_diff_layout(HistoryDiffLayout::Split, window, cx);
                    }
                })),
        )
}

fn format_blame_label(entry: &sgg_core::BlameEntry) -> String {
    let mut label = format!("{} · {} · {}", entry.author, entry.short_oid, entry.when);
    if !entry.summary.is_empty() {
        label.push_str(" · ");
        label.push_str(&entry.summary);
    }
    label
}

/// Blame strip for Unified Diff: follows the Modified-side line selection.
fn unified_diff_blame_bar(view: &AppView, cx: &App) -> Option<AnyElement> {
    if !view.show_inline_blame || view.preview_path.is_none() {
        return None;
    }
    let sel = view.history_diff.read(cx).selected_lines()?;
    if sel.side() != DiffSide::Modified {
        return None;
    }
    let line_1based = sel.start();
    let entry = view.blame_lines.get(line_1based.saturating_sub(1))?.as_ref()?;
    Some(
        div()
            .px_2()
            .py_1()
            .mb_1()
            .rounded_md()
            .bg(cx.theme().secondary)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .whitespace_nowrap()
            .overflow_hidden()
            .text_ellipsis()
            .child(format_blame_label(entry))
            .into_any_element(),
    )
}

/// Caret-line inline blame: same row as the caret, parked after the line end
/// (not mid-line after the caret) so it never covers source text.
///
/// `aligned_new`: Split Diff new-side mapping from display row → 1-based source line.
/// `editor_size`: must match the `Editor` size so padding insets line up with
/// the outer relative parent (input_bounds is the padded content box).
fn inline_blame_overlay(
    view: &AppView,
    editor: &Entity<EditorState>,
    aligned_new: Option<&AlignedSplitView>,
    editor_size: Size,
    cx: &App,
) -> Option<AnyElement> {
    if !view.show_inline_blame || view.preview_path.is_none() {
        return None;
    }

    let editor = editor.read(cx);
    let pos = editor.cursor_position();
    let display_line = pos.line as usize;
    let source_line_0 = if let Some(aligned) = aligned_new {
        let src = aligned.source_lines(false).get(display_line)?.source_line?;
        (src as usize).saturating_sub(1)
    } else {
        display_line
    };
    let entry = view.blame_lines.get(source_line_0)?.as_ref()?;
    let (cursor, line_height) = editor.cursor_layout()?;
    let scroll = editor.scroll_offset();
    let input = editor.input_bounds();
    // Code editors clamp left pad (see gpui-component Input::render).
    let pad_x = editor_size.input_px().min(px(6.));
    let pad_y = editor_size.input_py();

    // cursor_layout Y is unscrolled content coords; painter applies scroll.
    // Caret bounds are vertically inset inside the row — lift to the row top.
    let caret_inset = (line_height - cursor.size.height) / 2.;
    let visible_y = cursor.origin.y + scroll.y - caret_inset;
    // Absolute coords are relative to the Editor chrome; input_bounds is inset
    // by size padding — add it back or blame sits high (looks bottom-padded).
    let top = visible_y - input.origin.y + pad_y;
    if top + line_height < px(0.) || top > input.size.height + pad_y {
        return None;
    }

    // Place after end-of-line, not after a mid-line caret (avoids covering code).
    let line_text = editor
        .value()
        .lines()
        .nth(display_line)
        .unwrap_or("")
        .to_string();
    let line_utf16 = line_text.encode_utf16().count();
    let remaining = line_utf16.saturating_sub(pos.character as usize);
    let mono = f32::from(cx.theme().mono_font_size);
    let advance = mono * 0.6;
    let gap = px(16.);
    let after_eol = cursor.origin.x + cursor.size.width + gap + px(advance * remaining as f32)
        - input.origin.x
        + pad_x;

    let max_w = (input.size.width * 0.55).max(px(120.));
    let use_right_pin = after_eol + px(80.) > input.size.width + pad_x;

    let mono_family = cx.theme().mono_font_family.clone();
    let blame = div()
        .absolute()
        .top(top)
        .h(line_height)
        .flex()
        .items_center()
        .max_w(max_w)
        .px_2()
        // Match editor mono metrics so leading does not look like bottom pad.
        .font_family(mono_family)
        .text_size(cx.theme().mono_font_size)
        .line_height(relative(1.))
        .text_color(cx.theme().muted_foreground)
        .opacity(0.55)
        .whitespace_nowrap()
        .overflow_hidden()
        .text_ellipsis()
        .child(format_blame_label(entry));

    let blame = if use_right_pin {
        blame.right(px(10.))
    } else {
        blame.left(after_eol)
    };

    Some(blame.into_any_element())
}
