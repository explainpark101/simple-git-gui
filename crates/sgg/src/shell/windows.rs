use std::path::Path;

use anyhow::{Context, Result};
use winreg::enums::*;
use winreg::RegKey;

const MENU_NAME: &str = "OpenSgg";
const MENU_LABEL: &str = "여기서 sgg 열기";

pub fn install(exe: &Path) -> Result<()> {
    let exe = exe
        .canonicalize()
        .with_context(|| format!("canonicalize {}", exe.display()))?;
    let exe_str = exe.to_string_lossy().replace('/', "\\");
    let command = format!("\"{}\" \"%V\"", exe_str);

    write_menu(r"Software\Classes\Directory\shell", &command, &exe_str)?;
    write_menu(
        r"Software\Classes\Directory\Background\shell",
        &command,
        &exe_str,
    )?;
    Ok(())
}

fn write_menu(parent: &str, command: &str, exe: &str) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let shell = hkcu
        .create_subkey(parent)
        .with_context(|| format!("create {parent}"))?
        .0;
    let menu = shell
        .create_subkey(MENU_NAME)
        .context("create OpenSgg menu key")?
        .0;
    menu.set_value("", &MENU_LABEL)?;
    menu.set_value("Icon", &format!("{},0", exe))?;
    let cmd = menu.create_subkey("command")?.0;
    cmd.set_value("", &command)?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(r"Software\Classes\Directory\shell\OpenSgg");
    let _ = hkcu.delete_subkey_all(r"Software\Classes\Directory\Background\shell\OpenSgg");
    Ok(())
}