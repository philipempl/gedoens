//! Dateien zeigen und Werte ändern.

use std::collections::BTreeMap;
use std::path::Path;

use gedoens_domain::{Drift, Scalar, SecretPath};

use crate::service::Snapshot;
use crate::views::{to_path, to_steps, DriftView, Entry, FileGroup, FileSummary, FileView, Issue, Overview, Step};
use crate::{AppError, Gedoens, Result};

fn split(relative: &str) -> (String, String) {
    match relative.rsplit_once('/') {
        Some((folder, name)) => (format!("{folder}/"), name.to_owned()),
        None => (String::new(), relative.to_owned()),
    }
}

impl Snapshot {
    pub(crate) fn drift_of(&self, relative: &str) -> Option<Drift> {
        let file = self.files.iter().find(|f| f.relative == relative)?;
        let area = self.policy.as_ref()?.area_for(relative)?;
        let drift = Drift::between(&file.recipients, &area.recipients);
        (!drift.is_none()).then_some(drift)
    }

    pub(crate) fn drift_view(&self, drift: &Drift) -> DriftView {
        DriftView {
            missing: drift.missing.iter().map(|r| self.person(r)).collect(),
            extra: drift.extra.iter().map(|r| self.person(r)).collect(),
        }
    }
}

impl Gedoens {
    /// Die Startseite: was offen ist und welche Dateien es gibt.
    pub fn overview(&self, root: &Path) -> Result<Overview> {
        let snapshot = self.snapshot(root)?;
        let mut issues = Vec::new();
        let mut groups: BTreeMap<String, Vec<FileSummary>> = BTreeMap::new();

        for file in &snapshot.files {
            let conflicted = snapshot.conflicted.contains(&file.relative);
            let readable = snapshot.readable(file);
            let drift = if conflicted { None } else { snapshot.drift_of(&file.relative) };
            if conflicted {
                issues.push(Issue::Conflict { file: file.relative.clone() });
            } else if let (Some(drift), true) = (&drift, readable) {
                issues.push(Issue::Drift { file: file.relative.clone(), drift: snapshot.drift_view(drift) });
            }
            let (folder, name) = split(&file.relative);
            groups.entry(folder).or_default().push(FileSummary {
                path: file.relative.clone(),
                name,
                readable,
                drifted: drift.is_some(),
                conflicted,
            });
        }

        let mut everyone = snapshot.policy.as_ref().map(|p| p.recipients()).unwrap_or_default();
        for recipient in snapshot.files.iter().flat_map(|f| &f.recipients) {
            if !everyone.contains(recipient) {
                everyone.push(recipient.clone());
            }
        }
        let mut unnamed = 0;
        for recipient in &everyone {
            let person = snapshot.person(recipient);
            if person.name.is_none() && !person.me {
                unnamed += 1;
                issues.push(Issue::Unnamed { person });
            }
        }

        let checks_total = snapshot.files.len() + everyone.len();
        Ok(Overview {
            checks_passed: checks_total.saturating_sub(issues.len()),
            checks_total,
            unnamed,
            issues,
            groups: groups.into_iter().map(|(folder, files)| FileGroup { folder, files }).collect(),
        })
    }

    /// Eine Datei mit ihrer Gliederung. Werte sind nicht dabei.
    pub fn file(&self, root: &Path, relative: &str) -> Result<FileView> {
        let snapshot = self.snapshot(root)?;
        let file = snapshot.file(relative)?;
        let readable = snapshot.readable(file);
        let conflicted = snapshot.conflicted.iter().any(|c| c == relative);
        let entries = if readable && !conflicted {
            self.store
                .decrypt(&self.handle(root, file))?
                .outline()
                .into_iter()
                .map(|item| Entry { path: to_steps(&item.path), label: item.label, depth: item.depth, group: item.is_group })
                .collect()
        } else {
            Vec::new()
        };
        Ok(FileView {
            path: relative.to_owned(),
            name: split(relative).1,
            readable,
            conflicted,
            area: snapshot.policy.as_ref().and_then(|p| p.area_for(relative)).map(|a| a.display_name()),
            readers: file.recipients.iter().map(|r| snapshot.person(r)).collect(),
            drift: snapshot.drift_of(relative).map(|d| snapshot.drift_view(&d)),
            entries,
        })
    }

    /// Ein einzelner Wert im Klartext. Der einzige Weg, auf dem einer das Backend verlässt.
    pub fn reveal(&self, root: &Path, relative: &str, path: &[Step]) -> Result<String> {
        Ok(self.current(root, relative, &to_path(path))?.map(|s| s.display()).unwrap_or_default())
    }

    /// Ändert einen Wert oder legt ihn an. Gibt den alten Wert zurück, damit
    /// die Oberfläche Rückgängig anbieten kann.
    pub fn set_value(&self, root: &Path, relative: &str, path: &[Step], input: &str) -> Result<Option<String>> {
        let path = to_path(path);
        let previous = self.current(root, relative, &path)?;
        let value = Scalar::retyped(previous.as_ref(), input);
        let snapshot = self.snapshot(root)?;
        self.store.set(&self.handle(root, snapshot.file(relative)?), &path, &value)?;
        Ok(previous.map(|s| s.display()))
    }

    /// Löscht einen Eintrag und gibt seinen Wert zurück, für Rückgängig.
    pub fn remove_value(&self, root: &Path, relative: &str, path: &[Step]) -> Result<Option<String>> {
        let path = to_path(path);
        let previous = self.current(root, relative, &path)?;
        let snapshot = self.snapshot(root)?;
        self.store.unset(&self.handle(root, snapshot.file(relative)?), &path)?;
        Ok(previous.map(|s| s.display()))
    }

    /// Gleicht eine Datei an ihren Bereich an.
    pub fn align(&self, root: &Path, relative: &str) -> Result<()> {
        let snapshot = self.snapshot(root)?;
        let file = snapshot.file(relative)?;
        if !snapshot.readable(file) {
            return Err(AppError::NoAccess);
        }
        self.store.sync_recipients(&self.handle(root, file))
    }

    fn current(&self, root: &Path, relative: &str, path: &SecretPath) -> Result<Option<Scalar>> {
        let snapshot = self.snapshot(root)?;
        let file = snapshot.file(relative)?;
        if !snapshot.readable(file) {
            return Err(AppError::NoAccess);
        }
        Ok(self.store.decrypt(&self.handle(root, file))?.scalar(path).cloned())
    }
}
