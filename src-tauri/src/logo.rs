// ============================================================
// Förderer-Logo herunterladen (Etappe 3c).
//
// Das VOLLE Logo eines verifizierten Förderers wird vom Frontend (über den
// Sync-Client) als Data-URL geholt und hier per Speichern-Dialog als Bilddatei
// abgelegt. Die Nutzer:in kann es dann frei weiterverwenden (Projekt-
// kommunikation, Nachweise usw.). Reiner lokaler Dateizugriff – die Netz-Seite
// (Abruf) liegt in sync.rs.
// ============================================================

use tauri_plugin_dialog::DialogExt;

/// Bestimmt eine passende Dateiendung aus dem Data-URL-MIME. Förderer-Logos
/// sind PNG oder JPG (die Förderer-App lässt nur diese zu); Fallback png.
fn endung_aus_dataurl(daten: &str) -> &'static str {
    let anfang = daten.get(..32).unwrap_or(daten).to_ascii_lowercase();
    if anfang.starts_with("data:image/jpeg") || anfang.starts_with("data:image/jpg") {
        "jpg"
    } else if anfang.starts_with("data:image/webp") {
        "webp"
    } else if anfang.starts_with("data:image/gif") {
        "gif"
    } else {
        "png"
    }
}

/// Speichert ein als Data-URL übergebenes Förderer-Logo über einen Speichern-
/// Dialog als Bilddatei. Gibt den Zielpfad zurück (None, wenn abgebrochen).
#[tauri::command]
pub fn logo_herunterladen(
    app: tauri::AppHandle,
    daten_url: String,
    vorschlag_name: String,
) -> Result<Option<String>, String> {
    // Data-URL zu Bytes dekodieren (gemeinsamer Helfer der PDF-Erzeugung).
    let bytes = crate::pdf::logo_bytes(&daten_url).ok_or("Logo konnte nicht gelesen werden.")?;
    let ext = endung_aus_dataurl(&daten_url);

    let basis = vorschlag_name.trim();
    let basis = if basis.is_empty() { "foerderer-logo" } else { basis };
    let name = format!("{basis}.{ext}");

    let ziel = app
        .dialog()
        .file()
        .set_file_name(&name)
        .add_filter("Bild", &[ext])
        .blocking_save_file();
    let Some(fp) = ziel else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    std::fs::write(&pfad, &bytes).map_err(|e| format!("Datei nicht schreibbar: {e}"))?;
    Ok(Some(pfad.to_string_lossy().to_string()))
}
