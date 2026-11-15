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
            println!("Shell integration installed.");
        }
        Some(Commands::UninstallShell) => {
            attach_parent_console_for_cli();
            if let Err(err) = shell::uninstall() {
                eprintln!("uninstall-shell failed: {err:#}");
                std::process::exit(1);
            }
            println!("Shell integration removed.");
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