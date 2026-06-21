// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// ============================================================
// Antrag 3000 – Admin-App (Phase 3 / Etappe 4).
//
// Eine schlanke, EIGENE Anwendung (getrennt von der Nutzer-App) zum
// zentralen Pflegen der Förder-Datenbank: Meldungen sichten, geteilte
// Förderer einsehen, den Katalog hochladen.
//
// Sie hält KEINE sensiblen Tresor-Daten. Zur Anmeldung lädt sie ein
// vorhandenes Zugangs-Paket (.a3kpaket, dieselbe Datei wie die Nutzer-
// App) – das liefert den Geräte-Ausweis (mTLS) und die Team-Adresse.
// Zweiter Faktor ist ein TOTP-Code (Authenticator-App); der Server gibt
// dafür ein kurzlebiges Sitzungs-Token zurück, das die App mitschickt.
//
// Der mTLS-HTTP-Client ist (wie in der Nutzer-App) eine Rust-Aufgabe.
// ============================================================

use serde::{Deserialize, Serialize};
use tauri_plugin_dialog::DialogExt;

// --- Zugangs-Paket (.a3kpaket) lesen -----------------------------------

#[derive(Deserialize)]
struct Paket {
    #[serde(default)]
    typ: String,
    #[serde(default)]
    adresse: String,
    #[serde(default)]
    ausweis_pem: String,
    #[serde(default)]
    ca_pem: String,
}

#[derive(Serialize, Clone)]
pub struct ZugangsInfo {
    pub adresse: String,
    pub geraet_name: String,
    pub ausweis_pem: String,
    pub ca_pem: String,
}

/// Liest den Common Name (Gerätenamen) aus dem ersten Zertifikat im PEM.
fn geraet_name_aus_pem(pem: &str) -> Result<String, String> {
    let mut leser = std::io::BufReader::new(pem.as_bytes());
    let certs: Vec<_> = rustls_pemfile::certs(&mut leser)
        .collect::<Result<_, _>>()
        .map_err(|_| "Zertifikat im Paket nicht lesbar.".to_string())?;
    let cert_der = certs.first().ok_or("Im Paket ist kein Zertifikat.")?;
    let (_, cert) = x509_parser::parse_x509_certificate(cert_der.as_ref())
        .map_err(|_| "Zertifikat nicht entzifferbar.".to_string())?;
    let name = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|a| a.as_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unbenanntes Gerät".to_string());
    Ok(name)
}

/// Wandelt den Roh-Text eines .a3kpaket in geprüfte Zugangs-Infos um.
fn paket_aus_text(roh: &str) -> Result<ZugangsInfo, String> {
    let paket: Paket =
        serde_json::from_str(roh).map_err(|_| "Das ist kein gültiges Zugangs-Paket.".to_string())?;
    if paket.typ != "antrag3000-zugangspaket" {
        return Err("Das ist kein Antrag-3000-Zugangs-Paket.".into());
    }
    let adresse = paket.adresse.trim().to_string();
    if adresse.is_empty() {
        return Err("Im Paket fehlt die Team-Adresse.".into());
    }
    let geraet_name = geraet_name_aus_pem(&paket.ausweis_pem)?;
    // Privater Schlüssel muss vorhanden sein.
    let mut leser = std::io::BufReader::new(paket.ausweis_pem.as_bytes());
    let hat_key = rustls_pemfile::private_key(&mut leser)
        .map_err(|_| "Privater Schlüssel im Paket nicht lesbar.".to_string())?
        .is_some();
    if !hat_key {
        return Err("Im Paket fehlt der private Schlüssel.".into());
    }
    Ok(ZugangsInfo {
        adresse,
        geraet_name,
        ausweis_pem: paket.ausweis_pem,
        ca_pem: paket.ca_pem,
    })
}

/// Öffnet einen Datei-Dialog, lässt ein .a3kpaket wählen und liest es.
/// None = Auswahl abgebrochen.
#[tauri::command]
fn paket_waehlen(app: tauri::AppHandle) -> Result<Option<ZugangsInfo>, String> {
    let datei = app
        .dialog()
        .file()
        .add_filter("Zugangs-Paket", &["a3kpaket"])
        .blocking_pick_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    Ok(Some(paket_aus_text(&roh)?))
}

/// Öffnet einen Datei-Dialog für eine Katalog-Datei (JSON) und gibt ihren
/// Inhalt als Text zurück. None = abgebrochen.
#[tauri::command]
fn katalog_waehlen(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let datei = app
        .dialog()
        .file()
        .add_filter("Katalog (JSON)", &["json"])
        .blocking_pick_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    std::fs::read_to_string(&pfad).map(Some).map_err(|e| format!("Datei nicht lesbar: {e}"))
}

// --- mTLS-HTTP ----------------------------------------------------------

fn client_mit_ausweis(ausweis_pem: &str, ca_pem: &str) -> Result<reqwest::Client, String> {
    let id = reqwest::Identity::from_pem(ausweis_pem.as_bytes())
        .map_err(|e| format!("Ausweis ungültig: {e}"))?;
    let mut builder = reqwest::Client::builder()
        .identity(id)
        .timeout(std::time::Duration::from_secs(15));
    let ca = ca_pem.trim();
    if !ca.is_empty() {
        let root = reqwest::Certificate::from_pem(ca.as_bytes())
            .map_err(|e| format!("Team-CA-Zertifikat ungültig: {e}"))?;
        builder = builder.add_root_certificate(root);
    }
    builder
        .build()
        .map_err(|e| format!("Verbindungs-Client nicht erstellbar: {e}"))
}

/// Hängt die Ursachen-Kette eines Fehlers an (reqwest versteckt den
/// eigentlichen TLS-Grund in der `source`).
fn fehler_kette(e: &dyn std::error::Error) -> String {
    let mut s = e.to_string();
    let mut cur = e.source();
    while let Some(inner) = cur {
        s.push_str(" → ");
        s.push_str(&inner.to_string());
        cur = inner.source();
    }
    s
}

fn basis_url(adresse: &str) -> String {
    let a = adresse.trim().trim_end_matches('/');
    if a.starts_with("http://") || a.starts_with("https://") {
        a.to_string()
    } else {
        format!("https://{a}")
    }
}

/// Admin-Anmeldung: schickt den TOTP-Code, bekommt ein Sitzungs-Token.
#[tauri::command]
async fn admin_anmelden(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    code: String,
) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/admin/anmelden", basis_url(&adresse));
    let r = client
        .post(&url)
        .json(&serde_json::json!({ "code": code.trim() }))
        .send()
        .await
        .map_err(|e| format!("Anmeldung fehlgeschlagen: {}", fehler_kette(&e)))?;
    match r.status().as_u16() {
        401 => return Err("Code falsch oder abgelaufen.".into()),
        429 => return Err("Zu viele Versuche – einen Moment warten.".into()),
        s if !(200..300).contains(&s) => return Err(format!("Server antwortete mit {s}.")),
        _ => {}
    }
    let v: serde_json::Value = r.json().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))?;
    v.get("token")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Kein Sitzungs-Token erhalten.".into())
}

/// Gemeinsamer GET-Aufruf für die Admin-Lese-Endpunkte.
async fn admin_get(
    adresse: &str,
    ausweis_pem: &str,
    ca_pem: &str,
    token: &str,
    pfad: &str,
) -> Result<String, String> {
    let client = client_mit_ausweis(ausweis_pem, ca_pem)?;
    let url = format!("{}{}", basis_url(adresse), pfad);
    let r = client
        .get(&url)
        .header("x-admin-token", token)
        .send()
        .await
        .map_err(|e| format!("Abruf fehlgeschlagen: {}", fehler_kette(&e)))?;
    if r.status().as_u16() == 401 {
        return Err("Sitzung abgelaufen – bitte neu anmelden.".into());
    }
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}.", r.status()));
    }
    r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))
}

#[tauri::command]
async fn admin_meldungen(adresse: String, ausweis_pem: String, ca_pem: String, token: String) -> Result<String, String> {
    admin_get(&adresse, &ausweis_pem, &ca_pem, &token, "/api/admin/meldungen").await
}

#[tauri::command]
async fn admin_foerderer(adresse: String, ausweis_pem: String, ca_pem: String, token: String) -> Result<String, String> {
    admin_get(&adresse, &ausweis_pem, &ca_pem, &token, "/api/admin/foerderer").await
}

#[tauri::command]
async fn admin_vorschlaege(adresse: String, ausweis_pem: String, ca_pem: String, token: String) -> Result<String, String> {
    admin_get(&adresse, &ausweis_pem, &ca_pem, &token, "/api/admin/vorschlaege").await
}

/// Gemeinsamer POST ohne Body für die Vorschlags-Aktionen.
async fn admin_post(adresse: &str, ausweis_pem: &str, ca_pem: &str, token: &str, pfad: &str) -> Result<(), String> {
    let client = client_mit_ausweis(ausweis_pem, ca_pem)?;
    let url = format!("{}{}", basis_url(adresse), pfad);
    let r = client
        .post(&url)
        .header("x-admin-token", token)
        .send()
        .await
        .map_err(|e| format!("Senden fehlgeschlagen: {}", fehler_kette(&e)))?;
    if r.status().as_u16() == 401 {
        return Err("Sitzung abgelaufen – bitte neu anmelden.".into());
    }
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}.", r.status()));
    }
    Ok(())
}

#[tauri::command]
async fn admin_vorschlag_freigeben(adresse: String, ausweis_pem: String, ca_pem: String, token: String, vorschlag_id: String) -> Result<(), String> {
    admin_post(&adresse, &ausweis_pem, &ca_pem, &token, &format!("/api/admin/vorschlaege/{vorschlag_id}/freigeben")).await
}

#[tauri::command]
async fn admin_vorschlag_verwerfen(adresse: String, ausweis_pem: String, ca_pem: String, token: String, vorschlag_id: String) -> Result<(), String> {
    admin_post(&adresse, &ausweis_pem, &ca_pem, &token, &format!("/api/admin/vorschlaege/{vorschlag_id}/verwerfen")).await
}

/// Setzt den Status einer Meldung (offen/erledigt/verworfen).
#[tauri::command]
async fn admin_meldung_status(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    token: String,
    meldung_id: String,
    status: String,
) -> Result<(), String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/admin/meldungen/{}", basis_url(&adresse), meldung_id);
    let r = client
        .put(&url)
        .header("x-admin-token", &token)
        .json(&serde_json::json!({ "status": status }))
        .send()
        .await
        .map_err(|e| format!("Senden fehlgeschlagen: {}", fehler_kette(&e)))?;
    if r.status().as_u16() == 401 {
        return Err("Sitzung abgelaufen – bitte neu anmelden.".into());
    }
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}.", r.status()));
    }
    Ok(())
}

/// Entfernt einen geteilten Förderer (Admin darf jeden Eintrag löschen).
#[tauri::command]
async fn admin_foerderer_loeschen(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    token: String,
    foerderer_id: String,
) -> Result<(), String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/admin/foerderer/{}", basis_url(&adresse), foerderer_id);
    let r = client
        .delete(&url)
        .header("x-admin-token", &token)
        .send()
        .await
        .map_err(|e| format!("Löschen fehlgeschlagen: {}", fehler_kette(&e)))?;
    if r.status().as_u16() == 401 {
        return Err("Sitzung abgelaufen – bitte neu anmelden.".into());
    }
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}.", r.status()));
    }
    Ok(())
}

/// Lädt einen neuen Gesamt-Katalog hoch (PUT /api/admin/katalog).
#[tauri::command]
async fn admin_katalog_hochladen(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    token: String,
    katalog_json: String,
) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/admin/katalog", basis_url(&adresse));
    let r = client
        .put(&url)
        .header("x-admin-token", &token)
        .header("content-type", "application/json")
        .body(katalog_json)
        .send()
        .await
        .map_err(|e| format!("Hochladen fehlgeschlagen: {}", fehler_kette(&e)))?;
    match r.status().as_u16() {
        401 => return Err("Sitzung abgelaufen – bitte neu anmelden.".into()),
        400 => return Err("Der Katalog ist ungültig (Schema/Felder prüfen).".into()),
        413 => return Err("Der Katalog ist zu groß.".into()),
        s if !(200..300).contains(&s) => return Err(format!("Server antwortete mit {s}.")),
        _ => {}
    }
    r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            paket_waehlen,
            katalog_waehlen,
            admin_anmelden,
            admin_meldungen,
            admin_foerderer,
            admin_vorschlaege,
            admin_meldung_status,
            admin_foerderer_loeschen,
            admin_vorschlag_freigeben,
            admin_vorschlag_verwerfen,
            admin_katalog_hochladen,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Start der Admin-App");
}
