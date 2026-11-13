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

/// A change to a file as Git sees it, relative to the opened folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChange {
    Added,
    Modified,
    Deleted,
}

/// What to publish: these files, in one commit, pushed, optionally with a merge request.
#[derive(Debug, Clone)]
pub struct PublishRequest {
    /// The branch to commit on. A new one is created from the current state.
    pub branch: String,
    pub new_branch: bool,
    /// Relative to the opened folder. Only these paths go into the commit.
    pub paths: Vec<String>,
    pub message: String,
    pub merge_request: Option<MergeRequest>,
}

#[derive(Debug, Clone)]
pub struct MergeRequest {
    pub target: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct Published {
    pub branch: String,
    pub commit: String,
    pub merge_request_url: Option<String>,
    /// Said to the user when something worked only in part, e.g. no merge request.
    pub note: Option<String>,
}

/// Git, as far as the app needs it.
pub trait VersionControl: Send + Sync {
    fn conflicted(&self, root: &Path) -> Vec<String>;
    /// Ein Stand der Datei im Konflikt: 1 gemeinsamer Ursprung, 2 unserer, 3 der fremde.
    fn stage(&self, root: &Path, relative: &str, number: u8) -> Option<String>;
    fn mark_resolved(&self, root: &Path, relative: &str) -> Result<()>;
    /// Everything that differs from the last commit, untracked files included.
    fn changes(&self, root: &Path) -> Vec<(String, FileChange)>;
    /// The file as it was in the last commit.
    fn committed(&self, root: &Path, relative: &str) -> Option<String>;
    fn branch(&self, root: &Path) -> Option<String>;
    /// The branch merge requests go into, usually `main`.
    fn target_branch(&self, root: &Path) -> Option<String>;
    /// Commits on the current branch that the target branch does not have yet.
    fn ahead_of(&self, root: &Path, target: &str) -> usize;
    fn publish(&self, root: &Path, request: &PublishRequest) -> Result<Published>;
}

/// Where the own keys were looked for and where they turned up.
#[derive(Debug, Clone, Default)]
pub struct KeyLocations {
    pub searched: Vec<String>,
    pub found_in: Vec<String>,
}

/// The own keys, in the places SOPS looks for them.
pub trait KeyRing: Send + Sync {
    fn own(&self) -> Vec<Recipient>;
    fn locations(&self) -> KeyLocations;
    /// Creates a key, but only if there is none yet. Never overwrites anything.
    fn generate(&self) -> Result<Recipient>;
    /// Remembers a folder (or file) with keys that the usual search does not find.
    /// Returns the key file that was found there.
    fn remember(&self, place: &Path) -> Result<PathBuf>;
}
