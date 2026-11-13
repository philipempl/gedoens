use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use age::secrecy::ExposeSecret;
use gedoens_application::ports::{KeyLocations, KeyRing};
use gedoens_application::{AppError, Result};
use gedoens_domain::Recipient;

/// Name of the inline variable SOPS reads keys from.
const INLINE: &str = "SOPS_AGE_KEY";
/// Key files larger than this are not key files.
const MAX_KEY_FILE: u64 = 256 * 1024;

/// The user's own age keys.
///
/// They live in a `keys.txt`, ideally exactly where SOPS looks for it, so the same
/// key works in the app and in the terminal. An app started from the Dock does not
/// see variables from the shell profile, so gedoens searches every usual place,
/// plus a folder the user picked once and that is remembered outside the repository.
pub struct AgeKeyRing {
    /// Only for tests: this one file and nothing else.
    file: Option<PathBuf>,
    /// Where the folder picked by the user is remembered.
    settings: Option<PathBuf>,
}

impl Default for AgeKeyRing {
    fn default() -> Self {
        Self { file: None, settings: settings_file() }
    }
}

impl AgeKeyRing {
    pub fn at(file: PathBuf) -> Self {
        Self { file: Some(file), settings: None }
    }

    /// Every file that may hold keys, most specific first, without duplicates.
    fn candidates(&self) -> Vec<PathBuf> {
        if let Some(file) = &self.file {
            return vec![file.clone()];
        }
        let home = env::var_os("HOME").map(PathBuf::from);
        let mut all: Vec<PathBuf> = Vec::new();
        all.extend(self.remembered());
        all.extend(env::var_os("SOPS_AGE_KEY_FILE").map(PathBuf::from));
        all.extend(env::var_os("XDG_CONFIG_HOME").map(|xdg| PathBuf::from(xdg).join("sops/age/keys.txt")));
        // The place SOPS itself uses when nothing is set.
        if cfg!(windows) {
            all.extend(env::var_os("AppData").map(|dir| PathBuf::from(dir).join("sops").join("age").join("keys.txt")));
        } else if cfg!(target_os = "macos") {
            all.extend(home.as_ref().map(|h| h.join("Library/Application Support/sops/age/keys.txt")));
        }
        // Where most guides put it, also on macOS.
        all.extend(home.as_ref().map(|h| h.join(".config/sops/age/keys.txt")));

        let mut unique: Vec<PathBuf> = Vec::new();
        for path in all {
            if !unique.contains(&path) {
                unique.push(path);
            }
        }
        unique
    }

    fn remembered(&self) -> Option<PathBuf> {
        let text = fs::read_to_string(self.settings.as_ref()?).ok()?;
        let path = PathBuf::from(text.trim());
        (!text.trim().is_empty()).then_some(path)
    }

    fn inline(&self) -> Vec<String> {
        if self.file.is_some() {
            return Vec::new();
        }
        env::var(INLINE).map(|text| identities_in(&text)).unwrap_or_default()
    }

    /// All secret keys from all sources, one per line, ready for `SOPS_AGE_KEY`.
    /// Handing them to `sops` directly means it finds them wherever they live.
    pub fn secret_keys(&self) -> Vec<String> {
        let mut all = self.inline();
        for path in self.candidates() {
            for key in read_identities(&path) {
                if !all.contains(&key) {
                    all.push(key);
                }
            }
        }
        all
    }
}

impl KeyRing for AgeKeyRing {
    fn own(&self) -> Vec<Recipient> {
        let mut own: Vec<Recipient> = Vec::new();
        for key in self.secret_keys() {
            let Ok(identity) = key.parse::<age::x25519::Identity>() else { continue };
            if let Ok(recipient) = Recipient::parse(&identity.to_public().to_string()) {
                if !own.contains(&recipient) {
                    own.push(recipient);
                }
            }
        }
        own
    }

    fn locations(&self) -> KeyLocations {
        let candidates = self.candidates();
        let mut searched: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
        let mut found_in: Vec<String> = candidates
            .iter()
            .filter(|path| !read_identities(path).is_empty())
            .map(|p| p.display().to_string())
            .collect();
        if self.file.is_none() {
            searched.insert(0, format!("${INLINE}"));
            if !self.inline().is_empty() {
                found_in.insert(0, format!("${INLINE}"));
            }
        }
        KeyLocations { searched, found_in }
    }

    fn generate(&self) -> Result<Recipient> {
        if let Some(existing) = self.own().into_iter().next() {
            // Appending would not destroy anything, but a second key nobody uses is just confusing.
            return Err(AppError::Unsupported(format!("You already have a key ({}). gedoens uses it.", existing.short())));
        }
        let file = self
            .candidates()
            .into_iter()
            .next()
            .ok_or_else(|| AppError::Io("The place for keys.txt is unknown.".into()))?;
        if let Some(folder) = file.parent() {
            fs::create_dir_all(folder)?;
        }
        backup(&file)?;
        let identity = age::x25519::Identity::generate();
        let public = identity.to_public().to_string();

        let mut options = fs::OpenOptions::new();
        // Append, never truncate: whatever is in the file stays.
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

    fn remember(&self, place: &Path) -> Result<PathBuf> {
        let settings = self
            .settings
            .as_ref()
            .ok_or_else(|| AppError::Io("There is no place to remember the key folder.".into()))?;
        let file = key_file_in(place).ok_or_else(|| {
            AppError::Unsupported(format!(
                "There is no age key in {}. gedoens looks for a file with a line starting with AGE-SECRET-KEY-.",
                place.display()
            ))
        })?;
        if let Some(folder) = settings.parent() {
            fs::create_dir_all(folder)?;
        }
        fs::write(settings, file.display().to_string())?;
        Ok(file)
    }
}

/// Copies an existing key file next to itself before it is touched,
/// as `keys.txt.backup-<unix seconds>`. Nothing to do if there is no file yet.
fn backup(file: &Path) -> Result<Option<PathBuf>> {
    if !file.exists() {
        return Ok(None);
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "keys.txt".into());
    let mut target = file.with_file_name(format!("{name}.backup-{stamp}"));
    let mut n = 1;
    while target.exists() {
        target = file.with_file_name(format!("{name}.backup-{stamp}-{n}"));
        n += 1;
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(&target)?.write_all(&fs::read(file)?)?;
    Ok(Some(target))
}

/// The file with keys at this place: the file itself, a `keys.txt` in the folder,
/// or any other small file in the folder that holds a key.
fn key_file_in(place: &Path) -> Option<PathBuf> {
    if place.is_file() {
        return (!read_identities(place).is_empty()).then(|| place.to_path_buf());
    }
    let usual = place.join("keys.txt");
    if !read_identities(&usual).is_empty() {
        return Some(usual);
    }
    let mut files: Vec<PathBuf> = fs::read_dir(place).ok()?.flatten().map(|entry| entry.path()).filter(|p| p.is_file()).collect();
    files.sort();
    files.into_iter().find(|path| !read_identities(path).is_empty())
}

fn read_identities(path: &Path) -> Vec<String> {
    let small = fs::metadata(path).map(|m| m.len() <= MAX_KEY_FILE).unwrap_or(false);
    if !small {
        return Vec::new();
    }
    fs::read_to_string(path).map(|text| identities_in(&text)).unwrap_or_default()
}

fn identities_in(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| line.starts_with("AGE-SECRET-KEY-"))
        .filter(|line| line.parse::<age::x25519::Identity>().is_ok())
        .map(str::to_owned)
        .collect()
}

/// gedoens' own settings, outside any repository.
fn settings_file() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        PathBuf::from(env::var_os("AppData")?)
    } else if cfg!(target_os = "macos") {
        PathBuf::from(env::var_os("HOME")?).join("Library/Application Support")
    } else if let Some(xdg) = env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        PathBuf::from(env::var_os("HOME")?).join(".config")
    };
    Some(base.join("gedoens").join("key-file"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(folder: &Path) -> AgeKeyRing {
        AgeKeyRing { file: Some(folder.join("sops/age/keys.txt")), settings: Some(folder.join("gedoens/key-file")) }
    }

    #[test]
    fn a_generated_key_is_found_again() {
        let folder = tempfile::tempdir().unwrap();
        let ring = ring(folder.path());
        assert!(ring.own().is_empty());
        let created = ring.generate().unwrap();
        assert_eq!(ring.own(), vec![created]);
    }

    #[test]
    fn never_creates_a_second_key() {
        let folder = tempfile::tempdir().unwrap();
        let ring = ring(folder.path());
        let first = ring.generate().unwrap();
        let before = fs::read_to_string(folder.path().join("sops/age/keys.txt")).unwrap();
        assert!(ring.generate().is_err());
        assert_eq!(fs::read_to_string(folder.path().join("sops/age/keys.txt")).unwrap(), before);
        assert_eq!(ring.own(), vec![first]);
    }

    #[test]
    fn backs_up_an_existing_key_file_before_writing() {
        let folder = tempfile::tempdir().unwrap();
        let ring = ring(folder.path());
        let file = folder.path().join("sops/age/keys.txt");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "# just a note, no key yet\n").unwrap();
        ring.generate().unwrap();
        let backups: Vec<_> = fs::read_dir(file.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("keys.txt.backup-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read_to_string(backups[0].path()).unwrap(), "# just a note, no key yet\n");
        assert!(fs::read_to_string(&file).unwrap().starts_with("# just a note, no key yet\n"));
    }

    #[test]
    fn a_picked_folder_is_remembered() {
        let folder = tempfile::tempdir().unwrap();
        let elsewhere = folder.path().join("elsewhere");
        let made = AgeKeyRing::at(elsewhere.join("keys.txt")).generate().unwrap();

        let ring = AgeKeyRing { file: None, settings: Some(folder.path().join("gedoens/key-file")) };
        assert!(ring.remember(&folder.path().join("empty")).is_err());
        assert_eq!(ring.remember(&elsewhere).unwrap(), elsewhere.join("keys.txt"));
        assert!(ring.own().contains(&made));
        assert!(ring.locations().found_in.contains(&elsewhere.join("keys.txt").display().to_string()));
    }
}
