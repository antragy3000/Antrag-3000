// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ============================================================
// Antrag 3000 – Förderer-App.
//
// Schlanke, eigenständige App. Der Rust-Kern ist bewusst minimal: bisher
// nur ein "Speichern unter"-Dialog, um den Förderer-Export als Datei zu
// sichern (Weg A: der Förderer schickt diese Datei an Antrag 3000).
// ============================================================

use tauri_plugin_dialog::DialogExt;

/// Speichert den Export-Text über einen "Speichern unter"-Dialog.
/// Rückgabe: gewählter Pfad, oder None wenn abgebrochen.
#[tauri::command]
fn export_speichern(
    app: tauri::AppHandle,
    vorschlag_name: String,
    inhalt: String,
) -> Result<Option<String>, String> {
    let datei = app
        .dialog()
        .file()
        .set_file_name(&vorschlag_name)
        .add_filter("Antrag 3000 Förderer-Export", &["json"])
        .blocking_save_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    std::fs::write(&pfad, inhalt).map_err(|e| format!("Datei nicht speicherbar: {e}"))?;
    Ok(Some(pfad.to_string_lossy().to_string()))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![export_speichern])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Förderer-App");
}
