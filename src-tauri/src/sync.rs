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
    // Oeffentliches Team-CA-Zertifikat, damit die App auch dem
    // Server-Zertifikat (von derselben CA signiert) vertraut. Optional,
    // damit aeltere Pakete weiter funktionieren.
    #[serde(default)]
    ca_pem: String,
}

#[derive(Serialize)]
pub struct ZugangsInfo {
    pub adresse: String,
    pub geraet_name: String,
    pub ausweis_pem: String,
    pub ca_pem: String,
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

    Ok(ZugangsInfo {
        adresse,
        geraet_name,
        ausweis_pem: paket.ausweis_pem,
        ca_pem: paket.ca_pem,
    })
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

fn client_mit_ausweis(ausweis_pem: &str, ca_pem: &str) -> Result<reqwest::Client, String> {
    let id = reqwest::Identity::from_pem(ausweis_pem.as_bytes())
        .map_err(|e| format!("Ausweis ungueltig: {e}"))?;
    let mut builder = reqwest::Client::builder()
        .identity(id)
        .timeout(std::time::Duration::from_secs(12));
    // Wenn ein Team-CA-Zertifikat vorliegt, ihm auch fuer die Server-Seite
    // vertrauen (das Server-Zertifikat ist von derselben CA signiert).
    // Sonst gelten die normalen oeffentlichen Wurzeln (z. B. Let's Encrypt).
    let ca = ca_pem.trim();
    if !ca.is_empty() {
        let root = reqwest::Certificate::from_pem(ca.as_bytes())
            .map_err(|e| format!("Team-CA-Zertifikat ungueltig: {e}"))?;
        builder = builder.add_root_certificate(root);
    }
    builder
        .build()
        .map_err(|e| format!("Verbindungs-Client nicht erstellbar: {e}"))
}

/// Hängt die Ursachen-Kette eines Fehlers an (reqwest versteckt den
/// eigentlichen TLS-Grund in der `source`).
fn fehler_kette(e: &(dyn std::error::Error)) -> String {
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

/// Verbindungstest: GET /api/health mit Geraete-Ausweis.
#[tauri::command]
pub async fn sync_health(adresse: String, ausweis_pem: String, ca_pem: String) -> Result<bool, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/health", basis_url(&adresse));
    let r = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Nicht erreichbar: {}", fehler_kette(&e)))?;
    Ok(r.status().is_success())
}

/// Holt alle Board-Projekte des Teams (GET /api/board). Gibt die rohe
/// JSON-Antwort als Text zurueck; das Frontend wertet sie aus.
#[tauri::command]
pub async fn sync_get_board(adresse: String, ausweis_pem: String, ca_pem: String) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
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
    ca_pem: String,
    projekt_id: String,
    body_json: String,
) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
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

/// Entfernt ein Board-Projekt vom Team-Server (DELETE /api/board/{id}).
/// Wird genutzt, wenn ein Projekt lokal geloescht wurde – damit es nicht
/// als Leiche auf dem Team-Board stehen bleibt.
#[tauri::command]
pub async fn sync_delete_board(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    projekt_id: String,
) -> Result<(), String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/board/{}", basis_url(&adresse), projekt_id);
    let r = client
        .delete(&url)
        .send()
        .await
        .map_err(|e| format!("Loeschen fehlgeschlagen: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    Ok(())
}

/// Transparenz-Werkzeug ("Trockenlauf"): sendet die EXAKTEN Sende-Koerper,
/// die der echte Sync hochladen wuerde, an einen lokalen Mitschnitt-Server
/// (z. B. http://127.0.0.1:8099). So kann man UNABHAENGIG von der App und
/// ohne NAS nachpruefen, welche Felder die App ins Netz geben wuerde.
/// Nutzt bewusst KEINEN Ausweis und KEIN TLS – reiner Test gegen localhost.
#[tauri::command]
pub async fn sync_trockenlauf(ziel_url: String, koerper: Vec<String>) -> Result<usize, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Client-Fehler: {e}"))?;
    let basis = basis_url(&ziel_url);
    let mut gesendet = 0;
    for body in &koerper {
        client
            .post(format!("{}/api/board/trockenlauf", basis))
            .header("content-type", "application/json")
            .body(body.clone())
            .send()
            .await
            .map_err(|e| format!("Senden an den Mitschnitt fehlgeschlagen: {e}"))?;
        gesendet += 1;
    }
    Ok(gesendet)
}

/// Holt den aktuellen Förder-Katalog vom Team-Server (mTLS GET
/// /api/katalog). Gibt das rohe JSON zurück; Pruefung/Anwendung macht
/// das Frontend (gleiche Logik wie beim Update aus einer Datei).
#[tauri::command]
pub async fn sync_katalog_holen(adresse: String, ausweis_pem: String, ca_pem: String) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/katalog", basis_url(&adresse));
    let r = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Abruf fehlgeschlagen: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))
}

// --- Zertifikate erzeugen (Admin / Verwalter:in), reines Rust ----------

#[derive(Serialize)]
pub struct TeamCa {
    pub cert_pem: String,
    pub key_pem: String,
}

/// Erzeugt eine neue Team-CA (Aussteller-Stempel). Den privaten
/// Schluessel legt das Frontend verschluesselt in den Tresor.
#[tauri::command]
pub fn team_ca_erstellen() -> Result<TeamCa, String> {
    let key = rcgen::KeyPair::generate().map_err(|e| format!("Schluessel nicht erzeugbar: {e}"))?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
        .map_err(|e| format!("Zertifikat nicht vorbereitbar: {e}"))?;
    p.distinguished_name.push(rcgen::DnType::OrganizationName, "Antrag 3000 Team");
    p.distinguished_name.push(rcgen::DnType::CommonName, "Antrag 3000 Team CA");
    p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(0));
    p.key_usages = vec![rcgen::KeyUsagePurpose::KeyCertSign, rcgen::KeyUsagePurpose::CrlSign];
    p.not_before = rcgen::date_time_ymd(2024, 1, 1);
    p.not_after = rcgen::date_time_ymd(2035, 1, 1);
    let cert = p.self_signed(&key).map_err(|e| format!("CA nicht signierbar: {e}"))?;
    Ok(TeamCa { cert_pem: cert.pem(), key_pem: key.serialize_pem() })
}

/// Baut den Geraete-Ausweis (PEM: privater Schluessel + Zertifikat),
/// signiert von der Team-CA.
fn baue_geraet_pem(ca_cert_pem: &str, ca_key_pem: &str, geraet_name: &str) -> Result<String, String> {
    if geraet_name.is_empty() {
        return Err("Bitte einen Geraetenamen angeben.".into());
    }
    let ca_key = rcgen::KeyPair::from_pem(ca_key_pem)
        .map_err(|e| format!("CA-Schluessel ungueltig: {e}"))?;
    let ca_params = rcgen::CertificateParams::from_ca_cert_pem(ca_cert_pem)
        .map_err(|e| format!("CA-Zertifikat ungueltig: {e}"))?;
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .map_err(|e| format!("CA nicht ladbar: {e}"))?;
    let dev_key = rcgen::KeyPair::generate().map_err(|e| format!("Schluessel nicht erzeugbar: {e}"))?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
        .map_err(|e| format!("Zertifikat nicht vorbereitbar: {e}"))?;
    p.distinguished_name.push(rcgen::DnType::OrganizationName, "Antrag 3000 Team");
    p.distinguished_name.push(rcgen::DnType::CommonName, geraet_name);
    p.is_ca = rcgen::IsCa::NoCa;
    p.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    p.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
    p.not_before = rcgen::date_time_ymd(2024, 1, 1);
    p.not_after = rcgen::date_time_ymd(2030, 1, 1);
    let cert = p
        .signed_by(&dev_key, &ca_cert, &ca_key)
        .map_err(|e| format!("Geraete-Zertifikat nicht signierbar: {e}"))?;
    Ok(format!("{}{}", dev_key.serialize_pem(), cert.pem()))
}

fn paket_json(adresse: &str, pem: &str, ca_pem: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "typ": "antrag3000-zugangspaket",
        "version": 1,
        "adresse": adresse.trim(),
        "ausweis_pem": pem,
        "ca_pem": ca_pem,
    }))
    .unwrap_or_default()
}

/// Erzeugt ein Zugangs-Paket fuer ein Geraet und speichert es als Datei.
#[tauri::command]
pub fn geraet_paket_speichern(
    ca_cert_pem: String,
    ca_key_pem: String,
    geraet_name: String,
    adresse: String,
    ziel: String,
) -> Result<(), String> {
    let pem = baue_geraet_pem(&ca_cert_pem, &ca_key_pem, geraet_name.trim())?;
    std::fs::write(&ziel, paket_json(&adresse, &pem, &ca_cert_pem))
        .map_err(|e| format!("Datei nicht schreibbar: {e}"))
}

/// Erzeugt ein Zugangs-Paket fuer DIESES Geraet und gibt es direkt
/// zurueck (zum sofortigen Einrichten ohne Datei).
#[tauri::command]
pub fn geraet_paket_direkt(
    ca_cert_pem: String,
    ca_key_pem: String,
    geraet_name: String,
    adresse: String,
) -> Result<ZugangsInfo, String> {
    let pem = baue_geraet_pem(&ca_cert_pem, &ca_key_pem, geraet_name.trim())?;
    Ok(ZugangsInfo {
        adresse: adresse.trim().to_string(),
        geraet_name: geraet_name.trim().to_string(),
        ausweis_pem: pem,
        ca_pem: ca_cert_pem,
    })
}

/// Speichert das CA-Zertifikat (oeffentlich) als Datei fuer die NAS (Caddy).
#[tauri::command]
pub fn team_ca_cert_exportieren(cert_pem: String, ziel: String) -> Result<(), String> {
    std::fs::write(&ziel, cert_pem).map_err(|e| format!("Datei nicht schreibbar: {e}"))
}

/// Erzeugt ein NAS-Server-Zertifikat (von der Team-CA signiert) fuer die
/// angegebene Adresse (Tailscale-Name oder 100.x-IP) und speichert
/// `server.crt` + `server.key` neben dem gewaehlten Ziel. Caddy auf der
/// NAS nutzt diese fuer HTTPS; die App vertraut der Team-CA und damit
/// diesem Zertifikat – ganz ohne Let's Encrypt/Cloudflare.
#[tauri::command]
pub fn server_zertifikat_speichern(
    ca_cert_pem: String,
    ca_key_pem: String,
    adresse: String,
    ziel_crt: String,
) -> Result<(), String> {
    // Host aus der Adresse loesen (Schema/Pfad entfernen).
    let host = adresse
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if host.is_empty() {
        return Err("Bitte eine NAS-Adresse angeben.".into());
    }
    // Port fuer den SAN entfernen – Zertifikate enthalten keinen Port.
    let host = match host.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => h.to_string(),
        _ => host,
    };

    let ca_key = rcgen::KeyPair::from_pem(&ca_key_pem)
        .map_err(|e| format!("CA-Schluessel ungueltig: {e}"))?;
    let ca_params = rcgen::CertificateParams::from_ca_cert_pem(&ca_cert_pem)
        .map_err(|e| format!("CA-Zertifikat ungueltig: {e}"))?;
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .map_err(|e| format!("CA nicht ladbar: {e}"))?;

    let srv_key = rcgen::KeyPair::generate().map_err(|e| format!("Schluessel nicht erzeugbar: {e}"))?;
    let mut p = rcgen::CertificateParams::new(Vec::<String>::new())
        .map_err(|e| format!("Zertifikat nicht vorbereitbar: {e}"))?;
    p.distinguished_name.push(rcgen::DnType::CommonName, host.clone());
    // SAN passend zum Host: IP-Adresse oder DNS-Name.
    let san = match host.parse::<std::net::IpAddr>() {
        Ok(ip) => rcgen::SanType::IpAddress(ip),
        Err(_) => rcgen::SanType::DnsName(
            host.clone().try_into().map_err(|_| "Adresse ist kein gueltiger Name.".to_string())?,
        ),
    };
    p.subject_alt_names = vec![san];
    p.is_ca = rcgen::IsCa::NoCa;
    p.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    p.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    p.not_before = rcgen::date_time_ymd(2024, 1, 1);
    p.not_after = rcgen::date_time_ymd(2030, 1, 1);
    let cert = p
        .signed_by(&srv_key, &ca_cert, &ca_key)
        .map_err(|e| format!("Server-Zertifikat nicht signierbar: {e}"))?;

    // server.crt (an die gewaehlte Stelle) und server.key (daneben).
    std::fs::write(&ziel_crt, cert.pem())
        .map_err(|e| format!("server.crt nicht schreibbar: {e}"))?;
    let key_pfad = std::path::Path::new(&ziel_crt).with_file_name("server.key");
    std::fs::write(&key_pfad, srv_key.serialize_pem())
        .map_err(|e| format!("server.key nicht schreibbar: {e}"))?;
    Ok(())
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
        client_mit_ausweis(pem, "").expect("Client sollte mit dem Ausweis baubar sein");
    }

    #[test]
    fn in_app_paket_ist_parsebar() {
        // Team-CA + Geraete-Paket komplett in Rust erzeugen ...
        let ca = team_ca_erstellen().unwrap();
        let info = geraet_paket_direkt(
            ca.cert_pem.clone(),
            ca.key_pem.clone(),
            "Test-Geraet".into(),
            "team.example".into(),
        )
        .unwrap();
        assert_eq!(info.geraet_name, "Test-Geraet");
        assert_eq!(info.adresse, "team.example");
        assert!(info.ca_pem.contains("BEGIN CERTIFICATE"));
        client_mit_ausweis(&info.ausweis_pem, &info.ca_pem)
            .expect("rcgen-Ausweis muss vom mTLS-Client angenommen werden");

        // Server-Zertifikat aus derselben CA erzeugen (SAN = Adresse).
        let crt = std::env::temp_dir().join("a3000-server-test.crt");
        server_zertifikat_speichern(
            ca.cert_pem.clone(),
            ca.key_pem.clone(),
            "nas.example.ts.net".into(),
            crt.to_string_lossy().to_string(),
        )
        .expect("Server-Zertifikat muss erzeugbar sein");
        assert!(crt.exists());
        let key = crt.with_file_name("server.key");
        assert!(key.exists());
        let _ = std::fs::remove_file(&crt);
        let _ = std::fs::remove_file(&key);

        // ... als Datei speichern und wieder einlesen (Rundlauf).
        let pfad = std::env::temp_dir().join("a3000-inapp-test.a3kpaket");
        geraet_paket_speichern(
            ca.cert_pem.clone(),
            ca.key_pem.clone(),
            "Tablet-X".into(),
            "team.example".into(),
            pfad.to_string_lossy().to_string(),
        )
        .unwrap();
        let geprueft = zugangspaket_pruefen(pfad.to_string_lossy().to_string()).unwrap();
        assert_eq!(geprueft.geraet_name, "Tablet-X");
        let _ = std::fs::remove_file(&pfad);
    }
}
