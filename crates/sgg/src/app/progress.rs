//! Bottom-docked activity log for long-running Git actions.

use gpui_kit::SharedString;

use super::busy::BusyKind;
use super::i18n::UiText;

pub const PROGRESS_LOG_LIMIT: usize = 40;
pub const PROGRESS_DEFAULT_HEIGHT: f32 = 128.0;
pub const PROGRESS_MIN_HEIGHT: f32 = 88.0;
pub const PROGRESS_MAX_HEIGHT: f32 = 360.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct ProgressEntry {
    pub title: SharedString,
    pub detail: SharedString,
    pub state: ProgressState,
}

impl BusyKind {
    pub fn progress_label(self, t: UiText) -> &'static str {
        match self {
            Self::Fetch => t.fetch,
            Self::Pull => t.pull,
            Self::Push => t.push,
            Self::Commit => t.commit,
            Self::Stage | Self::StageAll => t.stage,
            Self::Unstage | Self::UnstageAll => t.unstage,
            Self::Revert => t.revert,
            Self::CreateBranch => t.create_switch,
            Self::SwitchBranch => t.tip_switch_branch,
            Self::RenameBranch => t.tip_rename_branch,
            Self::DeleteBranch => t.tip_delete_branch,
            Self::SavePreview => t.save,
            Self::SaveEditor => t.save_command,
            Self::OpenFolder => t.open_folder,
            Self::OpenProject => t.open_project_in,
            Self::FileRename => t.rename_file,
            Self::FileDelete => t.delete_file,
            Self::FileMove => t.move_file,
            Self::FileCreate => t.new_file,
            Self::FolderCreate => t.new_folder,
            Self::AddRemote => t.add_remote,
            Self::EditRemote => t.edit_remote,
            Self::DeleteRemote => t.delete_remote,
        }
    }
}
