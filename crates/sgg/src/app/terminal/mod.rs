//! Embedded PTY terminal (portable-pty + alacritty_terminal VTE).
//!
//! ROADMAP calls for a PTY-backed emulator rendered in GPUI. libghostty-vt is
//! preferred long-term but requires Zig at build time; we use alacritty's VTE
//! engine for a dependency-light shippable terminal today.

mod colors;
mod panel;
mod session;

pub use panel::{key_event_to_bytes, render as render_panel, terminal_line_px};
pub use session::{CursorState, DisplayLine, TerminalSession, TerminalSnapshot};

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

/// Multi-tab terminal dock state owned by [`super::view::AppView`].
pub struct TerminalDock {
    pub visible: bool,
    pub sessions: Vec<TerminalSession>,
    pub active: usize,
    next_id: u64,
    /// Reader threads flip this; the UI task clears it after `cx.notify`.
    pub(crate) dirty: Arc<AtomicBool>,
    /// Cached display lines for the active session (rebuilt when dirty).
    pub lines: Vec<DisplayLine>,
    /// Cached caret for the active session.
    pub cursor: CursorState,
    /// Bumps when sessions change so the UI can refresh titles/tabs.
    pub generation: u64,
    height: f32,
    /// Last applied PTY grid size (skip redundant resizes during layout).
    pub(crate) last_grid: (u16, u16),
}

pub const TERMINAL_DEFAULT_HEIGHT: f32 = 220.0;
pub const TERMINAL_MIN_HEIGHT: f32 = 120.0;
pub const TERMINAL_MAX_HEIGHT: f32 = 480.0;

impl Default for TerminalDock {
    fn default() -> Self {
        Self {
            visible: false,
            sessions: Vec::new(),
            active: 0,
            next_id: 1,
            dirty: Arc::new(AtomicBool::new(false)),
            lines: Vec::new(),
            cursor: CursorState::default(),
            generation: 0,
            height: TERMINAL_DEFAULT_HEIGHT,
            last_grid: (0, 0),
        }
    }
}

impl TerminalDock {
    pub fn height(&self) -> f32 {
        self.height.clamp(TERMINAL_MIN_HEIGHT, TERMINAL_MAX_HEIGHT)
    }

    pub fn set_height(&mut self, height: f32) {
        self.height = height.clamp(TERMINAL_MIN_HEIGHT, TERMINAL_MAX_HEIGHT);
    }

    pub fn active_session(&self) -> Option<&TerminalSession> {
        self.sessions.get(self.active)
    }

    pub fn active_session_mut(&mut self) -> Option<&mut TerminalSession> {
        self.sessions.get_mut(self.active)
    }

    /// Toggle the dock. Spawns the first session if opening with none.
    pub fn toggle(
        &mut self,
        cwd: Option<&Path>,
        shell: &crate::app::terminal_shell::TerminalShellPrefs,
    ) -> anyhow::Result<()> {
        if self.visible {
            self.visible = false;
            return Ok(());
        }
        self.visible = true;
        if self.sessions.is_empty() {
            self.spawn_session(cwd, shell)?;
        }
        self.refresh_lines();
        Ok(())
    }

    pub fn spawn_session(
        &mut self,
        cwd: Option<&Path>,
        shell: &crate::app::terminal_shell::TerminalShellPrefs,
    ) -> anyhow::Result<()> {
        let id = self.next_id;
        self.next_id += 1;
        let session = TerminalSession::spawn(id, cwd, shell, Arc::clone(&self.dirty))?;
        self.sessions.push(session);
        self.active = self.sessions.len() - 1;
        self.generation = self.generation.wrapping_add(1);
        self.refresh_lines();
        self.visible = true;
        Ok(())
    }

    pub fn close_active(&mut self) {
        if self.sessions.is_empty() {
            self.visible = false;
            return;
        }
        let idx = self.active.min(self.sessions.len() - 1);
        self.close_at(idx);
    }

    pub fn close_at(&mut self, idx: usize) {
        if idx >= self.sessions.len() {
            return;
        }
        self.sessions.remove(idx);
        if self.sessions.is_empty() {
            self.active = 0;
            self.visible = false;
            self.lines.clear();
            self.cursor = CursorState::default();
        } else {
            if self.active > idx {
                self.active -= 1;
            } else if self.active >= self.sessions.len() {
                self.active = self.sessions.len() - 1;
            }
            self.refresh_lines();
        }
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn select(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.active = index;
            self.refresh_lines();
            self.generation = self.generation.wrapping_add(1);
        }
    }

    pub fn write_input(&mut self, bytes: &[u8]) {
        if let Some(session) = self.active_session_mut() {
            session.write_input(bytes);
        }
    }

    pub fn resize_active(&mut self, cols: u16, rows: u16) {
        let _ = self.resize_all(cols, rows);
    }

    /// Resize every session's PTY to match the dock. Returns whether any changed.
    pub fn resize_all(&mut self, cols: u16, rows: u16) -> bool {
        if self.last_grid == (cols, rows) && !self.sessions.is_empty() {
            return false;
        }
        let mut changed = false;
        for session in &mut self.sessions {
            if session.resize(cols, rows) {
                changed = true;
            }
        }
        if changed || self.last_grid != (cols, rows) {
            self.last_grid = (cols, rows);
        }
        changed
    }

    pub fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    pub fn refresh_lines(&mut self) {
        let (lines, cursor) = match self.sessions.get(self.active) {
            Some(s) => (s.display_lines(), s.cursor_state()),
            None => (Vec::new(), CursorState::default()),
        };
        self.lines = lines;
        self.cursor = cursor;
    }
}

/// Shared counter for unique session titles.
pub(crate) static SESSION_SEQ: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_session_title() -> String {
    let n = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("Terminal {n}")
}

/// Shared grid snapshot used by the reader thread and UI.
pub(crate) type SharedSnapshot = Arc<Mutex<TerminalSnapshot>>;
