use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::input::Textarea;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{InteractiveElement as _, Styled as _, *};

use crate::app::files::list_tree;
use crate::app::view::AppView;
use crate::app::widgets::btn;

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let root = view.repo_root();
    let root_label = view.repo_root_display();
    let nodes = list_tree(&root, &view.expanded_dirs);
    let preview_title = match &view.preview_path {
        Some(path) => path.clone(),
        None => t.no_file_selected.into(),
    };

    div()
        .flex()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_col()
                .w(px(300.))
                .min_w(px(220.))
                .border_r_1()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
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
                .child(
                    div()
                        .id("files-tree")
                        .flex_1()
                        .min_h_0()
                        .py_1()
                        .flex()
                        .flex_col()
                        .overflow_y_scrollbar()
                        .children(nodes.into_iter().map(|node| {
                            let rel = node.rel_path.clone();
                            let rel_open = node.rel_path.clone();
                            let indent = px(8.0 + 14.0 * node.depth as f32);
                            let expanded =
                                view.expanded_dirs.iter().any(|p| p == &node.rel_path);
                            let selected = view
                                .preview_path
                                .as_ref()
                                .is_some_and(|p| p == &node.rel_path);

                            if node.is_dir {
                                let mark = if expanded { "▾" } else { "▸" };
                                div()
                                    .id(SharedString::from(format!("dir-row:{rel}")))
                                    .pl(indent)
                                    .pr_1()
                                    .child(
                                        btn(SharedString::from(format!("dir:{rel}")))
                                            .ghost()
                                            .small()
                                            .label(format!("{mark} {}", node.name))
                                            .tooltip(t.tip_expand_dir)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.toggle_dir(rel.clone(), cx);
                                            })),
                                    )
                                    .into_any_element()
                            } else {
                                div()
                                    .id(SharedString::from(format!("file:{rel}")))
                                    .flex()
                                    .items_center()
                                    .justify_start()
                                    .gap_1()
                                    .pl(indent)
                                    .pr_1()
                                    .py_1()
                                    .cursor_pointer()
                                    .when(selected, |this| {
                                        this.bg(cx.theme().accent.opacity(0.22))
                                    })
                                    .hover(|this| this.bg(cx.theme().muted.opacity(0.4)))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.preview_file(rel.clone(), window, cx);
                                    }))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_sm()
                                            .text_left()
                                            .whitespace_nowrap()
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .child(node.name.clone()),
                                    )
                                    .child(
                                        btn(SharedString::from(format!("tree-open:{rel_open}")))
                                            .ghost()
                                            .small()
                                            .label(t.open)
                                            .tooltip(t.tip_open)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.open_path_in_editor(&rel_open, cx);
                                            })),
                                    )
                                    .into_any_element()
                            }
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
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
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .font_semibold()
                                .text_left()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(preview_title),
                        )
                        .child(
                            btn("preview-save")
                                .primary()
                                .small()
                                .label(t.save)
                                .tooltip(t.tip_save_preview)
                                .disabled(view.preview_path.is_none())
                                .on_click(cx.listener(|this, _, _, cx| this.save_preview(cx))),
                        )
                        .when_some(view.preview_path.clone(), |this, path| {
                            this.child(
                                btn("preview-open-ext")
                                    .outline()
                                    .small()
                                    .label(t.open_external)
                                    .tooltip(t.tip_open_external)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_path_in_editor(&path, cx);
                                    })),
                            )
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .p_2()
                        .child(Textarea::new(&view.preview_input).h(relative(1.))),
                ),
        )
}