//! Bottom-docked terminal UI.

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{ActiveTheme as _, Selectable as _};
use gpui_kit::*;

use super::TerminalDock;
use crate::app::view::AppView;
use crate::app::widgets::{action_btn, display_btn};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> AnyElement {
    let t = view.t();
    let dock = &view.terminal;
    if !dock.visible {
        return div().into_any_element();
    }

    let height = dock.height();
    let active = dock.active;
    let titles: Vec<(usize, String)> = dock
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.title.clone()))
        .collect();
    let lines = dock.lines.clone();
    let focused = view.terminal_focused;

    div()
        .id("terminal-dock")
        .w_full()
        .flex()
        .flex_col()
        .flex_shrink_0()
        .border_t_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .h(px(height))
        .child(header(
            view,
            &titles,
            active,
            t.terminal,
            t.tip_new_terminal,
            t.tip_close_terminal,
            t.tip_hide_terminal,
            cx,
        ))
        .child(
            div()
                .id("terminal-body")
                .flex_1()
                .min_h_0()
                .w_full()
                .px_2()
                .py_1()
                .overflow_hidden()
                .font_family("ui-monospace, SFMono-Regular, Menlo, Consolas, monospace")
                .text_xs()
                .text_color(if focused {
                    cx.theme().foreground
                } else {
                    cx.theme().muted_foreground
                })
                .bg(cx.theme().secondary)
                .track_focus(&view.terminal_focus)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.focus_terminal(window, cx);
                    }),
                )
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    this.handle_terminal_key(event, cx);
                }))
                .child(
                    div()
                        .id("terminal-lines")
                        .size_full()
                        .overflow_hidden()
                        .children(lines.into_iter().enumerate().map(|(i, line)| {
                            div()
                                .id(("term-line", i))
                                .w_full()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .child(if line.is_empty() {
                                    " ".to_string()
                                } else {
                                    line
                                })
                        })),
                ),
        )
        .into_any_element()
}

fn header(
    _view: &AppView,
    titles: &[(usize, String)],
    active: usize,
    label: &str,
    tip_new: &str,
    tip_close: &str,
    tip_hide: &str,
    cx: &mut Context<AppView>,
) -> impl IntoElement {
    div()
        .id("terminal-header")
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .overflow_hidden()
                .children(titles.iter().map(|(i, title)| {
                    let idx = *i;
                    let selected = idx == active;
                    display_btn(("term-tab", idx))
                        .label(title.clone())
                        .selected(selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.terminal.select(idx);
                            this.focus_terminal(window, cx);
                            cx.notify();
                        }))
                })),
        )
        .child(
            action_btn("term-new", AssetIcon::Plus)
                .ghost()
                .tooltip(tip_new)
                .on_click(cx.listener(|this, _, window, cx| {
                    this.new_terminal_tab(window, cx);
                })),
        )
        .child(
            action_btn("term-close", AssetIcon::Close)
                .ghost()
                .tooltip(tip_close)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.terminal.close_active();
                    cx.notify();
                })),
        )
        .child(
            action_btn("term-hide", AssetIcon::ChevronDown)
                .ghost()
                .tooltip(tip_hide)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.terminal.visible = false;
                    this.terminal_focused = false;
                    cx.notify();
                })),
        )
}

/// Map a GPUI key event to PTY bytes (xterm-ish).
pub fn key_event_to_bytes(event: &KeyDownEvent) -> Option<Vec<u8>> {
    let ks = &event.keystroke;
    let key = ks.key.as_str();
    let mods = &ks.modifiers;

    // Prefer typed character when no control/alt/cmd modifiers.
    if !mods.control && !mods.alt && !mods.platform {
        if let Some(ch) = ks.key_char.as_deref() {
            if !ch.is_empty() && key != "enter" && key != "tab" && key != "escape" {
                return Some(ch.as_bytes().to_vec());
            }
        }
    }

    if mods.control && !mods.alt && !mods.platform {
        if let Some(c) = key.chars().next() {
            if c.is_ascii_alphabetic() {
                let ctrl = (c.to_ascii_lowercase() as u8) & 0x1f;
                return Some(vec![ctrl]);
            }
        }
        match key {
            "`" | "~" => return None, // handled as app shortcuts
            _ => {}
        }
    }

    match key {
        "enter" => Some(b"\r".to_vec()),
        "tab" => Some(b"\t".to_vec()),
        "escape" => Some(b"\x1b".to_vec()),
        "backspace" => Some(b"\x7f".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "up" => Some(b"\x1b[A".to_vec()),
        "down" => Some(b"\x1b[B".to_vec()),
        "right" => Some(b"\x1b[C".to_vec()),
        "left" => Some(b"\x1b[D".to_vec()),
        "home" => Some(b"\x1b[H".to_vec()),
        "end" => Some(b"\x1b[F".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        _ => {
            if !mods.control && !mods.alt && !mods.platform {
                ks.key_char
                    .as_ref()
                    .map(|s| s.as_bytes().to_vec())
                    .filter(|b| !b.is_empty())
            } else {
                None
            }
        }
    }
}

/// Rough column/row estimate from dock pixel size.
fn estimate_grid(width_px: f32, height_px: f32) -> (u16, u16) {
    let cols = ((width_px / 7.2).floor() as u16).clamp(40, 240);
    let rows = (((height_px - 28.0) / 14.0).floor() as u16).clamp(4, 80);
    (cols, rows)
}

impl TerminalDock {
    /// Apply an estimated size based on last known window width.
    pub fn maybe_resize_from_px(&mut self, width_px: f32) {
        let (cols, rows) = estimate_grid(width_px, self.height());
        self.resize_active(cols, rows);
    }
}
