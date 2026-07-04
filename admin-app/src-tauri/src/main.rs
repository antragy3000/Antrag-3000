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
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

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
    /// Pfad der gewaehlten .a3kpaket-Datei. Das Frontend merkt sich nur
    /// diesen Pfad (nicht den Schluessel), um das Paket beim naechsten Start
    /// automatisch zu laden.
    #[serde(default)]
    pub pfad: String,
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
        pfad: String::new(),
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
    let mut z = paket_aus_text(&roh)?;
    z.pfad = pfad.to_string_lossy().to_string();
    Ok(Some(z))
}

/// Laedt ein .a3kpaket direkt von einem bekannten Pfad (ohne Dialog), damit
/// das zuletzt genutzte Paket beim Start automatisch geladen werden kann.
/// Fehler, wenn die Datei fehlt/verschoben wurde – das Frontend faellt dann
/// auf die manuelle Auswahl zurueck.
#[tauri::command]
fn paket_laden(pfad: String) -> Result<ZugangsInfo, String> {
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let mut z = paket_aus_text(&roh)?;
    z.pfad = pfad;
    Ok(z)
}

// --- Zuletzt genutztes Paket merken (nur der Pfad, in einer kleinen Datei
//     im App-Konfigordner – zuverlaessiger als localStorage des Webviews) ---

fn merk_datei(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Konfig-Ordner nicht ermittelbar: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Konfig-Ordner nicht anlegbar: {e}"))?;
    Ok(dir.join("letztes_paket.txt"))
}

/// Merkt sich den Pfad des zuletzt genutzten .a3kpaket (nur den Pfad).
#[tauri::command]
fn pfad_merken(app: tauri::AppHandle, pfad: String) -> Result<(), String> {
    std::fs::write(merk_datei(&app)?, pfad).map_err(|e| format!("Konnte Pfad nicht merken: {e}"))
}

/// Gibt den gemerkten Pfad zurueck ("" wenn keiner gemerkt ist).
#[tauri::command]
fn pfad_gemerkt(app: tauri::AppHandle) -> String {
    merk_datei(&app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Loescht den gemerkten Pfad (z. B. wenn die Datei verschoben wurde).
#[tauri::command]
fn pfad_vergessen(app: tauri::AppHandle) -> Result<(), String> {
    if let Ok(p) = merk_datei(&app) {
        let _ = std::fs::remove_file(p);
    }
    Ok(())
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

/// Öffnet eine Web-Adresse im Standard-Browser (zum Prüfen einer Förderung
/// aus einer Meldung/einem Vorschlag heraus). Nur http/https werden geöffnet,
/// damit keine anderen Protokolle (z. B. file:) untergeschoben werden können.
#[tauri::command]
fn webseite_oeffnen(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let u = url.trim();
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        return Err("Nur Web-Adressen (http/https) werden geöffnet.".into());
    }
    app.opener()
        .open_url(u, None::<&str>)
        .map_err(|e| format!("Konnte die Webseite nicht öffnen: {e}"))
}

/// Holt den aktuell verteilten Gesamt-Katalog (GET /api/katalog). Braucht
/// nur das Geräte-Zertifikat (mTLS), kein Admin-Token – wir schicken es
/// trotzdem mit, schadet nicht. Wird gebraucht, um Vorschläge gegen den
/// Ist-Stand zu vergleichen (Diff) und um beim Bearbeiten/Übernehmen den
/// Katalog im Client zu ändern und als Ganzes wieder hochzuladen.
#[tauri::command]
async fn admin_katalog_holen(adresse: String, ausweis_pem: String, ca_pem: String, token: String) -> Result<String, String> {
    admin_get(&adresse, &ausweis_pem, &ca_pem, &token, "/api/katalog").await
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

// ============================================================
// Förderer-Aktivierung (Signatur-Herkunft) – Ausstellen.
//
// Die Admin-App führt eine EIGENE "Förderer-CA" (getrennt von der Team-CA).
// Damit stellt sie pro Förderer eine Aktivierungs-Datei aus: Schlüsselpaar +
// ein von der Förderer-CA signiertes Zertifikat (CN = Förderername). Die
// Förderer-App signiert damit später ihre Exporte -> jede Förderung ist
// fälschungssicher diesem Förderer zuordenbar.
//
// Der CA-Privatschlüssel bleibt in einer Datei (vom Admin sicher verwahrt) und
// verlässt den Rust-Teil NIE Richtung Weboberfläche; die App merkt sich nur
// den PFAD zur CA-Datei (im Konfig-Ordner).
// ============================================================

#[derive(Serialize, Deserialize)]
struct FoerdererCa {
    #[serde(default)]
    typ: String,
    cert_pem: String,
    key_pem: String,
}

fn epoch_sekunden() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Kleine Merk-Datei: speichert NUR den Pfad zur Förderer-CA-Datei.
fn ca_merk_datei(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Konfig-Ordner nicht ermittelbar: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Konfig-Ordner nicht anlegbar: {e}"))?;
    Ok(dir.join("foerderer_ca_pfad.txt"))
}

/// Liest die Förderer-CA aus der gemerkten Datei.
fn ca_laden(app: &tauri::AppHandle) -> Result<FoerdererCa, String> {
    let merk = ca_merk_datei(app)?;
    let pfad = std::fs::read_to_string(&merk)
        .map_err(|_| "Es ist noch keine Förderer-CA hinterlegt.".to_string())?;
    let pfad = pfad.trim();
    if pfad.is_empty() {
        return Err("Es ist noch keine Förderer-CA hinterlegt.".into());
    }
    let roh = std::fs::read_to_string(pfad).map_err(|e| format!("CA-Datei nicht lesbar: {e}"))?;
    let ca: FoerdererCa = serde_json::from_str(&roh).map_err(|e| format!("CA-Datei ungültig: {e}"))?;
    if ca.cert_pem.is_empty() || ca.key_pem.is_empty() {
        return Err("CA-Datei unvollständig.".into());
    }
    Ok(ca)
}

/// Erstellt eine neue Förderer-CA und speichert sie über einen Dialog.
#[tauri::command]
fn foerderer_ca_erstellen(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let key = rcgen::KeyPair::generate().map_err(|e| format!("Schlüssel nicht erzeugbar: {e}"))?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
        .map_err(|e| format!("CA-Parameter fehlerhaft: {e}"))?;
    p.distinguished_name
        .push(rcgen::DnType::OrganizationName, "Antrag 3000 Team");
    p.distinguished_name
        .push(rcgen::DnType::CommonName, "Antrag 3000 Förderer-CA");
    p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
    p.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
    ];
    p.not_before = rcgen::date_time_ymd(2024, 1, 1);
    p.not_after = rcgen::date_time_ymd(2035, 1, 1);
    let cert = p.self_signed(&key).map_err(|e| format!("CA nicht signierbar: {e}"))?;
    let ca = FoerdererCa {
        typ: "antrag3000-foerderer-ca".into(),
        cert_pem: cert.pem(),
        key_pem: key.serialize_pem(),
    };
    let inhalt =
        serde_json::to_string_pretty(&ca).map_err(|e| format!("CA nicht serialisierbar: {e}"))?;

    let datei = app
        .dialog()
        .file()
        .set_file_name("antrag3000-foerderer-ca.json")
        .add_filter("Förderer-CA", &["json"])
        .blocking_save_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    std::fs::write(&pfad, inhalt).map_err(|e| format!("CA nicht speicherbar: {e}"))?;
    let s = pfad.to_string_lossy().to_string();
    let _ = std::fs::write(ca_merk_datei(&app)?, &s);
    Ok(Some(s))
}

/// Wählt eine bestehende Förderer-CA-Datei und merkt sich den Pfad.
#[tauri::command]
fn foerderer_ca_waehlen(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let datei = app
        .dialog()
        .file()
        .add_filter("Förderer-CA", &["json"])
        .blocking_pick_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let ca: FoerdererCa =
        serde_json::from_str(&roh).map_err(|e| format!("Keine gültige CA-Datei: {e}"))?;
    if ca.cert_pem.is_empty() || ca.key_pem.is_empty() {
        return Err("CA-Datei unvollständig.".into());
    }
    let s = pfad.to_string_lossy().to_string();
    let _ = std::fs::write(ca_merk_datei(&app)?, &s);
    Ok(Some(s))
}

/// Gibt den gemerkten CA-Pfad zurück ("" wenn keiner/ungültig).
#[tauri::command]
fn foerderer_ca_pfad(app: tauri::AppHandle) -> String {
    ca_merk_datei(&app)
        .ok()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && std::path::Path::new(s).exists())
        .unwrap_or_default()
}

/// Stellt eine Aktivierungs-Datei für einen Förderer aus (von der Förderer-CA
/// signiertes Zertifikat + Schlüsselpaar).
#[tauri::command]
fn foerderer_aktivierung_erstellen(
    app: tauri::AppHandle,
    foerderer_name: String,
) -> Result<Option<String>, String> {
    let name = foerderer_name.trim().to_string();
    if name.is_empty() {
        return Err("Bitte einen Förderer-Namen angeben.".into());
    }
    let ca = ca_laden(&app)?;

    let ca_key =
        rcgen::KeyPair::from_pem(&ca.key_pem).map_err(|e| format!("CA-Schlüssel unlesbar: {e}"))?;
    let ca_cert = rcgen::CertificateParams::from_ca_cert_pem(&ca.cert_pem)
        .map_err(|e| format!("CA-Zertifikat unlesbar: {e}"))?
        .self_signed(&ca_key)
        .map_err(|e| format!("CA nicht rekonstruierbar: {e}"))?;

    let f_key = rcgen::KeyPair::generate().map_err(|e| format!("Schlüssel nicht erzeugbar: {e}"))?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
        .map_err(|e| format!("Parameter fehlerhaft: {e}"))?;
    p.distinguished_name
        .push(rcgen::DnType::OrganizationName, "Antrag 3000 Förderer");
    p.distinguished_name
        .push(rcgen::DnType::CommonName, name.as_str());
    p.is_ca = rcgen::IsCa::NoCa;
    p.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    p.not_before = rcgen::date_time_ymd(2024, 1, 1);
    p.not_after = rcgen::date_time_ymd(2035, 1, 1);
    let f_cert = p
        .signed_by(&f_key, &ca_cert, &ca_key)
        .map_err(|e| format!("Zertifikat nicht signierbar: {e}"))?;

    let paket = serde_json::json!({
        "typ": "antrag3000-foerderer-aktivierung",
        "version": 1,
        "foerderer_name": name,
        "ausgestellt_am": epoch_sekunden(),
        "foerderer_key_pem": f_key.serialize_pem(),
        "foerderer_cert_pem": f_cert.pem(),
        "ca_cert_pem": ca.cert_pem,
    });
    let inhalt =
        serde_json::to_string_pretty(&paket).map_err(|e| format!("Nicht serialisierbar: {e}"))?;

    let sicher: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let vorschlag = format!("antrag3000-aktivierung-{sicher}.json");
    let datei = app
        .dialog()
        .file()
        .set_file_name(&vorschlag)
        .add_filter("Förderer-Aktivierung", &["json"])
        .blocking_save_file();
    let Some(fp) = datei else { return Ok(None) };
    let pfad = fp.into_path().map_err(|e| format!("Pfad ungültig: {e}"))?;
    std::fs::write(&pfad, inhalt).map_err(|e| format!("Datei nicht speicherbar: {e}"))?;
    Ok(Some(pfad.to_string_lossy().to_string()))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            paket_waehlen,
            paket_laden,
            pfad_merken,
            pfad_gemerkt,
            pfad_vergessen,
            katalog_waehlen,
            admin_anmelden,
            admin_meldungen,
            admin_foerderer,
            admin_vorschlaege,
            admin_katalog_holen,
            webseite_oeffnen,
            admin_meldung_status,
            admin_foerderer_loeschen,
            admin_vorschlag_freigeben,
            admin_vorschlag_verwerfen,
            admin_katalog_hochladen,
            foerderer_ca_erstellen,
            foerderer_ca_waehlen,
            foerderer_ca_pfad,
            foerderer_aktivierung_erstellen,
        ])
        .run(tauri::generate_context!())
        .expect("Fehler beim Start der Admin-App");
}
