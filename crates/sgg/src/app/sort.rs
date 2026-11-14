use serde::{Deserialize, Serialize};
use sgg_core::{ChangeKind, FileChange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeSort {
    #[default]
    NameAsc,
    NameDesc,
    StatusAsc,
    StatusDesc,
}

impl ChangeSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::NameAsc => "Name ↑",
            Self::NameDesc => "Name ↓",
            Self::StatusAsc => "Status ↑",
            Self::StatusDesc => "Status ↓",
        }
    }
}

fn status_rank(label: &str) -> u8 {
    match label {
        "conflict" => 0,
        "added" => 1,
        "modified" => 2,
        "renamed" => 3,
        "deleted" => 4,
        "untracked" => 5,
        _ => 9,
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
    match sort {
        ChangeSort::NameAsc => a.path.to_lowercase().cmp(&b.path.to_lowercase()),
        ChangeSort::NameDesc => b.path.to_lowercase().cmp(&a.path.to_lowercase()),
        ChangeSort::StatusAsc => status_rank(&a.status_label)
            .cmp(&status_rank(&b.status_label))
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase())),
        ChangeSort::StatusDesc => status_rank(&b.status_label)
            .cmp(&status_rank(&a.status_label))
            .then_with(|| a.path.to_lowercase().cmp(&b.path.to_lowercase())),
    }
}

pub struct ChangeSection {
    pub title: &'static str,
    pub items: Vec<FileChange>,
}

pub fn group_and_sort(mut changes: Vec<FileChange>, sort: ChangeSort) -> Vec<ChangeSection> {
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
        if items.is_empty() {
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