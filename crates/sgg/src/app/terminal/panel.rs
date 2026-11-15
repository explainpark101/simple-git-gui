//! Bottom-docked terminal UI (Zed-inspired cell colors + Nerd Font).

use gpui_kit::assets::IconName as AssetIcon;
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::{ActiveTheme as _, Selectable as _};
use gpui_kit::*;

use super::colors::{rgb_to_hsla, terminal_bg_hsla};
use super::session::{CaretShape, CursorState, DisplayLine, StyleRun};
use super::TerminalDock;
use crate::app::prefs::DEFAULT_TERMINAL_FONT_FALLBACKS;
use crate::app::view::AppView;
use crate::app::widgets::{action_btn, display_btn};

pub fn render(view: &AppView, cx: &mut Context<AppView>) -> AnyElement {
    let t = view.t();
    let dock = &view.terminal;
    if !dock.visible {
        return div().into_any_element();
    }

    let active = dock.active;
    let titles: Vec<(usize, String)> = dock
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.title.clone()))
        .collect();
    let lines = dock.lines.clone();
    let cursor = dock.cursor;
    let focused = view.terminal_focused;
    let caret_on = view.terminal_caret_on;
    let term_font = terminal_font(&view.terminal_font_family);
    let font_size = view.terminal_font_size;
    let line_px = view.terminal_line_px();
    let cell_w = terminal_cell_width(font_size);
    let bg = terminal_bg_hsla();
    let pad_x = 8.0;
    let pad_y = 4.0;

    div()
        .id("terminal-dock")
        .size_full()
        .min_h_0()
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
        .on_mouse_down(
            MouseButton::Middle,
            cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.close_active_terminal(window, cx);
            }),
        )
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
                .relative()
                .flex_1()
                .min_h_0()
                .w_full()
                .px_2()
                .py_1()
                .overflow_hidden()
                .font(term_font.clone())
                .text_size(px(font_size))
                // Zed: line_height_px = font_size * multiplier; size cells to match.
                .line_height(px(line_px))
                .text_color(if focused {
                    rgb_to_hsla(0xe0, 0xe0, 0xe0)
                } else {
                    cx.theme().muted_foreground
                })
                .bg(bg)
                .tab_index(0)
                .track_focus(&view.terminal_focus)
                .key_context("TerminalPane")
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.focus_terminal(window, cx);
                    }),
                )
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.handle_terminal_key(event, window, cx);
                }))
                .child(
                    div()
                        .id("terminal-lines")
                        .size_full()
                        .overflow_hidden()
                        .children(lines.into_iter().enumerate().map(|(i, line)| {
                            render_line(i, line, term_font.clone(), line_px)
                        })),
                )
                .children(caret_element(
                    cursor,
                    focused,
                    caret_on,
                    cell_w,
                    line_px,
                    pad_x,
                    pad_y,
                )),
        )
        .into_any_element()
}

fn caret_element(
    cursor: CursorState,
    focused: bool,
    caret_on: bool,
    cell_w: f32,
    line_px: f32,
    pad_x: f32,
    pad_y: f32,
) -> Option<AnyElement> {
    if cursor.shape == CaretShape::Hidden || !cursor.in_view {
        return None;
    }
    // Blink while focused; keep a steady dim caret when unfocused.
    if focused && !caret_on {
        return None;
    }

    let left = pad_x + cursor.col as f32 * cell_w;
    let top = pad_y + cursor.row as f32 * line_px;
    let color = if focused {
        rgb_to_hsla(0xf0, 0xf0, 0xf0)
    } else {
        rgb_to_hsla(0x88, 0x88, 0x88)
    };

    // Prefer a beam (|) when the shell has not requested another shape.
    let shape = if cursor.shape == CaretShape::Block && !cursor.blinking {
        // Many shells default to Block; beam reads clearer for typing.
        CaretShape::Beam
    } else {
        cursor.shape
    };

    let caret = match shape {
        CaretShape::Beam => div()
            .id("term-caret")
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(2.))
            .h(px(line_px))
            .bg(color),
        CaretShape::Block => div()
            .id("term-caret")
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(cell_w.max(2.)))
            .h(px(line_px))
            .bg(color)
            .opacity(if focused { 0.35 } else { 0.2 }),
        CaretShape::Underline => div()
            .id("term-caret")
            .absolute()
            .left(px(left))
            .top(px(top + line_px - 2.0))
            .w(px(cell_w.max(2.)))
            .h(px(2.))
            .bg(color),
        CaretShape::Hidden => return None,
    };
    Some(caret.into_any_element())
}

fn render_line(i: usize, line: DisplayLine, base_font: Font, line_px: f32) -> AnyElement {
    let runs = style_runs_to_text_runs(&line.runs, &base_font);
    div()
        .id(("term-line", i))
        .w_full()
        .h(px(line_px))
        .flex()
        .flex_row()
        .items_center()
        .whitespace_nowrap()
        .overflow_hidden()
        .child(StyledText::new(line.text).with_runs(runs))
        .into_any_element()
}

fn style_runs_to_text_runs(runs: &[StyleRun], base_font: &Font) -> Vec<TextRun> {
    runs.iter()
        .map(|run| {
            let mut font = base_font.clone();
            if run.bold {
                font.weight = FontWeight::BOLD;
            }
            let mut color = rgb_to_hsla(run.fg[0], run.fg[1], run.fg[2]);
            if run.dim {
                color.a *= 0.7;
            }
            let background_color = run.bg.map(|bg| rgb_to_hsla(bg[0], bg[1], bg[2]));
            let underline = run.underline.then_some(UnderlineStyle {
                color: Some(color),
                thickness: px(1.),
                wavy: false,
            });
            TextRun {
                len: run.len,
                font,
                color,
                background_color,
                underline,
                strikethrough: None,
            }
        })
        .collect()
}

/// Resolve a GPUI [`Font`] for the terminal.
///
/// Unlike CSS, GPUI `font_family` is a **single** name. Comma-separated prefs are
/// split into primary + [`FontFallbacks`] (Zed `terminal.font_family` +
/// `terminal.font_fallbacks`).
pub fn terminal_font(family_pref: &str) -> Font {
    let parts: Vec<String> = family_pref
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let primary = parts
        .first()
        .cloned()
        .unwrap_or_else(|| crate::app::prefs::DEFAULT_TERMINAL_FONT_FAMILY.to_string());
    let mut fallbacks: Vec<String> = parts.into_iter().skip(1).collect();
    for name in DEFAULT_TERMINAL_FONT_FALLBACKS {
        if *name != primary.as_str() && !fallbacks.iter().any(|f| f == name) {
            fallbacks.push((*name).to_string());
        }
    }
    Font {
        family: primary.into(),
        features: FontFeatures::disable_ligatures(),
        fallbacks: Some(FontFallbacks::from_fonts(fallbacks)),
        weight: FontWeight::default(),
        style: FontStyle::default(),
    }
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
                        .on_mouse_down(
                            MouseButton::Middle,
                            cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.close_terminal_tab(idx, window, cx);
                            }),
                        )
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
                .on_click(cx.listener(|this, _, window, cx| {
                    this.close_active_terminal(window, cx);
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

/// Rough column/row estimate from dock pixel size and font metrics (Zed-style).
fn estimate_grid(width_px: f32, height_px: f32, cell_w: f32, line_h: f32) -> (u16, u16) {
    let header_px = 28.0;
    let pad_x = 16.0;
    let pad_y = 8.0;
    let cols = (((width_px - pad_x) / cell_w.max(1.0)).floor() as u16).clamp(40, 240);
    let rows = (((height_px - header_px - pad_y) / line_h.max(1.0)).floor() as u16).clamp(4, 80);
    (cols, rows)
}

impl TerminalDock {
    /// Apply an estimated size based on last known window width and font metrics.
    pub fn maybe_resize_from_px(&mut self, width_px: f32, font_size: f32, line_height: f32) {
        let cell_w = terminal_cell_width(font_size);
        let line_h = terminal_line_px(font_size, line_height);
        let (cols, rows) = estimate_grid(width_px, self.height(), cell_w, line_h);
        self.resize_active(cols, rows);
    }
}

/// Monospace cell width approximation (Zed measures the real em width).
pub fn terminal_cell_width(font_size: f32) -> f32 {
    (font_size * 0.6).max(6.0)
}

/// Pixel line height from font size × multiplier (Zed `TerminalLineHeight::value`).
pub fn terminal_line_px(font_size: f32, line_height: f32) -> f32 {
    (font_size * line_height.max(1.0)).ceil().max(font_size + 1.0)
}
