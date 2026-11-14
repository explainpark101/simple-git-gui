use std::path::{Path, PathBuf};

use git2::Repository;

use crate::error::RepoError;
use crate::find::find_git_root;
use crate::types::RepoSnapshot;

pub struct RepoState {
    pub(crate) root: PathBuf,
    pub(crate) repo: Repository,
}

impl RepoState {
    pub fn open_nearest(start: &Path) -> Result<Self, RepoError> {
        let root =
            find_git_root(start).ok_or_else(|| RepoError::NotARepo(start.to_path_buf()))?;
        let repo = Repository::open(&root)?;
        Ok(Self { root, repo })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn snapshot(&self) -> Result<RepoSnapshot, RepoError> {
        Ok(RepoSnapshot {
            root: self.root.clone(),
            head_branch: self.head_branch_name()?,
            changes: self.list_changes()?,
            branches: self.list_branches()?,
            remotes: self.list_remotes()?,
        })
    }

    pub(crate) fn head_branch_name(&self) -> Result<Option<String>, RepoError> {
        match self.repo.head() {
            Ok(head) => Ok(head.shorthand().map(|s| s.to_string())),
            Err(err) if err.code() == git2::ErrorCode::UnbornBranch => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}