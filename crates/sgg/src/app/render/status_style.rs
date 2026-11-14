use std::collections::HashMap;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{App, Hsla};
use sgg_core::FileChange;

/// Human-readable status label shown in the Changes list.
pub fn display_label(status: &str) -> &'static str {
    match status {
        "modified" => "Updated",
        "added" => "Added",
        "deleted" => "Deleted",
        "renamed" => "Renamed",
        "untracked" => "Untracked",
        "conflict" => "Conflict",
        _ => "Changed",
    }
}

pub fn status_color(status: &str, cx: &App) -> Hsla {
    let theme = cx.theme();
    match status {
        "added" | "untracked" => theme.success,
        "modified" => theme.warning,
        "deleted" | "conflict" => theme.danger,
        "renamed" => theme.info,
        _ => theme.muted_foreground,
    }
}

pub fn area_label(kind: sgg_core::ChangeKind) -> &'static str {
    match kind {
        sgg_core::ChangeKind::Staged => "Staged",
        sgg_core::ChangeKind::Unstaged => "Unstaged",
        sgg_core::ChangeKind::Untracked => "Untracked",
        sgg_core::ChangeKind::Conflicted => "Conflict",
    }
}

fn normalize_repo_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Higher = more urgent when a folder contains mixed statuses.
fn status_priority(status: &str) -> u8 {
    match status {
        "conflict" => 60,
        "deleted" => 50,
        "modified" => 40,
        "renamed" => 30,
        "added" | "untracked" => 20,
        _ => 10,
    }
}

/// Path → git status label for coloring the Files tree.
#[derive(Debug, Default, Clone)]
pub struct TreeStatusIndex {
    files: HashMap<String, String>,
}

impl TreeStatusIndex {
    pub fn from_changes(changes: &[FileChange]) -> Self {
        let mut files: HashMap<String, String> = HashMap::new();
        for change in changes {
            let path = normalize_repo_path(&change.path);
            let next = change.status_label.as_str();
            match files.get(&path) {
                Some(prev) if status_priority(prev) >= status_priority(next) => {}
                _ => {
                    files.insert(path, next.to_string());
                }
            }
        }
        Self { files }
    }

    /// Status for a file path, or the highest-priority status under a directory.
    pub fn status_for(&self, rel_path: &str, is_dir: bool) -> Option<&str> {
        let key = normalize_repo_path(rel_path);
        if !is_dir {
            return self.files.get(&key).map(String::as_str);
        }
        let prefix = format!("{key}/");
        let mut best: Option<&str> = None;
        let mut best_pri = 0u8;
        for (path, status) in &self.files {
            if path == &key || path.starts_with(&prefix) {
                let pri = status_priority(status);
                if pri > best_pri {
                    best_pri = pri;
                    best = Some(status.as_str());
                }
            }
        }
        best
    }

    pub fn color_for(&self, rel_path: &str, is_dir: bool, dim: f32, cx: &App) -> Hsla {
        match self.status_for(rel_path, is_dir) {
            Some(status) => status_color(status, cx).opacity(dim),
            None => cx.theme().foreground.opacity(dim),
        }
    }
}