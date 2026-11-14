use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _, h_resizable, resizable_panel,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, *};
use sgg_core::{ChangeKind, FileChange};

use super::status_style::{area_label, display_label, status_color, status_letter};
use crate::app::busy::BusyKind;
use crate::app::i18n::UiText;
use crate::app::sort::{
    paths_under_dir, section_as_tree, ChangeSection, ChangeSort, ChangeTreeRow,
};
use crate::app::view::AppView;
use crate::app::widgets::{
    confirm_btn, display_btn, icon_btn, icon_danger_btn, pane_body, pane_header, pane_toolbar,
    raised_action_btn, with_busy,
};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let sort = view.change_sort.normalized();
    let group_by_status = view.group_by_status;
    let as_tree = view.changes_as_tree;
    // Prefetched on reload / sort / group toggles — avoid regroup+sort every paint.
    let sections = view.changes_sections.clone();

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
                        // Input surface — distinct from secondary/toolbar action chips.
                        .bg(cx.theme().input_background())
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
        .child({
            let entity = cx.entity();
            let compact = view.changes_list_compact;
            let hide_labels = view.changes_hide_labels;
            let use_split = view.changes_split_view && group_by_status;
            let measure = canvas(
                {
                    let entity = entity.clone();
                    move |bounds, _, cx| {
                        let pane_w = bounds.size.width;
                        // Progressive density (widest → narrowest), per column:
                        //   1) hide_labels — section header buttons icon-only
                        //   2) compact     — letter status + row actions on hover
                        //   3) drop split  — stack sections in one column
                        // Compact MUST be reachable while split is on: with split at
                        // 720px, each col is ~360px, so row compact at 420px still fires.
                        // (A prior title-budget of 280px made compact impossible under split.)
                        const HIDE_LABELS_W: f32 = 520.;
                        const ROW_COMPACT_W: f32 = 420.;
                        const SPLIT_MIN_W: f32 = 720.;
                        let split = pane_w >= px(SPLIT_MIN_W);
                        let col_w = if split {
                            pane_w / 2.
                        } else {
                            pane_w
                        };
                        let hide_labels = col_w < px(HIDE_LABELS_W);
                        let compact = col_w < px(ROW_COMPACT_W);
                        // Skip entity.update during layout when flags are unchanged.
                        let same = entity.read_with(cx, |this, _| {
                            this.changes_hide_labels == hide_labels
                                && this.changes_list_compact == compact
                                && this.changes_split_view == split
                        });
                        if same {
                            return;
                        }
                        let _ = entity.update(cx, |this, cx| {
                            this.set_changes_density(hide_labels, compact, split, cx);
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
            .invisible();

            if use_split {
                let (staged, working): (Vec<_>, Vec<_>) = sections
                    .into_iter()
                    .partition(|s| s.title == "Staged");
                pane_body(cx)
                    .id("changes-list-split")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(measure)
                    .child(
                        h_resizable("changes-split")
                            .child(
                                resizable_panel()
                                    .size(px(360.))
                                    .size_range(px(240.)..px(720.))
                                    .child(
                                        changes_column(
                                            "changes-working",
                                            view,
                                            working,
                                            as_tree,
                                            compact,
                                            hide_labels,
                                            t,
                                            cx,
                                        )
                                        .into_any_element(),
                                    ),
                            )
                            .child(
                                resizable_panel().child(
                                    changes_column(
                                        "changes-staged",
                                        view,
                                        staged,
                                        as_tree,
                                        compact,
                                        hide_labels,
                                        t,
                                        cx,
                                    )
                                    .into_any_element(),
                                ),
                            ),
                    )
                    .into_any_element()
            } else {
                pane_body(cx)
                    .id("changes-list")
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .gap_2()
                    .flex()
                    .flex_col()
                    .items_stretch()
                    .child(measure)
                    .children(render_section_panels(
                        view, sections, as_tree, compact, hide_labels, t, cx,
                    ))
                    .into_any_element()
            }
        })
}

fn changes_column(
    id: &'static str,
    view: &AppView,
    sections: Vec<ChangeSection>,
    as_tree: bool,
    compact: bool,
    hide_labels: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .size_full()
        .min_w_0()
        .min_h_0()
        .p_2()
        .gap_2()
        .items_stretch()
        .children(render_section_panels(
            view, sections, as_tree, compact, hide_labels, t, cx,
        ))
}

/// One panel per status section: sticky header + independently scrolling body.
/// GPUI has no CSS `position: sticky`, so the header stays pinned as a sibling
/// above the scroll viewport instead of living inside it.
fn render_section_panels(
    view: &AppView,
    sections: Vec<ChangeSection>,
    as_tree: bool,
    compact: bool,
    hide_labels: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> Vec<AnyElement> {
    sections
        .into_iter()
        .map(|section| {
            let title_key = section.title.to_string();
            let expanded = !view.collapsed_change_sections.contains(&title_key);
            let body_rows = if expanded {
                section_body_rows(view, &section, as_tree, compact, t, cx)
            } else {
                Vec::new()
            };

            div()
                .id(SharedString::from(format!("section-panel:{}", section.title)))
                .w_full()
                .min_w_0()
                .min_h_0()
                .flex()
                .flex_col()
                .rounded(cx.theme().radius)
                .border_1()
                .border_color(cx.theme().border)
                .overflow_hidden()
                .when(expanded, |this| this.flex_1())
                .when(!expanded, |this| this.flex_none())
                .child(section_header(
                    view,
                    section.title,
                    section.items.len(),
                    expanded,
                    hide_labels,
                    t,
                    cx,
                ))
                .when(expanded, |this| {
                    this.child(
                        div()
                            .id(SharedString::from(format!(
                                "section-body:{}",
                                section.title
                            )))
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_start()
                            .px_1()
                            .py_1()
                            .gap_0()
                            .overflow_y_scrollbar()
                            .children(body_rows),
                    )
                })
                .into_any_element()
        })
        .collect()
}

fn section_body_rows(
    view: &AppView,
    section: &ChangeSection,
    as_tree: bool,
    compact: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> Vec<AnyElement> {
    let mut rows = Vec::new();
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
        return rows;
    }

    if as_tree {
        let tree = section_as_tree(
            section.title,
            &section.items,
            &view.collapsed_change_dirs,
        );
        for row in tree {
            match row {
                ChangeTreeRow::Dir {
                    name,
                    path,
                    key,
                    depth,
                } => {
                    let dir_expanded = !view.collapsed_change_dirs.contains(&key);
                    let folder_icon = if dir_expanded {
                        IconName::FolderOpen
                    } else {
                        IconName::FolderClosed
                    };
                    let indent = px(8.0 + 14.0 * depth as f32);
                    let key_toggle = key.clone();
                    let child_paths = paths_under_dir(&section.items, &path);
                            let all_selected = !child_paths.is_empty()
                                && child_paths.iter().all(|p| view.selected.contains(p));
                    let cb_id =
                        SharedString::from(format!("cb-dir:{}:{}", section.title, path));
                    rows.push(
                        div()
                            .id(SharedString::from(format!("chdir:{key}")))
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_start()
                            .gap_1()
                            .pl(indent)
                            .pr_1()
                            .py_1()
                            .cursor_pointer()
                            .child(
                                Checkbox::new(cb_id)
                                    .checked(all_selected)
                                    .tooltip(t.tip_select_dir)
                                    .on_click(cx.listener(move |this, checked, _, cx| {
                                        cx.stop_propagation();
                                        this.set_selected_many(&child_paths, *checked, cx);
                                    })),
                            )
                            .child(div().flex_shrink_0().child(folder_icon))
                            .child(
                                div()
                                    .text_sm()
                                    .text_left()
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(format!("{name}/")),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_change_dir(key_toggle.clone(), cx);
                            }))
                            .into_any_element(),
                    );
                }
                ChangeTreeRow::File {
                    change,
                    display_name,
                    depth,
                } => {
                    rows.push(
                        change_row(view, &change, &display_name, depth, compact, t, cx)
                            .into_any_element(),
                    );
                }
            }
        }
    } else {
        for change in &section.items {
            let display = change.path.clone();
            rows.push(
                change_row(view, change, &display, 0, compact, t, cx).into_any_element(),
            );
        }
    }
    rows
}

fn stage_filter_row(
    _view: &AppView,
    sort: ChangeSort,
    group_by_status: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let entity = cx.entity();

    pane_toolbar(cx)
        .id("stage-filter-row")
        .flex()
        .items_center()
        .justify_start()
        .gap_3()
        .px_3()
        .py_2()
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
        )
}

fn section_action_label(
    btn: gpui_kit::component::button::Button,
    label: &'static str,
    tip: &'static str,
    hide_labels: bool,
) -> gpui_kit::component::button::Button {
    btn.when(hide_labels, |this| this.compact())
        .when(!hide_labels, |this| this.label(label))
        .accessibility_label(label)
        .tooltip(tip)
}

fn section_header(
    view: &AppView,
    title: &'static str,
    count: usize,
    expanded: bool,
    hide_labels: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let label = section_title(title, t);
    let title_toggle = title.to_string();
    let chevron = if expanded {
        AssetIcon::ChevronDown
    } else {
        AssetIcon::ChevronRight
    };

    pane_header(cx)
        .id(SharedString::from(format!("section-head:{title}")))
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .child(
            // Left: title + bulk actions (Stage/Unstage All).
            div()
                .flex()
                .items_center()
                .justify_start()
                .gap_1()
                .min_w_0()
                .flex_1()
                .child(
                    icon_btn(
                        SharedString::from(format!("section-toggle:{title}")),
                        chevron,
                    )
                    .accessibility_label(if expanded {
                        t.tip_collapse_section
                    } else {
                        t.tip_expand_section
                    })
                    .tooltip(if expanded {
                        t.tip_collapse_section
                    } else {
                        t.tip_expand_section
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_change_section(title_toggle.clone(), cx);
                    })),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_shrink(1.)
                        .text_sm()
                        .font_bold()
                        .text_left()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(cx.theme().foreground)
                        .child(format!("{label} ({count})")),
                )
                .when(title == "Unstaged" || title == "All", |this| {
                    this.child(
                        with_busy(
                            section_action_label(
                                raised_action_btn("section-stage-all", AssetIcon::ListPlus, cx)
                                    .small(),
                                t.stage_all,
                                t.tip_stage_all,
                                hide_labels,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.stage_all(window, cx)
                            })),
                            view.busy,
                            BusyKind::StageAll,
                        ),
                    )
                })
                .when(title == "Untracked", |this| {
                    this.child(
                        with_busy(
                            section_action_label(
                                raised_action_btn(
                                    "section-stage-all-untracked",
                                    AssetIcon::ListPlus,
                                    cx,
                                )
                                .small(),
                                t.stage_all_untracked,
                                t.tip_stage_all_untracked,
                                hide_labels,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.stage_all_untracked(window, cx)
                            })),
                            view.busy,
                            BusyKind::StageAll,
                        ),
                    )
                })
                .when(title == "Staged" || title == "All", |this| {
                    this.child(
                        with_busy(
                            section_action_label(
                                raised_action_btn(
                                    "section-unstage-all",
                                    AssetIcon::ListMinus,
                                    cx,
                                )
                                .small(),
                                t.unstage_all,
                                t.tip_unstage_all,
                                hide_labels,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.unstage_all(window, cx)
                            })),
                            view.busy,
                            BusyKind::UnstageAll,
                        ),
                    )
                })
                .when(title == "Untracked", |this| {
                    this.child({
                        use crate::app::widgets::action_btn;
                        use gpui_kit::component::button::ButtonCustomVariant;
                        with_busy(
                            section_action_label(
                                action_btn("section-revert-all-untracked", AssetIcon::Undo2)
                                    .custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(cx.theme().danger)
                                            .foreground(cx.theme().danger_foreground)
                                            .hover(cx.theme().danger_hover)
                                            .active(cx.theme().danger_active)
                                            .shadow(true),
                                    )
                                    .small(),
                                t.revert_all_untracked,
                                t.tip_revert_all_untracked,
                                hide_labels,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_revert_all_untracked(window, cx)
                            })),
                            view.busy,
                            BusyKind::Revert,
                        )
                    })
                }),
        )
        .child(
            // Right: selection actions (Stage/Unstage/Revert selected).
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_1()
                .flex_shrink_0()
                .when(
                    title == "Unstaged" || title == "Untracked" || title == "All",
                    |this| {
                        this.child(
                            with_busy(
                                section_action_label(
                                    raised_action_btn(
                                        SharedString::from(format!(
                                            "section-stage-selected:{title}"
                                        )),
                                        AssetIcon::Plus,
                                        cx,
                                    )
                                    .small(),
                                    t.stage,
                                    t.tip_stage,
                                    hide_labels,
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.stage_selected(window, cx)
                                })),
                                view.busy,
                                BusyKind::Stage,
                            ),
                        )
                    },
                )
                .when(title == "Staged" || title == "All", |this| {
                    this.child(
                        with_busy(
                            section_action_label(
                                raised_action_btn(
                                    "section-unstage-selected",
                                    AssetIcon::Minus,
                                    cx,
                                )
                                .small(),
                                t.unstage,
                                t.tip_unstage,
                                hide_labels,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.unstage_selected(window, cx)
                            })),
                            view.busy,
                            BusyKind::Unstage,
                        ),
                    )
                })
                .when(title == "Unstaged" || title == "All", |this| {
                    this.child({
                        use crate::app::widgets::action_btn;
                        use gpui_kit::component::button::ButtonCustomVariant;
                        with_busy(
                            section_action_label(
                                action_btn("section-revert", AssetIcon::Undo2)
                                    .custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(cx.theme().danger)
                                            .foreground(cx.theme().danger_foreground)
                                            .hover(cx.theme().danger_hover)
                                            .active(cx.theme().danger_active)
                                            .shadow(true),
                                    )
                                    .small(),
                                t.revert_selected,
                                t.tip_revert_selected,
                                hide_labels,
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_revert(window, cx)
                            })),
                            view.busy,
                            BusyKind::Revert,
                        )
                    })
                }),
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
    compact: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let path_for_checkbox = change.path.clone();
    let path_for_row = change.path.clone();
    let path_open = change.path.clone();
    let path_revert = change.path.clone();
    let path_stage = change.path.clone();
    let path_unstage = change.path.clone();
    let show_stage = change.kind != ChangeKind::Staged;
    let show_unstage = change.kind == ChangeKind::Staged;
    let selected = view.selected.contains(&change.path);
    let previewing = view
        .preview_path
        .as_ref()
        .is_some_and(|p| p == &change.path);
    let full_status = display_label(&change.status_label);
    let status = if compact {
        status_letter(&change.status_label)
    } else {
        full_status
    };
    let area = area_label(change.kind);
    let color = status_color(&change.status_label, &view.status_colors, cx);
    let indent = px(8.0 + 14.0 * depth as f32);
    let group = SharedString::from(format!("change-row:{}:{}", area, change.path));

    let build_actions = |cx: &mut Context<AppView>| {
        div()
            .flex()
            .items_center()
            .justify_end()
            .gap_0()
            .child(
                icon_btn(
                    SharedString::from(format!("open:{}", change.path)),
                    IconName::ExternalLink,
                )
                .accessibility_label(t.open)
                .tooltip(t.tip_open)
                .on_click(cx.listener({
                    let path_open = path_open.clone();
                    move |this, _, _, cx| {
                        this.open_path_in_editor(&path_open, cx);
                    }
                })),
            )
            .when(show_stage, |this| {
                this.child(
                    icon_btn(
                        SharedString::from(format!("stage:{}", change.path)),
                        IconName::Plus,
                    )
                    .accessibility_label(t.stage)
                    .tooltip(t.tip_stage_one)
                    .on_click(cx.listener({
                        let path_stage = path_stage.clone();
                        move |this, _, window, cx| {
                            this.stage_one(path_stage.clone(), window, cx);
                        }
                    })),
                )
            })
            .when(show_unstage, |this| {
                this.child(
                    icon_btn(
                        SharedString::from(format!("unstage:{}", change.path)),
                        IconName::Minus,
                    )
                    .accessibility_label(t.unstage)
                    .tooltip(t.tip_unstage_one)
                    .on_click(cx.listener({
                        let path_unstage = path_unstage.clone();
                        move |this, _, window, cx| {
                            this.unstage_one(path_unstage.clone(), window, cx);
                        }
                    })),
                )
            })
            .child(
                icon_danger_btn(
                    SharedString::from(format!("revert:{}", change.path)),
                    IconName::Undo2,
                )
                .accessibility_label(t.revert)
                .tooltip(t.tip_revert)
                .on_click(cx.listener({
                    let path_revert = path_revert.clone();
                    move |this, _, window, cx| {
                        this.confirm_revert_one(path_revert.clone(), window, cx);
                    }
                })),
            )
    };

    div()
        .id(SharedString::from(format!(
            "change:{}:{}",
            area, change.path
        )))
        .group(group.clone())
        .relative()
        .flex()
        .w_full()
        .items_center()
        .justify_start()
        .gap_2()
        .pl(indent)
        .pr_2()
        .py_1()
        .rounded_md()
        .when(selected || previewing, |this| this.bg(cx.theme().accent.opacity(0.25)))
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
                .w(if compact { px(14.) } else { px(72.) })
                .flex_shrink_0()
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
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.preview_file(path_for_row.clone(), window, cx);
                }))
                .child(display_name.to_string()),
        )
        .when(!compact, |this| this.child(build_actions(cx)))
        .when(compact, |this| {
            // Overlay so hidden actions do not steal width from the file name.
            this.child(
                div()
                    .absolute()
                    .right_1()
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .invisible()
                    .group_hover(group, |style| style.visible())
                    .bg(cx.theme().colors.list.opacity(0.92))
                    .child(build_actions(cx)),
            )
        })
}
