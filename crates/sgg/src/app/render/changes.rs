use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Selectable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sgg_core::FileChange;

use super::status_style::{area_label, display_label, status_color};
use crate::app::i18n::UiText;
use crate::app::sort::{ChangeSort, group_and_sort};
use crate::app::view::AppView;
use crate::app::widgets::btn;

pub fn render(
    view: &AppView,
    changes: Vec<FileChange>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let sort = view.change_sort;
    let sections = group_and_sort(changes, sort);

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .border_r_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(div().font_semibold().text_left().child(t.changes))
                .child(
                    btn("stage")
                        .small()
                        .outline()
                        .label(t.stage)
                        .tooltip(t.tip_stage)
                        .on_click(cx.listener(|this, _, _, cx| this.stage_selected(cx))),
                )
                .child(
                    btn("unstage")
                        .small()
                        .outline()
                        .label(t.unstage)
                        .tooltip(t.tip_unstage)
                        .on_click(cx.listener(|this, _, _, cx| this.unstage_selected(cx))),
                )
                .child(
                    btn("stage-all")
                        .small()
                        .outline()
                        .label(t.stage_all)
                        .tooltip(t.tip_stage_all)
                        .on_click(cx.listener(|this, _, _, cx| this.stage_all(cx))),
                )
                .child(
                    btn("unstage-all")
                        .small()
                        .outline()
                        .label(t.unstage_all)
                        .tooltip(t.tip_unstage_all)
                        .on_click(cx.listener(|this, _, _, cx| this.unstage_all(cx))),
                )
                .child(
                    btn("revert")
                        .small()
                        .danger()
                        .label(t.revert_selected)
                        .tooltip(t.tip_revert_selected)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_revert(window, cx)
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_xs()
                        .text_left()
                        .text_color(cx.theme().muted_foreground)
                        .child(t.sort),
                )
                .children(
                    [
                        (
                            ChangeSort::NameAsc,
                            t.tip_sort_name_asc,
                        ),
                        (
                            ChangeSort::NameDesc,
                            t.tip_sort_name_desc,
                        ),
                        (
                            ChangeSort::StatusAsc,
                            t.tip_sort_status_asc,
                        ),
                        (
                            ChangeSort::StatusDesc,
                            t.tip_sort_status_desc,
                        ),
                    ]
                    .into_iter()
                    .map(|(mode, tip)| {
                        btn(SharedString::from(format!("sort-{}", mode.label())))
                            .small()
                            .outline()
                            .label(mode.label())
                            .tooltip(tip)
                            .selected(sort == mode)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_change_sort(mode, cx)
                            }))
                            .into_any_element()
                    }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Textarea::new(&view.commit_input))
                .child(
                    btn("commit")
                        .primary()
                        .label(t.commit)
                        .tooltip(t.tip_commit)
                        .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                ),
        )
        .child(
            div()
                .id("changes-list")
                .flex_1()
                .min_h_0()
                .p_2()
                .gap_1()
                .flex()
                .flex_col()
                .overflow_y_scrollbar()
                .children(sections.into_iter().flat_map(|section| {
                    let mut rows = Vec::new();
                    let title = section_title(section.title, t);
                    rows.push(
                        div()
                            .px_1()
                            .pt_2()
                            .pb_1()
                            .text_xs()
                            .font_semibold()
                            .text_left()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} ({})", title, section.items.len()))
                            .into_any_element(),
                    );
                    for change in section.items {
                        rows.push(change_row(view, &change, t, cx).into_any_element());
                    }
                    rows
                })),
        )
}

fn section_title(title: &str, t: UiText) -> &'static str {
    match title {
        "Conflicted" => t.section_conflicted,
        "Staged" => t.section_staged,
        "Unstaged" => t.section_unstaged,
        "Untracked" => t.section_untracked,
        _ => t.changes,
    }
}

fn change_row(
    view: &AppView,
    change: &FileChange,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let path_for_checkbox = change.path.clone();
    let path_for_row = change.path.clone();
    let path_open = change.path.clone();
    let path_preview = change.path.clone();
    let path_revert = change.path.clone();
    let selected = view.selected.iter().any(|p| p == &change.path);
    let status = display_label(&change.status_label);
    let area = area_label(change.kind);
    let color = status_color(&change.status_label, cx);

    div()
        .id(SharedString::from(format!(
            "change:{}:{}",
            area, change.path
        )))
        .flex()
        .w_full()
        .items_center()
        .justify_start()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_md()
        .when(selected, |this| this.bg(cx.theme().accent.opacity(0.25)))
        .hover(|this| this.bg(cx.theme().muted.opacity(0.45)))
        .child(
            Checkbox::new(SharedString::from(format!("cb:{}:{}", area, change.path)))
                .checked(selected)
                .tooltip(t.tip_select_change)
                .on_click(cx.listener(move |this, checked, _, cx| {
                    this.set_selected(path_for_checkbox.clone(), *checked, cx);
                })),
        )
        .child(
            div()
                .w(px(72.))
                .text_xs()
                .font_semibold()
                .text_left()
                .text_color(color)
                .child(status),
        )
        .child(
            div()
                .id(SharedString::from(format!("path:{}:{}", area, change.path)))
                .flex_1()
                .min_w_0()
                .text_sm()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .text_left()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_select(path_for_row.clone(), cx);
                }))
                .child(change.path.clone()),
        )
        .child(
            btn(SharedString::from(format!("open:{}", change.path)))
                .small()
                .ghost()
                .label(t.open)
                .tooltip(t.tip_open)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_path_in_editor(&path_open, cx);
                })),
        )
        .child(
            btn(SharedString::from(format!("preview:{}", change.path)))
                .small()
                .ghost()
                .label(t.preview)
                .tooltip(t.tip_preview)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.preview_file(path_preview.clone(), window, cx);
                })),
        )
        .child(
            btn(SharedString::from(format!("revert:{}", change.path)))
                .small()
                .danger()
                .label(t.revert)
                .tooltip(t.tip_revert)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.confirm_revert_one(path_revert.clone(), window, cx);
                })),
        )
}