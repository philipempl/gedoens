//! Konflikte nach einem Pull zusammenführen.

use std::path::Path;

use gedoens_domain::{merge, FileFormat, MergePlan, Side};

use crate::ports::FileHandle;
use crate::views::{to_path, to_steps, Choice, ClashView, MergePreview};
use crate::{AppError, Gedoens, Result};

struct Stages {
    format: FileFormat,
    base: Option<String>,
    ours: String,
    theirs: String,
    plan: MergePlan,
}

impl Gedoens {
    /// Zeigt, was sich von selbst zusammenführen lässt und wo jemand wählen muss.
    pub fn merge_preview(&self, root: &Path, relative: &str) -> Result<MergePreview> {
        let stages = self.stages(root, relative)?;
        Ok(MergePreview {
            automatic: stages.plan.theirs.len(),
            clashes: stages
                .plan
                .clashes
                .iter()
                .map(|clash| ClashView {
                    path: to_steps(&clash.path),
                    label: clash.path.label(),
                    ours: clash.ours.as_ref().map(|s| s.display()),
                    theirs: clash.theirs.as_ref().map(|s| s.display()),
                })
                .collect(),
        })
    }

    /// Führt die Datei zusammen und markiert sie in Git als gelöst.
    pub fn merge_apply(&self, root: &Path, relative: &str, choices: &[Choice]) -> Result<()> {
        let stages = self.stages(root, relative)?;

        // Hat die andere Seite die Empfänger geändert, ist ihr Stand die Basis.
        // Sonst ginge ein entzogener Zugriff beim Zusammenführen wieder verloren.
        let theirs_changed_readers = match &stages.base {
            Some(base) => self.workspace.recipients_in(&stages.theirs) != self.workspace.recipients_in(base),
            None => false,
        };
        let side = if theirs_changed_readers { Side::Theirs } else { Side::Ours };
        let choices: Vec<_> = choices
            .iter()
            .map(|c| (to_path(&c.path), if c.take_theirs { Side::Theirs } else { Side::Ours }))
            .collect();
        let changes = stages.plan.resolve(side, &choices)?;

        let content = if side == Side::Theirs { &stages.theirs } else { &stages.ours };
        self.workspace.write(root, relative, content)?;
        let handle = FileHandle {
            absolute: root.join(relative),
            format: stages.format,
            config: self.workspace.policy_path(root),
        };
        for change in &changes {
            match &change.value {
                Some(value) => self.store.set(&handle, &change.path, value)?,
                None => self.store.unset(&handle, &change.path)?,
            }
        }
        self.vcs.mark_resolved(root, relative)
    }

    fn stages(&self, root: &Path, relative: &str) -> Result<Stages> {
        let name = relative.rsplit('/').next().unwrap_or(relative);
        let format = FileFormat::from_file_name(name).ok_or_else(|| AppError::NotFound(relative.to_owned()))?;
        let missing = || AppError::Unsupported("Diese Datei steht in Git nicht im Konflikt.".into());
        let base = self.vcs.stage(root, relative, 1);
        let ours = self.vcs.stage(root, relative, 2).ok_or_else(missing)?;
        let theirs = self.vcs.stage(root, relative, 3).ok_or_else(missing)?;

        let leaves = |text: &str| -> Result<_> { Ok(self.store.decrypt_text(text, format)?.leaves()) };
        let base_leaves = match &base {
            Some(text) => leaves(text)?,
            None => Default::default(),
        };
        let plan = merge(&base_leaves, &leaves(&ours)?, &leaves(&theirs)?);
        Ok(Stages { format, base, ours, theirs, plan })
    }
}
