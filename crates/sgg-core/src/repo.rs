use std::path::{Path, PathBuf};

use git2::{BranchType, IndexAddOption, Repository, Status, StatusOptions, StatusShow};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::shell::{GitCliError, run_git};

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("no .git found from {0}")]
    NotARepo(PathBuf),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Cli(#[from] GitCliError),
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    Staged,
    Unstaged,
    Untracked,
    Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
    pub status_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSnapshot {
    pub root: PathBuf,
    pub head_branch: Option<String>,
    pub changes: Vec<FileChange>,
    pub branches: Vec<BranchInfo>,
    pub remotes: Vec<RemoteInfo>,
}

/// Walk upward from `start` until a `.git` entry is found.
pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let git_entry = current.join(".git");
        if git_entry.exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub struct RepoState {
    root: PathBuf,
    repo: Repository,
}

impl RepoState {
    pub fn open_nearest(start: &Path) -> Result<Self, RepoError> {
        let root =
            find_git_root(start).ok_or_else(|| RepoError::NotARepo(start.to_path_buf()))?;
        let repo = Repository::open(&root)?;
        Ok(Self { root, repo })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn snapshot(&self) -> Result<RepoSnapshot, RepoError> {
        Ok(RepoSnapshot {
            root: self.root.clone(),
            head_branch: self.head_branch_name()?,
            changes: self.list_changes()?,
            branches: self.list_branches()?,
            remotes: self.list_remotes()?,
        })
    }

    fn head_branch_name(&self) -> Result<Option<String>, RepoError> {
        match self.repo.head() {
            Ok(head) => Ok(head.shorthand().map(|s| s.to_string())),
            Err(err) if err.code() == git2::ErrorCode::UnbornBranch => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    pub fn list_changes(&self) -> Result<Vec<FileChange>, RepoError> {
        let mut opts = StatusOptions::new();
        opts.show(StatusShow::IndexAndWorkdir)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut out = Vec::new();

        for entry in statuses.iter() {
            let path = match entry.path() {
                Some(p) => p.to_string(),
                None => continue,
            };
            let status = entry.status();

            if status.intersects(Status::CONFLICTED) {
                out.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Conflicted,
                    status_label: "conflict".into(),
                });
                continue;
            }

            let staged = status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            );
            let unstaged = status.intersects(
                Status::WT_MODIFIED
                    | Status::WT_DELETED
                    | Status::WT_RENAMED
                    | Status::WT_TYPECHANGE,
            );
            let untracked = status.intersects(Status::WT_NEW);

            if staged {
                out.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Staged,
                    status_label: staged_label(status),
                });
            }
            if unstaged {
                out.push(FileChange {
                    path: path.clone(),
                    kind: ChangeKind::Unstaged,
                    status_label: unstaged_label(status),
                });
            }
            if untracked {
                out.push(FileChange {
                    path,
                    kind: ChangeKind::Untracked,
                    status_label: "untracked".into(),
                });
            }
        }

        Ok(out)
    }

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

fn staged_label(status: Status) -> String {
    if status.intersects(Status::INDEX_NEW) {
        "added".into()
    } else if status.intersects(Status::INDEX_DELETED) {
        "deleted".into()
    } else if status.intersects(Status::INDEX_RENAMED) {
        "renamed".into()
    } else {
        "modified".into()
    }
}

fn unstaged_label(status: Status) -> String {
    if status.intersects(Status::WT_DELETED) {
        "deleted".into()
    } else if status.intersects(Status::WT_RENAMED) {
        "renamed".into()
    } else {
        "modified".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_nearest_git_root() {
        let tmp = std::env::temp_dir().join(format!("sgg-test-{}", std::process::id()));
        let nested = tmp.join("a").join("b");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(tmp.join(".git")).unwrap();
        assert_eq!(find_git_root(&nested), Some(tmp.clone()));
        let _ = fs::remove_dir_all(tmp);
    }
}
