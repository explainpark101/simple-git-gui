use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;

impl RepoState {
    /// Create a revert commit for `oid` (`git revert --no-edit`).
    pub fn revert_commit(&self, oid: &str) -> Result<String, RepoError> {
        let oid = oid.trim();
        if oid.is_empty() {
            return Err(RepoError::Message("revert commit is empty".into()));
        }
        Ok(run_git(&self.root, &["revert", "--no-edit", oid])?)
    }

    pub fn revert_continue(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["revert", "--continue"])?)
    }

    pub fn revert_abort(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["revert", "--abort"])?)
    }
}
