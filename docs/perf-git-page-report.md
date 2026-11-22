# Git page performance investigation

Date: 2026-11-23  
Scope: Changes / Git page lag (commit message input, selection, preview, stage, general UI sluggishness)  
Method: Static code path analysis + hotfix implementation  
Hotfix: **1.6.9** — all report items below are implemented (ready to commit).

## Verdict

Git page slowness was primarily an **architecture / invalidation** problem. Almost every interaction called `cx.notify()` on monolithic `AppView`, rebuilding toolbar + sidebar + changes + preview.

## Implemented in 1.6.9

| Item | Status |
|------|--------|
| AI partials: no AppView `notify` per chunk | ✅ |
| Async preview / diff / gutter (generation token) | ✅ |
| `DiffFile::parse` + history file/diff load on background | ✅ |
| Blame overlay notify throttle (~50 ms) | ✅ |
| Pause auto-refresh while commit focused / AI generating | ✅ |
| Branch list `uniform_list` | ✅ |
| Child entities: `GitSidebarPane`, `ChangesPane`, `PreviewPaneHost`, `CommitComposer` | ✅ |
| Targeted `notify_changes` / `notify_sidebar` / `notify_preview` / `notify_git_work` | ✅ |
| Density / sidebar-compact notify coalesce | ✅ |
| Commit textarea isolated in `CommitComposer` | ✅ |

## Architecture (after fix)

```
AppView::render
 └─ AppPage::Git
     └─ h_resizable("git-panes")
         ├─ Entity<GitSidebarPane>
         └─ work
             ├─ Entity<ChangesPane>
             │    └─ Entity<CommitComposer>  // Textarea + AI + Commit
             └─ Entity<PreviewPaneHost>      // when preview open
```

Parent `notify` alone does **not** re-run child `Render`. High-frequency paths call the matching `notify_*` helper.

## Notify targeting

| Helper | Invalidates |
|--------|-------------|
| `notify_changes` | `changes_pane` + `commit_composer` |
| `notify_sidebar` | `git_sidebar` |
| `notify_preview` | `git_preview_pane` |
| `notify_git_work` | all three |

Still full `AppView` `cx.notify()` for: page switch, toolbar/status, errors, sidebar collapse layout, reload (also `notify_git_work`), theme/locale, terminal.

## Key files

- [crates/sgg/src/app/git_panes.rs](../crates/sgg/src/app/git_panes.rs)
- [crates/sgg/src/app/view.rs](../crates/sgg/src/app/view.rs) — pane entities, density coalesce, blame throttle
- [crates/sgg/src/app/ops.rs](../crates/sgg/src/app/ops.rs) — async preview/history loads
- [crates/sgg/src/app/render/changes.rs](../crates/sgg/src/app/render/changes.rs)
- [crates/sgg/src/app/render/sidebar.rs](../crates/sgg/src/app/render/sidebar.rs)
- [crates/sgg/src/app/render/preview_pane.rs](../crates/sgg/src/app/render/preview_pane.rs)
- [crates/sgg/src/app/render/mod.rs](../crates/sgg/src/app/render/mod.rs)
