use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use gedoens_application::views::{AccessView, Choice, FileView, KeyStatus, MergePreview, Overview, Step};
use gedoens_application::Gedoens;
use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

/// Nach dieser Zeit leert die App die Zwischenablage wieder.
const CLIPBOARD_LIFETIME: Duration = Duration::from_secs(30);

/// Das Einzige, was die Schale sich merkt: welcher Ordner offen ist.
pub struct Session {
    app: Gedoens,
    root: Mutex<Option<PathBuf>>,
}

/// Fehler gehen als fertiger Satz an die Oberfläche.
type Reply<T> = Result<T, String>;

impl Session {
    pub fn new(app: Gedoens) -> Self {
        Self { app, root: Mutex::new(None) }
    }

    /// Öffnet gleich einen Ordner, etwa bei `gedoens pfad/zum/repo` im Terminal.
    pub fn with_folder(self, folder: Option<PathBuf>) -> Self {
        if let Some(folder) = folder {
            let _ = self.open(folder);
        }
        self
    }

    fn root(&self) -> Reply<PathBuf> {
        self.root
            .lock()
            .map_err(|_| "Interner Fehler.".to_owned())?
            .clone()
            .ok_or_else(|| "Es ist kein Ordner geöffnet.".to_owned())
    }

    fn open(&self, folder: PathBuf) -> Reply<String> {
        if !folder.is_dir() {
            return Err("Diesen Ordner gibt es nicht mehr.".to_owned());
        }
        let name = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        *self.root.lock().map_err(|_| "Interner Fehler.".to_owned())? = Some(folder);
        Ok(name)
    }
}

fn say<T>(result: gedoens_application::Result<T>) -> Reply<T> {
    result.map_err(|error| error.to_string())
}

fn copy(handle: &AppHandle, text: String, forget: bool) -> Reply<()> {
    handle.clipboard().write_text(text.clone()).map_err(|e| e.to_string())?;
    if forget {
        let handle = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(CLIPBOARD_LIFETIME);
            // Nur leeren, wenn inzwischen nichts anderes kopiert wurde.
            if handle.clipboard().read_text().ok().as_deref() == Some(text.as_str()) {
                let _ = handle.clipboard().write_text(String::new());
            }
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn key_status(session: State<'_, Session>) -> Reply<KeyStatus> {
    Ok(session.app.key_status())
}

#[tauri::command]
pub async fn generate_key(session: State<'_, Session>) -> Reply<KeyStatus> {
    say(session.app.generate_key())
}

#[tauri::command]
pub async fn copy_public_key(handle: AppHandle, session: State<'_, Session>) -> Reply<()> {
    let key = session.app.key_status().public_keys.into_iter().next().ok_or("Du hast noch keinen Key.")?;
    copy(&handle, key, false)
}

/// Öffnet den Auswahldialog des Systems. Gibt den Pfad zurück oder nichts,
/// wenn abgebrochen wurde.
#[tauri::command]
pub async fn pick_folder(handle: AppHandle) -> Reply<Option<String>> {
    let picked = handle.dialog().file().blocking_pick_folder();
    Ok(picked.and_then(|path| path.into_path().ok()).map(|path| path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn open_folder(session: State<'_, Session>, folder: String) -> Reply<String> {
    session.open(PathBuf::from(folder))
}

/// Der Name des Ordners, der schon beim Start geöffnet wurde.
#[tauri::command]
pub async fn current_folder(session: State<'_, Session>) -> Reply<Option<String>> {
    Ok(session.root().ok().and_then(|root| root.file_name().map(|n| n.to_string_lossy().into_owned())))
}

#[tauri::command]
pub async fn overview(session: State<'_, Session>) -> Reply<Overview> {
    say(session.app.overview(&session.root()?))
}

#[tauri::command]
pub async fn file(session: State<'_, Session>, file: String) -> Reply<FileView> {
    say(session.app.file(&session.root()?, &file))
}

#[tauri::command]
pub async fn reveal(session: State<'_, Session>, file: String, path: Vec<Step>) -> Reply<String> {
    say(session.app.reveal(&session.root()?, &file, &path))
}

/// Kopiert einen Wert, ohne dass er durch die Oberfläche muss.
#[tauri::command]
pub async fn copy_value(handle: AppHandle, session: State<'_, Session>, file: String, path: Vec<Step>) -> Reply<()> {
    let value = say(session.app.reveal(&session.root()?, &file, &path))?;
    copy(&handle, value, true)
}

#[tauri::command]
pub async fn set_value(session: State<'_, Session>, file: String, path: Vec<Step>, value: String) -> Reply<Option<String>> {
    say(session.app.set_value(&session.root()?, &file, &path, &value))
}

#[tauri::command]
pub async fn remove_value(session: State<'_, Session>, file: String, path: Vec<Step>) -> Reply<Option<String>> {
    say(session.app.remove_value(&session.root()?, &file, &path))
}

#[tauri::command]
pub async fn align(session: State<'_, Session>, file: String) -> Reply<()> {
    say(session.app.align(&session.root()?, &file))
}

#[tauri::command]
pub async fn access(session: State<'_, Session>) -> Reply<AccessView> {
    say(session.app.access(&session.root()?))
}

#[tauri::command]
pub async fn grant(session: State<'_, Session>, recipient: String, area: usize, name: Option<String>) -> Reply<usize> {
    say(session.app.grant(&session.root()?, &recipient, area, name.as_deref()))
}

#[tauri::command]
pub async fn revoke(session: State<'_, Session>, recipient: String, area: usize) -> Reply<usize> {
    say(session.app.revoke(&session.root()?, &recipient, area))
}

#[tauri::command]
pub async fn name_key(session: State<'_, Session>, recipient: String, name: String) -> Reply<()> {
    say(session.app.name_key(&session.root()?, &recipient, &name))
}

#[tauri::command]
pub async fn merge_preview(session: State<'_, Session>, file: String) -> Reply<MergePreview> {
    say(session.app.merge_preview(&session.root()?, &file))
}

#[tauri::command]
pub async fn merge_apply(session: State<'_, Session>, file: String, choices: Vec<Choice>) -> Reply<()> {
    say(session.app.merge_apply(&session.root()?, &file, &choices))
}
