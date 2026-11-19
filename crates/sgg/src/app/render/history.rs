//! Git commit history page.

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Selectable as _, Sizable as _, h_resizable, resizable_panel,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use std::collections::{BTreeMap, HashSet};

use crate::app::busy::BusyKind;
use crate::app::i18n::UiText;
use crate::app::render::git_graph;
use crate::app::render::preview_pane;
use crate::app::render::status_style::history_name_status_key;
use crate::app::view::{AppView, HistoryFileViewMode};
use crate::app::widgets::{action_btn, display_btn, icon_btn, menu_item_with_icon, with_busy};
use sgg_core::{is_uncommitted_oid, HistoryFileChange, ResetMode, UNCOMMITTED_OID};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let selected = view.history_selected.clone();
    let commits = view.history_commits.clone();
    let graph = view.history_graph.clone();
    let bg = cx.theme().background;
    let muted = cx.theme().muted_foreground;
    let has_uncommitted = view.history_has_uncommitted();
    let history_rev = view.history_rev.clone();
    let preview_open = view.history_file_preview.is_some();
    let graph_w = git_graph::graph_width(graph.max_lanes);
    // Min width: keep the "History" / "히스토리" header title on one line
    // (px_3*2 + gap_2 + refresh control + title advance), and never narrower
    // than the graph gutter.
    let list_min = history_list_min_width(t.tab_history, graph_w);
    let list_default = (graph_w + px(300.)).max(px(320.)).max(list_min);

    // Nest like git-work: outer split is list | right. File preview gets its own
    // inner h_resizable so dragging that handle cannot cascade into the graph list
    // (gpui-base steals from earlier siblings once the middle panel hits min size).
    let list_panel = resizable_panel()
        .size(list_default)
        .size_range(list_min..px(640.))
        .child(
            div()
                .id("history-list")
                .size_full()
                .min_h_0()
                .min_w_0()
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .min_w_0()
                                .text_left()
                                .child(
                                    div()
                                        .font_semibold()
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(t.tab_history),
                                )
                                .when_some(history_rev.clone(), |this, rev| {
                                    this.child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(format!("{}: {rev}", t.history_rev_label)),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    icon_btn("history-graph-branches", AssetIcon::Settings)
                                        .accessibility_label(t.tip_history_graph_branches)
                                        .tooltip(t.tip_history_graph_branches)
                                        .selected(view.history_branch_filter_open)
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.toggle_history_branch_filter(window, cx);
                                        })),
                                )
                                .child(
                                    with_busy(
                                        action_btn("history-refresh", AssetIcon::RefreshCw)
                                            .ghost()
                                            .small()
                                            .tooltip(t.tip_refresh_history)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.reload_history(window, cx);
                                            })),
                                        view.busy,
                                        BusyKind::HistoryLoad,
                                    ),
                                ),
                        ),
                )
                .when(view.history_branch_filter_open, |this| {
                    this.child(history_branch_filter_panel(view, t, cx))
                })
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .px_1()
                        .py_1()
                        .when(commits.is_empty() && !has_uncommitted, |this| {
                            this.child(
                                div()
                                    .px_3()
                                    .py_4()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t.history_empty),
                            )
                        })
                        .when(has_uncommitted, |this| {
                            let row_graph = graph.clone();
                            let wip_selected =
                                selected.as_deref() == Some(UNCOMMITTED_OID);
                            this.child(
                                display_btn("hist:uncommitted")
                                    .ghost()
                                    .selected(wip_selected)
                                    .w_full()
                                    .h(px(git_graph::ROW_HEIGHT))
                                    .px_1()
                                    .tooltip(t.tip_history_uncommitted)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.select_history_commit(
                                            UNCOMMITTED_OID.to_string(),
                                            window,
                                            cx,
                                        );
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .items_stretch()
                                            .gap_2()
                                            .w_full()
                                            .h_full()
                                            .min_w_0()
                                            .overflow_hidden()
                                            .child(git_graph::row_canvas(
                                                row_graph, 0, bg, muted,
                                            ))
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .justify_center()
                                                    .gap_1()
                                                    .flex_1()
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .py_1()
                                                    .pr_2()
                                                    .text_left()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .line_height(relative(1.4))
                                                            .text_color(
                                                                cx.theme().muted_foreground,
                                                            )
                                                            .whitespace_nowrap()
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .child(t.history_uncommitted),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .line_height(relative(1.4))
                                                            .text_color(
                                                                cx.theme().muted_foreground,
                                                            )
                                                            .whitespace_nowrap()
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .child(t.tip_history_uncommitted),
                                                    ),
                                            ),
                                    ),
                            )
                        })
                        .children(commits.into_iter().enumerate().map(|(i, c)| {
                            let oid = c.full_oid.clone();
                            let selected = selected.as_deref() == Some(oid.as_str());
                            let row_graph = graph.clone();
                            let row = if has_uncommitted { i + 1 } else { i };
                            let meta = format!("{} · {} · {}", c.short_oid, c.author, c.when);
                            // Two-line hover detail: subject, then meta.
                            let tip = format!("{}\n{meta}", c.subject);
                            display_btn(SharedString::from(format!("hist:{}", c.short_oid)))
                                .ghost()
                                .selected(selected)
                                .w_full()
                                // Fixed row height; graph canvas stretches via h_full so
                                // lane strokes meet across adjacent rows with no gaps.
                                .h(px(git_graph::ROW_HEIGHT))
                                .px_1()
                                .tooltip(tip)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.select_history_commit(oid.clone(), window, cx);
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .items_stretch()
                                        .gap_2()
                                        .w_full()
                                        .h_full()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .child(git_graph::row_canvas(
                                            row_graph, row, bg, muted,
                                        ))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .justify_center()
                                                // Keep subject and meta on separate lines
                                                // with enough gap so glyphs never collide.
                                                .gap_1()
                                                .flex_1()
                                                .min_w_0()
                                                .overflow_hidden()
                                                .py_1()
                                                .pr_2()
                                                .text_left()
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .line_height(relative(1.4))
                                                        .text_left()
                                                        .whitespace_nowrap()
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(c.subject.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .line_height(relative(1.4))
                                                        .text_color(cx.theme().muted_foreground)
                                                        .text_left()
                                                        // Narrow panes: ellipsis instead of
                                                        // wrapping (which clipped under fixed height).
                                                        .whitespace_nowrap()
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(meta),
                                                ),
                                        ),
                                )
                                .into_any_element()
                        })),
                ),
        );

    let detail = detail_pane(view, cx);

    if preview_open {
        h_resizable("history-panes")
            .child(list_panel)
            .child(
                resizable_panel().child(
                    h_resizable("history-detail-preview")
                        .child(
                            resizable_panel()
                                .size(px(420.))
                                .size_range(px(280.)..px(720.))
                                .child(detail),
                        )
                        .child(
                            resizable_panel()
                                .size(px(420.))
                                .size_range(px(280.)..Pixels::MAX)
                                .child(
                                    preview_pane::render(view, true, false, cx).into_any_element(),
                                ),
                        ),
                ),
            )
            .into_any_element()
    } else {
        // Detail must flex (no size / size_range max) so the list handle can move —
        // same pattern as the outer right panel when preview is open. Sizing both
        // sides leaves them fighting max widths and the handle appears stuck.
        h_resizable("history-panes")
            .child(list_panel)
            .child(resizable_panel().child(detail))
            .into_any_element()
    }
}

/// Searchable checkbox list of branches included in the History graph.
fn history_branch_filter_panel(
    view: &AppView,
    t: UiText,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let query = view
        .history_branch_filter_input
        .read(cx)
        .value()
        .to_string()
        .to_lowercase();
    let branches: Vec<(String, bool)> = view
        .snapshot
        .as_ref()
        .map(|s| {
            s.branches
                .iter()
                .filter(|b| query.is_empty() || b.name.to_lowercase().contains(&query))
                .map(|b| (b.name.clone(), view.history_graph_branch_visible(&b.name)))
                .collect()
        })
        .unwrap_or_default();

    div()
        .flex()
        .flex_col()
        .gap_2()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(t.tip_history_graph_branches),
        )
        .child(
            Input::new(&view.history_branch_filter_input)
                .cleanable(true)
                .prefix(Icon::new(AssetIcon::Search).small()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .max_h(px(220.))
                .overflow_y_scrollbar()
                .children(branches.into_iter().map(|(name, visible)| {
                    let branch = name.clone();
                    Checkbox::new(SharedString::from(format!("hist-branch-filter:{name}")))
                        .small()
                        .label(name)
                        .checked(visible)
                        .on_click(cx.listener(move |this, checked, window, cx| {
                            this.set_history_graph_branch_visible(
                                branch.clone(),
                                *checked,
                                window,
                                cx,
                            );
                        }))
                })),
        )
}

/// Minimum History list width so the tab title stays on one row.
///
/// Header chrome: `px_3` × 2 + `gap_2` + small refresh control + title advance.
/// Also at least the git-graph gutter so lane strokes are not clipped.
fn history_list_min_width(title: &str, graph_w: Pixels) -> Pixels {
    const HEADER_PAD_X: f32 = 24.; // px_3 left + right
    const HEADER_GAP: f32 = 8.; // gap_2 between title and refresh
    const REFRESH_BTN: f32 = 32.; // small icon button
    // Approximate default UI font advances (semibold body size).
    let title_w: f32 = title
        .chars()
        .map(|c| if c.is_ascii() { 8.5 } else { 14. })
        .sum();
    let header_min = px(HEADER_PAD_X + HEADER_GAP + REFRESH_BTN + title_w);
    header_min.max(graph_w + px(8.))
}

fn detail_pane(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let Some(detail) = view.history_detail.clone() else {
        return div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(t.history_select_hint)
            .into_any_element();
    };

    let oid = detail.commit.full_oid.clone();
    let is_wip = is_uncommitted_oid(&oid);
    let subject = if is_wip {
        t.history_uncommitted.to_string()
    } else {
        detail.commit.subject.clone()
    };
    let meta = if is_wip {
        format!("{} · {}", detail.commit.short_oid, t.history_uncommitted_meta)
    } else {
        format!(
            "{} · {} <{}> · {}",
            detail.commit.short_oid,
            detail.commit.author,
            detail.commit.email,
            detail.commit.when
        )
    };
    let body = if detail.body.is_empty() {
        String::new()
    } else {
        detail.body.clone()
    };

    div()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_lg()
                        .font_semibold()
                        .text_left()
                        .child(subject),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .text_left()
                        .child(meta),
                )
                .when(!body.is_empty(), |this| {
                    this.child(
                        div()
                            .text_sm()
                            .text_left()
                            .whitespace_normal()
                            .child(body),
                    )
                })
                .when(!is_wip, |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .pt_1()
                            .child(
                                with_busy(
                                    display_btn("hist-reset-soft")
                                        .outline()
                                        .small()
                                        .label(t.reset_soft)
                                        .tooltip(t.tip_reset_soft)
                                        .on_click(cx.listener({
                                            let oid = oid.clone();
                                            move |this, _, window, cx| {
                                                this.confirm_reset(
                                                    oid.clone(),
                                                    ResetMode::Soft,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        })),
                                    view.busy,
                                    BusyKind::ResetSoft,
                                ),
                            )
                            .child(
                                with_busy(
                                    display_btn("hist-reset-mixed")
                                        .outline()
                                        .small()
                                        .label(t.reset_mixed)
                                        .tooltip(t.tip_reset_mixed)
                                        .on_click(cx.listener({
                                            let oid = oid.clone();
                                            move |this, _, window, cx| {
                                                this.confirm_reset(
                                                    oid.clone(),
                                                    ResetMode::Mixed,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        })),
                                    view.busy,
                                    BusyKind::ResetMixed,
                                ),
                            )
                            .child(
                                with_busy(
                                    display_btn("hist-reset-hard")
                                        .outline()
                                        .small()
                                        .label(t.reset_hard)
                                        .tooltip(t.tip_reset_hard)
                                        .on_click(cx.listener({
                                            let oid = oid.clone();
                                            move |this, _, window, cx| {
                                                this.confirm_reset(
                                                    oid.clone(),
                                                    ResetMode::Hard,
                                                    window,
                                                    cx,
                                                );
                                            }
                                        })),
                                    view.busy,
                                    BusyKind::ResetHard,
                                ),
                            )
                            .child(
                                with_busy(
                                    display_btn("hist-cherry-pick")
                                        .outline()
                                        .small()
                                        .label(t.cherry_pick)
                                        .tooltip(t.tip_cherry_pick)
                                        .on_click(cx.listener({
                                            let oid = oid.clone();
                                            move |this, _, window, cx| {
                                                this.confirm_cherry_pick(oid.clone(), window, cx);
                                            }
                                        })),
                                    view.busy,
                                    BusyKind::CherryPick,
                                ),
                            )
                            .child(
                                display_btn("hist-create-tag")
                                    .outline()
                                    .small()
                                    .label(t.create_tag)
                                    .tooltip(t.tip_create_tag)
                                    .on_click(cx.listener({
                                        let oid = oid.clone();
                                        move |this, _, window, cx| {
                                            this.start_create_tag(Some(oid.clone()), window, cx);
                                        }
                                    })),
                            ),
                    )
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_4()
                .py_2()
                .child(
                    div()
                        .font_semibold()
                        .text_sm()
                        .text_left()
                        .child(t.history_files),
                )
                .child(history_files_view_pill(view.history_files_as_tree, t, cx)),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar()
                .px_2()
                .pb_3()
                .when(detail.files.is_empty(), |this| {
                    this.child(
                        div()
                            .px_2()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.history_files_empty),
                    )
                })
                .children({
                    let rows = build_history_file_rows(
                        &detail.files,
                        view.history_files_as_tree,
                        &view.collapsed_history_dirs,
                    );
                    rows.into_iter().enumerate().map(|(i, row)| {
                        render_history_file_row(view, &oid, i, row, t, cx)
                    })
                }),
        )
        .into_any_element()
}

/// Icon pill: flat list vs folder tree for commit changed files.
fn history_files_view_pill(
    as_tree: bool,
    t: UiText,
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
            icon_btn("hist-files-flat", AssetIcon::ListPlus)
                .selected(!as_tree)
                .accessibility_label(t.changes_flat)
                .tooltip(t.tip_changes_flat)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_history_files_as_tree(false, cx);
                })),
        )
        .child(
            div()
                .w(px(1.))
                .self_stretch()
                .bg(cx.theme().border),
        )
        .child(
            icon_btn("hist-files-tree", AssetIcon::FolderOpen)
                .selected(as_tree)
                .accessibility_label(t.changes_tree)
                .tooltip(t.tip_changes_tree)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_history_files_as_tree(true, cx);
                })),
        )
}

#[derive(Clone)]
enum HistoryFileRow {
    Dir {
        name: String,
        key: String,
        depth: usize,
        expanded: bool,
    },
    File {
        status: String,
        path: String,
        label: String,
        depth: usize,
    },
}

fn build_history_file_rows(
    files: &[HistoryFileChange],
    as_tree: bool,
    collapsed: &HashSet<String>,
) -> Vec<HistoryFileRow> {
    if !as_tree {
        return files
            .iter()
            .map(|f| HistoryFileRow::File {
                status: f.status.clone(),
                path: f.path.clone(),
                label: f.path.clone(),
                depth: 0,
            })
            .collect();
    }

    #[derive(Default)]
    struct DirNode {
        children: BTreeMap<String, DirNode>,
        files: Vec<HistoryFileChange>,
    }

    let mut root = DirNode::default();
    for f in files {
        let parts: Vec<&str> = f.path.split('/').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            continue;
        }
        let mut node = &mut root;
        for part in &parts[..parts.len() - 1] {
            node = node.children.entry((*part).to_string()).or_default();
        }
        node.files.push(f.clone());
    }

    fn walk(
        node: &DirNode,
        prefix: &str,
        depth: usize,
        collapsed: &HashSet<String>,
        out: &mut Vec<HistoryFileRow>,
    ) {
        for (name, child) in &node.children {
            let key = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let expanded = !collapsed.contains(&key);
            out.push(HistoryFileRow::Dir {
                name: name.clone(),
                key: key.clone(),
                depth,
                expanded,
            });
            if expanded {
                walk(child, &key, depth + 1, collapsed, out);
            }
        }
        for f in &node.files {
            let label = f
                .path
                .rsplit('/')
                .next()
                .unwrap_or(f.path.as_str())
                .to_string();
            out.push(HistoryFileRow::File {
                status: f.status.clone(),
                path: f.path.clone(),
                label,
                depth,
            });
        }
    }

    let mut out = Vec::new();
    walk(&root, "", 0, collapsed, &mut out);
    out
}

fn render_history_file_row(
    view: &AppView,
    oid: &str,
    i: usize,
    row: HistoryFileRow,
    t: UiText,
    cx: &mut Context<AppView>,
) -> AnyElement {
    match row {
        HistoryFileRow::Dir {
            name,
            key,
            depth,
            expanded,
        } => {
            let indent = px(8.0 + 14.0 * depth as f32);
            let folder = if expanded {
                AssetIcon::FolderOpen
            } else {
                AssetIcon::FolderClosed
            };
            let key_toggle = key.clone();
            div()
                .id(("hist-dir", i))
                .w_full()
                .rounded(cx.theme().radius)
                .child(
                    display_btn(SharedString::from(format!("hist-dir-btn:{i}")))
                        .ghost()
                        .w_full()
                        .px_2()
                        .py_1()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_history_file_dir(key_toggle.clone(), cx);
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .w_full()
                                .min_w_0()
                                .pl(indent)
                                .text_sm()
                                .child(div().flex_shrink_0().child(folder))
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .text_left()
                                        .line_height(relative(1.35))
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .whitespace_nowrap()
                                        .child(name),
                                ),
                        ),
                )
                .into_any_element()
        }
        HistoryFileRow::File {
            status,
            path,
            label,
            depth,
        } => {
            let indent = px(8.0 + 14.0 * depth as f32);
            let path_click = path.clone();
            let path_menu = path.clone();
            let oid_click = oid.to_string();
            let oid_menu = oid.to_string();
            let entity = cx.entity();
            let selected_file = view
                .history_file_preview
                .as_ref()
                .is_some_and(|p| p.path == path && p.oid == oid);
            let status_key = history_name_status_key(&status);
            let status_color = view.effective_status_color(status_key, cx);
            div()
                .id(("hist-file", i))
                .w_full()
                .rounded(cx.theme().radius)
                .when(selected_file, |this| {
                    this.bg(cx.theme().accent.opacity(0.22))
                })
                .child(
                    display_btn(SharedString::from(format!("hist-file-btn:{i}")))
                        .ghost()
                        .w_full()
                        .px_2()
                        .py_1()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_history_file(
                                oid_click.clone(),
                                path_click.clone(),
                                HistoryFileViewMode::Diff,
                                window,
                                cx,
                            );
                        }))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .w_full()
                                .min_w_0()
                                .pl(indent)
                                .text_sm()
                                .child(
                                    div()
                                        .w(px(28.))
                                        .flex_shrink_0()
                                        .text_xs()
                                        .font_semibold()
                                        .text_color(status_color)
                                        .child(status),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .text_left()
                                        .line_height(relative(1.35))
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(label),
                                ),
                        )
                        .context_menu(move |menu, _, _| {
                            let oid_diff = oid_menu.clone();
                            let path_diff = path_menu.clone();
                            let oid_file = oid_menu.clone();
                            let path_file = path_menu.clone();
                            let entity_diff = entity.clone();
                            let entity_file = entity.clone();
                            menu.item(menu_item_with_icon(
                                t.history_view_diff,
                                AssetIcon::FileDiff,
                                move |_, window, cx| {
                                    entity_diff.update(cx, |this, cx| {
                                        this.open_history_file(
                                            oid_diff.clone(),
                                            path_diff.clone(),
                                            HistoryFileViewMode::Diff,
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            ))
                            .item(menu_item_with_icon(
                                t.history_view_file,
                                AssetIcon::File,
                                move |_, window, cx| {
                                    entity_file.update(cx, |this, cx| {
                                        this.open_history_file(
                                            oid_file.clone(),
                                            path_file.clone(),
                                            HistoryFileViewMode::FileAtCommit,
                                            window,
                                            cx,
                                        );
                                    });
                                },
                            ))
                        }),
                )
                .into_any_element()
        }
    }
}
