use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_resizable, resizable_panel};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, Styled as _, *};

use super::preview_pane;
use super::status_style::TreeStatusIndex;
use crate::app::busy::BusyKind;
use crate::app::files::list_tree;
use crate::app::view::{AppView, FileOpForm};
use crate::app::widgets::{
    action_btn, action_row, confirm_btn, icon_btn, icon_danger_btn, pane_body, pane_header,
    pane_toolbar, with_busy,
};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let root = view.repo_root();
    let root_label = view.repo_root_display();
    let nodes = list_tree(&root, &view.expanded_dirs);
    let form = view.file_op_form.clone();
    let entity = cx.entity();
    let status_index = TreeStatusIndex::from_changes(
        view.snapshot
            .as_ref()
            .map(|s| s.changes.as_slice())
            .unwrap_or(&[]),
    );

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
                                .child(div().font_semibold().text_left().child(t.files))
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
                        .child(
                            pane_body(cx)
                                .id("files-tree")
                                .flex_1()
                                .min_h_0()
                                .w_full()
                                .py_1()
                                .flex()
                                .flex_col()
                                .items_stretch()
                                .overflow_y_scrollbar()
                                .children(nodes.into_iter().map(|node| {
                                    let rel = node.rel_path.clone();
                                    let rel_menu = node.rel_path.clone();
                                    let rel_open = node.rel_path.clone();
                                    let indent = px(8.0 + 14.0 * node.depth as f32);
                                    let expanded =
                                        view.expanded_dirs.iter().any(|p| p == &node.rel_path);
                                    let selected = view
                                        .preview_path
                                        .as_ref()
                                        .is_some_and(|p| p == &node.rel_path);
                                    let entity = entity.clone();
                                    let is_dir = node.is_dir;
                                    let ignored = node.ignored;
                                    let dim = if ignored { 0.8 } else { 1.0 };
                                    let fg = status_index.color_for(&rel, is_dir, dim, cx);

                                    if is_dir {
                                        let folder_icon = if expanded {
                                            AssetIcon::FolderOpen
                                        } else {
                                            AssetIcon::FolderClosed
                                        };
                                        let rel_toggle = rel.clone();
                                        // Button::render hardcodes justify_center — use a div
                                        // row so icon+label stay left-aligned in a full-width item.
                                        div()
                                            .id(SharedString::from(format!("dir-row:{rel}")))
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_1()
                                            .pl(indent)
                                            .pr_1()
                                            .py_1()
                                            .text_color(fg)
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
                                            .child(file_op_icons(
                                                &rel,
                                                t.tip_rename_file,
                                                t.tip_delete_file,
                                                t.tip_move_file,
                                                dim,
                                                cx,
                                            ))
                                            .context_menu(move |menu, _, _| {
                                                file_context_menu(
                                                    menu,
                                                    &entity,
                                                    &rel_menu,
                                                    true,
                                                    t.rename_file,
                                                    t.delete_file,
                                                    t.move_file,
                                                    t.open_external,
                                                )
                                            })
                                            .into_any_element()
                                    } else {
                                        let rel_preview = rel.clone();
                                        div()
                                            .id(SharedString::from(format!("file:{rel}")))
                                            .w_full()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap_1()
                                            .pl(indent)
                                            .pr_1()
                                            .py_1()
                                            .cursor_pointer()
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
                                            .child(
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
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.open_path_in_editor(
                                                                    &rel_open, cx,
                                                                );
                                                            },
                                                        )),
                                                    ),
                                            )
                                            .context_menu({
                                                let entity = entity.clone();
                                                let rel_menu = rel.clone();
                                                move |menu, _, _| {
                                                    file_context_menu(
                                                        menu,
                                                        &entity,
                                                        &rel_menu,
                                                        false,
                                                        t.rename_file,
                                                        t.delete_file,
                                                        t.move_file,
                                                        t.open_external,
                                                    )
                                                }
                                            })
                                            .into_any_element()
                                    }
                                })),
                        )
                        .into_any_element(),
                ),
        )
        .child(
            resizable_panel().child(
                preview_pane::render(view, false, cx).into_any_element(),
            ),
        )
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
    _is_dir: bool,
    rename_label: &'static str,
    delete_label: &'static str,
    move_label: &'static str,
    open_label: &'static str,
) -> gpui_kit::component::menu::PopupMenu {
    let path_ren = path.to_string();
    let path_del = path.to_string();
    let path_mov = path.to_string();
    let path_open = path.to_string();
    let entity_ren = entity.clone();
    let entity_del = entity.clone();
    let entity_mov = entity.clone();
    let entity_open = entity.clone();

    menu.item(
        PopupMenuItem::new(rename_label)
            .icon(AssetIcon::Pencil)
            .on_click(move |_, window, cx| {
                entity_ren.update(cx, |this, cx| {
                    this.start_file_rename(path_ren.clone(), window, cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new(delete_label)
            .icon(AssetIcon::Trash)
            .on_click(move |_, window, cx| {
                entity_del.update(cx, |this, cx| {
                    this.confirm_delete_path(path_del.clone(), window, cx);
                });
            }),
    )
    .item(
        PopupMenuItem::new(move_label)
            .icon(AssetIcon::FolderInput)
            .on_click(move |_, window, cx| {
                entity_mov.update(cx, |this, cx| {
                    this.start_file_move(path_mov.clone(), window, cx);
                });
            }),
    )
    .separator()
    .item(
        PopupMenuItem::new(open_label)
            .icon(AssetIcon::ExternalLink)
            .on_click(move |_, _, cx| {
                entity_open.update(cx, |this, cx| {
                    this.open_path_in_editor(&path_open, cx);
                });
            }),
    )
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
