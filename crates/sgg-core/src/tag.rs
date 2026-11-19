use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;

#[derive(Debug, Clone)]
pub struct TagInfo {
    pub name: String,
    /// Peeled object name (usually the commit).
    pub target: String,
}

impl RepoState {
    /// Lightweight tag list: `name\toid` via `git for-each-ref`.
    pub fn list_tags(&self) -> Result<Vec<TagInfo>, RepoError> {
        let out = run_git(
            &self.root,
            &[
                "for-each-ref",
                "--sort=-creatordate",
                "--format=%(refname:short)\t%(objectname:short)",
                "refs/tags",
            ],
        )?;
        let mut tags = Vec::new();
        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let Some(name) = parts.next().map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            let target = parts.next().unwrap_or("").trim().to_string();
            tags.push(TagInfo {
                name: name.to_string(),
                target,
            });
        }
        Ok(tags)
    }

    /// Create a tag. With `message`, creates an annotated tag; otherwise lightweight.
    /// `target` defaults to HEAD when `None`.
    pub fn create_tag(
        &self,
        name: &str,
        target: Option<&str>,
        message: Option<&str>,
    ) -> Result<(), RepoError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(RepoError::Message("tag name is empty".into()));
        }
        let target = target.map(str::trim).filter(|s| !s.is_empty());
        let message = message.map(str::trim).filter(|s| !s.is_empty());
        match (message, target) {
            (Some(msg), Some(oid)) => {
                run_git(&self.root, &["tag", "-a", name, "-m", msg, oid])?;
            }
            (Some(msg), None) => {
                run_git(&self.root, &["tag", "-a", name, "-m", msg])?;
            }
            (None, Some(oid)) => {
                run_git(&self.root, &["tag", name, oid])?;
            }
            (None, None) => {
                run_git(&self.root, &["tag", name])?;
            }
        }
        Ok(())
    }

    pub fn delete_tag(&self, name: &str) -> Result<(), RepoError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(RepoError::Message("tag name is empty".into()));
        }
        run_git(&self.root, &["tag", "-d", name])?;
        Ok(())
    }

    pub fn push_tag(&self, remote: &str, name: &str) -> Result<String, RepoError> {
        let remote = remote.trim();
        let name = name.trim();
        if remote.is_empty() || name.is_empty() {
            return Err(RepoError::Message("remote and tag name are required".into()));
        }
        Ok(run_git(&self.root, &["push", remote, &format!("refs/tags/{name}")])?)
    }
}
