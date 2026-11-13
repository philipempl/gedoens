//! Wer darf was lesen.

use std::path::Path;

use gedoens_domain::{AccessPolicy, AreaLabel, Recipient};

use crate::service::Snapshot;
use crate::views::{AccessView, AreaView, PersonAccess};
use crate::{AppError, Gedoens, Result};

impl Snapshot {
    fn editable(&self, policy: &AccessPolicy, area: usize) -> bool {
        policy
            .areas
            .get(area)
            .is_some_and(|a| a.editable && a.recipients.iter().any(|r| self.own.contains(r)))
    }

    fn files_in(&self, policy: &AccessPolicy, area: usize) -> Vec<&crate::ports::FoundFile> {
        self.files
            .iter()
            .filter(|f| policy.area_for(&f.relative).is_some_and(|a| a.index == area))
            .collect()
    }
}

impl Gedoens {
    pub fn access(&self, root: &Path) -> Result<AccessView> {
        let snapshot = self.snapshot(root)?;
        let Some(policy) = &snapshot.policy else {
            return Ok(AccessView { has_policy: false, areas: Vec::new(), people: Vec::new() });
        };
        let labels: Vec<AreaLabel> = policy.areas.iter().map(|area| area.label()).collect();
        let areas = policy
            .areas
            .iter()
            .zip(&labels)
            .enumerate()
            .map(|(position, (area, label))| {
                // A catch-all rule after more specific ones only gets what they leave over.
                let after_siblings = labels[..position].iter().any(|earlier| earlier.folder == label.folder);
                let name = match (&label.file, after_siblings) {
                    (Some(file), _) => file.clone(),
                    (None, true) => "other files".to_owned(),
                    (None, false) if label.folder.is_none() => "everything".to_owned(),
                    (None, false) => "all files".to_owned(),
                };
                AreaView {
                    index: area.index,
                    folder: label.folder.clone(),
                    name,
                    pattern: area.pattern.clone(),
                    files: snapshot.files_in(policy, area.index).len(),
                    editable: snapshot.editable(policy, area.index),
                }
            })
            .collect();

        let mut everyone = policy.recipients();
        for recipient in snapshot.files.iter().flat_map(|f| &f.recipients) {
            if !everyone.contains(recipient) {
                everyone.push(recipient.clone());
            }
        }
        let mut people: Vec<PersonAccess> = everyone
            .iter()
            .map(|recipient| PersonAccess {
                person: snapshot.person(recipient),
                areas: policy.areas.iter().map(|a| a.recipients.contains(recipient)).collect(),
            })
            .collect();
        // Erst das, was etwas von einem will, dann man selbst, dann der Rest.
        people.sort_by_key(|p| (p.person.name.is_some() || p.person.me, !p.person.me));
        Ok(AccessView { has_policy: true, areas, people })
    }

    /// Gibt Zugriff auf einen Bereich und zieht alle seine Dateien nach.
    /// Gibt zurück, wie viele Dateien angeglichen wurden.
    pub fn grant(&self, root: &Path, recipient: &str, area: usize, name: Option<&str>) -> Result<usize> {
        let recipient = Recipient::parse(recipient)?;
        let (path, snapshot) = self.editable_area(root, area)?;
        self.policies.add_recipient(&path, area, &recipient, name)?;
        let policy = self.policies.load(&path)?;
        let files = snapshot.files_in(&policy, area);
        let mut synced = 0;
        for file in files.iter().filter(|f| snapshot.readable(f)) {
            self.store.sync_recipients(&self.handle(root, file))?;
            synced += 1;
        }
        Ok(synced)
    }

    /// Nimmt den Zugriff auf einen Bereich. Erst angleichen, dann rotieren:
    /// in dieser Reihenfolge, wie die SOPS-Dokumentation es vorgibt.
    pub fn revoke(&self, root: &Path, recipient: &str, area: usize) -> Result<usize> {
        let recipient = Recipient::parse(recipient)?;
        let (path, snapshot) = self.editable_area(root, area)?;
        if snapshot.own.contains(&recipient) {
            return Err(AppError::Unsupported("You cannot remove yourself.".into()));
        }
        self.policies.remove_recipient(&path, area, &recipient)?;
        let policy = self.policies.load(&path)?;
        let files = snapshot.files_in(&policy, area);
        let mut rotated = 0;
        for file in files.iter().filter(|f| snapshot.readable(f)) {
            let handle = self.handle(root, file);
            self.store.sync_recipients(&handle)?;
            self.store.rotate(&handle)?;
            rotated += 1;
        }
        Ok(rotated)
    }

    /// Gibt einem Key einen Namen. Er steht danach als Kommentar in der `.sops.yaml`.
    pub fn name_key(&self, root: &Path, recipient: &str, name: &str) -> Result<()> {
        let recipient = Recipient::parse(recipient)?;
        let path = self.workspace.policy_path(root).ok_or(AppError::NoPolicy)?;
        self.policies.rename(&path, &recipient, name)
    }

    fn editable_area(&self, root: &Path, area: usize) -> Result<(std::path::PathBuf, Snapshot)> {
        let path = self.workspace.policy_path(root).ok_or(AppError::NoPolicy)?;
        let snapshot = self.snapshot(root)?;
        let policy = snapshot.policy.as_ref().ok_or(AppError::NoPolicy)?;
        if !snapshot.editable(policy, area) {
            return Err(AppError::Unsupported(
                "You cannot change this area. Either you have no access to it, or the rule uses key groups.".into(),
            ));
        }
        Ok((path, snapshot))
    }
}
