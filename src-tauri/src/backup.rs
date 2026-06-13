// ============================================================
// Sicherung (Backup) des Tresors.
//
// Der Tresor (tresor.enc) ist bereits verschluesselt. Ein Backup ist
// schlicht eine Kopie dieser Datei an einen Ort, den der Nutzer waehlt
// (USB-Stick, Cloud-Ordner ...). Weil die Kopie verschluesselt ist,
// ist sie auch dort sicher.
//
// Wiederherstellen ersetzt den aktuellen Tresor durch eine Sicherung.
// Danach muss mit dem Passwort entsperrt werden, das zu DIESER
// Sicherung gehoert.
// ============================================================

use std::fs;

use crate::tresor;

/// Kopiert den Tresor an den gewaehlten Zielpfad.
#[tauri::command]
pub fn tresor_backup_erstellen(app: tauri::AppHandle, ziel: String) -> Result<(), String> {
    let quelle = tresor::tresor_pfad(&app)?;
    if !quelle.exists() {
        return Err("Es gibt noch keinen Tresor zum Sichern.".into());
    }
    fs::copy(&quelle, &ziel).map_err(|e| format!("Sicherung fehlgeschlagen: {e}"))?;
    Ok(())
}

/// Spielt eine Sicherung ein und ersetzt den aktuellen Tresor.
/// Der bisherige Tresor wird vorher beiseitegelegt (nicht geloescht).
#[tauri::command]
pub fn tresor_backup_einspielen(
    app: tauri::AppHandle,
    state: tauri::State<tresor::TresorZustand>,
    quelle: String,
) -> Result<(), String> {
    let daten = fs::read(&quelle).map_err(|e| format!("Sicherungsdatei nicht lesbar: {e}"))?;
    // Nur eine echte Antrag-3000-Sicherung akzeptieren.
    if daten.len() < 9 || &daten[..8] != tresor::MAGIC {
        return Err("Das ist keine gueltige Antrag-3000-Sicherung.".into());
    }

    let ziel = tresor::tresor_pfad(&app)?;
    if ziel.exists() {
        let vorher = ziel.with_extension("enc.vor-wiederherstellung");
        let _ = fs::remove_file(&vorher);
        fs::rename(&ziel, &vorher)
            .map_err(|e| format!("Aktuellen Tresor konnte nicht gesichert werden: {e}"))?;
    }
    fs::write(&ziel, &daten).map_err(|e| format!("Wiederherstellen fehlgeschlagen: {e}"))?;

    // Sicherheitshalber sperren: der wiederhergestellte Tresor wird mit
    // (s)einem Passwort neu entsperrt.
    *state.geheim.lock().unwrap() = None;
    Ok(())
}
