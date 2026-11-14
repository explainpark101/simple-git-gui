use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use sgg_core::RepoState;

#[derive(Debug, Clone)]
pub struct FsNode {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
    pub depth: usize,
    pub ignored: bool,
}

/// List visible tree rows under `root`, expanding directories listed in `expanded`.
pub fn list_tree(root: &Path, expanded: &HashSet<String>) -> Vec<FsNode> {
    let ignored_checker = RepoState::open_nearest(root).ok();
    let mut out = Vec::new();
    walk(root, "", 0, expanded, ignored_checker.as_ref(), &mut out);
    out
}

fn walk(
    root: &Path,
    rel: &str,
    depth: usize,
    expanded: &HashSet<String>,
    repo: Option<&RepoState>,
    out: &mut Vec<FsNode>,
) {
    let dir = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };

    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };

    let mut items: Vec<(String, PathBuf, bool)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name == ".git" || name == "." || name == ".." {
                return None;
            }
            let path = entry.path();
            let is_dir = path.is_dir();
            Some((name, path, is_dir))
        })
        .collect();

    items.sort_by(|a, b| match (a.2, b.2) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
    });

    for (name, _path, is_dir) in items {
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        let ignored = repo
            .map(|r| r.is_path_ignored(Path::new(&child_rel)))
            .unwrap_or(false);
        out.push(FsNode {
            name,
            rel_path: child_rel.clone(),
            is_dir,
            depth,
            ignored,
        });
        if is_dir && expanded.contains(&child_rel) {
            walk(root, &child_rel, depth + 1, expanded, repo, out);
        }
    }
}
