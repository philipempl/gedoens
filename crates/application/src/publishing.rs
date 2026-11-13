//! Committing secret changes and opening a merge request. Only secret files take part.

use std::path::Path;

use crate::ports::{FileChange, MergeRequest, PublishRequest};
use crate::views::{to_steps, ChangedFile, ChangesView, KeyChange, PublishInput, PublishedView};
use crate::{AppError, Gedoens, Result};

impl Gedoens {
    /// The secret files that differ from the last commit, with the values that changed (by path).
    pub fn changes(&self, root: &Path) -> Result<ChangesView> {
        let snapshot = self.snapshot(root)?;
        let branch = self.vcs.branch(root);
        if branch.is_none() {
            return Ok(ChangesView { is_repository: false, branch, target: None, ahead: 0, files: Vec::new(), others: 0 });
        }
        let target = self.vcs.target_branch(root);
        let ahead = target.as_deref().map(|t| self.vcs.ahead_of(root, t)).unwrap_or(0);
        let policy = self.policy_relative(root);

        let mut files = Vec::new();
        let mut others = 0;
        for (path, change) in self.vcs.changes(root) {
            let found = snapshot.files.iter().find(|f| f.relative == path);
            let is_policy = policy.as_deref() == Some(path.as_str());
            // A deleted secret file is no longer found by the scan; its committed text tells.
            let was_secret = change == FileChange::Deleted
                && self.vcs.committed(root, &path).is_some_and(|text| !self.workspace.recipients_in(&text).is_empty());
            if found.is_none() && !is_policy && !was_secret {
                others += 1;
                continue;
            }
            let status = match change {
                FileChange::Added => "added",
                FileChange::Modified => "modified",
                FileChange::Deleted => "deleted",
            };
            let mut view = ChangedFile { path: path.clone(), status, keys: Vec::new(), reencrypted: false, readable: true };
            if let Some(file) = found {
                view.readable = snapshot.readable(file);
                if view.readable {
                    let now = self.store.decrypt(&self.handle(root, file)).map(|n| n.leaves()).unwrap_or_default();
                    let before = self
                        .vcs
                        .committed(root, &path)
                        .and_then(|text| self.store.decrypt_text(&text, file.format).ok())
                        .map(|n| n.leaves())
                        .unwrap_or_default();
                    for (key, value) in &now {
                        match before.get(key) {
                            None => view.keys.push(KeyChange { path: to_steps(key), kind: "added" }),
                            Some(old) if old != value => view.keys.push(KeyChange { path: to_steps(key), kind: "changed" }),
                            _ => {}
                        }
                    }
                    for key in before.keys().filter(|k| !now.contains_key(*k)) {
                        view.keys.push(KeyChange { path: to_steps(key), kind: "removed" });
                    }
                    view.reencrypted = change == FileChange::Modified && view.keys.is_empty();
                }
            }
            files.push(view);
        }
        Ok(ChangesView { is_repository: true, branch, target, ahead, files, others })
    }

    /// Commits the chosen secret files, pushes, and asks GitLab for a merge request.
    /// Anything that is not a changed secret file is refused, so nothing else slips in.
    pub fn publish(&self, root: &Path, input: &PublishInput) -> Result<PublishedView> {
        let changes = self.changes(root)?;
        if !changes.is_repository {
            return Err(AppError::Unsupported("This folder is not a Git repository.".into()));
        }
        if input.files.is_empty() {
            return Err(AppError::Unsupported("Pick at least one file.".into()));
        }
        if let Some(stray) = input.files.iter().find(|f| !changes.files.iter().any(|c| &c.path == *f)) {
            return Err(AppError::Unsupported(format!("{stray} is not a changed secret file.")));
        }
        let message = input.message.trim();
        if message.is_empty() {
            return Err(AppError::Unsupported("Write a commit message.".into()));
        }
        let branch = input.branch.trim();
        if branch.is_empty() {
            return Err(AppError::Unsupported("Name the branch.".into()));
        }
        let merge_request = if input.merge_request {
            let target = changes.target.clone().ok_or_else(|| {
                AppError::Unsupported("The remote has no default branch, so there is nothing to open a merge request into.".into())
            })?;
            if target == branch {
                return Err(AppError::Unsupported(format!("A merge request from {branch} into itself makes no sense. Use a new branch.")));
            }
            Some(MergeRequest { target, title: message.lines().next().unwrap_or(message).to_owned(), description: describe(&changes, &input.files) })
        } else {
            None
        };
        let published = self.vcs.publish(
            root,
            &PublishRequest {
                branch: branch.to_owned(),
                new_branch: input.new_branch,
                paths: input.files.clone(),
                message: message.to_owned(),
                merge_request,
            },
        )?;
        Ok(PublishedView {
            branch: published.branch,
            commit: published.commit,
            merge_request_url: published.merge_request_url,
            note: published.note,
        })
    }

    fn policy_relative(&self, root: &Path) -> Option<String> {
        let path = self.workspace.policy_path(root)?;
        path.strip_prefix(root).ok().map(|p| p.to_string_lossy().replace('\\', "/"))
    }
}

/// The merge request text: which files and which values, never the values themselves.
fn describe(changes: &ChangesView, files: &[String]) -> String {
    let mut out = String::from("Secret changes made with gedoens.\n");
    for file in changes.files.iter().filter(|f| files.contains(&f.path)) {
        out.push_str(&format!("\n**{}** ({})\n", file.path, file.status));
        if file.reencrypted {
            out.push_str("- re-encrypted (recipients or data key)\n");
        }
        for key in &file.keys {
            let sign = match key.kind {
                "added" => "+",
                "removed" => "−",
                _ => "~",
            };
            out.push_str(&format!("- {sign} `{}`\n", dotted(&key.path)));
        }
    }
    out.push_str("\nValues are encrypted with SOPS and not shown here.\n");
    out
}

fn dotted(path: &[crate::views::Step]) -> String {
    path.iter()
        .enumerate()
        .map(|(n, step)| match step {
            crate::views::Step::Key(k) if n == 0 => k.clone(),
            crate::views::Step::Key(k) => format!(".{k}"),
            crate::views::Step::Index(i) => format!("[{i}]"),
        })
        .collect()
}
