// ============================================================
// Der Datentresor: verschluesselte lokale Speicherung
// ============================================================
//
// Aufbau der Datei tresor.enc:
//
//   [Magic "ANTRAG3K" 8 Byte][Version 1 Byte][Salt 16 Byte]
//   [Nonce 12 Byte][verschluesselte Daten ...]
//
// - Magic:   Erkennungszeichen, damit wir fremde Dateien sofort
//            zurueckweisen koennen.
// - Version: erlaubt spaetere Format-Aenderungen, ohne alte
//            Tresore zu brechen.
// - Salt:    Zufallswert fuer die Schluessel-Ableitung. Sorgt dafuer,
//            dass dasselbe Passwort bei jedem Nutzer einen anderen
//            Schluessel ergibt (vereitelt vorberechnete Tabellen).
// - Nonce:   Zufallswert fuer die Verschluesselung, bei JEDEM
//            Speichern neu (Pflicht bei AES-GCM).
//
// Sicherheitsprinzip: Das Passwort selbst wird NIRGENDS gespeichert.
// Aus Passwort + Salt wird per Argon2id der Schluessel abgeleitet.
// Ob das Passwort stimmt, zeigt allein das Echtheits-Siegel von
// AES-GCM beim Entschluesseln.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use tauri::Manager;
use zeroize::{Zeroize, Zeroizing};

pub(crate) const MAGIC: &[u8; 8] = b"ANTRAG3K";
const VERSION: u8 = 1;
pub(crate) const SALT_LAENGE: usize = 16;
const NONCE_LAENGE: usize = 12;
const KOPF_LAENGE: usize = 8 + 1 + SALT_LAENGE + NONCE_LAENGE;

/// Wird nach erfolgreichem Entsperren im Arbeitsspeicher gehalten
/// (nie auf der Festplatte). Beim Sperren oder Beenden ist es weg.
pub struct Geheimnis {
    schluessel: [u8; 32],
    salt: [u8; SALT_LAENGE],
    /// true, wenn diese Sitzung ueber "Auf diesem Geraet merken" (ohne
    /// Passwort) geoeffnet wurde. Nur dann darf ein neues Passwort ohne das
    /// alte gesetzt werden (tresor_passwort_neu_setzen).
    ueber_merken: bool,
}

/// Beim Verwerfen (Sperren oder Programmende) den Schluessel sicher mit
/// Nullen ueberschreiben, statt ihn nur freizugeben. Sonst koennte er als
/// Rest im Heap/Auslagerungsdatei/Crashdump zurueckbleiben. (Audit A1.)
impl Drop for Geheimnis {
    fn drop(&mut self) {
        self.schluessel.zeroize();
        self.salt.zeroize();
    }
}

/// Tauri verwaltet diesen Zustand und reicht ihn an die Befehle durch.
#[derive(Default)]
pub struct TresorZustand {
    pub(crate) geheim: Mutex<Option<Geheimnis>>,
}

/// Ordner fuer Anwendungsdaten (%APPDATA%\com.antrag3000.app) plus Dateiname.
pub(crate) fn tresor_pfad(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let ordner = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Datenordner nicht ermittelbar: {e}"))?;
    fs::create_dir_all(&ordner).map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
    Ok(ordner.join("tresor.enc"))
}

/// Passwort + Salt -> 32-Byte-Schluessel per Argon2id.
/// Die Standardwerte des Crates entsprechen den aktuellen
/// OWASP-Empfehlungen (19 MiB Speicher, 2 Durchgaenge).
pub(crate) fn schluessel_ableiten(passwort: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut schluessel = [0u8; 32];
    argon2::Argon2::default()
        .hash_password_into(passwort.as_bytes(), salt, &mut schluessel)
        .map_err(|e| format!("Schluessel-Ableitung fehlgeschlagen: {e}"))?;
    Ok(schluessel)
}

/// Daten verschluesseln und als kompletten Dateiinhalt zurueckgeben.
fn verschluesseln(daten: &str, schluessel: &[u8; 32], salt: &[u8; SALT_LAENGE]) -> Result<Vec<u8>, String> {
    // Nonce: bei jedem Speichern frisch aus dem Betriebssystem-Zufall.
    let mut nonce = [0u8; NONCE_LAENGE];
    OsRng.fill_bytes(&mut nonce);

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(schluessel));
    let verschluesselt = cipher
        .encrypt(Nonce::from_slice(&nonce), daten.as_bytes())
        .map_err(|_| "Verschluesselung fehlgeschlagen".to_string())?;

    let mut inhalt = Vec::with_capacity(KOPF_LAENGE + verschluesselt.len());
    inhalt.extend_from_slice(MAGIC);
    inhalt.push(VERSION);
    inhalt.extend_from_slice(salt);
    inhalt.extend_from_slice(&nonce);
    inhalt.extend_from_slice(&verschluesselt);
    Ok(inhalt)
}

/// Sicheres Schreiben: erst Temp-Datei, dann Vorversion als .bak
/// behalten, dann umbenennen. So zerstoert ein Absturz mitten im
/// Schreiben nie den vorhandenen Tresor.
fn sicher_schreiben(pfad: &PathBuf, inhalt: &[u8]) -> Result<(), String> {
    let tmp = pfad.with_extension("enc.tmp");
    fs::write(&tmp, inhalt).map_err(|e| format!("Schreiben fehlgeschlagen: {e}"))?;

    if pfad.exists() {
        let bak = pfad.with_extension("enc.bak");
        if bak.exists() {
            fs::remove_file(&bak).map_err(|e| format!("Alte Sicherung nicht loeschbar: {e}"))?;
        }
        fs::rename(pfad, &bak).map_err(|e| format!("Sicherung fehlgeschlagen: {e}"))?;
    }
    fs::rename(&tmp, pfad).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))?;
    Ok(())
}

// ============================================================
// Tauri-Befehle: das ist die Schnittstelle, die das JS-Frontend
// per invoke(...) aufrufen kann. Mehr kann das Frontend nicht --
// es hat nie direkten Zugriff auf Schluessel oder Dateisystem.
// ============================================================

/// Gibt es schon einen Tresor? -> "fehlt" oder "vorhanden"
#[tauri::command]
pub fn tresor_status(app: tauri::AppHandle) -> Result<String, String> {
    let pfad = tresor_pfad(&app)?;
    Ok(if pfad.exists() { "vorhanden".into() } else { "fehlt".into() })
}

/// Erster Start: neuen Tresor mit Passwort und Anfangsdaten anlegen.
#[tauri::command]
pub fn tresor_erstellen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    mut passwort: String,
    daten: String,
) -> Result<(), String> {
    if passwort.chars().count() < 8 {
        return Err("Das Passwort muss mindestens 8 Zeichen haben.".into());
    }
    let pfad = tresor_pfad(&app)?;
    if pfad.exists() {
        return Err("Es existiert bereits ein Tresor.".into());
    }

    // Salt einmalig pro Tresor wuerfeln, Schluessel ableiten.
    let mut salt = [0u8; SALT_LAENGE];
    OsRng.fill_bytes(&mut salt);
    let mut schluessel = schluessel_ableiten(&passwort, &salt)?;

    let inhalt = verschluesseln(&daten, &schluessel, &salt)?;
    sicher_schreiben(&pfad, &inhalt)?;

    // Schluessel im Arbeitsspeicher behalten, damit Speichern ohne
    // erneute Passworteingabe moeglich ist.
    *state.geheim.lock().unwrap() = Some(Geheimnis { schluessel, salt, ueber_merken: false });
    // Lokale Kopien von Schluessel und Passwort ueberschreiben (Audit A1);
    // der Schluessel im Tresor-Zustand bleibt erhalten.
    schluessel.zeroize();
    passwort.zeroize();
    Ok(())
}

/// Tresor mit Passwort oeffnen. Liefert die entschluesselten Daten (JSON).
#[tauri::command]
pub fn tresor_entsperren(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    mut passwort: String,
) -> Result<String, String> {
    let pfad = tresor_pfad(&app)?;
    let inhalt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;

    if inhalt.len() < KOPF_LAENGE || &inhalt[..8] != MAGIC {
        return Err("Die Tresor-Datei ist beschaedigt oder kein Antrag-3000-Tresor.".into());
    }
    if inhalt[8] != VERSION {
        return Err("Diese Tresor-Version wird noch nicht unterstuetzt.".into());
    }

    let mut salt = [0u8; SALT_LAENGE];
    salt.copy_from_slice(&inhalt[9..9 + SALT_LAENGE]);
    let nonce = &inhalt[9 + SALT_LAENGE..KOPF_LAENGE];
    let verschluesselt = &inhalt[KOPF_LAENGE..];

    let mut schluessel = schluessel_ableiten(&passwort, &salt)?;
    passwort.zeroize(); // Passwort wird nicht mehr gebraucht (Audit A1).
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&schluessel));

    // Bricht das Echtheits-Siegel, ist (praktisch immer) das Passwort
    // falsch. Wir geben einen festen Code zurueck, den das Frontend
    // in eine freundliche Meldung uebersetzt.
    let klartext = cipher
        .decrypt(Nonce::from_slice(nonce), verschluesselt)
        .map_err(|_| {
            schluessel.zeroize();
            "falsches_passwort".to_string()
        })?;

    let daten = String::from_utf8(klartext)
        .map_err(|_| "Tresor-Inhalt ist kein gueltiger Text.".to_string())?;

    *state.geheim.lock().unwrap() = Some(Geheimnis { schluessel, salt, ueber_merken: false });
    schluessel.zeroize(); // lokale Kopie ueberschreiben (Audit A1).
    Ok(daten)
}

/// Aktuellen Datenstand verschluesselt speichern (Tresor muss offen sein).
#[tauri::command]
pub fn tresor_speichern(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    daten: String,
) -> Result<(), String> {
    let geheim = state.geheim.lock().unwrap();
    let geheim = geheim
        .as_ref()
        .ok_or("Der Tresor ist nicht entsperrt.".to_string())?;

    let pfad = tresor_pfad(&app)?;
    let inhalt = verschluesseln(&daten, &geheim.schluessel, &geheim.salt)?;
    sicher_schreiben(&pfad, &inhalt)
}

/// Tresor sperren: Schluessel aus dem Arbeitsspeicher entfernen.
#[tauri::command]
pub fn tresor_sperren(state: tauri::State<TresorZustand>) {
    *state.geheim.lock().unwrap() = None;
}

/// Fuer die Sicherung mit Dateien: Schluessel und Salt der offenen Sitzung samt
/// der dazu passenden Tresor-Datei. Das Schloss haelt gleichzeitiges Speichern
/// fern, und gesichert wird nur ein Tresor, der sich auch oeffnen laesst.
pub(crate) fn offener_stand(
    app: &tauri::AppHandle,
) -> Result<(Zeroizing<[u8; 32]>, [u8; SALT_LAENGE], Vec<u8>), String> {
    let zustand = app.state::<TresorZustand>();
    let geheim = zustand.geheim.lock().unwrap();
    let geheim = geheim
        .as_ref()
        .ok_or("Der Tresor ist nicht entsperrt.".to_string())?;

    let inhalt = fs::read(tresor_pfad(app)?).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
    if inhalt.len() < KOPF_LAENGE || &inhalt[..8] != MAGIC || inhalt[9..9 + SALT_LAENGE] != geheim.salt {
        return Err("Die Tresor-Datei passt nicht zur offenen Sitzung. Bitte sperren und neu entsperren.".into());
    }
    let pruefung = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&geheim.schluessel)).decrypt(
        Nonce::from_slice(&inhalt[9 + SALT_LAENGE..KOPF_LAENGE]),
        &inhalt[KOPF_LAENGE..],
    );
    match pruefung {
        Ok(mut klartext) => klartext.zeroize(),
        Err(_) => return Err("Die Tresor-Datei passt nicht zur offenen Sitzung. Bitte sperren und neu entsperren.".into()),
    }
    Ok((Zeroizing::new(geheim.schluessel), geheim.salt, inhalt))
}

/// Womit der Zugang vor einem Passwortwechsel nachgewiesen wird.
enum Nachweis<'a> {
    /// Das bisherige Passwort (normaler Weg).
    Passwort(&'a str),
    /// Der Schluessel der offenen Sitzung – nur erlaubt, wenn sie ueber
    /// "Auf diesem Geraet merken" geoeffnet wurde (Passwort vergessen).
    Schluessel(&'a [u8; 32]),
}

/// Kern von "Passwort aendern" (ohne Dateisystem, damit testbar):
/// prueft den Nachweis am vorhandenen Dateiinhalt (das AES-GCM-Siegel
/// muss passen) und verschluesselt `daten` mit einem NEUEN Salt und dem aus
/// dem NEUEN Passwort abgeleiteten Schluessel. Liefert den neuen Dateiinhalt
/// samt Schluessel und Salt. Fehlercode "falsches_passwort" wie beim
/// Entsperren.
fn mit_neuem_passwort(
    inhalt_alt: &[u8],
    nachweis: Nachweis,
    neues_passwort: &str,
    daten: &str,
) -> Result<(Vec<u8>, [u8; 32], [u8; SALT_LAENGE]), String> {
    if neues_passwort.chars().count() < 8 {
        return Err("Das neue Passwort muss mindestens 8 Zeichen haben.".into());
    }
    if inhalt_alt.len() < KOPF_LAENGE || &inhalt_alt[..8] != MAGIC || inhalt_alt[8] != VERSION {
        return Err("Die Tresor-Datei ist beschaedigt oder kein Antrag-3000-Tresor.".into());
    }
    let salt_alt = &inhalt_alt[9..9 + SALT_LAENGE];
    let nonce = &inhalt_alt[9 + SALT_LAENGE..KOPF_LAENGE];
    let mut schluessel_alt = match nachweis {
        Nachweis::Passwort(p) => schluessel_ableiten(p, salt_alt)?,
        Nachweis::Schluessel(k) => *k,
    };
    let pruefung = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&schluessel_alt))
        .decrypt(Nonce::from_slice(nonce), &inhalt_alt[KOPF_LAENGE..]);
    schluessel_alt.zeroize();
    if let Ok(mut klartext) = pruefung {
        klartext.zeroize();
    } else {
        return Err("falsches_passwort".into());
    }

    // Frisches Salt: der neue Schluessel hat nichts mit dem alten gemein.
    let mut salt = [0u8; SALT_LAENGE];
    OsRng.fill_bytes(&mut salt);
    let schluessel = schluessel_ableiten(neues_passwort, &salt)?;
    let inhalt = verschluesseln(daten, &schluessel, &salt)?;
    Ok((inhalt, schluessel, salt))
}

/// Tresor-Passwort aendern (Tresor muss offen sein) – mit dem bisherigen
/// Passwort als Nachweis. `daten` ist der aktuelle Datenstand (wie bei
/// tresor_speichern). Rueckgabe siehe passwort_abschliessen.
#[tauri::command]
pub fn tresor_passwort_aendern(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    mut altes_passwort: String,
    mut neues_passwort: String,
    daten: String,
) -> Result<bool, String> {
    let mut geheim = state.geheim.lock().unwrap();
    if geheim.is_none() {
        altes_passwort.zeroize();
        neues_passwort.zeroize();
        return Err("Der Tresor ist nicht entsperrt.".into());
    }
    let pfad = tresor_pfad(&app)?;
    let inhalt_alt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
    let ergebnis =
        mit_neuem_passwort(&inhalt_alt, Nachweis::Passwort(&altes_passwort), &neues_passwort, &daten);
    altes_passwort.zeroize();
    neues_passwort.zeroize();
    let (inhalt, schluessel, salt) = ergebnis?;
    passwort_abschliessen(&app, &mut geheim, &pfad, inhalt, schluessel, salt)
}

/// Wurde die aktuelle Sitzung ueber "Auf diesem Geraet merken" geoeffnet?
/// (Nur fuer die Anzeige – erzwungen wird es in tresor_passwort_neu_setzen.)
#[tauri::command]
pub fn tresor_ueber_merken(state: tauri::State<TresorZustand>) -> bool {
    state.geheim.lock().unwrap().as_ref().map(|g| g.ueber_merken).unwrap_or(false)
}

/// Passwort vergessen, aber per "Auf diesem Geraet merken" entsperrt: neues
/// Passwort festlegen OHNE das alte. Nur erlaubt, wenn diese Sitzung
/// tatsaechlich ueber das Merken geoeffnet wurde. Begruendung: Wer an diesem
/// Windows-Konto sitzt, kommt ueber das Merken ohnehin an alle Daten (die
/// bewusste Abwaegung von "Auf diesem Geraet merken") – die Abfrage des alten
/// Passworts schuetzt in genau dieser Lage nicht zusaetzlich. Als Nachweis
/// dient der Schluessel der offenen Sitzung; er muss die Tresor-Datei oeffnen.
#[tauri::command]
pub fn tresor_passwort_neu_setzen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    mut neues_passwort: String,
    daten: String,
) -> Result<bool, String> {
    let mut geheim = state.geheim.lock().unwrap();
    let mut sitzungs_schluessel = match geheim.as_ref() {
        Some(g) if g.ueber_merken => g.schluessel,
        Some(_) => {
            neues_passwort.zeroize();
            return Err("nur_ueber_merken".into());
        }
        None => {
            neues_passwort.zeroize();
            return Err("Der Tresor ist nicht entsperrt.".into());
        }
    };
    let ergebnis = tresor_pfad(&app).and_then(|pfad| {
        let alt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
        let neu = mit_neuem_passwort(&alt, Nachweis::Schluessel(&sitzungs_schluessel), &neues_passwort, &daten)?;
        Ok((pfad, neu))
    });
    sitzungs_schluessel.zeroize();
    neues_passwort.zeroize();
    let (pfad, (inhalt, schluessel, salt)) = ergebnis?;
    passwort_abschliessen(&app, &mut geheim, &pfad, inhalt, schluessel, salt)
}

/// Gemeinsamer Abschluss beider Wege:
///  - neue Datei schreiben und zur Kontrolle mit dem neuen Schluessel oeffnen,
///  - die Vorversion .bak LOESCHEN – sie waere noch mit dem alten Passwort
///    lesbar, und genau das soll ein Passwortwechsel verhindern,
///  - "Auf diesem Geraet merken" mit dem neuen Schluessel erneuern,
///  - den Schluessel der Sitzung austauschen.
/// Rueckgabe: true, wenn das Merken erneuert wurde (bzw. keins aktiv war);
/// false, wenn es entfernt werden musste.
fn passwort_abschliessen(
    app: &tauri::AppHandle,
    geheim: &mut Option<Geheimnis>,
    pfad: &PathBuf,
    inhalt: Vec<u8>,
    mut schluessel: [u8; 32],
    salt: [u8; SALT_LAENGE],
) -> Result<bool, String> {
    sicher_schreiben(pfad, &inhalt)?;
    // Kontrolle: die geschriebene Datei muss sich mit dem neuen Schluessel oeffnen.
    let geschrieben = fs::read(pfad).map_err(|e| format!("Kontrolle nicht lesbar: {e}"))?;
    let kontrolle = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&schluessel)).decrypt(
        Nonce::from_slice(&geschrieben[9 + SALT_LAENGE..KOPF_LAENGE]),
        &geschrieben[KOPF_LAENGE..],
    );
    match kontrolle {
        Ok(mut k) => k.zeroize(),
        Err(_) => {
            // Sollte nie passieren: alte Fassung aus der .bak zurueckholen.
            let bak = pfad.with_extension("enc.bak");
            let _ = fs::rename(&bak, pfad);
            schluessel.zeroize();
            return Err("Kontrolle fehlgeschlagen – das alte Passwort gilt weiter.".into());
        }
    }
    let bak = pfad.with_extension("enc.bak");
    if bak.exists() {
        fs::remove_file(&bak).map_err(|e| format!("Alte Vorversion nicht loeschbar: {e}"))?;
    }

    // "Merken" erneuern, falls aktiv – sonst passte es nicht mehr zum Tresor.
    let m_pfad = merken_pfad(app)?;
    let mut merken_ok = true;
    if m_pfad.exists() {
        let erneuert = dpapi_schuetzen(&schluessel)
            .and_then(|geschuetzt| fs::write(&m_pfad, geschuetzt).map_err(|e| e.to_string()));
        if erneuert.is_err() {
            let _ = fs::remove_file(&m_pfad);
            merken_ok = false;
        }
    }

    // Der "vergessen"-Weg bleibt offen, solange das Merken noch gilt.
    let ueber_merken = geheim.as_ref().map(|g| g.ueber_merken).unwrap_or(false) && merken_ok;
    *geheim = Some(Geheimnis { schluessel, salt, ueber_merken });
    schluessel.zeroize(); // lokale Kopie (Audit A1)
    Ok(merken_ok)
}

/// "Neu aufsetzen" bei Passwortverlust: Die alte Datei wird NICHT
/// geloescht, sondern mit Datum umbenannt (falls das Passwort doch
/// wieder einfaellt). Danach kann ein frischer Tresor angelegt werden.
#[tauri::command]
pub fn tresor_neu_aufsetzen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    datum: String,
) -> Result<(), String> {
    // Das Datum kommt aus dem Frontend; wir lassen nur Ziffern und
    // Bindestriche zu, damit kein unsinniger Dateiname entstehen kann.
    if datum.is_empty() || datum.len() > 10 || !datum.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err("Ungueltiges Datum.".into());
    }

    let pfad = tresor_pfad(&app)?;
    if !pfad.exists() {
        return Err("Es gibt keinen Tresor zum Neu-Aufsetzen.".into());
    }
    let ordner = pfad.parent().ok_or("Pfad-Fehler".to_string())?;

    // Freien Namen finden: tresor-alt-2026-06-12.enc, -2, -3, ...
    let mut ziel = ordner.join(format!("tresor-alt-{datum}.enc"));
    let mut nr = 2;
    while ziel.exists() {
        ziel = ordner.join(format!("tresor-alt-{datum}-{nr}.enc"));
        nr += 1;
    }
    fs::rename(&pfad, &ziel).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))?;

    // Auch die .bak-Vorversion beiseitelegen, falls vorhanden.
    let bak = pfad.with_extension("enc.bak");
    if bak.exists() {
        let ziel_bak = ziel.with_extension("enc.bak");
        let _ = fs::rename(&bak, &ziel_bak);
    }

    // Ein gespeichertes "Auf diesem Geraet merken" passt nicht mehr zum
    // neuen Tresor -> entfernen, sonst schluege das Auto-Entsperren fehl.
    let _ = fs::remove_file(merken_pfad(&app)?);

    *state.geheim.lock().unwrap() = None;
    Ok(())
}

// ============================================================
// "Auf diesem Geraet merken" (Windows DPAPI)
// ============================================================
//
// Idee: Der Tresor bleibt unveraendert mit dem aus dem Passwort
// abgeleiteten Schluessel AES-verschluesselt. Zusaetzlich legen wir den
// 32-Byte-SCHLUESSEL in einer Datei merken.bin ab -- aber NICHT im
// Klartext, sondern mit Windows DPAPI an das angemeldete Windows-Konto
// gebunden (CryptProtectData). Nur derselbe Windows-Benutzer auf demselben
// Geraet kann ihn wieder entschluesseln. So startet die App ohne Passwort;
// das Passwort bleibt der Rueckfall, und "Geraet vergessen" loescht die
// Datei wieder.
//
// Sicherheits-Abwaegung (vom Nutzer fuer die Pilotphase so gewaehlt): Wer
// am entsperrten Windows-Konto sitzt, kann die App oeffnen. Die Tresor-
// Datei selbst bleibt fuer fremde Konten/Geraete unbrauchbar.

fn merken_pfad(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let ordner = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Datenordner nicht ermittelbar: {e}"))?;
    fs::create_dir_all(&ordner).map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
    Ok(ordner.join("merken.bin"))
}

/// Ist "Auf diesem Geraet merken" auf dieser Plattform ueberhaupt moeglich?
/// Nur Windows hat DPAPI (passwortloses Entsperren, an das Windows-Konto
/// gebunden). Auf macOS/Linux gibt es das (noch) nicht – das Frontend
/// blendet die Option dann aus, damit niemand auf einen Fehler laeuft.
#[tauri::command]
pub fn merken_moeglich() -> bool {
    cfg!(target_os = "windows")
}

/// Ist auf diesem Geraet ein passwortloses Entsperren hinterlegt?
#[tauri::command]
pub fn merken_status(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(merken_pfad(&app)?.exists())
}

/// Den aktuell offenen Tresor-Schluessel fuer dieses Geraet merken.
/// Der Tresor muss dafuer entsperrt sein.
#[tauri::command]
pub fn merken_anlegen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
) -> Result<(), String> {
    let geheim = state.geheim.lock().unwrap();
    let geheim = geheim
        .as_ref()
        .ok_or("Der Tresor ist nicht entsperrt.".to_string())?;
    let geschuetzt = dpapi_schuetzen(&geheim.schluessel)?;
    fs::write(merken_pfad(&app)?, geschuetzt)
        .map_err(|e| format!("Konnte das Merken nicht speichern: {e}"))?;
    Ok(())
}

/// Ohne Passwort entsperren: gemerkten Schluessel per DPAPI holen und den
/// Tresor damit oeffnen. Liefert die Daten (JSON) wie tresor_entsperren.
#[tauri::command]
pub fn merken_entsperren(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
) -> Result<String, String> {
    let m_pfad = merken_pfad(&app)?;
    let geschuetzt = fs::read(&m_pfad).map_err(|e| format!("Kein gemerkter Zugang: {e}"))?;
    let schluessel_vec = dpapi_entschuetzen(&geschuetzt).map_err(|e| {
        // Nicht entschluesselbar (anderes Konto/Geraet) -> verwerfen.
        let _ = fs::remove_file(&m_pfad);
        e
    })?;
    if schluessel_vec.len() != 32 {
        let _ = fs::remove_file(&m_pfad);
        return Err("merken_ungueltig".into());
    }
    let mut schluessel = [0u8; 32];
    schluessel.copy_from_slice(&schluessel_vec);

    let pfad = tresor_pfad(&app)?;
    let inhalt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
    if inhalt.len() < KOPF_LAENGE || &inhalt[..8] != MAGIC || inhalt[8] != VERSION {
        return Err("Die Tresor-Datei ist beschaedigt oder kein Antrag-3000-Tresor.".into());
    }
    let mut salt = [0u8; SALT_LAENGE];
    salt.copy_from_slice(&inhalt[9..9 + SALT_LAENGE]);
    let nonce = &inhalt[9 + SALT_LAENGE..KOPF_LAENGE];
    let verschluesselt = &inhalt[KOPF_LAENGE..];

    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&schluessel));
    let klartext = cipher
        .decrypt(Nonce::from_slice(nonce), verschluesselt)
        .map_err(|_| {
            // Schluessel passt nicht mehr zum Tresor (z. B. neu aufgesetzt).
            let _ = fs::remove_file(&m_pfad);
            "merken_ungueltig".to_string()
        })?;
    let daten = String::from_utf8(klartext)
        .map_err(|_| "Tresor-Inhalt ist kein gueltiger Text.".to_string())?;

    *state.geheim.lock().unwrap() = Some(Geheimnis { schluessel, salt, ueber_merken: true });
    Ok(daten)
}

/// Den gemerkten Zugang dieses Geraets entfernen (kuenftig wieder Passwort).
#[tauri::command]
pub fn merken_vergessen(app: tauri::AppHandle) -> Result<(), String> {
    let pfad = merken_pfad(&app)?;
    if pfad.exists() {
        fs::remove_file(&pfad).map_err(|e| format!("Konnte nicht entfernen: {e}"))?;
    }
    Ok(())
}

/// Daten mit DPAPI an das Windows-Konto binden (CryptProtectData).
#[cfg(target_os = "windows")]
fn dpapi_schuetzen(daten: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows_sys::Win32::Foundation::LocalFree;
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: daten.len() as u32,
            pbData: daten.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        );
        if ok == 0 {
            return Err("Windows-Schutz (DPAPI) fehlgeschlagen.".into());
        }
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        Ok(bytes)
    }
}

/// Mit DPAPI gebundene Daten wieder entschluesseln (CryptUnprotectData).
#[cfg(target_os = "windows")]
fn dpapi_entschuetzen(daten: &[u8]) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows_sys::Win32::Foundation::LocalFree;
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: daten.len() as u32,
            pbData: daten.as_ptr() as *mut u8,
        };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        );
        if ok == 0 {
            return Err("Gemerkter Zugang nicht entschluesselbar.".into());
        }
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as _);
        Ok(bytes)
    }
}

// Auf Nicht-Windows-Systemen ist das Merken (noch) nicht verfuegbar.
#[cfg(not(target_os = "windows"))]
fn dpapi_schuetzen(_daten: &[u8]) -> Result<Vec<u8>, String> {
    Err("\"Auf diesem Geraet merken\" ist derzeit nur unter Windows verfuegbar.".into())
}
#[cfg(not(target_os = "windows"))]
fn dpapi_entschuetzen(_daten: &[u8]) -> Result<Vec<u8>, String> {
    Err("\"Auf diesem Geraet merken\" ist derzeit nur unter Windows verfuegbar.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hilfsfunktion: Dateiinhalt wie tresor_erstellen ihn schreibt.
    fn tresor_inhalt(passwort: &str, daten: &str) -> Vec<u8> {
        let mut salt = [0u8; SALT_LAENGE];
        OsRng.fill_bytes(&mut salt);
        let schluessel = schluessel_ableiten(passwort, &salt).unwrap();
        verschluesseln(daten, &schluessel, &salt).unwrap()
    }

    /// Entschluesselt einen Dateiinhalt mit einem Passwort (wie tresor_entsperren).
    fn oeffnen(inhalt: &[u8], passwort: &str) -> Option<String> {
        let salt = &inhalt[9..9 + SALT_LAENGE];
        let nonce = &inhalt[9 + SALT_LAENGE..KOPF_LAENGE];
        let schluessel = schluessel_ableiten(passwort, salt).ok()?;
        let klar = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&schluessel))
            .decrypt(Nonce::from_slice(nonce), &inhalt[KOPF_LAENGE..])
            .ok()?;
        String::from_utf8(klar).ok()
    }

    #[test]
    fn passwort_aendern_neues_gilt_altes_nicht_mehr() {
        let alt = tresor_inhalt("altes-passwort", "{\"stand\":1}");
        let (neu, _schluessel, salt) =
            mit_neuem_passwort(&alt, Nachweis::Passwort("altes-passwort"), "neues-passwort", "{\"stand\":2}")
                .unwrap();
        assert_eq!(oeffnen(&neu, "neues-passwort").as_deref(), Some("{\"stand\":2}"));
        assert!(oeffnen(&neu, "altes-passwort").is_none(), "altes Passwort darf nicht mehr gehen");
        assert_ne!(&alt[9..9 + SALT_LAENGE], &salt[..], "neues Salt");
        assert_eq!(&neu[9..9 + SALT_LAENGE], &salt[..]);
    }

    #[test]
    fn passwort_aendern_mit_falschem_alten_passwort_scheitert() {
        let alt = tresor_inhalt("altes-passwort", "{}");
        let fehler =
            mit_neuem_passwort(&alt, Nachweis::Passwort("falsch-geraten"), "neues-passwort", "{}").unwrap_err();
        assert_eq!(fehler, "falsches_passwort");
    }

    #[test]
    fn passwort_aendern_verlangt_mindestlaenge() {
        let alt = tresor_inhalt("altes-passwort", "{}");
        assert!(mit_neuem_passwort(&alt, Nachweis::Passwort("altes-passwort"), "kurz", "{}").is_err());
    }

    #[test]
    fn passwort_aendern_weist_fremde_datei_ab() {
        let fremd = b"keine tresor-datei, viel zu kurz";
        assert!(mit_neuem_passwort(fremd, Nachweis::Passwort("a"), "neues-passwort", "{}").is_err());
    }

    #[test]
    fn passwort_neu_setzen_mit_sitzungsschluessel() {
        // Wie nach "Auf diesem Geraet merken": Schluessel bekannt, Passwort
        // vergessen. Der richtige Schluessel erlaubt den Wechsel, ein
        // falscher nicht.
        let mut salt = [0u8; SALT_LAENGE];
        OsRng.fill_bytes(&mut salt);
        let schluessel = schluessel_ableiten("vergessenes-passwort", &salt).unwrap();
        let alt = verschluesseln("{\"stand\":1}", &schluessel, &salt).unwrap();

        let (neu, _, _) =
            mit_neuem_passwort(&alt, Nachweis::Schluessel(&schluessel), "neues-passwort", "{\"stand\":1}")
                .unwrap();
        assert_eq!(oeffnen(&neu, "neues-passwort").as_deref(), Some("{\"stand\":1}"));
        assert!(oeffnen(&neu, "vergessenes-passwort").is_none());

        let falsch = [7u8; 32];
        let fehler =
            mit_neuem_passwort(&alt, Nachweis::Schluessel(&falsch), "neues-passwort", "{}").unwrap_err();
        assert_eq!(fehler, "falsches_passwort");
    }
}
