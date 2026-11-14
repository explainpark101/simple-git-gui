use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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

    let document = r#"<?xml version="1.0" encoding="UTF-8"?>
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
"#;
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