//! Values that appear in more than one file, such as one password for preview and production.

use std::collections::BTreeMap;
use std::path::Path;

use gedoens_domain::{compare, shared_values, Cell, Scalar, SecretPath};

use crate::views::{to_steps, CompareCell, CompareRowView, CompareView, FolderComparison, Place, SharedView};
use crate::{Gedoens, Result};

type Leaves = BTreeMap<SecretPath, Scalar>;
/// The files that could be read, and the ones left out.
type ReadResult = (Vec<(String, Leaves)>, Vec<String>);

impl Gedoens {
    /// Decrypts every file this user can read right now. Returns them and the ones left out.
    fn readable_leaves(&self, root: &Path) -> Result<ReadResult> {
        let snapshot = self.snapshot(root)?;
        let mut read = Vec::new();
        let mut skipped = Vec::new();
        for file in &snapshot.files {
            let usable = snapshot.readable(file) && !snapshot.conflicted.contains(&file.relative);
            match usable.then(|| self.store.decrypt(&self.handle(root, file))) {
                Some(Ok(tree)) => read.push((file.relative.clone(), tree.leaves())),
                _ => skipped.push(file.relative.clone()),
            }
        }
        Ok((read, skipped))
    }

    /// The secrets of each folder side by side, with equal values marked. No values leave.
    pub fn compare(&self, root: &Path) -> Result<CompareView> {
        let (read, skipped) = self.readable_leaves(root)?;
        let mut by_folder: BTreeMap<String, Vec<(String, Leaves)>> = BTreeMap::new();
        for (file, leaves) in read {
            let folder = file.rsplit_once('/').map(|(f, _)| f.to_owned()).unwrap_or_default();
            by_folder.entry(folder).or_default().push((file, leaves));
        }
        let folders = by_folder
            .into_iter()
            .map(|(folder, files)| {
                let comparison = compare(&files);
                FolderComparison {
                    folder,
                    files: comparison.files,
                    rows: comparison
                        .rows
                        .iter()
                        .map(|row| CompareRowView {
                            path: to_steps(&row.path),
                            cells: row
                                .cells
                                .iter()
                                .map(|cell| match cell {
                                    Cell::Missing => CompareCell { present: false, same: None },
                                    Cell::Unique => CompareCell { present: true, same: None },
                                    Cell::Same(n) => CompareCell { present: true, same: Some(*n) },
                                })
                                .collect(),
                        })
                        .collect(),
                }
            })
            .filter(|folder| !folder.rows.is_empty())
            .collect();
        Ok(CompareView { folders, skipped })
    }

    /// Compares every file this user can read. Only the places come back, never a value.
    /// Files that cannot be decrypted right now (conflicts, missing access) are skipped.
    pub fn shared_values(&self, root: &Path) -> Result<Vec<SharedView>> {
        let (decrypted, _) = self.readable_leaves(root)?;
        Ok(shared_values(&decrypted)
            .into_iter()
            .map(|shared| SharedView {
                places: shared
                    .places
                    .iter()
                    .map(|(file, path)| Place { file: file.clone(), path: to_steps(path) })
                    .collect(),
            })
            .collect())
    }
}
