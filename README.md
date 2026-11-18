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

### macOS installable package (`.app` + `.dmg`)

**Install from GitHub Releases** (avoids browser Gatekeeper quarantine):

```bash
curl -fsSL https://raw.githubusercontent.com/explainpark101/simple-git-gui/main/packaging/macos/install-from-github.sh | bash
```

If you already downloaded the DMG in a browser and macOS says it cannot verify the file:

```bash
xattr -cr ~/Downloads/simple-git-gui_*.dmg
open ~/Downloads/simple-git-gui_*.dmg
```

Then double-click **Install sgg.command** (or drag the `.app` to Applications).

On macOS, from the repo root (local package build):

```bash
bash build-dmg.sh
```

Artifacts land in `dist/`:

- `simple-git-gui.app`
- `simple-git-gui_<version>_aarch64.dmg` (Apple Silicon)

With Apple Developer ID + notary secrets configured in CI, the DMG is notarized and opens without Gatekeeper warnings. Without those secrets, builds are **ad-hoc signed** — use the `xattr` or `install-from-github.sh` path above.

`sgg` is symlinked to `/usr/local/bin/sgg` when writable, otherwise `~/.local/bin/sgg` (with a PATH snippet in `~/.zprofile`). Open a **new** terminal tab after install.

Optional re-register (PATH + Finder Services):

```bash
/Applications/simple-git-gui.app/Contents/MacOS/sgg install-shell
```

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

# Check / apply update from GitHub Releases (latest.json)
sgg check-update
sgg update
```

Settings → **Updates** can also check for a newer release and download the installer.

The Windows NSIS installer (`./build-exe.sh`) adds the install directory to the **user** `PATH`, so `sgg` is available in new terminals after setup. Uninstall removes that entry. On macOS, DMG install / first `.app` launch (or `install-shell`) registers a `sgg` symlink on `PATH` the same way.

### Shell integration

| OS | What `install-shell` does |
| --- | --- |
| Windows | HKCU Explorer context menu on folders / folder background: **여기서 sgg 열기** |
| macOS | `sgg` on `PATH` (symlink) + Finder Service **Open with sgg** under `~/Library/Services` |
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
  sgg-core/          git domain (no UI)
    find.rs          nearest .git walk
    state.rs         RepoState open / snapshot
    status.rs        changes list
    index_ops.rs     stage / unstage / commit
    branch.rs        list / create / switch
    remote.rs        remotes / push / pull / fetch
  sgg/               desktop app
    assets/icon/     app icon source PNG (embedded into Windows exe via build.rs)
    build.rs         generate multi-size .ico + winresource embed
    cli.rs           clap entry commands
    app/view.rs      window state
    app/ops.rs       UI → git actions
    app/render/      toolbar / changes / sidebar panels
    shell/           OS file-manager integration
```


Network git operations use the `git` CLI so credential helpers and SSH agents work like VS Code / Cursor.

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for terminal emulator, history, rebase, LLM commit messages, and packaging.
