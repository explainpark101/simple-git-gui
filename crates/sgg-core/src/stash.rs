use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;

#[derive(Debug, Clone)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
}

impl RepoState {
    /// Parse `git stash list --format=%gd\t%s`.
    pub fn stash_list(&self) -> Result<Vec<StashEntry>, RepoError> {
        let out = run_git(&self.root, &["stash", "list", "--format=%gd\t%s"])?;
        let mut entries = Vec::new();
        for line in out.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(2, '\t');
            let Some(gd) = parts.next() else {
                continue;
            };
            let message = parts.next().unwrap_or("").trim().to_string();
            // `stash@{N}`
            let index = gd
                .trim_start_matches("stash@{")
                .trim_end_matches('}')
                .parse::<usize>()
                .unwrap_or(entries.len());
            entries.push(StashEntry { index, message });
        }
        Ok(entries)
    }

    pub fn stash_push(
        &self,
        message: Option<&str>,
        include_untracked: bool,
    ) -> Result<String, RepoError> {
        let msg = message.map(str::trim).filter(|s| !s.is_empty());
        match (include_untracked, msg) {
            (true, Some(m)) => Ok(run_git(&self.root, &["stash", "push", "-u", "-m", m])?),
            (true, None) => Ok(run_git(&self.root, &["stash", "push", "-u"])?),
            (false, Some(m)) => Ok(run_git(&self.root, &["stash", "push", "-m", m])?),
            (false, None) => Ok(run_git(&self.root, &["stash", "push"])?),
        }
    }

    pub fn stash_pop(&self, index: Option<usize>) -> Result<String, RepoError> {
        match index {
            Some(i) => {
                let spec = format!("stash@{{{i}}}");
                Ok(run_git(&self.root, &["stash", "pop", &spec])?)
            }
            None => Ok(run_git(&self.root, &["stash", "pop"])?),
        }
    }

    pub fn stash_apply(&self, index: Option<usize>) -> Result<String, RepoError> {
        match index {
            Some(i) => {
                let spec = format!("stash@{{{i}}}");
                Ok(run_git(&self.root, &["stash", "apply", &spec])?)
            }
            None => Ok(run_git(&self.root, &["stash", "apply"])?),
        }
    }

    pub fn stash_drop(&self, index: usize) -> Result<String, RepoError> {
        let spec = format!("stash@{{{index}}}");
        Ok(run_git(&self.root, &["stash", "drop", &spec])?)
    }
}
