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

mod service_ca;
use service_ca::{Ca, StufenCa};

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
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
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

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
    // Service-CA (gehostetes Modell): signiert Team-Geräte-Ausweise. None, wenn
    // sie beim Start nicht geladen/erzeugt werden konnte – dann läuft der
    // bisherige Team-CA-Sync unbeirrt weiter, nur die Auto-Signierung ist aus.
    service_ca: Arc<Option<StufenCa>>,
    // Förderer-CA (Roadmap 6): signiert Förderer-Ausweise (Förderer verbinden
    // sich online). None ⇒ Förderer-Enrollment aus, Rest unberührt.
    foerderer_ca: Arc<Option<Ca>>,
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
/// Größenlimit für ein Board-Projekt (JSON, Bytes). Großzügig – das Board
/// enthält nur Status/Fristen/Dokumenttitel, nie sensible Inhalte.
const BOARD_MAX_BYTES: usize = 262_144;
/// Größenlimit für den hochgeladenen Gesamt-Katalog (JSON, Bytes).
const KATALOG_MAX_BYTES: usize = 2_000_000;
/// Größenlimit für ein einzelnes Förderer-Logo (Data-URL, Bytes). Ein Bild bis
/// ~1,5 MB wird als base64-Data-URL rund 2 MB groß; 2,5 MB lässt etwas Luft.
const LOGO_MAX_BYTES: usize = 2_500_000;
/// Gültigkeitsdauer einer Admin-Sitzung (Sekunden).
const SITZUNG_TTL_S: u64 = 1800;
/// Längenlimit für kurze Felder (id, Name, Art).
const FELD_MAX: usize = 200;
/// Längenlimit für die freie Anmerkung einer Meldung.
const TEXT_MAX: usize = 1000;
/// Gültigkeitsdauer einer Einmal-Einladung (Sekunden). 24 h = genug Zeit, den
/// Token zu übermitteln, aber kurz genug, dass er nicht lange „herumliegt".
const EINLADUNG_TTL_S: i64 = 86_400;
/// Größenlimit für den mitgeschickten öffentlichen Geräteschlüssel (PEM).
const PUBKEY_MAX_BYTES: usize = 4096;

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

    // WAL + busy_timeout: erlaubt, dass der wöchentliche Sammler-Lauf
    // (eigener Prozess) gleichzeitig mit dem laufenden Server auf dieselbe
    // SQLite-Datei zugreift, ohne sofort an "database is locked" zu
    // scheitern. WAL = parallele Leser + ein Schreiber; busy_timeout lässt
    // einen kurzen Schreib-Stau warten statt abzubrechen.
    let opts = SqliteConnectOptions::new()
        .filename(&db_pfad)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .expect("Datenbank konnte nicht geöffnet werden");

    schema_anlegen(&pool)
        .await
        .expect("Schema konnte nicht angelegt werden");

    // Service-CA (gehostetes Modell, Schritt 2): einmal erzeugen und danach
    // nur noch laden. Ordner per ENV SERVICE_CA_DIR (Docker: eigenes
    // beschreibbares Volume, damit Caddy das öffentliche service-ca.crt
    // mounten kann); ohne ENV neben der DB-Datei. Schlägt es fehl (z. B.
    // Ordner nicht beschreibbar), läuft der Server trotzdem weiter – der
    // bisherige Team-CA-Sync ist davon nicht betroffen.
    let ca_dir = service_ca_dir(&db_pfad);
    let service_ca = match StufenCa::laden_oder_erzeugen(&ca_dir) {
        Ok(ca) => {
            println!(
                "Service-CA (2-stufig) aktiv (Zwischen-Fingerabdruck {}…).",
                &ca.fingerprint()[..ca.fingerprint().len().min(16)]
            );
            Some(ca)
        }
        Err(e) => {
            eprintln!("WARNUNG: Service-CA nicht verfügbar ({e}). Team-CA-Sync läuft weiter.");
            None
        }
    };
    // Förderer-CA (Roadmap 6): getrennter Vertrauensanker für verbundene Förderer.
    let foerderer_ca = match Ca::laden_oder_erzeugen(&ca_dir, "foerderer-ca", "Antrag 3000 Förderer-CA") {
        Ok(ca) => {
            println!(
                "Förderer-CA aktiv (Fingerabdruck {}…).",
                &ca.fingerprint()[..ca.fingerprint().len().min(16)]
            );
            Some(ca)
        }
        Err(e) => {
            eprintln!("WARNUNG: Förderer-CA nicht verfügbar ({e}). Förderer-Enrollment aus.");
            None
        }
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/board", get(board_lesen))
        .route("/api/board/:projekt_id", put(board_schreiben).delete(board_loeschen))
        .route("/api/katalog", get(katalog_lesen))
        .route("/api/katalog/version", get(katalog_version))
        // Einzelplatz-Modus (ohne mTLS): oeffentlicher Katalog-Abruf. Wird
        // ueber den offenen :8445-Kanal (Caddy) ausgeliefert. Der Katalog
        // ist unkritisch (oeffentliche Foerder-Daten), daher ohne Zertifikat.
        .route("/api/katalog-oeffentlich", get(katalog_oeffentlich))
        // Förderer-Logos (Etappe 3c): Abruf per mTLS-Team + öffentlich (Caddy
        // :8445). Upload nur durch den Admin.
        .route("/api/logos/:logo_id", get(logo_lesen))
        .route("/api/logos/:logo_id/hash", get(logo_hash))
        .route("/api/logos-oeffentlich/:logo_id", get(logo_oeffentlich))
        .route("/api/meldung/:meldung_id", put(meldung_schreiben))
        .route("/api/foerderer", get(foerderer_lesen))
        .route("/api/foerderer/:foerderer_id", put(foerderer_schreiben).delete(foerderer_loeschen))
        // Enrollment (gehostetes Modell, Schritt 3):
        //  - Einladung erstellen: nur ein Eigentümer-Gerät per mTLS (:8443).
        //  - Verbinden/Ausweis: OHNE Zertifikat, per Einmal-Token; wird über den
        //    öffentlichen :443-Kanal freigegeben (das neue Gerät hat noch kein Zert).
        .route("/api/einladung", post(einladung_erstellen))
        .route("/api/enroll", post(enroll))
        // Team erstellen (gehostet, ohne Konto/E-Mail): bootstrappt ein neues
        // Konto + Eigentümer-Gerät. Ohne Zertifikat (das erste Gerät hat noch
        // keins) – über den öffentlichen :443-Kanal, mit Missbrauchs-Bremse.
        .route("/api/team", post(team_erstellen))
        // Förderer verbinden (Roadmap 6): der Vendor lädt einen Förderer ein
        // (admin-geschützt), der Förderer löst den Token ohne Zertifikat über
        // den öffentlichen :443-Kanal ein (Förderer-CA signiert kopiersicher).
        .route("/api/admin/foerderer-einladung", post(foerderer_einladung_erstellen))
        .route("/api/foerderer-enroll", post(foerderer_enroll))
        // Förderer-Ausweis kurzlebig + Auto-Erneuerung (Roadmap 10): erneuern per
        // mTLS, grace öffentlich (abgelaufener Ausweis kommt nicht mehr durch mTLS).
        .route("/api/foerderer/ausweis-erneuern", post(foerderer_ausweis_erneuern))
        .route("/api/foerderer/ausweis-grace", post(foerderer_ausweis_grace))
        // Ein verbundener Förderer pflegt seine Programme (Roadmap 6b, mTLS):
        .route("/api/foerderer-programme", get(foerderer_programme_lesen))
        .route(
            "/api/foerderer-programme/:programm_id",
            put(foerderer_programm_schreiben).delete(foerderer_programm_loeschen),
        )
        // Mitglieder verwalten (Eigentümer, per mTLS – ohne Vendor-Admin-TOTP):
        // Team-Geräte auflisten und sperren/entsperren.
        .route("/api/mitglieder", get(mitglieder_liste))
        .route("/api/mitglieder/:geraet_id", put(mitglied_status))
        .route("/api/ausweis/erneuern", post(ausweis_erneuern))
        .route("/api/ausweis/grace", post(ausweis_grace))
        // Admin (Etappe 4): Zwei-Faktor (Admin-Gerät + Passwort).
        .route("/api/admin/anmelden", post(admin_anmelden))
        .route("/api/admin/meldungen", get(admin_meldungen))
        .route("/api/admin/meldungen/:meldung_id", put(admin_meldung_status))
        .route("/api/admin/foerderer", get(admin_foerderer))
        .route("/api/admin/foerderer/:foerderer_id", delete(admin_foerderer_loeschen))
        // Geräte-Verwaltung: auflisten + sperren/entsperren (Zertifikat-Rückruf).
        .route("/api/admin/geraete", get(admin_geraete))
        .route("/api/admin/geraete/:geraet_id", put(admin_geraet_status))
        .route("/api/admin/katalog", put(admin_katalog_hochladen))
        .route("/api/admin/logos/:logo_id", put(admin_logo_hochladen))
        .route("/api/admin/vorschlaege", get(admin_vorschlaege))
        .route("/api/admin/vorschlaege/:vid/freigeben", post(admin_vorschlag_freigeben))
        .route("/api/admin/vorschlaege/:vid/verwerfen", post(admin_vorschlag_verwerfen))
        // Förderer-Kuratierung (Roadmap 6c): verbundene Förderer + ihre
        // Programm-Änderungen sehen und in den öffentlichen Katalog freigeben.
        .route("/api/admin/foerderer-verbunden", get(admin_foerderer_verbunden))
        .route("/api/admin/foerderer-programme", get(admin_foerderer_programme))
        .route(
            "/api/admin/foerderer-programme/:foerderer_id/:programm_id/freigeben",
            post(admin_foerderer_programm_freigeben),
        )
        .with_state(AppState {
            pool,
            schreib: Arc::new(Mutex::new(HashMap::new())),
            sitzungen: Arc::new(Mutex::new(HashMap::new())),
            service_ca: Arc::new(service_ca),
            foerderer_ca: Arc::new(foerderer_ca),
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
            -- Abo-Status des Teams (gehostetes Modell): frei | testphase | aktiv | abgelaufen.
            -- Default 'aktiv' = bestehende (self-hosted) Konten bleiben ungated.
            abo_status TEXT NOT NULL DEFAULT 'aktiv',
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        "CREATE TABLE IF NOT EXISTS nutzer (
            id INTEGER PRIMARY KEY,
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            anzeigename TEXT NOT NULL,
            rolle TEXT NOT NULL DEFAULT 'mitglied',
            status TEXT NOT NULL DEFAULT 'aktiv',
            -- Team-Eigentümer (gehostetes Modell): verwaltet Mitglieder + Abo.
            ist_eigentuemer INTEGER NOT NULL DEFAULT 0,
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
        // Förderer-Logos (Etappe 3c): der Admin lädt das VOLLE Logo eines
        // verifizierten Förderers hoch (getrennt vom Katalog, damit der von
        // allen geladene Katalog schlank bleibt – nur eine kleine Vorschau
        // steckt im Katalog). inhalt = Data-URL (data:image/...;base64,...).
        // Unkritische, öffentliche Förderer-Marke.
        "CREATE TABLE IF NOT EXISTS logo (
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            logo_id TEXT NOT NULL,
            inhalt TEXT NOT NULL,
            geaendert_am TEXT NOT NULL DEFAULT (datetime('now')),
            geaendert_von_geraet INTEGER REFERENCES geraet(id),
            PRIMARY KEY (konto_id, logo_id)
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
        // Einmal-Einladungen (gehostetes Modell, Schritt 3). Der Eigentümer
        // erzeugt einen kurzlebigen Token; ein neues Gerät löst ihn EINMAL ein
        // (Token gilt als Auth, weil das Gerät noch kein Zertifikat hat).
        // Nach dem Einlösen bleiben Ausweis + Schlüssel-Fingerabdruck stehen,
        // damit ein Gerät bei verlorener Antwort denselben Ausweis idempotent
        // erneut abholen kann (sonst wäre die Einladung verbrannt).
        "CREATE TABLE IF NOT EXISTS einladung (
            token TEXT PRIMARY KEY,
            konto_id INTEGER NOT NULL REFERENCES konto(id),
            bezeichnung TEXT,
            ablauf TEXT NOT NULL,
            benutzt INTEGER NOT NULL DEFAULT 0,
            ausweis_pem TEXT,
            pubkey_fingerprint TEXT,
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        // Verbundene Förderer (Roadmap 6): NICHT Teil eines Team-Kontos, sondern
        // vom Vendor eingeladene externe Partner, die ihre Programme online
        // pflegen. Identifiziert über den Fingerabdruck ihres Förderer-CA-
        // signierten Ausweises.
        "CREATE TABLE IF NOT EXISTS foerderer (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            cert_fingerprint TEXT NOT NULL UNIQUE,
            status TEXT NOT NULL DEFAULT 'aktiv',
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now')),
            zuletzt_gesehen TEXT
        )",
        // Einmal-Einladungen für Förderer (nur der Vendor/Admin erzeugt sie).
        // Gleiche Idempotenz-/Einlöse-Logik wie die Team-Einladung.
        "CREATE TABLE IF NOT EXISTS foerderer_einladung (
            token TEXT PRIMARY KEY,
            name TEXT,
            ablauf TEXT NOT NULL,
            benutzt INTEGER NOT NULL DEFAULT 0,
            ausweis_pem TEXT,
            pubkey_fingerprint TEXT,
            erstellt_am TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        // Programme, die ein verbundener Förderer online pflegt (Roadmap 6b).
        // Herkunft = der Förderer (foerderer_id, aus dem mTLS-Ausweis). status
        // 'zurueckgezogen' ist ein Soft-Delete, damit die Kuratierung (6c) eine
        // Löschung sieht und aus dem Katalog übernimmt.
        "CREATE TABLE IF NOT EXISTS foerderer_programm (
            foerderer_id INTEGER NOT NULL REFERENCES foerderer(id),
            programm_id TEXT NOT NULL,
            inhalt_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'aktiv',
            geaendert_am TEXT NOT NULL DEFAULT (datetime('now')),
            -- Zeitpunkt der letzten Katalog-Freigabe (6c). NULL = noch nie
            -- freigegeben; geaendert_am > freigegeben_am = offene Änderung.
            freigegeben_am TEXT,
            PRIMARY KEY (foerderer_id, programm_id)
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
    // Roadmap 10 (kurzlebige Ausweise): der vorige Fingerabdruck bleibt bei der
    // Erneuerung kurz gültig, damit die Umstellung kein Gerät aussperrt, falls die
    // App die neue Antwort verliert. Idempotenter ALTER wie oben.
    let _ = sqlx::query("ALTER TABLE geraet ADD COLUMN cert_fingerprint_vorher TEXT")
        .execute(pool)
        .await;
    // Gehostetes Modell, Schritt 1: Abo-Status je Konto + Team-Eigentümer je
    // Nutzer per ALTER nachziehen (bestehende DBs). Vorhandene Spalte → ALTER
    // schlägt fehl, wird bewusst ignoriert.
    let _ = sqlx::query("ALTER TABLE konto ADD COLUMN abo_status TEXT NOT NULL DEFAULT 'aktiv'")
        .execute(pool)
        .await;
    let _ = sqlx::query("ALTER TABLE nutzer ADD COLUMN ist_eigentuemer INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;
    // Der bestehende Team-Login (Nutzer 1) ist der Eigentümer.
    let _ = sqlx::query("UPDATE nutzer SET ist_eigentuemer = 1 WHERE id = 1")
        .execute(pool)
        .await;
    // Roadmap 6c: Freigabe-Zeitstempel je Förderer-Programm nachziehen.
    let _ = sqlx::query("ALTER TABLE foerderer_programm ADD COLUMN freigegeben_am TEXT")
        .execute(pool)
        .await;
    // Roadmap 10 (kurzlebige Förderer-Ausweise): Überlappungs-Fingerabdruck für
    // die Erneuerung – wie beim geraet, verhindert Aussperren bei der Umstellung.
    let _ = sqlx::query("ALTER TABLE foerderer ADD COLUMN cert_fingerprint_vorher TEXT")
        .execute(pool)
        .await;
    Ok(())
}

/// Welcher Reverse-Proxy steht VOR diesem Dienst und liefert den geprüften
/// Zertifikat-Header? Per ENV `CERT_HEADER_MODE`:
///   - "caddy" (Standard): nur `X-Client-Cert-DER` (Caddy/Tailscale & DDNS).
///   - "cloudflare": nur `Cf-Client-Cert-Sha256` bzw. `Cf-Client-Cert-Der-Base64`.
///
/// SICHERHEIT: Der Dienst vertraut AUSSCHLIESSLICH dem Header seines eigenen
/// Proxys und ignoriert die jeweils anderen. Sonst könnte ein Client hinter
/// Caddy einen `Cf-*`-Header selbst setzen und damit einen FREMDEN
/// Fingerabdruck vortäuschen (Ownership-Checks umgehen, Limits aushebeln).
/// Zusätzlich entfernt Caddy diese Header eingehend (Defense-in-depth).
fn cert_header_mode() -> &'static str {
    static MODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    MODE.get_or_init(|| {
        env::var("CERT_HEADER_MODE")
            .unwrap_or_else(|_| "caddy".into())
            .trim()
            .to_lowercase()
    })
}

/// Bildet den Geräte-Fingerabdruck (SHA-256-Hex des Zertifikats) – nur aus
/// dem zum konfigurierten Proxy passenden Header. None, wenn nichts
/// Brauchbares (bzw. nur ein fremder Header) da ist.
fn fingerprint(headers: &HeaderMap) -> Option<String> {
    fn hash_der(der_b64: &str) -> Option<String> {
        let der_b64 = der_b64.trim();
        if der_b64.is_empty() {
            return None;
        }
        let der = base64::engine::general_purpose::STANDARD.decode(der_b64).ok()?;
        let mut h = Sha256::new();
        h.update(&der);
        Some(hex::encode(h.finalize()))
    }

    if cert_header_mode() == "cloudflare" {
        // 1. Fertiger SHA-256-Fingerabdruck (Cloudflare, per Transform Rule).
        if let Some(fp) = headers.get("cf-client-cert-sha256").and_then(|v| v.to_str().ok()) {
            let norm = fp.trim().to_lowercase().replace(':', "");
            if norm.len() == 64 && norm.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(norm);
            }
        }
        // 2. Sonst das DER-Zertifikat von Cloudflare hashen.
        return hash_der(headers.get("cf-client-cert-der-base64")?.to_str().ok()?);
    }

    // Standard (Caddy/Tailscale & DDNS): NUR der von Caddy gesetzte Header.
    hash_der(headers.get("x-client-cert-der")?.to_str().ok()?)
}

/// Darf ein UNBEKANNTES (aber CA-geprüftes) Gerät sich automatisch anmelden?
/// Per ENV `AUTO_ENROLL` (Standard an). Nach dem Einrichten des Teams kann man
/// es auf "0"/"false"/"nein" stellen, damit keine weiteren Geräte mehr von
/// selbst dazukommen ("Trust on first use" einfrieren).
fn auto_enroll_erlaubt() -> bool {
    match env::var("AUTO_ENROLL") {
        Ok(v) => !matches!(v.trim().to_lowercase().as_str(), "0" | "false" | "nein" | "off"),
        Err(_) => true,
    }
}

/// Ordner für die Service-CA-Dateien (service-ca.crt/.key). Per ENV
/// `SERVICE_CA_DIR`; sonst der Ordner der DB-Datei (fällt auf "." zurück,
/// wenn die DB keinen Ordner-Anteil hat). So liegt die CA im Docker neben
/// einem beschreibbaren Volume, das Caddy für das öffentliche Zertifikat
/// mounten kann.
fn service_ca_dir(db_pfad: &str) -> PathBuf {
    if let Ok(d) = env::var("SERVICE_CA_DIR") {
        if !d.trim().is_empty() {
            return PathBuf::from(d.trim());
        }
    }
    PathBuf::from(db_pfad)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Ist das Abo-Gating (Schritt 5) SCHARF geschaltet? Per ENV `ABO_GATING`.
/// **Standard: AUS.** Solange aus, ist jede Team-Aktion erlaubt – es gibt kein
/// sichtbares Abo-Modell, der Betrieb ist unverändert. Die vollständige Logik
/// ist trotzdem gebaut und getestet und lässt sich später mit `ABO_GATING=1`
/// aktivieren.
fn abo_gating_an() -> bool {
    matches!(
        env::var("ABO_GATING").ok().as_deref().map(str::trim),
        Some("1") | Some("true") | Some("ja") | Some("on")
    )
}

/// Liest den Abo-Status eines Kontos (None, wenn es das Konto nicht gibt).
async fn abo_status(pool: &SqlitePool, konto_id: i64) -> Option<String> {
    sqlx::query_as::<_, (String,)>("SELECT abo_status FROM konto WHERE id = ?")
        .bind(konto_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|(s,)| s)
}

/// Zahl der Geräte eines Kontos (für die „ab dem 2. Gerät"-Grenze).
async fn geraete_zahl(pool: &SqlitePool, konto_id: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM geraet g JOIN nutzer n ON n.id = g.nutzer_id WHERE n.konto_id = ?",
    )
    .bind(konto_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
}

/// REINE Gate-Entscheidung (ohne DB/Umgebung – dadurch gut testbar):
/// - Gating aus → immer erlaubt.
/// - gültiges Abo (`aktiv`/`testphase`/`frei`) → erlaubt.
/// - sonst nur **Einzelplatz** (≤ 1 Gerät) frei; ab dem 2. Gerät gesperrt.
fn abo_gate_entscheiden(gating_an: bool, status: Option<&str>, geraete_gesamt: i64) -> bool {
    if !gating_an {
        return true;
    }
    if matches!(status, Some("aktiv") | Some("testphase") | Some("frei")) {
        return true;
    }
    geraete_gesamt <= 1
}

/// Abo-Gate (Schritt 5): Darf dieses Konto eine (schreibende/erweiternde)
/// Team-Aktion ausführen? `zusatz_geraet` zählt ein gerade entstehendes Gerät
/// mit (beim Enroll = das neue Gerät; beim Einladen = das eingeladene). Solange
/// `ABO_GATING` aus ist, immer `true` (kein sichtbares Abo).
async fn abo_gate_ok(pool: &SqlitePool, konto_id: i64, zusatz_geraet: i64) -> bool {
    if !abo_gating_an() {
        return true;
    }
    let status = abo_status(pool, konto_id).await;
    let geraete = geraete_zahl(pool, konto_id).await + zusatz_geraet;
    abo_gate_entscheiden(true, status.as_deref(), geraete)
}

/// Ermittelt zu einem Request das Konto (und die Geräte-id). Unbekannte,
/// aber von Caddy gegen unsere CA geprüfte Geräte werden – sofern erlaubt –
/// automatisch dem Standard-Team-Konto zugeordnet ("Trust on first use").
async fn konto_und_geraet(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> Result<(i64, i64), StatusCode> {
    let fp = fingerprint(headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Bekanntes Gerät? (Status MIT abfragen, nicht im WHERE filtern – sonst
    // würde ein gesperrtes Gerät unten einfach neu auto-enrollt.)
    if let Some((geraet_id, konto_id, status)) = sqlx::query_as::<_, (i64, i64, String)>(
        "SELECT g.id, n.konto_id, g.status
         FROM geraet g JOIN nutzer n ON n.id = g.nutzer_id
         WHERE g.cert_fingerprint = ? OR g.cert_fingerprint_vorher = ?",
    )
    .bind(&fp)
    .bind(&fp)
    .fetch_optional(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        // Gesperrtes/inaktives Gerät: abweisen – KEIN erneutes Auto-Enroll.
        // So wirkt eine Sperre (status <> 'aktiv') tatsächlich.
        if status != "aktiv" {
            return Err(StatusCode::FORBIDDEN);
        }
        let _ = sqlx::query("UPDATE geraet SET zuletzt_gesehen = datetime('now') WHERE id = ?")
            .bind(geraet_id)
            .execute(pool)
            .await;
        return Ok((konto_id, geraet_id));
    }

    // Ein verbundener FÖRDERER darf nicht als Team-Gerät auto-enrollt werden
    // (sein Zertifikat stammt aus der Förderer-CA; er gehört in kein Team-Konto).
    let ist_foerderer: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM foerderer WHERE cert_fingerprint = ?")
            .bind(&fp)
            .fetch_optional(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ist_foerderer.is_some() {
        return Err(StatusCode::FORBIDDEN);
    }

    // Unbekanntes Gerät: nur anlegen, wenn Auto-Enroll erlaubt ist.
    if !auto_enroll_erlaubt() {
        return Err(StatusCode::FORBIDDEN);
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

/// Prüft, dass der Aufrufer ein **Eigentümer-Gerät** ist (gültiges mTLS-Gerät,
/// dessen Nutzer `ist_eigentuemer = 1` hat). Gibt (konto_id, geraet_id) zurück.
/// Basis für die Eigentümer-Aktionen (einladen, Mitglieder verwalten) – ohne
/// den Vendor-Admin-TOTP.
async fn eigentuemer_pruefen(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> Result<(i64, i64), StatusCode> {
    let (konto_id, geraet_id) = konto_und_geraet(pool, headers).await?;
    let ist_eig: i64 = sqlx::query_scalar(
        "SELECT n.ist_eigentuemer FROM geraet g JOIN nutzer n ON n.id = g.nutzer_id WHERE g.id = ?",
    )
    .bind(geraet_id)
    .fetch_one(pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ist_eig == 0 {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok((konto_id, geraet_id))
}

/// Ermittelt den verbundenen FÖRDERER aus seinem mTLS-Ausweis (Förderer-CA-
/// signiert; Caddy vertraut ihr). Gibt die foerderer_id zurück oder 403, wenn
/// der Fingerabdruck kein bekannter/aktiver Förderer ist.
async fn foerderer_aus_cert(pool: &SqlitePool, headers: &HeaderMap) -> Result<i64, StatusCode> {
    let fp = fingerprint(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let row: Option<(i64, String)> =
        sqlx::query_as(
            "SELECT id, status FROM foerderer
             WHERE cert_fingerprint = ? OR cert_fingerprint_vorher = ?",
        )
            .bind(&fp)
            .bind(&fp)
            .fetch_optional(pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some((id, status)) = row else {
        return Err(StatusCode::FORBIDDEN);
    };
    if status != "aktiv" {
        return Err(StatusCode::FORBIDDEN); // gesperrter Förderer
    }
    let _ = sqlx::query("UPDATE foerderer SET zuletzt_gesehen = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await;
    Ok(id)
}

/// Tempo-Bremsen-Schlüssel für einen Förderer. Eigener Zahlenraum (negativ,
/// unterhalb des Team-Create-Sentinels -1), damit er nicht mit Geräte-ids
/// (positiv) kollidiert.
fn foerderer_tempo_key(foerderer_id: i64) -> i64 {
    -(1000 + foerderer_id)
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

    // Abo-Gate (Schritt 5): schreibende Team-Sync-Aktion. Solange ABO_GATING
    // aus ist, immer erlaubt; scharf: ab dem 2. Gerät nur mit gültigem Abo.
    if !abo_gate_ok(&st.pool, konto_id, 0).await {
        return Err(StatusCode::PAYMENT_REQUIRED);
    }

    // Tempo-Bremse + Längenlimits (wie bei Meldungen/Förderern). Das Board
    // war bisher ungebremst und ohne Größenlimit beschreibbar.
    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if projekt_id.len() > FELD_MAX {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

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
    if inhalt_str.len() > BOARD_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

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
    let (konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;
    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
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

/// Oeffentlicher Katalog-Abruf (Einzelplatz-Modus, ohne Geraete-Zertifikat).
/// Liefert den kuratierten Katalog des Teams (Konto 1) als rohes JSON. Wird
/// nur ueber den offenen :8445-Kanal (Caddy) freigegeben; der Katalog ist
/// unkritisch (oeffentliche Foerder-Daten).
async fn katalog_oeffentlich(
    State(st): State<AppState>,
) -> Result<(StatusCode, [(axum::http::HeaderName, &'static str); 1], String), StatusCode> {
    let text = katalog_text(&st.pool, 1).await.ok_or(StatusCode::NOT_FOUND)?;
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
// Förderer-Logos (Etappe 3c): das VOLLE Logo eines verifizierten Förderers,
// getrennt vom Katalog gespeichert (der Katalog trägt nur eine kleine
// Vorschau). Upload nur durch den Admin; Abruf für Team-Geräte (mTLS) und –
// im Einzelplatz-Modus – öffentlich (Konto 1). Unkritische Förderer-Marke.
// ============================================================

/// Holt das gespeicherte Logo (Data-URL) eines Kontos.
async fn logo_text(pool: &SqlitePool, konto_id: i64, logo_id: &str) -> Option<String> {
    sqlx::query_as::<_, (String,)>("SELECT inhalt FROM logo WHERE konto_id = ? AND logo_id = ?")
        .bind(konto_id)
        .bind(logo_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|(s,)| s)
}

/// Liefert ein Förderer-Logo als Data-URL (mTLS-Team-Abruf).
async fn logo_lesen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(logo_id): Path<String>,
) -> Result<(StatusCode, [(axum::http::HeaderName, &'static str); 1], String), StatusCode> {
    let (konto_id, _) = konto_und_geraet(&st.pool, &headers).await?;
    let text = logo_text(&st.pool, konto_id, &logo_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok((StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/plain")], text))
}

/// Öffentlicher Logo-Abruf (Einzelplatz-Modus, ohne Zertifikat), Konto 1.
async fn logo_oeffentlich(
    State(st): State<AppState>,
    Path(logo_id): Path<String>,
) -> Result<(StatusCode, [(axum::http::HeaderName, &'static str); 1], String), StatusCode> {
    let text = logo_text(&st.pool, 1, &logo_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok((StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/plain")], text))
}

/// Liefert den SHA-256 (Hex) des gespeicherten Logos – eine winzige Abfrage,
/// damit der Admin VOR dem Hochladen prüfen kann, ob genau dieses Logo schon
/// (und unverändert) auf dem Server liegt, und einen doppelten Upload spart.
/// 404, wenn (noch) kein Logo hinterlegt ist.
async fn logo_hash(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(logo_id): Path<String>,
) -> Result<(StatusCode, [(axum::http::HeaderName, &'static str); 1], String), StatusCode> {
    let (konto_id, _) = konto_und_geraet(&st.pool, &headers).await?;
    let text = logo_text(&st.pool, konto_id, &logo_id)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    let hex = hex::encode(h.finalize());
    Ok((StatusCode::OK, [(axum::http::header::CONTENT_TYPE, "text/plain")], hex))
}

/// Admin lädt/aktualisiert ein Förderer-Logo (PUT /api/admin/logos/{id}).
/// Body = Data-URL (data:image/...;base64,...). Upsert per (konto_id, logo_id).
async fn admin_logo_hochladen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(logo_id): Path<String>,
    body: String,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, geraet_id) = admin_pruefen(&st, &headers).await?;
    if logo_id.is_empty() || logo_id.len() > FELD_MAX {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.len() > LOGO_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    // Nur Bild-Data-URLs zulassen (kein beliebiger Inhalt).
    if !body.starts_with("data:image/") {
        return Err(StatusCode::BAD_REQUEST);
    }
    sqlx::query(
        "INSERT INTO logo (konto_id, logo_id, inhalt, geaendert_am, geaendert_von_geraet)
         VALUES (?, ?, ?, datetime('now'), ?)
         ON CONFLICT(konto_id, logo_id) DO UPDATE SET
            inhalt = excluded.inhalt,
            geaendert_am = excluded.geaendert_am,
            geaendert_von_geraet = excluded.geaendert_von_geraet",
    )
    .bind(konto_id)
    .bind(&logo_id)
    .bind(&body)
    .bind(geraet_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
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
// Enrollment (gehostetes Modell, Schritt 3): die vier Verben „aktivieren"
// (Einladung) und „verbinden" (Ausweis holen). Der Dienst ist der
// Vertrauensanker: die Service-CA signiert den Ausweis automatisch, sobald
// Einladung + Abo stimmen. Der private Geräteschlüssel bleibt lokal.
// ============================================================

/// Erzeugt einen zufälligen Einladungs-Token (160-Bit, base32) – dieselbe
/// unratbare Geheimnis-Quelle wie die Admin-Sitzungs-Token.
fn neuer_token() -> String {
    match Secret::generate_secret().to_encoded() {
        Secret::Encoded(s) => s,
        Secret::Raw(b) => Secret::Raw(b).to_encoded().to_string(),
    }
}

/// SHA-256-Hex eines (getrimmten) öffentlichen Schlüssels – bindet eine
/// eingelöste Einladung an genau diesen Schlüssel (idempotentes Abholen).
fn pubkey_fingerprint(pubkey_pem: &str) -> String {
    let mut h = Sha256::new();
    h.update(pubkey_pem.trim().as_bytes());
    hex::encode(h.finalize())
}

#[derive(Deserialize)]
struct EinladungBody {
    /// Optionaler Namens-Hinweis fürs einzuladende Gerät (nur Information).
    name: Option<String>,
}

#[derive(Serialize)]
struct EinladungAntwort {
    token: String,
    ablauf: String,
    /// Trust-Anker des gehosteten Modells (Service-CA-Zertifikat), damit der
    /// Eigentümer daraus ein vollständiges Zugangs-Paket bauen kann.
    service_ca_pem: String,
}

/// Einladung erstellen (POST /api/einladung). Nur ein EIGENTÜMER-Gerät (per
/// mTLS erkannt) darf einladen, und nur bei gültigem Abo (Einladen = die
/// bezahlte Team-Funktion). Läuft über den mTLS-Kanal (:8443).
async fn einladung_erstellen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EinladungBody>,
) -> Result<Json<EinladungAntwort>, StatusCode> {
    // Nur der Team-Eigentümer darf einladen.
    let (konto_id, geraet_id) = eigentuemer_pruefen(&st.pool, &headers).await?;

    // Abo-Gate (Schritt 5): Einladen zielt auf ein weiteres Gerät (zusatz = 1),
    // ist also die Aktion, die ein Konto zum „Team" macht. Solange ABO_GATING
    // aus ist, immer erlaubt.
    if !abo_gate_ok(&st.pool, konto_id, 1).await {
        return Err(StatusCode::PAYMENT_REQUIRED);
    }
    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Service-CA muss verfügbar sein, sonst könnte niemand den Token einlösen.
    let service_ca: &Option<StufenCa> = &st.service_ca;
    let ca = service_ca.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let name = body.name.as_deref().unwrap_or("").trim();
    if name.len() > FELD_MAX {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let name_opt = if name.is_empty() { None } else { Some(name) };

    let token = neuer_token();
    sqlx::query(
        "INSERT INTO einladung (token, konto_id, bezeichnung, ablauf)
         VALUES (?, ?, ?, datetime('now', ?))",
    )
    .bind(&token)
    .bind(konto_id)
    .bind(name_opt)
    .bind(format!("+{EINLADUNG_TTL_S} seconds"))
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (ablauf,): (String,) = sqlx::query_as("SELECT ablauf FROM einladung WHERE token = ?")
        .bind(&token)
        .fetch_one(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(EinladungAntwort {
        token,
        ablauf,
        service_ca_pem: ca.wurzel_cert_pem.clone(),
    }))
}

#[derive(Deserialize)]
struct EnrollBody {
    token: String,
    /// Der ÖFFENTLICHE Geräteschlüssel (SubjectPublicKeyInfo-PEM). Der private
    /// Teil bleibt auf dem Gerät und wird nie gesendet.
    #[serde(rename = "pubkeyPem", alias = "pubkey_pem")]
    pubkey_pem: String,
    name: Option<String>,
}

#[derive(Serialize, Debug)]
struct EnrollAntwort {
    /// Der frisch signierte Geräte-Ausweis (Zertifikat-PEM).
    ausweis_pem: String,
    /// Service-CA-Zertifikat (Trust-Anker des Modells).
    ///
    /// SCHRITT-4-NAHT: Womit der Client den SERVER prüft, wird erst in Schritt 4
    /// verdrahtet. Der :8443-Server-Cert ist derzeit Team-CA-signiert; damit ein
    /// rein Service-CA-Gerät den Server validiert, muss der Server-Cert später
    /// unter die Service-CA (oder der Client die passende Server-Wurzel erhalten).
    service_ca_pem: String,
}

/// Gültigkeit frisch ausgestellter Team-Geräte-Ausweise (Tage). Kurzlebig
/// (Roadmap 10): ein geklauter Ausweis verfällt von selbst; die App erneuert
/// rechtzeitig über `/api/ausweis/erneuern`.
const GERAET_GUELTIG_TAGE: i64 = 90;
/// Förderer-Ausweise: kurzlebig wie Team-Geräte – die Förderer-App erneuert
/// automatisch (analog Roadmap 10).
const FOERDERER_GUELTIG_TAGE: i64 = 90;
/// Kulanzfrist der Grace-Wiederanmeldung: so lange NACH Ablauf darf ein Gerät
/// über den öffentlichen Kanal (mit Besitznachweis) noch einen frischen Ausweis
/// holen – für den Fall „länger als die Gültigkeit offline gewesen".
const GRACE_TAGE: i64 = 90;

/// Verbinden (POST /api/enroll): ein neues Gerät löst seinen Einmal-Token ein
/// und erhält einen von der Service-CA signierten Ausweis. OHNE Zertifikat –
/// der Token ist die Berechtigung. Läuft über den öffentlichen :443-Kanal.
///
/// Idempotent: derselbe Token + derselbe Schlüssel geben denselben Ausweis
/// zurück (robust gegen verlorene Antworten); ein anderer Schlüssel nach dem
/// Einlösen wird abgewiesen (409). Antworten: 401 Token unbekannt,
/// 410 abgelaufen, 402 Abo, 409 schon anders eingelöst / Wettlauf verloren,
/// 503 keine CA, 400 ungültiger Schlüssel.
async fn enroll(
    State(st): State<AppState>,
    Json(body): Json<EnrollBody>,
) -> Result<Json<EnrollAntwort>, StatusCode> {
    let service_ca: &Option<StufenCa> = &st.service_ca;
    let ca = service_ca.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let token = body.token.trim();
    if token.is_empty() || token.len() > FELD_MAX {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.pubkey_pem.len() > PUBKEY_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let name = body.name.as_deref().unwrap_or("").trim();
    let name = if name.is_empty() { "Team-Gerät" } else { name };
    if name.len() > FELD_MAX {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let pk_fp = pubkey_fingerprint(&body.pubkey_pem);

    // Einladung nachschlagen.
    let row: Option<(i64, String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT konto_id, ablauf, benutzt, ausweis_pem, pubkey_fingerprint
         FROM einladung WHERE token = ?",
    )
    .bind(token)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some((konto_id, ablauf, benutzt, ausweis_gesp, pk_gesp)) = row else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    // Schon eingelöst? Nur derselbe Schlüssel darf den Ausweis erneut abholen.
    if benutzt != 0 {
        return match (ausweis_gesp, pk_gesp) {
            (Some(ausweis), Some(p)) if p == pk_fp => Ok(Json(EnrollAntwort {
                ausweis_pem: ausweis,
                service_ca_pem: ca.wurzel_cert_pem.clone(),
            })),
            _ => Err(StatusCode::CONFLICT),
        };
    }

    // Abgelaufen? (ISO-Zeitstrings vergleichen sich chronologisch korrekt.)
    let (jetzt,): (String,) = sqlx::query_as("SELECT datetime('now')")
        .fetch_one(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ablauf <= jetzt {
        return Err(StatusCode::GONE);
    }

    // Abo-Gate (Schritt 5): das neue Gerät zählt mit (zusatz = 1). Das erste
    // Gerät (Einzelplatz) bleibt frei; ab dem 2. braucht das Konto ein Abo.
    // Solange ABO_GATING aus ist, immer erlaubt.
    if !abo_gate_ok(&st.pool, konto_id, 1).await {
        return Err(StatusCode::PAYMENT_REQUIRED);
    }

    // Kopiersicher signieren (nur der öffentliche Schlüssel geht ein).
    let ausweis_pem = ca
        .ausweis_kette(&body.pubkey_pem, name, "Antrag 3000 Team-Gerät", GERAET_GUELTIG_TAGE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let cert_fp = service_ca::fingerprint_von_pem(&ausweis_pem)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Token ATOMAR beanspruchen (WHERE benutzt = 0) – verhindert, dass zwei
    // gleichzeitige Einlösungen beide ein Gerät anlegen. Wer hier 0 Zeilen
    // trifft, hat den Wettlauf verloren.
    let claim = sqlx::query(
        "UPDATE einladung SET benutzt = 1, ausweis_pem = ?, pubkey_fingerprint = ?
         WHERE token = ? AND benutzt = 0",
    )
    .bind(&ausweis_pem)
    .bind(&pk_fp)
    .bind(token)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if claim.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT);
    }

    // Erst nach gewonnenem Anspruch: Mitglied-Nutzer + Gerät anlegen. Der
    // Fingerabdruck ist der des ausgestellten Zertifikats – genau den weist das
    // Gerät später über Caddy vor, sodass konto_und_geraet es wiedererkennt.
    let nutzer = sqlx::query(
        "INSERT INTO nutzer (konto_id, anzeigename, rolle, ist_eigentuemer)
         VALUES (?, ?, 'mitglied', 0)",
    )
    .bind(konto_id)
    .bind(name)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query(
        "INSERT INTO geraet (nutzer_id, bezeichnung, cert_fingerprint, status)
         VALUES (?, ?, ?, 'aktiv')",
    )
    .bind(nutzer.last_insert_rowid())
    .bind(name)
    .bind(&cert_fp)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(EnrollAntwort {
        ausweis_pem,
        service_ca_pem: ca.wurzel_cert_pem.clone(),
    }))
}

#[derive(Deserialize)]
struct ErneuernBody {
    /// Frischer öffentlicher Geräteschlüssel (SPKI-PEM). Der private Teil bleibt
    /// lokal – die Erneuerung ist damit genauso kopiersicher wie das Enrollment.
    #[serde(rename = "pubkeyPem", alias = "pubkey_pem")]
    pubkey_pem: String,
}

#[derive(Serialize, Debug)]
struct ErneuernAntwort {
    ausweis_pem: String,
    service_ca_pem: String,
}

/// Ausweis erneuern (POST /api/ausweis/erneuern, NUR über mTLS): ein bereits
/// verbundenes Gerät holt sich vor Ablauf einen frischen, kurzlebigen Ausweis
/// (Roadmap 10). Es erzeugt dafür lokal ein NEUES Schlüsselpaar und schickt nur
/// den öffentlichen Teil. Der neue Fingerabdruck wird gesetzt; der bisherige
/// bleibt als `cert_fingerprint_vorher` kurz gültig, damit die Umstellung kein
/// Gerät aussperrt, falls die App die Antwort verliert (der alte Ausweis verfällt
/// ohnehin per Ablaufdatum). Kein Abo-Gate: eine Erneuerung hält ein bestehendes
/// Gerät nur am Leben, sie fügt keins hinzu.
async fn ausweis_erneuern(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ErneuernBody>,
) -> Result<Json<ErneuernAntwort>, StatusCode> {
    let ca = st
        .service_ca
        .as_ref()
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if body.pubkey_pem.len() > PUBKEY_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    // Aufrufer identifizieren (bestehendes, aktives mTLS-Gerät). Ein gesperrtes
    // oder unbekanntes Gerät bekommt hier nichts (konto_und_geraet → FORBIDDEN).
    let (_konto_id, geraet_id) = konto_und_geraet(&st.pool, &headers).await?;
    let alt_fp = fingerprint(&headers).ok_or(StatusCode::UNAUTHORIZED)?;

    // Anzeigenamen (CN) des Geräts beibehalten.
    let bezeichnung: String =
        sqlx::query_scalar("SELECT bezeichnung FROM geraet WHERE id = ?")
            .bind(geraet_id)
            .fetch_one(&st.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let neuer_ausweis = ca
        .ausweis_kette(
            &body.pubkey_pem,
            &bezeichnung,
            "Antrag 3000 Team-Gerät",
            GERAET_GUELTIG_TAGE,
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let neu_fp = service_ca::fingerprint_von_pem(&neuer_ausweis)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Neuen Fingerabdruck setzen, den bisherigen als Überlappung behalten.
    sqlx::query(
        "UPDATE geraet
         SET cert_fingerprint = ?, cert_fingerprint_vorher = ?, zuletzt_gesehen = datetime('now')
         WHERE id = ?",
    )
    .bind(&neu_fp)
    .bind(&alt_fp)
    .bind(geraet_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ErneuernAntwort {
        ausweis_pem: neuer_ausweis,
        service_ca_pem: ca.wurzel_cert_pem.clone(),
    }))
}

#[derive(Deserialize)]
struct GraceBody {
    /// Der alte (evtl. abgelaufene) Ausweis – nur der Zertifikatteil.
    #[serde(rename = "ausweisPem", alias = "ausweis_pem")]
    ausweis_pem: String,
    /// Frischer öffentlicher Geräteschlüssel (SPKI-PEM).
    #[serde(rename = "pubkeyPem", alias = "pubkey_pem")]
    pubkey_pem: String,
    /// Besitznachweis: ECDSA-P256-Signatur (DER) über die Bytes von `pubkey_pem`,
    /// erzeugt mit dem ALTEN privaten Schlüssel. Beweist, dass der Aufrufer den
    /// alten Schlüssel wirklich besitzt – nicht nur das öffentliche Zertifikat.
    #[serde(default)]
    proof: Vec<u8>,
}

/// Grace-Wiederanmeldung (POST /api/ausweis/grace, ÖFFENTLICH): heilt den Fall,
/// dass ein Gerät länger als die Ausweis-Gültigkeit offline war – der abgelaufene
/// Ausweis kommt nicht mehr durch den mTLS-Kanal. Ohne neue Einladung: die App
/// legt ihren alten (echt CA-signierten) Ausweis + einen frischen Schlüssel + den
/// Besitznachweis vor und bekommt innerhalb der Kulanzfrist einen neuen.
/// Sicherheit: (1) Ausweis echt CA-signiert + höchstens GRACE_TAGE abgelaufen,
/// (2) Besitznachweis mit dem alten Schlüssel (kein Kapern per öffentlichem Zert),
/// (3) Gerät weiterhin `aktiv` (eine Sperre verhindert die Grace-Erneuerung),
/// (4) Missbrauchs-Bremse. Kein Abo-Gate (hält nur ein Gerät am Leben).
async fn ausweis_grace(
    State(st): State<AppState>,
    Json(body): Json<GraceBody>,
) -> Result<Json<ErneuernAntwort>, StatusCode> {
    let ca = st
        .service_ca
        .as_ref()
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if body.pubkey_pem.len() > PUBKEY_MAX_BYTES
        || body.ausweis_pem.len() > 8192
        || body.proof.len() > 512
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    // Öffentlicher Kanal → globale Missbrauchs-Bremse (wie /api/team).
    if !tempo_ok(&st, -1) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // (1) Ausweis echt von der Service-CA + innerhalb der Kulanzfrist. Liefert den
    //     öffentlichen Schlüssel des alten Ausweises (SEC1) für (2).
    let sec1 = ca
        .grace_pubkey(&body.ausweis_pem, GRACE_TAGE)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // (2) Besitznachweis: die Signatur über pubkey_pem muss zum alten Schlüssel
    //     passen. Nur wer den alten PRIVATEN Schlüssel hat, kann sie erzeugen.
    use p256::ecdsa::signature::Verifier;
    let vk = p256::ecdsa::VerifyingKey::from_sec1_bytes(&sec1)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let sig = p256::ecdsa::Signature::from_der(&body.proof)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    vk.verify(body.pubkey_pem.as_bytes(), &sig)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // (3) Gerät muss bekannt UND aktiv sein (Sperre bleibt wirksam).
    let alt_fp = service_ca::fingerprint_von_pem(&body.ausweis_pem)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, status FROM geraet
         WHERE cert_fingerprint = ? OR cert_fingerprint_vorher = ?",
    )
    .bind(&alt_fp)
    .bind(&alt_fp)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some((geraet_id, status)) = row else {
        return Err(StatusCode::FORBIDDEN);
    };
    if status != "aktiv" {
        return Err(StatusCode::FORBIDDEN);
    }

    let bezeichnung: String =
        sqlx::query_scalar("SELECT bezeichnung FROM geraet WHERE id = ?")
            .bind(geraet_id)
            .fetch_one(&st.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let neuer_ausweis = ca
        .ausweis_kette(
            &body.pubkey_pem,
            &bezeichnung,
            "Antrag 3000 Team-Gerät",
            GERAET_GUELTIG_TAGE,
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let neu_fp = service_ca::fingerprint_von_pem(&neuer_ausweis)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query(
        "UPDATE geraet
         SET cert_fingerprint = ?, cert_fingerprint_vorher = ?, zuletzt_gesehen = datetime('now')
         WHERE id = ?",
    )
    .bind(&neu_fp)
    .bind(&alt_fp)
    .bind(geraet_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ErneuernAntwort {
        ausweis_pem: neuer_ausweis,
        service_ca_pem: ca.wurzel_cert_pem.clone(),
    }))
}

#[derive(Deserialize)]
struct TeamErstellenBody {
    /// Öffentlicher Geräteschlüssel (SPKI-PEM). Der private Teil bleibt lokal.
    #[serde(rename = "pubkeyPem", alias = "pubkey_pem")]
    pubkey_pem: String,
    /// Name des ersten (Eigentümer-)Geräts.
    name: Option<String>,
    /// Anzeigename des Teams.
    team_name: Option<String>,
}

#[derive(Serialize, Debug)]
struct TeamErstellenAntwort {
    ausweis_pem: String,
    service_ca_pem: String,
    konto_id: i64,
}

/// Team erstellen (POST /api/team): legt ein NEUES Konto an, macht das
/// aufrufende Gerät zum Eigentümer und stellt ihm kopiersicher einen Ausweis
/// aus. OHNE Zertifikat/Token – der öffentliche Selbstbedienungs-Weg, darum mit
/// Missbrauchs-Bremse. Kein Konto/keine E-Mail nötig (anonymes Konto, allein
/// über den Ausweis identifiziert – E-Mail/Wiederherstellung kommt später).
async fn team_erstellen(
    State(st): State<AppState>,
    Json(body): Json<TeamErstellenBody>,
) -> Result<Json<TeamErstellenAntwort>, StatusCode> {
    let service_ca: &Option<StufenCa> = &st.service_ca;
    let ca = service_ca.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Missbrauchs-Bremse (global): Sentinel-Geräte-id -1 (kein echtes Gerät hat
    // eine negative id). Begrenzt neue Teams pro Zeitfenster.
    if !tempo_ok(&st, -1) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if body.pubkey_pem.len() > PUBKEY_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let geraet_name = body.name.as_deref().unwrap_or("").trim();
    let geraet_name = if geraet_name.is_empty() { "Team-Gerät" } else { geraet_name };
    let team_name = body.team_name.as_deref().unwrap_or("").trim();
    let team_name = if team_name.is_empty() { "Team" } else { team_name };
    if geraet_name.len() > FELD_MAX || team_name.len() > FELD_MAX {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    // Kopiersicher signieren (nur der öffentliche Schlüssel geht ein).
    let ausweis_pem = ca
        .ausweis_kette(&body.pubkey_pem, geraet_name, "Antrag 3000 Team-Gerät", GERAET_GUELTIG_TAGE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let cert_fp = service_ca::fingerprint_von_pem(&ausweis_pem)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Neues (anonymes) Konto + Eigentümer-Nutzer + Gerät. Der abo_status bleibt
    // auf dem Spalten-Standard – die genaue Anfangsstufe ist eine spätere
    // Monetarisierungs-Entscheidung (Gating ist derzeit ohnehin aus).
    let konto = sqlx::query("INSERT INTO konto (name) VALUES (?)")
        .bind(team_name)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let konto_id = konto.last_insert_rowid();
    let nutzer = sqlx::query(
        "INSERT INTO nutzer (konto_id, anzeigename, rolle, ist_eigentuemer)
         VALUES (?, ?, 'eigentuemer', 1)",
    )
    .bind(konto_id)
    .bind(geraet_name)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query(
        "INSERT INTO geraet (nutzer_id, bezeichnung, cert_fingerprint, status)
         VALUES (?, ?, ?, 'aktiv')",
    )
    .bind(nutzer.last_insert_rowid())
    .bind(geraet_name)
    .bind(&cert_fp)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(TeamErstellenAntwort {
        ausweis_pem,
        service_ca_pem: ca.wurzel_cert_pem.clone(),
        konto_id,
    }))
}

// ============================================================
// Förderer verbinden (Roadmap 6): der Vendor lädt einen Förderer ein
// (admin-geschützt), der Förderer verbindet sich kopiersicher über den
// öffentlichen Kanal. Eigener Vertrauensanker (Förderer-CA), getrennt von den
// Team-Geräten.
// ============================================================

#[derive(Deserialize)]
struct FoerdererEinladungBody {
    /// Optionaler Namens-Hinweis für den einzuladenden Förderer.
    name: Option<String>,
}

#[derive(Serialize)]
struct FoerdererEinladungAntwort {
    token: String,
    ablauf: String,
    /// Förderer-CA-Zertifikat (Trust-Anker), damit der Vendor daraus ein
    /// vollständiges Einladungs-Paket bauen kann.
    foerderer_ca_pem: String,
}

/// Förderer-Einladung erstellen (POST /api/admin/foerderer-einladung). Nur der
/// Vendor/Admin (TOTP-Sitzung). Erzeugt einen kurzlebigen Einmal-Token.
async fn foerderer_einladung_erstellen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<FoerdererEinladungBody>,
) -> Result<Json<FoerdererEinladungAntwort>, StatusCode> {
    let (_konto_id, geraet_id) = admin_pruefen(&st, &headers).await?;
    if !tempo_ok(&st, geraet_id) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let foerderer_ca: &Option<Ca> = &st.foerderer_ca;
    let ca = foerderer_ca.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let name = body.name.as_deref().unwrap_or("").trim();
    if name.len() > FELD_MAX {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let name_opt = if name.is_empty() { None } else { Some(name) };

    let token = neuer_token();
    sqlx::query(
        "INSERT INTO foerderer_einladung (token, name, ablauf)
         VALUES (?, ?, datetime('now', ?))",
    )
    .bind(&token)
    .bind(name_opt)
    .bind(format!("+{EINLADUNG_TTL_S} seconds"))
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (ablauf,): (String,) =
        sqlx::query_as("SELECT ablauf FROM foerderer_einladung WHERE token = ?")
            .bind(&token)
            .fetch_one(&st.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(FoerdererEinladungAntwort {
        token,
        ablauf,
        foerderer_ca_pem: ca.cert_pem.clone(),
    }))
}

#[derive(Deserialize)]
struct FoerdererEnrollBody {
    token: String,
    #[serde(rename = "pubkeyPem", alias = "pubkey_pem")]
    pubkey_pem: String,
    name: Option<String>,
}

#[derive(Serialize, Debug)]
struct FoerdererEnrollAntwort {
    ausweis_pem: String,
    foerderer_ca_pem: String,
}

/// Förderer verbinden (POST /api/foerderer-enroll): löst den Einmal-Token ein
/// und erhält einen von der Förderer-CA signierten Ausweis. OHNE Zertifikat,
/// über den öffentlichen :443-Kanal. Idempotent + atomar wie das Team-Enroll.
async fn foerderer_enroll(
    State(st): State<AppState>,
    Json(body): Json<FoerdererEnrollBody>,
) -> Result<Json<FoerdererEnrollAntwort>, StatusCode> {
    let foerderer_ca: &Option<Ca> = &st.foerderer_ca;
    let ca = foerderer_ca.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let token = body.token.trim();
    if token.is_empty() || token.len() > FELD_MAX {
        return Err(StatusCode::BAD_REQUEST);
    }
    if body.pubkey_pem.len() > PUBKEY_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let name = body.name.as_deref().unwrap_or("").trim();
    let name = if name.is_empty() { "Förderer" } else { name };
    if name.len() > FELD_MAX {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let pk_fp = pubkey_fingerprint(&body.pubkey_pem);

    let row: Option<(String, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT ablauf, benutzt, ausweis_pem, pubkey_fingerprint
         FROM foerderer_einladung WHERE token = ?",
    )
    .bind(token)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some((ablauf, benutzt, ausweis_gesp, pk_gesp)) = row else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    if benutzt != 0 {
        return match (ausweis_gesp, pk_gesp) {
            (Some(ausweis), Some(p)) if p == pk_fp => Ok(Json(FoerdererEnrollAntwort {
                ausweis_pem: ausweis,
                foerderer_ca_pem: ca.cert_pem.clone(),
            })),
            _ => Err(StatusCode::CONFLICT),
        };
    }

    let (jetzt,): (String,) = sqlx::query_as("SELECT datetime('now')")
        .fetch_one(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if ablauf <= jetzt {
        return Err(StatusCode::GONE);
    }

    let ausweis_pem = ca
        .signiere(&body.pubkey_pem, name, "Antrag 3000 Förderer", FOERDERER_GUELTIG_TAGE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let cert_fp = service_ca::fingerprint_von_pem(&ausweis_pem)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let claim = sqlx::query(
        "UPDATE foerderer_einladung SET benutzt = 1, ausweis_pem = ?, pubkey_fingerprint = ?
         WHERE token = ? AND benutzt = 0",
    )
    .bind(&ausweis_pem)
    .bind(&pk_fp)
    .bind(token)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if claim.rows_affected() == 0 {
        return Err(StatusCode::CONFLICT);
    }

    sqlx::query("INSERT INTO foerderer (name, cert_fingerprint, status) VALUES (?, ?, 'aktiv')")
        .bind(name)
        .bind(&cert_fp)
        .execute(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(FoerdererEnrollAntwort {
        ausweis_pem,
        foerderer_ca_pem: ca.cert_pem.clone(),
    }))
}

/// Förderer-Ausweis erneuern (POST /api/foerderer/ausweis-erneuern, NUR mTLS):
/// ein verbundener Förderer holt vor Ablauf einen frischen, kurzlebigen Ausweis
/// (Roadmap 10). Frisches Schlüsselpaar entsteht in der Förderer-App; nur der
/// öffentliche Teil kommt her. Überlappung via `cert_fingerprint_vorher`.
async fn foerderer_ausweis_erneuern(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ErneuernBody>,
) -> Result<Json<FoerdererEnrollAntwort>, StatusCode> {
    let ca = st
        .foerderer_ca
        .as_ref()
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if body.pubkey_pem.len() > PUBKEY_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let fid = foerderer_aus_cert(&st.pool, &headers).await?;
    let alt_fp = fingerprint(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let name: String = sqlx::query_scalar("SELECT name FROM foerderer WHERE id = ?")
        .bind(fid)
        .fetch_one(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ausweis_pem = ca
        .signiere(&body.pubkey_pem, &name, "Antrag 3000 Förderer", FOERDERER_GUELTIG_TAGE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let neu_fp = service_ca::fingerprint_von_pem(&ausweis_pem)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query(
        "UPDATE foerderer
         SET cert_fingerprint = ?, cert_fingerprint_vorher = ?, zuletzt_gesehen = datetime('now')
         WHERE id = ?",
    )
    .bind(&neu_fp)
    .bind(&alt_fp)
    .bind(fid)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(FoerdererEnrollAntwort {
        ausweis_pem,
        foerderer_ca_pem: ca.cert_pem.clone(),
    }))
}

/// Förderer-Grace-Wiederanmeldung (POST /api/foerderer/ausweis-grace, ÖFFENTLICH):
/// wie bei den Team-Geräten – ein abgelaufener, aber echt Förderer-CA-signierter
/// Ausweis + Besitznachweis (p256-Signatur mit dem alten Schlüssel) ergibt
/// innerhalb der Kulanzfrist einen frischen. Förderer muss `aktiv` sein; globale
/// Missbrauchs-Bremse.
async fn foerderer_ausweis_grace(
    State(st): State<AppState>,
    Json(body): Json<GraceBody>,
) -> Result<Json<FoerdererEnrollAntwort>, StatusCode> {
    let ca = st
        .foerderer_ca
        .as_ref()
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    if body.pubkey_pem.len() > PUBKEY_MAX_BYTES
        || body.ausweis_pem.len() > 8192
        || body.proof.len() > 512
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if !tempo_ok(&st, -1) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let sec1 = ca
        .grace_pubkey(&body.ausweis_pem, GRACE_TAGE)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    use p256::ecdsa::signature::Verifier;
    let vk = p256::ecdsa::VerifyingKey::from_sec1_bytes(&sec1)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let sig = p256::ecdsa::Signature::from_der(&body.proof)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    vk.verify(body.pubkey_pem.as_bytes(), &sig)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let alt_fp = service_ca::fingerprint_von_pem(&body.ausweis_pem)
        .ok_or(StatusCode::BAD_REQUEST)?;
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, status FROM foerderer
         WHERE cert_fingerprint = ? OR cert_fingerprint_vorher = ?",
    )
    .bind(&alt_fp)
    .bind(&alt_fp)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some((fid, status)) = row else {
        return Err(StatusCode::FORBIDDEN);
    };
    if status != "aktiv" {
        return Err(StatusCode::FORBIDDEN);
    }
    let name: String = sqlx::query_scalar("SELECT name FROM foerderer WHERE id = ?")
        .bind(fid)
        .fetch_one(&st.pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ausweis_pem = ca
        .signiere(&body.pubkey_pem, &name, "Antrag 3000 Förderer", FOERDERER_GUELTIG_TAGE)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let neu_fp = service_ca::fingerprint_von_pem(&ausweis_pem)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query(
        "UPDATE foerderer
         SET cert_fingerprint = ?, cert_fingerprint_vorher = ?, zuletzt_gesehen = datetime('now')
         WHERE id = ?",
    )
    .bind(&neu_fp)
    .bind(&alt_fp)
    .bind(fid)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(FoerdererEnrollAntwort {
        ausweis_pem,
        foerderer_ca_pem: ca.cert_pem.clone(),
    }))
}

// ---- Förderer pflegen ihre Programme online (Roadmap 6b) ----

#[derive(Serialize, Debug)]
struct FoerdererProgramm {
    programm_id: String,
    inhalt: serde_json::Value,
    status: String,
    geaendert_am: String,
}

/// Alle Programme des aufrufenden Förderers (GET /api/foerderer-programme).
async fn foerderer_programme_lesen(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<FoerdererProgramm>>, StatusCode> {
    let fid = foerderer_aus_cert(&st.pool, &headers).await?;
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT programm_id, inhalt_json, status, geaendert_am
         FROM foerderer_programm WHERE foerderer_id = ? ORDER BY geaendert_am DESC",
    )
    .bind(fid)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(programm_id, json, status, geaendert_am)| FoerdererProgramm {
            programm_id,
            inhalt: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
            status,
            geaendert_am,
        })
        .collect();
    Ok(Json(liste))
}

#[derive(Deserialize)]
struct ProgrammBody {
    inhalt: serde_json::Value,
}

/// Ein Programm anlegen/aktualisieren (PUT /api/foerderer-programme/{id}).
/// Upsert je (Förderer, programm_id); „letzte Änderung gewinnt" (der Förderer
/// ist alleiniger Bearbeiter seiner eigenen Programme). Setzt den Status wieder
/// auf 'aktiv' (falls zuvor zurückgezogen).
async fn foerderer_programm_schreiben(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(programm_id): Path<String>,
    Json(body): Json<ProgrammBody>,
) -> Result<StatusCode, StatusCode> {
    let fid = foerderer_aus_cert(&st.pool, &headers).await?;
    if !tempo_ok(&st, foerderer_tempo_key(fid)) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    if programm_id.trim().is_empty() || programm_id.len() > FELD_MAX {
        return Err(StatusCode::BAD_REQUEST);
    }
    let inhalt_str = serde_json::to_string(&body.inhalt).map_err(|_| StatusCode::BAD_REQUEST)?;
    if inhalt_str.len() > FOERDERER_MAX_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    // Mindestens ein Name muss drin sein (wie bei geteilten Förderern).
    if body
        .inhalt
        .get("name")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Kontingent aktiver Programme je Förderer (Upsert der gleichen id zählt nicht mit).
    let anzahl: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM foerderer_programm
         WHERE foerderer_id = ? AND programm_id <> ? AND status = 'aktiv'",
    )
    .bind(fid)
    .bind(&programm_id)
    .fetch_one(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if anzahl >= FOERDERER_QUOTA {
        return Err(StatusCode::FORBIDDEN);
    }

    sqlx::query(
        "INSERT INTO foerderer_programm (foerderer_id, programm_id, inhalt_json, status, geaendert_am)
         VALUES (?, ?, ?, 'aktiv', datetime('now'))
         ON CONFLICT(foerderer_id, programm_id) DO UPDATE SET
            inhalt_json = excluded.inhalt_json,
            status = 'aktiv',
            geaendert_am = excluded.geaendert_am",
    )
    .bind(fid)
    .bind(&programm_id)
    .bind(&inhalt_str)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ein Programm zurückziehen (DELETE /api/foerderer-programme/{id}).
/// Soft-Delete: `status = 'zurueckgezogen'` – der Eintrag bleibt sichtbar,
/// damit die Kuratierung (6c) die Löschung in den Katalog übernehmen kann.
async fn foerderer_programm_loeschen(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(programm_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let fid = foerderer_aus_cert(&st.pool, &headers).await?;
    let res = sqlx::query(
        "UPDATE foerderer_programm SET status = 'zurueckgezogen', geaendert_am = datetime('now')
         WHERE foerderer_id = ? AND programm_id = ?",
    )
    .bind(fid)
    .bind(&programm_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, Debug)]
struct MitgliedZeile {
    id: i64,
    bezeichnung: String,
    status: String,
    zuletzt_gesehen: Option<String>,
    erstellt_am: String,
    ist_eigentuemer: bool,
    /// Ist das das aufrufende Gerät? (Damit die App „sich selbst" nicht sperrt.)
    dieses_geraet: bool,
}

/// Alle Geräte des eigenen Teams (GET /api/mitglieder). Nur der Eigentümer.
async fn mitglieder_liste(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<MitgliedZeile>>, StatusCode> {
    let (konto_id, mein_geraet) = eigentuemer_pruefen(&st.pool, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, String, i64)>(
        "SELECT g.id, g.bezeichnung, g.status, g.zuletzt_gesehen, g.erstellt_am, n.ist_eigentuemer
         FROM geraet g JOIN nutzer n ON n.id = g.nutzer_id
         WHERE n.konto_id = ? ORDER BY g.erstellt_am",
    )
    .bind(konto_id)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(id, bezeichnung, status, zuletzt_gesehen, erstellt_am, eig)| MitgliedZeile {
            id,
            bezeichnung,
            status,
            zuletzt_gesehen,
            erstellt_am,
            ist_eigentuemer: eig != 0,
            dieses_geraet: id == mein_geraet,
        })
        .collect();
    Ok(Json(liste))
}

/// Setzt den Status eines Team-Geräts (PUT /api/mitglieder/{id}): aktiv |
/// gesperrt. Nur der Eigentümer, nur im eigenen Konto. Das aufrufende Gerät
/// kann sich nicht selbst sperren (kein Aussperren).
async fn mitglied_status(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(geraet_id): Path<i64>,
    Json(body): Json<StatusBody>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, mein_geraet) = eigentuemer_pruefen(&st.pool, &headers).await?;
    if !["aktiv", "gesperrt"].contains(&body.status.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if geraet_id == mein_geraet && body.status == "gesperrt" {
        return Err(StatusCode::CONFLICT);
    }
    let res = sqlx::query(
        "UPDATE geraet SET status = ?
         WHERE id = ? AND nutzer_id IN (SELECT id FROM nutzer WHERE konto_id = ?)",
    )
    .bind(&body.status)
    .bind(geraet_id)
    .bind(konto_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
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

#[derive(Serialize)]
struct GeraetZeile {
    id: i64,
    bezeichnung: String,
    cert_fingerprint: String,
    status: String,
    zuletzt_gesehen: Option<String>,
    erstellt_am: String,
}

/// Alle Geräte des Kontos (GET /api/admin/geraete) – zum Sperren/Entsperren
/// (Zertifikat-Rückruf). Nur für Admins.
async fn admin_geraete(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<GeraetZeile>>, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, String, String, Option<String>, String)>(
        "SELECT g.id, g.bezeichnung, g.cert_fingerprint, g.status, g.zuletzt_gesehen, g.erstellt_am
         FROM geraet g JOIN nutzer n ON n.id = g.nutzer_id
         WHERE n.konto_id = ? ORDER BY g.zuletzt_gesehen DESC",
    )
    .bind(konto_id)
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(id, bezeichnung, cert_fingerprint, status, zuletzt_gesehen, erstellt_am)| GeraetZeile {
            id,
            bezeichnung,
            cert_fingerprint,
            status,
            zuletzt_gesehen,
            erstellt_am,
        })
        .collect();
    Ok(Json(liste))
}

/// Setzt den Status eines Geräts (PUT /api/admin/geraete/{id}). Erlaubt:
/// aktiv, gesperrt. Ein gesperrtes Gerät wird abgewiesen und NICHT neu
/// auto-enrollt (siehe konto_und_geraet) – das ist der Zertifikat-Rückruf.
/// Nur für Admins.
async fn admin_geraet_status(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(geraet_id): Path<i64>,
    Json(body): Json<StatusBody>,
) -> Result<StatusCode, StatusCode> {
    let (konto_id, _) = admin_pruefen(&st, &headers).await?;
    if !["aktiv", "gesperrt"].contains(&body.status.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let res = sqlx::query(
        "UPDATE geraet SET status = ?
         WHERE id = ? AND nutzer_id IN (SELECT id FROM nutzer WHERE konto_id = ?)",
    )
    .bind(&body.status)
    .bind(geraet_id)
    .bind(konto_id)
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
    // Gleiche WAL-/busy_timeout-Einstellung wie der Server, damit der
    // Sammler-Lauf den laufenden Server nicht aussperrt (siehe main()).
    let opts = SqliteConnectOptions::new()
        .filename(&db_pfad)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
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

/// Entfernt eine Förderung (nach id) aus dem verteilten Katalog. Gegenstück zu
/// `katalog_eintrag_anwenden` – für zurückgezogene Förderer-Programme (6c).
/// Kein Fehler, wenn die id gar nicht (mehr) im Katalog steht.
async fn katalog_eintrag_entfernen(
    pool: &SqlitePool,
    konto_id: i64,
    geraet_id: i64,
    foerderung_id: &str,
) -> Result<(), StatusCode> {
    let Some(text) = katalog_text(pool, konto_id).await else {
        return Ok(()); // kein Katalog → nichts zu entfernen
    };
    let mut v: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some(arr) = v.get_mut("foerderungen").and_then(|x| x.as_array_mut()) else {
        return Ok(());
    };
    let vorher = arr.len();
    arr.retain(|f| f.get("id").and_then(|x| x.as_str()) != Some(foerderung_id));
    if arr.len() == vorher {
        return Ok(()); // war nicht drin
    }
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

// ============================================================
// Förderer-Kuratierung (Roadmap 6c): der Vendor/Admin sieht verbundene Förderer
// und ihre Programm-Änderungen und gibt sie in den öffentlichen Katalog frei
// (Löschungen werden mit übernommen).
// ============================================================

#[derive(Serialize)]
struct VerbundenerFoerderer {
    id: i64,
    name: String,
    status: String,
    zuletzt_gesehen: Option<String>,
    anzahl_programme: i64,
}

/// Verbundene Förderer auflisten (GET /api/admin/foerderer-verbunden).
async fn admin_foerderer_verbunden(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<VerbundenerFoerderer>>, StatusCode> {
    let _ = admin_pruefen(&st, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, i64)>(
        "SELECT f.id, f.name, f.status, f.zuletzt_gesehen,
                (SELECT COUNT(*) FROM foerderer_programm p
                 WHERE p.foerderer_id = f.id AND p.status = 'aktiv')
         FROM foerderer f ORDER BY f.erstellt_am DESC",
    )
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(id, name, status, zuletzt_gesehen, anzahl_programme)| VerbundenerFoerderer {
            id,
            name,
            status,
            zuletzt_gesehen,
            anzahl_programme,
        })
        .collect();
    Ok(Json(liste))
}

#[derive(Serialize)]
struct KuratierProgramm {
    foerderer_id: i64,
    foerderer_name: String,
    programm_id: String,
    inhalt: serde_json::Value,
    status: String,
    geaendert_am: String,
    /// Offen = seit der letzten Freigabe geändert (oder nie freigegeben).
    offen: bool,
}

/// Alle Programme verbundener Förderer für die Kuratierung
/// (GET /api/admin/foerderer-programme), mit „offen"-Kennzeichen.
async fn admin_foerderer_programme(
    State(st): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<KuratierProgramm>>, StatusCode> {
    let _ = admin_pruefen(&st, &headers).await?;
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, String, Option<String>)>(
        "SELECT p.foerderer_id, f.name, p.programm_id, p.inhalt_json, p.status, p.geaendert_am, p.freigegeben_am
         FROM foerderer_programm p JOIN foerderer f ON f.id = p.foerderer_id
         ORDER BY p.geaendert_am DESC",
    )
    .fetch_all(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let liste = rows
        .into_iter()
        .map(|(foerderer_id, foerderer_name, programm_id, json, status, geaendert_am, freigegeben_am)| {
            let offen = match &freigegeben_am {
                None => true,
                Some(fr) => &geaendert_am > fr,
            };
            KuratierProgramm {
                foerderer_id,
                foerderer_name,
                programm_id,
                inhalt: serde_json::from_str(&json).unwrap_or(serde_json::Value::Null),
                status,
                geaendert_am,
                offen,
            }
        })
        .collect();
    Ok(Json(liste))
}

/// Ein Förderer-Programm freigeben (POST
/// /api/admin/foerderer-programme/{foerderer_id}/{programm_id}/freigeben):
/// bringt den Katalog in Deckung mit dem aktuellen Stand – aktiv → übernehmen,
/// zurückgezogen → aus dem Katalog entfernen – und setzt `freigegeben_am`.
async fn admin_foerderer_programm_freigeben(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path((foerderer_id, programm_id)): Path<(i64, String)>,
) -> Result<StatusCode, StatusCode> {
    let (_konto_id, geraet_id) = admin_pruefen(&st, &headers).await?;
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT inhalt_json, status FROM foerderer_programm WHERE foerderer_id = ? AND programm_id = ?",
    )
    .bind(foerderer_id)
    .bind(&programm_id)
    .fetch_optional(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let Some((inhalt_str, status)) = row else {
        return Err(StatusCode::NOT_FOUND);
    };

    // Namensraum-id für den Katalog, damit sich zwei Förderer nicht überschreiben.
    let katalog_id = format!("f{foerderer_id}-{programm_id}");
    if status == "aktiv" {
        let mut inhalt: serde_json::Value =
            serde_json::from_str(&inhalt_str).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        inhalt["id"] = serde_json::Value::String(katalog_id.clone());
        // Herkunfts-Stempel: automatisch die verbundene Förderer-Identität.
        inhalt["herkunft"] =
            serde_json::json!({ "typ": "foerderer-verbunden", "foerderer_id": foerderer_id });
        katalog_eintrag_anwenden(&st.pool, 1, geraet_id, inhalt).await?;
    } else {
        katalog_eintrag_entfernen(&st.pool, 1, geraet_id, &katalog_id).await?;
    }

    sqlx::query(
        "UPDATE foerderer_programm SET freigegeben_am = datetime('now')
         WHERE foerderer_id = ? AND programm_id = ?",
    )
    .bind(foerderer_id)
    .bind(&programm_id)
    .execute(&st.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Baut einen frischen In-Memory-Server-Zustand (eigene DB + Service-CA)
    /// für die Enrollment-Tests.
    async fn test_state() -> AppState {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1) // dieselbe In-Memory-DB über alle Abfragen
            .connect_with(opts)
            .await
            .unwrap();
        schema_anlegen(&pool).await.unwrap();
        // Eindeutiger CA-Ordner je Aufruf – sonst kollidieren parallele Tests
        // (gleicher Ordner: einer löscht, während ein anderer hineinschreibt).
        use std::sync::atomic::{AtomicU64, Ordering};
        static NR: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "a3k-test-ca-{}-{}",
            std::process::id(),
            NR.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let service = StufenCa::laden_oder_erzeugen(&dir).unwrap();
        let foerderer = Ca::laden_oder_erzeugen(&dir, "foerderer-ca", "Antrag 3000 Förderer-CA").unwrap();
        AppState {
            pool,
            schreib: Arc::new(Mutex::new(HashMap::new())),
            sitzungen: Arc::new(Mutex::new(HashMap::new())),
            service_ca: Arc::new(Some(service)),
            foerderer_ca: Arc::new(Some(foerderer)),
        }
    }

    /// Ein gültiger Token wird EINMAL eingelöst → signierter Ausweis + Gerät
    /// angelegt. Derselbe Token + Schlüssel holt idempotent denselben Ausweis;
    /// ein ANDERER Schlüssel nach dem Einlösen wird abgewiesen (409). Es
    /// entsteht dabei nur ein einziges Gerät.
    #[tokio::test]
    async fn enroll_einmal_und_idempotent() {
        let st = test_state().await;
        sqlx::query(
            "INSERT INTO einladung (token, konto_id, ablauf) VALUES ('tok1', 1, datetime('now', '+1 hour'))",
        )
        .execute(&st.pool)
        .await
        .unwrap();

        // Geräteseite: Schlüsselpaar lokal, nur der öffentliche Teil wird gesendet.
        let geraet_key = rcgen::KeyPair::generate().unwrap();
        let pubkey_pem = geraet_key.public_key_pem();

        let antwort = enroll(
            State(st.clone()),
            Json(EnrollBody {
                token: "tok1".into(),
                pubkey_pem: pubkey_pem.clone(),
                name: Some("Laptop".into()),
            }),
        )
        .await
        .expect("Einlösung muss klappen");
        assert!(antwort.0.ausweis_pem.contains("BEGIN CERTIFICATE"));

        // Genau ein Gerät, mit dem Fingerabdruck des ausgestellten Ausweises.
        let erwartet_fp = service_ca::fingerprint_von_pem(&antwort.0.ausweis_pem).unwrap();
        let (anzahl, fp): (i64, String) =
            sqlx::query_as("SELECT COUNT(*), MAX(cert_fingerprint) FROM geraet")
                .fetch_one(&st.pool)
                .await
                .unwrap();
        assert_eq!(anzahl, 1);
        assert_eq!(fp, erwartet_fp);

        // Idempotent: gleicher Token + Schlüssel → gleicher Ausweis, kein 2. Gerät.
        let wieder = enroll(
            State(st.clone()),
            Json(EnrollBody {
                token: "tok1".into(),
                pubkey_pem: pubkey_pem.clone(),
                name: Some("Laptop".into()),
            }),
        )
        .await
        .expect("idempotentes Abholen muss klappen");
        assert_eq!(wieder.0.ausweis_pem, antwort.0.ausweis_pem);
        let anzahl2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM geraet")
            .fetch_one(&st.pool)
            .await
            .unwrap();
        assert_eq!(anzahl2, 1, "kein zweites Gerät bei idempotentem Abholen");

        // Anderer Schlüssel nach dem Einlösen → 409.
        let anderer = rcgen::KeyPair::generate().unwrap();
        let err = enroll(
            State(st.clone()),
            Json(EnrollBody {
                token: "tok1".into(),
                pubkey_pem: anderer.public_key_pem(),
                name: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::CONFLICT);
    }

    /// Ein abgelaufener Token wird abgewiesen (410) und legt kein Gerät an.
    #[tokio::test]
    async fn enroll_abgelaufen() {
        let st = test_state().await;
        sqlx::query(
            "INSERT INTO einladung (token, konto_id, ablauf) VALUES ('alt', 1, datetime('now', '-1 hour'))",
        )
        .execute(&st.pool)
        .await
        .unwrap();

        let geraet_key = rcgen::KeyPair::generate().unwrap();
        let err = enroll(
            State(st.clone()),
            Json(EnrollBody {
                token: "alt".into(),
                pubkey_pem: geraet_key.public_key_pem(),
                name: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::GONE);

        let anzahl: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM geraet")
            .fetch_one(&st.pool)
            .await
            .unwrap();
        assert_eq!(anzahl, 0);
    }

    /// Ein unbekannter Token → 401.
    #[tokio::test]
    async fn enroll_unbekannter_token() {
        let st = test_state().await;
        let geraet_key = rcgen::KeyPair::generate().unwrap();
        let err = enroll(
            State(st.clone()),
            Json(EnrollBody {
                token: "gibtsnicht".into(),
                pubkey_pem: geraet_key.public_key_pem(),
                name: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::UNAUTHORIZED);
    }

    // --- Mitglieder verwalten (Eigentümer) ---

    /// SHA-256-Hex roher DER-Bytes (wie der Server aus dem Zertifikat-Header).
    fn fp_von(der: &[u8]) -> String {
        let mut h = Sha256::new();
        h.update(der);
        hex::encode(h.finalize())
    }

    /// Baut einen Request-Header mit dem (base64-kodierten) Client-Zertifikat,
    /// wie Caddy ihn setzt.
    fn hdr(der: &[u8]) -> HeaderMap {
        use base64::Engine as _;
        let mut h = HeaderMap::new();
        let b64 = base64::engine::general_purpose::STANDARD.encode(der);
        h.insert("x-client-cert-der", b64.parse().unwrap());
        h
    }

    /// Konto 1 mit Eigentümer-Gerät + einem Mitglied-Gerät. Gibt (State, DER des
    /// Eigentümer-Geräts, DER des Mitglied-Geräts, Mitglied-geraet_id) zurück.
    async fn setup_team() -> (AppState, Vec<u8>, Vec<u8>, i64) {
        let st = test_state().await;
        let der_owner = b"owner-cert".to_vec();
        sqlx::query(
            "INSERT INTO geraet (nutzer_id, bezeichnung, cert_fingerprint, status)
             VALUES (1, 'Owner-Laptop', ?, 'aktiv')",
        )
        .bind(fp_von(&der_owner))
        .execute(&st.pool)
        .await
        .unwrap();

        let mitglied = sqlx::query(
            "INSERT INTO nutzer (konto_id, anzeigename, rolle, ist_eigentuemer)
             VALUES (1, 'Mitglied', 'mitglied', 0)",
        )
        .execute(&st.pool)
        .await
        .unwrap();
        let der_member = b"member-cert".to_vec();
        let g = sqlx::query(
            "INSERT INTO geraet (nutzer_id, bezeichnung, cert_fingerprint, status)
             VALUES (?, 'Mitglied-Tablet', ?, 'aktiv')",
        )
        .bind(mitglied.last_insert_rowid())
        .bind(fp_von(&der_member))
        .execute(&st.pool)
        .await
        .unwrap();
        (st, der_owner, der_member, g.last_insert_rowid())
    }

    /// Der Eigentümer sieht beide Geräte (mit „dieses Gerät"/Eigentümer-Flags)
    /// und kann ein Mitglied-Gerät sperren.
    #[tokio::test]
    async fn mitglieder_liste_und_sperren() {
        let (st, der_owner, _der_member, member_id) = setup_team().await;

        let liste = mitglieder_liste(State(st.clone()), hdr(&der_owner))
            .await
            .expect("Liste")
            .0;
        assert_eq!(liste.len(), 2);
        let selbst = liste.iter().find(|m| m.dieses_geraet).unwrap();
        assert!(selbst.ist_eigentuemer);
        let member = liste.iter().find(|m| !m.ist_eigentuemer).unwrap();
        assert_eq!(member.id, member_id);
        assert_eq!(member.status, "aktiv");

        mitglied_status(
            State(st.clone()),
            hdr(&der_owner),
            Path(member_id),
            Json(StatusBody { status: "gesperrt".into() }),
        )
        .await
        .expect("sperren muss klappen");
        let (s,): (String,) = sqlx::query_as("SELECT status FROM geraet WHERE id = ?")
            .bind(member_id)
            .fetch_one(&st.pool)
            .await
            .unwrap();
        assert_eq!(s, "gesperrt");
    }

    /// Der Eigentümer kann sich nicht selbst aussperren (409).
    #[tokio::test]
    async fn eigentuemer_kann_sich_nicht_selbst_sperren() {
        let (st, der_owner, _m, _id) = setup_team().await;
        let liste = mitglieder_liste(State(st.clone()), hdr(&der_owner))
            .await
            .unwrap()
            .0;
        let self_id = liste.iter().find(|m| m.dieses_geraet).unwrap().id;
        let err = mitglied_status(
            State(st.clone()),
            hdr(&der_owner),
            Path(self_id),
            Json(StatusBody { status: "gesperrt".into() }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::CONFLICT);
    }

    /// Ein Nicht-Eigentümer (Mitglied-Gerät) sieht die Mitglieder nicht (403).
    #[tokio::test]
    async fn nur_eigentuemer_sieht_mitglieder() {
        let (st, _o, der_member, _id) = setup_team().await;
        let err = mitglieder_liste(State(st.clone()), hdr(&der_member))
            .await
            .unwrap_err();
        assert_eq!(err, StatusCode::FORBIDDEN);
    }

    /// Der Eigentümer eines Kontos kann kein Gerät eines FREMDEN Kontos sperren.
    #[tokio::test]
    async fn fremdes_konto_bleibt_unberuehrt() {
        let (st, der_owner, _m, _id) = setup_team().await;
        sqlx::query("INSERT INTO konto (id, name) VALUES (2, 'Anderes')")
            .execute(&st.pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO nutzer (id, konto_id, anzeigename, rolle, ist_eigentuemer)
             VALUES (99, 2, 'X', 'mitglied', 0)",
        )
        .execute(&st.pool)
        .await
        .unwrap();
        let der_fremd = b"fremd-cert".to_vec();
        let g = sqlx::query(
            "INSERT INTO geraet (nutzer_id, bezeichnung, cert_fingerprint, status)
             VALUES (99, 'Fremd', ?, 'aktiv')",
        )
        .bind(fp_von(&der_fremd))
        .execute(&st.pool)
        .await
        .unwrap();

        let err = mitglied_status(
            State(st.clone()),
            hdr(&der_owner),
            Path(g.last_insert_rowid()),
            Json(StatusBody { status: "gesperrt".into() }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::NOT_FOUND);
    }

    /// Team erstellen: legt ein neues Konto an, macht das erste Gerät zum
    /// Eigentümer (kopiersicher signiert), und zwei Aufrufe ergeben zwei Konten.
    #[tokio::test]
    async fn team_erstellen_bootstrappt_eigentuemer() {
        let st = test_state().await;
        let key = rcgen::KeyPair::generate().unwrap();
        let antw = team_erstellen(
            State(st.clone()),
            Json(TeamErstellenBody {
                pubkey_pem: key.public_key_pem(),
                name: Some("Mein-Laptop".into()),
                team_name: Some("Ateliers".into()),
            }),
        )
        .await
        .expect("Team erstellen muss klappen")
        .0;
        assert!(antw.ausweis_pem.contains("BEGIN CERTIFICATE"));
        assert!(antw.konto_id >= 2, "neues Konto, nicht das geseedete Konto 1");

        let (eig,): (i64,) = sqlx::query_as("SELECT ist_eigentuemer FROM nutzer WHERE konto_id = ?")
            .bind(antw.konto_id)
            .fetch_one(&st.pool)
            .await
            .unwrap();
        assert_eq!(eig, 1, "erstes Gerät ist Eigentümer");

        let fp = service_ca::fingerprint_von_pem(&antw.ausweis_pem).unwrap();
        let (cnt,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM geraet WHERE cert_fingerprint = ?")
            .bind(&fp)
            .fetch_one(&st.pool)
            .await
            .unwrap();
        assert_eq!(cnt, 1);

        // Zweites Team → eigenes, anderes Konto.
        let key2 = rcgen::KeyPair::generate().unwrap();
        let antw2 = team_erstellen(
            State(st.clone()),
            Json(TeamErstellenBody {
                pubkey_pem: key2.public_key_pem(),
                name: None,
                team_name: None,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_ne!(antw.konto_id, antw2.konto_id);
    }

    /// Ausweis-Erneuerung: ein verbundenes Gerät bekommt einen frischen Ausweis;
    /// der Fingerabdruck wird umgestellt, der bisherige bleibt als Überlappung
    /// gültig (kein Aussperren), und dieselbe Geräte-Zeile bleibt bestehen.
    #[tokio::test]
    async fn ausweis_erneuern_setzt_neuen_fp_mit_ueberlappung() {
        let (st, der_owner, _der_member, _member_id) = setup_team().await;
        let alt_fp = fp_von(&der_owner);

        let (geraet_id,): (i64,) =
            sqlx::query_as("SELECT id FROM geraet WHERE cert_fingerprint = ?")
                .bind(&alt_fp)
                .fetch_one(&st.pool)
                .await
                .unwrap();

        let key = rcgen::KeyPair::generate().unwrap();
        let antw = ausweis_erneuern(
            State(st.clone()),
            hdr(&der_owner),
            Json(ErneuernBody { pubkey_pem: key.public_key_pem() }),
        )
        .await
        .expect("Erneuerung muss klappen")
        .0;
        assert!(antw.ausweis_pem.contains("BEGIN CERTIFICATE"));

        let neu_fp = service_ca::fingerprint_von_pem(&antw.ausweis_pem).unwrap();
        assert_ne!(neu_fp, alt_fp, "der Ausweis muss sich ändern");

        // Dieselbe Zeile, neuer Fingerabdruck, alter als Überlappung.
        let (fp, vorher): (String, Option<String>) = sqlx::query_as(
            "SELECT cert_fingerprint, cert_fingerprint_vorher FROM geraet WHERE id = ?",
        )
        .bind(geraet_id)
        .fetch_one(&st.pool)
        .await
        .unwrap();
        assert_eq!(fp, neu_fp);
        assert_eq!(vorher.as_deref(), Some(alt_fp.as_str()));

        // Überlappung: der ALTE Ausweis wird weiter erkannt (kein Aussperren).
        let (_konto, g2) = konto_und_geraet(&st.pool, &hdr(&der_owner)).await.unwrap();
        assert_eq!(g2, geraet_id);
    }

    /// Grace-Wiederanmeldung: ein abgelaufen-gedachtes, aber echt CA-signiertes
    /// Gerät bekommt mit gültigem Besitznachweis über den öffentlichen Kanal einen
    /// frischen Ausweis (Überlappung gesetzt); ein falscher Besitznachweis (fremder
    /// Schlüssel) wird abgewiesen.
    #[tokio::test]
    async fn grace_erneuerung_mit_besitznachweis() {
        use p256::ecdsa::{signature::Signer, Signature, SigningKey};
        use p256::pkcs8::DecodePrivateKey;

        let st = test_state().await;
        // Altes Gerät: rcgen-Schlüssel, echt von der Service-CA signiert.
        let alt_key = rcgen::KeyPair::generate().unwrap();
        let alt_cert = st
            .service_ca
            .as_ref()
            .as_ref()
            .unwrap()
            .ausweis_kette(&alt_key.public_key_pem(), "Alt-Laptop", "Antrag 3000 Team-Gerät", 90)
            .unwrap();
        let alt_fp = service_ca::fingerprint_von_pem(&alt_cert).unwrap();
        sqlx::query(
            "INSERT INTO geraet (nutzer_id, bezeichnung, cert_fingerprint, status)
             VALUES (1, 'Alt-Laptop', ?, 'aktiv')",
        )
        .bind(&alt_fp)
        .execute(&st.pool)
        .await
        .unwrap();

        // Neuer Schlüssel + Besitznachweis (Signatur über den neuen pubkey mit dem ALTEN Schlüssel).
        let neu_key = rcgen::KeyPair::generate().unwrap();
        let neu_pub = neu_key.public_key_pem();
        let sk = SigningKey::from_pkcs8_pem(&alt_key.serialize_pem()).unwrap();
        let sig: Signature = sk.sign(neu_pub.as_bytes());

        let antw = ausweis_grace(
            State(st.clone()),
            Json(GraceBody {
                ausweis_pem: alt_cert.clone(),
                pubkey_pem: neu_pub.clone(),
                proof: sig.to_der().as_bytes().to_vec(),
            }),
        )
        .await
        .expect("Grace muss klappen")
        .0;
        assert!(antw.ausweis_pem.contains("BEGIN CERTIFICATE"));

        let neu_fp = service_ca::fingerprint_von_pem(&antw.ausweis_pem).unwrap();
        let (fp, vorher): (String, Option<String>) = sqlx::query_as(
            "SELECT cert_fingerprint, cert_fingerprint_vorher FROM geraet WHERE cert_fingerprint = ?",
        )
        .bind(&neu_fp)
        .fetch_one(&st.pool)
        .await
        .unwrap();
        assert_eq!(fp, neu_fp);
        assert_eq!(vorher.as_deref(), Some(alt_fp.as_str()));

        // Falscher Besitznachweis (fremder Schlüssel) → abgewiesen.
        let fremd = rcgen::KeyPair::generate().unwrap();
        let sk_fremd = SigningKey::from_pkcs8_pem(&fremd.serialize_pem()).unwrap();
        let sig_fremd: Signature = sk_fremd.sign(neu_pub.as_bytes());
        let err = ausweis_grace(
            State(st.clone()),
            Json(GraceBody {
                ausweis_pem: alt_cert.clone(),
                pubkey_pem: neu_pub.clone(),
                proof: sig_fremd.to_der().as_bytes().to_vec(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::UNAUTHORIZED);
    }

    /// Zwei-Ebenen-CA: `ausweis_kette` liefert Blatt + Zwischen-CA; das Blatt ist
    /// von der Zwischen-CA signiert, die Zwischen-CA von der Wurzel (vollständige
    /// Kette). Der Ausweis-Fingerabdruck ist der des Blatts – nicht der ganzen Kette.
    #[tokio::test]
    async fn zwischen_ca_kette_verifiziert() {
        use x509_parser::pem::parse_x509_pem;
        let st = test_state().await;
        let ca = st.service_ca.as_ref().as_ref().unwrap();

        let key = rcgen::KeyPair::generate().unwrap();
        let kette = ca
            .ausweis_kette(&key.public_key_pem(), "Laptop", "Antrag 3000 Team-Gerät", 90)
            .unwrap();
        assert_eq!(
            kette.matches("BEGIN CERTIFICATE").count(),
            2,
            "Kette muss Blatt + Zwischen-CA enthalten"
        );

        let (rest, blatt_pem) = parse_x509_pem(kette.as_bytes()).unwrap();
        let (_, zw_pem) = parse_x509_pem(rest).unwrap();
        let (_, w_pem) = parse_x509_pem(ca.wurzel_cert_pem.as_bytes()).unwrap();
        let blatt = blatt_pem.parse_x509().unwrap();
        let zw = zw_pem.parse_x509().unwrap();
        let wurzel = w_pem.parse_x509().unwrap();
        blatt
            .verify_signature(Some(zw.public_key()))
            .expect("Blatt muss von der Zwischen-CA signiert sein");
        zw.verify_signature(Some(wurzel.public_key()))
            .expect("Zwischen-CA muss von der Wurzel signiert sein");

        // Fingerabdruck = der des Blatts (erstes Zertifikat), nicht der Kette.
        let nur_blatt = kette.split("-----END CERTIFICATE-----").next().unwrap().to_string()
            + "-----END CERTIFICATE-----\n";
        assert_eq!(
            service_ca::fingerprint_von_pem(&kette),
            service_ca::fingerprint_von_pem(&nur_blatt),
        );
    }

    /// Förderer verbinden: Token einlösen → Förderer-CA-signierter Ausweis +
    /// Förderer-Zeile angelegt; idempotent, anderer Schlüssel → 409.
    #[tokio::test]
    async fn foerderer_enroll_und_idempotent() {
        let st = test_state().await;
        sqlx::query(
            "INSERT INTO foerderer_einladung (token, ablauf) VALUES ('ftok', datetime('now', '+1 hour'))",
        )
        .execute(&st.pool)
        .await
        .unwrap();

        let key = rcgen::KeyPair::generate().unwrap();
        let pubkey_pem = key.public_key_pem();

        let antw = foerderer_enroll(
            State(st.clone()),
            Json(FoerdererEnrollBody {
                token: "ftok".into(),
                pubkey_pem: pubkey_pem.clone(),
                name: Some("Kulturstiftung".into()),
            }),
        )
        .await
        .expect("Förderer-Enroll muss klappen")
        .0;
        assert!(antw.ausweis_pem.contains("BEGIN CERTIFICATE"));

        let fp = service_ca::fingerprint_von_pem(&antw.ausweis_pem).unwrap();
        let (cnt, name): (i64, String) =
            sqlx::query_as("SELECT COUNT(*), MAX(name) FROM foerderer WHERE cert_fingerprint = ?")
                .bind(&fp)
                .fetch_one(&st.pool)
                .await
                .unwrap();
        assert_eq!(cnt, 1);
        assert_eq!(name, "Kulturstiftung");

        // Idempotent: gleicher Token + Schlüssel → gleicher Ausweis, kein 2. Förderer.
        let wieder = foerderer_enroll(
            State(st.clone()),
            Json(FoerdererEnrollBody {
                token: "ftok".into(),
                pubkey_pem: pubkey_pem.clone(),
                name: None,
            }),
        )
        .await
        .expect("idempotent")
        .0;
        assert_eq!(wieder.ausweis_pem, antw.ausweis_pem);
        let cnt2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foerderer")
            .fetch_one(&st.pool)
            .await
            .unwrap();
        assert_eq!(cnt2, 1);

        // Anderer Schlüssel nach dem Einlösen → 409.
        let anderer = rcgen::KeyPair::generate().unwrap();
        let err = foerderer_enroll(
            State(st.clone()),
            Json(FoerdererEnrollBody {
                token: "ftok".into(),
                pubkey_pem: anderer.public_key_pem(),
                name: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::CONFLICT);
    }

    /// Förderer-Ausweis: Erneuern (mTLS) setzt neuen Fingerabdruck mit Überlappung;
    /// Grace (öffentlich) heilt mit gültigem Besitznachweis, weist einen falschen ab.
    #[tokio::test]
    async fn foerderer_ausweis_erneuern_und_grace() {
        use p256::ecdsa::{signature::Signer, Signature, SigningKey};
        use p256::pkcs8::DecodePrivateKey;

        let st = test_state().await;
        // Verbundenen Förderer anlegen (echt Förderer-CA-signiert).
        let alt_key = rcgen::KeyPair::generate().unwrap();
        let alt_cert = st
            .foerderer_ca
            .as_ref()
            .as_ref()
            .unwrap()
            .signiere(&alt_key.public_key_pem(), "Kulturstiftung", "Antrag 3000 Förderer", 90)
            .unwrap();
        let alt_fp = service_ca::fingerprint_von_pem(&alt_cert).unwrap();
        sqlx::query("INSERT INTO foerderer (name, cert_fingerprint, status) VALUES ('Kulturstiftung', ?, 'aktiv')")
            .bind(&alt_fp)
            .execute(&st.pool)
            .await
            .unwrap();

        // Erneuern (mTLS): Header aus dem Zertifikat-DER bauen.
        let der = {
            let (_, p) = x509_parser::pem::parse_x509_pem(alt_cert.as_bytes()).unwrap();
            p.contents
        };
        let neu_key = rcgen::KeyPair::generate().unwrap();
        let antw = foerderer_ausweis_erneuern(
            State(st.clone()),
            hdr(&der),
            Json(ErneuernBody { pubkey_pem: neu_key.public_key_pem() }),
        )
        .await
        .expect("Erneuern muss klappen")
        .0;
        let neu_fp = service_ca::fingerprint_von_pem(&antw.ausweis_pem).unwrap();
        let (fp, vorher): (String, Option<String>) = sqlx::query_as(
            "SELECT cert_fingerprint, cert_fingerprint_vorher FROM foerderer WHERE cert_fingerprint = ?",
        )
        .bind(&neu_fp)
        .fetch_one(&st.pool)
        .await
        .unwrap();
        assert_eq!(fp, neu_fp);
        assert_eq!(vorher.as_deref(), Some(alt_fp.as_str()));

        // Grace (öffentlich): alter Ausweis + Besitznachweis mit dem ALTEN Schlüssel.
        let neu2 = rcgen::KeyPair::generate().unwrap();
        let neu2_pub = neu2.public_key_pem();
        let sk = SigningKey::from_pkcs8_pem(&alt_key.serialize_pem()).unwrap();
        let sig: Signature = sk.sign(neu2_pub.as_bytes());
        let g = foerderer_ausweis_grace(
            State(st.clone()),
            Json(GraceBody {
                ausweis_pem: alt_cert.clone(),
                pubkey_pem: neu2_pub.clone(),
                proof: sig.to_der().as_bytes().to_vec(),
            }),
        )
        .await
        .expect("Grace muss klappen")
        .0;
        assert!(g.ausweis_pem.contains("BEGIN CERTIFICATE"));

        // Falscher Besitznachweis (fremder Schlüssel) → abgewiesen.
        let fremd = rcgen::KeyPair::generate().unwrap();
        let sk_f = SigningKey::from_pkcs8_pem(&fremd.serialize_pem()).unwrap();
        let sig_f: Signature = sk_f.sign(neu2_pub.as_bytes());
        let err = foerderer_ausweis_grace(
            State(st.clone()),
            Json(GraceBody {
                ausweis_pem: alt_cert.clone(),
                pubkey_pem: neu2_pub.clone(),
                proof: sig_f.to_der().as_bytes().to_vec(),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::UNAUTHORIZED);
    }

    /// Ein verbundener Förderer legt ein Programm an, sieht es, zieht es zurück
    /// (Soft-Delete); ein unbekanntes Zertifikat wird abgewiesen (403).
    #[tokio::test]
    async fn foerderer_programme_crud() {
        let st = test_state().await;
        let der = b"foerderer-cert".to_vec();
        sqlx::query("INSERT INTO foerderer (name, cert_fingerprint, status) VALUES ('Stiftung', ?, 'aktiv')")
            .bind(fp_von(&der))
            .execute(&st.pool)
            .await
            .unwrap();

        // Anlegen.
        foerderer_programm_schreiben(
            State(st.clone()),
            hdr(&der),
            Path("p1".into()),
            Json(ProgrammBody { inhalt: serde_json::json!({ "name": "Programm A" }) }),
        )
        .await
        .expect("anlegen");

        // Lesen.
        let liste = foerderer_programme_lesen(State(st.clone()), hdr(&der))
            .await
            .unwrap()
            .0;
        assert_eq!(liste.len(), 1);
        assert_eq!(liste[0].programm_id, "p1");
        assert_eq!(liste[0].status, "aktiv");

        // Zurückziehen (Soft-Delete).
        foerderer_programm_loeschen(State(st.clone()), hdr(&der), Path("p1".into()))
            .await
            .expect("zurückziehen");
        let (status,): (String,) =
            sqlx::query_as("SELECT status FROM foerderer_programm WHERE programm_id = 'p1'")
                .fetch_one(&st.pool)
                .await
                .unwrap();
        assert_eq!(status, "zurueckgezogen");

        // Fehlender Name → 400.
        let err = foerderer_programm_schreiben(
            State(st.clone()),
            hdr(&der),
            Path("p2".into()),
            Json(ProgrammBody { inhalt: serde_json::json!({ "foo": "bar" }) }),
        )
        .await
        .unwrap_err();
        assert_eq!(err, StatusCode::BAD_REQUEST);

        // Unbekanntes Zertifikat → 403.
        let err = foerderer_programme_lesen(State(st.clone()), hdr(b"fremd"))
            .await
            .unwrap_err();
        assert_eq!(err, StatusCode::FORBIDDEN);
    }

    /// Admin-Kuratierung (6c): ein Förderer-Programm freigeben → landet unter
    /// einer Namensraum-id im Katalog; zurückgezogen + freigeben → wieder raus.
    #[tokio::test]
    async fn admin_kuratiert_foerderer_programme() {
        let st = test_state().await;
        // Admin-Sitzung direkt anlegen + passender Header.
        st.sitzungen.lock().unwrap().insert(
            "admintok".into(),
            AdminSitzung { konto_id: 1, geraet_id: 1, ablauf: Instant::now() + Duration::from_secs(300) },
        );
        let admin_hdr = || {
            let mut h = HeaderMap::new();
            h.insert("x-admin-token", "admintok".parse().unwrap());
            h
        };
        // Admin-Gerät (id 1) – in Produktion real; hier für den FK
        // geaendert_von_geraet beim Katalog-Schreiben nötig.
        sqlx::query("INSERT INTO geraet (id, nutzer_id, bezeichnung, cert_fingerprint, status) VALUES (1, 1, 'Admin', 'adminfp', 'aktiv')")
            .execute(&st.pool)
            .await
            .unwrap();
        // Basis-Katalog für Konto 1 + ein Förderer mit einem Programm.
        sqlx::query(
            "INSERT INTO katalog_aktuell (konto_id, inhalt_json, schema_version) VALUES (1, '{\"schema_version\":1,\"foerderungen\":[]}', 1)",
        )
        .execute(&st.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO foerderer (id, name, cert_fingerprint, status) VALUES (7, 'Stiftung', 'fp7', 'aktiv')")
            .execute(&st.pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO foerderer_programm (foerderer_id, programm_id, inhalt_json, status) VALUES (7, 'p1', '{\"name\":\"Programm A\"}', 'aktiv')")
            .execute(&st.pool)
            .await
            .unwrap();

        // Liste zeigt es als offen.
        let liste = admin_foerderer_programme(State(st.clone()), admin_hdr()).await.unwrap().0;
        assert_eq!(liste.len(), 1);
        assert!(liste[0].offen);

        // Freigeben → landet unter f7-p1 im Katalog.
        admin_foerderer_programm_freigeben(State(st.clone()), admin_hdr(), Path((7, "p1".into())))
            .await
            .expect("freigeben");
        let (kat,): (String,) =
            sqlx::query_as("SELECT inhalt_json FROM katalog_aktuell WHERE konto_id = 1")
                .fetch_one(&st.pool)
                .await
                .unwrap();
        assert!(kat.contains("f7-p1"));
        assert!(kat.contains("Programm A"));

        // Danach nicht mehr offen.
        let liste = admin_foerderer_programme(State(st.clone()), admin_hdr()).await.unwrap().0;
        assert!(!liste[0].offen);

        // Zurückziehen + freigeben → aus dem Katalog entfernt.
        sqlx::query("UPDATE foerderer_programm SET status = 'zurueckgezogen', geaendert_am = datetime('now', '+1 second') WHERE foerderer_id = 7 AND programm_id = 'p1'")
            .execute(&st.pool)
            .await
            .unwrap();
        admin_foerderer_programm_freigeben(State(st.clone()), admin_hdr(), Path((7, "p1".into())))
            .await
            .expect("entfernen");
        let (kat,): (String,) =
            sqlx::query_as("SELECT inhalt_json FROM katalog_aktuell WHERE konto_id = 1")
                .fetch_one(&st.pool)
                .await
                .unwrap();
        assert!(!kat.contains("f7-p1"));

        // Verbundene-Förderer-Liste.
        let vf = admin_foerderer_verbunden(State(st.clone()), admin_hdr()).await.unwrap().0;
        assert_eq!(vf.len(), 1);
        assert_eq!(vf[0].name, "Stiftung");
    }

    /// Abo-Gate-Logik (Schritt 5): aus = immer frei; scharf = Einzelplatz frei,
    /// ab dem 2. Gerät nur mit gültigem Abo.
    #[test]
    fn abo_gate_logik() {
        // Gating AUS: immer erlaubt, egal was.
        assert!(abo_gate_entscheiden(false, Some("abgelaufen"), 9));
        assert!(abo_gate_entscheiden(false, None, 9));

        // Gating AN, gültiges Abo: immer erlaubt.
        assert!(abo_gate_entscheiden(true, Some("aktiv"), 9));
        assert!(abo_gate_entscheiden(true, Some("testphase"), 5));
        assert!(abo_gate_entscheiden(true, Some("frei"), 3));

        // Gating AN, kein gültiges Abo: nur Einzelplatz (≤ 1 Gerät) frei.
        assert!(abo_gate_entscheiden(true, Some("abgelaufen"), 1));
        assert!(abo_gate_entscheiden(true, Some("abgelaufen"), 0));
        assert!(!abo_gate_entscheiden(true, Some("abgelaufen"), 2));
        assert!(!abo_gate_entscheiden(true, None, 2));
    }

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
