//! Confirm dialogs with pointer cursors on footer buttons.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::command::{Command, CommandItem, CommandState};
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex};
use gpui_kit::*;

use super::busy::BusyKind;
use super::view::AppView;
use super::widgets::{btn, confirm_btn, with_busy};

/// Open a confirm alert whose OK/Cancel buttons use `cursor: pointer`.
///
/// `on_ok` returning `true` closes the dialog. Prefer [`open_busy_confirm`] when
/// the OK action should show a loading spinner before the dialog closes.
pub fn open_confirm(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    cancel_text: impl Into<SharedString>,
    danger: bool,
    on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
) {
    // Rapid re-clicks on the same confirm trigger must not stack modals.
    if window.has_active_dialog(cx) {
        return;
    }

    let title = title.into();
    let description = description.into();
    let ok_text = ok_text.into();
    let cancel_text = cancel_text.into();
    let on_ok = Rc::new(on_ok);

    window.open_alert_dialog(cx, move |alert, _, _| {
        let on_ok = on_ok.clone();
        let mut ok = btn("alert-ok").label(ok_text.clone());
        ok = if danger {
            ok.danger()
        } else {
            ok.primary()
        };
        ok = ok.on_click(move |ev, window, cx| {
            if on_ok(ev, window, cx) {
                window.close_dialog(cx);
            }
        });

        alert
            .title(title.clone())
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(
                        DialogClose::new().trigger(|b| {
                            b.outline()
                                .cursor_pointer()
                                .label(cancel_text.clone())
                        }),
                    )
                    .child(ok),
            )
    });
}

/// Confirm alert for long / network git ops (fetch, pull, push).
///
/// Closes the dialog immediately and runs `work`, which must manage its own
/// [`BusyKind`] + Progress dock (typically via a background `git` spawn).
/// Do **not** wrap `work` in [`AppView::run_busy`]: that runs on the UI thread
/// and would freeze the app for the duration of `git push` / credential prompts.
pub fn open_busy_confirm(
    window: &mut Window,
    cx: &mut App,
    app: Entity<AppView>,
    kind: BusyKind,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    cancel_text: impl Into<SharedString>,
    ok_icon: AssetIcon,
    work: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) {
    if window.has_active_dialog(cx) {
        return;
    }

    let title = title.into();
    let description = description.into();
    let cancel_text = cancel_text.into();
    let ok = BusyConfirmOk::mount(
        cx,
        app.clone(),
        kind,
        ok_text.into(),
        ok_icon,
        Rc::new(work),
    );
    let cancel = BusyConfirmCancel::mount(cx, app, kind, cancel_text);

    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(cancel.clone())
                    .child(ok.clone()),
            )
    });
}

/// Update-available confirm: OK downloads; Later persists `skipped_update_version`.
///
/// Do **not** wrap download in [`BusyConfirmOk`] / `run_busy`: download is async and
/// manages its own `BusyKind::DownloadUpdate` + Progress dock. `run_busy` would clear
/// busy and finish the Running line as soon as the spawn returns.
pub fn open_update_confirm(
    window: &mut Window,
    cx: &mut App,
    app: Entity<AppView>,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    later_text: impl Into<SharedString>,
    skip_version: String,
) {
    if window.has_active_dialog(cx) {
        return;
    }

    let title = title.into();
    let description = description.into();
    let ok = UpdatePromptDownload::mount(cx, app.clone(), ok_text.into());
    let later = UpdatePromptLater::mount(cx, app, later_text.into(), skip_version);

    window.open_alert_dialog(cx, move |alert, _, _| {
        alert
            .title(title.clone())
            .description(description.clone())
            .footer(
                DialogFooter::new()
                    .child(later.clone())
                    .child(ok.clone()),
            )
    });
}

pub(super) struct BusyConfirmOk {
    app: Entity<AppView>,
    kind: BusyKind,
    label: SharedString,
    icon: AssetIcon,
    work: Rc<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>,
}

impl BusyConfirmOk {
    pub(super) fn mount(
        cx: &mut App,
        app: Entity<AppView>,
        kind: BusyKind,
        label: SharedString,
        icon: AssetIcon,
        work: Rc<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>,
    ) -> Entity<Self> {
        cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            Self {
                app,
                kind,
                label,
                icon,
                work,
            }
        })
    }
}

impl Render for BusyConfirmOk {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.app.read(cx).busy;
        let loading = busy == Some(self.kind);
        let blocked = busy.is_some();
        let app = self.app.clone();
        let kind = self.kind;
        let work = self.work.clone();

        with_busy(
            confirm_btn("alert-ok-busy", self.icon, cx)
                .label(self.label.clone())
                .disabled(blocked && !loading)
                .on_click(move |_, window, cx| {
                    // Close confirm immediately; progress continues in the sidebar Progress dock.
                    // `work` owns busy lifecycle (async network git) — do not call run_busy.
                    let _ = kind;
                    window.close_dialog(cx);
                    app.update(cx, |this, cx| {
                        work(this, window, cx);
                    });
                }),
            busy,
            kind,
        )
    }
}

pub(super) struct BusyConfirmCancel {
    app: Entity<AppView>,
    kind: BusyKind,
    label: SharedString,
}

impl BusyConfirmCancel {
    pub(super) fn mount(
        cx: &mut App,
        app: Entity<AppView>,
        kind: BusyKind,
        label: SharedString,
    ) -> Entity<Self> {
        cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            Self { app, kind, label }
        })
    }
}

impl Render for BusyConfirmCancel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = self.app.read(cx).busy == Some(self.kind);
        btn("alert-cancel-busy")
            .outline()
            .label(self.label.clone())
            .disabled(loading)
            .on_click(move |_, window, cx| {
                if !loading {
                    window.close_dialog(cx);
                }
            })
    }
}

/// "Download & open" on the update dialog — starts async download (own busy/progress).
pub(super) struct UpdatePromptDownload {
    app: Entity<AppView>,
    label: SharedString,
}

impl UpdatePromptDownload {
    pub(super) fn mount(
        cx: &mut App,
        app: Entity<AppView>,
        label: SharedString,
    ) -> Entity<Self> {
        cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            Self { app, label }
        })
    }
}

impl Render for UpdatePromptDownload {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.app.read(cx).busy;
        let loading = busy == Some(BusyKind::DownloadUpdate);
        let blocked = busy.is_some();
        let app = self.app.clone();

        with_busy(
            confirm_btn("update-prompt-download", AssetIcon::CloudDownload, cx)
                .label(self.label.clone())
                .disabled(blocked && !loading)
                .on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                    app.update(cx, |this, cx| {
                        this.download_available_update(cx);
                    });
                }),
            busy,
            BusyKind::DownloadUpdate,
        )
    }
}

/// "Later" on the update dialog — remember this version so silent checks stay quiet.
pub(super) struct UpdatePromptLater {
    app: Entity<AppView>,
    label: SharedString,
    version: String,
}

impl UpdatePromptLater {
    pub(super) fn mount(
        cx: &mut App,
        app: Entity<AppView>,
        label: SharedString,
        version: String,
    ) -> Entity<Self> {
        cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            Self { app, label, version }
        })
    }
}

impl Render for UpdatePromptLater {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = self.app.read(cx).busy == Some(BusyKind::DownloadUpdate);
        let app = self.app.clone();
        let version = self.version.clone();
        btn("update-prompt-later")
            .outline()
            .label(self.label.clone())
            .disabled(loading)
            .on_click(move |_, window, cx| {
                if loading {
                    return;
                }
                app.update(cx, |this, cx| {
                    this.skip_update_version(version.clone(), cx);
                });
                window.close_dialog(cx);
            })
    }
}

/// Skip / dismiss for the fetch-on-open dialog (applies "don't ask" prefs, no fetch).
pub(super) struct FetchPromptSkip {
    app: Entity<AppView>,
    label: SharedString,
}

impl FetchPromptSkip {
    pub(super) fn mount(
        cx: &mut App,
        app: Entity<AppView>,
        label: SharedString,
    ) -> Entity<Self> {
        cx.new(|cx| {
            cx.observe(&app, |_, _, cx| cx.notify()).detach();
            Self { app, label }
        })
    }
}

impl Render for FetchPromptSkip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = self.app.read(cx).busy == Some(BusyKind::Fetch);
        let app = self.app.clone();
        btn("fetch-prompt-skip")
            .outline()
            .label(self.label.clone())
            .disabled(loading)
            .on_click(move |_, window, cx| {
                if loading {
                    return;
                }
                app.update(cx, |this, cx| {
                    this.apply_fetch_prompt_prefs(cx);
                });
                window.close_dialog(cx);
            })
    }
}

/// Spotlight / Raycast-style palette: search recent repos or type a path to open.
pub fn open_recent_projects_picker(window: &mut Window, cx: &mut Context<AppView>) {
    if window.has_active_dialog(cx) {
        return;
    }

    let view = cx.entity();
    let paths = view.read(cx).recent_repos.clone();
    let t = view.read(cx).t();
    let title = t.recent_projects_picker_title;
    let placeholder = t.recent_projects_picker_placeholder;
    let open_path_fmt = t.recent_projects_picker_open_path;
    let empty_label = t.recent_projects_picker_empty;
    let hint = t.recent_projects_picker_hint;
    let state = cx.new(|cx| CommandState::new(window, cx));

    window.open_dialog(cx, move |dialog, window, cx| {
        state.update(cx, |state, cx| state.focus(window, cx));

        let state = state.clone();
        let view = view.clone();
        let paths = paths.clone();
        let open_path_fmt = open_path_fmt;
        let empty_label = empty_label;
        let title = title;
        let hint = hint;

        dialog
            .close_button(false)
            .overlay_closable(true)
            .margin_top(px(96.))
            .w(px(560.))
            .p_0()
            .content(move |content, _, cx| {
                let query = state.read(cx).query(cx);
                let trimmed = query.trim().to_string();
                let has_typed_path = !trimmed.is_empty();

                let mut items: Vec<CommandItem> = paths
                    .iter()
                    .map(|path| recent_picker_item(path))
                    .collect();

                if has_typed_path {
                    let open_label = open_path_fmt.replace("{path}", &trimmed);
                    items.push(
                        CommandItem::new()
                            .label(open_label)
                            .keywords([trimmed.clone()])
                            .icon(AssetIcon::FolderOpen),
                    );
                }

                let state_confirm = state.clone();
                let view_confirm = view.clone();
                let paths_confirm = paths.clone();
                let empty_label = empty_label;

                content.child(
                    Command::new(&state)
                        .bordered(false)
                        .placeholder(placeholder)
                        .max_h(px(360.))
                        .items(items)
                        .empty(move |_, _, cx| {
                            div()
                                .px_4()
                                .py_6()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .text_center()
                                .child(empty_label)
                        })
                        .header(move |_, _, cx| {
                            h_flex()
                                .justify_between()
                                .items_center()
                                .px_3()
                                .py_2()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .child(title),
                                )
                        })
                        .footer(move |_, _, cx| {
                            h_flex()
                                .px_3()
                                .py_2()
                                .border_t_1()
                                .border_color(cx.theme().border)
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(hint),
                                )
                        })
                        .on_confirm(move |index, window, cx| {
                            if index.section != 0 {
                                return;
                            }
                            let query = state_confirm.read(cx).query(cx);
                            let typed = query.trim().to_string();
                            let open_row = if typed.is_empty() {
                                None
                            } else {
                                Some(paths_confirm.len())
                            };

                            let path = if open_row == Some(index.row) {
                                expand_typed_repo_path(&typed)
                            } else {
                                match paths_confirm.get(index.row) {
                                    Some(p) => PathBuf::from(p),
                                    None => return,
                                }
                            };

                            window.close_dialog(cx);
                            view_confirm.update(cx, |this, cx| {
                                this.open_repo_path(path, cx);
                            });
                        }),
                )
            })
    });
}

fn recent_picker_item(path: &str) -> CommandItem {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();
    let path_owned = path.to_string();
    let name_for_row = name.clone();
    let path_for_row = path_owned.clone();

    CommandItem::new()
        .label(name)
        .keywords([path_owned])
        .child(move |_, cx| {
            h_flex()
                .flex_1()
                .gap_2()
                .items_center()
                .min_w_0()
                .child(
                    Icon::new(AssetIcon::Folder)
                        .small()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .text_sm()
                                .line_height(relative(1.35))
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(name_for_row.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .line_height(relative(1.35))
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(path_for_row.clone()),
                        ),
                )
        })
}

fn expand_typed_repo_path(raw: &str) -> PathBuf {
    let raw = raw.trim();
    if raw == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }
    if let Some(rest) = raw
        .strip_prefix("~/")
        .or_else(|| raw.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(raw)
}
