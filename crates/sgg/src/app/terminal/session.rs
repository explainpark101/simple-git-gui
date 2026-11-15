//! One PTY-backed shell session and its VTE grid.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use anyhow::{Context as _, Result};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use super::{next_session_title, SharedSnapshot};

const DEFAULT_COLS: u16 = 100;
const DEFAULT_ROWS: u16 = 24;

/// Visible-line text snapshot shared with the UI thread.
#[derive(Clone, Default)]
pub struct TerminalSnapshot {
    pub lines: Vec<String>,
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

pub struct TerminalSession {
    #[allow(dead_code)]
    pub id: u64,
    pub title: String,
    snapshot: SharedSnapshot,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    term: Arc<Mutex<Term<VoidListener>>>,
    cols: u16,
    rows: u16,
    /// Keep the child process alive for the session lifetime.
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    _reader: Option<JoinHandle<()>>,
}

impl TerminalSession {
    pub fn spawn(id: u64, cwd: Option<&Path>, dirty: Arc<AtomicBool>) -> Result<Self> {
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

        let mut cmd = CommandBuilder::new_default_prog();
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

        let size = TermSize::new(cols, rows);
        let mut config = Config::default();
        config.scrolling_history = 2_000;
        let term = Term::new(config, &size, VoidListener);

        let title = next_session_title();
        let snapshot = Arc::new(Mutex::new(TerminalSnapshot {
            lines: vec![String::new(); rows as usize],
            cols,
            rows,
            title: title.clone(),
            exited: false,
        }));

        let term = Arc::new(Mutex::new(term));
        let writer = Arc::new(Mutex::new(writer));
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
                                let lines = extract_lines(&term);
                                let mut snap = snap_bg.lock();
                                snap.lines = lines;
                            }
                            dirty_bg.store(true, Ordering::Release);
                        }
                        Err(_) => break,
                    }
                }
                {
                    let mut snap = snap_bg.lock();
                    snap.exited = true;
                    snap.lines.push("[process exited]".into());
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
            cols,
            rows,
            _child: child,
            _reader: Some(reader_handle),
        })
    }

    pub fn display_lines(&self) -> Vec<String> {
        self.snapshot.lock().lines.clone()
    }

    pub fn write_input(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut writer = self.writer.lock();
        let _ = writer.write_all(bytes);
        let _ = writer.flush();
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        let cols = cols.max(2);
        let rows = rows.max(1);
        if cols == self.cols && rows == self.rows {
            return;
        }
        self.cols = cols;
        self.rows = rows;
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
            let lines = extract_lines(&term);
            let mut snap = self.snapshot.lock();
            snap.cols = cols;
            snap.rows = rows;
            snap.lines = lines;
        }
    }
}

fn extract_lines(term: &Term<VoidListener>) -> Vec<String> {
    let grid = term.grid();
    let cols = term.columns();
    let rows = term.screen_lines();
    let display_offset = grid.display_offset() as i32;
    let mut out = Vec::with_capacity(rows);
    for row in 0..rows {
        let line = Line(row as i32 - display_offset);
        let mut s = String::with_capacity(cols);
        for col in 0..cols {
            let cell = &grid[line][Column(col)];
            if cell.c == '\0' {
                s.push(' ');
            } else {
                s.push(cell.c);
            }
        }
        // Keep trailing spaces for alignment; trim only all-space blank lines lightly.
        out.push(s.trim_end().to_string());
    }
    out
}
