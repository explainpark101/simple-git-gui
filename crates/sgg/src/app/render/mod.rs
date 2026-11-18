mod changes;
mod files_page;
mod git_graph;
mod history;
mod preview_pane;
mod settings;
mod sidebar;
pub(super) mod status_style;
mod toolbar;

use std::path::Path;

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{h_resizable, resizable_panel, v_resizable};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::page::AppPage;
use super::terminal::{TERMINAL_MAX_HEIGHT, TERMINAL_MIN_HEIGHT};
use super::view::AppView;
use crate::app::busy::BusyKind;
use crate::app::widgets::{action_btn, icon_btn, list_row_btn, with_busy};
use crate::app::{
    ClosePreview, HistoryBack, HistoryForward, NewTerminalTab, SavePreview, ToggleTerminal,
};
use sgg_core::{BranchInfo, RemoteInfo};

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Borrow snapshot slices — avoid cloning large vecs every frame.
        let empty_branches: &[BranchInfo] = &[];
        let empty_remotes: &[RemoteInfo] = &[];
        let branches = self
            .snapshot
            .as_ref()
            .map(|s| s.branches.as_slice())
            .unwrap_or(empty_branches);
        let remotes = self
            .snapshot
            .as_ref()
            .map(|s| s.remotes.as_slice())
            .unwrap_or(empty_remotes);
        let focus = self.focus_handle(cx);
        let preview_open = self.preview_path.is_some();
        let sidebar_collapsed = self.sidebar_collapsed;
        let terminal_visible = self.terminal.visible;

        div()
            .id("sgg-root")
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&focus)
            .key_context("AppView")
            .tab_index(0)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &HistoryBack, window, cx| {
                this.history_back(window, cx);
            }))
            .on_action(cx.listener(|this, _: &HistoryForward, window, cx| {
                this.history_forward(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SavePreview, window, cx| {
                this.save_preview(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ClosePreview, window, cx| {
                this.close_preview(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &NewTerminalTab, window, cx| {
                this.new_terminal_tab(window, cx);
            }))
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, window, cx| this.history_back(window, cx)),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, window, cx| this.history_forward(window, cx)),
            )
            .child(toolbar::render(self, cx))
            .when_some(self.error.clone(), |this, err| {
                this.child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(cx.theme().danger)
                        .text_color(cx.theme().background)
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .text_left()
                                .line_height(relative(1.35))
                                .child(err),
                        )
                        .child(
                            icon_btn("dismiss-error", AssetIcon::Close)
                                .tooltip(self.t().tip_dismiss_error)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.clear_error(cx);
                                })),
                        ),
                )
            })
            .child({
                let main = div().size_full().min_h_0().w_full().child(if !self.has_repo()
                    && self.page != AppPage::Settings
                {
                    welcome(self, cx).into_any_element()
                } else {
                    match self.page {
                        AppPage::Git => {
                            // Changes min: wide enough that sort + stage-actions dropdown
                            // do not overlap when the preview pane is open.
                            // Sidebar min: remote header (fetch/pull/push + collapse) stays usable.
                            let work = if preview_open {
                                h_resizable("git-work")
                                    .child(
                                        resizable_panel()
                                            .size(px(360.))
                                            .size_range(px(340.)..px(560.))
                                            .child(
                                                changes::render(self, cx).into_any_element(),
                                            ),
                                    )
                                    .child(
                                        resizable_panel().child(
                                            preview_pane::render(self, true, false, cx)
                                                .into_any_element(),
                                        ),
                                    )
                                    .into_any_element()
                            } else {
                                changes::render(self, cx).into_any_element()
                            };

                            h_resizable("git-panes")
                                .child(
                                    resizable_panel()
                                        .size(if sidebar_collapsed {
                                            px(48.)
                                        } else {
                                            px(300.)
                                        })
                                        .size_range(if sidebar_collapsed {
                                            px(40.)..px(56.)
                                        } else {
                                            px(280.)..px(480.)
                                        })
                                        .child(
                                            sidebar::render(self, branches, remotes, cx),
                                        ),
                                )
                                .child(resizable_panel().child(work))
                                .into_any_element()
                        }
                        AppPage::History => history::render(self, cx).into_any_element(),
                        AppPage::Files => files_page::render(self, cx).into_any_element(),
                        AppPage::Settings => settings::render(self, cx).into_any_element(),
                    }
                });

                if terminal_visible {
                    let term_h = self.terminal.height();
                    let entity = cx.entity();
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .child(
                            v_resizable("app-main-terminal")
                                .child(resizable_panel().child(main))
                                .child(
                                    resizable_panel()
                                        .size(px(term_h))
                                        .size_range(
                                            px(TERMINAL_MIN_HEIGHT)..px(TERMINAL_MAX_HEIGHT),
                                        )
                                        .child(crate::app::terminal::render_panel(self, cx)),
                                )
                                .on_resize(move |state, window, cx| {
                                    let height = state
                                        .read(cx)
                                        .sizes()
                                        .last()
                                        .copied()
                                        .map(f32::from)
                                        .unwrap_or(term_h);
                                    entity.update(cx, |this, cx| {
                                        this.set_terminal_height(height, window, cx);
                                    });
                                }),
                        )
                        .into_any_element()
                } else {
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .child(main)
                        .into_any_element()
                }
            })
    }
}

fn welcome(view: &AppView, cx: &mut Context<AppView>) -> impl IntoElement {
    let t = view.t();
    let recent: Vec<String> = view.recent_repos.iter().take(5).cloned().collect();
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .gap_4()
        .px_8()
        .child(
            div()
                .text_xl()
                .font_semibold()
                .text_left()
                .child(t.welcome_title),
        )
        .child(
            div()
                .max_w(px(420.))
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .text_center()
                .child(t.welcome_body),
        )
        .child(
            with_busy(
                action_btn("welcome-open-folder", AssetIcon::FolderOpen)
                    .primary()
                    .label(t.open_folder)
                    .tooltip(t.tip_open_folder)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_folder(window, cx)
                    })),
                view.busy,
                BusyKind::OpenFolder,
            ),
        )
        .when(!recent.is_empty(), |this| {
            this.child(
                div()
                    .w_full()
                    .max_w(px(480.))
                    .mt_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_left()
                            .text_color(cx.theme().muted_foreground)
                            .child(t.recent_projects),
                    )
                    .children(recent.into_iter().enumerate().map(|(i, path)| {
                        let label = recent_repo_label(&path);
                        let open_path = path.clone();
                        list_row_btn(
                            SharedString::from(format!("welcome-recent:{i}")),
                            label,
                        )
                        .tooltip(path)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_repo_path(std::path::PathBuf::from(open_path.clone()), cx);
                        }))
                        .into_any_element()
                    })),
            )
        })
}

fn recent_repo_label(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|name| format!("{name}  —  {path}"))
        .unwrap_or_else(|| path.to_string())
}
