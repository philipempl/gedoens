use std::ffi::OsString;
use std::path::{Path, PathBuf};

use gedoens_application::ports::{FileHandle, SecretStore};
use gedoens_application::{AppError, Result};
use gedoens_domain::{FileFormat, Node, Scalar, SecretPath};

use crate::{json_tree, locate, process};

/// Steuert das offizielle `sops`-Programm. gedoens baut SOPS nicht nach.
pub struct SopsCli {
    program: Option<PathBuf>,
}

impl SopsCli {
    pub fn locate() -> Self {
        Self { program: locate::program("sops", "GEDOENS_SOPS") }
    }

    pub fn at(program: PathBuf) -> Self {
        Self { program: Some(program) }
    }

    fn run(&self, config: Option<&Path>, args: &[&str], file: Option<&Path>, tail: &[&str], stdin: Option<&str>) -> Result<String> {
        let program = self.program.as_deref().ok_or(AppError::ToolMissing { tool: "sops" })?;
        let mut all: Vec<OsString> = Vec::new();
        if let Some(config) = config {
            // Ohne diese Angabe sucht SOPS die .sops.yaml vom Arbeitsverzeichnis aus,
            // nicht vom Ordner der Datei.
            all.push("--config".into());
            all.push(config.into());
        }
        all.extend(args.iter().map(OsString::from));
        if let Some(file) = file {
            all.push(file.into());
        }
        all.extend(tail.iter().map(OsString::from));

        let output = process::run(program, all, stdin).map_err(|_| AppError::ToolMissing { tool: "sops" })?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Failed to get the data key") {
            Err(AppError::NoAccess)
        } else {
            Err(AppError::Tool { tool: "sops", message: process::summary(&output.stderr) })
        }
    }

    fn parse(json: &str) -> Result<Node> {
        serde_json::from_str(json)
            .map(json_tree::to_node)
            .map_err(|e| AppError::Tool { tool: "sops", message: e.to_string() })
    }
}

impl SecretStore for SopsCli {
    fn decrypt(&self, file: &FileHandle) -> Result<Node> {
        let kind = file.format.sops_type();
        let json = self.run(
            file.config.as_deref(),
            &["decrypt", "--input-type", kind, "--output-type", "json"],
            Some(&file.absolute),
            &[],
            None,
        )?;
        Self::parse(&json)
    }

    fn decrypt_text(&self, encrypted: &str, format: FileFormat) -> Result<Node> {
        let kind = format.sops_type();
        let json = self.run(None, &["decrypt", "--input-type", kind, "--output-type", "json"], None, &[], Some(encrypted))?;
        Self::parse(&json)
    }

    fn set(&self, file: &FileHandle, path: &SecretPath, value: &Scalar) -> Result<()> {
        let kind = file.format.sops_type();
        let index = path.to_sops_index()?;
        self.run(
            file.config.as_deref(),
            &["set", "--input-type", kind, "--output-type", kind, "--value-stdin"],
            Some(&file.absolute),
            &[&index],
            Some(&json_tree::to_json(value)),
        )
        .map(drop)
    }

    fn unset(&self, file: &FileHandle, path: &SecretPath) -> Result<()> {
        let kind = file.format.sops_type();
        let index = path.to_sops_index()?;
        self.run(
            file.config.as_deref(),
            &["unset", "--input-type", kind, "--output-type", kind],
            Some(&file.absolute),
            &[&index],
            None,
        )
        .map(drop)
    }

    fn sync_recipients(&self, file: &FileHandle) -> Result<()> {
        let kind = file.format.sops_type();
        self.run(file.config.as_deref(), &["updatekeys", "--yes", "--input-type", kind], Some(&file.absolute), &[], None)
            .map(drop)
    }

    fn rotate(&self, file: &FileHandle) -> Result<()> {
        let kind = file.format.sops_type();
        self.run(
            file.config.as_deref(),
            &["rotate", "--in-place", "--input-type", kind, "--output-type", kind],
            Some(&file.absolute),
            &[],
            None,
        )
        .map(drop)
    }
}
