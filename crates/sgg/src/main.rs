// GUI launches must never allocate a console (Explorer / "Open with sgg" /
// `sgg <path>`). CLI subcommands re-attach the parent terminal for stdout.
#![windows_subsystem = "windows"]

mod app;
mod cli;
mod shell;

use clap::Parser;

use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::InstallShell) => {
            attach_parent_console_for_cli();
            if let Err(err) = shell::install() {
                eprintln!("install-shell failed: {err:#}");
                std::process::exit(1);
            }
            println!("Shell integration installed (PATH + file-manager).");
        }
        Some(Commands::UninstallShell) => {
            attach_parent_console_for_cli();
            if let Err(err) = shell::uninstall() {
                eprintln!("uninstall-shell failed: {err:#}");
                std::process::exit(1);
            }
            println!("Shell integration removed (PATH + file-manager).");
        }
        Some(Commands::CheckUpdate) => {
            attach_parent_console_for_cli();
            match app::update::fetch_latest() {
                Ok(info) => {
                    let current = app::update::current_version();
                    println!("current: {current}");
                    println!("latest:  {}", info.version);
                    println!("tag:     {}", info.tag);
                    if let Some(notes) = info.notes.as_deref() {
                        println!("notes:   {notes}");
                    }
                    if let Some(at) = info.published_at.as_deref() {
                        println!("published_at: {at}");
                    }
                    if app::update::is_newer(&info.version, current) {
                        if let Some(asset) = info.asset_for_this_platform() {
                            println!("update:  available");
                            println!("asset:   {} ({})", asset.name, asset.url);
                        } else {
                            println!("update:  available (no asset for this platform)");
                        }
                        std::process::exit(2);
                    } else {
                        println!("update:  up-to-date");
                    }
                }
                Err(err) => {
                    eprintln!("check-update failed: {err:#}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Update) => {
            attach_parent_console_for_cli();
            match app::update::fetch_latest() {
                Ok(info) => {
                    let current = app::update::current_version();
                    if !app::update::is_newer(&info.version, current) {
                        println!("Already on latest version ({current}).");
                        return;
                    }
                    println!("Updating {current} → {} …", info.version);
                    match app::update::download_and_open_latest(&info) {
                        Ok(path) => {
                            println!("Downloaded and opened: {}", path.display());
                            println!("Finish installing, then restart sgg.");
                        }
                        Err(err) => {
                            eprintln!("update failed: {err:#}");
                            std::process::exit(1);
                        }
                    }
                }
                Err(err) => {
                    eprintln!("update failed: {err:#}");
                    std::process::exit(1);
                }
            }
        }
        None => {
            // No PATH → empty start (pick a folder in-app). `sgg <path>` / shell
            // "Open here" still open that location.
            app::run(cli.path);
        }
    }
}

/// Re-attach the parent terminal so `install-shell` / `uninstall-shell` can print.
fn attach_parent_console_for_cli() {
    #[cfg(windows)]
    {
        // ATTACH_PARENT_PROCESS = (DWORD)-1
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(dw_process_id: u32) -> i32;
        }
        unsafe {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}