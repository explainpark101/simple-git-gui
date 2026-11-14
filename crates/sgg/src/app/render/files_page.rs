use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_resizable, resizable_panel};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, Styled as _, *};

use super::preview_pane;
use crate::app::busy::BusyKind;
use crate::app::view::{AppView, FileOpForm};
use crate::app::widgets::{
    action_btn, action_row, confirm_btn, icon_btn, icon_danger_btn, menu_item_with_icon,
    pane_body, pane_header, pane_toolbar, with_busy,
};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let root_label = view.repo_root_display();
    let nodes = view.files_tree_nodes.clone();
    let form = view.file_op_form.clone();
    let entity = cx.entity();
    let status_index = &view.files_status_index;

    h_resizable("files-panes")
        .child(
            resizable_panel()
                .size(px(300.))
                .size_range(px(180.)..px(520.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .child(
                            pane_header(cx)
                                .flex()
                                .flex_col()
                                .gap_1()
                                .px_3()
                                .py_2()
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .gap_2()
                                        .child(div().font_semibold().text_left().child(t.files))
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_end()
                                                .gap_0()
                                                .flex_shrink_0()
                                                .child(
                                                    icon_btn("files-new-file", AssetIcon::FilePlus)
                                                        .accessibility_label(t.new_file)
                                                        .tooltip(t.tip_new_file)
                                                        .on_click(cx.listener(
                                                            |this, _, window, cx| {
                                                                this.start_new_file(
                                                                    String::new(),
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    icon_btn(
                                                        "files-new-folder",
                                                        AssetIcon::FolderPlus,
                                                    )
                                                    .accessibility_label(t.new_folder)
                                                    .tooltip(t.tip_new_folder)
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.start_new_folder(
                                                                String::new(),
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_left()
                                        .text_color(cx.theme().muted_foreground)
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(root_label),
                                ),
                        )
                        .when(!matches!(form, FileOpForm::Hidden), |this| {
                            this.child(file_op_form_panel(view, &form, cx))
                        })
                        .child({
                            let entity_measure = entity.clone();
                            let compact = view.files_tree_compact;
                            pane_body(cx)
                                .id("files-tree")
                                .relative()
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .py_1()
                                .flex()
                                .flex_col()
                                .items_stretch()
                                .overflow_y_scrollbar()
                                .child(
                                    canvas(
                                        move |bounds, _, cx| {
                                            let compact = bounds.size.width < px(260.);
                                            if entity_measure.read_with(cx, |this, _| {
                                                this.files_tree_compact == compact
                                            }) {
                                                return;
                                            }
                                            let _ = entity_measure.update(cx, |this, cx| {
                                                this.set_files_tree_compact(compact, cx);
                                            });
                                        },
                                        |_, _, _, _| {},
                                    )
                                    .absolute()
                                    .size_full()
                                    .invisible(),
                                )
                                .children(nodes.into_iter().map(|node| {
                                    let rel = node.rel_path.clone();
                                    let rel_menu = node.rel_path.clone();
                                    let rel_open = node.rel_path.clone();
                                    let depth = node.depth;
                                    let indent = px(TREE_INDENT_BASE + TREE_INDENT_STEP * depth as f32);
                                    let guide_color = cx.theme().border.opacity(0.85);
                                    let expanded =
                                        view.expanded_dirs.contains(&node.rel_path);
                                    let selected = view
                                        .preview_path
                                        .as_ref()
                                        .is_some_and(|p| p == &node.rel_path);
                                    let entity = entity.clone();
                                    let is_dir = node.is_dir;
                                    let ignored = node.ignored;
                                    let dim = if ignored { 0.42 } else { 1.0 };
                                    let fg = if ignored {
                                        cx.theme().muted_foreground.opacity(0.55)
                                    } else {
                                        status_index.color_for(
                                            &rel,
                                            is_dir,
                                            dim,
                                            &view.status_colors,
                                            cx,
                                        )
                                    };
                                    let group =
                                        SharedString::from(format!("fs-row:{rel}"));
                                    let compact = compact;

                                    if is_dir {
                                        let folder_icon = if expanded {
                                            AssetIcon::FolderOpen
                                        } else {
                                            AssetIcon::FolderClosed
                                        };
                                        let rel_toggle = rel.clone();
                                        div()
                                            .id(SharedString::from(format!("dir-row:{rel}")))
                                            .group(group.clone())
                                            .relative()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_1()
                                            .pl(indent)
                                            .pr_1()
                                            .py_1()
                                            .when(ignored, |this| this.opacity(0.55))
                                            .text_color(fg)
                                            .child(tree_guides(depth, guide_color))
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!("dir:{rel}")))
                                                    .flex()
                                                    .items_center()
                                                    .justify_start()
                                                    .gap_1()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .cursor_pointer()
                                                    .child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .text_color(fg)
                                                            .child(folder_icon),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_left()
                                                            .text_color(fg)
                                                            .whitespace_nowrap()
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .child(node.name.clone()),
                                                    )
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.toggle_dir(rel_toggle.clone(), cx);
                                                    })),
                                            )
                                            .when(!compact, |this| {
                                                this.child(file_op_icons(
                                                    &rel,
                                                    t.tip_rename_file,
                                                    t.tip_delete_file,
                                                    t.tip_move_file,
                                                    dim,
                                                    cx,
                                                ))
                                            })
                                            .when(compact, |this| {
                                                this.child(
                                                    div()
                                                        .absolute()
                                                        .right_1()
                                                        .top_0()
                                                        .bottom_0()
                                                        .flex()
                                                        .items_center()
                                                        .invisible()
                                                        .group_hover(group, |s| s.visible())
                                                        .bg(cx.theme().colors.list.opacity(0.92))
                                                        .child(file_op_icons(
                                                            &rel,
                                                            t.tip_rename_file,
                                                            t.tip_delete_file,
                                                            t.tip_move_file,
                                                            dim,
                                                            cx,
                                                        )),
                                                )
                                            })
                                            .context_menu(move |menu, _, _| {
                                                file_context_menu(
                                                    menu,
                                                    &entity,
                                                    &rel_menu,
                                                    true,
                                                    t.new_file,
                                                    t.new_folder,
                                                    t.rename_file,
                                                    t.delete_file,
                                                    t.move_file,
                                                    t.open_external,
                                                )
                                            })
                                            .into_any_element()
                                    } else {
                                        let rel_preview = rel.clone();
                                        let file_actions = |cx: &mut Context<AppView>| {
                                            div()
                                                .flex()
                                                .items_center()
                                                .justify_end()
                                                .gap_0()
                                                .child(file_op_icons(
                                                    &rel,
                                                    t.tip_rename_file,
                                                    t.tip_delete_file,
                                                    t.tip_move_file,
                                                    dim,
                                                    cx,
                                                ))
                                                .child(
                                                    icon_btn(
                                                        SharedString::from(format!(
                                                            "file-open:{rel_open}"
                                                        )),
                                                        AssetIcon::ExternalLink,
                                                    )
                                                    .accessibility_label(t.open)
                                                    .tooltip(t.tip_open)
                                                    .on_click(cx.listener({
                                                        let rel_open = rel_open.clone();
                                                        move |this, _, _, cx| {
                                                            this.open_path_in_editor(
                                                                &rel_open, cx,
                                                            );
                                                        }
                                                    })),
                                                )
                                        };
                                        div()
                                            .id(SharedString::from(format!("file:{rel}")))
                                            .group(group.clone())
                                            .relative()
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_1()
                                            .pl(indent)
                                            .pr_1()
                                            .py_1()
                                            .cursor_pointer()
                                            .when(ignored, |this| this.opacity(0.55))
                                            .text_color(fg)
                                            .when(selected, |this| {
                                                this.bg(cx.theme().accent.opacity(0.22))
                                            })
                                            .hover(|this| this.bg(cx.theme().muted.opacity(0.4)))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.preview_file(
                                                    rel_preview.clone(),
                                                    window,
                                                    cx,
                                                );
                                            }))
                                            .child(tree_guides(depth, guide_color))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_start()
                                                    .gap_1()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .child(
                                                        div()
                                                            .flex_shrink_0()
                                                            .text_color(fg)
                                                            .child(AssetIcon::File),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_left()
                                                            .text_color(fg)
                                                            .whitespace_nowrap()
                                                            .overflow_hidden()
                                                            .text_ellipsis()
                                                            .child(node.name.clone()),
                                                    ),
                                            )
                                            .when(!compact, |this| this.child(file_actions(cx)))
                                            .when(compact, |this| {
                                                this.child(
                                                    div()
                                                        .absolute()
                                                        .right_1()
                                                        .top_0()
                                                        .bottom_0()
                                                        .flex()
                                                        .items_center()
                                                        .invisible()
                                                        .group_hover(group, |s| s.visible())
                                                        .bg(cx.theme().colors.list.opacity(0.92))
                                                        .child(file_actions(cx)),
                                                )
                                            })
                                            .context_menu({
                                                let entity = entity.clone();
                                                let rel_menu = rel.clone();
                                                move |menu, _, _| {
                                                    file_context_menu(
                                                        menu,
                                                        &entity,
                                                        &rel_menu,
                                                        false,
                                                        t.new_file,
                                                        t.new_folder,
                                                        t.rename_file,
                                                        t.delete_file,
                                                        t.move_file,
                                                        t.open_external,
                                                    )
                                                }
                                            })
                                            .into_any_element()
                                    }
                                }))
                        })
                        .into_any_element(),
                ),
        )
        .when(!view.preview_tabs.is_empty(), |this| {
            this.child(
                resizable_panel().child(
                    preview_pane::render(view, true, true, cx).into_any_element(),
                ),
            )
        })
}

const TREE_INDENT_BASE: f32 = 8.0;
const TREE_INDENT_STEP: f32 = 14.0;

/// Vertical nesting guidelines that run the full row height so deep folders
/// stay easy to track while scrolling.
fn tree_guides(depth: usize, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .children((0..depth).map(move |level| {
            let x = px(TREE_INDENT_BASE + TREE_INDENT_STEP * level as f32 + 6.0);
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(x)
                .w(px(1.))
                .bg(color)
        }))
}

fn file_op_icons(
    rel: &str,
    tip_rename: &'static str,
    tip_delete: &'static str,
    tip_move: &'static str,
    opacity: f32,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let rel_rename = rel.to_string();
    let rel_delete = rel.to_string();
    let rel_move = rel.to_string();
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap_0()
        .flex_shrink_0()
        .opacity(opacity)
        .child(
            icon_btn(
                SharedString::from(format!("fs-ren:{rel}")),
                AssetIcon::Pencil,
            )
            .accessibility_label(tip_rename)
            .tooltip(tip_rename)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.start_file_rename(rel_rename.clone(), window, cx);
            })),
        )
        .child(
            icon_danger_btn(
                SharedString::from(format!("fs-del:{rel}")),
                AssetIcon::Trash,
            )
            .accessibility_label(tip_delete)
            .tooltip(tip_delete)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.confirm_delete_path(rel_delete.clone(), window, cx);
            })),
        )
        .child(
            icon_btn(
                SharedString::from(format!("fs-mov:{rel}")),
                AssetIcon::FolderInput,
            )
            .accessibility_label(tip_move)
            .tooltip(tip_move)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.start_file_move(rel_move.clone(), window, cx);
            })),
        )
}

fn file_context_menu(
    menu: gpui_kit::component::menu::PopupMenu,
    entity: &Entity<AppView>,
    path: &str,
    is_dir: bool,
    new_file_label: &'static str,
    new_folder_label: &'static str,
    rename_label: &'static str,
    delete_label: &'static str,
    move_label: &'static str,
    open_label: &'static str,
) -> gpui_kit::component::menu::PopupMenu {
    let path_ren = path.to_string();
    let path_del = path.to_string();
    let path_mov = path.to_string();
    let path_open = path.to_string();
    let path_new_file = path.to_string();
    let path_new_folder = path.to_string();
    let entity_ren = entity.clone();
    let entity_del = entity.clone();
    let entity_mov = entity.clone();
    let entity_open = entity.clone();
    let entity_new_file = entity.clone();
    let entity_new_folder = entity.clone();

    let menu = if is_dir {
        menu.item(menu_item_with_icon(
            new_file_label,
            AssetIcon::FilePlus,
            move |_, window, cx| {
                entity_new_file.update(cx, |this, cx| {
                    this.start_new_file(path_new_file.clone(), window, cx);
                });
            },
        ))
        .item(menu_item_with_icon(
            new_folder_label,
            AssetIcon::FolderPlus,
            move |_, window, cx| {
                entity_new_folder.update(cx, |this, cx| {
                    this.start_new_folder(path_new_folder.clone(), window, cx);
                });
            },
        ))
        .separator()
    } else {
        menu
    };

    menu.item(menu_item_with_icon(
        rename_label,
        AssetIcon::Pencil,
        move |_, window, cx| {
            entity_ren.update(cx, |this, cx| {
                this.start_file_rename(path_ren.clone(), window, cx);
            });
        },
    ))
    .item(menu_item_with_icon(
        delete_label,
        AssetIcon::Trash,
        move |_, window, cx| {
            entity_del.update(cx, |this, cx| {
                this.confirm_delete_path(path_del.clone(), window, cx);
            });
        },
    ))
    .item(menu_item_with_icon(
        move_label,
        AssetIcon::FolderInput,
        move |_, window, cx| {
            entity_mov.update(cx, |this, cx| {
                this.start_file_move(path_mov.clone(), window, cx);
            });
        },
    ))
    .separator()
    .item(menu_item_with_icon(
        open_label,
        AssetIcon::ExternalLink,
        move |_, _, cx| {
            entity_open.update(cx, |this, cx| {
                this.open_path_in_editor(&path_open, cx);
            });
        },
    ))
}

fn file_op_form_panel(
    view: &AppView,
    form: &FileOpForm,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    let t = view.t();
    let title = match form {
        FileOpForm::Rename { from } => format!("{}: {from}", t.rename_file),
        FileOpForm::Move { from } => format!("{}: {from}", t.move_file),
        FileOpForm::NewFile { parent } if parent.is_empty() => t.new_file.to_string(),
        FileOpForm::NewFile { parent } => format!("{}: {parent}/", t.new_file),
        FileOpForm::NewFolder { parent } if parent.is_empty() => t.new_folder.to_string(),
        FileOpForm::NewFolder { parent } => format!("{}: {parent}/", t.new_folder),
        FileOpForm::Hidden => String::new(),
    };

    pane_toolbar(cx)
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .border_b_1()
        .child(
            div()
                .text_xs()
                .text_left()
                .text_color(cx.theme().muted_foreground)
                .child(title),
        )
        .child(Input::new(&view.file_op_input))
        .child(
            action_row()
                .child(
                    action_btn("file-op-cancel", AssetIcon::Close)
                        .ghost()
                        .small()
                        .label(t.branch_cancel)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_file_op_form(window, cx);
                        })),
                )
                .child({
                    let (icon, busy_kind) = match &view.file_op_form {
                        FileOpForm::Move { .. } => (AssetIcon::FolderInput, BusyKind::FileMove),
                        FileOpForm::NewFile { .. } => (AssetIcon::FilePlus, BusyKind::FileCreate),
                        FileOpForm::NewFolder { .. } => {
                            (AssetIcon::FolderPlus, BusyKind::FolderCreate)
                        }
                        _ => (AssetIcon::Pencil, BusyKind::FileRename),
                    };
                    with_busy(
                        confirm_btn("file-op-confirm", icon, cx)
                            .small()
                            .label(t.branch_confirm)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.confirm_file_op_form(window, cx);
                            })),
                        view.busy,
                        busy_kind,
                    )
                }),
        )
}
