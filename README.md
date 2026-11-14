# simple-git-gui (sgg)

Lightweight native Git GUI built with Rust and [GPUI](https://www.gpui.rs/) via [gpui-kit](https://gpui-kit.com).

Opens against the nearest `.git` root from a folder (or the current directory), then lets you inspect changes, commit, push/pull/fetch, and switch/create branches.

## Requirements

- Rust stable (recent)
- Git on `PATH`
- **Windows:** Windows SDK (for GPUI shader compile; `fxc.exe`). If build fails looking for `fxc.exe`, install the matching Windows 10/11 SDK or set `GPUI_FXC_PATH`.
- **macOS:** Xcode + command line tools
- **Linux:** usual GPUI deps (graphics / font libraries per gpui-kit docs)

## Build

```bash
cargo build --release -p sgg
```

Binary: `target/release/sgg` (or `sgg.exe` on Windows).

## Usage

```bash
# Open nearest repo from the current directory
sgg

# Open nearest repo from a path
sgg /path/to/project

# Register file-manager "Open with sgg" / "여기서 sgg 열기"
sgg install-shell

# Remove shell integration
sgg uninstall-shell
```

### Shell integration

| OS | What `install-shell` does |
| --- | --- |
| Windows | HKCU Explorer context menu on folders / folder background: **여기서 sgg 열기** |
| macOS | Finder Service **Open with sgg** under `~/Library/Services` (right-click folder → Services) |
| Linux | Nautilus script, `inode/directory` desktop entry, and file-manager action |

After installing on macOS, you may need to enable the service once under **System Settings → Keyboard → Keyboard Shortcuts → Services**.

## Layout (MVP)

- **Changes** — status list, multi-select, Stage / Unstage / Stage all, commit message + Commit
- **Toolbar** — Refresh, Fetch, Pull, Push
- **Branches** — local/remote list, switch local branch, create & switch
- **Remotes** — name + URL list

## Architecture

```
crates/
  sgg-core/   find .git, status/stage/commit/branch via git2; push/pull/fetch/switch via git CLI
  sgg/        GPUI app + clap CLI + shell integration
```

Network git operations use the `git` CLI so credential helpers and SSH agents work like VS Code / Cursor.

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for terminal emulator, history, rebase, LLM commit messages, and packaging.
