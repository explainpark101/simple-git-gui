use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "sgg", version, about = "simple-git-gui — lightweight Git GUI")]
pub struct Cli {
    /// Folder or file used to locate the nearest git repository.
    /// Omit to start with no folder open (choose one in the UI).
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Register "Open sgg here" in the OS file manager
    InstallShell,
    /// Remove file-manager integration
    UninstallShell,
    /// Fetch GitHub latest.json and print whether an update is available
    CheckUpdate,
    /// Download and open the latest installer for this platform (if newer)
    Update,
}