mod app;
mod cli;
mod shell;

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
            // No PATH → empty start (pick a folder in-app). `sgg <path>` / shell
            // "Open here" still open that location.
            app::run(cli.path);
        }
    }
}