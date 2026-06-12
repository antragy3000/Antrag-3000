// Einstiegspunkt des Rust-Kerns. Hier wird beim Start registriert,
// welche Befehle das JS-Frontend aufrufen darf (invoke_handler) und
// welcher Zustand im Speicher verwaltet wird (manage).

mod tresor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(tresor::TresorZustand::default())
        .invoke_handler(tauri::generate_handler![
            tresor::tresor_status,
            tresor::tresor_erstellen,
            tresor::tresor_entsperren,
            tresor::tresor_speichern,
            tresor::tresor_sperren,
            tresor::tresor_neu_aufsetzen
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Anwendung");
}
