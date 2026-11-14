//! File-manager context-menu / service registration.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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

#[cfg(windows)]
mod windows {
    use super::*;
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

        write_menu(
            r"Software\Classes\Directory\shell",
            &command,
            &exe_str,
        )?;
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
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub fn install(exe: &Path) -> Result<()> {
        let home = dirs_home()?;
        let services = home.join("Library/Services");
        fs::create_dir_all(&services)?;

        let workflow = services.join("Open with sgg.workflow");
        if workflow.exists() {
            fs::remove_dir_all(&workflow)?;
        }
        let contents = workflow.join("Contents");
        fs::create_dir_all(&contents)?;

        let exe = exe
            .canonicalize()
            .with_context(|| format!("canonicalize {}", exe.display()))?;
        let exe_str = exe.to_string_lossy();

        // Minimal Automator/Services document that opens selected Finder folders in sgg.
        let document = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>NSServices</key>
  <array>
    <dict>
      <key>NSMenuItem</key>
      <dict>
        <key>default</key>
        <string>Open with sgg</string>
      </dict>
      <key>NSMessage</key>
      <string>runWorkflowAsService</string>
      <key>NSRequiredContext</key>
      <dict>
        <key>NSApplicationIdentifier</key>
        <string>com.apple.finder</string>
      </dict>
      <key>NSSendFileTypes</key>
      <array>
        <string>public.folder</string>
      </array>
    </dict>
  </array>
</dict>
</plist>
"#
        );
        fs::write(contents.join("Info.plist"), document)?;

        let wflow = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>actions</key>
  <array>
    <dict>
      <key>action</key>
      <string>com.apple.RunShellScript</string>
      <key>ActionParameters</key>
      <dict>
        <key>COMMAND_STRING</key>
        <string>"{exe}" "$@"</string>
        <key>shell</key>
        <string>/bin/zsh</string>
        <key>inputMethod</key>
        <integer>1</integer>
      </dict>
    </dict>
  </array>
</dict>
</plist>
"#,
            exe = exe_str.replace('"', "\\\"")
        );
        fs::write(contents.join("document.wflow"), wflow)?;

        // Refresh Services menu cache when available.
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs")
            .arg("-flush")
            .status();
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let home = dirs_home()?;
        let workflow = home.join("Library/Services/Open with sgg.workflow");
        if workflow.exists() {
            fs::remove_dir_all(workflow)?;
        }
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs")
            .arg("-flush")
            .status();
        Ok(())
    }

    fn dirs_home() -> Result<PathBuf> {
        env::var_os("HOME")
            .map(PathBuf::from)
            .context("HOME is not set")
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod linux {
    use super::*;

    pub fn install(exe: &Path) -> Result<()> {
        let home = env::var_os("HOME").map(PathBuf::from).context("HOME is not set")?;
        let exe = exe
            .canonicalize()
            .with_context(|| format!("canonicalize {}", exe.display()))?;
        let exe_str = exe.to_string_lossy();

        // Nautilus scripts (Files)
        let nautilus = home.join(".local/share/nautilus/scripts");
        fs::create_dir_all(&nautilus)?;
        let script = nautilus.join("Open with sgg");
        let body = format!(
            "#!/bin/sh\nexec \"{exe}\" \"${{1:-$PWD}}\"\n",
            exe = exe_str
        );
        fs::write(&script, body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&script)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script, perms)?;
        }

        // Generic desktop action for directories
        let apps = home.join(".local/share/applications");
        fs::create_dir_all(&apps)?;
        let desktop = apps.join("sgg-open-folder.desktop");
        let desktop_body = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Open with sgg\n\
             Exec=\"{exe}\" %f\n\
             MimeType=inode/directory;\n\
             NoDisplay=false\n\
             Terminal=false\n\
             Categories=Development;\n",
            exe = exe_str
        );
        fs::write(desktop, desktop_body)?;

        // Nemo / Caja actions when present
        let actions = home.join(".local/share/file-manager/actions");
        fs::create_dir_all(&actions)?;
        let action = actions.join("sgg.desktop");
        let action_body = format!(
            "[Desktop Entry]\n\
             Type=Action\n\
             Name=Open with sgg\n\
             Profiles=profile-zero;\n\
             \n\
             [X-Action-Profile profile-zero]\n\
             MimeTypes=inode/directory;\n\
             Exec=\"{exe}\" %f\n\
             Name=Open with sgg\n",
            exe = exe_str
        );
        fs::write(action, action_body)?;
        Ok(())
    }

    pub fn uninstall() -> Result<()> {
        let home = env::var_os("HOME").map(PathBuf::from).context("HOME is not set")?;
        let paths = [
            home.join(".local/share/nautilus/scripts/Open with sgg"),
            home.join(".local/share/applications/sgg-open-folder.desktop"),
            home.join(".local/share/file-manager/actions/sgg.desktop"),
        ];
        for path in paths {
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}
