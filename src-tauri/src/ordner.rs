// ============================================================
// Ordnerverwaltung der Nutzdaten (CLAUDE.md):
//   [Dokumente]\Antrag 3000\[Projektname]\[Foerderungsname]\
//
// Der Rust-Kern uebernimmt nur die Systemzugriffe: Ordner anlegen,
// im Windows-Explorer oeffnen, bei Projekt-Umbenennung mitziehen.
// Welche Ordner wann gebraucht werden, entscheidet das Frontend.
// ============================================================

use std::fs;
use std::path::PathBuf;

use tauri::Manager;

/// Wurzelordner aller Nutzdaten: Dokumente\Antrag 3000
fn wurzel(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dokumente = app
        .path()
        .document_dir()
        .map_err(|e| format!("Dokumente-Ordner nicht ermittelbar: {e}"))?;
    Ok(dokumente.join("Antrag 3000"))
}

/// Macht aus einem freien Namen einen gueltigen Windows-Ordnernamen:
/// verbotene Zeichen werden durch _ ersetzt, Leerraum und Punkte am
/// Rand entfernt (Windows erlaubt sie dort nicht).
fn bereinigen(name: &str) -> Result<String, String> {
    const VERBOTEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let ersetzt: String = name
        .chars()
        .map(|c| {
            if VERBOTEN.contains(&c) || (c as u32) < 32 {
                '_'
            } else {
                c
            }
        })
        .collect();
    let sauber = ersetzt
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_string();
    if sauber.is_empty() {
        return Err("Der Name ergibt keinen gueltigen Ordnernamen.".into());
    }
    Ok(sauber)
}

/// Legt den Ordner (Projekt, optional mit Foerderungs-Unterordner)
/// bei Bedarf an und oeffnet ihn im Windows-Explorer.
#[tauri::command]
pub fn ordner_oeffnen(
    app: tauri::AppHandle,
    projekt: String,
    foerderung: Option<String>,
) -> Result<String, String> {
    let mut pfad = wurzel(&app)?.join(bereinigen(&projekt)?);
    if let Some(f) = foerderung {
        pfad = pfad.join(bereinigen(&f)?);
    }
    fs::create_dir_all(&pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    tauri_plugin_opener::open_path(pfad.clone(), None::<&str>)
        .map_err(|e| format!("Ordner laesst sich nicht oeffnen: {e}"))?;
    Ok(pfad.to_string_lossy().to_string())
}

/// Zieht den Projektordner bei einer Umbenennung mit um.
/// Existiert der alte Ordner nicht, ist nichts zu tun.
#[tauri::command]
pub fn ordner_umbenennen(
    app: tauri::AppHandle,
    alt: String,
    neu: String,
) -> Result<(), String> {
    let w = wurzel(&app)?;
    let alter_pfad = w.join(bereinigen(&alt)?);
    let neuer_pfad = w.join(bereinigen(&neu)?);
    if alter_pfad == neuer_pfad || !alter_pfad.exists() {
        return Ok(());
    }
    if neuer_pfad.exists() {
        return Err(format!(
            "Es existiert bereits ein Ordner namens \"{}\".",
            neuer_pfad.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    fs::rename(&alter_pfad, &neuer_pfad).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))
}
