use gedoens_domain::DomainError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

/// Jeder Fehler sagt in einem Satz, was los ist und was zu tun ist.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Your key is not in this file. Ask someone with access to add you.")]
    NoAccess,
    #[error("This folder has no .sops.yaml. Access can only be managed once it has one.")]
    NoPolicy,
    #[error("{0} was not found.")]
    NotFound(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{tool} says: {message}")]
    Tool { tool: &'static str, message: String },
    #[error("{tool} is not installed or could not be found.")]
    ToolMissing { tool: &'static str },
    #[error("The file could not be read or written: {0}")]
    Io(String),
    #[error(transparent)]
    Domain(#[from] DomainError),
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Io(error.to_string())
    }
}
