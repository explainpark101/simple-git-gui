use std::path::Path;

use git2::IndexAddOption;

use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;

impl RepoState {
    pub fn stage_paths(&self, paths: &[&str]) -> Result<(), RepoError> {
        let mut index = self.repo.index()?;
        for path in paths {
            let p = Path::new(path);
            if !self.root.join(p).exists() {
                index.remove_path(p)?;
            } else {
                index.add_path(p)?;
            }
        }
        index.write()?;
        Ok(())
    }

    pub fn unstage_paths(&self, paths: &[&str]) -> Result<(), RepoError> {
        match self.repo.head() {
            Ok(head) => {
                let obj = head.peel(git2::ObjectType::Commit)?;
                self.repo.reset_default(Some(&obj), paths)?;
            }
            Err(err) if err.code() == git2::ErrorCode::UnbornBranch => {
                let mut index = self.repo.index()?;
                for path in paths {
                    index.remove_path(Path::new(path))?;
                }
                index.write()?;
            }
            Err(err) => return Err(err.into()),
        }
        Ok(())
    }

    pub fn stage_all(&self) -> Result<(), RepoError> {
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;
        Ok(())
    }

    pub fn unstage_all(&self) -> Result<(), RepoError> {
        match self.repo.head() {
            Ok(head) => {
                let obj = head.peel(git2::ObjectType::Commit)?;
                // Mixed reset keeps the working tree and clears the index.
                self.repo.reset(&obj, git2::ResetType::Mixed, None)?;
            }
            Err(err) if err.code() == git2::ErrorCode::UnbornBranch => {
                let mut index = self.repo.index()?;
                index.clear()?;
                index.write()?;
            }
            Err(err) => return Err(err.into()),
        }
        Ok(())
    }

    /// Discard local changes for paths (index + worktree) back to HEAD.
    /// Untracked paths are deleted from disk.
    pub fn revert_paths(&self, paths: &[&str]) -> Result<(), RepoError> {
        if paths.is_empty() {
            return Err(RepoError::Message("no paths selected".into()));
        }

        let mut tracked = Vec::new();
        let mut untracked = Vec::new();
        for path in paths {
            let full = self.root.join(path);
            let in_head = self
                .repo
                .head()
                .ok()
                .and_then(|h| h.peel_to_tree().ok())
                .and_then(|tree| tree.get_path(Path::new(path)).ok())
                .is_some();
            let in_index = self
                .repo
                .index()
                .ok()
                .and_then(|index| index.get_path(Path::new(path), 0))
                .is_some();

            if in_head || in_index {
                tracked.push(*path);
            } else if full.exists() {
                untracked.push(*path);
            }
        }

        if !tracked.is_empty() {
            let mut args = vec!["restore", "--source=HEAD", "--staged", "--worktree", "--"];
            args.extend(tracked.iter().copied());
            run_git(&self.root, &args)?;
        }

        for path in untracked {
            let full = self.root.join(path);
            if full.is_dir() {
                std::fs::remove_dir_all(&full).map_err(|e| {
                    RepoError::Message(format!("failed to remove {}: {e}", full.display()))
                })?;
            } else if full.exists() {
                std::fs::remove_file(&full).map_err(|e| {
                    RepoError::Message(format!("failed to remove {}: {e}", full.display()))
                })?;
            }
        }

        Ok(())
    }

    pub fn commit(&self, message: &str) -> Result<String, RepoError> {
        let message = message.trim();
        if message.is_empty() {
            return Err(RepoError::Message("commit message is empty".into()));
        }

        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let sig = self.repo.signature()?;

        let parents = match self.repo.head() {
            Ok(head) => {
                let commit = head.peel_to_commit()?;
                vec![commit]
            }
            Err(err) if err.code() == git2::ErrorCode::UnbornBranch => Vec::new(),
            Err(err) => return Err(err.into()),
        };

        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
        let oid = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;
        Ok(oid.to_string())
    }
}