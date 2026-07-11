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
    // Größenlimit (Audit E2): ein Zugangs-Paket ist nur ein paar KB (Adresse +
    // Zertifikat + Schlüssel als PEM). So kann eine riesige Datei nicht den
    // Speicher fluten.
    if let Ok(meta) = std::fs::metadata(&pfad) {
        if meta.len() > 256 * 1024 {
            return Err("Die Datei ist zu groß für ein Zugangs-Paket.".into());
        }
    }
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
    // vertrauen (das Server-Zertifikat ist von derselben CA signiert) – und
    // dann AUSSCHLIESSLICH ihr: die oeffentlichen Wurzeln (Let's Encrypt &
    // Co.) werden abgeschaltet, damit nur unser eigener Server akzeptiert
    // wird (Audit E1). Ohne Team-CA (aeltere Pakete) gelten die normalen
    // oeffentlichen Wurzeln weiter.
    let ca = ca_pem.trim();
    if !ca.is_empty() {
        let root = reqwest::Certificate::from_pem(ca.as_bytes())
            .map_err(|e| format!("Team-CA-Zertifikat ungueltig: {e}"))?;
        builder = builder
            .add_root_certificate(root)
            .tls_built_in_root_certs(false);
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

    // SICHERHEIT: Dieser Transparenz-Befehl darf AUSSCHLIESSLICH an localhost
    // senden. Sonst waere er ein Egress-Kanal, ueber den (z. B. via XSS) ein
    // beliebiger Koerper an ein fremdes Netzwerkziel geschickt werden koennte.
    let url = reqwest::Url::parse(&basis).map_err(|_| "Ungueltige Ziel-Adresse.".to_string())?;
    let host = url.host_str().unwrap_or("").trim_start_matches('[').trim_end_matches(']');
    let ist_loopback = host == "localhost"
        || host
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false);
    if !ist_loopback {
        return Err("Der Trockenlauf darf nur an localhost (127.0.0.1) senden.".into());
    }

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

/// Einzelplatz-Modus: Katalog OHNE Zertifikat ueber den offenen Kanal holen
/// (GET <basis>/katalog, in der Regel http://<nas>:8445/katalog). Reine
/// HTTP-Anfrage, kein Geraete-Ausweis. Der Katalog ist unkritisch.
#[tauri::command]
pub async fn katalog_oeffentlich_holen(adresse: String) -> Result<String, String> {
    let a = adresse.trim().trim_end_matches('/');
    let basis = if a.starts_with("http://") || a.starts_with("https://") {
        a.to_string()
    } else {
        // Der Einzelplatz-Kanal laeuft ueber http (im Tailscale verschluesselt).
        format!("http://{a}")
    };
    let url = format!("{basis}/katalog");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Client-Fehler: {e}"))?;
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

/// Holt ein volles Förderer-Logo (Data-URL) vom Team-Server (mTLS GET
/// /api/logos/{id}). Ok(None), wenn (noch) kein Logo hinterlegt ist (404).
/// Genutzt für die Danksagung/Credits; die kleine Kachel-Vorschau steckt schon
/// im Katalog.
#[tauri::command]
pub async fn sync_logo_holen(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    logo_id: String,
) -> Result<Option<String>, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/logos/{}", basis_url(&adresse), logo_id);
    let r = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Abruf fehlgeschlagen: {}", fehler_kette(&e)))?;
    match r.status().as_u16() {
        404 => Ok(None),
        s if !(200..300).contains(&s) => Err(format!("Server antwortete mit {s}")),
        _ => Ok(Some(
            r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))?,
        )),
    }
}

/// Einzelplatz-Modus: volles Förderer-Logo OHNE Zertifikat holen
/// (GET <basis>/logos/{id}, i. d. R. http://<nas>:8445/logos/{id}).
#[tauri::command]
pub async fn logo_oeffentlich_holen(
    adresse: String,
    logo_id: String,
) -> Result<Option<String>, String> {
    let a = adresse.trim().trim_end_matches('/');
    let basis = if a.starts_with("http://") || a.starts_with("https://") {
        a.to_string()
    } else {
        format!("http://{a}")
    };
    let url = format!("{basis}/logos/{logo_id}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Client-Fehler: {e}"))?;
    let r = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Abruf fehlgeschlagen: {e}"))?;
    match r.status().as_u16() {
        404 => Ok(None),
        s if !(200..300).contains(&s) => Err(format!("Server antwortete mit {s}")),
        _ => Ok(Some(
            r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))?,
        )),
    }
}

/// Sendet EINE Katalog-Meldung an den Team-Server (mTLS PUT
/// /api/meldung/{id}). Der Server macht Upsert per id; der Body
/// (foerderungId/Name/Art/Text) wird vom Frontend gebaut. Bei einer
/// Spam-Bremse (429) o. Ä. kommt ein klarer Fehler zurück, den der
/// Sync-Takt einfach beim nächsten Mal erneut versucht.
#[tauri::command]
pub async fn sync_meldung_senden(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    meldung_id: String,
    body_json: String,
) -> Result<(), String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/meldung/{}", basis_url(&adresse), meldung_id);
    let r = client
        .put(&url)
        .header("content-type", "application/json")
        .body(body_json)
        .send()
        .await
        .map_err(|e| format!("Senden fehlgeschlagen: {}", fehler_kette(&e)))?;
    if r.status().as_u16() == 429 {
        return Err("Zu viele Anfragen – wird später erneut versucht.".into());
    }
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    Ok(())
}

/// Holt die vom Team geteilten eigenen Förderer (mTLS GET
/// /api/foerderer). Gibt die rohe JSON-Liste zurück; das Frontend
/// wandelt sie in Katalog-Form um.
#[tauri::command]
pub async fn sync_foerderer_holen(adresse: String, ausweis_pem: String, ca_pem: String) -> Result<String, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/foerderer", basis_url(&adresse));
    let r = client.get(&url).send().await.map_err(|e| format!("Abruf fehlgeschlagen: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    r.text().await.map_err(|e| format!("Antwort nicht lesbar: {e}"))
}

/// Teilt/aktualisiert EINEN eigenen Förderer (mTLS PUT
/// /api/foerderer/{id}). Der Body (nur öffentliche Felder) wird vom
/// Frontend gebaut; Upsert per id auf dem Server.
#[tauri::command]
pub async fn sync_foerderer_senden(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    foerderer_id: String,
    body_json: String,
) -> Result<(), String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/foerderer/{}", basis_url(&adresse), foerderer_id);
    let r = client
        .put(&url)
        .header("content-type", "application/json")
        .body(body_json)
        .send()
        .await
        .map_err(|e| format!("Senden fehlgeschlagen: {}", fehler_kette(&e)))?;
    if r.status().as_u16() == 429 {
        return Err("Zu viele Anfragen – wird später erneut versucht.".into());
    }
    if !r.status().is_success() {
        return Err(format!("Server antwortete mit {}", r.status()));
    }
    Ok(())
}

/// Zieht einen geteilten eigenen Förderer zurück (mTLS DELETE
/// /api/foerderer/{id}).
#[tauri::command]
pub async fn sync_foerderer_loeschen(
    adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    foerderer_id: String,
) -> Result<(), String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/foerderer/{}", basis_url(&adresse), foerderer_id);
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

// ============================================================
// Gehostetes Modell (Schritt 4): kopiersicheres Enrollment, Client-Seite.
//
// Anders als das alte Datei-Paket (das den privaten Schlüssel MITSCHICKTE)
// erzeugt das neue Gerät sein Schlüsselpaar hier LOKAL und sendet nur den
// öffentlichen Teil an den Server. Der Server signiert (Service-CA) und gibt
// nur das Zertifikat zurück; erst hier setzen wir Schlüssel + Zertifikat zum
// vollen Ausweis zusammen. Der private Schlüssel verlässt das Gerät nie.
//
// Server-Trust-Naht: Welche CA das neue Gerät dem SERVER-Zertifikat glaubt,
// steht in der Einladung (`ca_pem`) – der Einladende kennt sie bereits. So
// braucht es dafür keine Server-Änderung, und die bestehende (self-hosted)
// Installation bleibt unberührt.
// ============================================================

/// HTTP-Client für den ÖFFENTLICHEN Enroll-Kanal (das neue Gerät hat noch
/// keinen Ausweis). Vertraut den öffentlichen Wurzeln (Let's Encrypt) UND –
/// falls mitgegeben – zusätzlich der Einladungs-CA (für self-hosted Server mit
/// eigener Wurzel). Kein Client-Zertifikat: der Token ist die Berechtigung.
fn client_oeffentlich(ca_pem: &str) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(15));
    let ca = ca_pem.trim();
    if !ca.is_empty() {
        if let Ok(root) = reqwest::Certificate::from_pem(ca.as_bytes()) {
            builder = builder.add_root_certificate(root);
        }
    }
    builder
        .build()
        .map_err(|e| format!("Verbindungs-Client nicht erstellbar: {e}"))
}

/// Nimmt eine Einladung an: erzeugt das Schlüsselpaar lokal, löst den Token am
/// öffentlichen Enroll-Kanal ein und gibt einen fertigen Zugang zurück (den das
/// Frontend wie ein Zugangs-Paket verschlüsselt in den Tresor legt).
#[tauri::command]
pub async fn enroll_annehmen(
    enroll_url: String,
    sync_adresse: String,
    ca_pem: String,
    token: String,
    geraet_name: String,
) -> Result<ZugangsInfo, String> {
    let name = geraet_name.trim();
    if name.is_empty() {
        return Err("Bitte einen Geraetenamen angeben.".into());
    }
    if token.trim().is_empty() {
        return Err("Es fehlt der Einladungs-Code.".into());
    }

    // Schlüsselpaar LOKAL erzeugen – nur der öffentliche Teil wird gesendet.
    let key = rcgen::KeyPair::generate().map_err(|e| format!("Schluessel nicht erzeugbar: {e}"))?;
    let pubkey_pem = key.public_key_pem();

    let client = client_oeffentlich(&ca_pem)?;
    let url = format!("{}/api/enroll", basis_url(&enroll_url));
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
            401 => "Der Einladungs-Code ist ungueltig.".to_string(),
            402 => "Fuer dieses Team ist kein aktives Abo hinterlegt.".to_string(),
            409 => "Diese Einladung wurde bereits mit einem anderen Geraet eingeloest.".to_string(),
            410 => "Der Einladungs-Code ist abgelaufen.".to_string(),
            503 => "Der Server kann derzeit keine Ausweise ausstellen.".to_string(),
            s => format!("Server antwortete mit {s}"),
        });
    }

    #[derive(Deserialize)]
    struct EnrollAntwort {
        #[serde(default)]
        ausweis_pem: String,
    }
    let antwort: EnrollAntwort = r
        .json()
        .await
        .map_err(|e| format!("Antwort nicht lesbar: {e}"))?;
    if !antwort.ausweis_pem.contains("CERTIFICATE") {
        return Err("Der Server hat keinen gueltigen Ausweis geliefert.".into());
    }

    // Lokalen Schlüssel + ausgestelltes Zertifikat zum vollen Ausweis fügen.
    let ausweis_pem = format!("{}{}", key.serialize_pem(), antwort.ausweis_pem);
    let geraet_name = geraet_name_aus_pem(&ausweis_pem).unwrap_or_else(|_| name.to_string());

    Ok(ZugangsInfo {
        adresse: sync_adresse.trim().to_string(),
        geraet_name,
        ausweis_pem,
        ca_pem,
    })
}

#[derive(Serialize)]
pub struct EinladungErgebnis {
    pub token: String,
    pub ablauf: String,
    /// Fertiges Einladungs-Paket (JSON) zum Weitergeben an das neue Gerät.
    pub paket_json: String,
}

/// Erstellt eine Einladung (nur der Eigentümer, per mTLS): fragt am Server einen
/// Einmal-Token an und baut daraus ein Einladungs-Paket, das die Server-Adressen
/// und die Server-Trust-CA mitführt.
#[tauri::command]
pub async fn einladung_erstellen(
    sync_adresse: String,
    ausweis_pem: String,
    ca_pem: String,
    enroll_url: String,
    geraet_name: String,
) -> Result<EinladungErgebnis, String> {
    let client = client_mit_ausweis(&ausweis_pem, &ca_pem)?;
    let url = format!("{}/api/einladung", basis_url(&sync_adresse));
    let body = serde_json::json!({ "name": geraet_name.trim() }).to_string();
    let r = client
        .post(&url)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("Einladung fehlgeschlagen: {}", fehler_kette(&e)))?;
    let status = r.status();
    if !status.is_success() {
        return Err(match status.as_u16() {
            401 | 403 => "Nur der Team-Eigentuemer darf einladen.".to_string(),
            402 => "Zum Einladen ist ein aktives Abo noetig.".to_string(),
            503 => "Der Server kann derzeit keine Einladungen ausstellen.".to_string(),
            s => format!("Server antwortete mit {s}"),
        });
    }

    #[derive(Deserialize)]
    struct Ant {
        #[serde(default)]
        token: String,
        #[serde(default)]
        ablauf: String,
    }
    let a: Ant = r
        .json()
        .await
        .map_err(|e| format!("Antwort nicht lesbar: {e}"))?;
    if a.token.trim().is_empty() {
        return Err("Der Server lieferte keinen Einladungs-Code.".into());
    }

    let paket_json = serde_json::to_string_pretty(&serde_json::json!({
        "typ": "antrag3000-einladung",
        "version": 1,
        "enroll_url": enroll_url.trim(),
        "sync_adresse": sync_adresse.trim(),
        "token": a.token,
        "ablauf": a.ablauf,
        // Server-Trust-CA fürs neue Gerät (Schritt-4-Naht): der Einladende kennt sie.
        "ca_pem": ca_pem,
    }))
    .unwrap_or_default();

    Ok(EinladungErgebnis {
        token: a.token,
        ablauf: a.ablauf,
        paket_json,
    })
}

#[derive(Serialize, Deserialize)]
pub struct EinladungInhalt {
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
}

/// Liest ein Einladungs-Paket (.a3keinladung / JSON) und gibt seine Felder
/// zurück; das Frontend ruft danach `enroll_annehmen` auf.
#[tauri::command]
pub fn einladung_lesen(pfad: String) -> Result<EinladungInhalt, String> {
    if let Ok(meta) = std::fs::metadata(&pfad) {
        if meta.len() > 256 * 1024 {
            return Err("Die Datei ist zu groß für eine Einladung.".into());
        }
    }
    let roh = std::fs::read_to_string(&pfad).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let inhalt: EinladungInhalt =
        serde_json::from_str(&roh).map_err(|_| "Das ist keine gueltige Einladung.".to_string())?;
    if inhalt.typ != "antrag3000-einladung" {
        return Err("Das ist keine Antrag-3000-Einladung.".into());
    }
    if inhalt.token.trim().is_empty() {
        return Err("In der Einladung fehlt der Code.".into());
    }
    Ok(inhalt)
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
    "ausweis_pem":  "-----BEGIN EC PRIVATE KEY-----\r\nMHcCAQEEIJpR2GSmQWCUqxJQ80QwRryaW4eoJRPFRR/Mp0D51VoxoAoGCCqGSM49\r\nAwEHoUQDQgAEcUmeIX+sUqoaMSlZsVIy0iEtHvCkk/kLEw5V9vQgsci3KbyN8XYj\r\nYpGHsWz1uUW4p5jselGewjEw6Mq7tXC5MQ==\r\n-----END EC PRIVATE KEY-----\r\n-----BEGIN CERTIFICATE-----\r\nMIIBrzCCAVSgAwIBAgIUS5q/ghmnXUYVA+WMMPTo87BJcRMwCgYIKoZIzj0EAwIw\r\nOTEZMBcGA1UECgwQQW50cmFnIDMwMDAgVGVhbTEcMBoGA1UEAwwTQW50cmFnIDMw\r\nMDAgVGVhbSBDQTAeFw0yNjA3MDIxMzU1MzhaFw0yODEyMTgxMzU1MzhaMDExGTAX\r\nBgNVBAoMEEFudHJhZyAzMDAwIFRlYW0xFDASBgNVBAMMC0xhcHRvcC1UZXN0MFkw\r\nEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEcUmeIX+sUqoaMSlZsVIy0iEtHvCkk/kL\r\nEw5V9vQgsci3KbyN8XYjYpGHsWz1uUW4p5jselGewjEw6Mq7tXC5MaNCMEAwHQYD\r\nVR0OBBYEFMg8TvGOOugRtN9gP5rrukqBJVddMB8GA1UdIwQYMBaAFAYZut6cFPxY\r\nYNMyChKiV/1r3l6dMAoGCCqGSM49BAMCA0kAMEYCIQCXWJa6qmFl82LwEvsyu8tc\r\nu1laMmLUqPTaqaw1jsapZgIhANinjSoE7lFOfdBjdzKOW6Lm3Egh/KhzbCYM6q2s\r\nM7Tm\r\n-----END CERTIFICATE-----\r\n"
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

    /// Kopiersicheres Enrollment (Client-Seite): ein lokal erzeugter Schlüssel
    /// + das (hier ersatzweise selbstsignierte) Zertifikat ergeben zusammen
    /// einen Ausweis, den der mTLS-Client annimmt – genau die Zusammensetzung,
    /// die `enroll_annehmen` nach der Server-Antwort vornimmt.
    #[test]
    fn lokaler_schluessel_plus_zertifikat_ergibt_ausweis() {
        let key = rcgen::KeyPair::generate().unwrap();
        // Nur der öffentliche Teil würde an den Server gehen.
        assert!(key.public_key_pem().contains("PUBLIC KEY"));

        // Server-Ersatz: ein Zertifikat für genau diesen Schlüssel.
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        p.distinguished_name
            .push(rcgen::DnType::CommonName, "Neues-Geraet");
        let cert_pem = p.self_signed(&key).unwrap().pem();

        // Wie in enroll_annehmen: lokaler Schlüssel + Zertifikat.
        let ausweis_pem = format!("{}{}", key.serialize_pem(), cert_pem);
        assert_eq!(geraet_name_aus_pem(&ausweis_pem).unwrap(), "Neues-Geraet");
        client_mit_ausweis(&ausweis_pem, "")
            .expect("zusammengesetzter Ausweis muss vom mTLS-Client angenommen werden");
    }

    /// Ein Einladungs-Paket (wie es `einladung_erstellen` baut) wird von
    /// `einladung_lesen` mit allen Feldern wieder eingelesen; falscher Typ und
    /// fehlender Code werden abgewiesen.
    #[test]
    fn einladung_rundlauf() {
        let paket = serde_json::json!({
            "typ": "antrag3000-einladung",
            "version": 1,
            "enroll_url": "sync.example",
            "sync_adresse": "team.example:8443",
            "token": "ABC123",
            "ablauf": "2026-07-12 10:00:00",
            "ca_pem": "-----BEGIN CERTIFICATE-----\nX\n-----END CERTIFICATE-----\n",
        });
        let pfad = std::env::temp_dir().join("a3000-einladung-test.a3keinladung");
        std::fs::write(&pfad, paket.to_string()).unwrap();

        let gelesen = einladung_lesen(pfad.to_string_lossy().to_string()).unwrap();
        assert_eq!(gelesen.token, "ABC123");
        assert_eq!(gelesen.sync_adresse, "team.example:8443");
        assert_eq!(gelesen.enroll_url, "sync.example");
        assert!(gelesen.ca_pem.contains("BEGIN CERTIFICATE"));
        let _ = std::fs::remove_file(&pfad);

        // Falscher Typ → Fehler.
        let falsch = std::env::temp_dir().join("a3000-einladung-falsch.json");
        std::fs::write(&falsch, r#"{"typ":"etwas-anderes","token":"x"}"#).unwrap();
        assert!(einladung_lesen(falsch.to_string_lossy().to_string()).is_err());
        let _ = std::fs::remove_file(&falsch);
    }
}
