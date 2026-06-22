// ============================================================
// Antrag 3000 – Sync-Dienst ("Topf 2")
//
// Nimmt ausschliesslich UNKRITISCHE Team-Daten entgegen (Board:
// Projektname, Förderungs-Status, Fristen, Förderer-Kontakt ohne Notiz).
// Sensible Daten liegen verschlüsselt auf den Geräten und erreichen
// diesen Dienst nie.
//
// Sicherheit: Der Dienst lauscht NUR intern (im Docker-Netz). Davor
// steht Caddy und prüft per mTLS, dass ein Gerät ein gültiges, von
// unserer Team-CA ausgestelltes Zertifikat hat. Caddy reicht das
// Client-Zertifikat als Base64 im Header "X-Client-Cert-DER" durch;
// daraus bilden wir den Geräte-Fingerabdruck (SHA-256).
//
// Datenmodell ist mandantenfähig (Konto → Nutzer → Geräte), auch wenn
// das MVP nur ein gemeinsames Team-Konto nutzt.
//
// Transport (entschieden): Cloudflare Tunnel + Cloudflare Access mit
// mTLS. Cloudflare prueft das Geraete-Zertifikat (eigene Team-CA dort
// hinterlegt) und reicht es als Header `Cf-Client-Cert-Der-Base64`
// durch. Damit der Dienst auch hinter Caddy (direkter DDNS-Weg)
// funktioniert, akzeptiert er ZUSAETZLICH den alten Header
// `X-Client-Cert-DER`.
// ============================================================

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::Engine;
use totp_rs::{Algorithm, Secret, TOTP};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
    // Tempo-Bremse gegen Spam: je Gerät die Zeitpunkte der letzten
    // Schreibvorgänge (Meldungen/Förderer). Liegt im Speicher (reicht
    // gegen Flutung; nach Neustart leer). Siehe tempo_ok().
    schreib: Arc<Mutex<HashMap<i64, Vec<Instant>>>>,
    // Admin-Sitzungen: Token -> Sitzung. Entstehen beim Anmelden (nach
    // gültigem TOTP-Code) und laufen nach SITZUNG_TTL_S ab. Im Speicher;
    // nach Neustart muss sich der Admin neu anmelden.
    sitzungen: Arc<Mutex<HashMap<String, AdminSitzung>>>,
}

/// Eine offene Admin-Sitzung (an ein Gerät gebunden, mit Ablaufzeit).
struct AdminSitzung {
    konto_id: i64,
    geraet_id: i64,
    ablauf: Instant,
}

// --- Spam-/Missbrauchs-Schutz (serverseitig; der Client ist nicht
//     vertrauenswürdig). Startwerte bewusst grosszügig, leicht tunbar. ---
/// Zeitfenster der Tempo-Bremse (Sekunden).
const TEMPO_FENSTER_S: u64 = 60;
/// Erlaubte Schreibvorgänge je Gerät und Zeitfenster (sonst 429).
const TEMPO_MAX: usize = 30;
/// Höchstzahl OFFENER Meldungen je Gerät (Kontingent).
const MELDUNG_QUOTA: i64 = 50;
/// Höchstzahl geteilter eigener Förderer je Gerät (Kontingent).
const FOERDERER_QUOTA: i64 = 100;
/// Größenlimit für einen geteilten Förderer-Datensatz (JSON, Bytes).
const FOERDERER_MAX_BYTES: usize = 8192;
/// Größenlimit für den hochgeladenen Gesamt-Katalog (JSON, Bytes).
const KATALOG_MAX_BYTES: usize = 2_000_000;
/// Gültigkeitsdauer einer Admin-Sitzung (Sekunden).
const SITZUNG_TTL_S: u64 = 1800;
/// Längenlimit für kurze Felder (id, Name, Art).
const FELD_MAX: usize = 200;
/// Längenlimit für die freie Anmerkung einer Meldung.
const TEXT_MAX: usize = 1000;

/// Bildet aus dem aktuellen Katalog und Sammler-Kandidaten die Vorschläge:
/// NEU (id fehlt im Katalog) oder GEÄNDERT (id vorhanden, Inhalt anders).
/// Unveränderte werden übersprungen. Reines Datenmodell (testbar).
/// Rückgabe je Eintrag: (art, foerderung_id, vorgeschlagener Inhalt).
fn vorschlaege_bilden(
    katalog: &[serde_json::Value],
    kandidaten: &[serde_json::Value],
) -> Vec<(String, String, serde_json::Value)> {
    let mut vorhanden: HashMap<String, &serde_json::Value> = HashMap::new();
    for f in katalog {
        if let Some(id) = f.get("id").and_then(|x| x.as_str()) {
            vorhanden.insert(id.to_string(), f);
        }
    }
    let mut out = Vec::new();
    for k in kandidaten {
        let Some(id) = k.get("id").and_then(|x| x.as_str()) else { continue };
        // Mindestens id + name, sonst unbrauchbar.
        if k.get("name").and_then(|x| x.as_str()).unwrap_or("").is_empty() {
            continue;
        }
        match vorhanden.get(id) {
            None => out.push(("neu".to_string(), id.to_string(), k.clone())),
            Some(alt) => {
                if **alt != *k {
                    out.push(("geaendert".to_string(), id.to_string(), k.clone()));
                }
            }
        }
    }
    out
}

/// Tempo-Bremse: Gibt true zurück, wenn das Gerät noch schreiben darf,
/// und merkt sich diesen Schreibvorgang. Alte Einträge ausserhalb des
/// Fensters werden dabei verworfen.
fn tempo_ok(st: &AppState, geraet_id: i64) -> bool {
    let jetzt = Instant::now();
    let mut map = st.schreib.lock().unwrap();
    let liste = map.entry(geraet_id).or_default();
    liste.retain(|t| jetzt.duration_since(*t).as_secs() < TEMPO_FENSTER_S);
    if liste.len() >= TEMPO_MAX {
        return false;
    }
    liste.push(jetzt);
    true
}

#[tokio::main]
async fn main() {
    // Hilfsmodus zum Einrichten des Admin-Zugangs (TOTP):
    //   server totp
    // Erzeugt ein neues Geheimnis und gibt es als ADMIN_TOTP_SECRET sowie
    // als otpauth://-URL aus. Das Geheimnis kommt in die server/.env, die
    // URL scannst du EINMAL mit einer Authenticator-App (Google
    // Authenticator, Aegis, 1Password …).
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("totp") {
        let secret = Secret::generate_secret();
        let b32 = match secret.to_encoded() {
            Secret::Encoded(s) => s,
            Secret::Raw(b) => Secret::Raw(b).to_encoded().to_string(),
        };
        let totp = totp_bauen(secret.to_bytes().expect("Secret-Bytes"))
            .expect("TOTP-Aufbau fehlgeschlagen");
        let url = totp.get_url();
        println!("ADMIN_TOTP_SECRET={b32}");
        println!();
        println!("{url}");
        println!();
        // QR-Code zum Scannen direkt im Terminal (offline). Falls die
        // Erzeugung scheitert, bleibt die manuelle Eingabe des Geheimnisses.
        match qrcode::QrCode::new(url.as_bytes()) {
            Ok(code) => {
                let bild = code
                    .render::<qrcode::render::unicode::Dense1x2>()
                    .quiet_zone(true)
                    .build();
                println!("Mit der Authenticator-App scannen:");
                println!("{bild}");
            }
            Err(_) => {
                println!("(QR-Code nicht erzeugbar – Geheimnis oben manuell eintragen.)");
            }
        }
        println!("Hinweis: ADMIN_TOTP_SECRET in server/.env eintragen und den Server neu bauen.");
        return;
    }

    // Sammler-Lauf (Etappe 4 Teil 3):
    //   server sammeln <quelle.json>
    // Vergleicht die Kandidaten aus der Quelldatei mit dem aktuellen
    // Katalog und legt offene Vorschläge an (neu/geändert). Gedacht für
    // einen wöchentlichen Cron-Aufruf; der Admin gibt sie danach in der
    // Admin-App frei.
    if args.get(1).map(|s| s.as_str()) == Some("sammeln") {
        let quelle = args.get(2).cloned().unwrap_or_default();
        if quelle.is_empty() {
            eprintln!("Aufruf: server sammeln <quelle.json>");
            std::process::exit(2);
        }
        match sammeln_lauf(&quelle).await {
            Ok((neu, geaendert)) => {
                println!("Sammler fertig: {neu} neu, {geaendert} geändert (offene Vorschläge).");
            }
            Err(e) => {
                eprintln!("Sammler-Fehler: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let db_pfad = env::var("DB_PFAD").unwrap_or_else(|_| "antrag3000.sqlite".into());
    let lausch = env::var("LAUSCH").unwrap_or_else(|_| "0.0.0.0:8080".into());

    let opts = SqliteConnectOptions::new()
        .filename(&db_pfad)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("Datenbank konnte nicht geöffnet werden");

    schema_anlegen(&pool)
        .await
        .expect("Schema konnte nicht angelegt werden");

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/board", get(board_lesen))
        .route("/api/board/:projekt_id", put(board_schreiben).delete(board_loeschen))
        .route("/api/katalog", get(katalog_lesen))
        .route("/api/katalog/version", get(katalog_version))
        .route("/api/meldung/:meldung_id", put(meldung_schreiben))
        .route("/api/foerderer", get(foerderer_lesen))
        .route("/api/foerderer/:foerderer_id", put(foerderer_schreiben).delete(foerderer_loeschen))
        // Admin (Etappe 4): Zwei-Faktor (Admin-Gerät + Passwort).
        .route("/api/admin/anmelden", post(admin_anmelden))
        .route("/api/admin/meldungen", get(admin_meldungen))
        .route("/api/admin/meldungen/:meldung_id", put(admin_meldung_status))
        .route("/api/admin/foerderer", get(admin_foerderer))
        .route("/api/admin/foerderer/:foerderer_id", delete(admin_foerderer_loeschen))
        .route("/api/admin/katalog", put(admin_katalog_hochladen))
        .route("/api/admin/vorschlaege", get(admin_vorschlaege))
        .route("/api/admin/vorschlaege/:vid/freigeben", post(admin_vorschlag_freigeben))
        .route("/api/admin/vorschlaege/:vid/verwerfen", post(admin_vorschlag_verwerfen))
        .with_state(AppState {
            pool,
            schreib: Arc::new(Mutex::new(HashMap::new())),
            sitzungen: Arc::new(Mutex::new(HashMap::new())),
        });

    let addr: SocketAddr = lausch.parse().expect("LAUSCH ist keine gültige Adresse");
    println!("Antrag-3000-Sync-Dienst lauscht intern auf {addr}");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Port konnte nicht geöffnet werden");
    axum::serve(listener, app)
        .await
        .expect("Server-Fehler");
}

/// Legt die Tabellen an (idempotent) und sät das Standard-Team-Konto.
async fn schema_anlegen(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let stmts = [
        "CREATE TABLE IF NOT EXISTS konto (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'aktiv',
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        "CREATE TABLE IF NOT EXISTS nutzer (
            id INTEGER PRIMARY KEY,
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            anzeigename TEXT NOT NULL,
            rolle TEXT NOT NULL DEFAULT 'mitglied',
            status TEXT NOT NULL DEFAULT 'aktiv',
            passwort_hash TEXT,
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        "CREATE TABLE IF NOT EXISTS geraet (
            id INTEGER PRIMARY KEY,
            nutzer_id INTEGER NOT NULL REFERENCES nutzer(id),
            bezeichnung TEXT NOT NULL,
            cert_fingerprint TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'aktiv',
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now')),
            zuletzt_gesehen TEXT
        )",
        "CREATE TABLE IF NOT EXISTS board_projekt (
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            projekt_id TEXT NOT NULL,
            inhalt_json TEXT NOT NULL,
            version INTEGER NOT NULL DEFAULT 1,
            geaendert_am TEXT NOT NULL DEFAULT (datetime('now')),
            geaendert_von_geraet INTEGER REFERENCES geraet(id),
            PRIMARY KEY (konto_id, projekt_id)
        )",
        // Gemeldete Fehler/veraltete Daten zu Katalog-Förderungen. Fliessen
        // in die Admin-Kuratierung (Etappe 4). Upsert per (konto_id, id):
        // erneutes Senden derselben Meldung überschreibt, häuft nicht an.
        "CREATE TABLE IF NOT EXISTS meldung (
            id TEXT NOT NULL,
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            geraet_id INTEGER NOT NULL REFERENCES geraet(id),
            foerderung_id TEXT NOT NULL,
            foerderung_name TEXT,
            art TEXT NOT NULL,
            text TEXT,
            status TEXT NOT NULL DEFAULT 'offen',
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now')),
            geaendert_am TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (konto_id, id)
        )",
        // Der zentral gepflegte Förder-Katalog (Etappe 4): der Admin lädt
        // ihn hoch, der Server verteilt ihn. In der DB (statt nur Datei),
        // damit der Upload ohne beschreibbares Datei-Volume funktioniert.
        "CREATE TABLE IF NOT EXISTS katalog_aktuell (
            konto_id INTEGER PRIMARY KEY REFERENCES konto(id),
            inhalt_json TEXT NOT NULL,
            stand TEXT,
            schema_version INTEGER,
            geaendert_am TEXT NOT NULL DEFAULT (datetime('now')),
            geaendert_von_geraet INTEGER REFERENCES geraet(id)
        )",
        // Sammler-Vorschläge (Etappe 4 Teil 3): der Sammler schlägt neue/
        // geänderte Förderungen vor; der Admin gibt sie frei oder verwirft
        // sie VOR der Verteilung. id = foerderung_id (ein offener Vorschlag
        // je Förderung).
        "CREATE TABLE IF NOT EXISTS vorschlag (
            id TEXT NOT NULL,
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            foerderung_id TEXT NOT NULL,
            art TEXT NOT NULL,
            inhalt_json TEXT NOT NULL,
            quelle TEXT,
            status TEXT NOT NULL DEFAULT 'offen',
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (konto_id, id)
        )",
        // Vom Team geteilte EIGENE Förderer (öffentliche Felder; die freie
        // Beschreibung bleibt lokal beim Ersteller). Upsert per (konto_id,
        // id); geraet_id zeigt, wer ihn beigetragen hat.
        "CREATE TABLE IF NOT EXISTS geteilte_foerderung (
            id TEXT NOT NULL,
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            geraet_id INTEGER NOT NULL REFERENCES geraet(id),
            inhalt_json TEXT NOT NULL,
            geaendert_am TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (konto_id, id)
        )",
        // MVP: ein gemeinsames Team-Konto + Team-Nutzer.
        "INSERT OR IGNORE INTO konto (id, name) VALUES (1, 'Team')",
        "INSERT OR IGNORE INTO nutzer (id, konto_id, anzeigename, rolle) VALUES (1, 1, 'Team-Login', 'admin')",
    ];
    for s in stmts {
        sqlx::query(s).execute(pool).await?;
    }
    // Admin-Kennzeichen am Gerät (Etappe 4). Per ALTER ergänzt, damit auch
    // bestehende Datenbanken nachgezogen werden; bei vorhandener Spalte
    // schlägt ALTER fehl – das ignorieren wir bewusst.
    let _ = sqlx::query("ALTER TABLE geraet ADD COLUMN ist_admin INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    Ok(())
}

/// Bildet den Geräte-Fingerabdruck (SHA-256-Hex des Zertifikats). Drei
/// Wege werden akzeptiert:
///  1. `Cf-Client-Cert-Sha256` – fertiger Fingerabdruck von Cloudflare
///     (per Transform Rule weitergegeben; einfachster Weg).
///  2. `Cf-Client-Cert-Der-Base64` – das DER-Zertifikat von Cloudflare.
///  3. `X-Client-Cert-DER` – das DER-Zertifikat von Caddy (DDNS-Variante).
/// None, wenn nichts Brauchbares da ist.
fn fingerprint(headers: &HeaderMap) -> Option<String> {
    // 1. Fertiger SHA-256-Fingerabdruck (Cloudflare).
    if let Some(fp) = headers.get("cf-client-cert-sha256").and_then(|v| v.to_str().ok()) {
        let norm = fp.trim().to_lowercase().replace(':', "");
        if norm.len() == 64 && norm.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(norm);
        }
    }
    // 2./3. Sonst das DER-Zertifikat selbst hashen.
    let der_b64 = headers
        .get("cf-client-cert-der-base64")
        .or_else(|| headers.get("x-client-cert-der"))?
        .to_str()
        .ok()?
        .trim()
        .to_string();
    if der_b64.is_empty() {
        return None;
    }
    let der = base64::engine::general_purpose::STANDARD.decode(der_b64).ok()?;
    let mut h = Sha256::new();
    h.update(&der);
    Some(hex::encode(h.finalize()))
}

/// Ermittelt zu einem Request das Konto (und die Geräte-id). Unbekannte,
/// aber von Caddy gegen unsere CA geprüfte Geräte werden automatisch dem
/// Standard-Team-Konto zugeordnet ("Trust on first use" – die CA bürgt
/// bereits, dass das Zertifikat von uns stammt).
async fn konto_und_geraet(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> Result<(i64, i64), StatusCode> {
    let fp = fingerprint(headers).ok_or(StatusCode::UNAUTHORIZED)?;

    if let Some((geraet_id, konto_id)) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT g.id, n.konto_id
         FROM geraet g JOIN nutzer n ON n.id = g.nutzer_id
         WHERE g.cert_fingerprint = ? AND g.status = 'aktiv'",
    )
    .bind(&fp)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let _ = sqlx::query("UPDATE geraet SET zuletzt_gesehen = datetime('now') WHERE id = ?")
            .bind(geraet_id)
            .execute(pool)
            .await;
        return Ok((konto_id, geraet_id));
    }

    // Auto-Enroll unter Konto/Nutzer 1.
    let bezeichnung = headers
        .get("x-client-cert-subject")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.chars().take(80).collect::<String>())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Gerät {}", &fp[..fp.len().min(12)]));

    let res = sqlx::query(
        "INSERT INTO geraet (nutzer_id, bezeichnung, cert_fingerprint, status)
         VALUES (1, ?, ?, 'aktiv')",
    )
    .bind(&bezeichnung)
    .bind(&fp)
    .execute(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((1, res.last_insert_rowid()))
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Serialize)]
struct BoardProjekt {
    projekt_id: String,
    inhalt: serde_json::Value,
    version: i64,
    geaendert_am: String,
}

/// Alle Board-Projekte des Kontos des aufrufenden Geräts.
async fn board_lesen(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<BoardProjekt>>, StatusCode> {
    let (konto_id, _) = konto_und_geraet(&st.pool, &headers).await?;
    let rows = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT projekt_id, inhalt_json, version, geaendert_am
         FROM board_projekt WHERE konto_id = ? ORDER BY geaendert_am DESC",
    )
    .bind(konto_id)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(projekt_id, json, version, geaendert_am)| BoardProjekt {
            projekt_id,
            inhalt: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
            version,
            geaendert_am,
        })
        .collect();
    Ok(Json(liste))
}

#[derive(Deserialize)]
struct BoardSchreiben {
    inhalt: serde_json::Value,
    /// Version, auf der die Änderung basiert (für Konflikt-Erkennung).
    basis_version: Option<i64>,
}

#[derive(Serialize)]
struct SchreibAntwort {
    version: i64,
    konflikt: bool,
    /// Bei Konflikt: der aktuelle Server-Stand, damit der Client mischen kann.
    aktuell: Option<serde_json::Value>,
}

/// Ein Board-Projekt anlegen/aktualisieren ("letzte Änderung gewinnt",
/// mit optionaler Konflikt-Erkennung über basis_version).
async fn board_schreiben(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(projekt_id): Path<String>,
    Json(body): Json<BoardSchreiben>,
) -> Result<Json<SchreibAntwort>, StatusCode> {
    let (konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;

    let aktuell = sqlx::query_as::<_, (i64, String)>(
        "SELECT version, inhalt_json FROM board_projekt WHERE konto_id = ? AND projekt_id = ?",
    )
    .bind(konto_id)
    .bind(&projekt_id)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let (Some(basis), Some((akt_ver, akt_json))) = (body.basis_version, &aktuell) {
        if basis != *akt_ver {
            return Ok(Json(SchreibAntwort {
                version: *akt_ver,
                konflikt: true,
                aktuell: serde_json::from_str(akt_json).ok(),
            }));
        }
    }

    let neue_version = aktuell.as_ref().map(|(v, _)| v + 1).unwrap_or(1);
    let inhalt_str = serde_json::to_string(&body.inhalt).map_err(|_| StatusCode::BAD_REQUEST)?;

    sqlx::query(
        "INSERT INTO board_projekt
            (konto_id, projekt_id, inhalt_json, version, geaendert_am, geaendert_von_geraet)
         VALUES (?, ?, ?, ?, datetime('now'), ?)
         ON CONFLICT(konto_id, projekt_id) DO UPDATE SET
            inhalt_json = excluded.inhalt_json,
            version = excluded.version,
            geaendert_am = excluded.geaendert_am,
            geaendert_von_geraet = excluded.geaendert_von_geraet",
    )
    .bind(konto_id)
    .bind(&projekt_id)
    .bind(&inhalt_str)
    .bind(neue_version)
    .bind(geraet_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SchreibAntwort {
        version: neue_version,
        konflikt: false,
        aktuell: None,
    }))
}

/// Ein Board-Projekt entfernen (z. B. wenn das Team es nicht mehr teilt).
async fn board_loeschen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(projekt_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, _) = konto_und_geraet(&st.pool, &headers).await?;
    sqlx::query("DELETE FROM board_projekt WHERE konto_id = ? AND projekt_id = ?")
        .bind(konto_id)
        .bind(&projekt_id)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================
// Förder-Katalog (Phase 3 / Etappe 3): der Server verteilt den
// zentral gepflegten Katalog (unkritisch). In Etappe 3 wird die Datei
// vom Admin bereitgestellt (Pfad ENV KATALOG_PFAD); die eigene Admin-
// Anwendung zum Hochladen folgt in Etappe 4. Nur Geräte mit gültigem
// Team-Zertifikat dürfen ihn abrufen.
// ============================================================

fn katalog_pfad() -> String {
    env::var("KATALOG_PFAD").unwrap_or_else(|_| "katalog.json".into())
}

/// Liefert den aktuellen Katalog-Text: zuerst aus der DB (vom Admin
/// hochgeladen), sonst aus der mitgelieferten Datei (KATALOG_PFAD). So
/// funktioniert der erste Start ohne Upload, und ab dem ersten Upload
/// gewinnt die DB-Fassung.
async fn katalog_text(pool: &SqlitePool, konto_id: i64) -> Option<String> {
    if let Ok(Some((json,))) = sqlx::query_as::<_, (String,)>(
        "SELECT inhalt_json FROM katalog_aktuell WHERE konto_id = ?",
    )
    .bind(konto_id)
    .fetch_optional(pool)
    .await
    {
        return Some(json);
    }
    tokio::fs::read_to_string(katalog_pfad()).await.ok()
}

/// Liefert den aktuellen Förder-Katalog (rohes JSON).
async fn katalog_lesen(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, [(axum::http::HeaderName, &'static str); 1], String), StatusCode> {
    let (konto_id, _) = konto_und_geraet(&st.pool, &headers).await?;
    let text = katalog_text(&st.pool, konto_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        text,
    ))
}

#[derive(Serialize)]
struct KatalogVersion {
    stand: Option<String>,
    schema_version: Option<i64>,
}

/// Liefert nur Stand/Version des Katalogs (für schnelle „Gibt es Neues?"-
/// Abfragen, ohne den ganzen Katalog zu übertragen).
async fn katalog_version(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<KatalogVersion>, StatusCode> {
    let (konto_id, _) = konto_und_geraet(&st.pool, &headers).await?;
    let text = katalog_text(&st.pool, konto_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(KatalogVersion {
        stand: v.get("stand").and_then(|x| x.as_str()).map(|s| s.to_string()),
        schema_version: v.get("schema_version").and_then(|x| x.as_i64()),
    }))
}

// ============================================================
// Meldungen (Phase 3 / Etappe 3 Teil 2): Nutzer:innen melden falsche/
// veraltete Förderungen. Die Meldungen fliessen in die Admin-Kuratierung.
// Hier greift der serverseitige Spam-Schutz (Tempo-Limit, Kontingent,
// Größen-Limit, Upsert per id) – der Client ist nicht vertrauenswürdig.
// ============================================================

#[derive(Deserialize)]
struct MeldungBody {
    #[serde(rename = "foerderungId")]
    foerderung_id: String,
    #[serde(rename = "foerderungName")]
    foerderung_name: Option<String>,
    art: String,
    text: Option<String>,
}

/// Eine Meldung anlegen/aktualisieren (PUT /api/meldung/{id}). Die id wird
/// vom Client vergeben; erneutes Senden überschreibt denselben Eintrag
/// (kein Anhäufen). Antworten: 204 ok, 429 zu schnell, 413 zu gross,
/// 403 Kontingent voll, 400 ungültig.
async fn meldung_schreiben(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(meldung_id): Path<String>,
    Json(body): Json<MeldungBody>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;

    // 1. Tempo-Bremse (Schreibvorgänge/Minute je Gerät).
    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // 2. Größen-Limit (Datensatz/Feld).
    if meldung_id.len() > FELD_MAX
        || body.foerderung_id.len() > FELD_MAX
        || body.art.len() > FELD_MAX
        || body.foerderung_name.as_deref().unwrap_or("").len() > FELD_MAX
        || body.text.as_deref().unwrap_or("").len() > TEXT_MAX
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if body.foerderung_id.trim().is_empty() || body.art.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // 3. Kontingent: Zahl der OFFENEN Meldungen je Gerät begrenzen. Ein
    //    Upsert der gleichen id zählt nicht mit (id <> ?).
    let offen: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM meldung WHERE geraet_id = ? AND status = 'offen' AND id <> ?",
    )
    .bind(geraet_id)
    .bind(&meldung_id)
    .fetch_one(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if offen >= MELDUNG_QUOTA {
        return Err(StatusCode::FORBIDDEN);
    }

    // 4. Upsert per (konto_id, id).
    sqlx::query(
        "INSERT INTO meldung
            (id, konto_id, geraet_id, foerderung_id, foerderung_name, art, text, status, erstellt_am, geaendert_am)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'offen', datetime('now'), datetime('now'))
         ON CONFLICT(konto_id, id) DO UPDATE SET
            foerderung_id = excluded.foerderung_id,
            foerderung_name = excluded.foerderung_name,
            art = excluded.art,
            text = excluded.text,
            geaendert_am = excluded.geaendert_am",
    )
    .bind(&meldung_id)
    .bind(konto_id)
    .bind(geraet_id)
    .bind(body.foerderung_id.trim())
    .bind(body.foerderung_name.as_deref().unwrap_or("").trim())
    .bind(body.art.trim())
    .bind(body.text.as_deref().unwrap_or("").trim())
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

// ============================================================
// Geteilte eigene Förderer (Phase 3 / Etappe 3 Teil 2b): das Team teilt
// die ÖFFENTLICHEN Felder selbst recherchierter Förderer. Die freie
// Beschreibung bleibt lokal (Client sendet sie gar nicht erst). Auch hier
// greift der serverseitige Spam-Schutz.
// ============================================================

#[derive(Serialize)]
struct GeteilteFoerderung {
    id: String,
    geraet_id: i64,
    inhalt: serde_json::Value,
    geaendert_am: String,
}

/// Alle geteilten Förderer des Kontos (GET /api/foerderer).
async fn foerderer_lesen(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<GeteilteFoerderung>>, StatusCode> {
    let (konto_id, _) = konto_und_geraet(&st.pool, &headers).await?;
    let rows = sqlx::query_as::<_, (String, i64, String, String)>(
        "SELECT id, geraet_id, inhalt_json, geaendert_am
         FROM geteilte_foerderung WHERE konto_id = ? ORDER BY geaendert_am DESC",
    )
    .bind(konto_id)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(id, geraet_id, json, geaendert_am)| GeteilteFoerderung {
            id,
            geraet_id,
            inhalt: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
            geaendert_am,
        })
        .collect();
    Ok(Json(liste))
}

#[derive(Deserialize)]
struct FoerdererBody {
    inhalt: serde_json::Value,
}

/// Einen eigenen Förderer teilen/aktualisieren (PUT /api/foerderer/{id}).
/// Upsert per id; nur das eigene Gerät darf seinen Eintrag ändern.
/// Antworten wie bei Meldungen (429/413/403/400).
async fn foerderer_schreiben(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(foerderer_id): Path<String>,
    Json(body): Json<FoerdererBody>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;

    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    let inhalt_str = serde_json::to_string(&body.inhalt).map_err(|_| StatusCode::BAD_REQUEST)?;
    if foerderer_id.len() > FELD_MAX || inhalt_str.len() > FOERDERER_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    // Kontingent: Zahl der geteilten Förderer dieses Geräts begrenzen
    // (Upsert der gleichen id zählt nicht mit).
    let anzahl: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM geteilte_foerderung WHERE geraet_id = ? AND id <> ?",
    )
    .bind(geraet_id)
    .bind(&foerderer_id)
    .fetch_one(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if anzahl >= FOERDERER_QUOTA {
        return Err(StatusCode::FORBIDDEN);
    }

    // Upsert nur auf den EIGENEN Eintrag: Ein anderes Gerät darf einen
    // fremden Datensatz nicht überschreiben (geraet_id im WHERE der
    // Update-Klausel ist über das ON CONFLICT nicht möglich; daher prüfen
    // wir vorher, ob die id schon einem anderen Gerät gehört).
    if let Some(besitzer) = sqlx::query_scalar::<_, i64>(
        "SELECT geraet_id FROM geteilte_foerderung WHERE konto_id = ? AND id = ?",
    )
    .bind(konto_id)
    .bind(&foerderer_id)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        if besitzer != geraet_id {
            return Err(StatusCode::FORBIDDEN);
        }
    }

    sqlx::query(
        "INSERT INTO geteilte_foerderung (id, konto_id, geraet_id, inhalt_json, geaendert_am)
         VALUES (?, ?, ?, ?, datetime('now'))
         ON CONFLICT(konto_id, id) DO UPDATE SET
            inhalt_json = excluded.inhalt_json,
            geaendert_am = excluded.geaendert_am",
    )
    .bind(&foerderer_id)
    .bind(konto_id)
    .bind(geraet_id)
    .bind(&inhalt_str)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

/// Einen eigenen geteilten Förderer zurückziehen (DELETE
/// /api/foerderer/{id}). Nur das eigene Gerät darf löschen.
async fn foerderer_loeschen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(foerderer_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;
    sqlx::query(
        "DELETE FROM geteilte_foerderung WHERE konto_id = ? AND id = ? AND geraet_id = ?",
    )
    .bind(konto_id)
    .bind(&foerderer_id)
    .bind(geraet_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

// ============================================================
// Admin (Phase 3 / Etappe 4): zentrale Pflege der Förder-Datenbank.
//
// Zwei-Faktor-Schutz, weil ein Admin viel bewegen kann:
//  1. BESITZ Team-Gerät: ein gültiges Team-Zertifikat (mTLS).
//  2. BESITZ Authenticator: ein gültiger TOTP-Code (Header X-Admin-Code)
//     gegen das Geheimnis aus ENV ADMIN_TOTP_SECRET.
// Beim Anmelden werden beide geprüft; danach hält ein kurzlebiges
// Sitzungs-Token (Header X-Admin-Token) die Sitzung, weil TOTP-Codes nach
// 30 s ablaufen. Ohne ADMIN_TOTP_SECRET ist der Admin-Zugang deaktiviert.
// ============================================================

/// Baut ein TOTP-Objekt aus rohen Geheimnis-Bytes (SHA1, 6 Stellen,
/// 30-s-Schritt, ±1 Schritt Toleranz – die üblichen Authenticator-Werte).
fn totp_bauen(bytes: Vec<u8>) -> Result<TOTP, String> {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        bytes,
        Some("Antrag 3000".to_string()),
        "Admin".to_string(),
    )
    .map_err(|e| e.to_string())
}

/// Liest das Admin-TOTP aus der Umgebung (None = Admin deaktiviert).
fn totp_aus_env() -> Option<TOTP> {
    let b32 = env::var("ADMIN_TOTP_SECRET").ok().filter(|s| !s.trim().is_empty())?;
    let bytes = Secret::Encoded(b32.trim().to_string()).to_bytes().ok()?;
    totp_bauen(bytes).ok()
}

/// Prüft einen TOTP-Code (6 Ziffern) gegen das Admin-Geheimnis.
fn totp_ok(code: &str) -> bool {
    match totp_aus_env() {
        Some(t) => t.check_current(code.trim()).unwrap_or(false),
        None => false,
    }
}

/// Legt eine neue Admin-Sitzung an und gibt ihr Token zurück. Das Token
/// ist ein zufälliges 160-Bit-Geheimnis (base32).
fn sitzung_anlegen(st: &AppState, konto_id: i64, geraet_id: i64) -> String {
    let token = match Secret::generate_secret().to_encoded() {
        Secret::Encoded(s) => s,
        Secret::Raw(b) => Secret::Raw(b).to_encoded().to_string(),
    };
    let mut map = st.sitzungen.lock().unwrap();
    let jetzt = Instant::now();
    map.retain(|_, s| s.ablauf > jetzt); // abgelaufene aufräumen
    map.insert(
        token.clone(),
        AdminSitzung {
            konto_id,
            geraet_id,
            ablauf: jetzt + Duration::from_secs(SITZUNG_TTL_S),
        },
    );
    token
}

/// Prüft das Sitzungs-Token aus dem Header X-Admin-Token. Gibt
/// (konto_id, geraet_id) zurück oder 401, wenn keine gültige Sitzung.
async fn admin_pruefen(st: &AppState, headers: &HeaderMap) -> Result<(i64, i64), StatusCode> {
    // Das Zertifikat (Faktor 1) wird ohnehin von Caddy/mTLS erzwungen.
    let token = headers
        .get("x-admin-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut map = st.sitzungen.lock().unwrap();
    let jetzt = Instant::now();
    map.retain(|_, s| s.ablauf > jetzt);
    match map.get(&token) {
        Some(s) => Ok((s.konto_id, s.geraet_id)),
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Deserialize)]
struct AnmeldeBody {
    code: String,
}

#[derive(Serialize)]
struct AnmeldeAntwort {
    token: String,
    gueltig_s: u64,
}

/// Admin-Login (POST /api/admin/anmelden): prüft den TOTP-Code, markiert
/// das aufrufende Gerät als Admin und gibt ein Sitzungs-Token zurück, das
/// die App danach als Header X-Admin-Token mitschickt. Tempo-Bremse gegen
/// Code-Raten.
async fn admin_anmelden(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AnmeldeBody>,
) -> Result<Json<AnmeldeAntwort>, StatusCode> {
    let (konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;
    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if body.code.trim().is_empty() || !totp_ok(&body.code) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    sqlx::query("UPDATE geraet SET ist_admin = 1 WHERE id = ?")
        .bind(geraet_id)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let token = sitzung_anlegen(&st, konto_id, geraet_id);
    Ok(Json(AnmeldeAntwort { token, gueltig_s: SITZUNG_TTL_S }))
}

#[derive(Serialize)]
struct MeldungZeile {
    id: String,
    geraet_id: i64,
    foerderung_id: String,
    foerderung_name: Option<String>,
    art: String,
    text: Option<String>,
    status: String,
    erstellt_am: String,
    geaendert_am: String,
}

/// Alle Meldungen des Kontos (GET /api/admin/meldungen) – die Eingangsbox
/// der Kuratierung. Nur für Admins.
async fn admin_meldungen(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<MeldungZeile>>, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    let rows = sqlx::query_as::<_, (String, i64, String, Option<String>, String, Option<String>, String, String, String)>(
        "SELECT id, geraet_id, foerderung_id, foerderung_name, art, text, status, erstellt_am, geaendert_am
         FROM meldung WHERE konto_id = ? ORDER BY geaendert_am DESC",
    )
    .bind(konto_id)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(id, geraet_id, foerderung_id, foerderung_name, art, text, status, erstellt_am, geaendert_am)| {
            MeldungZeile { id, geraet_id, foerderung_id, foerderung_name, art, text, status, erstellt_am, geaendert_am }
        })
        .collect();
    Ok(Json(liste))
}

#[derive(Deserialize)]
struct StatusBody {
    status: String,
}

/// Setzt den Status einer Meldung (PUT /api/admin/meldungen/{id}). Erlaubt:
/// offen, erledigt, verworfen. Nur für Admins.
async fn admin_meldung_status(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(meldung_id): Path<String>,
    Json(body): Json<StatusBody>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    if !["offen", "erledigt", "verworfen"].contains(&body.status.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let res = sqlx::query(
        "UPDATE meldung SET status = ?, geaendert_am = datetime('now')
         WHERE konto_id = ? AND id = ?",
    )
    .bind(&body.status)
    .bind(konto_id)
    .bind(&meldung_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Entfernt einen geteilten Förderer (DELETE /api/admin/foerderer/{id}).
/// Anders als der Nutzer-Endpunkt darf der Admin JEDEN Eintrag des Kontos
/// löschen (z. B. einen unpassenden). Nur für Admins.
async fn admin_foerderer_loeschen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(foerderer_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    sqlx::query("DELETE FROM geteilte_foerderung WHERE konto_id = ? AND id = ?")
        .bind(konto_id)
        .bind(&foerderer_id)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Alle geteilten Förderer des Kontos (GET /api/admin/foerderer) – für die
/// Kuratierung. Nutzt dieselbe Form wie /api/foerderer, aber Admin-geschützt.
async fn admin_foerderer(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<GeteilteFoerderung>>, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    let rows = sqlx::query_as::<_, (String, i64, String, String)>(
        "SELECT id, geraet_id, inhalt_json, geaendert_am
         FROM geteilte_foerderung WHERE konto_id = ? ORDER BY geaendert_am DESC",
    )
    .bind(konto_id)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(id, geraet_id, json, geaendert_am)| GeteilteFoerderung {
            id,
            geraet_id,
            inhalt: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
            geaendert_am,
        })
        .collect();
    Ok(Json(liste))
}

#[derive(Serialize)]
struct KatalogHochladenAntwort {
    stand: Option<String>,
    schema_version: Option<i64>,
    anzahl: usize,
}

/// Lädt einen neuen Gesamt-Katalog hoch (PUT /api/admin/katalog). Body =
/// das rohe Katalog-JSON. Wird geprüft (Objekt mit schema_version und
/// Liste foerderungen, jede mit id+name) und in der DB abgelegt; ab dann
/// liefert GET /api/katalog diese Fassung. Nur für Admins.
async fn admin_katalog_hochladen(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<KatalogHochladenAntwort>, StatusCode> {
    let (konto_id, geraet_id) = admin_pruefen(&st, &headers).await?;

    if body.len() > KATALOG_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Mindest-Prüfung (wie der Client beim Einspielen): Objekt mit
    // schema_version und einer Liste foerderungen, jede mit id und name.
    let schema_version = v.get("schema_version").and_then(|x| x.as_i64());
    let foerderungen = v.get("foerderungen").and_then(|x| x.as_array());
    let (Some(_), Some(liste)) = (schema_version, foerderungen) else {
        return Err(StatusCode::BAD_REQUEST);
    };
    for f in liste {
        let hat_id = f.get("id").and_then(|x| x.as_str()).is_some_and(|s| !s.is_empty());
        let hat_name = f.get("name").and_then(|x| x.as_str()).is_some_and(|s| !s.is_empty());
        if !hat_id || !hat_name {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let stand = v.get("stand").and_then(|x| x.as_str()).map(|s| s.to_string());
    let anzahl = liste.len();

    sqlx::query(
        "INSERT INTO katalog_aktuell
            (konto_id, inhalt_json, stand, schema_version, geaendert_am, geaendert_von_geraet)
         VALUES (?, ?, ?, ?, datetime('now'), ?)
         ON CONFLICT(konto_id) DO UPDATE SET
            inhalt_json = excluded.inhalt_json,
            stand = excluded.stand,
            schema_version = excluded.schema_version,
            geaendert_am = excluded.geaendert_am,
            geaendert_von_geraet = excluded.geaendert_von_geraet",
    )
    .bind(konto_id)
    .bind(&body)
    .bind(&stand)
    .bind(schema_version)
    .bind(geraet_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(KatalogHochladenAntwort { stand, schema_version, anzahl }))
}

// ============================================================
// Sammler-Vorschläge (Etappe 4 Teil 3): der Sammler (CLI `server sammeln`)
// legt Vorschläge an; der Admin gibt sie hier frei (in den Katalog
// übernehmen) oder verwirft sie.
// ============================================================

/// Ein Sammler-Lauf: liest Kandidaten aus der Quelldatei, vergleicht mit
/// dem aktuellen Katalog (Konto 1) und legt offene Vorschläge an. Gibt
/// (neu, geaendert) zurück. Wird von `server sammeln <quelle.json>`
/// aufgerufen (z. B. wöchentlich per Cron).
async fn sammeln_lauf(quelle_pfad: &str) -> Result<(usize, usize), String> {
    let db_pfad = env::var("DB_PFAD").unwrap_or_else(|_| "antrag3000.sqlite".into());
    let opts = SqliteConnectOptions::new().filename(&db_pfad).create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .map_err(|e| format!("Datenbank: {e}"))?;
    schema_anlegen(&pool).await.map_err(|e| format!("Schema: {e}"))?;

    let roh = tokio::fs::read_to_string(quelle_pfad)
        .await
        .map_err(|e| format!("Quelle nicht lesbar: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&roh).map_err(|e| format!("Quelle ist kein JSON: {e}"))?;
    let kandidaten: Vec<serde_json::Value> = if let Some(a) = v.as_array() {
        a.clone()
    } else if let Some(a) = v.get("foerderungen").and_then(|x| x.as_array()) {
        a.clone()
    } else {
        return Err("Quelle enthält keine Förderungs-Liste.".into());
    };

    let konto_id = 1i64;
    let katalog_roh = katalog_text(&pool, konto_id).await.unwrap_or_else(|| "{}".into());
    let katalog_v: serde_json::Value =
        serde_json::from_str(&katalog_roh).unwrap_or(serde_json::Value::Null);
    let katalog_liste: Vec<serde_json::Value> = katalog_v
        .get("foerderungen")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();

    let vorschlaege = vorschlaege_bilden(&katalog_liste, &kandidaten);
    let quelle_name = std::path::Path::new(quelle_pfad)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("sammler")
        .to_string();

    let mut neu = 0usize;
    let mut geaendert = 0usize;
    for (art, fid, inhalt) in &vorschlaege {
        let inhalt_str = inhalt.to_string();
        // Einen bestehenden Vorschlag mit gleichem Inhalt nicht anrühren –
        // sonst würde ein bereits verworfener wieder geöffnet.
        let vorhanden: Option<(String,)> =
            sqlx::query_as("SELECT inhalt_json FROM vorschlag WHERE konto_id = ? AND id = ?")
                .bind(konto_id)
                .bind(fid)
                .fetch_optional(&pool)
                .await
                .map_err(|e| e.to_string())?;
        if let Some((alt,)) = &vorhanden {
            if alt == &inhalt_str {
                continue;
            }
        }
        sqlx::query(
            "INSERT INTO vorschlag (id, konto_id, foerderung_id, art, inhalt_json, quelle, status, erstellt_am)
             VALUES (?, ?, ?, ?, ?, ?, 'offen', datetime('now'))
             ON CONFLICT(konto_id, id) DO UPDATE SET
                art = excluded.art, inhalt_json = excluded.inhalt_json,
                quelle = excluded.quelle, status = 'offen', erstellt_am = datetime('now')",
        )
        .bind(fid)
        .bind(konto_id)
        .bind(fid)
        .bind(art)
        .bind(&inhalt_str)
        .bind(&quelle_name)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
        if art == "neu" {
            neu += 1;
        } else {
            geaendert += 1;
        }
    }
    Ok((neu, geaendert))
}

#[derive(Serialize)]
struct VorschlagZeile {
    id: String,
    foerderung_id: String,
    art: String,
    inhalt: serde_json::Value,
    quelle: Option<String>,
    erstellt_am: String,
}

/// Offene Vorschläge (GET /api/admin/vorschlaege).
async fn admin_vorschlaege(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VorschlagZeile>>, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String)>(
        "SELECT id, foerderung_id, art, inhalt_json, quelle, erstellt_am
         FROM vorschlag WHERE konto_id = ? AND status = 'offen' ORDER BY erstellt_am DESC",
    )
    .bind(konto_id)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(id, foerderung_id, art, json, quelle, erstellt_am)| VorschlagZeile {
            id,
            foerderung_id,
            art,
            inhalt: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
            quelle,
            erstellt_am,
        })
        .collect();
    Ok(Json(liste))
}

/// Übernimmt eine (neue oder geänderte) Förderung in den verteilten
/// Katalog (Tabelle katalog_aktuell): ersetzt den Eintrag gleicher id oder
/// hängt ihn an, setzt `stand` auf heute.
async fn katalog_eintrag_anwenden(
    pool: &SqlitePool,
    konto_id: i64,
    geraet_id: i64,
    foerderung: serde_json::Value,
) -> Result<(), StatusCode> {
    let text = katalog_text(pool, konto_id).await.ok_or(StatusCode::NOT_FOUND)?;
    let mut v: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let fid = foerderung
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_string();
    let arr = v
        .get_mut("foerderungen")
        .and_then(|x| x.as_array_mut())
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(pos) = arr
        .iter()
        .position(|f| f.get("id").and_then(|x| x.as_str()) == Some(fid.as_str()))
    {
        arr[pos] = foerderung;
    } else {
        arr.push(foerderung);
    }
    // Stand auf heute setzen (Datum aus SQLite, ohne Extra-Abhängigkeit).
    let (heute,): (String,) = sqlx::query_as("SELECT date('now')")
        .fetch_one(pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    v["stand"] = serde_json::Value::String(heute);

    let neu_str = v.to_string();
    let stand = v.get("stand").and_then(|x| x.as_str()).map(|s| s.to_string());
    let schema_version = v.get("schema_version").and_then(|x| x.as_i64());
    sqlx::query(
        "INSERT INTO katalog_aktuell
            (konto_id, inhalt_json, stand, schema_version, geaendert_am, geaendert_von_geraet)
         VALUES (?, ?, ?, ?, datetime('now'), ?)
         ON CONFLICT(konto_id) DO UPDATE SET
            inhalt_json = excluded.inhalt_json, stand = excluded.stand,
            schema_version = excluded.schema_version, geaendert_am = excluded.geaendert_am,
            geaendert_von_geraet = excluded.geaendert_von_geraet",
    )
    .bind(konto_id)
    .bind(&neu_str)
    .bind(&stand)
    .bind(schema_version)
    .bind(geraet_id)
    .execute(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(())
}

/// Vorschlag freigeben (POST /api/admin/vorschlaege/{id}/freigeben):
/// übernimmt ihn in den Katalog und markiert ihn als freigegeben.
async fn admin_vorschlag_freigeben(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(vid): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, geraet_id) = admin_pruefen(&st, &headers).await?;
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT inhalt_json FROM vorschlag WHERE konto_id = ? AND id = ? AND status = 'offen'",
    )
    .bind(konto_id)
    .bind(&vid)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some((inhalt_str,)) = row else {
        return Err(StatusCode::NOT_FOUND);
    };
    let foerderung: serde_json::Value =
        serde_json::from_str(&inhalt_str).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    katalog_eintrag_anwenden(&st.pool, konto_id, geraet_id, foerderung).await?;

    sqlx::query("UPDATE vorschlag SET status = 'freigegeben' WHERE konto_id = ? AND id = ?")
        .bind(konto_id)
        .bind(&vid)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Vorschlag verwerfen (POST /api/admin/vorschlaege/{id}/verwerfen).
async fn admin_vorschlag_verwerfen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(vid): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    let res = sqlx::query("UPDATE vorschlag SET status = 'verworfen' WHERE konto_id = ? AND id = ?")
        .bind(konto_id)
        .bind(&vid)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Erzeugt ein Geheimnis wie der `totp`-Hilfsmodus, bildet den aktuell
    /// gültigen Code und prüft, dass totp_ok ihn akzeptiert und falsche
    /// Codes ablehnt.
    #[test]
    fn admin_totp_roundtrip() {
        let secret = Secret::generate_secret();
        let b32 = match secret.to_encoded() {
            Secret::Encoded(s) => s,
            Secret::Raw(b) => Secret::Raw(b).to_encoded().to_string(),
        };
        std::env::set_var("ADMIN_TOTP_SECRET", &b32);

        let totp = totp_bauen(secret.to_bytes().unwrap()).unwrap();
        let code = totp.generate_current().unwrap();
        assert!(totp_ok(&code));
        assert!(!totp_ok("000000"));
        assert!(!totp_ok(""));

        // Ohne gesetztes Geheimnis ist der Admin-Zugang deaktiviert.
        std::env::remove_var("ADMIN_TOTP_SECRET");
        assert!(!totp_ok(&code));
    }

    /// Der Sammler erkennt neue, geänderte und unveränderte Förderungen.
    #[test]
    fn sammler_vorschlaege() {
        let katalog = vec![
            serde_json::json!({ "id": "a", "name": "Alpha", "foerderhoehe_text": "1000" }),
            serde_json::json!({ "id": "b", "name": "Beta" }),
        ];
        let kandidaten = vec![
            // unverändert -> kein Vorschlag
            serde_json::json!({ "id": "a", "name": "Alpha", "foerderhoehe_text": "1000" }),
            // geändert -> Vorschlag
            serde_json::json!({ "id": "b", "name": "Beta", "foerderhoehe_text": "2000" }),
            // neu -> Vorschlag
            serde_json::json!({ "id": "c", "name": "Gamma" }),
            // ohne name -> ignoriert
            serde_json::json!({ "id": "d" }),
        ];
        let v = vorschlaege_bilden(&katalog, &kandidaten);
        assert_eq!(v.len(), 2);
        let neu: Vec<_> = v.iter().filter(|(art, _, _)| art == "neu").collect();
        let geaendert: Vec<_> = v.iter().filter(|(art, _, _)| art == "geaendert").collect();
        assert_eq!(neu.len(), 1);
        assert_eq!(neu[0].1, "c");
        assert_eq!(geaendert.len(), 1);
        assert_eq!(geaendert[0].1, "b");
    }
}
