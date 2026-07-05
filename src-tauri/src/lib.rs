// Einstiegspunkt des Rust-Kerns. Hier wird beim Start registriert,
// welche Befehle das JS-Frontend aufrufen darf (invoke_handler) und
// welcher Zustand im Speicher verwaltet wird (manage).

mod backup;
mod beleg;
mod dokument;
mod excel;
mod katalog;
mod logo;
mod ordner;
mod pdf;
mod sync;
mod tresor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Etappe 5: signiertes Selbstupdate + Neustart nach dem Update.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(tresor::TresorZustand::default())
        .invoke_handler(tauri::generate_handler![
            tresor::tresor_status,
            tresor::tresor_erstellen,
            tresor::tresor_entsperren,
            tresor::tresor_speichern,
            tresor::tresor_sperren,
            tresor::tresor_neu_aufsetzen,
            tresor::merken_moeglich,
            tresor::merken_status,
            tresor::merken_anlegen,
            tresor::merken_entsperren,
            tresor::merken_vergessen,
            ordner::ordner_oeffnen,
            ordner::ordner_umbenennen,
            ordner::temp_aufraeumen,
            dokument::formular_word_erzeugen,
            dokument::dokument_hochladen,
            dokument::verwendungsnachweis_word,
            pdf::verwendungsnachweis_pdf,
            beleg::beleg_datei_hinzufuegen,
            beleg::beleg_datei_oeffnen,
            beleg::beleg_datei_exportieren,
            beleg::beleg_datei_entfernen,
            beleg::beleg_ordner_entfernen,
            pdf::antrags_pdf_vorschau,
            pdf::antrags_pdf_speichern,
            sync::zugangspaket_pruefen,
            sync::sync_health,
            sync::sync_get_board,
            sync::sync_put_board,
            sync::sync_delete_board,
            sync::sync_trockenlauf,
            sync::sync_katalog_holen,
            sync::katalog_oeffentlich_holen,
            sync::sync_logo_holen,
            sync::logo_oeffentlich_holen,
            logo::logo_herunterladen,
            sync::sync_meldung_senden,
            sync::sync_foerderer_holen,
            sync::sync_foerderer_senden,
            sync::sync_foerderer_loeschen,
            sync::team_ca_erstellen,
            sync::geraet_paket_speichern,
            sync::geraet_paket_direkt,
            sync::team_ca_cert_exportieren,
            sync::server_zertifikat_speichern,
            excel::kfp_excel_schreiben,
            katalog::katalog_laden,
            katalog::katalog_speichern,
            katalog::katalog_kandidat_lesen,
            katalog::katalog_zuruecksetzen,
            backup::tresor_backup_erstellen,
            backup::tresor_backup_einspielen
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Starten der Anwendung");
}
