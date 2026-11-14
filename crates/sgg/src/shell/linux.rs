use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn install(exe: &Path) -> Result<()> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;
    let exe = exe
        .canonicalize()
        .with_context(|| format!("canonicalize {}", exe.display()))?;
    let exe_str = exe.to_string_lossy();

    let nautilus = home.join(".local/share/nautilus/scripts");
    fs::create_dir_all(&nautilus)?;
    let script = nautilus.join("Open with sgg");
    let body = format!(
        "#!/bin/sh\nexec \"{exe}\" \"${{1:-$PWD}}\"\n",
        exe = exe_str
    );
    fs::write(&script, body)?;
    let mut perms = fs::metadata(&script)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms)?;

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
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;
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