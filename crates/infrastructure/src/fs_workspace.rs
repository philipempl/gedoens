use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use gedoens_application::ports::{FoundFile, Workspace};
use gedoens_application::Result;
use gedoens_domain::{FileFormat, Recipient};
use regex::Regex;

const SKIPPED_FOLDERS: [&str; 4] = ["node_modules", "target", "vendor", "dist"];
const LARGEST_FILE: u64 = 2 * 1024 * 1024;
const POLICY_FILE: &str = ".sops.yaml";

/// Der geöffnete Ordner auf der Platte.
pub struct FsWorkspace;

impl FsWorkspace {
    fn visit(&self, root: &Path, folder: &Path, found: &mut Vec<FoundFile>) -> std::io::Result<()> {
        let mut entries: Vec<_> = fs::read_dir(folder)?.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                if !name.starts_with('.') && !SKIPPED_FOLDERS.contains(&name.as_str()) {
                    self.visit(root, &path, found)?;
                }
                continue;
            }
            let Some(format) = FileFormat::from_file_name(&name) else { continue };
            if name == POLICY_FILE || !kind.is_file() || entry.metadata()?.len() > LARGEST_FILE {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else { continue };
            if !is_sops_file(&text, format) {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            found.push(FoundFile { relative, format, recipients: self.recipients_in(&text) });
        }
        Ok(())
    }
}

/// Jedes Format trägt die SOPS-Metadaten an einer anderen Stelle.
fn is_sops_file(text: &str, format: FileFormat) -> bool {
    match format {
        FileFormat::Yaml => text.lines().any(|l| l.starts_with("sops:")),
        FileFormat::Json => text.contains("\"sops\":"),
        FileFormat::Dotenv => text.lines().any(|l| l.starts_with("sops_version=")),
        FileFormat::Ini => text.lines().any(|l| l.trim() == "[sops]"),
    }
}

impl Workspace for FsWorkspace {
    fn scan(&self, root: &Path) -> Result<Vec<FoundFile>> {
        let mut found = Vec::new();
        self.visit(root, root, &mut found)?;
        Ok(found)
    }

    fn recipients_in(&self, encrypted: &str) -> Vec<Recipient> {
        static PATTERN: OnceLock<Regex> = OnceLock::new();
        let pattern = PATTERN.get_or_init(|| {
            Regex::new(r#"recipient["']?\s*[:=]\s*["']?(age1[0-9a-z]+|ssh-(?:ed25519|rsa) [A-Za-z0-9+/=]+)"#).expect("festes Muster")
        });
        let mut all: Vec<Recipient> = Vec::new();
        for capture in pattern.captures_iter(encrypted) {
            if let Ok(recipient) = Recipient::parse(&capture[1]) {
                if !all.contains(&recipient) {
                    all.push(recipient);
                }
            }
        }
        all
    }

    fn write(&self, root: &Path, relative: &str, content: &str) -> Result<()> {
        Ok(fs::write(root.join(relative), content)?)
    }

    fn policy_path(&self, root: &Path) -> Option<PathBuf> {
        let path = root.join(POLICY_FILE);
        path.is_file().then_some(path)
    }
}
