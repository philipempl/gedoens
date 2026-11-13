use gedoens_domain::DomainError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AppError>;

/// Jeder Fehler sagt in einem Satz, was los ist und was zu tun ist.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Dein Key steht nicht in dieser Datei. Bitte jemanden mit Zugriff, dich einzutragen.")]
    NoAccess,
    #[error("In diesem Ordner gibt es keine .sops.yaml. Zugriff lässt sich erst mit ihr verwalten.")]
    NoPolicy,
    #[error("{0} wurde nicht gefunden.")]
    NotFound(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{tool} meldet: {message}")]
    Tool { tool: &'static str, message: String },
    #[error("{tool} ist nicht installiert oder wurde nicht gefunden.")]
    ToolMissing { tool: &'static str },
    #[error("Die Datei ließ sich nicht lesen oder schreiben: {0}")]
    Io(String),
    #[error(transparent)]
    Domain(#[from] DomainError),
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Io(error.to_string())
    }
}
