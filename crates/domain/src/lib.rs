//! Fachlogik von gedoens.
//!
//! Diese Schicht kennt weder Dateisystem noch SOPS noch Git. Sie beschreibt,
//! was ein Empfänger, ein Bereich, eine Abweichung und ein Zusammenführen sind,
//! und lässt sich deshalb ohne jedes Werkzeug testen.

mod access;
mod drift;
mod error;
mod file_format;
mod merge;
mod recipient;
mod reuse;
mod secret_path;
mod secret_tree;

pub use access::{AccessPolicy, Area, AreaLabel};
pub use drift::Drift;
pub use error::DomainError;
pub use file_format::FileFormat;
pub use merge::{merge, Change, Clash, MergePlan, Side};
pub use recipient::Recipient;
pub use reuse::{compare, is_secret_name, shared_values, Cell, CompareRow, Comparison, SharedValue, MIN_SECRET_LENGTH};
pub use secret_path::{Segment, SecretPath};
pub use secret_tree::{Node, OutlineItem, Scalar};
