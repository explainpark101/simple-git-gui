use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _, Size,
    resizable_panel, v_resizable,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, *};
use sgg_core::{BranchInfo, RemoteInfo, RepoOperation};

use crate::app::busy::BusyKind;
use crate::app::i18n::UiText;
use crate::app::ops::RemoteAction;
use crate::app::progress::{ProgressState, PROGRESS_MAX_HEIGHT, PROGRESS_MIN_HEIGHT};
use crate::app::remote_url::remote_browse_url;
use crate::app::view::{AppView, BranchForm, RemoteForm};
use crate::app::widgets::{
    action_btn, action_row, confirm_btn, display_btn, icon_btn, icon_danger_btn, list_row_btn,
    menu_item_with_icon, pane_body, pane_header, with_busy,
};

pub fn render(
    view: &AppView,
    branches: &[BranchInfo],
    remotes: &[RemoteInfo],
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();

    if view.sidebar_collapsed {
        return div()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .size_full()
            .bg(cx.theme().sidebar)
            .py_2()
            .child(
                icon_btn("sidebar-expand", AssetIcon::PanelLeftOpen)
                    .accessibility_label(t.tip_expand_sidebar)
                    .tooltip(t.tip_expand_sidebar)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.toggle_sidebar_collapsed(cx);
                    })),
            )
            .into_any_element();
    }

    let selected_remote = view.selected_remote.clone();
    let branch_form = view.branch_form.clone();
    let remote_form = view.remote_form.clone();
    let remote_form_open = !matches!(remote_form, RemoteForm::Hidden);
    let branch_form_open = !matches!(branch_form, BranchForm::Hidden);
    let remote_compact = view.remote_actions_compact;
    let entity = cx.entity();
    let progress_collapsed = view.progress_collapsed;
    let progress_h = view.progress_panel_height.clamp(
        PROGRESS_MIN_HEIGHT,
        PROGRESS_MAX_HEIGHT,
    );

    div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .bg(cx.theme().sidebar)
        .child(
            pane_header(cx)
                .id("remote-actions-header")
                .relative()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_2()
                .child(
                    canvas(
                        {
                            let entity = entity.clone();
                            move |bounds, _, cx| {
                                // Fetch/Pull/Push labels need room beside the remote chip.
                                let compact = bounds.size.width < px(360.);
                                if entity.read_with(cx, |this, _| {
                                    this.remote_actions_compact == compact
                                }) {
                                    return;
                                }
                                let _ = entity.update(cx, |this, cx| {
                                    this.set_remote_actions_compact(compact, cx);
                                });
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full()
                    .invisible(),
                )
                .child(remote_sync_chip(view, &selected_remote, cx))
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .justify_end()
                        .items_center()
                        .gap_1()
                        .child(
                            with_busy(
                                action_btn("fetch", AssetIcon::CloudDownload)
                                    .outline()
                                    .small()
                                    .when(remote_compact, |this| this.compact())
                                    .when(!remote_compact, |this| this.label(t.fetch))
                                    .accessibility_label(t.fetch)
                                    .tooltip(t.tip_fetch)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_remote_action(
                                            RemoteAction::Fetch,
                                            window,
                                            cx,
                                        )
                                    })),
                                view.busy,
                                BusyKind::Fetch,
                            ),
                        )
                        .child(
                            with_busy(
                                action_btn("pull", AssetIcon::ArrowDownToLine)
                                    .outline()
                                    .small()
                                    .when(remote_compact, |this| this.compact())
                                    .when(!remote_compact, |this| this.label(t.pull))
                                    .accessibility_label(t.pull)
                                    .tooltip(t.tip_pull)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_remote_action(
                                            RemoteAction::Pull,
                                            window,
                                            cx,
                                        )
                                    })),
                                view.busy,
                                BusyKind::Pull,
                            ),
                        )
                        .child(
                            with_busy(
                                action_btn("push", AssetIcon::CloudUpload)
                                    .primary()
                                    .small()
                                    .when(remote_compact, |this| this.compact())
                                    .when(!remote_compact, |this| this.label(t.push))
                                    .accessibility_label(t.push)
                                    .tooltip(t.tip_push)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_remote_action(
                                            RemoteAction::Push,
                                            window,
                                            cx,
                                        )
                                    })),
                                view.busy,
                                BusyKind::Push,
                            ),
                        )
                        .child(
                            icon_btn("sidebar-collapse", AssetIcon::PanelLeftClose)
                                .accessibility_label(t.tip_collapse_sidebar)
                                .tooltip(t.tip_collapse_sidebar)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.toggle_sidebar_collapsed(cx);
                                })),
                        ),
                ),
        )
        .child({
            // Forms are painted as overlays (last children) so Branches / Progress
            // headers cannot cover confirm buttons — GPUI has no CSS z-index.
            let remotes_el = remotes_panel(view, remotes, selected_remote, cx);
            let branches_el = branches_panel(view, branches, cx);

            if progress_collapsed {
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(
                        div().flex_1().min_h_0().child(
                            v_resizable("sidebar-panes")
                                .child(
                                    resizable_panel()
                                        .size(px(140.))
                                        .size_range(px(80.)..px(360.))
                                        .child(remotes_el),
                                )
                                .child(resizable_panel().child(branches_el)),
                        ),
                    )
                    .child(progress_header(view, true, cx))
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_resizable("sidebar-panes-progress")
                            .child(
                                resizable_panel()
                                    .size(px(140.))
                                    .size_range(px(80.)..px(360.))
                                    .child(remotes_el),
                            )
                            .child(resizable_panel().child(branches_el))
                            .child(
                                resizable_panel()
                                    .size(px(progress_h))
                                    .size_range(
                                        px(PROGRESS_MIN_HEIGHT)..px(PROGRESS_MAX_HEIGHT),
                                    )
                                    .child(progress_panel(view, cx)),
                            ),
                    )
                    .into_any_element()
            }
        })
        .when(remote_form_open, |this| {
            this.child(form_overlay(
                "remote-form-overlay",
                FormOverlayAnchor::Top,
                progress_collapsed,
                progress_h,
                remote_form_panel(view, &remote_form, cx).into_any_element(),
                cx,
            ))
        })
        .when(branch_form_open, |this| {
            this.child(form_overlay(
                "branch-form-overlay",
                FormOverlayAnchor::AboveProgress,
                progress_collapsed,
                progress_h,
                branch_form_panel(view, &branch_form, cx).into_any_element(),
                cx,
            ))
        })
        .into_any_element()
}

fn remote_sync_chip(
    view: &AppView,
    selected_remote: &str,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let head = view
        .snapshot
        .as_ref()
        .and_then(|s| s.branches.iter().find(|b| b.is_head && !b.is_remote));
    let ahead = head.map(|b| b.ahead).unwrap_or(0);
    let behind = head.map(|b| b.behind).unwrap_or(0);
    let changes_count = view
        .snapshot
        .as_ref()
        .map(|s| s.changes.len())
        .unwrap_or(0);
    let operation = view
        .snapshot
        .as_ref()
        .map(|s| s.operation)
        .unwrap_or(RepoOperation::Clean);

    div()
        .min_w_0()
        .flex_1()
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .rounded(cx.theme().radius)
        .bg(cx.theme().accent.opacity(0.28))
        .child(
            display_btn("selected-remote")
                .ghost()
                .px_0()
                .py_0()
                .min_w_0()
                .flex_shrink_1()
                .font_semibold()
                .text_sm()
                .label(selected_remote.to_string())
                .tooltip(selected_remote.to_string()),
        )
        .when(behind > 0, |this| {
            let tip = t.tip_sync_behind.replace("{n}", &behind.to_string());
            this.child(sync_count_badge(
                "sync-behind",
                AssetIcon::ArrowDown,
                behind,
                cx.theme().danger,
                tip,
            ))
        })
        .when(ahead > 0, |this| {
            let tip = t.tip_sync_ahead.replace("{n}", &ahead.to_string());
            this.child(sync_count_badge(
                "sync-ahead",
                AssetIcon::ArrowUp,
                ahead,
                cx.theme().warning,
                tip,
            ))
        })
        // Dirty working tree with nothing to push yet → yellow status.
        .when(changes_count > 0 && ahead == 0, |this| {
            let tip = t.tip_sync_dirty.replace("{n}", &changes_count.to_string());
            this.child(sync_count_badge(
                "sync-dirty",
                AssetIcon::FileDiff,
                changes_count,
                cx.theme().warning,
                tip,
            ))
        })
        .when(operation != RepoOperation::Clean, |this| {
            this.child(operation_badge(operation, t, cx))
        })
}

fn sync_count_badge(
    id: &'static str,
    icon: AssetIcon,
    count: usize,
    color: Hsla,
    tip: impl Into<SharedString>,
) -> impl IntoElement {
    display_btn(id)
        .ghost()
        .compact()
        .small()
        .flex_shrink_0()
        .tooltip(tip)
        .child(
            div()
                .flex()
                .items_center()
                .gap_0()
                .text_color(color)
                .child(Icon::new(icon).small().text_color(color))
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(color)
                        .child(count.to_string()),
                ),
        )
}

fn operation_badge(
    operation: RepoOperation,
    t: UiText,
    cx: &App,
) -> impl IntoElement {
    let (label, tip, icon, color) = match operation {
        RepoOperation::Merge => (
            t.op_merge,
            t.tip_op_merge,
            AssetIcon::GitMerge,
            cx.theme().warning,
        ),
        RepoOperation::Rebase => (
            t.op_rebase,
            t.tip_op_rebase,
            AssetIcon::Redo2,
            cx.theme().magenta,
        ),
        RepoOperation::CherryPick => (
            t.op_cherry_pick,
            t.tip_op_cherry_pick,
            AssetIcon::GitPullRequestCreateArrow,
            cx.theme().accent,
        ),
        RepoOperation::Revert => (
            t.op_revert,
            t.tip_op_revert,
            AssetIcon::Undo2,
            cx.theme().warning,
        ),
        RepoOperation::Bisect => (
            t.op_bisect,
            t.tip_op_bisect,
            AssetIcon::Split,
            cx.theme().accent,
        ),
        RepoOperation::Other => (
            t.op_other,
            t.tip_op_other,
            AssetIcon::CircleAlert,
            cx.theme().danger,
        ),
        RepoOperation::Clean => unreachable!(),
    };

    display_btn("repo-operation")
        .ghost()
        .compact()
        .small()
        .flex_shrink_0()
        .tooltip(tip)
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .px_1()
                .rounded(cx.theme().radius)
                .bg(color.opacity(0.22))
                .text_color(color)
                .child(Icon::new(icon).small().text_color(color))
                .child(
                    div()
                        .text_xs()
                        .font_semibold()
                        .text_color(color)
                        .child(label),
                ),
        )
}

enum FormOverlayAnchor {
    /// Under the Fetch/Pull/Push chrome — covers Remotes list / Branches header.
    Top,
    /// Just above the Progress dock so confirm stays clickable.
    AboveProgress,
}

fn form_overlay(
    id: &'static str,
    anchor: FormOverlayAnchor,
    progress_collapsed: bool,
    progress_h: f32,
    body: AnyElement,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let progress_chrome = if progress_collapsed {
        36.0
    } else {
        progress_h
    };
    let card = div()
        .w_full()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_md()
        .occlude()
        .child(body);

    div()
        .id(id)
        .absolute()
        .left_2()
        .right_2()
        .when(matches!(anchor, FormOverlayAnchor::Top), |this| {
            // Sit under the remote-actions header (~48px).
            this.top(px(48.))
        })
        .when(matches!(anchor, FormOverlayAnchor::AboveProgress), |this| {
            this.bottom(px(progress_chrome + 4.))
        })
        .child(card)
}

fn progress_header(
    view: &AppView,
    collapsed: bool,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let busy = view.busy.is_some();
    let chevron = if collapsed {
        IconName::ChevronUp
    } else {
        IconName::ChevronDown
    };

    pane_header(cx)
        .id("progress-header")
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .gap_2()
        .px_3()
        .py_1()
        .cursor_pointer()
        .border_t_1()
        .border_color(cx.theme().sidebar_border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(
                    div()
                        .font_semibold()
                        .text_sm()
                        .text_left()
                        .child(t.progress),
                )
                .when(busy, |this| {
                    this.child(Spinner::new().with_size(Size::Small))
                }),
        )
        .child(div().flex_shrink_0().child(chevron))
        .on_click(cx.listener(|this, _, _, cx| {
            this.toggle_progress_collapsed(cx);
        }))
}

fn progress_panel(view: &AppView, cx: &mut Context<AppView>) -> AnyElement {
    let t = view.t();
    let log_empty = view.progress_log.is_empty();

    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .bg(cx.theme().sidebar)
        .child(progress_header(view, false, cx))
        .child(
            pane_body(cx)
                .id("progress-log")
                .flex_1()
                .min_h_0()
                .px_2()
                .py_1()
                .flex()
                .flex_col()
                .gap_1()
                .items_start()
                .overflow_y_scrollbar()
                .when(log_empty, |this| {
                    this.child(
                        div()
                            .px_1()
                            .py_1()
                            .text_xs()
                            .text_left()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.progress_empty),
                    )
                })
                .children(view.progress_log.iter().rev().map(|entry| {
                    let (color, mark) = match entry.state {
                        ProgressState::Running => (cx.theme().accent, "…"),
                        ProgressState::Done => (cx.theme().success, "✓"),
                        ProgressState::Failed => (cx.theme().danger, "✗"),
                    };
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_0()
                        .px_1()
                        .py_1()
                        .rounded(cx.theme().radius)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_semibold()
                                        .text_color(color)
                                        .child(mark),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .font_semibold()
                                        .text_left()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(entry.title.clone()),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_left()
                                .font_family(".GitHub Mono, Cascadia Mono, Consolas, monospace")
                                .text_color(cx.theme().muted_foreground)
                                .whitespace_normal()
                                .child(entry.detail.clone()),
                        )
                        .into_any_element()
                })),
        )
        .into_any_element()
}

fn remotes_panel(
    view: &AppView,
    remotes: &[RemoteInfo],
    selected_remote: String,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let t = view.t();
    let form = view.remote_form.clone();
    let entity = cx.entity();
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            pane_header(cx)
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .py_2()
                .child(div().font_semibold().text_left().child(t.remotes))
                .child(
                    icon_btn("remote-new", IconName::Plus)
                        .accessibility_label(t.add_remote)
                        .tooltip(t.tip_add_remote)
                        .selected(matches!(form, RemoteForm::Add | RemoteForm::Edit { .. }))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_add_remote_form(window, cx);
                        })),
                ),
        )
        .child(
            div()
                .id("remote-list")
                .flex_1()
                .min_h_0()
                .px_2()
                .pb_2()
                .gap_1()
                .flex()
                .flex_col()
                .items_start()
                .overflow_y_scroll()
                .children(remotes.iter().map(|r| {
                    let name = r.name.clone();
                    let name_select = name.clone();
                    let selected = name == selected_remote;
                    let url = r.url.clone();
                    let text = match &url {
                        Some(u) => format!("{} — {}", r.name, u),
                        None => r.name.clone(),
                    };
                    let tip = text.clone();
                    let browseable = url
                        .as_deref()
                        .and_then(remote_browse_url)
                        .is_some();
                    let entity = entity.clone();
                    div()
                        .id(SharedString::from(format!("remote-row:{}", r.name)))
                        .w_full()
                        .flex()
                        .justify_start()
                        .rounded(cx.theme().radius)
                        .when(selected, |this| this.bg(cx.theme().accent.opacity(0.28)))
                        .child(
                            list_row_btn(
                                SharedString::from(format!("remote:{}", r.name)),
                                text,
                            )
                            .w_full()
                            .tooltip(tip)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_remote(name_select.clone(), cx);
                            })),
                        )
                        .context_menu(move |menu, _, _| {
                            remote_context_menu(
                                menu,
                                &entity,
                                &name,
                                url.clone(),
                                browseable,
                                t.edit_remote,
                                t.delete_remote,
                                t.open_remote_url,
                            )
                        })
                        .into_any_element()
                })),
        )
        .into_any_element()
}

fn remote_form_panel(
    view: &AppView,
    form: &RemoteForm,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let title = match form {
        RemoteForm::Add => t.add_remote,
        RemoteForm::Edit { name } => name.as_str(),
        RemoteForm::Hidden => "",
    };
    let busy_kind = match form {
        RemoteForm::Edit { .. } => BusyKind::EditRemote,
        _ => BusyKind::AddRemote,
    };
    let icon = match form {
        RemoteForm::Edit { .. } => AssetIcon::Pencil,
        _ => AssetIcon::Plus,
    };

    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(title.to_string()),
        )
        .child(Input::new(&view.remote_name_input))
        .child(Input::new(&view.remote_url_input))
        .child(
            action_row()
                .child(
                    action_btn("remote-form-cancel", AssetIcon::Close)
                        .ghost()
                        .small()
                        .label(t.branch_cancel)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_remote_form(window, cx);
                        })),
                )
                .child(with_busy(
                    confirm_btn("remote-form-confirm", icon, cx)
                        .small()
                        .label(t.branch_confirm)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm_remote_form(window, cx);
                        })),
                    view.busy,
                    busy_kind,
                )),
        )
}

fn remote_context_menu(
    menu: gpui_kit::component::menu::PopupMenu,
    entity: &Entity<AppView>,
    name: &str,
    url: Option<String>,
    browseable: bool,
    edit_label: &'static str,
    delete_label: &'static str,
    open_label: &'static str,
) -> gpui_kit::component::menu::PopupMenu {
    let name_edit = name.to_string();
    let name_del = name.to_string();
    let url_edit = url.clone();
    let entity_edit = entity.clone();
    let entity_del = entity.clone();
    let entity_open = entity.clone();

    let menu = menu
        .item(menu_item_with_icon(
            edit_label,
            AssetIcon::Pencil,
            move |_, window, cx| {
                entity_edit.update(cx, |this, cx| {
                    this.start_edit_remote(name_edit.clone(), url_edit.clone(), window, cx);
                });
            },
        ))
        .item(menu_item_with_icon(
            delete_label,
            AssetIcon::Trash,
            move |_, window, cx| {
                entity_del.update(cx, |this, cx| {
                    this.confirm_delete_remote(name_del.clone(), window, cx);
                });
            },
        ));

    if browseable {
        if let Some(raw_url) = url {
            menu.separator().item(menu_item_with_icon(
                open_label,
                AssetIcon::ExternalLink,
                move |_, _, cx| {
                    entity_open.update(cx, |this, cx| {
                        this.open_remote_in_browser(&raw_url, cx);
                    });
                },
            ))
        } else {
            menu
        }
    } else {
        menu
    }
}

fn branches_panel(
    view: &AppView,
    branches: &[BranchInfo],
    cx: &mut Context<AppView>,
) -> AnyElement {
    let t = view.t();
    let form = view.branch_form.clone();
    let show_remote = view.show_remote_branches;
    let search_compact = view.branch_search_compact;
    let search_open = view.branch_search_open;
    let has_changes = view
        .snapshot
        .as_ref()
        .map(|s| !s.changes.is_empty())
        .unwrap_or(false);
    let query = view
        .branch_search_input
        .read(cx)
        .value()
        .trim()
        .to_lowercase();
    let visible: Vec<&BranchInfo> = branches
        .iter()
        .filter(|b| show_remote || !b.is_remote)
        .filter(|b| query.is_empty() || b.name.to_lowercase().contains(&query))
        .collect();
    let entity = cx.entity();
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            pane_header(cx)
                .id("branches-header")
                .relative()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .child(
                    canvas(
                        {
                            let entity = entity.clone();
                            move |bounds, _, cx| {
                                // Title + actions leave room for an inline search field.
                                let compact = bounds.size.width < px(280.);
                                if entity.read_with(cx, |this, _| {
                                    this.branch_search_compact == compact
                                }) {
                                    return;
                                }
                                let _ = entity.update(cx, |this, cx| {
                                    this.set_branch_search_compact(compact, cx);
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
                        .font_semibold()
                        .text_left()
                        .flex_shrink_0()
                        .child(t.branches),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .when(!search_compact, |this| {
                            this.child(
                                Input::new(&view.branch_search_input)
                                    .cleanable(true)
                                    .prefix(Icon::new(IconName::Search).small()),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_0()
                        .when(search_compact, |this| {
                            this.child(
                                icon_btn("branch-search-toggle", IconName::Search)
                                    .accessibility_label(t.tip_branch_search)
                                    .tooltip(t.tip_branch_search)
                                    .selected(search_open)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_branch_search_open(window, cx);
                                    })),
                            )
                        })
                        .child(
                            icon_btn("branch-new", IconName::Plus)
                                .accessibility_label(t.create_switch)
                                .tooltip(t.tip_new_branch)
                                .selected(matches!(
                                    form,
                                    BranchForm::Create | BranchForm::Rename { .. }
                                ))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_create_branch_form(window, cx);
                                })),
                        )
                        .child(
                            icon_btn("branch-more", IconName::Ellipsis)
                                .accessibility_label(t.tip_branch_more)
                                .tooltip(t.tip_branch_more)
                                .dropdown_menu_with_anchor(Anchor::BottomRight, {
                                    let entity = entity.clone();
                                    let label = t.show_remote_branches;
                                    let tip = t.tip_show_remote_branches;
                                    move |menu, _, cx| {
                                        let show_remote = entity
                                            .read(cx)
                                            .show_remote_branches;
                                        let entity = entity.clone();
                                        menu.item(PopupMenuItem::element(move |_, _| {
                                            Switch::new("branch-menu-show-remote")
                                                .label(label)
                                                .checked(show_remote)
                                                .tooltip(tip)
                                                .on_click({
                                                    let entity = entity.clone();
                                                    move |checked, _, cx| {
                                                        let _ = entity.update(cx, |this, cx| {
                                                            this.set_show_remote_branches(
                                                                *checked, cx,
                                                            );
                                                        });
                                                    }
                                                })
                                        }))
                                    }
                                }),
                        ),
                ),
        )
        .when(search_compact && search_open, |this| {
            this.child(
                div()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .border_b_1()
                    .border_color(cx.theme().sidebar_border)
                    .bg(cx.theme().sidebar)
                    .child(
                        Input::new(&view.branch_search_input)
                            .cleanable(true)
                            .prefix(Icon::new(IconName::Search).small()),
                    ),
            )
        })
        .child(
            pane_body(cx)
                .id("branch-list")
                .flex_1()
                .min_h_0()
                .p_2()
                .gap_1()
                .flex()
                .flex_col()
                .items_start()
                .overflow_y_scroll()
                .children(
                    visible
                        .into_iter()
                        .map(|b| branch_row(b, has_changes, view, cx)),
                ),
        )
        .into_any_element()
}

fn branch_form_panel(
    view: &AppView,
    form: &BranchForm,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let title = match form {
        BranchForm::Create => t.create_switch,
        BranchForm::Rename { from } => from.as_str(),
        BranchForm::Hidden => "",
    };

    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(title.to_string()),
        )
        .child(Input::new(&view.branch_input))
        .child(
            action_row()
                .child(
                    action_btn("branch-form-cancel", AssetIcon::Close)
                        .ghost()
                        .small()
                        .label(t.branch_cancel)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_branch_form(window, cx);
                        })),
                )
                .child({
                    let busy_kind = match &view.branch_form {
                        BranchForm::Rename { .. } => BusyKind::RenameBranch,
                        _ => BusyKind::CreateBranch,
                    };
                    let icon = match &view.branch_form {
                        BranchForm::Rename { .. } => AssetIcon::Pencil,
                        _ => AssetIcon::GitBranchPlus,
                    };
                    with_busy(
                        confirm_btn("branch-form-confirm", icon, cx)
                            .small()
                            .label(t.branch_confirm)
                            .tooltip(t.tip_create_branch)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_branch_form(window, cx);
                            })),
                        view.busy,
                        busy_kind,
                    )
                }),
        )
}

fn branch_row(
    branch: &BranchInfo,
    has_changes: bool,
    view: &AppView,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let t = view.t();
    let name = branch.name.clone();
    let name_switch = name.clone();
    let name_rename = name.clone();
    let name_delete = name.clone();
    let label = if branch.is_remote {
        format!("remotes/{}", branch.name)
    } else {
        branch.name.clone()
    };
    let sync = branch_sync_ui(branch, has_changes, t, cx);
    let tip = if branch.is_remote {
        format!("remotes/{}", branch.name)
    } else {
        match &sync {
            Some(sync) => format!("{}\n{}", branch.name, sync.tip),
            None => branch.name.clone(),
        }
    };
    let can_switch = !branch.is_remote && !branch.is_head;
    let can_rename = !branch.is_remote;
    let can_delete = !branch.is_remote && !branch.is_head;
    let head_color = if has_changes {
        cx.theme().warning
    } else {
        cx.theme().success
    };

    div()
        .id(SharedString::from(format!("branch-row:{}", branch.name)))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_1()
        .rounded(cx.theme().radius)
        .when(branch.is_head, |this| this.bg(cx.theme().accent.opacity(0.28)))
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_1()
                .when(branch.is_head, |this| {
                    this.child(
                        div()
                            .flex_shrink_0()
                            .child(
                                Icon::new(AssetIcon::CircleCheck)
                                    .small()
                                    .text_color(head_color),
                            ),
                    )
                })
                .when(!branch.is_head && !branch.is_remote, |this| {
                    // Align names with HEAD rows that show a check icon.
                    this.child(div().w(px(14.)).flex_shrink_0())
                })
                .child(
                    list_row_btn(
                        SharedString::from(format!("branch:{}", branch.name)),
                        label,
                    )
                    .flex_1()
                    .min_w_0()
                    .tooltip(tip.clone())
                    // Remote rows stay non-interactive; HEAD uses accent bg instead of disabled dim.
                    .disabled(branch.is_remote)
                    .when(can_switch, |this| {
                        this.on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_switch_branch(name_switch.clone(), window, cx);
                        }))
                    }),
                )
                .when_some(sync, |this, sync| {
                    this.child(branch_status_dot(
                        SharedString::from(format!("branch-sync:{}", name)),
                        sync.color,
                        sync.tip,
                    ))
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .gap_0()
                .when(can_rename, |this| {
                    this.child(
                        icon_btn(
                            SharedString::from(format!("branch-ren:{}", name)),
                            AssetIcon::Pencil,
                        )
                        .accessibility_label(t.tip_rename_branch)
                        .tooltip(t.tip_rename_branch)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.start_rename_branch(name_rename.clone(), window, cx);
                        })),
                    )
                })
                .when(can_delete, |this| {
                    this.child(
                        icon_danger_btn(
                            SharedString::from(format!("branch-del:{}", name)),
                            AssetIcon::Trash,
                        )
                        .accessibility_label(t.tip_delete_branch)
                        .tooltip(t.tip_delete_branch)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.confirm_delete_branch(name_delete.clone(), window, cx);
                        })),
                    )
                }),
        )
        .into_any_element()
}

struct BranchSyncUi {
    color: Hsla,
    tip: String,
}

fn branch_sync_ui(
    branch: &BranchInfo,
    has_changes: bool,
    t: UiText,
    cx: &App,
) -> Option<BranchSyncUi> {
    if branch.is_remote {
        return None;
    }
    let theme = cx.theme();
    let Some(upstream) = branch.upstream.as_deref() else {
        // No remote: gray when clean, yellow when the working tree is dirty.
        return Some(if has_changes {
            BranchSyncUi {
                color: theme.warning,
                tip: t.tip_branch_dirty.to_string(),
            }
        } else {
            BranchSyncUi {
                color: theme.muted_foreground,
                tip: t.tip_branch_no_remote.to_string(),
            }
        });
    };
    let ahead = branch.ahead;
    let behind = branch.behind;
    let (color, tip) = match (ahead > 0, behind > 0) {
        (false, false) if has_changes => (theme.warning, t.tip_branch_dirty.to_string()),
        (false, false) => (
            theme.success,
            t.tip_branch_in_sync.replace("{upstream}", upstream),
        ),
        (true, false) => (
            theme.warning,
            t.tip_branch_ahead
                .replace("{ahead}", &ahead.to_string())
                .replace("{upstream}", upstream),
        ),
        (false, true) => (
            theme.danger,
            t.tip_branch_behind
                .replace("{behind}", &behind.to_string())
                .replace("{upstream}", upstream),
        ),
        (true, true) => (
            theme.magenta,
            t.tip_branch_diverged
                .replace("{ahead}", &ahead.to_string())
                .replace("{behind}", &behind.to_string())
                .replace("{upstream}", upstream),
        ),
    };
    Some(BranchSyncUi { color, tip })
}

/// Soft traffic-light status glow after the branch name.
fn branch_status_dot(
    id: impl Into<ElementId>,
    color: Hsla,
    tip: impl Into<SharedString>,
) -> impl IntoElement {
    display_btn(id)
        .ghost()
        .compact()
        .small()
        .flex_shrink_0()
        .tooltip(tip)
        .child(
            div()
                .size(px(10.))
                .rounded_full()
                .bg(color.opacity(0.35))
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(px(6.)).rounded_full().bg(color)),
        )
}
