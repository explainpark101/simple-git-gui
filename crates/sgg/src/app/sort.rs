use serde::{Deserialize, Serialize};
use sgg_core::{ChangeKind, FileChange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSort {
    #[default]
    NameAsc,
    NameDesc,
    /// Legacy values from older settings — treated as [`NameAsc`].
    #[serde(other)]
    Legacy,
}

impl ChangeSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::NameAsc => "Name ↑",
            Self::NameDesc => "Name ↓",
            Self::Legacy => "Name ↑",
        }
    }

    pub fn normalized(self) -> Self {
        match self {
            Self::Legacy => Self::NameAsc,
            other => other,
        }
    }
}

fn area_rank(kind: ChangeKind) -> u8 {
    match kind {
        ChangeKind::Conflicted => 0,
        ChangeKind::Staged => 1,
        ChangeKind::Unstaged => 2,
        ChangeKind::Untracked => 3,
    }
}

pub fn compare_changes(a: &FileChange, b: &FileChange, sort: ChangeSort) -> std::cmp::Ordering {
    match sort.normalized() {
        ChangeSort::NameAsc | ChangeSort::Legacy => {
            a.path.to_lowercase().cmp(&b.path.to_lowercase())
        }
        ChangeSort::NameDesc => b.path.to_lowercase().cmp(&a.path.to_lowercase()),
    }
}

pub struct ChangeSection {
    pub title: &'static str,
    pub items: Vec<FileChange>,
}

/// Build change sections.
///
/// When `group_by_status` is true, files are split into status groups.
/// `Staged` and `Unstaged` are always present (may be empty).
/// `Conflicted` / `Untracked` appear only when they have items.
///
/// When `group_by_status` is false, all files are shown in a single flat list.
pub fn group_and_sort(
    mut changes: Vec<FileChange>,
    sort: ChangeSort,
    group_by_status: bool,
) -> Vec<ChangeSection> {
    if !group_by_status {
        changes.sort_by(|a, b| compare_changes(a, b, sort));
        return vec![ChangeSection {
            title: "All",
            items: changes,
        }];
    }

    changes.sort_by(|a, b| {
        area_rank(a.kind)
            .cmp(&area_rank(b.kind))
            .then_with(|| compare_changes(a, b, sort))
    });

    let mut sections = Vec::new();
    for kind in [
        ChangeKind::Conflicted,
        ChangeKind::Staged,
        ChangeKind::Unstaged,
        ChangeKind::Untracked,
    ] {
        let items: Vec<_> = changes
            .iter()
            .filter(|c| c.kind == kind)
            .cloned()
            .collect();
        let always_show = matches!(kind, ChangeKind::Staged | ChangeKind::Unstaged);
        if items.is_empty() && !always_show {
            continue;
        }
        let title = match kind {
            ChangeKind::Conflicted => "Conflicted",
            ChangeKind::Staged => "Staged",
            ChangeKind::Unstaged => "Unstaged",
            ChangeKind::Untracked => "Untracked",
        };
        sections.push(ChangeSection { title, items });
    }
    sections
}

#[derive(Debug, Clone)]
pub enum ChangeTreeRow {
    Dir {
        name: String,
        /// Unique expand key: `{section}::{dir_path}`
        key: String,
        depth: usize,
    },
    File {
        change: FileChange,
        /// File name only (last path segment) when nested; full path at root.
        display_name: String,
        depth: usize,
    },
}

#[derive(Default)]
struct DirNode {
    dirs: std::collections::BTreeMap<String, DirNode>,
    files: Vec<FileChange>,
}

/// Flatten section items into folder-tree rows for the Changes list.
pub fn section_as_tree(
    section: &'static str,
    items: &[FileChange],
    expanded: &[String],
) -> Vec<ChangeTreeRow> {
    let mut root = DirNode::default();
    for change in items {
        insert_path(&mut root, &change.path, change.clone());
    }

    let mut out = Vec::new();
    flatten_tree(section, "", &root, 0, expanded, &mut out);
    out
}

fn insert_path(node: &mut DirNode, path: &str, change: FileChange) {
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        node.files.push(change);
        return;
    }
    let mut cur = node;
    for (i, part) in parts.iter().enumerate() {
        if i + 1 == parts.len() {
            cur.files.push(change);
            return;
        }
        cur = cur.dirs.entry((*part).to_string()).or_default();
    }
}

fn flatten_tree(
    section: &'static str,
    prefix: &str,
    node: &DirNode,
    depth: usize,
    expanded: &[String],
    out: &mut Vec<ChangeTreeRow>,
) {
    for (name, child) in &node.dirs {
        let dir_path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let key = format!("{section}::{dir_path}");
        out.push(ChangeTreeRow::Dir {
            name: name.clone(),
            key: key.clone(),
            depth,
        });
        if expanded.iter().any(|k| k == &key) {
            flatten_tree(section, &dir_path, child, depth + 1, expanded, out);
        }
    }

    let mut files = node.files.clone();
    files.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
    for change in files {
        let display_name = change
            .path
            .rsplit('/')
            .next()
            .unwrap_or(change.path.as_str())
            .to_string();
        out.push(ChangeTreeRow::File {
            change,
            display_name,
            depth,
        });
    }
}