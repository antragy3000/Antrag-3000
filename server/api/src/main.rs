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
use std::time::Instant;

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use base64::Engine;
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
/// Längenlimit für kurze Felder (id, Name, Art).
const FELD_MAX: usize = 200;
/// Längenlimit für die freie Anmerkung einer Meldung.
const TEXT_MAX: usize = 1000;

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
    // Hilfsmodus zum Erzeugen des Admin-Passwort-Hashes:
    //   server hash "meinPasswort"
    // Den ausgegebenen Hash trägt der Admin als ENV ADMIN_PASSWORT_HASH
    // ein. So liegt nie das Klartext-Passwort auf dem Server.
    let args: Vec<String> = env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("hash") {
        let pw = args.get(2).cloned().unwrap_or_default();
        if pw.is_empty() {
            eprintln!("Aufruf: server hash <passwort>");
            std::process::exit(2);
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(pw.as_bytes(), &salt)
            .expect("Hash fehlgeschlagen")
            .to_string();
        println!("{hash}");
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
        .route("/api/admin/foerderer", get(admin_foerderer))
        .route("/api/admin/katalog", put(admin_katalog_hochladen))
        .with_state(AppState {
            pool,
            schreib: Arc::new(Mutex::new(HashMap::new())),
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
//  1. BESITZ: ein gültiges Team-Gerät (mTLS) – und dieses Gerät muss als
//     Admin hinterlegt sein (geraet.ist_admin = 1).
//  2. WISSEN: das Admin-Passwort (Header X-Admin-Passwort), geprüft gegen
//     den argon2-Hash aus ENV ADMIN_PASSWORT_HASH.
// Ist kein ADMIN_PASSWORT_HASH gesetzt, ist der Admin-Zugang deaktiviert.
// ============================================================

/// Prüft ein Klartext-Passwort gegen den hinterlegten argon2-Hash.
fn passwort_ok(klartext: &str) -> bool {
    let hash = match env::var("ADMIN_PASSWORT_HASH") {
        Ok(h) if !h.trim().is_empty() => h,
        _ => return false, // kein Passwort gesetzt → Admin deaktiviert
    };
    let parsed = match PasswordHash::new(hash.trim()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(klartext.as_bytes(), &parsed)
        .is_ok()
}

/// Beide Faktoren prüfen. Gibt (konto_id, geraet_id) zurück oder einen
/// Fehlerstatus (401 Passwort, 403 kein Admin-Gerät).
async fn admin_pruefen(st: &AppState, headers: &HeaderMap) -> Result<(i64, i64), StatusCode> {
    let (konto_id, geraet_id) = konto_und_geraet(&st.pool, headers).await?;
    // Faktor 2: Passwort-Header.
    let pw = headers
        .get("x-admin-passwort")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if pw.is_empty() || !passwort_ok(pw) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    // Faktor 1: dieses Gerät muss als Admin hinterlegt sein.
    let ist: i64 = sqlx::query_scalar("SELECT COALESCE(ist_admin, 0) FROM geraet WHERE id = ?")
        .bind(geraet_id)
        .fetch_one(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ist != 1 {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok((konto_id, geraet_id))
}

#[derive(Deserialize)]
struct AnmeldeBody {
    passwort: String,
}

/// Admin-Login (POST /api/admin/anmelden): Beweist das Passwort und macht
/// DAS AUFRUFENDE Gerät zum Admin-Gerät (bindet Faktor 1 an dieses
/// Zertifikat). Danach verlangen alle Admin-Endpunkte zusätzlich den
/// Passwort-Header. Tempo-Bremse gegen Passwort-Raten.
async fn admin_anmelden(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<AnmeldeBody>,
) -> Result<StatusCode, StatusCode> {
    let (_konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;
    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if body.passwort.is_empty() || !passwort_ok(&body.passwort) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    sqlx::query("UPDATE geraet SET ist_admin = 1 WHERE id = ?")
        .bind(geraet_id)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Erzeugt einen Hash wie der `hash`-Hilfsmodus und prüft, dass
    /// passwort_ok das richtige Passwort akzeptiert und falsche ablehnt.
    #[test]
    fn admin_passwort_roundtrip() {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(b"GeheimesAdminPasswort", &salt)
            .unwrap()
            .to_string();
        std::env::set_var("ADMIN_PASSWORT_HASH", &hash);

        assert!(passwort_ok("GeheimesAdminPasswort"));
        assert!(!passwort_ok("falsch"));
        assert!(!passwort_ok(""));

        // Ohne gesetzten Hash ist der Admin-Zugang deaktiviert.
        std::env::remove_var("ADMIN_PASSWORT_HASH");
        assert!(!passwort_ok("GeheimesAdminPasswort"));
    }
}
