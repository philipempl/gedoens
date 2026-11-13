use std::path::Path;
use std::sync::Arc;

use gedoens_domain::{AccessPolicy, Recipient};

use crate::ports::{FileHandle, FoundFile, KeyRing, PolicyRepository, SecretStore, VersionControl, Workspace};
use crate::views::{KeyStatus, Person};
use crate::{AppError, Result};

/// Der Einstieg in alle Anwendungsfälle. Hält keinen Zustand und keinen Klartext:
/// jede Anfrage liest frisch von der Platte.
#[derive(Clone)]
pub struct Gedoens {
    pub(crate) store: Arc<dyn SecretStore>,
    pub(crate) workspace: Arc<dyn Workspace>,
    pub(crate) policies: Arc<dyn PolicyRepository>,
    pub(crate) vcs: Arc<dyn VersionControl>,
    pub(crate) keys: Arc<dyn KeyRing>,
}

/// Alles, was die Anwendungsfälle über den geöffneten Ordner wissen müssen.
pub(crate) struct Snapshot {
    pub files: Vec<FoundFile>,
    pub policy: Option<AccessPolicy>,
    pub own: Vec<Recipient>,
    pub conflicted: Vec<String>,
}

impl Snapshot {
    pub fn readable(&self, file: &FoundFile) -> bool {
        file.recipients.iter().any(|r| self.own.contains(r))
    }

    pub fn person(&self, recipient: &Recipient) -> Person {
        let name = self.policy.as_ref().and_then(|p| p.name_of(recipient));
        let mut person = Person::new(recipient, name, self.own.contains(recipient));
        person.suggestion = self.policy.as_ref().and_then(|p| p.hint_for(recipient)).map(str::to_owned);
        person
    }

    pub fn file(&self, relative: &str) -> Result<&FoundFile> {
        self.files
            .iter()
            .find(|f| f.relative == relative)
            .ok_or_else(|| AppError::NotFound(relative.to_owned()))
    }
}

impl Gedoens {
    pub fn new(
        store: Arc<dyn SecretStore>,
        workspace: Arc<dyn Workspace>,
        policies: Arc<dyn PolicyRepository>,
        vcs: Arc<dyn VersionControl>,
        keys: Arc<dyn KeyRing>,
    ) -> Self {
        Self { store, workspace, policies, vcs, keys }
    }

    pub fn key_status(&self) -> KeyStatus {
        let locations = self.keys.locations();
        KeyStatus {
            public_keys: self.keys.own().iter().map(|r| r.as_str().to_owned()).collect(),
            found_in: locations.found_in,
            searched: locations.searched,
        }
    }

    /// Creates a key only when none exists anywhere. An existing key is never replaced.
    pub fn generate_key(&self) -> Result<KeyStatus> {
        if self.keys.own().is_empty() {
            self.keys.generate()?;
        }
        Ok(self.key_status())
    }

    /// Uses the keys in a folder the user picked and remembers it for next time.
    pub fn use_key_folder(&self, place: &Path) -> Result<KeyStatus> {
        self.keys.remember(place)?;
        Ok(self.key_status())
    }

    pub(crate) fn snapshot(&self, root: &Path) -> Result<Snapshot> {
        let policy = match self.workspace.policy_path(root) {
            Some(path) => Some(self.policies.load(&path)?),
            None => None,
        };
        Ok(Snapshot {
            files: self.workspace.scan(root)?,
            policy,
            own: self.keys.own(),
            conflicted: self.vcs.conflicted(root),
        })
    }

    pub(crate) fn handle(&self, root: &Path, file: &FoundFile) -> FileHandle {
        FileHandle {
            absolute: root.join(&file.relative),
            format: file.format,
            config: self.workspace.policy_path(root),
        }
    }
}
