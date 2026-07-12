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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Gehostetes Modell (Roadmap 7): online verbinden + live syncen.
            verbinden::foerderer_einladung_lesen,
            verbinden::foerderer_einladung_waehlen,
            verbinden::foerderer_verbinden,
            verbinden::verbindung_status,
            verbinden::verbindung_trennen,
            verbinden::programme_holen,
            verbinden::programm_senden,
            verbinden::programm_loeschen,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Förderer-App");
}
