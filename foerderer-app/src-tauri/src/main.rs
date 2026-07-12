// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ============================================================
// Antrag 3000 – Förderer-App (Rust-Kern).
//
// Bewusst schlank: Datei speichern (Export) + Signatur-Herkunft. Nach dem
// Laden einer Aktivierung (von der Admin-App ausgestellt) wird jeder Export
// mit dem Förderer-Schlüssel signiert -> die Förderung ist fälschungssicher
// diesem Förderer zuordenbar. Der Förderer-Privatschlüssel bleibt im Rust-
// Teil / in der Aktivierungs-Datei und geht NIE in die Weboberfläche; die
// App merkt sich nur den PFAD zur Aktivierungs-Datei.
// ============================================================

mod verbinden;

use base64::Engine as _;
use p256::ecdsa::{signature::Signer, Signature, SigningKey};
use p256::pkcs8::DecodePrivateKey;
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

/// Speichert den Export-Text über einen "Speichern unter"-Dialog.
/// Rückgabe: gewählter Pfad, oder None wenn abgebrochen.
#[tauri::command]
fn export_speichern(
    app: tauri::AppHandle,
    vorschlag_name: String,
    inhalt: String,
) -> Result<Option<String>, String> {
    let datei = app
        .dialog()
        .file()
        .set_file_name(&vorschlag_name)
        .add_filter("Antrag 3000 Förderer-Export", &["json"])
        .blocking_save_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    std::fs::write(&pfad, inhalt).map_err(|e| format!("Datei nicht speicherbar: {e}"))?;
    Ok(Some(pfad.to_string_lossy().to_string()))
}

// --- Aktivierung / Signatur-Herkunft ----------------------------------

#[derive(Deserialize)]
struct Aktivierung {
    #[serde(default)]
    typ: String,
    #[serde(default)]
    foerderer_name: String,
    #[serde(default)]
    foerderer_key_pem: String,
    #[serde(default)]
    foerderer_cert_pem: String,
    #[serde(default)]
    ca_cert_pem: String,
}

#[derive(Serialize)]
struct AktStatus {
    aktiv: bool,
    foerderer_name: String,
}

#[derive(Serialize)]
struct Signatur {
    algo: String,
    wert: String,
    foerderer_name: String,
    foerderer_cert_pem: String,
    ca_cert_pem: String,
}

/// Kleine Merk-Datei: speichert NUR den Pfad zur Aktivierungs-Datei.
fn akt_merk_datei(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Konfig-Ordner nicht ermittelbar: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Konfig-Ordner nicht anlegbar: {e}"))?;
    Ok(dir.join("aktivierung_pfad.txt"))
}

/// Liest die Aktivierung aus der gemerkten Datei.
fn aktivierung_lesen(app: &tauri::AppHandle) -> Result<Aktivierung, String> {
    let merk = akt_merk_datei(app)?;
    let pfad = std::fs::read_to_string(&merk)
        .map_err(|_| "Diese App ist noch nicht aktiviert.".to_string())?;
    let pfad = pfad.trim();
    if pfad.is_empty() {
        return Err("Diese App ist noch nicht aktiviert.".into());
    }
    let roh =
        std::fs::read_to_string(pfad).map_err(|e| format!("Aktivierungs-Datei nicht lesbar: {e}"))?;
    let a: Aktivierung =
        serde_json::from_str(&roh).map_err(|e| format!("Aktivierung ungültig: {e}"))?;
    if a.foerderer_key_pem.is_empty() || a.foerderer_cert_pem.is_empty() {
        return Err("Aktivierung unvollständig.".into());
    }
    Ok(a)
}

/// Wählt eine Aktivierungs-Datei, prüft sie und merkt sich den Pfad.
#[tauri::command]
fn aktivierung_waehlen(app: tauri::AppHandle) -> Result<Option<AktStatus>, String> {
    let datei = app
        .dialog()
        .file()
        .add_filter("Förderer-Aktivierung", &["json"])
        .blocking_pick_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let a: Aktivierung =
        serde_json::from_str(&roh).map_err(|e| format!("Keine gültige Aktivierung: {e}"))?;
    if a.typ != "antrag3000-foerderer-aktivierung" {
        return Err("Das ist keine Förderer-Aktivierungs-Datei.".into());
    }
    if a.foerderer_key_pem.is_empty() || a.foerderer_cert_pem.is_empty() {
        return Err("Aktivierung unvollständig.".into());
    }
    let name = a.foerderer_name.clone();
    let s = pfad.to_string_lossy().to_string();
    let _ = std::fs::write(akt_merk_datei(&app)?, &s);
    Ok(Some(AktStatus { aktiv: true, foerderer_name: name }))
}

/// Gibt zurück, ob die App aktiviert ist (+ Förderer-Name).
#[tauri::command]
fn aktivierung_status(app: tauri::AppHandle) -> AktStatus {
    match aktivierung_lesen(&app) {
        Ok(a) => AktStatus { aktiv: true, foerderer_name: a.foerderer_name },
        Err(_) => AktStatus { aktiv: false, foerderer_name: String::new() },
    }
}

/// Entfernt die Aktivierung (nur den gemerkten Pfad; die Datei selbst bleibt).
#[tauri::command]
fn aktivierung_entfernen(app: tauri::AppHandle) -> Result<(), String> {
    if let Ok(f) = akt_merk_datei(&app) {
        let _ = std::fs::remove_file(f);
    }
    Ok(())
}

/// Signiert den Export-Inhalt (JSON-Text der Nutzlast) mit dem Förderer-
/// Schlüssel (ECDSA P-256). Gibt Signatur + Zertifikate zurück.
#[tauri::command]
fn export_signieren(app: tauri::AppHandle, inhalt: String) -> Result<Signatur, String> {
    let a = aktivierung_lesen(&app)?;
    let sk = SigningKey::from_pkcs8_pem(&a.foerderer_key_pem)
        .map_err(|e| format!("Signatur-Schlüssel unlesbar: {e}"))?;
    let sig: Signature = sk.sign(inhalt.as_bytes());
    let wert = base64::engine::general_purpose::STANDARD.encode(sig.to_der().as_bytes());
    Ok(Signatur {
        algo: "ecdsa-p256-sha256".into(),
        wert,
        foerderer_name: a.foerderer_name,
        foerderer_cert_pem: a.foerderer_cert_pem,
        ca_cert_pem: a.ca_cert_pem,
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            export_speichern,
            aktivierung_waehlen,
            aktivierung_status,
            aktivierung_entfernen,
            export_signieren,
            // Gehostetes Modell (Roadmap 7): online verbinden + live syncen.
            verbinden::foerderer_einladung_lesen,
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
