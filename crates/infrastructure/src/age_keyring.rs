use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use age::secrecy::ExposeSecret;
use gedoens_application::ports::KeyRing;
use gedoens_application::{AppError, Result};
use gedoens_domain::Recipient;

/// Die eigenen Keys in der `keys.txt`, genau dort, wo SOPS sie sucht.
/// So funktioniert derselbe Key in der App und im Terminal.
#[derive(Default)]
pub struct AgeKeyRing {
    /// Nur für Tests. Sonst gilt der Ort, den SOPS vorgibt.
    file: Option<PathBuf>,
}

impl AgeKeyRing {
    pub fn at(file: PathBuf) -> Self {
        Self { file: Some(file) }
    }

    /// Die Suchreihenfolge aus der SOPS-Dokumentation.
    fn key_file(&self) -> Option<PathBuf> {
        if let Some(file) = &self.file {
            return Some(file.clone());
        }
        if let Some(explicit) = env::var_os("SOPS_AGE_KEY_FILE") {
            return Some(PathBuf::from(explicit));
        }
        if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(xdg).join("sops/age/keys.txt"));
        }
        if cfg!(windows) {
            return env::var_os("AppData").map(|dir| PathBuf::from(dir).join("sops").join("age").join("keys.txt"));
        }
        let home = PathBuf::from(env::var_os("HOME")?);
        Some(if cfg!(target_os = "macos") {
            home.join("Library/Application Support/sops/age/keys.txt")
        } else {
            home.join(".config/sops/age/keys.txt")
        })
    }
}

impl KeyRing for AgeKeyRing {
    fn own(&self) -> Vec<Recipient> {
        let Some(text) = self.key_file().and_then(|file| fs::read_to_string(file).ok()) else {
            return Vec::new();
        };
        text.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("AGE-SECRET-KEY-"))
            .filter_map(|line| line.parse::<age::x25519::Identity>().ok())
            .filter_map(|identity| Recipient::parse(&identity.to_public().to_string()).ok())
            .collect()
    }

    fn generate(&self) -> Result<Recipient> {
        let file = self.key_file().ok_or_else(|| AppError::Io("Der Ort für die keys.txt ist unbekannt.".into()))?;
        if let Some(folder) = file.parent() {
            fs::create_dir_all(folder)?;
        }
        let identity = age::x25519::Identity::generate();
        let public = identity.to_public().to_string();

        let mut options = fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut handle = options.open(&file)?;
        writeln!(handle, "# public key: {public}")?;
        writeln!(handle, "{}", identity.to_string().expose_secret())?;
        Ok(Recipient::parse(&public)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_key_is_found_again() {
        let folder = tempfile::tempdir().unwrap();
        let ring = AgeKeyRing::at(folder.path().join("sops/age/keys.txt"));
        assert!(ring.own().is_empty());
        let created = ring.generate().unwrap();
        assert_eq!(ring.own(), vec![created]);
    }
}
