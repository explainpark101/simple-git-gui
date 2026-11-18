# Auto-update specification

Status: implemented on **`feat/auto-update`** (landing on `main` via PR).  
Workspace version for the first auto-update release: **`1.4.2`**.

This spec is the source of truth for the feature.

## Goal

Ship a GitHub Release asset `latest.json` that records the release version and per-platform download URLs. The desktop app and CLI read that manifest, compare against `CARGO_PKG_VERSION`, and download + open the matching installer when a newer version exists.

Update does **not** silently replace a running binary in-place. It downloads the platform installer (DMG / NSIS / AppImage / portable) and opens it so the user finishes install and restarts `sgg`.

## Stable URLs

| Purpose | URL |
| --- | --- |
| Manifest (preferred) | `https://github.com/explainpark101/simple-git-gui/releases/latest/download/latest.json` |
| API fallback | `https://api.github.com/repos/explainpark101/simple-git-gui/releases/latest` |
| Release notes | `https://github.com/explainpark101/simple-git-gui/releases/tag/vX.Y.Z` |

`GITHUB_REPO` constant / script default: `explainpark101/simple-git-gui` (override in CI via `github.repository`).

## `latest.json` schema

```json
{
  "version": "1.4.2",
  "tag": "v1.4.2",
  "name": "simple-git-gui",
  "notes": "https://github.com/explainpark101/simple-git-gui/releases/tag/v1.4.2",
  "published_at": "2026-11-17T06:30:41Z",
  "downloads": {
    "macos-aarch64": {
      "name": "simple-git-gui_1.4.2_aarch64.dmg",
      "url": "https://github.com/explainpark101/simple-git-gui/releases/download/v1.4.2/simple-git-gui_1.4.2_aarch64.dmg"
    }
  }
}
```

### Fields

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `version` | string | yes | Semver without leading `v` (e.g. `1.4.2`) |
| `tag` | string | yes | Git tag (e.g. `v1.4.2`) |
| `name` | string | no | Product name; informational |
| `notes` | string | no | HTML/release page URL |
| `published_at` | string | no | ISO-8601 UTC |
| `downloads` | object | yes | Map of platform key → asset |

### Download asset object

| Field | Type | Required |
| --- | --- | --- |
| `name` | string | yes — filename as uploaded to the Release |
| `url` | string | yes — direct browser download URL |

### Platform download keys

| Key | Typical asset name pattern |
| --- | --- |
| `macos-aarch64` | `simple-git-gui_${VERSION}_aarch64.dmg` |
| `macos-x64` | `simple-git-gui_${VERSION}_x64.dmg` |
| `windows-x64` | `sgg_${VERSION}_x64-setup.exe` |
| `windows-arm64` | `sgg_${VERSION}_arm64-setup.exe` |
| `linux-x64` | `sgg_${VERSION}_x86_64.AppImage` |
| `windows-x64-portable` | `sgg_portable.exe` |
| `windows-arm64-portable` | `sgg_portable_arm64.exe` |
| `linux-x64-portable` | `linux_sgg_portable` |

Rules:

- Only include keys for assets that **actually exist** in the release upload set.
- Client picks primary key for the host OS/arch; if missing, falls back to the matching `*-portable` key.
- Unrecognized filenames in the asset dir are skipped (logged), not written into `downloads`.

## Version comparison

- Local version: `env!("CARGO_PKG_VERSION")`.
- Remote version: `latest.json` → `version` (leading `v` stripped; pre-release / build metadata after `-`/`+` ignored for ordering).
- Parse as `(major, minor, patch)` numeric triples; newer iff remote tuple `>` local.
- Examples: `1.4.2` > `1.4.1`; `v1.4.2` vs `1.4.1` → newer; equal → up to date.

## Release pipeline

Owned by `.github/workflows/release.yml` publish job, after artifacts are downloaded into `release-assets/`:

1. Run `packaging/generate-latest-json.sh release-assets` with env:
   - `VERSION` = workspace version from prepare job
   - `TAG` = `v${VERSION}`
   - `REPO` = `${{ github.repository }}`
2. Script scans `release-assets/*` (and one nested level), maps known filenames → keys, writes `release-assets/latest.json`.
3. Fail if zero recognized assets.
4. Upload **all** of `release-assets/**` (including `latest.json`) via `softprops/action-gh-release`.
5. Release body should mention the manifest URL and:

   ```bash
   sgg check-update
   sgg update
   ```

Generator script path: `packaging/generate-latest-json.sh` (must be executable / invoked via `bash`).

## Client behavior

### Module

`crates/sgg/src/app/update.rs` (dependency: `ureq` with `json` feature).

Responsibilities:

- `fetch_latest()` — GET `latest.json`; on missing asset / hard failure, fall back to GitHub Releases API and map known asset names to the same download keys.
- `is_newer(remote, local)`
- `platform_download_key()` / `platform_portable_key()`
- `download_file(url, dest)` — write via `*.partial` then rename
- `download_dir()` — `dirs::download_dir()` else home else `.`
- `open_downloaded(path)`:
  - macOS: `xattr -cr` then `open`
  - Windows: `cmd /C start "" path`
  - Linux: ensure executable bit, `xdg-open`
- `download_and_open_latest(&LatestInfo)`

### Preferences

`AppPrefs.auto_check_updates: bool` (default `true`, serde `default = true`).  
`AppPrefs.skipped_update_version: String` — set when the user chooses **Later**.

### UI (Settings → Updates)

- Show current version.
- Toggle “Check on launch”.
- Button **Check for updates** (`BusyKind::CheckUpdate`).
- When newer: show available version + **Download & open** (`BusyKind::DownloadUpdate`).
- Confirm dialog: title/desc with `{latest}` / `{current}`; OK downloads (async busy owned by download); Later skips that version on silent launch checks.
- i18n: English + Korean strings for all update labels/errors.

### Launch

If `auto_check_updates`, defer a **silent** check after window host is ready. Silent mode only opens the update dialog when a newer version exists (no error toast on network failure; skipped version stays quiet).

### CLI

| Command | Behavior | Exit |
| --- | --- | --- |
| `sgg check-update` | Print current / latest / tag / notes / asset; “up-to-date” or “available” | `0` up-to-date; `2` update available; `1` error |
| `sgg update` | If newer, download + open installer | `0` ok / already latest; `1` error |

Attach parent console on Windows for these subcommands (same pattern as `install-shell`).

## Non-goals (v1)

- In-place binary self-replace / Sparkle / MSIX silent update.
- Differential / delta patches.
- Forced / blocking updates.
- Signing verification beyond what the OS installer / Gatekeeper already does.
- Updating while a Git operation is mid-flight (busy flags already gate concurrent Check/Download).

## Related macOS distribution notes

Browser-downloaded DMGs may hit Gatekeeper when builds are ad-hoc signed. Separate from auto-update, but download-and-open should strip quarantine (`xattr -cr`) on macOS before `open`. See `docs/github-release.md` and `packaging/macos/install-from-github.sh`.

## Acceptance checklist

- [x] `packaging/generate-latest-json.sh` and Release job generate + upload `latest.json`.
- [ ] After a release, `…/releases/latest/download/latest.json` returns valid JSON with `version` and at least one `downloads` entry for that release’s real assets.
- [x] `crates/sgg/src/app/update.rs` fetches manifest (API fallback OK).
- [x] Settings Updates UI: check, silent launch check, download & open.
- [x] `sgg check-update` / `sgg update` CLI wired.
- [x] Workspace version bumped to `1.4.2` so clients on `1.4.1` detect the first release.
- [x] i18n EN/KO complete; Gatekeeper notes do not claim notarization without secrets.

## Merge plan

1. Merge `feat/auto-update` → `main` (this PR).
2. Push to `main` so Release workflow builds `v1.4.2` and uploads `latest.json`.
3. Smoke: `sgg check-update` / Settings → Updates against that release.
