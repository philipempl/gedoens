use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("Das ist kein gültiger Public Key. Er muss mit age1 beginnen.")]
    InvalidRecipient,
    #[error("Der Name „{0}“ enthält Zeichen, die SOPS in Namen nicht versteht.")]
    UnsupportedKey(String),
    #[error("Für „{0}“ fehlt die Entscheidung, welcher Wert gilt.")]
    MissingChoice(String),
    #[error("Ein Bereich braucht mindestens einen Empfänger.")]
    EmptyArea,
}
