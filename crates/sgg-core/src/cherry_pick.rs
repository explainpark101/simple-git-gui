use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;

impl RepoState {
    pub fn cherry_pick(&self, oid: &str) -> Result<String, RepoError> {
        let oid = oid.trim();
        if oid.is_empty() {
            return Err(RepoError::Message("cherry-pick commit is empty".into()));
        }
        Ok(run_git(&self.root, &["cherry-pick", oid])?)
    }

    pub fn cherry_pick_continue(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["cherry-pick", "--continue"])?)
    }

    pub fn cherry_pick_abort(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["cherry-pick", "--abort"])?)
    }
}
