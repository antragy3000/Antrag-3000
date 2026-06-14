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
// ============================================================

use std::env;
use std::net::SocketAddr;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

#[derive(Clone)]
struct AppState {
    pool: SqlitePool,
}

#[tokio::main]
async fn main() {
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
        .with_state(AppState { pool });

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
        // MVP: ein gemeinsames Team-Konto + Team-Nutzer.
        "INSERT OR IGNORE INTO konto (id, name) VALUES (1, 'Team')",
        "INSERT OR IGNORE INTO nutzer (id, konto_id, anzeigename, rolle) VALUES (1, 1, 'Team-Login', 'admin')",
    ];
    for s in stmts {
        sqlx::query(s).execute(pool).await?;
    }
    Ok(())
}

/// Bildet den Geräte-Fingerabdruck aus dem von Caddy durchgereichten
/// Client-Zertifikat (Base64-DER → SHA-256-Hex). None, wenn keins da ist.
fn fingerprint(headers: &HeaderMap) -> Option<String> {
    let der_b64 = headers.get("x-client-cert-der")?.to_str().ok()?.trim().to_string();
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
