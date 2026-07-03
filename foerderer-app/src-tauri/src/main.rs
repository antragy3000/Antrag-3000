// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ============================================================
// Antrag 3000 – Förderer-App (Grundgerüst / Phase 0).
//
// Eigenständige, schlanke Anwendung – getrennt von der Nutzer-App und der
// Admin-App. Aktuell nur ein leeres, lauffähiges Fenster. Funktionen
// folgen bewusst in kleinen Schritten; erst dann kommen weitere
// Abhängigkeiten (z. B. Datei-/Netzwerkzugriff) hinzu.
// ============================================================

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Förderer-App");
}
