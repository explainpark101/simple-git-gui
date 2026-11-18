use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::ContextMenuExt as _;
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
    let node_count = view.files_tree_nodes.len();
    let form = view.file_op_form.clone();
    let entity = cx.entity();

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
                            let entity_list = entity.clone();
                            pane_body(cx)
                                .id("files-tree")
                                .relative()
                                .flex_1()
                                .min_h_0()
                                .w_full()
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
                                .child(
                                    uniform_list("files-tree-vl", node_count, {
                                        let entity = entity_list;
                                        move |range, window, cx| {
                                            let entity = entity.clone();
                                            let view = entity.read(cx);
                                            let compact = view.files_tree_compact;
                                            let t = view.t();
                                            range
                                                .filter_map(|i| view.files_tree_nodes.get(i).cloned())
                                                .map(|node| {
                                                    files_tree_row(
                                                        &entity, &view, node, compact, t, window, cx,
                                                    )
                                                })
                                                .collect()
                                        }
                                    })
                                    .size_full()
                                    .py_1(),
                                )
                        })
                        .into_any_element(),
                ),
        )
        .child(
            // Keep a second panel (and drag handle) even with no open file.
            resizable_panel().child(
                if view.preview_tabs.is_empty() {
                    pane_body(cx).flex_1().min_w_0().into_any_element()
                } else {
                    preview_pane::render(view, true, true, cx).into_any_element()
                },
            ),
        )
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


fn files_tree_row(
    entity: &Entity<AppView>,
    view: &AppView,
    node: crate::app::files::FsNode,
    compact: bool,
    t: crate::app::i18n::UiText,
    _window: &mut Window,
    cx: &App,
) -> AnyElement {
    let _ = compact;
    let rel = node.rel_path.clone();
    let depth = node.depth;
    let indent = px(TREE_INDENT_BASE + TREE_INDENT_STEP * depth as f32);
    let guide_color = cx.theme().border.opacity(0.85);
    let expanded = view.expanded_dirs.contains(&node.rel_path);
    let selected = view
        .preview_path
        .as_ref()
        .is_some_and(|p| p == &node.rel_path);
    let is_dir = node.is_dir;
    let ignored = node.ignored;
    let dim = if ignored { 0.42 } else { 1.0 };
    let fg = if ignored {
        cx.theme().muted_foreground.opacity(0.55)
    } else {
        view.files_status_index.color_for(
            &rel,
            is_dir,
            dim,
            &view.status_colors,
            cx,
        )
    };
    let group = SharedString::from(format!("fs-row:{rel}"));
    let entity_toggle = entity.clone();
    let entity_preview = entity.clone();
    let entity_menu = entity.clone();
    let actions = file_op_icons_entity(entity, &rel, t, dim);

    if is_dir {
        let folder_icon = if expanded {
            AssetIcon::FolderOpen
        } else {
            AssetIcon::FolderClosed
        };
        let rel_toggle = rel.clone();
        let rel_menu = rel.clone();
        div()
            .id(SharedString::from(format!("dir-row:{rel}")))
            .group(group.clone())
            .relative()
            .w_full()
            .h(px(28.))
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            .pl(indent)
            .pr_1()
            .when(ignored, |this| this.opacity(0.55))
            .text_color(fg)
            .child(tree_guides(depth, guide_color))
            .child(
                div()
                    .id(SharedString::from(format!("dir-hit:{rel}")))
                    .flex()
                    .items_center()
                    .justify_start()
                    .gap_1()
                    .min_w_0()
                    .flex_1()
                    .cursor_pointer()
                    .child(div().flex_shrink_0().text_color(fg).child(folder_icon))
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
                    .on_click(move |_, _, cx| {
                        let _ = entity_toggle.update(cx, |this, cx| {
                            this.toggle_dir(rel_toggle.clone(), cx);
                        });
                    }),
            )
            .child(
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
                    .child(actions),
            )
            .context_menu(move |menu, _, _| {
                file_context_menu(
                    menu,
                    &entity_menu,
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
        let rel_menu = rel.clone();
        let rel_open = rel.clone();
        let entity_open = entity.clone();
        div()
            .id(SharedString::from(format!("file:{rel}")))
            .group(group.clone())
            .relative()
            .w_full()
            .h(px(28.))
            .flex()
            .items_center()
            .justify_between()
            .gap_1()
            .pl(indent)
            .pr_1()
            .cursor_pointer()
            .when(ignored, |this| this.opacity(0.55))
            .text_color(fg)
            .when(selected, |this| this.bg(cx.theme().accent.opacity(0.22)))
            .hover(|this| this.bg(cx.theme().muted.opacity(0.4)))
            .on_click(move |_, window, cx| {
                let _ = entity_preview.update(cx, |this, cx| {
                    this.preview_file(rel_preview.clone(), window, cx);
                });
            })
            .child(tree_guides(depth, guide_color))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_start()
                    .gap_1()
                    .min_w_0()
                    .flex_1()
                    .child(div().flex_shrink_0().text_color(fg).child(AssetIcon::File))
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
            .child(
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
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_0()
                            .child(actions)
                            .child(
                                icon_btn(
                                    SharedString::from(format!("file-open:{rel_open}")),
                                    AssetIcon::ExternalLink,
                                )
                                .accessibility_label(t.open)
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    let _ = entity_open.update(cx, |this, cx| {
                                        this.open_path_in_editor(&rel_open, cx);
                                    });
                                }),
                            ),
                    ),
            )
            .context_menu(move |menu, _, _| {
                file_context_menu(
                    menu,
                    &entity_menu,
                    &rel_menu,
                    false,
                    t.new_file,
                    t.new_folder,
                    t.rename_file,
                    t.delete_file,
                    t.move_file,
                    t.open_external,
                )
            })
            .into_any_element()
    }
}

fn file_op_icons_entity(
    entity: &Entity<AppView>,
    rel: &str,
    t: crate::app::i18n::UiText,
    opacity: f32,
) -> impl IntoElement {
    let rel_rename = rel.to_string();
    let rel_delete = rel.to_string();
    let rel_move = rel.to_string();
    let entity_ren = entity.clone();
    let entity_del = entity.clone();
    let entity_mov = entity.clone();
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
            .accessibility_label(t.tip_rename_file)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let _ = entity_ren.update(cx, |this, cx| {
                    this.start_file_rename(rel_rename.clone(), window, cx);
                });
            }),
        )
        .child(
            icon_danger_btn(
                SharedString::from(format!("fs-del:{rel}")),
                AssetIcon::Trash,
            )
            .accessibility_label(t.tip_delete_file)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let _ = entity_del.update(cx, |this, cx| {
                    this.confirm_delete_path(rel_delete.clone(), window, cx);
                });
            }),
        )
        .child(
            icon_btn(
                SharedString::from(format!("fs-mov:{rel}")),
                AssetIcon::FolderInput,
            )
            .accessibility_label(t.tip_move_file)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let _ = entity_mov.update(cx, |this, cx| {
                    this.start_file_move(rel_move.clone(), window, cx);
                });
            }),
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
