//! Die Schale: ein Fenster und die Befehle, die die Oberfläche aufrufen darf.
//! Fachliches steht hier nicht. Jeder Befehl reicht an einen Anwendungsfall weiter.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod updates;

fn main() {
    // `gedoens pfad/zum/repo` öffnet den Ordner direkt.
    let folder = std::env::args_os().nth(1).map(std::path::PathBuf::from).filter(|path| path.is_dir());
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::Updates::new())
        .setup(|app| {
            updates::start(app.handle());
            Ok(())
        })
        .manage(commands::Session::new(gedoens_infrastructure::build()).with_folder(folder))
        .invoke_handler(tauri::generate_handler![
            commands::update_status,
            commands::check_for_update,
            commands::restart_to_update,
            commands::key_status,
            commands::generate_key,
            commands::pick_key_folder,
            commands::copy_public_key,
            commands::pick_folder,
            commands::open_folder,
            commands::current_folder,
            commands::overview,
            commands::shared_values,
            commands::compare,
            commands::changes,
            commands::publish,
            commands::open_url,
            commands::file,
            commands::reveal,
            commands::copy_value,
            commands::set_value,
            commands::remove_value,
            commands::align,
            commands::access,
            commands::grant,
            commands::revoke,
            commands::name_key,
            commands::merge_preview,
            commands::merge_apply,
        ])
        .run(tauri::generate_context!())
        .expect("gedoens could not start");
}
