// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ============================================================
// Antrag 3000 – Förderer-App (Rust-Kern).
//
// Bewusst schlank: Der Förderer verbindet sich ONLINE mit dem Dienst
// (kopiersichere Einladung – der Schlüssel entsteht lokal und reist nie mit)
// und pflegt seine Programme live. Alle Verbindungs-/Netzwerk-Befehle liegen
// in verbinden.rs; der Förderer-Privatschlüssel bleibt im Rust-Teil und geht
// NIE in die Weboberfläche.
//
// (Das frühere Datei-Export-/Signatur-Modell wurde durch die authentifizierte
// Online-Verbindung ersetzt und ist entfallen.)
// ============================================================

mod verbinden;

use tauri_plugin_updater::UpdaterExt;

// --- Signiertes Selbstupdate (wie Nutzer- und Admin-App) ---

/// Prüft den Updater-Endpoint. Rückgabe: neue Version, oder None wenn aktuell.
#[tauri::command]
async fn nach_update_suchen(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let upd = app.updater().map_err(|e| e.to_string())?;
    match upd.check().await.map_err(|e| e.to_string())? {
        Some(u) => Ok(Some(u.version)),
        None => Ok(None),
    }
}

/// Lädt das (minisign-verifizierte) Update, installiert es und startet neu.
#[tauri::command]
async fn update_installieren(app: tauri::AppHandle) -> Result<(), String> {
    let upd = app.updater().map_err(|e| e.to_string())?;
    if let Some(u) = upd.check().await.map_err(|e| e.to_string())? {
        u.download_and_install(|_, _| {}, || {})
            .await
            .map_err(|e| e.to_string())?;
        app.restart();
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            // Signiertes Selbstupdate.
            nach_update_suchen,
            update_installieren,
            // Gehostetes Modell (Roadmap 7): online verbinden + live syncen.
            verbinden::foerderer_einladung_lesen,
            verbinden::foerderer_einladung_waehlen,
            verbinden::foerderer_verbinden,
            verbinden::verbindung_status,
            verbinden::verbindung_trennen,
            verbinden::programme_holen,
            verbinden::programm_senden,
            verbinden::programm_loeschen,
            verbinden::ausweis_auto_erneuern,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Förderer-App");
}
