//! Anwendungsfälle von gedoens.
//!
//! Diese Schicht sagt, was die App kann: Dateien zeigen, Werte ändern, Zugriff
//! vergeben, Konflikte zusammenführen. Wie das technisch geschieht, steht hinter
//! den Schnittstellen in [`ports`]. Die Adapter dafür liegen in der Infrastruktur.

mod access;
mod error;
mod files;
mod merging;
mod publishing;
pub mod ports;
mod reuse;
mod service;
pub mod views;

pub use error::{AppError, Result};
pub use service::Gedoens;
