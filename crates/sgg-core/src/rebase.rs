use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;

impl RepoState {
    /// Rebase the current branch onto `onto` (local branch or remote-tracking ref).
    pub fn rebase_onto(&self, onto: &str) -> Result<String, RepoError> {
        let onto = onto.trim();
        if onto.is_empty() {
            return Err(RepoError::Message("rebase target is empty".into()));
        }
        Ok(run_git(&self.root, &["rebase", onto])?)
    }

    pub fn rebase_continue(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["rebase", "--continue"])?)
    }

    pub fn rebase_abort(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["rebase", "--abort"])?)
    }

    pub fn rebase_skip(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["rebase", "--skip"])?)
    }
}
