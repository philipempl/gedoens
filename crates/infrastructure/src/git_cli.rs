use std::path::{Path, PathBuf};

use gedoens_application::ports::VersionControl;
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
        process::run(program, all, None).ok()
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
}
