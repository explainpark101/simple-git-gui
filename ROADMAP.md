# simple-git-gui (sgg) Roadmap

This document tracks planned work beyond the current MVP.

## Done in MVP (v0.1)

- Cross-platform desktop shell (Windows / macOS / Linux) via Rust + GPUI (gpui-kit)
- Resolve nearest `.git` from the launch path and run git ops against that repo root
- Changes view: status list, stage / unstage, commit
- Sync: push, pull
- Branches: list, create, switch
- Remotes: list
- Shell integration install/uninstall:
  - Windows Explorer context menu ("Open sgg here" / Korean label support)
  - macOS Finder Services / Quick Action for folders
  - Linux file-manager actions / desktop entry for directories

## Planned

### Terminal emulator (high priority)

- Embed a PTY-backed terminal (`portable-pty` + VTE-style parser)
- Render ANSI output inside a GPUI view
- Default cwd = repo root
- Split pane: Changes | Terminal

### History

- Commit graph / linear history list
- Select commit to show message, author, date, file list
- Diff against parent
- Soft/hard/mixed reset (with confirmations)

### More git operations

- Fetch (all / selected remote)
- Rebase — see **Rebase (planned)** below
- Merge + conflict status surface
- Cherry-pick
- Tag create / delete / push
- Remote add / rename / remove / set-url

### Rebase (planned)

Goal: rebase the current branch onto another branch (or upstream) from the GUI, with a clear path through conflicts via continue / abort / skip — without dropping to an external terminal for the happy path.

**Core actions**

- **Rebase onto** — pick a local branch (or remote-tracking ref) as the upstream; run `git rebase <onto>` (optional later: `--onto` with an explicit newbase)
- **Rebase continue** — after the user resolves conflicts and stages files, run `git rebase --continue`
- **Rebase abort** — `git rebase --abort` with confirm; restore pre-rebase state
- **Rebase skip** — `git rebase --skip` when the current commit should be dropped during conflict recovery
- **Interactive rebase (later)** — optional `git rebase -i` / todo editor; defer until History or a dedicated reorder UI exists
- **Pull with rebase (later)** — Settings or Pull menu option for `git pull --rebase`

**Conflict / in-progress UX**

- Detect in-progress rebase (`.git/rebase-merge` / `rebase-apply`) on load and after ops; surface a banner or Progress dock entry: “Rebase in progress”
- Changes view while rebasing: emphasize conflicted paths; Stage / continue workflow matches existing conflict handling where possible
- Disable or confirm dangerous actions (switch branch, hard reset) while a rebase is in progress
- Status line / Progress log: started onto `X`, stopped on commit, continued, aborted, completed, or failed

**UI sketch**

- Branches panel: context menu or toolbar action “Rebase onto…” → pick target branch (confirm dialog with busy spinner, same pattern as switch/fetch)
- While rebasing: sticky actions Continue / Skip / Abort near Changes or in the Progress dock
- Optional: show “onto” branch and current stop commit in the banner

**Implementation notes**

- Prefer git CLI (`rebase`, `rebase --continue|--abort|--skip`) via existing core helpers; refresh `RepoSnapshot` (and conflicted change list) after each step
- Wire `BusyKind` variants (`Rebase`, `RebaseContinue`, `RebaseAbort`, `RebaseSkip`) and i18n (en/ko) for labels and confirm copy
- Do not auto-rebase on pull in v1; keep rebase explicit unless the user opts into pull-with-rebase later
- Interactive rebase and rebase of a range from History can wait for the History panel

### Stash (planned)

Goal: stash dirty work from the Changes view and restore it later without leaving the GUI.

**Core actions**

- **Stash push** — save current tracked (and optionally untracked) changes; optional message; confirm when working tree is empty or only untracked
- **Stash pop** — apply latest (or selected) stash and drop it on success; surface conflicts and keep the stash entry if apply fails
- **Stash apply** — same as pop but keep the stash entry
- **Stash drop** — remove one entry (with confirm)
- **Stash list** — show `stash@{n}`, short message, and date in a sidebar panel or Changes toolbar dropdown

**UI sketch**

- Changes toolbar: Stash / Pop actions (busy spinner + confirm dialogs, same pattern as fetch/revert)
- Optional stash panel: list entries; row actions Apply, Pop, Drop; click to preview stash diff later
- Status line / error: conflict and empty-stash feedback via existing `set_ok_status` / `set_error`

**Implementation notes**

- Prefer git CLI (`stash push|pop|apply|drop|list`) via existing core helpers; refresh `RepoSnapshot` after each op
- Wire `BusyKind` variants (`StashPush`, `StashPop`, …) and i18n (en/ko) for labels and confirm copy
- Include untracked (`-u`) as a Settings or confirm checkbox, default off
- Do not auto-stash on branch switch in v1; keep stash explicit from the Changes UI

### Diff UX

- Side-by-side and unified diff for selected file
- Line staging (hunk / line)
- Ignore whitespace toggle

### LLM commit messages

- Provider trait (`OpenAI`, `Anthropic`, `OpenRouter`, local HTTP)
- Secure API key storage (OS keychain / credential manager)
- Prompt built from staged diff + optional user hint
- Preview / edit before commit
- Offline fallback when no provider is configured

### CLI

- Ship / expose `sgg` as a command-line entry point (on `PATH` after install)
- Open a repository from the shell: `sgg [path]` (default: cwd; resolve nearest `.git`)
- Common flags: `--help`, `--version`, optional `--page git|files|settings`
- Headless-friendly error messages when path is not a git repo
- Document install paths so shells and agents can invoke the same binary as the GUI

### Packaging

- Smaller release binary (`LTO`, strip, size-oriented profile)
- Windows installer (optional context-menu registration)
- macOS `.app` bundle + notarization notes
- Linux AppImage / `.deb`

### Polish

- Keyboard shortcuts (stage, commit, push, pull, refresh)
- Multi-repo recent list
- Theme (light/dark) following OS when possible
- Localization (en / ko) for UI strings

### Future

- Embed / reuse the editor stack used by [Zed](https://zed.dev) (Zed’s editor component) for in-app file editing and preview, replacing or extending the current basic text preview
