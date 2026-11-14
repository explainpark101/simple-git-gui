use git2::{Status, StatusOptions, StatusShow};

use crate::error::RepoError;
use crate::state::RepoState;
use crate::types::{ChangeKind, FileChange};

impl RepoState {
    pub fn list_changes(&self) -> Result<Vec<FileChange>, RepoError> {
        let mut opts = StatusOptions::new();
        opts.show(StatusShow::IndexAndWorkdir)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut out = Vec::new();

        for entry in statuses.iter() {
            let path = match entry.path() {
                Some(p) => p.to_string(),
                None => continue,
            };
            let status = entry.status();

            if status.intersects(Status::CONFLICTED) {
                out.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Conflicted,
                    status_label: "conflict".into(),
                });
                continue;
            }

            let staged = status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            );
            let unstaged = status.intersects(
                Status::WT_MODIFIED
                    | Status::WT_DELETED
                    | Status::WT_RENAMED
                    | Status::WT_TYPECHANGE,
            );
            let untracked = status.intersects(Status::WT_NEW);

            if staged {
                out.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Staged,
                    status_label: staged_label(status),
                });
            }
            if unstaged {
                out.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Unstaged,
                    status_label: unstaged_label(status),
                });
            }
            if untracked {
                out.push(FileChange {
                    path,
                    kind: ChangeKind::Untracked,
                    status_label: "untracked".into(),
                });
            }
        }

        Ok(out)
    }
}

fn staged_label(status: Status) -> String {
    if status.intersects(Status::INDEX_NEW) {
        "added".into()
    } else if status.intersects(Status::INDEX_DELETED) {
        "deleted".into()
    } else if status.intersects(Status::INDEX_RENAMED) {
        "renamed".into()
    } else {
        "modified".into()
    }
}

fn unstaged_label(status: Status) -> String {
    if status.intersects(Status::WT_DELETED) {
        "deleted".into()
    } else if status.intersects(Status::WT_RENAMED) {
        "renamed".into()
    } else {
        "modified".into()
    }
}