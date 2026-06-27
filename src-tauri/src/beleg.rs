// ============================================================
// Abrechnungs-Modus, Phase A2: Beleg-Dateien.
//
// Belege (Fotos/Scans/PDF) sind hochsensibel. Sie werden mit dem
// Tresor-Schluessel VERSCHLUESSELT im Projektordner abgelegt:
//   [Dokumente]\Antrag 3000\[Projekt]\_Belege\[Beleg-ID]\[ref].enc
// Im Tresor steht nur ein Verweis ({ ref, name, ext, groesse }). Der
// Klartext liegt nie unverschluesselt auf der Platte – ausser kurz beim
// Ansehen, wenn die Datei in einen Temp-Ordner entschluesselt und im
// System-Betrachter geoeffnet wird (beleg_datei_oeffnen).
//
// Der Rust-Kern macht nur die Systemarbeit (lesen, ver-/entschluesseln,
// schreiben, oeffnen). Welche Belege es gibt, verwaltet das Frontend im
// Tresor.
// ============================================================

use std::fs;
use std::path::PathBuf;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use serde::Serialize;

use crate::ordner;
use crate::tresor::TresorZustand;

const ERLAUBT: [&str; 4] = ["pdf", "jpg", "jpeg", "png"];
const MAX_BYTES: u64 = 30 * 1024 * 1024; // 30 MB je Datei

/// Verweis auf eine gespeicherte Beleg-Datei (das merkt sich das Frontend
/// im Tresor; serde serialisiert das Feld `r#ref` als "ref").
#[derive(Serialize)]
pub struct BelegDatei {
    pub r#ref: String,
    pub name: String,
    pub ext: String,
    pub groesse: u64,
}

/// Zufaellige 32-stellige Hex-Kennung (keine Rueckschluesse auf den Inhalt).
fn neue_kennung() -> String {
    let mut b = [0u8; 16];
    OsRng.fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Ordner fuer die Dateien EINES Belegs.
fn beleg_ordner(app: &tauri::AppHandle, projekt: &str, beleg_id: &str) -> Result<PathBuf, String> {
    Ok(ordner::wurzel(app)?
        .join(ordner::bereinigen(projekt)?)
        .join("_Belege")
        .join(ordner::bereinigen(beleg_id)?))
}

/// Schuetzt vor Pfad-Tricks im Datei-Verweis (er kommt aus dem Tresor,
/// wir pruefen aber trotzdem).
fn pruefe_ref(r: &str) -> Result<&str, String> {
    if r.is_empty() || r.contains('/') || r.contains('\\') || r.contains("..") {
        return Err("Ungueltiger Datei-Verweis.".into());
    }
    Ok(r)
}

/// Eine gewaehlte Datei verschluesselt im Beleg-Ordner ablegen. Gibt den
/// Verweis zurueck, den das Frontend im Beleg speichert.
#[tauri::command]
pub fn beleg_datei_hinzufuegen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    projekt: String,
    beleg_id: String,
    quelle: String,
) -> Result<BelegDatei, String> {
    let quell_pfad = std::path::Path::new(&quelle);
    let ext = quell_pfad
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !ERLAUBT.contains(&ext.as_str()) {
        return Err("Nur PDF-, JPG- oder PNG-Dateien sind erlaubt.".into());
    }
    let meta = fs::metadata(quell_pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    if meta.len() > MAX_BYTES {
        return Err("Die Datei ist zu groß (max. 30 MB).".into());
    }

    let klar = fs::read(quell_pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let verschluesselt = crate::tresor::datei_verschluesseln(&state, &klar)?;

    let ordner_pfad = beleg_ordner(&app, &projekt, &beleg_id)?;
    fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;

    let datei_ref = format!("{}.enc", neue_kennung());
    let ziel = ordner_pfad.join(&datei_ref);
    fs::write(&ziel, &verschluesselt).map_err(|e| format!("Datei nicht speicherbar: {e}"))?;

    let name = quell_pfad
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Beleg")
        .to_string();
    Ok(BelegDatei { r#ref: datei_ref, name, ext, groesse: meta.len() })
}

/// Eine Beleg-Datei zum Ansehen entschluesseln und im System-Betrachter
/// oeffnen (in einen Temp-Ordner; dort liegt sie kurzzeitig im Klartext).
#[tauri::command]
pub fn beleg_datei_oeffnen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    projekt: String,
    beleg_id: String,
    datei_ref: String,
    name: String,
) -> Result<(), String> {
    let pfad = beleg_ordner(&app, &projekt, &beleg_id)?.join(pruefe_ref(&datei_ref)?);
    let roh = fs::read(&pfad).map_err(|e| format!("Beleg-Datei nicht lesbar: {e}"))?;
    let klar = crate::tresor::datei_entschluesseln(&state, &roh)?;

    let temp = std::env::temp_dir().join("Antrag3000-Belege");
    fs::create_dir_all(&temp).map_err(|e| format!("Temp-Ordner nicht anlegbar: {e}"))?;
    let sicher_name = ordner::bereinigen(&name).unwrap_or_else(|_| "Beleg".into());
    let ziel = temp.join(&sicher_name);
    fs::write(&ziel, &klar).map_err(|e| format!("Temp-Datei nicht schreibbar: {e}"))?;

    tauri_plugin_opener::open_path(ziel, None::<&str>)
        .map_err(|e| format!("Datei laesst sich nicht oeffnen: {e}"))?;
    Ok(())
}

/// Eine Beleg-Datei entschluesselt an einen selbst gewaehlten Ort
/// speichern („herunterladen"). Das Ziel kommt aus dem Speichern-Dialog
/// des Frontends; ab da liegt die Datei dort im Klartext (bewusste,
/// vom Nutzer gewaehlte Ausgabe – wie beim Excel/Word-Export).
#[tauri::command]
pub fn beleg_datei_exportieren(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    projekt: String,
    beleg_id: String,
    datei_ref: String,
    ziel: String,
) -> Result<(), String> {
    let pfad = beleg_ordner(&app, &projekt, &beleg_id)?.join(pruefe_ref(&datei_ref)?);
    let roh = fs::read(&pfad).map_err(|e| format!("Beleg-Datei nicht lesbar: {e}"))?;
    let klar = crate::tresor::datei_entschluesseln(&state, &roh)?;
    fs::write(&ziel, &klar).map_err(|e| format!("Datei nicht speicherbar: {e}"))?;
    Ok(())
}

/// Eine einzelne Beleg-Datei loeschen.
#[tauri::command]
pub fn beleg_datei_entfernen(
    app: tauri::AppHandle,
    projekt: String,
    beleg_id: String,
    datei_ref: String,
) -> Result<(), String> {
    let pfad = beleg_ordner(&app, &projekt, &beleg_id)?.join(pruefe_ref(&datei_ref)?);
    if pfad.exists() {
        fs::remove_file(&pfad).map_err(|e| format!("Datei nicht loeschbar: {e}"))?;
    }
    Ok(())
}

/// Den ganzen Datei-Ordner eines Belegs loeschen (beim Loeschen des Belegs).
#[tauri::command]
pub fn beleg_ordner_entfernen(
    app: tauri::AppHandle,
    projekt: String,
    beleg_id: String,
) -> Result<(), String> {
    let pfad = beleg_ordner(&app, &projekt, &beleg_id)?;
    if pfad.exists() {
        fs::remove_dir_all(&pfad).map_err(|e| format!("Beleg-Ordner nicht loeschbar: {e}"))?;
    }
    Ok(())
}
