// ============================================================
// Foerder-Katalog-Override (Phase 3 / Etappe 1).
//
// Der Foerder-Katalog ist UNKRITISCH (Topf 2). Mitgeliefert wird eine
// Standard-Fassung (in der App eingebacken). Ein Update kann eine neuere
// Fassung als Datei im App-Datenordner ablegen; die App laedt dann diese
// statt der Standard-Fassung. Faellt die Datei weg, gilt wieder der
// Werkszustand.
//
// Hier passiert NUR Dateizugriff (Rust-Aufgabe laut CLAUDE.md). Ob eine
// geladene Fassung gueltig/uebernehmbar ist, entscheidet das Frontend.
// ============================================================

use std::fs;
use std::path::PathBuf;
use tauri::Manager;

/// Pfad der Override-Datei: %APPDATA%\<app>\foerderungen.json
fn katalog_pfad(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let ordner = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Datenordner nicht ermittelbar: {e}"))?;
    fs::create_dir_all(&ordner).map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
    Ok(ordner.join("foerderungen.json"))
}

/// Laedt die Override-Fassung des Katalogs, falls vorhanden. Gibt den
/// rohen JSON-Text zurueck (None, wenn keine Datei da ist). Das Pruefen
/// uebernimmt das Frontend.
#[tauri::command]
pub fn katalog_laden(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let pfad = katalog_pfad(&app)?;
    if !pfad.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&pfad)
        .map_err(|e| format!("Katalog-Datei nicht lesbar: {e}"))?;
    Ok(Some(text))
}

/// Speichert eine (vom Nutzer freigegebene) Katalog-Fassung als Override.
/// Schreibt erst in eine Temp-Datei und benennt dann um, damit ein
/// Absturz mitten im Schreiben nie eine halbe Datei hinterlaesst.
#[tauri::command]
pub fn katalog_speichern(app: tauri::AppHandle, inhalt: String) -> Result<(), String> {
    let pfad = katalog_pfad(&app)?;
    let tmp = pfad.with_extension("json.tmp");
    fs::write(&tmp, inhalt.as_bytes())
        .map_err(|e| format!("Katalog nicht schreibbar: {e}"))?;
    fs::rename(&tmp, &pfad).map_err(|e| format!("Katalog-Umbenennen fehlgeschlagen: {e}"))?;
    Ok(())
}

/// Entfernt den Override -> die App nutzt wieder die mitgelieferte
/// Standard-Fassung ("Werkszustand").
#[tauri::command]
pub fn katalog_zuruecksetzen(app: tauri::AppHandle) -> Result<(), String> {
    let pfad = katalog_pfad(&app)?;
    if pfad.exists() {
        fs::remove_file(&pfad).map_err(|e| format!("Katalog nicht entfernbar: {e}"))?;
    }
    Ok(())
}
