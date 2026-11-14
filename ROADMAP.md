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
- Rebase (onto branch, continue/abort/skip)
- Merge + conflict status surface
- Stash list / push / pop / apply / drop
- Cherry-pick
- Tag create / delete / push
- Remote add / rename / remove / set-url

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
