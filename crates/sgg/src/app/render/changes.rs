use std::ops::Range;

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, IconName, Sizable as _, h_resizable, resizable_panel,
    v_resizable,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, *};
use sgg_core::{ChangeKind, FileChange};

use super::status_style::{area_label, display_label, status_color, status_letter};
use crate::app::busy::BusyKind;
use crate::app::i18n::UiText;
use crate::app::sort::{ChangeFlatRow, ChangeSort};
use crate::app::view::AppView;
use crate::app::widgets::{
    confirm_btn, display_btn, icon_btn, icon_danger_btn, menu_item_with_icon, pane_body, pane_header,
    pane_toolbar, raised_action_btn, with_busy,
};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let sort = view.change_sort.normalized();
    let group_by_status = view.group_by_status;
    let section_count = view.changes_sections.len();
    let staged_idxs: Vec<usize> = (0..section_count)
        .filter(|&i| view.changes_sections[i].title == "Staged")
        .collect();
    let working_idxs: Vec<usize> = (0..section_count)
        .filter(|&i| view.changes_sections[i].title != "Staged")
        .collect();
    let all_idxs: Vec<usize> = (0..section_count).collect();

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
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .child({
                            let entity = cx.entity();
                            let has_repo = view.has_repo();
                            div()
                                .flex()
                                .items_center()
                                .justify_end()
                                .px_1()
                                .pt_1()
                                .child(
                                    div()
                                        .context_menu(move |menu, _, _| {
                                            let entity_gen = entity.clone();
                                            let entity_settings = entity.clone();
                                            menu.item(menu_item_with_icon(
                                                t.commit_ai_menu_generate,
                                                AssetIcon::Sparkles,
                                                move |_, window, cx| {
                                                    entity_gen.update(cx, |this, cx| {
                                                        this.generate_commit_message(window, cx);
                                                    });
                                                },
                                            ))
                                            .item(menu_item_with_icon(
                                                t.commit_ai_open_settings,
                                                AssetIcon::Settings,
                                                move |_, window, cx| {
                                                    entity_settings.update(cx, |this, cx| {
                                                        this.open_commit_ai_settings(window, cx);
                                                    });
                                                },
                                            ))
                                        })
                                        .child(with_busy(
                                            icon_btn("generate-commit-ai", AssetIcon::Sparkles)
                                                .accessibility_label(t.generate_commit_message)
                                                .tooltip(t.tip_generate_commit_message)
                                                .disabled(!has_repo)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.generate_commit_message(window, cx);
                                                })),
                                            view
                                                .is_generating_commit_message()
                                                .then_some(BusyKind::GenerateCommitMessage),
                                            BusyKind::GenerateCommitMessage,
                                        )),
                                )
                        })
                        .child(
                            div()
                                .w_full()
                                .px_2()
                                .pb_2()
                                .child(
                                    Textarea::new(&view.commit_input)
                                        .appearance(true)
                                        .border_1()
                                        .border_color(cx.theme().border)
                                        .rounded(cx.theme().radius),
                                ),
                        ),
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
            // Top sections are always Staged vs working; split does not need status subgroups.
            let use_split = view.changes_split_view;
            let measure = canvas(
                {
                    let entity = entity.clone();
                    move |bounds, _, cx| {
                        let pane_w = bounds.size.width;
                        // Progressive density (widest → narrowest), per column:
                        //   1) compact — letter status + row actions on hover
                        //   2) drop split — stack sections in one column
                        // Section header icon-only is measured per section header.
                        // Compact MUST be reachable while split is on: with split at
                        // 720px, each col is ~360px, so row compact at 420px still fires.
                        const ROW_COMPACT_W: f32 = 420.;
                        const SPLIT_MIN_W: f32 = 720.;
                        let split = pane_w >= px(SPLIT_MIN_W);
                        let col_w = if split {
                            pane_w / 2.
                        } else {
                            pane_w
                        };
                        let compact = col_w < px(ROW_COMPACT_W);
                        // Skip entity.update during layout when flags are unchanged.
                        let same = entity.read_with(cx, |this, _| {
                            this.changes_list_compact == compact
                                && this.changes_split_view == split
                        });
                        if same {
                            return;
                        }
                        let _ = entity.update(cx, |this, cx| {
                            this.set_changes_density(compact, split, cx);
                        });
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full()
            .invisible();

            if use_split {
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
                                            &working_idxs,
                                            compact,
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
                                        &staged_idxs,
                                        compact,
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
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .p_2()
                    .child(measure)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .child(section_stack(
                                "changes-stack",
                                view,
                                &all_idxs,
                                compact,
                                t,
                                cx,
                            )),
                    )
                    .into_any_element()
            }
        })
}

fn changes_column(
    id: &'static str,
    view: &AppView,
    section_idxs: &[usize],
    compact: bool,
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
        .child(
            div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .child(section_stack(
                    SharedString::from(format!("{id}-stack")),
                    view,
                    section_idxs,
                    compact,
                    t,
                    cx,
                )),
        )
}

/// Stack of status sections with vertical resize handles between them.
fn section_stack(
    stack_id: impl Into<ElementId>,
    view: &AppView,
    section_idxs: &[usize],
    compact: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let panels = render_section_panels(view, section_idxs, compact, t, cx);
    match panels.len() {
        0 => div().size_full().min_h_0().into_any_element(),
        1 => div()
            .size_full()
            .min_h_0()
            .child(panels.into_iter().next().unwrap())
            .into_any_element(),
        _ => {
            let last = panels.len() - 1;
            let mut group = v_resizable(stack_id);
            for (i, panel) in panels.into_iter().enumerate() {
                let slot = if i == last {
                    // Last panel absorbs remaining height.
                    resizable_panel().child(panel)
                } else {
                    resizable_panel()
                        .size(px(180.))
                        .size_range(px(72.)..px(900.))
                        .child(panel)
                };
                group = group.child(slot);
            }
            group.into_any_element()
        }
    }
}

/// One panel per status section: sticky header + virtualized body.
fn render_section_panels(
    view: &AppView,
    section_idxs: &[usize],
    compact: bool,
    t: UiText,
    cx: &mut Context<AppView>,
) -> Vec<AnyElement> {
    let entity = cx.entity();
    section_idxs
        .iter()
        .copied()
        .filter_map(|section_idx| {
            let section = view.changes_sections.get(section_idx)?;
            let title = section.title;
            let item_count = section.items.len();
            let title_key = title.to_string();
            let expanded = !view.collapsed_change_sections.contains(&title_key);
            let row_count = view
                .changes_flat_rows
                .get(section_idx)
                .map(|r| r.len())
                .unwrap_or(0);

            Some(
                div()
                    .id(SharedString::from(format!("section-panel:{title}")))
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_hidden()
                    .child(section_header(
                        view, title, item_count, expanded, t, cx,
                    ))
                    .when(expanded, |this| {
                        let entity = entity.clone();
                        this.child(
                            uniform_list(
                                SharedString::from(format!("section-vl:{title}")),
                                row_count,
                                move |range: Range<usize>, window, cx| {
                                    render_flat_range(
                                        &entity, section_idx, range, compact, t, window, cx,
                                    )
                                },
                            )
                            .flex_1()
                            .size_full()
                            .min_h_0()
                            .px_1()
                            .py_1(),
                        )
                    })
                    .into_any_element(),
            )
        })
        .collect()
}

fn render_flat_range(
    entity: &Entity<AppView>,
    section_idx: usize,
    range: Range<usize>,
    compact: bool,
    t: UiText,
    _window: &mut Window,
    cx: &mut App,
) -> Vec<AnyElement> {
    let view = entity.read(cx);
    let rows = view
        .changes_flat_rows
        .get(section_idx)
        .map(|r| r.as_slice())
        .unwrap_or(&[]);
    range
        .filter_map(|i| rows.get(i).cloned())
        .map(|row| match row {
            ChangeFlatRow::Empty => div()
                .px_2()
                .py_1()
                .h(px(28.))
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(t.section_empty)
                .into_any_element(),
            ChangeFlatRow::StatusGroup { label, count } => div()
                .w_full()
                .h(px(24.))
                .flex()
                .items_center()
                .px_2()
                .text_xs()
                .font_semibold()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{label} ({count})"))
                .into_any_element(),
            ChangeFlatRow::Dir {
                name,
                path: _,
                key,
                depth,
                child_paths,
            } => {
                let dir_expanded = !view.collapsed_change_dirs.contains(&key);
                let folder_icon = if dir_expanded {
                    IconName::FolderOpen
                } else {
                    IconName::FolderClosed
                };
                let indent = px(8.0 + 14.0 * depth as f32);
                let key_toggle = key.clone();
                let all_selected = !child_paths.is_empty()
                    && child_paths.iter().all(|p| view.selected.contains(p));
                let entity_dir = entity.clone();
                let entity_cb = entity.clone();
                let child_paths_cb = child_paths.clone();
                div()
                    .id(SharedString::from(format!("chdir:{key}")))
                    .w_full()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_start()
                    .gap_1()
                    .pl(indent)
                    .pr_1()
                    .cursor_pointer()
                    .child(
                        Checkbox::new(SharedString::from(format!("cb-dir:{key}")))
                            .checked(all_selected)
                            .on_click(move |checked, _, cx| {
                                cx.stop_propagation();
                                let _ = entity_cb.update(cx, |this, cx| {
                                    this.set_selected_many(&child_paths_cb, *checked, cx);
                                });
                            }),
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
                    .on_click(move |_, _, cx| {
                        let _ = entity_dir.update(cx, |this, cx| {
                            this.toggle_change_dir(key_toggle.clone(), cx);
                        });
                    })
                    .into_any_element()
            }
            ChangeFlatRow::File {
                change,
                display_name,
                depth,
            } => change_row(entity, &view, &change, &display_name, depth, compact, t, cx)
                .into_any_element(),
        })
        .collect()
}

fn stage_filter_row(
    view: &AppView,
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
            with_busy(
                raised_action_btn("filter-stage-all", AssetIcon::ListPlus, cx)
                    .small()
                    .label(t.stage_all)
                    .tooltip(t.tip_stage_all)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.stage_all(window, cx);
                    })),
                view.busy,
                BusyKind::StageAll,
            ),
        )
        .child(
            with_busy(
                raised_action_btn("filter-unstage-all", AssetIcon::ListMinus, cx)
                    .small()
                    .label(t.unstage_all)
                    .tooltip(t.tip_unstage_all)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.unstage_all(window, cx);
                    })),
                view.busy,
                BusyKind::UnstageAll,
            ),
        )
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
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let label = section_title(title, t);
    let title_toggle = title.to_string();
    let hide_labels = view
        .changes_section_hide_labels
        .get(title)
        .copied()
        .unwrap_or(false);
    let chevron = if expanded {
        AssetIcon::ChevronDown
    } else {
        AssetIcon::ChevronRight
    };
    let entity = cx.entity();
    let measure_title = title.to_string();

    pane_header(cx)
        .id(SharedString::from(format!("section-head:{title}")))
        .relative()
        .w_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py_1()
        .child(
            canvas(
                {
                    let entity = entity.clone();
                    move |bounds, _, cx| {
                        // Icon-only when this section header itself is narrow
                        // (split columns / stacked pane — measured per section).
                        const HIDE_LABELS_W: f32 = 520.;
                        let hide = bounds.size.width < px(HIDE_LABELS_W);
                        if entity.read_with(cx, |this, _| {
                            this.changes_section_hide_labels
                                .get(measure_title.as_str())
                                .copied()
                                .unwrap_or(false)
                                == hide
                        }) {
                            return;
                        }
                        let title = measure_title.clone();
                        let _ = entity.update(cx, |this, cx| {
                            this.set_section_hide_labels(&title, hide, cx);
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
                .when(
                    title == "Unstaged"
                        || title == "Untracked"
                        || title == "Staged"
                        || title == "All"
                        || title == "Conflicted",
                    |this| {
                        let all_selected = view.section_all_selected(title);
                        let title_sel = title.to_string();
                        let (label, tip) = if all_selected {
                            (t.deselect_all_section, t.tip_deselect_all_section)
                        } else {
                            (t.select_all_section, t.tip_select_all_section)
                        };
                        this.child(
                            section_action_label(
                                display_btn(SharedString::from(format!(
                                    "section-select-all:{title}"
                                )))
                                .outline()
                                .small()
                                .icon(if all_selected {
                                    AssetIcon::ListMinus
                                } else {
                                    AssetIcon::ListPlus
                                }),
                                label,
                                tip,
                                hide_labels,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_select_section(title_sel.clone(), cx);
                            })),
                        )
                    },
                )
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
    entity: &Entity<AppView>,
    view: &AppView,
    change: &FileChange,
    display_name: &str,
    depth: usize,
    compact: bool,
    t: UiText,
    cx: &App,
) -> impl IntoElement {
    let path = change.path.clone();
    let path_cb = path.clone();
    let path_preview = path.clone();
    let path_open = path.clone();
    let path_revert = path.clone();
    let path_stage = path.clone();
    let path_unstage = path.clone();
    let show_stage = change.kind != ChangeKind::Staged;
    let show_unstage = change.kind == ChangeKind::Staged;
    let selected = view.selected.contains(&change.path);
    let previewing = view
        .git_preview_path
        .as_ref()
        .is_some_and(|p| p == &change.path);
    let status = if compact {
        status_letter(&change.status_label)
    } else {
        display_label(&change.status_label)
    };
    let area = area_label(change.kind);
    let color = status_color(&change.status_label, &view.status_colors, cx);
    let indent = px(8.0 + 14.0 * depth as f32);
    let group = SharedString::from(format!("change-row:{}:{}", area, change.path));

    // Hover-reveal actions only (one set). No per-button tooltips — those
    // register MouseMove listeners on every row and dominate hit-testing cost.
    let entity_open = entity.clone();
    let entity_stage = entity.clone();
    let entity_unstage = entity.clone();
    let entity_revert = entity.clone();
    let actions = div()
        .flex()
        .items_center()
        .justify_end()
        .gap_0()
        .child(
            icon_btn(
                SharedString::from(format!("open:{path}")),
                IconName::ExternalLink,
            )
            .accessibility_label(t.open)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                let _ = entity_open.update(cx, |this, cx| {
                    this.open_path_in_editor(&path_open, cx);
                });
            }),
        )
        .when(show_stage, |this| {
            this.child(
                icon_btn(
                    SharedString::from(format!("stage:{path}")),
                    IconName::Plus,
                )
                .accessibility_label(t.stage)
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let _ = entity_stage.update(cx, |this, cx| {
                        this.stage_one(path_stage.clone(), window, cx);
                    });
                }),
            )
        })
        .when(show_unstage, |this| {
            this.child(
                icon_btn(
                    SharedString::from(format!("unstage:{path}")),
                    IconName::Minus,
                )
                .accessibility_label(t.unstage)
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let _ = entity_unstage.update(cx, |this, cx| {
                        this.unstage_one(path_unstage.clone(), window, cx);
                    });
                }),
            )
        })
        .child(
            icon_danger_btn(
                SharedString::from(format!("revert:{path}")),
                IconName::Undo2,
            )
            .accessibility_label(t.revert)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let _ = entity_revert.update(cx, |this, cx| {
                    this.confirm_revert_one(path_revert.clone(), window, cx);
                });
            }),
        );

    let entity_cb = entity.clone();
    let entity_row = entity.clone();
    div()
        .id(SharedString::from(format!("change:{area}:{path}")))
        .group(group.clone())
        .relative()
        .flex()
        .w_full()
        .h(px(28.))
        .items_center()
        .justify_start()
        .gap_2()
        .pl(indent)
        .pr_2()
        .rounded_md()
        .when(selected || previewing, |this| this.bg(cx.theme().accent.opacity(0.25)))
        // Single row hover listener (not also on path/buttons).
        .hover(|this| this.bg(cx.theme().muted.opacity(0.45)))
        .child(
            Checkbox::new(SharedString::from(format!("cb:{area}:{path}")))
                .checked(selected)
                .on_click(move |checked, _, cx| {
                    let _ = entity_cb.update(cx, |this, cx| {
                        this.set_selected(path_cb.clone(), *checked, cx);
                    });
                }),
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
                .flex_1()
                .min_w_0()
                .text_sm()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .text_left()
                .child(display_name.to_string()),
        )
        .on_click(move |_, window, cx| {
            let _ = entity_row.update(cx, |this, cx| {
                this.preview_file(path_preview.clone(), window, cx);
            });
        })
        .child(
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
                .child(actions),
        )
}
