use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;
use crate::types::RemoteInfo;

impl RepoState {
    pub fn list_remotes(&self) -> Result<Vec<RemoteInfo>, RepoError> {
        let mut out = Vec::new();
        for name in self.repo.remotes()?.iter().flatten() {
            let remote = self.repo.find_remote(name)?;
            out.push(RemoteInfo {
                name: name.to_string(),
                url: remote.url().map(|s| s.to_string()),
            });
        }
        Ok(out)
    }

    pub fn push(&self, remote: Option<&str>) -> Result<String, RepoError> {
        let remote = remote.unwrap_or("origin");
        let out = run_git(&self.root, &["push", "-u", remote, "HEAD"])?;
        Ok(out)
    }

    pub fn pull(&self, remote: Option<&str>) -> Result<String, RepoError> {
        let remote = remote.unwrap_or("origin");
        let branch = self
            .head_branch_name()?
            .unwrap_or_else(|| "HEAD".to_string());
        let out = run_git(&self.root, &["pull", remote, &branch])?;
        Ok(out)
    }

    pub fn fetch(&self, remote: Option<&str>) -> Result<String, RepoError> {
        let remote = remote.unwrap_or("origin");
        let out = run_git(&self.root, &["fetch", remote])?;
        Ok(out)
    }
}