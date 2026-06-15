// Einstiegspunkt des Rust-Kerns. Hier wird beim Start registriert,
// welche Befehle das JS-Frontend aufrufen darf (invoke_handler) und
// welcher Zustand im Speicher verwaltet wird (manage).

mod backup;
mod dokument;
mod excel;
mod katalog;
mod ordner;
mod pdf;
mod sync;
mod tresor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(tresor::TresorZustand::default())
        .invoke_handler(tauri::generate_handler![
            tresor::tresor_status,
            tresor::tresor_erstellen,
            tresor::tresor_entsperren,
            tresor::tresor_speichern,
            tresor::tresor_sperren,
            tresor::tresor_neu_aufsetzen,
            ordner::ordner_oeffnen,
            ordner::ordner_umbenennen,
            dokument::formular_word_erzeugen,
            dokument::dokument_hochladen,
            pdf::antrags_pdf_vorschau,
            pdf::antrags_pdf_speichern,
            sync::zugangspaket_pruefen,
            sync::sync_health,
            sync::sync_get_board,
            sync::sync_put_board,
            sync::sync_delete_board,
            sync::sync_trockenlauf,
            sync::team_ca_erstellen,
            sync::geraet_paket_speichern,
            sync::geraet_paket_direkt,
            sync::team_ca_cert_exportieren,
            excel::kfp_excel_schreiben,
            katalog::katalog_laden,
            katalog::katalog_speichern,
            katalog::katalog_zuruecksetzen,
            backup::tresor_backup_erstellen,
            backup::tresor_backup_einspielen
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Anwendung");
}
