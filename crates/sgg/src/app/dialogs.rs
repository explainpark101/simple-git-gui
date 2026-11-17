//! Confirm dialogs with pointer cursors on footer buttons.

use std::rc::Rc;

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::Disableable as _;
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

/// Confirm alert that keeps the dialog open, shows a spinner on OK while
/// `kind` is busy, then closes after `work` finishes.
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
pub fn open_update_confirm(
    window: &mut Window,
    cx: &mut App,
    app: Entity<AppView>,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    later_text: impl Into<SharedString>,
    skip_version: String,
    work: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) {
    let title = title.into();
    let description = description.into();
    let ok = BusyConfirmOk::mount(
        cx,
        app.clone(),
        BusyKind::DownloadUpdate,
        ok_text.into(),
        AssetIcon::CloudDownload,
        Rc::new(work),
    );
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
                    window.close_dialog(cx);
                    app.update(cx, |this, cx| {
                        let work = work.clone();
                        this.run_busy(kind, window, cx, move |this, window, cx| {
                            work(this, window, cx);
                        });
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
