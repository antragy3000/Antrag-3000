// ============================================================
// Förderer-App – gehostetes Modell (Roadmap 7): online verbinden + live syncen.
//
// Ersetzt das alte Datei-Export-Modell: Der Förderer erzeugt sein Schlüsselpaar
// LOKAL, löst eine Vendor-Einladung ein (Förderer-CA signiert kopiersicher) und
// pflegt seine Programme direkt über den mTLS-Kanal am Server. Der private
// Schlüssel entsteht hier und verlässt das Gerät nie.
//
// Speicherung: der Zugang (Ausweis + Adresse) liegt im App-Konfig-Ordner
// (zugang.json) – gleiche Posture wie die bisherige Aktivierungs-Datei (der
// Schlüssel lag schon immer als Datei auf der Platte). Eine spätere
// Verschlüsselung wäre ein eigener Härtungs-Schritt.
// ============================================================

use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Gespeicherter Zugang eines verbundenen Förderers.
#[derive(Serialize, Deserialize)]
struct Zugang {
    /// mTLS-Sync-Adresse (Team-Kanal, i. d. R. :8443).
    adresse: String,
    /// Privater Schlüssel + Zertifikat (lokal erzeugt, kopiersicher).
    ausweis_pem: String,
    /// Server-Trust-CA (prüft das Server-Zertifikat).
    ca_pem: String,
    foerderer_name: String,
}

fn zugang_datei(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Konfig-Ordner nicht ermittelbar: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Konfig-Ordner nicht anlegbar: {e}"))?;
    Ok(dir.join("zugang.json"))
}

fn zugang_lesen(app: &tauri::AppHandle) -> Result<Zugang, String> {
    let pfad = zugang_datei(app)?;
    let roh = std::fs::read_to_string(&pfad)
        .map_err(|_| "Diese App ist noch nicht verbunden.".to_string())?;
    let z: Zugang = serde_json::from_str(&roh).map_err(|e| format!("Zugang ungültig: {e}"))?;
    if z.ausweis_pem.is_empty() {
        return Err("Zugang unvollständig.".into());
    }
    Ok(z)
}

// --- Einladung lesen ---------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct FoerdererEinladung {
    #[serde(default)]
    pub typ: String,
    #[serde(default)]
    pub enroll_url: String,
    #[serde(default)]
    pub sync_adresse: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub ablauf: String,
    #[serde(default)]
    pub ca_pem: String,
    #[serde(default)]
    pub name: String,
}

/// Liest ein Förderer-Einladungs-Paket (vom Vendor ausgestellt).
#[tauri::command]
pub fn foerderer_einladung_lesen(pfad: String) -> Result<FoerdererEinladung, String> {
    if let Ok(meta) = std::fs::metadata(&pfad) {
        if meta.len() > 256 * 1024 {
            return Err("Die Datei ist zu groß für eine Einladung.".into());
        }
    }
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let e: FoerdererEinladung = serde_json::from_str(&roh)
        .map_err(|_| "Das ist keine gültige Förderer-Einladung.".to_string())?;
    if e.typ != "antrag3000-foerderer-einladung" {
        return Err("Das ist keine Antrag-3000-Förderer-Einladung.".into());
    }
    if e.token.trim().is_empty() {
        return Err("In der Einladung fehlt der Code.".into());
    }
    Ok(e)
}

// --- HTTP-Helfer (wie in der Nutzer-App) -------------------------------

fn basis_url(adresse: &str) -> String {
    let a = adresse.trim().trim_end_matches('/');
    if a.starts_with("http://") || a.starts_with("https://") {
        a.to_string()
    } else {
        format!("https://{a}")
    }
}

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

/// mTLS-Client mit Förderer-Ausweis; vertraut fürs Server-Zertifikat
/// AUSSCHLIESSLICH der mitgegebenen CA (öffentliche Wurzeln aus).
fn client_mit_ausweis(ausweis_pem: &str, ca_pem: &str) -> Result<reqwest::Client, String> {
    let id = reqwest::Identity::from_pem(ausweis_pem.as_bytes())
        .map_err(|e| format!("Ausweis ungültig: {e}"))?;
    let mut b = reqwest::Client::builder()
        .identity(id)
        .timeout(std::time::Duration::from_secs(12));
    let ca = ca_pem.trim();
    if !ca.is_empty() {
        let root = reqwest::Certificate::from_pem(ca.as_bytes())
            .map_err(|e| format!("CA-Zertifikat ungültig: {e}"))?;
        b = b.add_root_certificate(root).tls_built_in_root_certs(false);
    }
    b.build().map_err(|e| format!("Verbindungs-Client nicht erstellbar: {e}"))
}

/// Client für den ÖFFENTLICHEN Enroll-Kanal (noch kein Ausweis). Öffentliche
/// Wurzeln + optional zusätzlich die Einladungs-CA.
fn client_oeffentlich(ca_pem: &str) -> Result<reqwest::Client, String> {
    let mut b = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15));
    let ca = ca_pem.trim();
    if !ca.is_empty() {
        if let Ok(root) = reqwest::Certificate::from_pem(ca.as_bytes()) {
            b = b.add_root_certificate(root);
        }
    }
    b.build().map_err(|e| format!("Verbindungs-Client nicht erstellbar: {e}"))
}

// --- verbinden (Enrollment) --------------------------------------------

#[derive(Serialize)]
pub struct VerbindungStatus {
    pub verbunden: bool,
    pub foerderer_name: String,
    pub adresse: String,
}

/// Nimmt eine Einladung an: erzeugt das Schlüsselpaar lokal, löst den Token am
/// öffentlichen Enroll-Kanal ein, setzt den Ausweis zusammen und speichert den
/// Zugang. Der private Schlüssel verlässt das Gerät nie.
#[tauri::command]
pub async fn foerderer_verbinden(
    app: tauri::AppHandle,
    enroll_url: String,
    sync_adresse: String,
    ca_pem: String,
    token: String,
    foerderer_name: String,
) -> Result<VerbindungStatus, String> {
    let name = foerderer_name.trim();
    if name.is_empty() {
        return Err("Bitte einen Förderer-Namen angeben.".into());
    }
    if token.trim().is_empty() {
        return Err("Es fehlt der Einladungs-Code.".into());
    }

    let key = rcgen::KeyPair::generate().map_err(|e| format!("Schlüssel nicht erzeugbar: {e}"))?;
    let pubkey_pem = key.public_key_pem();

    let client = client_oeffentlich(&ca_pem)?;
    let url = format!("{}/api/foerderer-enroll", basis_url(&enroll_url));
    let body = serde_json::json!({
        "token": token.trim(),
        "pubkeyPem": pubkey_pem,
        "name": name,
    })
    .to_string();
    let r = client
        .post(&url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Verbinden fehlgeschlagen: {}", fehler_kette(&e)))?;
    let status = r.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 => "Der Einladungs-Code ist ungültig.".to_string(),
            409 => "Diese Einladung wurde bereits eingelöst.".to_string(),
            410 => "Der Einladungs-Code ist abgelaufen.".to_string(),
            503 => "Der Server kann derzeit keine Ausweise ausstellen.".to_string(),
            s => format!("Server antwortete mit {s}"),
        });
    }

    #[derive(Deserialize)]
    struct Ant {
        #[serde(default)]
        ausweis_pem: String,
    }
    let ant: Ant = r
        .json()
        .await
        .map_err(|e| format!("Antwort nicht lesbar: {e}"))?;
    if !ant.ausweis_pem.contains("CERTIFICATE") {
        return Err("Der Server hat keinen gültigen Ausweis geliefert.".into());
    }

    let ausweis_pem = format!("{}{}", key.serialize_pem(), ant.ausweis_pem);
    let z = Zugang {
        adresse: sync_adresse.trim().to_string(),
        ausweis_pem,
        ca_pem: ca_pem.trim().to_string(),
        foerderer_name: name.to_string(),
    };
    let inhalt =
        serde_json::to_string_pretty(&z).map_err(|e| format!("Zugang nicht serialisierbar: {e}"))?;
    std::fs::write(zugang_datei(&app)?, inhalt)
        .map_err(|e| format!("Zugang nicht speicherbar: {e}"))?;

    Ok(VerbindungStatus {
        verbunden: true,
        foerderer_name: z.foerderer_name,
        adresse: z.adresse,
    })
}

/// Gibt zurück, ob die App verbunden ist (+ Name/Adresse).
#[tauri::command]
pub fn verbindung_status(app: tauri::AppHandle) -> VerbindungStatus {
    match zugang_lesen(&app) {
        Ok(z) => VerbindungStatus {
            verbunden: true,
            foerderer_name: z.foerderer_name,
            adresse: z.adresse,
        },
        Err(_) => VerbindungStatus {
            verbunden: false,
            foerderer_name: String::new(),
            adresse: String::new(),
        },
    }
}

/// Trennt die Verbindung (entfernt den gespeicherten Zugang lokal).
#[tauri::command]
pub fn verbindung_trennen(app: tauri::AppHandle) -> Result<(), String> {
    if let Ok(f) = zugang_datei(&app) {
        let _ = std::fs::remove_file(f);
    }
    Ok(())
}

// --- Programme live syncen ---------------------------------------------

/// Holt die eigenen Programme vom Server (mTLS GET /api/foerderer-programme).
#[tauri::command]
pub async fn programme_holen(app: tauri::AppHandle) -> Result<String, String> {
    let z = zugang_lesen(&app)?;
    let client = client_mit_ausweis(&z.ausweis_pem, &z.ca_pem)?;
    let url = format!("{}/api/foerderer-programme", basis_url(&z.adresse));
    let r = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Abruf fehlgeschlagen: {}", fehler_kette(&e)))?;
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))
}

/// Legt ein Programm an/aktualisiert es (mTLS PUT /api/foerderer-programme/{id}).
/// `inhalt_json` ist der Programm-Datensatz (JSON-Objekt mit mind. "name").
#[tauri::command]
pub async fn programm_senden(
    app: tauri::AppHandle,
    programm_id: String,
    inhalt_json: String,
) -> Result<(), String> {
    let z = zugang_lesen(&app)?;
    let client = client_mit_ausweis(&z.ausweis_pem, &z.ca_pem)?;
    let url = format!("{}/api/foerderer-programme/{}", basis_url(&z.adresse), programm_id);
    let inhalt: serde_json::Value = serde_json::from_str(&inhalt_json)
        .map_err(|_| "Programm-Inhalt ist kein gültiges JSON.".to_string())?;
    let body = serde_json::json!({ "inhalt": inhalt }).to_string();
    let r = client
        .put(&url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Senden fehlgeschlagen: {}", fehler_kette(&e)))?;
    match r.status().as_u16() {
        429 => Err("Zu viele Anfragen – bitte kurz warten.".into()),
        s if (200..300).contains(&s) => Ok(()),
        s => Err(format!("Server antwortete mit {s}")),
    }
}

/// Zieht ein Programm zurück (mTLS DELETE /api/foerderer-programme/{id}).
#[tauri::command]
pub async fn programm_loeschen(app: tauri::AppHandle, programm_id: String) -> Result<(), String> {
    let z = zugang_lesen(&app)?;
    let client = client_mit_ausweis(&z.ausweis_pem, &z.ca_pem)?;
    let url = format!("{}/api/foerderer-programme/{}", basis_url(&z.adresse), programm_id);
    let r = client
        .delete(&url)
        .send()
        .await
        .map_err(|e| format!("Löschen fehlgeschlagen: {}", fehler_kette(&e)))?;
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ein lokal erzeugter Schlüssel + (hier selbstsigniertes) Zertifikat
    /// ergeben zusammen einen Ausweis, den der mTLS-Client annimmt – genau die
    /// Zusammensetzung, die `foerderer_verbinden` nach der Server-Antwort macht.
    #[test]
    fn zusammengesetzter_ausweis_wird_akzeptiert() {
        let key = rcgen::KeyPair::generate().unwrap();
        assert!(key.public_key_pem().contains("PUBLIC KEY"));
        let p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        let cert_pem = p.self_signed(&key).unwrap().pem();
        let ausweis_pem = format!("{}{}", key.serialize_pem(), cert_pem);
        client_mit_ausweis(&ausweis_pem, "").expect("Ausweis muss angenommen werden");
    }

    #[test]
    fn basis_url_ergaenzt_https() {
        assert_eq!(basis_url("team.example:8443"), "https://team.example:8443");
        assert_eq!(basis_url("https://x/"), "https://x");
    }
}
