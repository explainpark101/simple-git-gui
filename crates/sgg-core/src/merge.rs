use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;

impl RepoState {
    /// Merge `name` into the current branch (`git merge <name>`).
    pub fn merge_branch(&self, name: &str) -> Result<String, RepoError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(RepoError::Message("merge branch is empty".into()));
        }
        Ok(run_git(&self.root, &["merge", name])?)
    }

    pub fn merge_abort(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["merge", "--abort"])?)
    }

    /// After conflicts are resolved and staged: `git merge --continue`.
    pub fn merge_continue(&self) -> Result<String, RepoError> {
        Ok(run_git(&self.root, &["merge", "--continue"])?)
    }
}
