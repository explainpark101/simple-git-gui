//! Detect / install / login for Cursor Agent CLI and OpenAI Codex CLI.
//!
//! Layout mirrors T3 Code's managed tools idea (`~/.t3/tools/codex/{version}`):
//! app-managed installs live under `{data}/simple-git-gui/tools/…` with
//! `active.json`. We also reuse an existing T3 managed Codex when present.
//!
//! Cursor is installed via the official install script (same as T3 docs);
//! Codex prefers the official `codex-package-*.tar.gz` GitHub release asset.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

/// 0.0..=1.0 install progress callback (Send so background tasks can report).
pub type InstallProgressFn = Arc<dyn Fn(f32) + Send + Sync>;

fn report(progress: Option<&InstallProgressFn>, value: f32) {
    if let Some(cb) = progress {
        cb(value.clamp(0.0, 1.0));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliToolKind {
    Cursor,
    Codex,
}

impl CliToolKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Cursor => "Cursor CLI",
            Self::Codex => "Codex CLI",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliStatus {
    pub binary: Option<String>,
    pub source: Option<&'static str>,
    pub logged_in: Option<bool>,
}

pub fn tools_root() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("simple-git-gui")
        .join("tools")
}

fn managed_codex_root() -> PathBuf {
    tools_root().join("codex")
}

fn managed_cursor_bin() -> PathBuf {
    #[cfg(windows)]
    {
        tools_root().join("cursor").join("agent.exe")
    }
    #[cfg(not(windows))]
    {
        tools_root().join("cursor").join("agent")
    }
}

fn looks_like_bin(path: &Path) -> bool {
    path.is_file()
}

fn path_if_bin(path: &Path) -> Option<String> {
    if looks_like_bin(path) {
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Find Cursor Agent without installing. Prefer `cursor-agent` (T3 docs), then `agent`.
pub fn detect_cursor() -> Option<(String, &'static str)> {
    if let Some(p) = which_first(&["cursor-agent", "agent"]) {
        return Some((p, "path"));
    }
    if let Some(p) = path_if_bin(&managed_cursor_bin()) {
        return Some((p, "managed"));
    }
    detect_cursor_common_paths().map(|p| (p, "common"))
}

/// Find Codex without installing.
pub fn detect_codex() -> Option<(String, &'static str)> {
    if let Some(p) = which_first(&["codex"]) {
        return Some((p, "path"));
    }
    if let Some(p) = detect_sgg_managed_codex() {
        return Some((p, "managed"));
    }
    if let Some(p) = detect_t3_codex() {
        return Some((p, "t3"));
    }
    detect_codex_common_paths().map(|p| (p, "common"))
}

pub fn detect_cursor_bin() -> Option<String> {
    detect_cursor().map(|(p, _)| p)
}

pub fn detect_codex_bin() -> Option<String> {
    detect_codex().map(|(p, _)| p)
}

fn managed_gh_bin() -> PathBuf {
    #[cfg(windows)]
    {
        tools_root().join("gh").join("gh.exe")
    }
    #[cfg(not(windows))]
    {
        tools_root().join("gh").join("gh")
    }
}

/// Find GitHub CLI (`gh`) — PATH first, then app-managed install.
pub fn detect_gh_bin() -> Option<String> {
    if let Some(p) = which_first(&["gh"]) {
        return Some(p);
    }
    path_if_bin(&managed_gh_bin())
}

/// Download the official portable `gh` release into `{tools}/gh/` and return its path.
pub fn install_gh(progress: Option<InstallProgressFn>) -> Result<String> {
    install_gh_with_progress(progress.as_ref())
}

fn install_gh_with_progress(progress: Option<&InstallProgressFn>) -> Result<String> {
    report(progress, 0.02);
    let (tag, version) = lookup_gh_latest_version().unwrap_or_else(|_| {
        ("v2.105.0".into(), "2.105.0".into())
    });
    let asset = platform_gh_asset(&version)?;
    let url = format!("https://github.com/cli/cli/releases/download/{tag}/{asset}");
    let root = tools_root().join("gh");
    let version_dir = root.join(&version);
    if version_dir.exists() {
        let _ = fs::remove_dir_all(&version_dir);
    }
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("create {}", version_dir.display()))?;
    let archive = version_dir.join(&asset);
    download_file(&url, &archive, progress, 0.05, 0.78)?;
    report(progress, 0.8);
    extract_archive(&archive, &version_dir)?;
    let _ = fs::remove_file(&archive);
    report(progress, 0.88);

    #[cfg(windows)]
    let exe_name = "gh.exe";
    #[cfg(not(windows))]
    let exe_name = "gh";

    let found = find_named_bin(&version_dir, exe_name)
        .ok_or_else(|| anyhow!("gh archive missing {exe_name}"))?;
    let dest = managed_gh_bin();
    mirror_bin_to(&found, &dest)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms)?;
    }

    // Keep a small pointer so we can find the active managed binary later.
    let _ = fs::write(
        root.join("active.json"),
        serde_json::to_string_pretty(&ActiveRelease {
            version: version.clone(),
        })?,
    );

    report(progress, 1.0);
    path_if_bin(&dest).ok_or_else(|| anyhow!("installed gh missing at {}", dest.display()))
}

fn lookup_gh_latest_version() -> Result<(String, String)> {
    #[derive(Deserialize)]
    struct GhRelease {
        tag_name: String,
    }
    let release: GhRelease = http_agent(30)
        .get("https://api.github.com/repos/cli/cli/releases/latest")
        .set("Accept", "application/vnd.github+json")
        .call()
        .context("GET cli/cli latest release")?
        .into_json()
        .context("parse cli/cli release JSON")?;
    let tag = release.tag_name.trim().to_string();
    if tag.is_empty() {
        bail!("empty gh release tag");
    }
    let version = tag.trim_start_matches('v').to_string();
    if version.is_empty() {
        bail!("empty gh version");
    }
    Ok((tag, version))
}

fn platform_gh_asset(version: &str) -> Result<String> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        return Ok(format!("gh_{version}_windows_amd64.zip"));
    }
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        return Ok(format!("gh_{version}_windows_arm64.zip"));
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Ok(format!("gh_{version}_macOS_arm64.zip"));
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Ok(format!("gh_{version}_macOS_amd64.zip"));
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok(format!("gh_{version}_linux_amd64.tar.gz"));
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Ok(format!("gh_{version}_linux_arm64.tar.gz"));
    }
    #[allow(unreachable_code)]
    {
        bail!("unsupported platform for managed gh install")
    }
}

pub fn status(kind: CliToolKind) -> CliStatus {
    let (binary, source) = match kind {
        CliToolKind::Cursor => detect_cursor()
            .map(|(b, s)| (Some(b), Some(s)))
            .unwrap_or((None, None)),
        CliToolKind::Codex => detect_codex()
            .map(|(b, s)| (Some(b), Some(s)))
            .unwrap_or((None, None)),
    };
    let logged_in = binary.as_deref().and_then(|bin| probe_login(kind, bin));
    CliStatus {
        binary,
        source,
        logged_in,
    }
}

pub fn ensure_cli(kind: CliToolKind) -> Result<String> {
    ensure_cli_with_progress(kind, None)
}

pub fn ensure_cli_with_progress(
    kind: CliToolKind,
    progress: Option<InstallProgressFn>,
) -> Result<String> {
    match kind {
        CliToolKind::Cursor => ensure_cursor_with_progress(progress),
        CliToolKind::Codex => ensure_codex_with_progress(progress),
    }
}

pub fn ensure_cursor() -> Result<String> {
    ensure_cursor_with_progress(None)
}

pub fn ensure_codex() -> Result<String> {
    ensure_codex_with_progress(None)
}

pub fn ensure_cursor_with_progress(progress: Option<InstallProgressFn>) -> Result<String> {
    if let Some((p, _)) = detect_cursor() {
        report(progress.as_ref(), 1.0);
        return Ok(p);
    }
    report(progress.as_ref(), 0.05);
    install_cursor(progress.as_ref())?;
    report(progress.as_ref(), 0.95);
    let bin = detect_cursor_bin().ok_or_else(|| {
        anyhow!("Cursor Agent install finished but `cursor-agent` / `agent` was not found")
    })?;
    report(progress.as_ref(), 1.0);
    Ok(bin)
}

pub fn ensure_codex_with_progress(progress: Option<InstallProgressFn>) -> Result<String> {
    if let Some((p, _)) = detect_codex() {
        report(progress.as_ref(), 1.0);
        return Ok(p);
    }
    report(progress.as_ref(), 0.02);
    if let Err(package_err) = install_codex_package(progress.as_ref()) {
        report(progress.as_ref(), 0.05);
        if let Err(bare_err) = install_codex_bare_exe(progress.as_ref()) {
            report(progress.as_ref(), 0.1);
            install_codex_via_official_script(progress.as_ref()).with_context(|| {
                format!(
                    "managed package failed ({package_err:#}); bare download failed ({bare_err:#}); official installer also failed"
                )
            })?;
        }
    }
    report(progress.as_ref(), 0.98);
    let bin = detect_codex_bin()
        .ok_or_else(|| anyhow!("Codex CLI install finished but `codex` was not found"))?;
    report(progress.as_ref(), 1.0);
    Ok(bin)
}

/// Open an interactive login terminal for the CLI (browser OAuth).
pub fn open_login(kind: CliToolKind) -> Result<()> {
    let bin = ensure_cli(kind)?;
    spawn_interactive_login(&bin, "login")
}

fn probe_login(kind: CliToolKind, bin: &str) -> Option<bool> {
    match kind {
        CliToolKind::Cursor => {
            let out = run_capture(bin, &["status", "--format", "json"]).ok()?;
            if out.contains("\"loggedIn\":true")
                || out.contains("\"logged_in\":true")
                || out.to_lowercase().contains("logged in")
            {
                return Some(true);
            }
            if out.contains("\"loggedIn\":false") || out.contains("\"logged_out\"") {
                return Some(false);
            }
            // Non-empty status without hard failure → treat as unknown/ok.
            Some(!out.trim().is_empty())
        }
        CliToolKind::Codex => {
            // `codex login status` exists on recent builds; fall back to help probe.
            if let Ok(out) = run_capture(bin, &["login", "status"]) {
                let lower = out.to_lowercase();
                if lower.contains("logged") && lower.contains("in") && !lower.contains("not") {
                    return Some(true);
                }
                if lower.contains("not logged") || lower.contains("logged out") {
                    return Some(false);
                }
            }
            run_capture(bin, &["--version"]).ok().map(|v| !v.trim().is_empty())
        }
    }
}

fn detect_sgg_managed_codex() -> Option<String> {
    let root = managed_codex_root();
    let active_path = root.join("active.json");
    let text = fs::read_to_string(&active_path).ok()?;
    let active: ActiveRelease = serde_json::from_str(&text).ok()?;
    let version = active.version.trim();
    if version.is_empty() {
        return None;
    }
    #[cfg(windows)]
    let name = "codex.exe";
    #[cfg(not(windows))]
    let name = "codex";
    let bin = root.join(version).join("bin").join(name);
    path_if_bin(&bin).or_else(|| path_if_bin(&root.join(name)))
}

fn detect_t3_codex() -> Option<String> {
    let home = dirs::home_dir()?;
    let active_path = home
        .join(".t3")
        .join("tools")
        .join("codex")
        .join("active.json");
    let text = fs::read_to_string(&active_path).ok()?;
    let active: ActiveRelease = serde_json::from_str(&text).ok()?;
    let version = active.version.trim();
    if version.is_empty() {
        return None;
    }
    #[cfg(windows)]
    let name = "codex.exe";
    #[cfg(not(windows))]
    let name = "codex";
    let bin = home
        .join(".t3")
        .join("tools")
        .join("codex")
        .join(version)
        .join("bin")
        .join(name);
    path_if_bin(&bin)
}

fn detect_cursor_common_paths() -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        for name in ["cursor-agent", "agent"] {
            candidates.push(home.join(".local").join("bin").join(name));
            #[cfg(windows)]
            {
                candidates.push(home.join(".local").join("bin").join(format!("{name}.exe")));
                candidates.push(home.join(".local").join("bin").join(format!("{name}.cmd")));
            }
            candidates.push(home.join(".cursor").join("bin").join(name));
            #[cfg(windows)]
            {
                candidates.push(home.join(".cursor").join("bin").join(format!("{name}.exe")));
            }
        }
    }
    if let Some(local) = dirs::data_local_dir() {
        for name in ["agent.exe", "cursor-agent.exe", "agent", "cursor-agent"] {
            candidates.push(local.join("cursor-agent").join(name));
        }
    }
    if let Some(roaming) = dirs::config_dir() {
        // npm global shims on Windows often land in %APPDATA%\npm
        for name in ["agent.cmd", "cursor-agent.cmd", "agent.exe", "cursor-agent.exe"] {
            candidates.push(roaming.join("npm").join(name));
        }
    }
    for path in candidates {
        if let Some(p) = path_if_bin(&path) {
            return Some(p);
        }
    }
    None
}

fn detect_codex_common_paths() -> Option<String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local").join("bin").join("codex"));
        #[cfg(windows)]
        {
            candidates.push(home.join(".local").join("bin").join("codex.exe"));
            candidates.push(home.join(".local").join("bin").join("codex.cmd"));
        }
        candidates.push(home.join(".npm-global").join("bin").join("codex"));
    }
    if let Some(roaming) = dirs::config_dir() {
        candidates.push(roaming.join("npm").join("codex.cmd"));
        candidates.push(roaming.join("npm").join("codex.exe"));
    }
    for path in candidates {
        if let Some(p) = path_if_bin(&path) {
            return Some(p);
        }
    }
    None
}

fn install_cursor(progress: Option<&InstallProgressFn>) -> Result<()> {
    report(progress, 0.15);
    #[cfg(windows)]
    {
        let status = run_hidden(
            "powershell",
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "irm 'https://cursor.com/install?win32=true' | iex",
            ],
        )?;
        if !status.success() {
            bail!(
                "Cursor Agent installer exited {}",
                status.code().unwrap_or(-1)
            );
        }
    }
    #[cfg(not(windows))]
    {
        let status = run_hidden(
            "bash",
            &["-lc", "curl -fsS https://cursor.com/install | bash"],
        )?;
        if !status.success() {
            bail!(
                "Cursor Agent installer exited {}",
                status.code().unwrap_or(-1)
            );
        }
    }
    report(progress, 0.8);

    if let Some(found) = which_first(&["cursor-agent", "agent"]).or_else(detect_cursor_common_paths)
    {
        let _ = mirror_bin_to(&PathBuf::from(&found), &managed_cursor_bin());
    }
    report(progress, 0.9);
    Ok(())
}

fn install_codex_via_official_script(progress: Option<&InstallProgressFn>) -> Result<()> {
    report(progress, 0.2);
    #[cfg(windows)]
    {
        let status = run_hidden(
            "powershell",
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "irm https://chatgpt.com/codex/install.ps1 | iex",
            ],
        )?;
        if !status.success() {
            bail!(
                "Codex official installer exited {}",
                status.code().unwrap_or(-1)
            );
        }
    }
    #[cfg(not(windows))]
    {
        let status = run_hidden(
            "bash",
            &[
                "-lc",
                "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
            ],
        )?;
        if !status.success() {
            bail!(
                "Codex official installer exited {}",
                status.code().unwrap_or(-1)
            );
        }
    }
    report(progress, 0.7);
    if let Some(found) = which_first(&["codex"]) {
        let dest = managed_codex_root().join(if cfg!(windows) {
            "codex.exe"
        } else {
            "codex"
        });
        let _ = mirror_bin_to(&PathBuf::from(&found), &dest);
        let _ = write_active_release("path-mirror");
    }
    report(progress, 0.9);
    Ok(())
}

/// T3-style: download `codex-package-{target}.tar.gz` into versioned managed dir.
fn install_codex_package(progress: Option<&InstallProgressFn>) -> Result<()> {
    let target = platform_codex_target()?;
    report(progress, 0.05);
    let version = lookup_codex_npm_version().unwrap_or_else(|_| "0.170.0".into());
    let file_name = format!("codex-package-{target}.tar.gz");
    let url = format!(
        "https://github.com/openai/codex/releases/download/rust-v{version}/{file_name}"
    );
    let version_dir = managed_codex_root().join(&version);
    fs::create_dir_all(&version_dir)
        .with_context(|| format!("create {}", version_dir.display()))?;
    let archive = version_dir.join(&file_name);
    download_file(&url, &archive, progress, 0.08, 0.75)?;
    report(progress, 0.78);
    extract_archive(&archive, &version_dir)?;
    report(progress, 0.9);
    let _ = fs::remove_file(&archive);

    #[cfg(windows)]
    let exe_name = "codex.exe";
    #[cfg(not(windows))]
    let exe_name = "codex";
    let bin = version_dir.join("bin").join(exe_name);
    if !looks_like_bin(&bin) {
        // Some archives flatten differently — search.
        if let Some(found) = find_named_bin(&version_dir, exe_name) {
            let dest_dir = version_dir.join("bin");
            fs::create_dir_all(&dest_dir)?;
            let dest = dest_dir.join(exe_name);
            if found != dest {
                fs::copy(&found, &dest)
                    .with_context(|| format!("copy {} → {}", found.display(), dest.display()))?;
            }
        } else {
            bail!("Codex package missing bin/{exe_name}");
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let bin = version_dir.join("bin").join(exe_name);
        let mut perms = fs::metadata(&bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&bin, perms)?;
    }

    let complete = InstalledRelease {
        version: version.clone(),
        target: target.to_string(),
        sha256: String::new(),
    };
    fs::write(
        version_dir.join(".install-complete.json"),
        serde_json::to_string_pretty(&complete)?,
    )?;
    write_active_release(&version)?;
    report(progress, 0.95);
    Ok(())
}

/// Lighter fallback: single platform exe (enough for `codex exec`).
fn install_codex_bare_exe(progress: Option<&InstallProgressFn>) -> Result<()> {
    let asset = platform_codex_bare_asset()?;
    report(progress, 0.08);
    let version = lookup_codex_npm_version().unwrap_or_else(|_| "0.170.0".into());
    let version_dir = managed_codex_root().join(&version);
    let bin_dir = version_dir.join("bin");
    fs::create_dir_all(&bin_dir).with_context(|| format!("create {}", bin_dir.display()))?;
    #[cfg(windows)]
    let dest = bin_dir.join("codex.exe");
    #[cfg(not(windows))]
    let dest = bin_dir.join("codex");

    let url = format!(
        "https://github.com/openai/codex/releases/download/rust-v{version}/{asset}"
    );
    if asset.ends_with(".tar.gz") {
        let archive = version_dir.join(asset);
        download_file(&url, &archive, progress, 0.1, 0.8)?;
        report(progress, 0.82);
        extract_archive(&archive, &version_dir)?;
        let _ = fs::remove_file(&archive);
        if let Some(found) = find_named_bin(&version_dir, "codex") {
            if found != dest {
                let _ = fs::remove_file(&dest);
                fs::rename(&found, &dest).or_else(|_| {
                    fs::copy(&found, &dest).map(|_| ())
                })?;
            }
        }
    } else {
        download_file(&url, &dest, progress, 0.1, 0.9)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms)?;
    }

    if !looks_like_bin(&dest) {
        bail!("downloaded Codex binary missing at {}", dest.display());
    }
    write_active_release(&version)?;
    report(progress, 0.95);
    Ok(())
}

fn write_active_release(version: &str) -> Result<()> {
    let root = managed_codex_root();
    fs::create_dir_all(&root)?;
    let active = ActiveRelease {
        version: version.to_string(),
    };
    fs::write(
        root.join("active.json"),
        serde_json::to_string_pretty(&active)?,
    )?;
    Ok(())
}

fn lookup_codex_npm_version() -> Result<String> {
    let agent = http_agent(30);
    #[derive(Deserialize)]
    struct NpmLatest {
        version: String,
    }
    let latest: NpmLatest = agent
        .get("https://registry.npmjs.org/@openai/codex/latest")
        .call()
        .context("GET @openai/codex latest")?
        .into_json()
        .context("parse npm latest JSON")?;
    let v = latest.version.trim().to_string();
    if v.is_empty() {
        bail!("empty npm version");
    }
    Ok(v)
}

fn platform_codex_target() -> Result<&'static str> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        return Ok("x86_64-pc-windows-msvc");
    }
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        return Ok("aarch64-pc-windows-msvc");
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Ok("aarch64-apple-darwin");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Ok("x86_64-apple-darwin");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("x86_64-unknown-linux-musl");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Ok("aarch64-unknown-linux-musl");
    }
    #[allow(unreachable_code)]
    {
        bail!("unsupported platform for managed Codex package")
    }
}

fn platform_codex_bare_asset() -> Result<&'static str> {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        return Ok("codex-x86_64-pc-windows-msvc.exe");
    }
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        return Ok("codex-aarch64-pc-windows-msvc.exe");
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        return Ok("codex-aarch64-apple-darwin.tar.gz");
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        return Ok("codex-x86_64-apple-darwin.tar.gz");
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        return Ok("codex-x86_64-unknown-linux-musl.tar.gz");
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        return Ok("codex-aarch64-unknown-linux-musl.tar.gz");
    }
    #[allow(unreachable_code)]
    {
        bail!("unsupported platform for bare Codex download")
    }
}

fn http_agent(read_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(read_secs))
        .user_agent(&format!(
            "simple-git-gui/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
}

fn download_file(
    url: &str,
    dest: &Path,
    progress: Option<&InstallProgressFn>,
    progress_start: f32,
    progress_end: f32,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let response = http_agent(600)
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let total = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0);
    let mut reader = response.into_reader();
    let tmp = dest.with_extension(format!(
        "{}.partial",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("tmp")
    ));
    {
        let mut file = File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        let mut buf = [0u8; 64 * 1024];
        let mut copied: u64 = 0;
        let mut last_report = progress_start;
        report(progress, progress_start);
        loop {
            let n = reader.read(&mut buf).context("download body")?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).context("write download chunk")?;
            copied += n as u64;
            let frac = if let Some(total) = total {
                let t = (copied as f32 / total as f32).clamp(0.0, 1.0);
                progress_start + (progress_end - progress_start) * t
            } else {
                // Unknown size: ease toward progress_end without finishing.
                let eased = 1.0 - (-(copied as f32) / (8.0 * 1024.0 * 1024.0)).exp();
                progress_start + (progress_end - progress_start) * eased * 0.95
            };
            if frac - last_report >= 0.01 || frac >= progress_end - 0.001 {
                report(progress, frac);
                last_report = frac;
            }
        }
        file.flush()?;
    }
    report(progress, progress_end);
    fs::rename(&tmp, dest).with_context(|| {
        format!(
            "rename {} → {}",
            tmp.display(),
            dest.display()
        )
    })?;
    Ok(())
}

fn extract_archive(archive: &Path, out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir).with_context(|| format!("create {}", out_dir.display()))?;
    let tar = system_tar();
    let archive_s = archive
        .to_str()
        .ok_or_else(|| anyhow!("non-utf8 archive path"))?;
    let out_s = out_dir
        .to_str()
        .ok_or_else(|| anyhow!("non-utf8 out dir"))?;
    // bsdtar: .zip → -xf; .tar.gz → -xzf
    let name = archive
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let flags: &[&str] = if name.ends_with(".zip") {
        &["-xf", archive_s, "-C", out_s]
    } else {
        &["-xzf", archive_s, "-C", out_s]
    };
    let status = run_hidden(&tar, flags)?;
    if !status.success() {
        bail!("tar extract failed ({})", status.code().unwrap_or(-1));
    }
    Ok(())
}

fn system_tar() -> String {
    #[cfg(windows)]
    {
        // T3 uses System32 bsdtar — Git-for-Windows GNU tar can fail on some archives.
        let system_root = std::env::var("SystemRoot")
            .or_else(|_| std::env::var("windir"))
            .unwrap_or_else(|_| r"C:\Windows".into());
        format!(r"{system_root}\System32\tar.exe")
    }
    #[cfg(not(windows))]
    {
        "tar".into()
    }
}

fn find_named_bin(root: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if entry.file_name().to_string_lossy() == name
                || entry.file_name().to_string_lossy().starts_with("codex-")
                    && !entry.file_name().to_string_lossy().contains('.')
            {
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn mirror_bin_to(src: &Path, dest: &Path) -> Result<()> {
    if !looks_like_bin(src) {
        bail!("source missing: {}", src.display());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    if dest.exists() {
        let _ = fs::remove_file(dest);
    }
    fs::copy(src, dest)
        .with_context(|| format!("copy {} → {}", src.display(), dest.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dest, perms)?;
    }
    Ok(())
}

fn which_first(names: &[&str]) -> Option<String> {
    for name in names {
        if let Some(p) = which_bin(name) {
            return Some(p);
        }
    }
    None
}

fn which_bin(name: &str) -> Option<String> {
    #[cfg(windows)]
    let finder = "where";
    #[cfg(not(windows))]
    let finder = "which";

    let mut cmd = Command::new(finder);
    cmd.arg(name)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_console(&mut cmd);
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next()?.trim();
    if first.is_empty() {
        return None;
    }
    Some(first.to_string())
}

fn run_capture(program: &str, args: &[&str]) -> Result<String> {
    let mut cmd = command_for(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut cmd);
    let output = cmd.output().with_context(|| format!("spawn {program}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() && stdout.is_empty() {
        bail!("{program} exited {}: {stderr}", output.status.code().unwrap_or(-1));
    }
    if !stdout.is_empty() {
        Ok(stdout)
    } else {
        Ok(stderr)
    }
}

fn run_hidden(program: &str, args: &[&str]) -> Result<std::process::ExitStatus> {
    let mut cmd = command_for(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut cmd);
    let output = cmd
        .output()
        .with_context(|| format!("spawn {program}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            bail!("{program} failed: {stderr}");
        }
    }
    Ok(output.status)
}

fn spawn_interactive_login(bin: &str, subcommand: &str) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        let mut cmd = command_for(bin);
        cmd.arg(subcommand).creation_flags(CREATE_NEW_CONSOLE);
        cmd.spawn()
            .with_context(|| format!("spawn interactive {bin} {subcommand}"))?;
    }
    #[cfg(not(windows))]
    {
        // Prefer a terminal when available; otherwise just spawn (browser flow).
        let term = which_first(&["x-terminal-emulator", "gnome-terminal", "xterm", "open"]);
        if let Some(term) = term {
            if term.ends_with("open") {
                // macOS: open a Terminal window running the login command.
                let script = format!("{bin} {subcommand}");
                Command::new("osascript")
                    .args([
                        "-e",
                        &format!("tell application \"Terminal\" to do script \"{script}\""),
                    ])
                    .spawn()
                    .context("osascript Terminal login")?;
            } else {
                Command::new(term)
                    .args(["-e", bin, subcommand])
                    .spawn()
                    .with_context(|| format!("spawn terminal for {bin} {subcommand}"))?;
            }
        } else {
            Command::new(bin)
                .arg(subcommand)
                .spawn()
                .with_context(|| format!("spawn {bin} {subcommand}"))?;
        }
    }
    Ok(())
}

fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Build a [`Command`] that can run Windows `.cmd` / `.bat` / `.ps1` wrappers.
///
/// `CreateProcess` cannot pass arguments to batch files directly ("batch file
/// arguments are invalid"). Cursor Agent's `cursor-agent.cmd` forwards to a
/// sibling `.ps1`, so prefer that; otherwise run via `cmd.exe /C`.
pub(crate) fn command_for(program: &str) -> Command {
    #[cfg(windows)]
    {
        let path = Path::new(program);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "cmd" || ext == "bat" {
            let ps1 = path.with_extension("ps1");
            if ps1.is_file() {
                let mut cmd = Command::new(powershell_exe());
                cmd.arg("-NoProfile")
                    .arg("-ExecutionPolicy")
                    .arg("Bypass")
                    .arg("-File")
                    .arg(ps1);
                return cmd;
            }
            let mut cmd = Command::new("cmd.exe");
            cmd.arg("/D").arg("/C").arg(program);
            return cmd;
        }
        if ext == "ps1" {
            let mut cmd = Command::new(powershell_exe());
            cmd.arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(program);
            return cmd;
        }
    }
    Command::new(program)
}

#[cfg(windows)]
fn powershell_exe() -> PathBuf {
    std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("windir"))
        .map(|root| {
            PathBuf::from(root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe")
        })
        .unwrap_or_else(|| {
            PathBuf::from(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe")
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveRelease {
    version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledRelease {
    version: String,
    target: String,
    #[serde(default)]
    sha256: String,
}
