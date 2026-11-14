use std::path::PathBuf;

use thiserror::Error;

use crate::shell::GitCliError;

#[derive(Debug, Error)]
pub enum RepoError {
    #[error("no .git found from {0}")]
    NotARepo(PathBuf),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Cli(#[from] GitCliError),
    #[error("{0}")]
    Message(String),
}