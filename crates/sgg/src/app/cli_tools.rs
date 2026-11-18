//! Detect and install Cursor Agent CLI / OpenAI Codex CLI inside the app.
//!
//! Detection order mirrors T3 Code's managed-tools idea:
//! PATH → app-managed `…/simple-git-gui/tools/…` → T3 `~/.t3/tools/codex` →
//! common user install locations. When missing, Codex is downloaded from
//! GitHub Releases into the managed dir (official install script as fallback);
//! Cursor Agent uses the official install script, then is re-detected.

use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliToolKind {
    Cursor,
    Codex,
}

pub fn tools_root() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("simple-git-gui")
        .join("tools")
}

fn managed_codex_bin() -> PathBuf {
    #[cfg(windows)]
    {
        tools_root().join("codex").join("codex.exe")
    }
    #[cfg(not(windows))]
    {
        tools_root().join("codex").join("codex")
    }
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

/// Find Cursor Agent (`agent` / `cursor-agent`) without installing.
pub fn detect_cursor() -> Option<String> {
    which_first(&["agent", "cursor-agent"])
        .or_else(|| path_if_bin(&managed_cursor_bin()))
        .or_else(detect_cursor_common_paths)
}

/// Find Codex CLI without installing.
pub fn detect_codex() -> Option<String> {
    which_first(&["codex"])
        .or_else(|| path_if_bin(&managed_codex_bin()))
        .or_else(detect_t3_codex)
}

/// Ensure the tool exists: detect, otherwise install, then detect again.
pub fn ensure_cli(kind: CliToolKind) -> Result<String> {
    match kind {
        CliToolKind::Cursor => ensure_cursor(),
        CliToolKind::Codex => ensure_codex(),
    }
}

pub fn ensure_cursor() -> Result<String> {
    if let Some(p) = detect_cursor() {
        return Ok(p);
    }
    install_cursor()?;
    detect_cursor().ok_or_else(|| {
        anyhow!("Cursor Agent install finished but `agent` / `cursor-agent` was not found")
    })
}

pub fn ensure_codex() -> Result<String> {
    if let Some(p) = detect_codex() {
        return Ok(p);
    }
    if let Err(download_err) = install_codex_managed_download() {
        install_codex_via_official_script().with_context(|| {
            format!(
                "managed Codex download failed ({download_err:#}); official installer also failed"
            )
        })?;
    }
    detect_codex().ok_or_else(|| anyhow!("Codex CLI install finished but `codex` was not found"))
}

fn path_if_bin(path: &Path) -> Option<String> {
    if looks_like_bin(path) {
        Some(path.to_string_lossy().into_owned())
    } else {
        None
    }
}

fn detect_t3_codex() -> Option<String> {
    let home = dirs::home_dir()?;
    let active_path = home
        .join(".t3")
        .join("tools")
        .join("codex")
        .join("active.json");
    let text = fs::read_to_string(&active_path).ok()?;
    #[derive(Deserialize)]
    struct Active {
        version: String,
    }
    let active: Active = serde_json::from_str(&text).ok()?;
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
        candidates.push(home.join(".local").join("bin").join("agent"));
        candidates.push(home.join(".local").join("bin").join("cursor-agent"));
        #[cfg(windows)]
        {
            candidates.push(home.join(".local").join("bin").join("agent.exe"));
            candidates.push(home.join(".local").join("bin").join("cursor-agent.exe"));
            candidates.push(home.join(".local").join("bin").join("agent.cmd"));
        }
        candidates.push(home.join(".cursor").join("bin").join("agent"));
        #[cfg(windows)]
        {
            candidates.push(home.join(".cursor").join("bin").join("agent.exe"));
        }
    }
    if let Some(local) = dirs::data_local_dir() {
        candidates.push(local.join("cursor-agent").join("agent.exe"));
        candidates.push(local.join("cursor-agent").join("cursor-agent.exe"));
        candidates.push(local.join("cursor-agent").join("agent"));
        candidates.push(local.join("cursor-agent").join("cursor-agent"));
    }
    for path in candidates {
        if let Some(p) = path_if_bin(&path) {
            return Some(p);
        }
    }
    None
}

fn install_cursor() -> Result<()> {
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

    if let Some(found) = which_first(&["agent", "cursor-agent"]).or_else(detect_cursor_common_paths)
    {
        let _ = mirror_bin_to(&PathBuf::from(&found), &managed_cursor_bin());
    }
    Ok(())
}

fn install_codex_via_official_script() -> Result<()> {
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
    if let Some(found) = which_first(&["codex"]) {
        let _ = mirror_bin_to(&PathBuf::from(&found), &managed_codex_bin());
    }
    Ok(())
}

fn install_codex_managed_download() -> Result<()> {
    let asset = platform_codex_asset_name()?;
    let dest = managed_codex_bin();
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }

    let url = resolve_codex_asset_url(&asset)?;
    if asset.ends_with(".tar.gz") {
        let archive = dest.with_extension("tar.gz.download");
        download_file(&url, &archive)?;
        extract_codex_tarball(&archive, &dest)?;
        let _ = fs::remove_file(&archive);
    } else {
        download_file(&url, &dest)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&dest)
            .with_context(|| format!("stat {}", dest.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&dest, perms)
            .with_context(|| format!("chmod {}", dest.display()))?;
    }

    if !looks_like_bin(&dest) {
        bail!("downloaded Codex binary missing at {}", dest.display());
    }
    Ok(())
}

fn platform_codex_asset_name() -> Result<&'static str> {
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
        bail!("unsupported platform for managed Codex download")
    }
}

#[derive(Deserialize)]
struct GhRelease {
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn resolve_codex_asset_url(asset_name: &str) -> Result<String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(60))
        .user_agent(&format!(
            "simple-git-gui/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build();
    let release: GhRelease = agent
        .get("https://api.github.com/repos/openai/codex/releases/latest")
        .set("Accept", "application/vnd.github+json")
        .call()
        .context("GET openai/codex latest release")?
        .into_json()
        .context("parse GitHub release JSON")?;
    release
        .assets
        .into_iter()
        .find(|a| a.name == asset_name)
        .map(|a| a.browser_download_url)
        .ok_or_else(|| anyhow!("Codex release asset `{asset_name}` not found"))
}

fn download_file(url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(600))
        .user_agent(&format!(
            "simple-git-gui/{}",
            env!("CARGO_PKG_VERSION")
        ))
        .build();
    let response = agent.get(url).call().with_context(|| format!("GET {url}"))?;
    let mut reader = response.into_reader();
    let tmp = dest.with_extension(format!(
        "{}.partial",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("tmp")
    ));
    {
        let mut file = File::create(&tmp).with_context(|| format!("create {}", tmp.display()))?;
        copy(&mut reader, &mut file).context("download body")?;
        file.flush()?;
    }
    fs::rename(&tmp, dest).with_context(|| {
        format!(
            "rename {} → {}",
            tmp.display(),
            dest.display()
        )
    })?;
    Ok(())
}

fn extract_codex_tarball(archive: &Path, dest_bin: &Path) -> Result<()> {
    let out_dir = dest_bin
        .parent()
        .ok_or_else(|| anyhow!("Codex dest has no parent"))?;
    fs::create_dir_all(out_dir).with_context(|| format!("create {}", out_dir.display()))?;

    let status = run_hidden(
        "tar",
        &[
            "-xzf",
            archive
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 archive path"))?,
            "-C",
            out_dir
                .to_str()
                .ok_or_else(|| anyhow!("non-utf8 out dir"))?,
        ],
    )?;
    if !status.success() {
        bail!("tar extract failed ({})", status.code().unwrap_or(-1));
    }

    // Archives contain a platform-named binary (e.g. codex-aarch64-apple-darwin).
    let mut found: Option<PathBuf> = None;
    for entry in fs::read_dir(out_dir).with_context(|| format!("read {}", out_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("codex") && !name.ends_with(".tar.gz") && path.is_file() {
            if path == dest_bin {
                return Ok(());
            }
            found = Some(path);
            break;
        }
    }
    let Some(found) = found else {
        bail!("no codex binary inside {}", archive.display());
    };
    if found != dest_bin {
        if dest_bin.exists() {
            let _ = fs::remove_file(dest_bin);
        }
        fs::rename(&found, dest_bin).or_else(|_| {
            fs::copy(&found, dest_bin).map(|_| ()).with_context(|| {
                format!(
                    "copy {} → {}",
                    found.display(),
                    dest_bin.display()
                )
            })
        })?;
        if found.exists() && found != dest_bin {
            let _ = fs::remove_file(&found);
        }
    }
    Ok(())
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

fn run_hidden(program: &str, args: &[&str]) -> Result<std::process::ExitStatus> {
    let mut cmd = Command::new(program);
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

fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}
