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

//
// ------------------------------------------------------------
// Organisationen (Variante B+, Etappe O1)
// ------------------------------------------------------------
//
// Jede Organisation ist ein EIGENER Tresor mit eigener Datei:
//
//   %APPDATA%\com.antrag3000.app\organisationen\<id>.enc      Tresor
//   %APPDATA%\com.antrag3000.app\organisationen\<id>.enc.bak  Vorversion
//   %APPDATA%\com.antrag3000.app\organisationen\<id>.merken   "Merken"
//
// <id> sind 16 zufaellige Hex-Zeichen. Der Dateiname verraet also nichts
// ueber die Organisation; ihr Name steht verschluesselt im Tresor.
//
// Warum getrennte Dateien statt einer: So kann jede Organisation ein
// eigenes Passwort haben, und sie laesst sich spaeter als Ganzes sichern
// oder weitergeben. Teilen sich mehrere Organisationen ein Passwort,
// oeffnet das Entsperren einfach alle Tresore, die sich damit
// entschluesseln lassen. Eine unverschluesselte Liste "wer gehoert zu wem"
// gibt es bewusst nicht.
//
// Umzug: Der bisherige einzelne Tresor (tresor.enc, merken.bin) wird beim
// ersten Zugriff zur Organisation ERSTE_ORG verschoben (siehe umziehen()).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::Serialize;
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
/// Je OFFENER Organisation ein Schluessel (Schluessel = Organisations-ID).
#[derive(Default)]
pub struct TresorZustand {
    pub(crate) geheim: Mutex<HashMap<String, Geheimnis>>,
}

/// Was das Frontend nach dem Entsperren je geoeffneter Organisation bekommt.
#[derive(Serialize)]
pub struct OffeneOrg {
    id: String,
    daten: String,
}

/// ID, unter der der bisherige einzelne Tresor weiterlebt. Fest statt
/// zufaellig, damit ein abgebrochener Umzug beim naechsten Start genau dort
/// weitermacht (jeder Schritt ist einzeln wiederholbar).
pub(crate) const ERSTE_ORG: &str = "0000000000000000";
const ORDNER_ORGS: &str = "organisationen";
const FREMDE_ORG: &str = "Unbekannte Organisation.";

/// Nur genau 16 Kleinbuchstaben-Hexzeichen. Die ID kommt (auch) aus dem
/// Frontend und wird Teil eines Dateinamens – mehr als das darf nicht
/// durchgehen (kein "..", kein "\", kein Laufwerk).
fn id_gueltig(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Ordner fuer Anwendungsdaten (%APPDATA%\com.antrag3000.app).
fn datenordner(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let ordner = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Datenordner nicht ermittelbar: {e}"))?;
    fs::create_dir_all(&ordner).map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
    Ok(ordner)
}

/// Verschiebt den bisherigen einzelnen Tresor in den Organisations-Ordner.
/// Jeder Schritt passiert nur, wenn die Quelle da und das Ziel frei ist:
/// Bricht der Umzug mittendrin ab, setzt ihn der naechste Aufruf fort, und
/// vorhandene Organisationen werden nie ueberschrieben. Verschieben auf
/// demselben Laufwerk ist ein einziger Schritt des Betriebssystems – die
/// Datei ist also immer ganz am alten oder ganz am neuen Ort.
fn umziehen(basis: &Path) -> Result<(), String> {
    let orgs = basis.join(ORDNER_ORGS);
    fs::create_dir_all(&orgs).map_err(|e| format!("Datenordner nicht anlegbar: {e}"))?;
    for (alt, endung) in [("tresor.enc", "enc"), ("tresor.enc.bak", "enc.bak"), ("merken.bin", "merken")] {
        let von = basis.join(alt);
        let nach = orgs.join(format!("{ERSTE_ORG}.{endung}"));
        if von.exists() && !nach.exists() {
            fs::rename(&von, &nach).map_err(|e| format!("Tresor-Umzug fehlgeschlagen: {e}"))?;
        }
    }
    Ok(())
}

/// Ordner aller Organisationen; erledigt vorher den Umzug (falls noetig).
fn org_ordner(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let basis = datenordner(app)?;
    umziehen(&basis)?;
    Ok(basis.join(ORDNER_ORGS))
}

/// Alle vorhandenen Organisationen (IDs), sortiert. Andere Dateien im Ordner
/// (Vorversionen, beiseitegelegte Tresore) zaehlen nicht.
fn org_ids(orgs: &Path) -> Vec<String> {
    let mut ids: Vec<String> = fs::read_dir(orgs)
        .map(|eintraege| {
            eintraege
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter_map(|n| n.strip_suffix(".enc").map(str::to_string))
                .filter(|id| id_gueltig(id))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

/// Pfad der Tresor-Datei einer Organisation (ID wird geprueft).
pub(crate) fn tresor_pfad(app: &tauri::AppHandle, org: &str) -> Result<PathBuf, String> {
    if !id_gueltig(org) {
        return Err(FREMDE_ORG.into());
    }
    Ok(org_ordner(app)?.join(format!("{org}.enc")))
}

/// Pfad fuer eine NEUE Organisation: frische Zufalls-ID, die es noch nicht gibt.
pub(crate) fn neuer_tresor_pfad(app: &tauri::AppHandle) -> Result<(String, PathBuf), String> {
    let orgs = org_ordner(app)?;
    loop {
        let mut zufall = [0u8; 8];
        OsRng.fill_bytes(&mut zufall);
        let id: String = zufall.iter().map(|b| format!("{b:02x}")).collect();
        let pfad = orgs.join(format!("{id}.enc"));
        if id != ERSTE_ORG && !pfad.exists() {
            return Ok((id, pfad));
        }
    }
}

/// Prueft Kopf und Siegel eines Tresor-Inhalts und liefert den Klartext.
/// Fehlercodes: "falsches_passwort" (Siegel bricht) oder ein Text zur
/// beschaedigten Datei.
fn inhalt_oeffnen(inhalt: &[u8], schluessel: &[u8; 32]) -> Result<String, String> {
    if inhalt.len() < KOPF_LAENGE || &inhalt[..8] != MAGIC || inhalt[8] != VERSION {
        return Err(BESCHAEDIGT.into());
    }
    let klartext = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(schluessel))
        .decrypt(Nonce::from_slice(&inhalt[9 + SALT_LAENGE..KOPF_LAENGE]), &inhalt[KOPF_LAENGE..])
        .map_err(|_| "falsches_passwort".to_string())?;
    String::from_utf8(klartext).map_err(|_| "Tresor-Inhalt ist kein gueltiger Text.".to_string())
}

const BESCHAEDIGT: &str = "Die Tresor-Datei ist beschaedigt oder kein Antrag-3000-Tresor.";

/// Salt aus dem Kopf einer Tresor-Datei (oder Fehler bei fremder Datei).
fn salt_lesen(inhalt: &[u8]) -> Result<[u8; SALT_LAENGE], String> {
    if inhalt.len() < KOPF_LAENGE || &inhalt[..8] != MAGIC || inhalt[8] != VERSION {
        return Err(BESCHAEDIGT.into());
    }
    let mut salt = [0u8; SALT_LAENGE];
    salt.copy_from_slice(&inhalt[9..9 + SALT_LAENGE]);
    Ok(salt)
}

/// Kern des Entsperrens (ohne Tauri, damit testbar): Versucht das Passwort an
/// JEDER Organisation im Ordner und liefert alle, die sich damit oeffnen.
/// Jede Datei hat ihr eigenes Salt, darum wird je Datei neu abgeleitet.
/// Oeffnet keine: "falsches_passwort" – oder der Beschaedigungs-Hinweis, wenn
/// eine Datei kaputt war (sonst saehe ein Defekt wie ein Tippfehler aus).
fn alle_oeffnen(orgs: &Path, passwort: &str) -> Result<Vec<(String, String, Geheimnis)>, String> {
    let mut geoeffnet = Vec::new();
    let mut beschaedigt = false;
    for id in org_ids(orgs) {
        let Ok(inhalt) = fs::read(orgs.join(format!("{id}.enc"))) else {
            beschaedigt = true;
            continue;
        };
        let Ok(salt) = salt_lesen(&inhalt) else {
            beschaedigt = true;
            continue;
        };
        let mut schluessel = schluessel_ableiten(passwort, &salt)?;
        match inhalt_oeffnen(&inhalt, &schluessel) {
            Ok(daten) => geoeffnet.push((id, daten, Geheimnis { schluessel, salt, ueber_merken: false })),
            Err(e) if e != "falsches_passwort" => beschaedigt = true,
            Err(_) => {}
        }
        schluessel.zeroize(); // lokale Kopie (Audit A1)
    }
    if geoeffnet.is_empty() {
        return Err(if beschaedigt { BESCHAEDIGT.into() } else { "falsches_passwort".into() });
    }
    Ok(geoeffnet)
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

/// Gibt es schon einen Tresor (mindestens eine Organisation)?
/// -> "fehlt" oder "vorhanden". Erledigt nebenbei den Umzug des alten Tresors.
#[tauri::command]
pub fn tresor_status(app: tauri::AppHandle) -> Result<String, String> {
    let orgs = org_ordner(&app)?;
    Ok(if org_ids(&orgs).is_empty() { "fehlt".into() } else { "vorhanden".into() })
}

/// Neue Organisation mit Passwort und Anfangsdaten anlegen (beim ersten
/// Start die erste). Liefert die ID der neuen Organisation. Eine vorhandene
/// Datei wird dabei nie ueberschrieben: Jede neue bekommt eine frische ID.
#[tauri::command]
pub fn tresor_erstellen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    mut passwort: String,
    daten: String,
) -> Result<String, String> {
    if passwort.chars().count() < 8 {
        passwort.zeroize();
        return Err("Das Passwort muss mindestens 8 Zeichen haben.".into());
    }
    let (id, pfad) = neuer_tresor_pfad(&app)?;

    // Salt einmalig pro Tresor wuerfeln, Schluessel ableiten.
    let mut salt = [0u8; SALT_LAENGE];
    OsRng.fill_bytes(&mut salt);
    let abgeleitet = schluessel_ableiten(&passwort, &salt);
    passwort.zeroize(); // Passwort wird nicht mehr gebraucht (Audit A1).
    let mut schluessel = abgeleitet?;

    let ergebnis = verschluesseln(&daten, &schluessel, &salt).and_then(|inhalt| sicher_schreiben(&pfad, &inhalt));
    if let Err(e) = ergebnis {
        schluessel.zeroize();
        return Err(e);
    }

    // Schluessel im Arbeitsspeicher behalten, damit Speichern ohne
    // erneute Passworteingabe moeglich ist.
    state.geheim.lock().unwrap().insert(id.clone(), Geheimnis { schluessel, salt, ueber_merken: false });
    schluessel.zeroize(); // lokale Kopie (Audit A1)
    Ok(id)
}

/// Mit Passwort entsperren: oeffnet ALLE Organisationen, die dieses Passwort
/// haben, und liefert je eine {id, daten}. Schon offene bleiben offen – so
/// kann man spaeter eine Organisation mit eigenem Passwort dazunehmen.
#[tauri::command]
pub fn tresor_entsperren(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    mut passwort: String,
) -> Result<Vec<OffeneOrg>, String> {
    let ergebnis = org_ordner(&app).and_then(|orgs| alle_oeffnen(&orgs, &passwort));
    passwort.zeroize(); // Passwort wird nicht mehr gebraucht (Audit A1).
    let geoeffnet = ergebnis?;

    let mut geheim = state.geheim.lock().unwrap();
    let mut antwort = Vec::with_capacity(geoeffnet.len());
    for (id, daten, g) in geoeffnet {
        geheim.insert(id.clone(), g);
        antwort.push(OffeneOrg { id, daten });
    }
    Ok(antwort)
}

/// Aktuellen Datenstand einer offenen Organisation verschluesselt speichern.
#[tauri::command]
pub fn tresor_speichern(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    daten: String,
) -> Result<(), String> {
    let pfad = tresor_pfad(&app, &org)?;
    let geheim = state.geheim.lock().unwrap();
    let geheim = geheim
        .get(&org)
        .ok_or("Der Tresor ist nicht entsperrt.".to_string())?;
    let inhalt = verschluesseln(&daten, &geheim.schluessel, &geheim.salt)?;
    sicher_schreiben(&pfad, &inhalt)
}

/// Sperren: alle Schluessel aus dem Arbeitsspeicher entfernen (Drop
/// ueberschreibt sie mit Nullen).
#[tauri::command]
pub fn tresor_sperren(state: tauri::State<TresorZustand>) {
    state.geheim.lock().unwrap().clear();
}

/// Fuer die Sicherung mit Dateien: Schluessel und Salt einer offenen
/// Organisation samt der dazu passenden Tresor-Datei. Das Schloss haelt
/// gleichzeitiges Speichern fern, und gesichert wird nur ein Tresor, der sich
/// auch oeffnen laesst.
pub(crate) fn offener_stand(
    app: &tauri::AppHandle,
    org: &str,
) -> Result<(Zeroizing<[u8; 32]>, [u8; SALT_LAENGE], Vec<u8>), String> {
    let pfad = tresor_pfad(app, org)?;
    let zustand = app.state::<TresorZustand>();
    let geheim = zustand.geheim.lock().unwrap();
    let geheim = geheim
        .get(org)
        .ok_or("Der Tresor ist nicht entsperrt.".to_string())?;

    let unpassend = "Die Tresor-Datei passt nicht zur offenen Sitzung. Bitte sperren und neu entsperren.";
    let inhalt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
    if salt_lesen(&inhalt).ok() != Some(geheim.salt) {
        return Err(unpassend.into());
    }
    match inhalt_oeffnen(&inhalt, &geheim.schluessel) {
        Ok(mut klartext) => klartext.zeroize(),
        Err(_) => return Err(unpassend.into()),
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

/// Passwort einer Organisation aendern (sie muss offen sein) – mit dem
/// bisherigen Passwort als Nachweis. `daten` ist ihr aktueller Datenstand (wie
/// bei tresor_speichern). Rueckgabe siehe passwort_abschliessen.
#[tauri::command]
pub fn tresor_passwort_aendern(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    mut altes_passwort: String,
    mut neues_passwort: String,
    daten: String,
) -> Result<bool, String> {
    let mut geheim = state.geheim.lock().unwrap();
    let ergebnis = (|| {
        if !geheim.contains_key(&org) {
            return Err("Der Tresor ist nicht entsperrt.".to_string());
        }
        let pfad = tresor_pfad(&app, &org)?;
        let inhalt_alt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
        let neu = mit_neuem_passwort(&inhalt_alt, Nachweis::Passwort(&altes_passwort), &neues_passwort, &daten)?;
        Ok((pfad, neu))
    })();
    altes_passwort.zeroize();
    neues_passwort.zeroize();
    let (pfad, (inhalt, schluessel, salt)) = ergebnis?;
    passwort_abschliessen(&app, &mut geheim, &org, &pfad, inhalt, schluessel, salt)
}

/// Wurde diese Organisation ueber "Auf diesem Geraet merken" geoeffnet?
/// (Nur fuer die Anzeige – erzwungen wird es in tresor_passwort_neu_setzen.)
#[tauri::command]
pub fn tresor_ueber_merken(state: tauri::State<TresorZustand>, org: String) -> bool {
    state.geheim.lock().unwrap().get(&org).map(|g| g.ueber_merken).unwrap_or(false)
}

/// Passwort vergessen, aber per "Auf diesem Geraet merken" entsperrt: neues
/// Passwort festlegen OHNE das alte. Nur erlaubt, wenn diese Organisation
/// tatsaechlich ueber das Merken geoeffnet wurde. Begruendung: Wer an diesem
/// Windows-Konto sitzt, kommt ueber das Merken ohnehin an alle Daten (die
/// bewusste Abwaegung von "Auf diesem Geraet merken") – die Abfrage des alten
/// Passworts schuetzt in genau dieser Lage nicht zusaetzlich. Als Nachweis
/// dient der Schluessel der offenen Sitzung; er muss die Tresor-Datei oeffnen.
#[tauri::command]
pub fn tresor_passwort_neu_setzen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    mut neues_passwort: String,
    daten: String,
) -> Result<bool, String> {
    let mut geheim = state.geheim.lock().unwrap();
    let mut sitzungs_schluessel = match geheim.get(&org) {
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
    let ergebnis = tresor_pfad(&app, &org).and_then(|pfad| {
        let alt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
        let neu = mit_neuem_passwort(&alt, Nachweis::Schluessel(&sitzungs_schluessel), &neues_passwort, &daten)?;
        Ok((pfad, neu))
    });
    sitzungs_schluessel.zeroize();
    neues_passwort.zeroize();
    let (pfad, (inhalt, schluessel, salt)) = ergebnis?;
    passwort_abschliessen(&app, &mut geheim, &org, &pfad, inhalt, schluessel, salt)
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
    geheim: &mut HashMap<String, Geheimnis>,
    org: &str,
    pfad: &PathBuf,
    inhalt: Vec<u8>,
    mut schluessel: [u8; 32],
    salt: [u8; SALT_LAENGE],
) -> Result<bool, String> {
    if let Err(e) = sicher_schreiben(pfad, &inhalt) {
        schluessel.zeroize();
        return Err(e);
    }
    // Kontrolle: die geschriebene Datei muss sich mit dem neuen Schluessel oeffnen.
    let kontrolle = fs::read(pfad)
        .map_err(|e| format!("Kontrolle nicht lesbar: {e}"))
        .and_then(|geschrieben| inhalt_oeffnen(&geschrieben, &schluessel));
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
        if let Err(e) = fs::remove_file(&bak) {
            schluessel.zeroize();
            return Err(format!("Alte Vorversion nicht loeschbar: {e}"));
        }
    }

    // "Merken" erneuern, falls aktiv – sonst passte es nicht mehr zum Tresor.
    let mut merken_ok = true;
    if let Ok(m_pfad) = merken_pfad(app, org) {
        if m_pfad.exists() {
            let erneuert = dpapi_schuetzen(&schluessel)
                .and_then(|geschuetzt| fs::write(&m_pfad, geschuetzt).map_err(|e| e.to_string()));
            if erneuert.is_err() {
                let _ = fs::remove_file(&m_pfad);
                merken_ok = false;
            }
        }
    }

    // Der "vergessen"-Weg bleibt offen, solange das Merken noch gilt.
    let ueber_merken = geheim.get(org).map(|g| g.ueber_merken).unwrap_or(false) && merken_ok;
    geheim.insert(org.to_string(), Geheimnis { schluessel, salt, ueber_merken });
    schluessel.zeroize(); // lokale Kopie (Audit A1)
    Ok(merken_ok)
}

/// "Neu aufsetzen" bei Passwortverlust: ALLE Tresore werden NICHT geloescht,
/// sondern mit Datum beiseitegelegt (falls das Passwort doch wieder einfaellt):
/// organisationen\alt-2026-06-12-<id>.enc. Danach kann ein frischer Tresor
/// angelegt werden. (Das gezielte Neu-Aufsetzen EINER Organisation mit eigenem
/// Passwort folgt in Etappe O4.)
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
    let orgs = org_ordner(&app)?;
    let ids = org_ids(&orgs);
    if ids.is_empty() {
        return Err("Es gibt keinen Tresor zum Neu-Aufsetzen.".into());
    }
    let mut geheim = state.geheim.lock().unwrap();
    for id in ids {
        beiseitelegen(&orgs, &id, &datum)?;
    }
    geheim.clear();
    Ok(())
}

/// Legt Tresor (+ Vorversion) einer Organisation unter einem freien
/// "alt-…"-Namen ab und entfernt ihr "Merken" (es passte nicht mehr).
fn beiseitelegen(orgs: &Path, id: &str, datum: &str) -> Result<(), String> {
    let pfad = orgs.join(format!("{id}.enc"));
    // Freien Namen finden: alt-2026-06-12-<id>.enc, -2, -3, ...
    let mut ziel = orgs.join(format!("alt-{datum}-{id}.enc"));
    let mut nr = 2;
    while ziel.exists() {
        ziel = orgs.join(format!("alt-{datum}-{id}-{nr}.enc"));
        nr += 1;
    }
    fs::rename(&pfad, &ziel).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))?;
    let bak = pfad.with_extension("enc.bak");
    if bak.exists() {
        let _ = fs::rename(&bak, ziel.with_extension("enc.bak"));
    }
    let _ = fs::remove_file(orgs.join(format!("{id}.merken")));
    Ok(())
}

// ============================================================
// "Auf diesem Geraet merken" (Windows DPAPI)
// ============================================================
//
// Idee: Der Tresor bleibt unveraendert mit dem aus dem Passwort
// abgeleiteten Schluessel AES-verschluesselt. Zusaetzlich legen wir den
// 32-Byte-SCHLUESSEL je Organisation in einer Datei <id>.merken ab -- aber
// NICHT im Klartext, sondern mit Windows DPAPI an das angemeldete
// Windows-Konto gebunden (CryptProtectData). Nur derselbe Windows-Benutzer
// auf demselben Geraet kann ihn wieder entschluesseln. So startet die App
// ohne Passwort; das Passwort bleibt der Rueckfall, und "Geraet vergessen"
// loescht die Dateien wieder.
//
// Sicherheits-Abwaegung (vom Nutzer fuer die Pilotphase so gewaehlt): Wer
// am entsperrten Windows-Konto sitzt, kann die App oeffnen. Die Tresor-
// Datei selbst bleibt fuer fremde Konten/Geraete unbrauchbar.

fn merken_pfad(app: &tauri::AppHandle, org: &str) -> Result<PathBuf, String> {
    if !id_gueltig(org) {
        return Err(FREMDE_ORG.into());
    }
    Ok(org_ordner(app)?.join(format!("{org}.merken")))
}

/// Ist "Auf diesem Geraet merken" auf dieser Plattform ueberhaupt moeglich?
/// Nur Windows hat DPAPI (passwortloses Entsperren, an das Windows-Konto
/// gebunden). Auf macOS/Linux faellt es (vorerst) weg -- das Frontend
/// blendet die Option dann aus, damit niemand auf einen Fehler laeuft.
#[tauri::command]
pub fn merken_moeglich() -> bool {
    cfg!(target_os = "windows")
}

/// Ist auf diesem Geraet fuer mindestens eine Organisation ein
/// passwortloses Entsperren hinterlegt?
#[tauri::command]
pub fn merken_status(app: tauri::AppHandle) -> Result<bool, String> {
    let orgs = org_ordner(&app)?;
    Ok(org_ids(&orgs).iter().any(|id| orgs.join(format!("{id}.merken")).exists()))
}

/// Alle gerade offenen Organisationen fuer dieses Geraet merken.
#[tauri::command]
pub fn merken_anlegen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
) -> Result<(), String> {
    let geheim = state.geheim.lock().unwrap();
    if geheim.is_empty() {
        return Err("Der Tresor ist nicht entsperrt.".into());
    }
    for (id, g) in geheim.iter() {
        let geschuetzt = dpapi_schuetzen(&g.schluessel)?;
        fs::write(merken_pfad(&app, id)?, geschuetzt)
            .map_err(|e| format!("Konnte das Merken nicht speichern: {e}"))?;
    }
    Ok(())
}

/// Ohne Passwort entsperren: jede gemerkte Organisation per DPAPI-Schluessel
/// oeffnen. Liefert dieselbe Liste wie tresor_entsperren. Ein gemerkter
/// Zugang, der nicht (mehr) passt, wird verworfen.
#[tauri::command]
pub fn merken_entsperren(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
) -> Result<Vec<OffeneOrg>, String> {
    let orgs = org_ordner(&app)?;
    let mut geheim = state.geheim.lock().unwrap();
    let mut antwort = Vec::new();
    for id in org_ids(&orgs) {
        let m_pfad = orgs.join(format!("{id}.merken"));
        let Ok(geschuetzt) = fs::read(&m_pfad) else { continue };
        let Some(mut schluessel) = gemerkter_schluessel(&geschuetzt) else {
            // Nicht entschluesselbar (anderes Konto/Geraet) -> verwerfen.
            let _ = fs::remove_file(&m_pfad);
            continue;
        };
        let geoeffnet = fs::read(orgs.join(format!("{id}.enc")))
            .map_err(|_| BESCHAEDIGT.to_string())
            .and_then(|inhalt| Ok((salt_lesen(&inhalt)?, inhalt_oeffnen(&inhalt, &schluessel)?)));
        match geoeffnet {
            Ok((salt, daten)) => {
                geheim.insert(id.clone(), Geheimnis { schluessel, salt, ueber_merken: true });
                antwort.push(OffeneOrg { id, daten });
            }
            // Schluessel passt nicht mehr zum Tresor (z. B. Passwort woanders
            // geaendert) -> verwerfen, kuenftig wieder per Passwort.
            Err(_) => {
                let _ = fs::remove_file(&m_pfad);
            }
        }
        schluessel.zeroize(); // lokale Kopie (Audit A1)
    }
    if antwort.is_empty() {
        return Err("merken_ungueltig".into());
    }
    Ok(antwort)
}

/// DPAPI-geschuetzten Schluessel auspacken; None, wenn das nicht geht oder
/// die Laenge nicht stimmt.
fn gemerkter_schluessel(geschuetzt: &[u8]) -> Option<[u8; 32]> {
    let mut roh = dpapi_entschuetzen(geschuetzt).ok()?;
    let ergebnis = (roh.len() == 32).then(|| {
        let mut s = [0u8; 32];
        s.copy_from_slice(&roh);
        s
    });
    roh.zeroize();
    ergebnis
}

/// Alle gemerkten Zugaenge dieses Geraets entfernen (kuenftig wieder Passwort).
#[tauri::command]
pub fn merken_vergessen(app: tauri::AppHandle) -> Result<(), String> {
    let orgs = org_ordner(&app)?;
    for id in org_ids(&orgs) {
        let pfad = orgs.join(format!("{id}.merken"));
        if pfad.exists() {
            fs::remove_file(&pfad).map_err(|e| format!("Konnte nicht entfernen: {e}"))?;
        }
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

    // ---------- Organisationen (Etappe O1) ----------

    /// Eigener Ordner je Test, wird am Ende entfernt.
    struct Ordner(PathBuf);
    impl Ordner {
        fn neu() -> Self {
            let mut zufall = [0u8; 8];
            OsRng.fill_bytes(&mut zufall);
            let pfad = std::env::temp_dir().join(format!("a3000-org-test-{:x}", u64::from_le_bytes(zufall)));
            fs::create_dir_all(&pfad).unwrap();
            Ordner(pfad)
        }
    }
    impl Drop for Ordner {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn id_pruefung_laesst_nur_16_hexzeichen_durch() {
        assert!(id_gueltig("0123456789abcdef"));
        assert!(id_gueltig(ERSTE_ORG));
        for schlecht in ["", "0123456789ABCDEF", "0123456789abcde", "0123456789abcdef0", "..\\..\\tresor.en", "0123456789abcdeg"] {
            assert!(!id_gueltig(schlecht), "{schlecht} darf nicht gelten");
        }
    }

    #[test]
    fn umzug_verschiebt_alten_tresor_und_ist_wiederholbar() {
        let o = Ordner::neu();
        fs::write(o.0.join("tresor.enc"), tresor_inhalt("passwort-1", "{\"alt\":1}")).unwrap();
        fs::write(o.0.join("merken.bin"), b"gemerkt").unwrap();

        umziehen(&o.0).unwrap();
        let orgs = o.0.join(ORDNER_ORGS);
        assert!(!o.0.join("tresor.enc").exists());
        assert!(!o.0.join("merken.bin").exists());
        assert_eq!(org_ids(&orgs), vec![ERSTE_ORG.to_string()]);
        assert_eq!(fs::read(orgs.join(format!("{ERSTE_ORG}.merken"))).unwrap(), b"gemerkt");

        // Ein zweiter Lauf aendert nichts.
        umziehen(&o.0).unwrap();
        assert_eq!(org_ids(&orgs), vec![ERSTE_ORG.to_string()]);
        let offen = alle_oeffnen(&orgs, "passwort-1").unwrap();
        assert_eq!(offen[0].1, "{\"alt\":1}");
    }

    #[test]
    fn umzug_ueberschreibt_nie_eine_vorhandene_organisation() {
        let o = Ordner::neu();
        let orgs = o.0.join(ORDNER_ORGS);
        fs::create_dir_all(&orgs).unwrap();
        fs::write(orgs.join(format!("{ERSTE_ORG}.enc")), tresor_inhalt("passwort-1", "{\"neu\":1}")).unwrap();
        fs::write(o.0.join("tresor.enc"), tresor_inhalt("passwort-1", "{\"alt\":1}")).unwrap();

        umziehen(&o.0).unwrap();
        assert!(o.0.join("tresor.enc").exists(), "alter Tresor bleibt unangetastet liegen");
        assert_eq!(alle_oeffnen(&orgs, "passwort-1").unwrap()[0].1, "{\"neu\":1}");
    }

    #[test]
    fn ein_passwort_oeffnet_alle_die_es_teilen() {
        let o = Ordner::neu();
        let orgs = &o.0;
        fs::write(orgs.join("aaaaaaaaaaaaaaaa.enc"), tresor_inhalt("gemeinsam-123", "{\"org\":\"a\"}")).unwrap();
        fs::write(orgs.join("bbbbbbbbbbbbbbbb.enc"), tresor_inhalt("eigenes-456", "{\"org\":\"b\"}")).unwrap();
        fs::write(orgs.join("cccccccccccccccc.enc"), tresor_inhalt("gemeinsam-123", "{\"org\":\"c\"}")).unwrap();
        // Beiseitegelegte und fremde Dateien zaehlen nicht.
        fs::write(orgs.join("alt-2026-06-12-dddddddddddddddd.enc"), tresor_inhalt("gemeinsam-123", "{}")).unwrap();
        fs::write(orgs.join("notizen.txt"), b"x").unwrap();

        let offen = alle_oeffnen(orgs, "gemeinsam-123").unwrap();
        let ids: Vec<&str> = offen.iter().map(|(id, _, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["aaaaaaaaaaaaaaaa", "cccccccccccccccc"]);
        // Jede Datei hat ihr eigenes Salt – also auch ihren eigenen Schluessel.
        assert_ne!(offen[0].2.salt, offen[1].2.salt);
        assert_ne!(offen[0].2.schluessel, offen[1].2.schluessel);

        let eigen = alle_oeffnen(orgs, "eigenes-456").unwrap();
        assert_eq!(eigen.len(), 1);
        assert_eq!(eigen[0].1, "{\"org\":\"b\"}");

        assert_eq!(alle_oeffnen(orgs, "falsch-geraten").err().as_deref(), Some("falsches_passwort"));
    }

    #[test]
    fn beschaedigte_datei_wird_nicht_als_falsches_passwort_gemeldet() {
        let o = Ordner::neu();
        fs::write(o.0.join("aaaaaaaaaaaaaaaa.enc"), b"kaputt").unwrap();
        assert_eq!(alle_oeffnen(&o.0, "irgendein-pw").err().as_deref(), Some(BESCHAEDIGT));
        // Oeffnet aber eine andere, zaehlt nur die.
        fs::write(o.0.join("bbbbbbbbbbbbbbbb.enc"), tresor_inhalt("irgendein-pw", "{}")).unwrap();
        assert_eq!(alle_oeffnen(&o.0, "irgendein-pw").unwrap().len(), 1);
    }

    #[test]
    fn beiseitelegen_versteckt_tresor_und_entfernt_merken() {
        let o = Ordner::neu();
        let id = "aaaaaaaaaaaaaaaa";
        fs::write(o.0.join(format!("{id}.enc")), tresor_inhalt("passwort-1", "{}")).unwrap();
        fs::write(o.0.join(format!("{id}.enc.bak")), b"vorversion").unwrap();
        fs::write(o.0.join(format!("{id}.merken")), b"gemerkt").unwrap();

        beiseitelegen(&o.0, id, "2026-06-12").unwrap();
        assert!(org_ids(&o.0).is_empty());
        assert!(o.0.join(format!("alt-2026-06-12-{id}.enc")).exists());
        assert!(o.0.join(format!("alt-2026-06-12-{id}.enc.bak")).exists());
        assert!(!o.0.join(format!("{id}.merken")).exists());

        // Gleicher Tag noch einmal: freier Name, nichts wird ueberschrieben.
        fs::write(o.0.join(format!("{id}.enc")), tresor_inhalt("passwort-2", "{}")).unwrap();
        beiseitelegen(&o.0, id, "2026-06-12").unwrap();
        assert!(o.0.join(format!("alt-2026-06-12-{id}-2.enc")).exists());
    }
}
