//! Settings page section ids, scroll anchors, and search matching.

use gpui_kit::*;

use super::i18n::UiText;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    Appearance,
    StatusColors,
    UiFont,
    InlineBlame,
    DiffLineBackground,
    TerminalShell,
    TerminalFont,
    CommitAi,
    Language,
    ChangesView,
    HistoryColumns,
    AutoRefresh,
    Branches,
    ExternalEditor,
    FetchOnOpen,
    PullStrategy,
    Repository,
    Updates,
}

impl SettingsSection {
    pub const ALL: [Self; 18] = [
        Self::Appearance,
        Self::StatusColors,
        Self::UiFont,
        Self::InlineBlame,
        Self::DiffLineBackground,
        Self::TerminalShell,
        Self::TerminalFont,
        Self::CommitAi,
        Self::Language,
        Self::ChangesView,
        Self::HistoryColumns,
        Self::AutoRefresh,
        Self::Branches,
        Self::ExternalEditor,
        Self::FetchOnOpen,
        Self::PullStrategy,
        Self::Repository,
        Self::Updates,
    ];

    pub fn title(self, t: UiText) -> &'static str {
        match self {
            Self::Appearance => t.appearance,
            Self::StatusColors => t.status_colors,
            Self::UiFont => t.ui_font,
            Self::InlineBlame => t.inline_blame,
            Self::DiffLineBackground => t.diff_line_background,
            Self::TerminalShell => t.terminal_shell,
            Self::TerminalFont => t.terminal_font,
            Self::CommitAi => t.commit_ai,
            Self::Language => t.language,
            Self::ChangesView => t.changes_view,
            Self::HistoryColumns => t.history_columns,
            Self::AutoRefresh => t.auto_refresh,
            Self::Branches => t.branches,
            Self::ExternalEditor => t.external_editor,
            Self::FetchOnOpen => t.fetch_on_open,
            Self::PullStrategy => t.pull_strategy,
            Self::Repository => t.repository,
            Self::Updates => t.updates,
        }
    }

    pub fn search_haystack(self, t: UiText) -> Vec<&'static str> {
        match self {
            Self::Appearance => vec![t.appearance, t.appearance_help, t.theme_light, t.theme_dark, t.theme_system],
            Self::StatusColors => vec![t.status_colors, t.status_colors_help],
            Self::UiFont => vec![t.ui_font, t.ui_font_help],
            Self::InlineBlame => vec![t.inline_blame],
            Self::DiffLineBackground => vec![t.diff_line_background],
            Self::TerminalShell => vec![t.terminal_shell, t.terminal_shell_help],
            Self::TerminalFont => vec![t.terminal_font, t.terminal_font_help],
            Self::CommitAi => vec![t.commit_ai, t.commit_ai_help],
            Self::Language => vec![t.language, t.language_help],
            Self::ChangesView => vec![t.changes_view, t.changes_view_help],
            Self::HistoryColumns => vec![
                t.history_columns,
                t.history_columns_help,
                t.history_col_description,
                t.history_col_date,
                t.history_col_author,
                t.history_col_commit,
            ],
            Self::AutoRefresh => vec![t.auto_refresh, t.auto_refresh_help],
            Self::Branches => {
                vec![t.branches, t.show_remote_branches, t.branch_sync_display]
            }
            Self::ExternalEditor => vec![t.external_editor, t.external_editor_help],
            Self::FetchOnOpen => vec![t.fetch_on_open, t.fetch_on_open_help],
            Self::PullStrategy => vec![t.pull_strategy, t.pull_strategy_help],
            Self::Repository => vec![t.repository, t.recent_projects, t.current_path],
            Self::Updates => vec![t.updates, t.updates_help],
        }
    }

    pub fn matches_query(self, query: &str, t: UiText) -> bool {
        let q = query.trim();
        if q.is_empty() {
            return true;
        }
        let q = q.to_ascii_lowercase();
        self.search_haystack(t)
            .into_iter()
            .any(|s| s.to_ascii_lowercase().contains(&q))
    }
}

#[derive(Clone)]
pub struct SettingsAnchors {
    appearance: ScrollAnchor,
    status_colors: ScrollAnchor,
    ui_font: ScrollAnchor,
    inline_blame: ScrollAnchor,
    diff_line_background: ScrollAnchor,
    terminal_shell: ScrollAnchor,
    terminal_font: ScrollAnchor,
    commit_ai: ScrollAnchor,
    language: ScrollAnchor,
    changes_view: ScrollAnchor,
    history_columns: ScrollAnchor,
    auto_refresh: ScrollAnchor,
    branches: ScrollAnchor,
    external_editor: ScrollAnchor,
    fetch_on_open: ScrollAnchor,
    pull_strategy: ScrollAnchor,
    repository: ScrollAnchor,
    updates: ScrollAnchor,
}

impl SettingsAnchors {
    pub fn new(handle: &ScrollHandle) -> Self {
        let mk = || ScrollAnchor::for_handle(handle.clone());
        Self {
            appearance: mk(),
            status_colors: mk(),
            ui_font: mk(),
            inline_blame: mk(),
            diff_line_background: mk(),
            terminal_shell: mk(),
            terminal_font: mk(),
            commit_ai: mk(),
            language: mk(),
            changes_view: mk(),
            history_columns: mk(),
            auto_refresh: mk(),
            branches: mk(),
            external_editor: mk(),
            fetch_on_open: mk(),
            pull_strategy: mk(),
            repository: mk(),
            updates: mk(),
        }
    }

    pub fn get(&self, section: SettingsSection) -> ScrollAnchor {
        match section {
            SettingsSection::Appearance => self.appearance.clone(),
            SettingsSection::StatusColors => self.status_colors.clone(),
            SettingsSection::UiFont => self.ui_font.clone(),
            SettingsSection::InlineBlame => self.inline_blame.clone(),
            SettingsSection::DiffLineBackground => self.diff_line_background.clone(),
            SettingsSection::TerminalShell => self.terminal_shell.clone(),
            SettingsSection::TerminalFont => self.terminal_font.clone(),
            SettingsSection::CommitAi => self.commit_ai.clone(),
            SettingsSection::Language => self.language.clone(),
            SettingsSection::ChangesView => self.changes_view.clone(),
            SettingsSection::HistoryColumns => self.history_columns.clone(),
            SettingsSection::AutoRefresh => self.auto_refresh.clone(),
            SettingsSection::Branches => self.branches.clone(),
            SettingsSection::ExternalEditor => self.external_editor.clone(),
            SettingsSection::FetchOnOpen => self.fetch_on_open.clone(),
            SettingsSection::PullStrategy => self.pull_strategy.clone(),
            SettingsSection::Repository => self.repository.clone(),
            SettingsSection::Updates => self.updates.clone(),
        }
    }
}

/// Hold-middle-button continuous scroll relative to the press origin.
///
/// Cursor above origin → scroll up; below → scroll down. Speed scales with
/// vertical distance from the origin (small dead zone near the press point).
#[derive(Clone)]
pub struct MiddleScrollDrag {
    pub scroll: ScrollHandle,
    pub origin: Point<Pixels>,
    pub cursor: Point<Pixels>,
}

impl MiddleScrollDrag {
    const DEAD_ZONE_PX: f32 = 12.;
    const SPEED_PER_PX: f32 = 0.28;
    const MIN_STEP_PX: f32 = 2.;
    const MAX_STEP_PX: f32 = 56.;

    pub fn update_cursor(&mut self, cursor: Point<Pixels>) {
        self.cursor = cursor;
    }

    /// Apply one autoscroll step from the latest cursor vs origin.
    pub fn tick(&self) {
        let dy = f32::from(self.cursor.y - self.origin.y);
        let abs = dy.abs();
        if abs <= Self::DEAD_ZONE_PX {
            return;
        }
        let speed = ((abs - Self::DEAD_ZONE_PX) * Self::SPEED_PER_PX)
            .clamp(Self::MIN_STEP_PX, Self::MAX_STEP_PX);
        // ScrollHandle y is ≤ 0; more negative = further down the content.
        let step = if dy > 0. { -speed } else { speed };
        let mut offset = self.scroll.offset();
        offset.y += px(step);
        let max = self.scroll.max_offset();
        offset.x = offset.x.clamp(-max.x, px(0.));
        offset.y = offset.y.clamp(-max.y, px(0.));
        self.scroll.set_offset(offset);
    }
}
