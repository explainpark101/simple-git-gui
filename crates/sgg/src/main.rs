mod app;
mod shell;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "sgg", version, about = "simple-git-gui — lightweight Git GUI")]
struct Cli {
    /// Folder or file used to locate the nearest git repository
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Register "Open sgg here" in the OS file manager
    InstallShell,
    /// Remove file-manager integration
    UninstallShell,
}

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
            let start = cli
                .path
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            app::run(start);
        }
    }
}