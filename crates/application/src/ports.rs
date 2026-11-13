//! Schnittstellen, über die die Anwendungsfälle die Außenwelt erreichen.
//! Jede hat genau einen Adapter in der Infrastruktur und lässt sich im Test ersetzen.

use std::path::{Path, PathBuf};

use gedoens_domain::{AccessPolicy, FileFormat, Node, Recipient, Scalar, SecretPath};

use crate::Result;

/// Eine verschlüsselte Datei, so wie ein Werkzeug sie ansprechen muss.
#[derive(Debug, Clone)]
pub struct FileHandle {
    pub absolute: PathBuf,
    pub format: FileFormat,
    /// Die zuständige `.sops.yaml`. SOPS sucht sie sonst vom Arbeitsverzeichnis aus.
    pub config: Option<PathBuf>,
}

/// Eine gefundene SOPS-Datei mit den Empfängern aus ihren Metadaten.
#[derive(Debug, Clone)]
pub struct FoundFile {
    pub relative: String,
    pub format: FileFormat,
    pub recipients: Vec<Recipient>,
}

/// Ver- und Entschlüsseln. Der Adapter ruft das offizielle `sops`-Programm auf.
pub trait SecretStore: Send + Sync {
    fn decrypt(&self, file: &FileHandle) -> Result<Node>;
    /// Entschlüsselt Text, der nicht auf der Platte liegt, etwa einen Stand aus Git.
    fn decrypt_text(&self, encrypted: &str, format: FileFormat) -> Result<Node>;
    fn set(&self, file: &FileHandle, path: &SecretPath, value: &Scalar) -> Result<()>;
    fn unset(&self, file: &FileHandle, path: &SecretPath) -> Result<()>;
    /// Gleicht die Empfänger der Datei an ihre Regel an.
    fn sync_recipients(&self, file: &FileHandle) -> Result<()>;
    /// Erneuert den Datenschlüssel der Datei.
    fn rotate(&self, file: &FileHandle) -> Result<()>;
}

/// Der geöffnete Ordner.
pub trait Workspace: Send + Sync {
    fn scan(&self, root: &Path) -> Result<Vec<FoundFile>>;
    fn recipients_in(&self, encrypted: &str) -> Vec<Recipient>;
    fn write(&self, root: &Path, relative: &str, content: &str) -> Result<()>;
    fn policy_path(&self, root: &Path) -> Option<PathBuf>;
}

/// Lesen und Ändern der `.sops.yaml`. Namen stehen dort als Kommentar am Key.
pub trait PolicyRepository: Send + Sync {
    fn load(&self, path: &Path) -> Result<AccessPolicy>;
    fn add_recipient(&self, path: &Path, area: usize, recipient: &Recipient, name: Option<&str>) -> Result<()>;
    fn remove_recipient(&self, path: &Path, area: usize, recipient: &Recipient) -> Result<()>;
    fn rename(&self, path: &Path, recipient: &Recipient, name: &str) -> Result<()>;
}

/// Git, soweit die App es braucht: Konflikte erkennen und als gelöst markieren.
pub trait VersionControl: Send + Sync {
    fn conflicted(&self, root: &Path) -> Vec<String>;
    /// Ein Stand der Datei im Konflikt: 1 gemeinsamer Ursprung, 2 unserer, 3 der fremde.
    fn stage(&self, root: &Path, relative: &str, number: u8) -> Option<String>;
    fn mark_resolved(&self, root: &Path, relative: &str) -> Result<()>;
}

/// Die eigenen Keys, am selben Ort, an dem auch SOPS sie sucht.
pub trait KeyRing: Send + Sync {
    fn own(&self) -> Vec<Recipient>;
    fn generate(&self) -> Result<Recipient>;
}
