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
            Self::HistoryLoad => t.tab_history,
            Self::ResetSoft => t.reset_soft,
            Self::ResetMixed => t.reset_mixed,
            Self::ResetHard => t.reset_hard,
            Self::CheckUpdate => t.check_for_updates,
            Self::DownloadUpdate => t.download_update,
            Self::GenerateCommitMessage => t.generate_commit_message,
            Self::InstallCli => t.commit_ai_install_cli,
            Self::Rebase => t.rebase_onto,
            Self::RebaseContinue => t.rebase_continue,
            Self::RebaseAbort => t.rebase_abort,
            Self::RebaseSkip => t.rebase_skip,
            Self::Merge => t.merge_branch,
            Self::MergeContinue => t.merge_continue,
            Self::MergeAbort => t.merge_abort,
            Self::CherryPick => t.cherry_pick,
            Self::CherryPickContinue => t.cherry_pick_continue,
            Self::CherryPickAbort => t.cherry_pick_abort,
            Self::TagCreate => t.create_tag,
            Self::TagDelete => t.delete_tag,
            Self::TagPush => t.push_tag,
            Self::StashPush => t.stash_push,
            Self::StashPop => t.stash_pop,
            Self::StashApply => t.stash_apply,
            Self::StashDrop => t.stash_drop,
        }
    }
}
