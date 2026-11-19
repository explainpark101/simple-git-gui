//! One PTY-backed shell session and its VTE grid.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Color, NamedColor, Processor, Rgb};
use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, MasterPty, PtySize};

use super::colors::{color_rgb, is_default_background};
use super::{next_session_title, SharedSnapshot};
use crate::app::terminal_shell::TerminalShellPrefs;
use alacritty_terminal::vte::ansi::CursorShape;

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 24;

/// One styled UTF-8 span within a display line (Zed BatchedTextRun-ish).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleRun {
    pub len: usize,
    pub fg: [u8; 3],
    /// `None` = use the dock background (don't paint a cell bg).
    pub bg: Option<[u8; 3]>,
    pub bold: bool,
    pub dim: bool,
    pub underline: bool,
}

/// One visible terminal row with ANSI attributes preserved.
#[derive(Clone, Debug, Default)]
pub struct DisplayLine {
    pub text: String,
    pub runs: Vec<StyleRun>,
}

/// Viewport-relative caret (Zed-style).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaretShape {
    Beam,
    Block,
    Underline,
    Hidden,
}

impl Default for CaretShape {
    fn default() -> Self {
        Self::Beam
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CursorState {
    pub col: u16,
    pub row: u16,
    pub shape: CaretShape,
    pub blinking: bool,
    /// False when the cursor is scrolled out of the viewport.
    pub in_view: bool,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            col: 0,
            row: 0,
            shape: CaretShape::Beam,
            blinking: true,
            in_view: true,
        }
    }
}

/// Visible-line snapshot shared with the UI thread.
#[derive(Clone, Default)]
pub struct TerminalSnapshot {
    pub lines: Vec<DisplayLine>,
    pub cursor: CursorState,
    pub cols: u16,
    pub rows: u16,
    #[allow(dead_code)]
    pub title: String,
    #[allow(dead_code)]
    pub exited: bool,
}

struct TermSize {
    columns: usize,
    screen_lines: usize,
}

impl TermSize {
    fn new(cols: u16, rows: u16) -> Self {
        Self {
            columns: cols.max(2) as usize,
            screen_lines: rows.max(1) as usize,
        }
    }
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.screen_lines
    }

    fn screen_lines(&self) -> usize {
        self.screen_lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

/// Relays VTE replies (DSR/CPR, color, size) back into the PTY.
/// Without this, Windows ConPTY shells stall on `\x1b[6n` and never paint a prompt.
struct PtyRelay {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    cols: Arc<Mutex<u16>>,
    rows: Arc<Mutex<u16>>,
}

impl EventListener for PtyRelay {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                let mut writer = self.writer.lock();
                let _ = writer.write_all(text.as_bytes());
                let _ = writer.flush();
            }
            Event::TextAreaSizeRequest(format) => {
                let cols = *self.cols.lock();
                let rows = *self.rows.lock();
                let reply = format(WindowSize {
                    num_lines: rows,
                    num_cols: cols,
                    cell_width: 8,
                    cell_height: 16,
                });
                let mut writer = self.writer.lock();
                let _ = writer.write_all(reply.as_bytes());
                let _ = writer.flush();
            }
            Event::ColorRequest(index, format) => {
                // Neutral defaults so color queries don't hang the shell.
                let rgb = match index {
                    i if i == NamedColor::Background as usize => Rgb {
                        r: 30,
                        g: 30,
                        b: 30,
                    },
                    i if i == NamedColor::Foreground as usize => Rgb {
                        r: 220,
                        g: 220,
                        b: 220,
                    },
                    _ => Rgb {
                        r: 200,
                        g: 200,
                        b: 200,
                    },
                };
                let reply = format(rgb);
                let mut writer = self.writer.lock();
                let _ = writer.write_all(reply.as_bytes());
                let _ = writer.flush();
            }
            Event::Title(title) => {
                let _ = title;
            }
            _ => {}
        }
    }
}

pub struct TerminalSession {
    #[allow(dead_code)]
    pub id: u64,
    pub title: String,
    snapshot: SharedSnapshot,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    term: Arc<Mutex<Term<PtyRelay>>>,
    size_cols: Arc<Mutex<u16>>,
    size_rows: Arc<Mutex<u16>>,
    cols: u16,
    rows: u16,
    /// Keep the child process alive for the session lifetime.
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    _reader: Option<JoinHandle<()>>,
}

impl TerminalSession {
    pub fn spawn(
        id: u64,
        cwd: Option<&Path>,
        shell: &TerminalShellPrefs,
        dirty: Arc<AtomicBool>,
    ) -> Result<Self> {
        let cols = DEFAULT_COLS;
        let rows = DEFAULT_ROWS;
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("open pty")?;

        let mut cmd = shell.build_command();
        if let Some(dir) = cwd {
            cmd.cwd(dir);
        }
        // Hint interactive apps that we are a color-capable terminal.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = pair
            .slave
            .spawn_command(cmd)
            .context("spawn shell in pty")?;
        // Drop slave so the child owns the only slave fd (required on Unix).
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("clone pty reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("take pty writer")?;
        let master = pair.master;

        let writer = Arc::new(Mutex::new(writer));
        let size_cols = Arc::new(Mutex::new(cols));
        let size_rows = Arc::new(Mutex::new(rows));
        let relay = PtyRelay {
            writer: Arc::clone(&writer),
            cols: Arc::clone(&size_cols),
            rows: Arc::clone(&size_rows),
        };

        let size = TermSize::new(cols, rows);
        let mut config = Config::default();
        config.scrolling_history = 2_000;
        let term = Term::new(config, &size, relay);

        let title = next_session_title();
        let snapshot = Arc::new(Mutex::new(TerminalSnapshot {
            lines: vec![DisplayLine::default(); rows as usize],
            cursor: CursorState::default(),
            cols,
            rows,
            title: title.clone(),
            exited: false,
        }));

        let term = Arc::new(Mutex::new(term));
        let master = Arc::new(Mutex::new(master));

        let term_bg = Arc::clone(&term);
        let snap_bg = Arc::clone(&snapshot);
        let dirty_bg = Arc::clone(&dirty);
        let reader_handle = thread::Builder::new()
            .name(format!("sgg-pty-{id}"))
            .spawn(move || {
                let mut parser: Processor = Processor::new();
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            {
                                let mut term = term_bg.lock();
                                parser.advance(&mut *term, &buf[..n]);
                                let (lines, cursor) = extract_display(&term);
                                let mut snap = snap_bg.lock();
                                snap.lines = lines;
                                snap.cursor = cursor;
                            }
                            dirty_bg.store(true, Ordering::Release);
                        }
                        Err(_) => break,
                    }
                }
                {
                    let mut snap = snap_bg.lock();
                    snap.exited = true;
                }
                dirty_bg.store(true, Ordering::Release);
            })
            .context("spawn pty reader thread")?;

        Ok(Self {
            id,
            title,
            snapshot,
            writer,
            master,
            term,
            size_cols,
            size_rows,
            cols,
            rows,
            _child: child,
            _reader: Some(reader_handle),
        })
    }

    pub fn display_lines(&self) -> Vec<DisplayLine> {
        self.snapshot.lock().lines.clone()
    }

    pub fn cursor_state(&self) -> CursorState {
        self.snapshot.lock().cursor
    }

    /// True after the PTY reader hits EOF (shell/`exit` closed the session).
    pub fn has_exited(&self) -> bool {
        self.snapshot.lock().exited
    }

    pub fn write_input(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut writer = self.writer.lock();
        let _ = writer.write_all(bytes);
        let _ = writer.flush();
    }

    /// Resize the PTY + VTE grid. Returns `true` when the size actually changed.
    pub fn resize(&mut self, cols: u16, rows: u16) -> bool {
        let cols = cols.max(2);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows {
            return false;
        }
        self.cols = cols;
        self.rows = rows;
        *self.size_cols.lock() = cols;
        *self.size_rows.lock() = rows;
        {
            let master = self.master.lock();
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
        {
            let mut term = self.term.lock();
            term.resize(TermSize::new(cols, rows));
            let (lines, cursor) = extract_display(&term);
            let mut snap = self.snapshot.lock();
            snap.cols = cols;
            snap.rows = rows;
            snap.lines = lines;
            snap.cursor = cursor;
        }
        true
    }
}

fn extract_display(term: &Term<PtyRelay>) -> (Vec<DisplayLine>, CursorState) {
    let grid = term.grid();
    let cols = term.columns();
    let rows = term.screen_lines();
    let display_offset = grid.display_offset() as i32;
    let mut out = Vec::with_capacity(rows);
    for row in 0..rows {
        let line = Line(row as i32 - display_offset);
        let mut text = String::with_capacity(cols);
        let mut runs: Vec<StyleRun> = Vec::new();
        let mut cur: Option<StyleRun> = None;

        for col in 0..cols {
            let cell = &grid[line][Column(col)];
            if cell.flags.contains(Flags::WIDE_CHAR_SPACER)
                || cell.flags.contains(Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            if cell.flags.contains(Flags::HIDDEN) {
                continue;
            }

            let mut fg_color = cell.fg;
            let mut bg_color = cell.bg;
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg_color, &mut bg_color);
            }

            let fg = color_rgb(fg_color);
            let bg = if is_default_background(bg_color)
                && !cell.flags.contains(Flags::INVERSE)
            {
                None
            } else {
                Some(color_rgb(bg_color))
            };
            let bold = cell.flags.intersects(Flags::BOLD | Flags::BOLD_ITALIC);
            let dim = cell.flags.contains(Flags::DIM);
            let underline = cell.flags.intersects(Flags::ALL_UNDERLINES);

            let mut chunk = String::new();
            let ch = if cell.c == '\0' { ' ' } else { cell.c };
            chunk.push(ch);
            if let Some(extra) = cell.zerowidth() {
                for z in extra {
                    chunk.push(*z);
                }
            }
            let len = chunk.len();
            text.push_str(&chunk);

            let style = StyleRun {
                len,
                fg,
                bg,
                bold,
                dim,
                underline,
            };
            match cur.as_mut() {
                Some(run)
                    if run.fg == style.fg
                        && run.bg == style.bg
                        && run.bold == style.bold
                        && run.dim == style.dim
                        && run.underline == style.underline =>
                {
                    run.len += style.len;
                }
                Some(_) => {
                    runs.push(cur.take().unwrap());
                    cur = Some(style);
                }
                None => cur = Some(style),
            }
        }
        if let Some(run) = cur {
            runs.push(run);
        }
        // Keep at least one cell so empty rows still reserve line height.
        if text.is_empty() {
            text.push(' ');
            runs = vec![StyleRun {
                len: 1,
                fg: color_rgb(Color::Named(NamedColor::Foreground)),
                bg: None,
                bold: false,
                dim: false,
                underline: false,
            }];
        }
        out.push(DisplayLine { text, runs });
    }

    let point = term.grid().cursor.point;
    let display_offset = term.grid().display_offset() as i32;
    let viewport_row = point.line.0 + display_offset;
    let col = point.column.0 as u16;
    let style = term.cursor_style();
    let shape = match style.shape {
        CursorShape::Block | CursorShape::HollowBlock => CaretShape::Block,
        CursorShape::Underline => CaretShape::Underline,
        CursorShape::Beam => CaretShape::Beam,
        CursorShape::Hidden => CaretShape::Hidden,
    };
    let in_view =
        viewport_row >= 0 && (viewport_row as usize) < rows && (col as usize) < cols;
    let cursor = CursorState {
        col,
        row: viewport_row.max(0) as u16,
        shape,
        blinking: style.blinking,
        in_view,
    };
    (out, cursor)
}
