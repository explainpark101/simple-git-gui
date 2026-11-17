use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const PATH_MARKER_BEGIN: &str = "# >>> sgg PATH >>>";
const PATH_MARKER_END: &str = "# <<< sgg PATH <<<";
const LOCAL_BIN_REL: &str = ".local/bin";

pub fn install(exe: &Path) -> Result<()> {
    install_cli_path(exe)?;
    install_finder_service(exe)?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    uninstall_cli_path()?;
    uninstall_finder_service()?;
    Ok(())
}

/// Idempotent: put `sgg` on PATH when running from a `.app` bundle (DMG install).
/// No-op for plain `cargo run` / unpackaged binaries.
pub fn ensure_cli_path_from_bundle() -> Result<()> {
    let exe = env::current_exe().context("resolve current executable path")?;
    let exe = exe
        .canonicalize()
        .with_context(|| format!("canonicalize {}", exe.display()))?;
    if !is_bundled_app_exe(&exe) {
        return Ok(());
    }
    install_cli_path(&exe)?;
    Ok(())
}

fn install_cli_path(exe: &Path) -> Result<()> {
    let exe = exe
        .canonicalize()
        .with_context(|| format!("canonicalize {}", exe.display()))?;
    let link = cli_link_path()?;
    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }

    match link.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() || meta.is_file() => {
            fs::remove_file(&link)
                .with_context(|| format!("remove existing {}", link.display()))?;
        }
        Ok(_) => bail!("{} exists and is not a file/symlink", link.display()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).with_context(|| format!("stat {}", link.display())),
    }

    symlink(&exe, &link).with_context(|| {
        format!(
            "symlink {} -> {}",
            link.display(),
            exe.display()
        )
    })?;

    if let Some(dir) = link.parent() {
        ensure_bin_dir_on_path(dir)?;
    }
    Ok(())
}

fn uninstall_cli_path() -> Result<()> {
    for link in candidate_cli_links() {
        if link.symlink_metadata().is_ok() {
            let _ = fs::remove_file(&link);
        }
    }
    remove_path_snippet_from_shell_rc()?;
    Ok(())
}

fn cli_link_path() -> Result<PathBuf> {
    // Prefer /usr/local/bin when writable (usually already on macOS PATH).
    let usr_local = PathBuf::from("/usr/local/bin/sgg");
    if can_write_link_in(Path::new("/usr/local/bin")) {
        return Ok(usr_local);
    }
    let home = dirs_home()?;
    Ok(home.join(LOCAL_BIN_REL).join("sgg"))
}

fn candidate_cli_links() -> Vec<PathBuf> {
    let mut links = vec![PathBuf::from("/usr/local/bin/sgg")];
    if let Ok(home) = dirs_home() {
        links.push(home.join(LOCAL_BIN_REL).join("sgg"));
    }
    links
}

fn can_write_link_in(dir: &Path) -> bool {
    if !dir.is_dir() {
        // Try creating /usr/local/bin when missing (common on fresh macOS).
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
    }
    let probe = dir.join(".sgg-write-test");
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn ensure_bin_dir_on_path(bin_dir: &Path) -> Result<()> {
    // /usr/local/bin is on the default macOS PATH — nothing to write.
    if bin_dir == Path::new("/usr/local/bin") {
        return Ok(());
    }

    let home = dirs_home()?;
    let local_bin = home.join(LOCAL_BIN_REL);
    if bin_dir != local_bin.as_path() {
        return Ok(());
    }

    let export_line = format!(
        "export PATH=\"{bin}:$PATH\"",
        bin = local_bin.display()
    );
    let snippet = format!(
        "{PATH_MARKER_BEGIN}\n{export_line}\n{PATH_MARKER_END}\n"
    );

    // Login shells: zsh (macOS default) via .zprofile, bash via .bash_profile.
    for name in [".zprofile", ".bash_profile"] {
        let rc = home.join(name);
        upsert_path_snippet(&rc, &snippet)?;
    }
    Ok(())
}

fn upsert_path_snippet(rc: &Path, snippet: &str) -> Result<()> {
    let existing = if rc.exists() {
        fs::read_to_string(rc).with_context(|| format!("read {}", rc.display()))?
    } else {
        String::new()
    };
    if existing.contains(PATH_MARKER_BEGIN) {
        return Ok(());
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc)
        .with_context(|| format!("open {}", rc.display()))?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(b"\n")?;
    file.write_all(snippet.as_bytes())?;
    Ok(())
}

fn remove_path_snippet_from_shell_rc() -> Result<()> {
    let home = dirs_home()?;
    for name in [".zprofile", ".bash_profile"] {
        let rc = home.join(name);
        if !rc.exists() {
            continue;
        }
        let existing = fs::read_to_string(&rc)
            .with_context(|| format!("read {}", rc.display()))?;
        if !existing.contains(PATH_MARKER_BEGIN) {
            continue;
        }
        let cleaned = strip_marked_block(&existing);
        fs::write(&rc, cleaned).with_context(|| format!("write {}", rc.display()))?;
    }
    Ok(())
}

fn strip_marked_block(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut skipping = false;
    for line in src.lines() {
        if line.trim() == PATH_MARKER_BEGIN {
            skipping = true;
            continue;
        }
        if line.trim() == PATH_MARKER_END {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn is_bundled_app_exe(exe: &Path) -> bool {
    // .../Something.app/Contents/MacOS/sgg
    let mut components = exe.components().rev();
    let file = components.next();
    let macos = components.next();
    let contents = components.next();
    let app = components.next();
    matches!(
        (file, macos, contents, app),
        (
            Some(std::path::Component::Normal(f)),
            Some(std::path::Component::Normal(m)),
            Some(std::path::Component::Normal(c)),
            Some(std::path::Component::Normal(a)),
        ) if f == "sgg"
            && m == "MacOS"
            && c == "Contents"
            && a.to_string_lossy().ends_with(".app")
    )
}

fn install_finder_service(exe: &Path) -> Result<()> {
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

fn uninstall_finder_service() -> Result<()> {
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
