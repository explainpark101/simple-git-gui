mod app;
mod cli;
mod shell;

use std::path::PathBuf;

use clap::Parser;

use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::InstallShell) => {
            if let Err(err) = shell::install() {
                eprintln!("install-shell failed: {err:#}");
                std::process::exit(1);
            }
            println!("Shell integration installed.");
        }
        Some(Commands::UninstallShell) => {
            if let Err(err) = shell::uninstall() {
                eprintln!("uninstall-shell failed: {err:#}");
                std::process::exit(1);
            }
            println!("Shell integration removed.");
        }
        None => {
            let start = cli.path.unwrap_or_else(|| {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
            });
            app::run(start);
        }
    }
}