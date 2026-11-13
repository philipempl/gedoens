use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("That is not a valid public key. It has to start with age1.")]
    InvalidRecipient,
    #[error("The name “{0}” contains characters SOPS does not understand in names.")]
    UnsupportedKey(String),
    #[error("No decision yet on which value applies to “{0}”.")]
    MissingChoice(String),
    #[error("An area needs at least one recipient.")]
    EmptyArea,
}
