// ============================================================
// Team-Synchronisation, Client-Seite (Phase 2 / Etappe 4).
//
// - zugangspaket_pruefen: liest ein .a3kpaket (JSON), prueft Zertifikat
//   und privaten Schluessel und liest den Geraetenamen (CN) aus dem
//   Zertifikat. WAS gespeichert wird, entscheidet das Frontend (legt es
//   verschluesselt in den Tresor).
// - sync_health / sync_get / sync_put: mTLS-HTTP-Aufrufe zum Sync-Server
//   (laut CLAUDE.md ist der mTLS-HTTP-Client eine Rust-Aufgabe). Der
//   Geraete-Ausweis (PEM mit Schluessel+Zertifikat) wird je Aufruf
//   uebergeben.
// ============================================================

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Paket {
    #[serde(default)]
    typ: String,
    #[serde(default)]
    adresse: String,
    #[serde(default)]
    ausweis_pem: String,
}

#[derive(Serialize)]
pub struct ZugangsInfo {
    pub adresse: String,
    pub geraet_name: String,
    pub ausweis_pem: String,
}

/// Liest und prueft ein Zugangs-Paket (.a3kpaket).
#[tauri::command]
pub fn zugangspaket_pruefen(pfad: String) -> Result<ZugangsInfo, String> {
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let paket: Paket = serde_json::from_str(&roh)
        .map_err(|_| "Das ist kein gueltiges Zugangs-Paket.".to_string())?;
    if paket.typ != "antrag3000-zugangspaket" {
        return Err("Das ist kein Antrag-3000-Zugangs-Paket.".into());
    }
    let adresse = paket.adresse.trim().to_string();
    if adresse.is_empty() {
        return Err("Im Paket fehlt die Team-Adresse.".into());
    }

    let geraet_name = geraet_name_aus_pem(&paket.ausweis_pem)?;

    // Privater Schluessel muss vorhanden sein.
    let mut leser = std::io::BufReader::new(paket.ausweis_pem.as_bytes());
    let hat_key = rustls_pemfile::private_key(&mut leser)
        .map_err(|_| "Privater Schluessel im Paket nicht lesbar.".to_string())?
        .is_some();
    if !hat_key {
        return Err("Im Paket fehlt der private Schluessel.".into());
    }

    Ok(ZugangsInfo { adresse, geraet_name, ausweis_pem: paket.ausweis_pem })
}

/// Liest den Common Name (Geraetenamen) aus dem ersten Zertifikat im PEM.
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
        .unwrap_or_else(|| "Unbenanntes Geraet".to_string());
    Ok(name)
}

// --- mTLS-HTTP -----------------------------------------------------------

fn client_mit_ausweis(ausweis_pem: &str) -> Result<reqwest::Client, String> {
    let id = reqwest::Identity::from_pem(ausweis_pem.as_bytes())
        .map_err(|e| format!("Ausweis ungueltig: {e}"))?;
    reqwest::Client::builder()
        .identity(id)
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|e| format!("Verbindungs-Client nicht erstellbar: {e}"))
}

fn basis_url(adresse: &str) -> String {
    let a = adresse.trim().trim_end_matches('/');
    if a.starts_with("http://") || a.starts_with("https://") {
        a.to_string()
    } else {
        format!("https://{a}")
    }
}

/// Verbindungstest: GET /api/health mit Geraete-Ausweis.
#[tauri::command]
pub async fn sync_health(adresse: String, ausweis_pem: String) -> Result<bool, String> {
    let client = client_mit_ausweis(&ausweis_pem)?;
    let url = format!("{}/api/health", basis_url(&adresse));
    let r = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Nicht erreichbar: {e}"))?;
    Ok(r.status().is_success())
}

/// Holt alle Board-Projekte des Teams (GET /api/board). Gibt die rohe
/// JSON-Antwort als Text zurueck; das Frontend wertet sie aus.
#[tauri::command]
pub async fn sync_get_board(adresse: String, ausweis_pem: String) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem)?;
    let url = format!("{}/api/board", basis_url(&adresse));
    let r = client.get(&url).send().await.map_err(|e| format!("Abruf fehlgeschlagen: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))
}

/// Schreibt/aktualisiert ein Board-Projekt (PUT /api/board/{id}). Body
/// wird vom Frontend gebaut (inhalt + basis_version). Gibt die rohe
/// JSON-Antwort zurueck.
#[tauri::command]
pub async fn sync_put_board(
    adresse: String,
    ausweis_pem: String,
    projekt_id: String,
    body_json: String,
) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem)?;
    let url = format!("{}/api/board/{}", basis_url(&adresse), projekt_id);
    let r = client
        .put(&url)
        .header("content-type", "application/json")
        .body(body_json)
        .send()
        .await
        .map_err(|e| format!("Senden fehlgeschlagen: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
    "typ":  "antrag3000-zugangspaket",
    "version":  1,
    "adresse":  "demoteam.synology.me",
    "ausweis_pem":  "-----BEGIN EC PRIVATE KEY-----\r\n***REMOVED***\r\n***REMOVED***\r\n***REMOVED***==\r\n-----END EC PRIVATE KEY-----\r\n-----BEGIN CERTIFICATE-----\r\nMIIB4jCCAYegAwIBAgIULjipeApuHbJs4rh1CjoVxJn+eTgwCgYIKoZIzj0EAwIw\r\nOTEZMBcGA1UECgwQQW50cmFnIDMwMDAgVGVhbTEcMBoGA1UEAwwTQW50cmFnIDMw\r\nMDAgVGVhbSBDQTAeFw0yNjA2MTQyMDAzNTZaFw0yODA5MTYyMDAzNTZaMDExGTAX\r\nBgNVBAoMEEFudHJhZyAzMDAwIFRlYW0xFDASBgNVBAMMC0xhcHRvcC1UZXN0MFkw\r\nEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEIyCIfZLv8OTyVshPIzEDJfzPHoGykivo\r\ne1FqwIfDdk1Ugc9aSCZsbhWewik9Q5d2qBbaR1S3zNYE8xcW9DGG56N1MHMwDAYD\r\nVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCB4AwEwYDVR0lBAwwCgYIKwYBBQUHAwIw\r\nHQYDVR0OBBYEFICyyuK0Q4/18TJQBKxzOzsC2+0lMB8GA1UdIwQYMBaAFIr5CcE5\r\no0xsQWSSP49AC8UD56leMAoGCCqGSM49BAMCA0kAMEYCIQDmwAsqjs8QP+1x1lDY\r\nLPVyT2h+MKUOLOqRKUw1oYgnqgIhAN62VODqIz//Y9J3HYYNYYv+u6NgE3s47tRL\r\nMMwoJP2T\r\n-----END CERTIFICATE-----\r\n"
}"#;

    #[test]
    fn paket_pruefen_liest_name_und_adresse() {
        let pfad = std::env::temp_dir().join("a3000-test-paket.a3kpaket");
        std::fs::write(&pfad, FIXTURE).unwrap();
        let info = zugangspaket_pruefen(pfad.to_string_lossy().to_string()).unwrap();
        assert_eq!(info.adresse, "demoteam.synology.me");
        assert_eq!(info.geraet_name, "Laptop-Test");
        assert!(info.ausweis_pem.contains("BEGIN CERTIFICATE"));
        let _ = std::fs::remove_file(&pfad);
    }

    #[test]
    fn reqwest_akzeptiert_den_ausweis() {
        // Stellt sicher, dass das EC-PEM-Format des Skripts vom mTLS-Client
        // angenommen wird (ohne Netzwerk).
        let paket: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let pem = paket["ausweis_pem"].as_str().unwrap();
        client_mit_ausweis(pem).expect("Client sollte mit dem Ausweis baubar sein");
    }
}
