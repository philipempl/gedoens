use std::env;
use std::path::PathBuf;

/// Sucht ein Programm: erst die ausdrückliche Angabe, dann neben der App
/// (mitgeliefert), dann im PATH und an den üblichen Orten.
///
/// Die üblichen Orte sind nötig, weil eine App, die auf macOS aus dem Finder
/// startet, den PATH der Shell nicht kennt und Homebrew sonst nicht findet.
pub fn program(name: &str, override_var: &str) -> Option<PathBuf> {
    if let Some(explicit) = env::var_os(override_var).map(PathBuf::from) {
        if explicit.is_file() {
            return Some(explicit);
        }
    }
    let file_name = if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() };

    let mut folders: Vec<PathBuf> = Vec::new();
    if let Some(beside_app) = env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)) {
        folders.push(beside_app);
    }
    if let Some(path) = env::var_os("PATH") {
        folders.extend(env::split_paths(&path));
    }
    for usual in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/run/current-system/sw/bin"] {
        folders.push(PathBuf::from(usual));
    }
    if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
        folders.push(home.join(".nix-profile/bin"));
        folders.push(home.join(".local/bin"));
    }
    folders.into_iter().map(|folder| folder.join(&file_name)).find(|candidate| candidate.is_file())
}
