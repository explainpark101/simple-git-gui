//! File-manager context-menu / service registration.

use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;

pub fn install() -> Result<()> {
    let exe = current_exe()?;
    #[cfg(windows)]
    {
        windows::install(&exe)?;
    }
    #[cfg(target_os = "macos")]
    {
        macos::install(&exe)?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux::install(&exe)?;
    }
    Ok(())
}

/// macOS: ensure `sgg` is on PATH when the GUI runs from a `.app` (DMG install).
#[cfg(target_os = "macos")]
pub fn ensure_cli_path_from_bundle() -> Result<()> {
    macos::ensure_cli_path_from_bundle()
}

pub fn uninstall() -> Result<()> {
    #[cfg(windows)]
    {
        windows::uninstall()?;
    }
    #[cfg(target_os = "macos")]
    {
        macos::uninstall()?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux::uninstall()?;
    }
    Ok(())
}

fn current_exe() -> Result<PathBuf> {
    env::current_exe().context("resolve current executable path")
}