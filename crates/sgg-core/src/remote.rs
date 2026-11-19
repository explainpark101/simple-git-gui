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

    /// Pull from `remote` using an explicit divergent-branch strategy.
    ///
    /// `reconcile` must be one of `--no-rebase`, `--rebase`, or `--ff-only`.
    /// Passing it avoids Git's "Need to specify how to reconcile divergent
    /// branches" failure when `pull.rebase` / `pull.ff` are unset.
    pub fn pull(&self, remote: Option<&str>, reconcile: &str) -> Result<String, RepoError> {
        let remote = remote.unwrap_or("origin");
        let branch = self
            .head_branch_name()?
            .unwrap_or_else(|| "HEAD".to_string());
        let flag = match reconcile.trim() {
            "--rebase" | "--no-rebase" | "--ff-only" => reconcile.trim(),
            _ => "--no-rebase",
        };
        let out = run_git(&self.root, &["pull", flag, remote, &branch])?;
        Ok(out)
    }

    pub fn fetch(&self, remote: Option<&str>) -> Result<String, RepoError> {
        let remote = remote.unwrap_or("origin");
        let out = run_git(&self.root, &["fetch", remote])?;
        Ok(out)
    }

    pub fn add_remote(&self, name: &str, url: &str) -> Result<(), RepoError> {
        let name = name.trim();
        let url = url.trim();
        if name.is_empty() || url.is_empty() {
            return Err(RepoError::Message("Remote name and URL are required".into()));
        }
        run_git(&self.root, &["remote", "add", name, url])?;
        Ok(())
    }

    pub fn remove_remote(&self, name: &str) -> Result<(), RepoError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(RepoError::Message("Remote name is required".into()));
        }
        run_git(&self.root, &["remote", "remove", name])?;
        Ok(())
    }

    pub fn rename_remote(&self, old: &str, new: &str) -> Result<(), RepoError> {
        let old = old.trim();
        let new = new.trim();
        if old.is_empty() || new.is_empty() {
            return Err(RepoError::Message("Remote names are required".into()));
        }
        if old == new {
            return Ok(());
        }
        run_git(&self.root, &["remote", "rename", old, new])?;
        Ok(())
    }

    pub fn set_remote_url(&self, name: &str, url: &str) -> Result<(), RepoError> {
        let name = name.trim();
        let url = url.trim();
        if name.is_empty() || url.is_empty() {
            return Err(RepoError::Message("Remote name and URL are required".into()));
        }
        run_git(&self.root, &["remote", "set-url", name, url])?;
        Ok(())
    }
}