use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, *};
use sgg_core::{ChangeKind, FileChange};

use super::status_style::{area_label, display_label, status_color};
use crate::app::i18n::UiText;
use crate::app::sort::{ChangeSort, ChangeTreeRow, group_and_sort, section_as_tree};
use crate::app::view::AppView;
use crate::app::widgets::{action_btn, display_btn, icon_btn, icon_danger_btn};

pub fn render(
    view: &AppView,
    changes: Vec<FileChange>,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let sort = view.change_sort.normalized();
    let group_by_status = view.group_by_status;
    let as_tree = view.changes_as_tree;
    let sections = group_and_sort(changes, sort, group_by_status);

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
                .justify_start()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(div().font_semibold().text_left().child(t.changes))
                .child(
                    icon_btn("refresh", IconName::RefreshCw)
                        .accessibility_label(t.refresh)
                        .tooltip(t.tip_refresh)
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
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
                    div().flex().justify_center().child(
                        action_btn("commit")
                            .primary()
                            .label(t.commit)
                            .tooltip(t.tip_commit)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.commit(window, cx)),
                            ),
                    ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap_2()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    action_btn("stage")
                        .small()
                        .outline()
                        .label(t.stage)
                        .tooltip(t.tip_stage)
                        .on_click(cx.listener(|this, _, _, cx| this.stage_selected(cx))),
                )
                .child(
                    action_btn("unstage")
                        .small()
                        .outline()
                        .label(t.unstage)
                        .tooltip(t.tip_unstage)
                        .on_click(cx.listener(|this, _, _, cx| this.unstage_selected(cx))),
                )
                .child(
                    action_btn("stage-all")
                        .small()
                        .outline()
                        .label(t.stage_all)
                        .tooltip(t.tip_stage_all)
                        .on_click(cx.listener(|this, _, _, cx| this.stage_all(cx))),
                )
                .child(
                    action_btn("unstage-all")
                        .small()
                        .outline()
                        .label(t.unstage_all)
                        .tooltip(t.tip_unstage_all)
                        .on_click(cx.listener(|this, _, _, cx| this.unstage_all(cx))),
                )
                .child(
                    action_btn("revert")
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
                .justify_start()
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
                .child(
                    display_btn("sort-group-status")
                        .small()
                        .outline()
                        .label(t.group_by_status)
                        .tooltip(t.tip_group_by_status)
                        .selected(group_by_status)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_group_by_status(!group_by_status, cx)
                        })),
                )
                .child(
                    display_btn("sort-name-asc")
                        .small()
                        .outline()
                        .label(ChangeSort::NameAsc.label())
                        .tooltip(t.tip_sort_name_asc)
                        .selected(sort == ChangeSort::NameAsc)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_change_sort(ChangeSort::NameAsc, cx)
                        })),
                )
                .child(
                    display_btn("sort-name-desc")
                        .small()
                        .outline()
                        .label(ChangeSort::NameDesc.label())
                        .tooltip(t.tip_sort_name_desc)
                        .selected(sort == ChangeSort::NameDesc)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.set_change_sort(ChangeSort::NameDesc, cx)
                        })),
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
                .items_start()
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

                    if section.items.is_empty() {
                        rows.push(
                            div()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .text_left()
                                .text_color(cx.theme().muted_foreground)
                                .child(t.section_empty)
                                .into_any_element(),
                        );
                    } else if as_tree {
                        let tree =
                            section_as_tree(section.title, &section.items, &view.expanded_change_dirs);
                        for row in tree {
                            match row {
                                ChangeTreeRow::Dir {
                                    name, key, depth, ..
                                } => {
                                    let expanded =
                                        view.expanded_change_dirs.iter().any(|k| k == &key);
                                    let folder_icon = if expanded {
                                        IconName::FolderOpen
                                    } else {
                                        IconName::FolderClosed
                                    };
                                    let indent = px(8.0 + 14.0 * depth as f32);
                                    let key_toggle = key.clone();
                                    rows.push(
                                        div()
                                            .id(SharedString::from(format!("chdir:{key}")))
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_start()
                                            .pl(indent)
                                            .pr_1()
                                            .child(
                                                display_btn(SharedString::from(format!(
                                                    "chdir-btn:{key}"
                                                )))
                                                .ghost()
                                                .small()
                                                .icon(folder_icon)
                                                .label(format!("{name}/"))
                                                .tooltip(t.tip_expand_dir)
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.toggle_change_dir(
                                                        key_toggle.clone(),
                                                        cx,
                                                    );
                                                })),
                                            )
                                            .into_any_element(),
                                    );
                                }
                                ChangeTreeRow::File {
                                    change,
                                    display_name,
                                    depth,
                                } => {
                                    rows.push(
                                        change_row(
                                            view,
                                            &change,
                                            &display_name,
                                            depth,
                                            t,
                                            cx,
                                        )
                                        .into_any_element(),
                                    );
                                }
                            }
                        }
                    } else {
                        for change in section.items {
                            let display = change.path.clone();
                            rows.push(
                                change_row(view, &change, &display, 0, t, cx).into_any_element(),
                            );
                        }
                    }
                    rows
                })),
        )
}

fn section_title(title: &str, t: UiText) -> &'static str {
    match title {
        "All" => t.section_all,
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
    display_name: &str,
    depth: usize,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let path_for_checkbox = change.path.clone();
    let path_for_row = change.path.clone();
    let path_open = change.path.clone();
    let path_preview = change.path.clone();
    let path_revert = change.path.clone();
    let path_stage = change.path.clone();
    let path_unstage = change.path.clone();
    let can_stage = change.kind != ChangeKind::Staged;
    let can_unstage = change.kind == ChangeKind::Staged;
    let selected = view.selected.iter().any(|p| p == &change.path);
    let status = display_label(&change.status_label);
    let area = area_label(change.kind);
    let color = status_color(&change.status_label, cx);
    let indent = px(8.0 + 14.0 * depth as f32);

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
        .pl(indent)
        .pr_2()
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
        .child(IconName::File)
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
                .child(display_name.to_string()),
        )
        .child(
            icon_btn(
                SharedString::from(format!("open:{}", change.path)),
                IconName::ExternalLink,
            )
            .accessibility_label(t.open)
            .tooltip(t.tip_open)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_path_in_editor(&path_open, cx);
            })),
        )
        .child(
            icon_btn(
                SharedString::from(format!("stage:{}", change.path)),
                IconName::Plus,
            )
            .accessibility_label(t.stage)
            .tooltip(t.tip_stage_one)
            .disabled(!can_stage)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.stage_one(path_stage.clone(), cx);
            })),
        )
        .child(
            icon_btn(
                SharedString::from(format!("unstage:{}", change.path)),
                IconName::Minus,
            )
            .accessibility_label(t.unstage)
            .tooltip(t.tip_unstage_one)
            .disabled(!can_unstage)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.unstage_one(path_unstage.clone(), cx);
            })),
        )
        .child(
            icon_btn(
                SharedString::from(format!("preview:{}", change.path)),
                IconName::Eye,
            )
            .accessibility_label(t.preview)
            .tooltip(t.tip_preview)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.preview_file(path_preview.clone(), window, cx);
            })),
        )
        .child(
            icon_danger_btn(
                SharedString::from(format!("revert:{}", change.path)),
                IconName::Undo2,
            )
            .accessibility_label(t.revert)
            .tooltip(t.tip_revert)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.confirm_revert_one(path_revert.clone(), window, cx);
            })),
        )
}