//! Hot-path helpers: snapshot fingerprints and cheap equality for refresh skips.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use sgg_core::RepoSnapshot;

/// Stable fingerprint of repo UI state. Used to skip `cx.notify` when auto-refresh
/// finds no meaningful change (avoids full-tree re-render every few seconds).
pub fn snapshot_fingerprint(snap: &RepoSnapshot) -> u64 {
    let mut h = DefaultHasher::new();
    snap.root.hash(&mut h);
    snap.head_branch.hash(&mut h);
    snap.changes.len().hash(&mut h);
    for c in &snap.changes {
        c.path.hash(&mut h);
        c.kind.hash(&mut h);
        c.status_label.hash(&mut h);
    }
    snap.branches.len().hash(&mut h);
    for b in &snap.branches {
        b.name.hash(&mut h);
        b.is_head.hash(&mut h);
        b.is_remote.hash(&mut h);
    }
    snap.remotes.len().hash(&mut h);
    for r in &snap.remotes {
        r.name.hash(&mut h);
        r.url.hash(&mut h);
    }
    h.finish()
}
