use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, *};
use sgg_core::{ChangeKind, FileChange};

use super::status_style::{area_label, display_label, status_color};
use crate::app::busy::BusyKind;
use crate::app::i18n::UiText;
use crate::app::sort::{group_and_sort, section_as_tree, ChangeSort, ChangeTreeRow};
use crate::app::view::AppView;
use crate::app::widgets::{
    confirm_btn, display_btn, icon_btn, icon_danger_btn, pane_body, pane_header, pane_toolbar,
    raised_action_btn, with_busy,
};

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
        .size_full()
        .min_w_0()
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .justify_start()
                .gap_2()
                .px_3()
                .py_2()
                .child(div().font_semibold().text_left().child(t.changes))
                .child(
                    icon_btn("refresh", IconName::RefreshCw)
                        .accessibility_label(t.refresh)
                        .tooltip(t.tip_refresh)
                        .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                ),
        )
        .child(
            pane_toolbar(cx)
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(
                    div()
                        .w_full()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .child(Textarea::new(&view.commit_input).appearance(false)),
                )
                .child(
                    with_busy(
                        confirm_btn("commit", AssetIcon::GitCommitVertical, cx)
                            .w_full()
                            .label(t.commit)
                            .tooltip(t.tip_commit)
                            .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                        view.busy,
                        BusyKind::Commit,
                    ),
                ),
        )
        .child(stage_filter_row(view, sort, group_by_status, t, cx))
        .child(
            pane_body(cx)
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
                        let tree = section_as_tree(
                            section.title,
                            &section.items,
                            &view.expanded_change_dirs,
                        );
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
                                                    this.toggle_change_dir(key_toggle.clone(), cx);
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
                                        change_row(view, &change, &display_name, depth, t, cx)
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

fn stage_filter_row(
    view: &AppView,
    sort: ChangeSort,
    group_by_status: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let entity = cx.entity();
    let compact = view.stage_bar_compact;

    pane_toolbar(cx)
        .id("stage-filter-row")
        .relative()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_3()
        .py_2()
        .min_w_0()
        .child(
            canvas(
                {
                    let entity = entity.clone();
                    move |bounds, _, cx| {
                        let compact = bounds.size.width < px(520.);
                        // Avoid notify storms during live resize: only write when the mode flips.
                        let _ = entity.update(cx, |this, cx| {
                            this.set_stage_bar_compact(compact, cx);
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
            .invisible(),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_start()
                .gap_3()
                .min_w_0()
                .child(
                    Switch::new("group-by-status")
                        .small()
                        .label(t.group_by_status)
                        .checked(group_by_status)
                        .tooltip(t.tip_group_by_status)
                        .on_click({
                            let entity = entity.clone();
                            move |checked, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.set_group_by_status(*checked, cx);
                                });
                            }
                        }),
                )
                .child(
                    display_btn("sort-name-toggle")
                        .small()
                        .outline()
                        .icon(AssetIcon::ArrowUpDown)
                        .label(sort.label())
                        .tooltip(t.tip_sort_name_toggle)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_change_sort_name(cx);
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .min_w_0()
                .when(!compact, |this| {
                    this.child(
                        with_busy(
                            raised_action_btn("stage", AssetIcon::Plus, cx)
                                .small()
                                .label(t.stage)
                                .tooltip(t.tip_stage)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.stage_selected(window, cx)
                                })),
                            view.busy,
                            BusyKind::Stage,
                        ),
                    )
                    .child(
                        with_busy(
                            raised_action_btn("unstage", AssetIcon::Minus, cx)
                                .small()
                                .label(t.unstage)
                                .tooltip(t.tip_unstage)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.unstage_selected(window, cx)
                                })),
                            view.busy,
                            BusyKind::Unstage,
                        ),
                    )
                    .child(
                        with_busy(
                            raised_action_btn("stage-all", AssetIcon::ListPlus, cx)
                                .small()
                                .label(t.stage_all)
                                .tooltip(t.tip_stage_all)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.stage_all(window, cx)
                                })),
                            view.busy,
                            BusyKind::StageAll,
                        ),
                    )
                    .child(
                        with_busy(
                            raised_action_btn("unstage-all", AssetIcon::ListMinus, cx)
                                .small()
                                .label(t.unstage_all)
                                .tooltip(t.tip_unstage_all)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.unstage_all(window, cx)
                                })),
                            view.busy,
                            BusyKind::UnstageAll,
                        ),
                    )
                    .child(
                        with_busy(
                            {
                                use gpui_kit::component::button::ButtonCustomVariant;
                                use crate::app::widgets::action_btn;
                                action_btn("revert", AssetIcon::Undo2)
                                    .custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(cx.theme().danger)
                                            .foreground(cx.theme().danger_foreground)
                                            .hover(cx.theme().danger_hover)
                                            .active(cx.theme().danger_active)
                                            .shadow(true),
                                    )
                                    .small()
                                    .label(t.revert_selected)
                                    .tooltip(t.tip_revert_selected)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_revert(window, cx)
                                    }))
                            },
                            view.busy,
                            BusyKind::Revert,
                        ),
                    )
                })
                .when(compact, |this| {
                    this.child(stage_actions_dropdown(view, t, cx))
                }),
        )
}

fn stage_actions_dropdown(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    use gpui_kit::component::button::ButtonCustomVariant;
    let entity = cx.entity();
    let busy = view.busy;
    display_btn("stage-actions-menu")
        .small()
        .custom(
            ButtonCustomVariant::new(cx)
                .color(cx.theme().secondary)
                .foreground(cx.theme().secondary_foreground)
                .hover(cx.theme().secondary_hover)
                .active(cx.theme().secondary_active)
                .shadow(true),
        )
        .icon(AssetIcon::Ellipsis)
        .label(t.stage_actions)
        .tooltip(t.tip_stage_actions)
        .loading(matches!(
            busy,
            Some(
                BusyKind::Stage
                    | BusyKind::Unstage
                    | BusyKind::StageAll
                    | BusyKind::UnstageAll
                    | BusyKind::Revert
            )
        ))
        .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
            let e1 = entity.clone();
            let e2 = entity.clone();
            let e3 = entity.clone();
            let e4 = entity.clone();
            let e5 = entity.clone();
            menu.item(
                PopupMenuItem::new(t.stage)
                    .icon(AssetIcon::Plus)
                    .on_click(move |_, window, cx| {
                        e1.update(cx, |this, cx| this.stage_selected(window, cx));
                    }),
            )
            .item(
                PopupMenuItem::new(t.unstage)
                    .icon(AssetIcon::Minus)
                    .on_click(move |_, window, cx| {
                        e2.update(cx, |this, cx| this.unstage_selected(window, cx));
                    }),
            )
            .item(
                PopupMenuItem::new(t.stage_all)
                    .icon(AssetIcon::ListPlus)
                    .on_click(move |_, window, cx| {
                        e3.update(cx, |this, cx| this.stage_all(window, cx));
                    }),
            )
            .item(
                PopupMenuItem::new(t.unstage_all)
                    .icon(AssetIcon::ListMinus)
                    .on_click(move |_, window, cx| {
                        e4.update(cx, |this, cx| this.unstage_all(window, cx));
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new(t.revert_selected)
                    .icon(AssetIcon::Undo2)
                    .on_click(move |_, window, cx| {
                        e5.update(cx, |this, cx| this.confirm_revert(window, cx));
                    }),
            )
        })
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
            .on_click(cx.listener(move |this, _, window, cx| {
                this.stage_one(path_stage.clone(), window, cx);
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
            .on_click(cx.listener(move |this, _, window, cx| {
                this.unstage_one(path_unstage.clone(), window, cx);
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
