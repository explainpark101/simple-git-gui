//! App auto-update via GitHub Release `latest.json`.
//!
//! Stable fetch URL (always the newest release asset named `latest.json`):
//! `https://github.com/<repo>/releases/latest/download/latest.json`
//!
//! If that asset is missing, falls back to the GitHub Releases API and maps
//! known asset filenames to the same download keys.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

pub const GITHUB_REPO: &str = "explainpark101/simple-git-gui";

pub fn latest_json_url() -> String {
    format!("https://github.com/{GITHUB_REPO}/releases/latest/download/latest.json")
}

fn releases_latest_api_url() -> String {
    format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest")
}

#[derive(Debug, Clone, Deserialize)]
pub struct LatestInfo {
    pub version: String,
    pub tag: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    pub downloads: HashMap<String, DownloadAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadAsset {
    pub name: String,
    pub url: String,
}

impl LatestInfo {
    pub fn asset_for_key(&self, key: &str) -> Option<&DownloadAsset> {
        self.downloads.get(key)
    }

    /// Preferred installer for this host, falling back to portable when needed.
    pub fn asset_for_this_platform(&self) -> Option<&DownloadAsset> {
        let primary = platform_download_key();
        if let Some(asset) = self.asset_for_key(primary) {
            return Some(asset);
        }
        let portable = platform_portable_key();
        self.asset_for_key(portable)
    }
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn platform_download_key() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "macos-aarch64"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "macos-x64"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x64"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "windows-arm64"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x64"
    }
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
    )))]
    {
        "unknown"
    }
}

pub fn platform_portable_key() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "windows-x64-portable"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "windows-arm64-portable"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "linux-x64-portable"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
    )))]
    {
        "unknown"
    }
}

/// Compare dotted numeric versions (`1.4.2`, optional pre-release suffix ignored for ordering).
pub fn is_newer(remote: &str, local: &str) -> bool {
    parse_semver(remote) > parse_semver(local)
}

fn parse_semver(raw: &str) -> (u64, u64, u64) {
    let core = raw.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next().unwrap_or(core);
    let mut parts = core.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (major, minor, patch)
}

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(&format!("simple-git-gui/{}", current_version()))
        .build()
}

fn http_download_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(600))
        .user_agent(&format!("simple-git-gui/{}", current_version()))
        .build()
}

pub fn fetch_latest() -> Result<LatestInfo> {
    match fetch_latest_json() {
        Ok(info) => Ok(info),
        Err(json_err) => match fetch_latest_from_github_api() {
            Ok(info) => Ok(info),
            Err(api_err) => Err(json_err).context(format!("GitHub API fallback also failed: {api_err:#}")),
        },
    }
}

fn fetch_latest_json() -> Result<LatestInfo> {
    let url = latest_json_url();
    let response = http_agent()
        .get(&url)
        .call()
        .with_context(|| format!("GET {url}"))?;
    let info: LatestInfo = response.into_json().context("parse latest.json")?;
    if info.version.trim().is_empty() {
        bail!("latest.json missing version");
    }
    Ok(info)
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn fetch_latest_from_github_api() -> Result<LatestInfo> {
    let url = releases_latest_api_url();
    let response = http_agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .with_context(|| format!("GET {url}"))?;
    let release: GhRelease = response.into_json().context("parse GitHub release JSON")?;
    let version = release
        .tag_name
        .trim()
        .trim_start_matches('v')
        .to_string();
    if version.is_empty() {
        bail!("GitHub release missing tag_name");
    }
    let mut downloads = HashMap::new();
    for asset in release.assets {
        if let Some(key) = download_key_for_asset_name(&asset.name, &version) {
            downloads.insert(
                key.to_string(),
                DownloadAsset {
                    name: asset.name,
                    url: asset.browser_download_url,
                },
            );
        }
    }
    if downloads.is_empty() {
        bail!("GitHub release has no recognized installer assets");
    }
    Ok(LatestInfo {
        version,
        tag: release.tag_name,
        notes: release.html_url.or(release.body),
        published_at: release.published_at,
        downloads,
    })
}

fn download_key_for_asset_name(name: &str, version: &str) -> Option<&'static str> {
    let macos_aarch64 = format!("simple-git-gui_{version}_aarch64.dmg");
    let macos_x64 = format!("simple-git-gui_{version}_x64.dmg");
    let win_x64 = format!("sgg_{version}_x64-setup.exe");
    let win_arm64 = format!("sgg_{version}_arm64-setup.exe");
    let linux = format!("sgg_{version}_x86_64.AppImage");
    if name == macos_aarch64 {
        Some("macos-aarch64")
    } else if name == macos_x64 {
        Some("macos-x64")
    } else if name == win_x64 {
        Some("windows-x64")
    } else if name == win_arm64 {
        Some("windows-arm64")
    } else if name == linux {
        Some("linux-x64")
    } else if name == "sgg_portable.exe" {
        Some("windows-x64-portable")
    } else if name == "sgg_portable_arm64.exe" {
        Some("windows-arm64-portable")
    } else if name == "linux_sgg_portable" {
        Some("linux-x64-portable")
    } else {
        None
    }
}

pub fn download_file(url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    let response = http_download_agent()
        .get(url)
        .call()
        .with_context(|| format!("GET {url}"))?;
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

pub fn download_dir() -> PathBuf {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn open_downloaded(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("xattr")
            .args(["-cr"])
            .arg(path)
            .status();
        Command::new("open")
            .arg(path)
            .spawn()
            .with_context(|| format!("open {}", path.display()))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW — avoid a brief console flash when launching the installer.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        Command::new("cmd")
            .args(["/C", "start", "", &path.to_string_lossy()])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .with_context(|| format!("start {}", path.display()))?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(path) {
                let mut perms = meta.permissions();
                perms.set_mode(perms.mode() | 0o111);
                let _ = fs::set_permissions(path, perms);
            }
        }
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .with_context(|| format!("xdg-open {}", path.display()))?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err(anyhow!("open downloaded file is unsupported on this OS"))
    }
}

/// Fetch latest.json, download the platform installer, open it.
pub fn download_and_open_latest(info: &LatestInfo) -> Result<PathBuf> {
    let asset = info
        .asset_for_this_platform()
        .ok_or_else(|| {
            anyhow!(
                "no download for platform key {} (or portable)",
                platform_download_key()
            )
        })?;
    let dest = download_dir().join(&asset.name);
    download_file(&asset.url, &dest)?;
    open_downloaded(&dest)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions() {
        assert!(is_newer("1.4.2", "1.4.1"));
        assert!(!is_newer("1.4.1", "1.4.1"));
        assert!(!is_newer("1.4.0", "1.4.1"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("v1.4.2", "1.4.1"));
    }

    #[test]
    fn maps_asset_names() {
        assert_eq!(
            download_key_for_asset_name("sgg_1.4.2_x64-setup.exe", "1.4.2"),
            Some("windows-x64")
        );
        assert_eq!(
            download_key_for_asset_name("simple-git-gui_1.4.2_aarch64.dmg", "1.4.2"),
            Some("macos-aarch64")
        );
        assert_eq!(download_key_for_asset_name("latest.json", "1.4.2"), None);
    }
}
