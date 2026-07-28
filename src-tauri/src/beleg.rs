// ============================================================
// Abrechnungs-Modus: Beleg-Dateien (Fotos/Scans/PDF).
//
// Belege werden als LESBARE Dateien im Projektordner abgelegt:
//   [Dokumente]\Antrag 3000\[Projekt]\Belege\[Empfaenger-Zweck_Datum_KS].pdf
// So kann man den Ordner im Explorer oeffnen, die Belege ansehen, drucken
// und (z. B. fuer die Pruefung) weitergeben. Das ist eine bewusste,
// vom Nutzer gewaehlte Ablage – wie die Checklisten-Dokumente beim Antrag
// und der KFP-Excel-Export: die Dateien bleiben rein lokal und verlassen
// das Geraet NIE ueber eine Netzwerkverbindung (die Sync-Ebene fasst den
// Projektordner nicht an). Die hochsensiblen Beleg-ANGABEN (Betraege,
// Lieferanten, Kostenstellen) liegen weiter verschluesselt im Tresor; hier
// geht es nur um die dazugehoerige Bild-/PDF-Datei.
//
// Der Rust-Kern macht nur die Systemarbeit (Datei kopieren, oeffnen,
// loeschen, Ordner oeffnen). Welche Datei zu welchem Beleg gehoert, merkt
// sich das Frontend im Tresor (nur der Dateiname).
// ============================================================

use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::ordner;

const ERLAUBT: [&str; 4] = ["pdf", "jpg", "jpeg", "png"];
const MAX_BYTES: u64 = 30 * 1024 * 1024; // 30 MB je Datei

/// Verweis auf eine gespeicherte Beleg-Datei (das merkt sich das Frontend
/// im Tresor). Der Dateiname ist zugleich die Kennung im Belegordner.
#[derive(Serialize)]
pub struct BelegDatei {
    pub name: String,
    pub ext: String,
    pub groesse: u64,
}

/// Der (lesbare) Belegordner EINES Projekts.
fn belegordner(app: &tauri::AppHandle, projekt: &str) -> Result<PathBuf, String> {
    Ok(ordner::wurzel(app)?
        .join(ordner::bereinigen(projekt)?)
        .join("Belege"))
}

/// Schuetzt vor Pfad-Tricks im Dateinamen (er kommt aus dem Tresor, wir
/// pruefen aber trotzdem): kein Verzeichniswechsel erlaubt.
fn pruefe_name(n: &str) -> Result<&str, String> {
    if n.is_empty() || n.contains('/') || n.contains('\\') || n.contains("..") {
        return Err("Ungueltiger Dateiname.".into());
    }
    Ok(n)
}

/// Findet einen freien Dateinamen im Ordner: gibt es "Name.pdf" schon,
/// wird "Name (2).pdf", "Name (3).pdf" usw. probiert. So ueberschreibt ein
/// zweiter Beleg mit gleichem Empfaenger/Zweck/Datum nie den ersten.
fn freier_name(ordner_pfad: &std::path::Path, basis: &str, ext: &str) -> String {
    let voll = |name: &str| {
        if ext.is_empty() {
            name.to_string()
        } else {
            format!("{name}.{ext}")
        }
    };
    let erster = voll(basis);
    if !ordner_pfad.join(&erster).exists() {
        return erster;
    }
    for i in 2..1000 {
        let kandidat = voll(&format!("{basis} ({i})"));
        if !ordner_pfad.join(&kandidat).exists() {
            return kandidat;
        }
    }
    // Fallback (praktisch unerreichbar): eindeutig ueber Zeitstempel.
    voll(&format!(
        "{basis} ({})",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ))
}

/// Eine gewaehlte Datei lesbar benannt in den Belegordner kopieren. `wunschname`
/// ist der vom Frontend gebaute Name OHNE Endung (Empfaenger-Zweck_Datum_KS);
/// die Endung uebernimmt der Kern von der Quelldatei. Gibt den tatsaechlich
/// vergebenen Dateinamen zurueck, den das Frontend im Beleg speichert.
#[tauri::command]
pub fn beleg_datei_hinzufuegen(
    app: tauri::AppHandle,
    projekt: String,
    quelle: String,
    wunschname: String,
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

    let ordner_pfad = belegordner(&app, &projekt)?;
    fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Belegordner nicht anlegbar: {e}"))?;

    // Wunschnamen zu einem gueltigen Dateinamen bereinigen (verbotene Zeichen
    // -> _), leere Eingabe faellt auf "Beleg" zurueck.
    let basis = ordner::bereinigen(&wunschname).unwrap_or_else(|_| "Beleg".into());
    let name = freier_name(&ordner_pfad, &basis, &ext);

    let ziel = ordner_pfad.join(&name);
    fs::copy(quell_pfad, &ziel).map_err(|e| format!("Datei nicht kopierbar: {e}"))?;

    Ok(BelegDatei { name, ext, groesse: meta.len() })
}

/// Eine Beleg-Datei im System-Betrachter oeffnen.
#[tauri::command]
pub fn beleg_datei_oeffnen(
    app: tauri::AppHandle,
    projekt: String,
    name: String,
) -> Result<(), String> {
    let pfad = belegordner(&app, &projekt)?.join(pruefe_name(&name)?);
    if !pfad.exists() {
        return Err("Die Datei wurde im Belegordner nicht gefunden.".into());
    }
    tauri_plugin_opener::open_path(pfad, None::<&str>)
        .map_err(|e| format!("Datei laesst sich nicht oeffnen: {e}"))?;
    Ok(())
}

/// Eine einzelne Beleg-Datei loeschen.
#[tauri::command]
pub fn beleg_datei_entfernen(
    app: tauri::AppHandle,
    projekt: String,
    name: String,
) -> Result<(), String> {
    let pfad = belegordner(&app, &projekt)?.join(pruefe_name(&name)?);
    if pfad.exists() {
        fs::remove_file(&pfad).map_err(|e| format!("Datei nicht loeschbar: {e}"))?;
    }
    Ok(())
}

/// Den Belegordner des Projekts im Explorer oeffnen (legt ihn bei Bedarf an).
#[tauri::command]
pub fn beleg_ordner_oeffnen(app: tauri::AppHandle, projekt: String) -> Result<String, String> {
    let pfad = belegordner(&app, &projekt)?;
    fs::create_dir_all(&pfad).map_err(|e| format!("Belegordner nicht anlegbar: {e}"))?;
    tauri_plugin_opener::open_path(pfad.clone(), None::<&str>)
        .map_err(|e| format!("Belegordner laesst sich nicht oeffnen: {e}"))?;
    Ok(pfad.to_string_lossy().to_string())
}
