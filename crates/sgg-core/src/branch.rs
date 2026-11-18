use git2::{Branch, BranchType, ErrorCode, Oid};

use crate::error::RepoError;
use crate::shell::run_git;
use crate::state::RepoState;
use crate::types::BranchInfo;

impl RepoState {
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>, RepoError> {
        let mut out = Vec::new();
        for branch in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let name = branch.name()?.unwrap_or("").to_string();
            let (upstream, ahead, behind) = self.local_tracking(&branch, &name)?;
            out.push(BranchInfo {
                name,
                is_head: branch.is_head(),
                is_remote: false,
                upstream,
                ahead,
                behind,
            });
        }
        for branch in self.repo.branches(Some(BranchType::Remote))? {
            let (branch, _) = branch?;
            let name = branch.name()?.unwrap_or("").to_string();
            out.push(BranchInfo {
                name,
                is_head: false,
                is_remote: true,
                upstream: None,
                ahead: 0,
                behind: 0,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Resolve upstream ref + ahead/behind vs that tip.
    /// Prefers configured tracking; falls back to `*/<local-name>` on a remote.
    fn local_tracking(
        &self,
        branch: &Branch<'_>,
        local_name: &str,
    ) -> Result<(Option<String>, usize, usize), RepoError> {
        let local_oid = branch.get().target();
        let upstream = match branch.upstream() {
            Ok(up) => {
                let name = up.name()?.unwrap_or("").to_string();
                let oid = up.get().target();
                Some((name, oid))
            }
            Err(err) if err.code() == ErrorCode::NotFound => {
                self.find_remote_ref_for(local_name)?
            }
            Err(err) => return Err(err.into()),
        };

        let Some((up_name, up_oid)) = upstream else {
            return Ok((None, 0, 0));
        };

        let (ahead, behind) = match (local_oid, up_oid) {
            (Some(local), Some(remote)) => self
                .repo
                .graph_ahead_behind(local, remote)
                .unwrap_or((0, 0)),
            _ => (0, 0),
        };
        Ok((Some(up_name), ahead, behind))
    }

    fn find_remote_ref_for(
        &self,
        local_name: &str,
    ) -> Result<Option<(String, Option<Oid>)>, RepoError> {
        let mut fallback = None;
        for branch in self.repo.branches(Some(BranchType::Remote))? {
            let (branch, _) = branch?;
            let Some(name) = branch.name()? else {
                continue;
            };
            // `origin/main` → short `main`; `origin/feat/x` → `feat/x`
            let Some((_, short)) = name.split_once('/') else {
                continue;
            };
            if short != local_name {
                continue;
            }
            let oid = branch.get().target();
            if name.starts_with("origin/") {
                return Ok(Some((name.to_string(), oid)));
            }
            if fallback.is_none() {
                fallback = Some((name.to_string(), oid));
            }
        }
        Ok(fallback)
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

    /// Fetch `remote` then create/switch a local branch tracking `remote_ref`
    /// (`origin/feat/x` → local `feat/x`). Returns the local branch name.
    pub fn checkout_remote_branch(&self, remote_ref: &str) -> Result<String, RepoError> {
        let remote_ref = remote_ref.trim();
        let Some((remote, local)) = remote_ref.split_once('/') else {
            return Err(RepoError::Message(format!(
                "invalid remote branch `{remote_ref}` (expected remote/name)"
            )));
        };
        if remote.is_empty() || local.is_empty() {
            return Err(RepoError::Message(format!(
                "invalid remote branch `{remote_ref}`"
            )));
        }
        if local == "HEAD" {
            return Err(RepoError::Message(
                "cannot check out remote HEAD; pick a named branch".into(),
            ));
        }

        run_git(&self.root, &["fetch", remote])?;

        let local_exists = self.repo.find_branch(local, BranchType::Local).is_ok();
        if local_exists {
            run_git(&self.root, &["switch", local])?;
        } else {
            // Creates local `local` tracking `remote_ref` and switches to it.
            run_git(&self.root, &["switch", "--track", remote_ref])?;
        }
        Ok(local.to_string())
    }

    pub fn rename_branch(&self, old: &str, new: &str) -> Result<(), RepoError> {
        let old = old.trim();
        let new = new.trim();
        if old.is_empty() || new.is_empty() {
            return Err(RepoError::Message("branch name is empty".into()));
        }
        if old == new {
            return Ok(());
        }
        run_git(&self.root, &["branch", "-m", old, new])?;
        Ok(())
    }

    pub fn delete_branch(&self, name: &str) -> Result<(), RepoError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(RepoError::Message("branch name is empty".into()));
        }
        run_git(&self.root, &["branch", "-D", name])?;
        Ok(())
    }
}
