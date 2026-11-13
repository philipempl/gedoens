use std::path::{Path, PathBuf};

use gedoens_application::ports::{FileChange, PublishRequest, Published, VersionControl};
use gedoens_application::{AppError, Result};

use crate::{locate, process};

/// Das installierte Git. Fehlt es, gibt es eben keine Konflikte zu zeigen.
pub struct GitCli {
    program: Option<PathBuf>,
}

impl GitCli {
    pub fn locate() -> Self {
        Self { program: locate::program("git", "GEDOENS_GIT") }
    }

    fn run(&self, root: &Path, args: &[&str]) -> Option<std::process::Output> {
        let program = self.program.as_deref()?;
        let mut all = vec!["-C".to_owned(), root.to_string_lossy().into_owned()];
        all.extend(args.iter().map(|a| (*a).to_owned()));
        // Never wait for a password prompt nobody can see: fail and say so instead.
        process::run(program, all, None, &[("GIT_TERMINAL_PROMPT", "0".to_owned())]).ok()
    }

    /// Runs and returns stdout, or the error Git gave.
    fn must(&self, root: &Path, args: &[&str]) -> Result<String> {
        match self.run(root, args) {
            Some(output) if output.status.success() => Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned()),
            Some(output) => Err(AppError::Tool { tool: "git", message: process::summary(&output.stderr) }),
            None => Err(AppError::ToolMissing { tool: "git" }),
        }
    }

    fn text(&self, root: &Path, args: &[&str]) -> Option<String> {
        self.must(root, args).ok().filter(|s| !s.is_empty())
    }

    fn push(&self, root: &Path, branch: &str, options: &[String]) -> Option<std::process::Output> {
        let mut args: Vec<&str> = vec!["push", "--porcelain", "-u", "origin"];
        let refspec = format!("HEAD:refs/heads/{branch}");
        args.push(&refspec);
        for option in options {
            args.push("-o");
            args.push(option);
        }
        self.run(root, &args)
    }
}

impl VersionControl for GitCli {
    fn conflicted(&self, root: &Path) -> Vec<String> {
        match self.run(root, &["diff", "--name-only", "--diff-filter=U", "--relative"]) {
            Some(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).lines().map(str::to_owned).filter(|l| !l.is_empty()).collect()
            }
            _ => Vec::new(),
        }
    }

    fn stage(&self, root: &Path, relative: &str, number: u8) -> Option<String> {
        let output = self.run(root, &["show", &format!(":{number}:./{relative}")])?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn mark_resolved(&self, root: &Path, relative: &str) -> Result<()> {
        match self.run(root, &["add", "--", relative]) {
            Some(output) if output.status.success() => Ok(()),
            Some(output) => Err(AppError::Tool { tool: "git", message: process::summary(&output.stderr) }),
            None => Err(AppError::ToolMissing { tool: "git" }),
        }
    }

    fn changes(&self, root: &Path) -> Vec<(String, FileChange)> {
        let mut out = Vec::new();
        // Tracked files, staged or not, compared with the last commit; paths relative to the folder.
        if let Some(text) = self.text(root, &["diff", "HEAD", "--name-status", "--relative", "--no-renames", "-z"]) {
            let mut parts = text.split('\0').filter(|s| !s.is_empty());
            while let (Some(status), Some(path)) = (parts.next(), parts.next()) {
                let change = match status.chars().next() {
                    Some('A') => FileChange::Added,
                    Some('D') => FileChange::Deleted,
                    _ => FileChange::Modified,
                };
                out.push((path.to_owned(), change));
            }
        }
        if let Some(text) = self.text(root, &["ls-files", "--others", "--exclude-standard", "-z"]) {
            out.extend(text.split('\0').filter(|s| !s.is_empty()).map(|p| (p.to_owned(), FileChange::Added)));
        }
        out
    }

    fn committed(&self, root: &Path, relative: &str) -> Option<String> {
        let output = self.run(root, &["show", &format!("HEAD:./{relative}")])?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn branch(&self, root: &Path) -> Option<String> {
        self.text(root, &["rev-parse", "--is-inside-work-tree"])?;
        Some(self.text(root, &["branch", "--show-current"]).unwrap_or_else(|| "(detached)".to_owned()))
    }

    fn target_branch(&self, root: &Path) -> Option<String> {
        let full = self.text(root, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])?;
        Some(full.strip_prefix("origin/").unwrap_or(&full).to_owned())
    }

    fn ahead_of(&self, root: &Path, target: &str) -> usize {
        self.text(root, &["rev-list", "--count", &format!("origin/{target}..HEAD")])
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }

    fn publish(&self, root: &Path, request: &PublishRequest) -> Result<Published> {
        let branch = request.branch.as_str();
        self.must(root, &["check-ref-format", "--branch", branch])
            .map_err(|_| AppError::Unsupported(format!("“{branch}” is not a valid branch name.")))?;
        if request.new_branch {
            // Takes the uncommitted changes along; fails if the branch exists.
            self.must(root, &["switch", "-c", branch])?;
        } else if self.branch(root).as_deref() != Some(branch) {
            return Err(AppError::Unsupported(format!("You are not on {branch}.")));
        }

        let mut add = vec!["add", "-A", "--"];
        add.extend(request.paths.iter().map(String::as_str));
        self.must(root, &add)?;
        // With paths, `commit` takes exactly these and leaves anything else staged alone.
        let mut commit = vec!["commit", "-m", request.message.as_str(), "--"];
        commit.extend(request.paths.iter().map(String::as_str));
        self.must(root, &commit)?;
        let sha = self.text(root, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();

        let options: Vec<String> = match &request.merge_request {
            Some(mr) => vec![
                "merge_request.create".to_owned(),
                format!("merge_request.target={}", mr.target),
                format!("merge_request.title={}", mr.title),
                format!("merge_request.description={}", mr.description),
            ],
            None => Vec::new(),
        };
        let mut output = self.push(root, branch, &options).ok_or(AppError::ToolMissing { tool: "git" })?;
        let mut note = None;
        let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if !output.status.success() && !options.is_empty() && stderr.contains("push options") {
            // Not GitLab, or push options are switched off: push plainly.
            output = self.push(root, branch, &[]).ok_or(AppError::ToolMissing { tool: "git" })?;
            stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            note = Some("The remote does not take merge request options, so the branch was pushed without one.".to_owned());
        }
        if !output.status.success() {
            let hint = if stderr.contains("terminal prompts disabled") || stderr.contains("Authentication failed") {
                " Git needs credentials: push once from the terminal so they get stored, then try again."
            } else {
                ""
            };
            return Err(AppError::Tool {
                tool: "git",
                message: format!("committed as {sha} on {branch}, but the push failed: {}{hint}", process::summary(&output.stderr)),
            });
        }
        let merge_request_url = merge_request_link(&stderr);
        if request.merge_request.is_some() && merge_request_url.is_none() && note.is_none() {
            note = Some("Pushed. GitLab did not report a merge request; maybe one is already open for this branch.".to_owned());
        }
        Ok(Published { branch: branch.to_owned(), commit: sha, merge_request_url, note })
    }
}

/// GitLab answers a push with `remote:   https://…/-/merge_requests/12`.
fn merge_request_link(remote_output: &str) -> Option<String> {
    remote_output
        .split_whitespace()
        // `/merge_requests/new?…` is only the offer to create one, not a merge request.
        .find(|word| word.starts_with("http") && word.contains("/merge_requests/") && !word.contains("/merge_requests/new"))
        .map(|word| word.trim_end_matches(|c: char| !c.is_ascii_alphanumeric()).to_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use gedoens_application::ports::MergeRequest;

    use super::*;

    #[test]
    fn reads_the_merge_request_link_from_gitlab() {
        let said = "remote: \nremote: View merge request for secrets/x:\nremote:   https://git.example.com/team/repo/-/merge_requests/42   \nremote: \n";
        assert_eq!(merge_request_link(said).as_deref(), Some("https://git.example.com/team/repo/-/merge_requests/42"));
        let offer = "remote: To create a merge request for x, visit:\nremote:   https://git.example.com/team/repo/-/merge_requests/new?merge_request%5Bsource_branch%5D=x\n";
        assert_eq!(merge_request_link(offer), None);
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    #[test]
    fn commits_only_the_chosen_files_and_pushes_a_new_branch() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = tmp.path().join("remote.git");
        let work = tmp.path().join("work");
        fs::create_dir_all(&work).unwrap();
        git(tmp.path(), &["init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
        git(&work, &["init", "-q", "-b", "main"]);
        git(&work, &["config", "user.email", "test@example.com"]);
        git(&work, &["config", "user.name", "Test"]);
        git(&work, &["remote", "add", "origin", remote.to_str().unwrap()]);
        fs::create_dir_all(work.join("env")).unwrap();
        fs::write(work.join("env/secrets.yaml"), "a: 1\n").unwrap();
        fs::write(work.join("README.md"), "hi\n").unwrap();
        git(&work, &["add", "."]);
        git(&work, &["commit", "-q", "-m", "start"]);
        git(&work, &["push", "-q", "-u", "origin", "main"]);
        git(&work, &["remote", "set-head", "origin", "main"]);

        fs::write(work.join("env/secrets.yaml"), "a: 2\n").unwrap();
        fs::write(work.join("env/new.yaml"), "b: 1\n").unwrap();
        fs::write(work.join("README.md"), "changed, not ours\n").unwrap();

        let cli = GitCli { program: Some(PathBuf::from("git")) };
        let mut changes = cli.changes(&work);
        changes.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            changes,
            vec![
                ("README.md".to_owned(), FileChange::Modified),
                ("env/new.yaml".to_owned(), FileChange::Added),
                ("env/secrets.yaml".to_owned(), FileChange::Modified),
            ]
        );
        assert_eq!(cli.target_branch(&work).as_deref(), Some("main"));
        assert_eq!(cli.committed(&work, "env/secrets.yaml").as_deref(), Some("a: 1\n"));

        let published = cli
            .publish(
                &work,
                &PublishRequest {
                    branch: "secrets/rotate-db".into(),
                    new_branch: true,
                    paths: vec!["env/secrets.yaml".into(), "env/new.yaml".into()],
                    message: "Rotate the database password".into(),
                    // A plain bare repository refuses push options: the fallback must kick in.
                    merge_request: Some(MergeRequest { target: "main".into(), title: "t".into(), description: "d".into() }),
                },
            )
            .unwrap();
        assert_eq!(published.branch, "secrets/rotate-db");
        assert!(published.merge_request_url.is_none());
        assert!(published.note.is_some());
        assert_eq!(git(&work, &["branch", "--show-current"]), "secrets/rotate-db");
        assert_eq!(git(&work, &["show", "--name-only", "--format=", "HEAD"]), "env/new.yaml\nenv/secrets.yaml");
        assert_eq!(git(&work, &["status", "--porcelain"]), "M README.md", "the other change stays uncommitted");
        assert_eq!(git(&remote, &["log", "-1", "--format=%s", "secrets/rotate-db"]), "Rotate the database password");
        assert_eq!(cli.ahead_of(&work, "main"), 1);
    }
}
