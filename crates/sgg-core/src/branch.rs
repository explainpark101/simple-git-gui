use git2::BranchType;

use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;
use crate::types::BranchInfo;

impl RepoState {
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>, RepoError> {
        let mut out = Vec::new();
        for branch in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let name = branch.name()?.unwrap_or("").to_string();
            out.push(BranchInfo {
                name,
                is_head: branch.is_head(),
                is_remote: false,
            });
        }
        for branch in self.repo.branches(Some(BranchType::Remote))? {
            let (branch, _) = branch?;
            let name = branch.name()?.unwrap_or("").to_string();
            out.push(BranchInfo {
                name,
                is_head: false,
                is_remote: true,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn create_branch(&self, name: &str) -> Result<(), RepoError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(RepoError::Message("branch name is empty".into()));
        }
        let commit = self.repo.head()?.peel_to_commit()?;
        self.repo.branch(name, &commit, false)?;
        Ok(())
    }

    pub fn switch_branch(&self, name: &str) -> Result<(), RepoError> {
        run_git(&self.root, &["switch", name])?;
        Ok(())
    }
}