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
pub(crate) fn id_gueltig(id: &str) -> bool {
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
pub(crate) fn org_ordner(app: &tauri::AppHandle) -> Result<PathBuf, String> {
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
    Ok(neue_id_in(&org_ordner(app)?))
}

/// Frische Zufalls-ID im Ordner `orgs` (ohne Tauri, damit testbar).
pub(crate) fn neue_id_in(orgs: &Path) -> (String, PathBuf) {
    loop {
        let mut zufall = [0u8; 8];
        OsRng.fill_bytes(&mut zufall);
        let id: String = zufall.iter().map(|b| format!("{b:02x}")).collect();
        let pfad = orgs.join(format!("{id}.enc"));
        if id != ERSTE_ORG && !pfad.exists() {
            return (id, pfad);
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
pub(crate) fn verschluesseln(daten: &str, schluessel: &[u8; 32], salt: &[u8; SALT_LAENGE]) -> Result<Vec<u8>, String> {
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

/// IDs aller Organisationen auf diesem Geraet (auch gesperrter). Nur die
/// Zufalls-IDs – Namen kennt Rust nicht, die stehen verschluesselt im Tresor.
#[tauri::command]
pub fn tresor_liste(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    Ok(org_ids(&org_ordner(&app)?))
}

/// Stimmt dieses Passwort fuer diese (offene) Organisation? Dient der
/// Bestaetigung, wenn eine neue Organisation "dasselbe Passwort" bekommen
/// soll – sonst laege bei einem Tippfehler unbemerkt ein anderes Passwort
/// auf ihr. Nur fuer offene Organisationen, damit das kein Weg zum Raten an
/// gesperrten wird.
#[tauri::command]
pub fn tresor_passwort_pruefen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    mut passwort: String,
) -> Result<bool, String> {
    let ergebnis = (|| {
        if !state.geheim.lock().unwrap().contains_key(&org) {
            return Err("Der Tresor ist nicht entsperrt.".to_string());
        }
        let inhalt = fs::read(tresor_pfad(&app, &org)?).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
        let salt = salt_lesen(&inhalt)?;
        let mut schluessel = schluessel_ableiten(&passwort, &salt)?;
        let passt = inhalt_oeffnen(&inhalt, &schluessel).map(|mut k| k.zeroize()).is_ok();
        schluessel.zeroize();
        Ok(passt)
    })();
    passwort.zeroize();
    ergebnis
}

/// Eine (offene) Organisation entfernen. Die Datei wird – wie beim Neu-
/// Aufsetzen – verschluesselt beiseitegelegt (alt-<datum>-<id>.enc), nicht
/// sofort geloescht; ihr "Merken" verschwindet. Die letzte Organisation
/// laesst sich so nicht entfernen (dafuer gibt es "Neu aufsetzen").
#[tauri::command]
pub fn tresor_entfernen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    datum: String,
) -> Result<(), String> {
    if datum.is_empty() || datum.len() > 10 || !datum.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err("Ungueltiges Datum.".into());
    }
    if !id_gueltig(&org) {
        return Err(FREMDE_ORG.into());
    }
    let orgs = org_ordner(&app)?;
    let mut geheim = state.geheim.lock().unwrap();
    if !geheim.contains_key(&org) {
        return Err("Die Organisation ist nicht entsperrt.".into());
    }
    let ids = org_ids(&orgs);
    if !ids.iter().any(|i| i == &org) {
        return Err(FREMDE_ORG.into());
    }
    if ids.len() < 2 {
        return Err("Die letzte Organisation kann nicht entfernt werden.".into());
    }
    beiseitelegen(&orgs, &org, &datum)?;
    geheim.remove(&org);
    Ok(())
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

/// Fuer "alle Organisationen sichern": jede Tresor-Datei so, wie sie ist
/// (verschluesselt – auch gesperrte und solche mit eigenem Passwort). Die
/// aktive kommt schon geprueft aus offener_stand. Das Schloss haelt
/// gleichzeitiges Speichern fern.
pub(crate) fn alle_tresore(
    app: &tauri::AppHandle,
    aktiv: &str,
    aktiv_inhalt: Vec<u8>,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let orgs = org_ordner(app)?;
    let zustand = app.state::<TresorZustand>();
    let _geheim = zustand.geheim.lock().unwrap();
    let mut alle = vec![(aktiv.to_string(), aktiv_inhalt)];
    for id in org_ids(&orgs) {
        if id == aktiv {
            continue;
        }
        let inhalt = fs::read(orgs.join(format!("{id}.enc"))).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
        salt_lesen(&inhalt).map_err(|_| "Eine Organisation auf diesem Gerät ist beschädigt und lässt sich nicht sichern.".to_string())?;
        alle.push((id, inhalt));
    }
    Ok(alle)
}

/// Oeffnet einen Tresor-Inhalt mit einem Passwort (eigenes Salt aus dem
/// Kopf). None, wenn das Passwort nicht passt – z. B. eine Organisation mit
/// eigenem Passwort in einer Sicherung aller Organisationen.
pub(crate) fn mit_passwort_oeffnen(inhalt: &[u8], passwort: &str) -> Option<String> {
    let salt = salt_lesen(inhalt).ok()?;
    let mut schluessel = schluessel_ableiten(passwort, &salt).ok()?;
    let daten = inhalt_oeffnen(inhalt, &schluessel).ok();
    schluessel.zeroize();
    daten
}

/// Womit der Zugang vor einem Passwortwechsel nachgewiesen wird.
#[derive(Clone, Copy)]
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

/// Ergebnis eines Passwortwechsels fuer das Frontend.
#[derive(Serialize, Debug)]
pub struct PasswortWechsel {
    /// Alle Organisationen, die jetzt das neue Passwort haben (die aktive zuerst).
    geaendert: Vec<String>,
    /// false, wenn ein "Merken" dabei entfernt werden musste.
    merken_ok: bool,
    /// true, wenn eine Vorversion mit dem alten Passwort nicht geloescht
    /// werden konnte (sollte nie passieren; das Frontend warnt dann).
    vorversion_rest: bool,
}

/// Eine fertig neu verschluesselte Tresor-Datei, noch nicht geschrieben.
struct Neufassung {
    id: String,
    pfad: PathBuf,
    inhalt: Vec<u8>,
    schluessel: Zeroizing<[u8; 32]>,
    salt: [u8; SALT_LAENGE],
}

/// Kern des Passwortwechsels (ohne Tauri, damit testbar).
///
/// Welche Organisationen "dasselbe Passwort haben", steht nirgends – das ist
/// Absicht (keine Liste "wer gehoert zu wem"). Darum gilt: Eine Organisation
/// teilt das Passwort, wenn sich ihre Datei mit dem ALTEN Passwort oeffnen
/// laesst. Das wird je Datei geprueft (jede hat ihr eigenes Salt). So kommen
/// auch Organisationen mit, die gerade gesperrt sind, und eine Organisation
/// mit eigenem Passwort bleibt garantiert unberuehrt.
///
/// Ablauf "alles oder nichts":
///  1. Alles im Arbeitsspeicher vorbereiten (pruefen, neu verschluesseln).
///     Faellt hier etwas um, ist noch nichts geschrieben.
///  2. Alle Dateien schreiben (die Vorversion bleibt als .bak liegen) und mit
///     dem neuen Schluessel kontrollieren. Scheitert eine, kommen alle schon
///     geschriebenen aus ihrer .bak zurueck: Dann gilt ueberall das alte Passwort.
///  3. Erst dann die .bak loeschen – sie waere noch mit dem alten Passwort lesbar.
///
/// `daten` ist der aktuelle Stand der aktiven Organisation (wie beim Speichern);
/// die anderen werden mit dem Inhalt ihrer Datei neu verschluesselt.
fn passwort_wechseln(
    orgs: &Path,
    aktiv: &str,
    nachweis: Nachweis,
    neues_passwort: &str,
    daten: &str,
    geteilte: bool,
    schreiben: impl Fn(&PathBuf, &[u8]) -> Result<(), String>,
) -> Result<(Vec<Neufassung>, bool), String> {
    if !id_gueltig(aktiv) {
        return Err(FREMDE_ORG.into());
    }
    // 1. Vorbereiten
    let pfad = orgs.join(format!("{aktiv}.enc"));
    let alt = fs::read(&pfad).map_err(|e| format!("Tresor nicht lesbar: {e}"))?;
    let (inhalt, schluessel, salt) = mit_neuem_passwort(&alt, nachweis, neues_passwort, daten)?;
    let mut neu = vec![Neufassung { id: aktiv.to_string(), pfad, inhalt, schluessel: Zeroizing::new(schluessel), salt }];

    if let (true, Nachweis::Passwort(altes)) = (geteilte, &nachweis) {
        for id in org_ids(orgs).into_iter().filter(|i| i != aktiv) {
            let pfad = orgs.join(format!("{id}.enc"));
            let Ok(inhalt_alt) = fs::read(&pfad) else { continue };
            let Ok(salt_alt) = salt_lesen(&inhalt_alt) else { continue };
            let schluessel_alt = Zeroizing::new(schluessel_ableiten(altes, &salt_alt)?);
            // Laesst sie sich mit dem alten Passwort nicht oeffnen, hat sie ein
            // anderes (oder ist beschaedigt): dann bleibt sie, wie sie ist.
            let Ok(klartext) = inhalt_oeffnen(&inhalt_alt, &schluessel_alt) else { continue };
            let klartext = Zeroizing::new(klartext);
            let mut salt = [0u8; SALT_LAENGE];
            OsRng.fill_bytes(&mut salt);
            let schluessel = Zeroizing::new(schluessel_ableiten(neues_passwort, &salt)?);
            let inhalt = verschluesseln(&klartext, &schluessel, &salt)?;
            neu.push(Neufassung { id, pfad, inhalt, schluessel, salt });
        }
    }

    // 2. Schreiben und kontrollieren
    let zurueckholen = |geschrieben: &[&Neufassung]| {
        for n in geschrieben {
            let bak = n.pfad.with_extension("enc.bak");
            if bak.exists() {
                let _ = fs::remove_file(&n.pfad);
                let _ = fs::rename(&bak, &n.pfad);
            }
        }
    };
    let mut geschrieben: Vec<&Neufassung> = Vec::new();
    for n in &neu {
        if let Err(e) = schreiben(&n.pfad, &n.inhalt) {
            // Ging beim Schreiben DIESER Datei die Hauptdatei verloren, die
            // gerade beiseitegelegte Fassung zurueckholen. Sonst ist sie heil.
            let bak = n.pfad.with_extension("enc.bak");
            if !n.pfad.exists() && bak.exists() {
                let _ = fs::rename(&bak, &n.pfad);
            }
            zurueckholen(&geschrieben);
            return Err(format!("{e} – das alte Passwort gilt weiter."));
        }
        geschrieben.push(n);
    }
    for n in &neu {
        let passt = fs::read(&n.pfad)
            .map_err(|e| e.to_string())
            .and_then(|g| inhalt_oeffnen(&g, &n.schluessel))
            .map(|mut k| k.zeroize())
            .is_ok();
        if !passt {
            zurueckholen(&geschrieben);
            return Err("Kontrolle fehlgeschlagen – das alte Passwort gilt weiter.".into());
        }
    }

    // 3. Vorversionen mit dem alten Passwort entfernen
    let mut vorversion_rest = false;
    for n in &neu {
        let bak = n.pfad.with_extension("enc.bak");
        if bak.exists() && fs::remove_file(&bak).is_err() {
            vorversion_rest = true;
        }
    }
    Ok((neu, vorversion_rest))
}

/// Gemeinsamer Abschluss nach dem Wechsel: "Merken" erneuern (sonst passte es
/// nicht mehr) und die Schluessel der offenen Organisationen austauschen.
/// Gesperrte, die mitgeaendert wurden, bleiben gesperrt.
fn passwort_abschliessen(
    orgs: &Path,
    geheim: &mut HashMap<String, Geheimnis>,
    neu: Vec<Neufassung>,
    vorversion_rest: bool,
) -> PasswortWechsel {
    let mut merken_ok = true;
    let mut geaendert = Vec::with_capacity(neu.len());
    for n in neu {
        let m_pfad = orgs.join(format!("{}.merken", n.id));
        let mut gemerkt = false;
        if m_pfad.exists() {
            let erneuert = dpapi_schuetzen(&n.schluessel[..])
                .and_then(|geschuetzt| fs::write(&m_pfad, geschuetzt).map_err(|e| e.to_string()));
            if erneuert.is_ok() {
                gemerkt = true;
            } else {
                let _ = fs::remove_file(&m_pfad);
                merken_ok = false;
            }
        }
        if let Some(g) = geheim.get(&n.id) {
            // Der "vergessen"-Weg bleibt offen, solange das Merken noch gilt.
            let ueber_merken = g.ueber_merken && gemerkt;
            geheim.insert(n.id.clone(), Geheimnis { schluessel: *n.schluessel, salt: n.salt, ueber_merken });
        }
        geaendert.push(n.id);
    }
    PasswortWechsel { geaendert, merken_ok, vorversion_rest }
}

/// Passwort einer Organisation aendern (sie muss offen sein) – mit dem
/// bisherigen Passwort als Nachweis. Mit `geteilte` aendert es sich auch bei
/// allen anderen Organisationen, die sich mit dem bisherigen Passwort oeffnen
/// lassen (siehe passwort_wechseln). `daten` ist der aktuelle Stand der aktiven.
#[tauri::command]
pub fn tresor_passwort_aendern(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    mut altes_passwort: String,
    mut neues_passwort: String,
    daten: String,
    geteilte: bool,
) -> Result<PasswortWechsel, String> {
    let mut geheim = state.geheim.lock().unwrap();
    let ergebnis = (|| {
        if !geheim.contains_key(&org) {
            return Err("Der Tresor ist nicht entsperrt.".to_string());
        }
        let orgs = org_ordner(&app)?;
        let (neu, rest) = passwort_wechseln(
            &orgs,
            &org,
            Nachweis::Passwort(&altes_passwort),
            &neues_passwort,
            &daten,
            geteilte,
            |p, i| sicher_schreiben(p, i),
        )?;
        Ok((orgs, neu, rest))
    })();
    altes_passwort.zeroize();
    neues_passwort.zeroize();
    let (orgs, neu, rest) = ergebnis?;
    Ok(passwort_abschliessen(&orgs, &mut geheim, neu, rest))
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
///
/// Gilt NUR fuer diese eine Organisation: Ohne das alte Passwort laesst sich
/// nicht pruefen, welche anderen es geteilt haben.
#[tauri::command]
pub fn tresor_passwort_neu_setzen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    mut neues_passwort: String,
    daten: String,
) -> Result<PasswortWechsel, String> {
    let mut geheim = state.geheim.lock().unwrap();
    let sitzungs_schluessel = match geheim.get(&org) {
        Some(g) if g.ueber_merken => Zeroizing::new(g.schluessel),
        Some(_) => {
            neues_passwort.zeroize();
            return Err("nur_ueber_merken".into());
        }
        None => {
            neues_passwort.zeroize();
            return Err("Der Tresor ist nicht entsperrt.".into());
        }
    };
    let ergebnis = org_ordner(&app).and_then(|orgs| {
        let (neu, rest) = passwort_wechseln(
            &orgs,
            &org,
            Nachweis::Schluessel(&sitzungs_schluessel),
            &neues_passwort,
            &daten,
            false,
            |p, i| sicher_schreiben(p, i),
        )?;
        Ok((orgs, neu, rest))
    });
    neues_passwort.zeroize();
    let (orgs, neu, rest) = ergebnis?;
    Ok(passwort_abschliessen(&orgs, &mut geheim, neu, rest))
}

/// "Neu aufsetzen" bei Passwortverlust: ALLE Tresore werden NICHT geloescht,
/// sondern mit Datum beiseitegelegt (falls das Passwort doch wieder einfaellt):
/// organisationen\alt-2026-06-12-<id>.enc. Danach kann ein frischer Tresor
/// angelegt werden. (Fuer EINE gesperrte Organisation siehe
/// tresor_gesperrte_neu_aufsetzen.)
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

/// Passwort EINER Organisation vergessen, die anderen sind offen: Nur sie wird
/// beiseitegelegt (wie beim Neu-Aufsetzen, nicht geloescht). Geht nur fuer eine
/// GESPERRTE Organisation und nur in einer offenen Sitzung – eine offene wuerde
/// man ueber "Entfernen" loswerden, und vom Sperr-Bildschirm aus kennt die App
/// die Namen nicht (sie stehen verschluesselt in den Tresoren).
#[tauri::command]
pub fn tresor_gesperrte_neu_aufsetzen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    org: String,
    datum: String,
) -> Result<(), String> {
    if datum.is_empty() || datum.len() > 10 || !datum.chars().all(|c| c.is_ascii_digit() || c == '-') {
        return Err("Ungueltiges Datum.".into());
    }
    let orgs = org_ordner(&app)?;
    let geheim = state.geheim.lock().unwrap();
    gesperrte_beiseitelegen(&orgs, &geheim, &org, &datum)
}

fn gesperrte_beiseitelegen(
    orgs: &Path,
    geheim: &HashMap<String, Geheimnis>,
    org: &str,
    datum: &str,
) -> Result<(), String> {
    if !id_gueltig(org) || !org_ids(orgs).iter().any(|i| i == org) {
        return Err(FREMDE_ORG.into());
    }
    if geheim.is_empty() {
        return Err("Der Tresor ist nicht entsperrt.".into());
    }
    if geheim.contains_key(org) {
        return Err("Diese Organisation ist offen. Zum Loswerden gibt es „Entfernen“.".into());
    }
    beiseitelegen(orgs, org, datum)
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

/// Offene Organisationen fuer dieses Geraet merken: ohne `orgs` alle offenen
/// (beim Anmelden), sonst nur die genannten (z. B. eine neue Organisation mit
/// demselben Passwort). Eine Organisation mit EIGENEM Passwort, die erst
/// spaeter in der Sitzung geoeffnet wurde, wird so nicht ungefragt gemerkt.
#[tauri::command]
pub fn merken_anlegen(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    orgs: Option<Vec<String>>,
) -> Result<(), String> {
    let geheim = state.geheim.lock().unwrap();
    if geheim.is_empty() {
        return Err("Der Tresor ist nicht entsperrt.".into());
    }
    let gewuenscht = |id: &String| orgs.as_ref().map_or(true, |liste| liste.contains(id));
    for (id, g) in geheim.iter().filter(|(id, _)| gewuenscht(id)) {
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

/// Welche Organisationen sind auf diesem Geraet gemerkt? (Nur IDs.)
#[tauri::command]
pub fn merken_liste(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let orgs = org_ordner(&app)?;
    Ok(org_ids(&orgs).into_iter().filter(|id| orgs.join(format!("{id}.merken")).exists()).collect())
}

/// Gemerkte Zugaenge entfernen (kuenftig wieder Passwort): ohne `orgs` alle
/// dieses Geraets ("Geraet vergessen"), sonst nur die genannten.
#[tauri::command]
pub fn merken_vergessen(app: tauri::AppHandle, orgs: Option<Vec<String>>) -> Result<(), String> {
    let ordner = org_ordner(&app)?;
    merken_entfernen(&ordner, orgs.as_deref())
}

fn merken_entfernen(orgs: &Path, nur: Option<&[String]>) -> Result<(), String> {
    for id in org_ids(orgs).into_iter().filter(|id| nur.map_or(true, |l| l.contains(id))) {
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

    /// Testordner mit Organisationen (id, passwort, daten).
    fn orgs_ordner(zweck: &str, orgs: &[(&str, &str, &str)]) -> PathBuf {
        let eindeutig = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let o = std::env::temp_dir().join(format!("a3000-o4-{zweck}-{eindeutig}"));
        fs::create_dir_all(&o).unwrap();
        for (id, pw, daten) in orgs {
            fs::write(o.join(format!("{id}.enc")), tresor_inhalt(pw, daten)).unwrap();
        }
        o
    }
    const A: &str = "aaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccc";
    fn datei(o: &Path, id: &str) -> Vec<u8> {
        fs::read(o.join(format!("{id}.enc"))).unwrap()
    }
    fn normal_schreiben(p: &PathBuf, i: &[u8]) -> Result<(), String> {
        sicher_schreiben(p, i)
    }

    #[test]
    fn passwortwechsel_erfasst_genau_die_geteilten() {
        let o = orgs_ordner("geteilt", &[(A, "gemeinsam1", "{\"a\":1}"), (B, "gemeinsam1", "{\"b\":1}"), (C, "eigenes-pw", "{\"c\":1}")]);
        let (neu, rest) =
            passwort_wechseln(&o, A, Nachweis::Passwort("gemeinsam1"), "nagelneu99", "{\"a\":2}", true, normal_schreiben).unwrap();
        let ids: Vec<&str> = neu.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec![A, B], "aktive zuerst, dazu nur die mit demselben Passwort");
        assert!(!rest);
        assert_eq!(oeffnen(&datei(&o, A), "nagelneu99").as_deref(), Some("{\"a\":2}"), "aktive mit neuem Stand");
        assert_eq!(oeffnen(&datei(&o, B), "nagelneu99").as_deref(), Some("{\"b\":1}"), "geteilte mit ihrem Inhalt");
        assert!(oeffnen(&datei(&o, A), "gemeinsam1").is_none());
        assert!(oeffnen(&datei(&o, B), "gemeinsam1").is_none());
        assert_eq!(oeffnen(&datei(&o, C), "eigenes-pw").as_deref(), Some("{\"c\":1}"), "eigenes Passwort bleibt unberuehrt");
        for id in [A, B, C] {
            assert!(!o.join(format!("{id}.enc.bak")).exists(), "keine Vorversion mit altem Passwort");
        }
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn nach_dem_wechsel_speichert_die_sitzung_mit_neuem_passwort() {
        let o = orgs_ordner("sitzung", &[(A, "gemeinsam1", "a"), (B, "gemeinsam1", "b")]);
        let mut geheim = HashMap::new();
        geheim.insert(A.to_string(), Geheimnis { schluessel: [9; 32], salt: [0; SALT_LAENGE], ueber_merken: false });
        let (neu, rest) = passwort_wechseln(&o, A, Nachweis::Passwort("gemeinsam1"), "nagelneu99", "a", true, normal_schreiben).unwrap();
        let ergebnis = passwort_abschliessen(&o, &mut geheim, neu, rest);
        assert_eq!(ergebnis.geaendert, vec![A.to_string(), B.to_string()]);
        assert!(ergebnis.merken_ok);
        assert!(!geheim.contains_key(B), "die gesperrte bleibt gesperrt");
        let g = geheim.get(A).unwrap();
        assert_eq!(salt_lesen(&datei(&o, A)).unwrap(), g.salt, "Sitzung kennt das neue Salt");
        // So speichert die App danach: muss mit dem neuen Passwort aufgehen.
        let gespeichert = verschluesseln("a-weiter", &g.schluessel, &g.salt).unwrap();
        assert_eq!(oeffnen(&gespeichert, "nagelneu99").as_deref(), Some("a-weiter"));
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn passwortwechsel_ohne_geteilte_nur_die_aktive() {
        let o = orgs_ordner("einzeln", &[(A, "gemeinsam1", "a"), (B, "gemeinsam1", "b")]);
        let (neu, _) = passwort_wechseln(&o, A, Nachweis::Passwort("gemeinsam1"), "nagelneu99", "a", false, normal_schreiben).unwrap();
        assert_eq!(neu.len(), 1);
        assert_eq!(oeffnen(&datei(&o, B), "gemeinsam1").as_deref(), Some("b"));
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn passwortwechsel_ist_alles_oder_nichts() {
        let o = orgs_ordner("rollback", &[(A, "gemeinsam1", "a"), (B, "gemeinsam1", "b")]);
        let zaehler = std::cell::Cell::new(0);
        let klemmt = |p: &PathBuf, i: &[u8]| {
            zaehler.set(zaehler.get() + 1);
            if zaehler.get() == 2 {
                return Err("Platte voll".to_string());
            }
            sicher_schreiben(p, i)
        };
        let fehler = passwort_wechseln(&o, A, Nachweis::Passwort("gemeinsam1"), "nagelneu99", "a-neu", true, klemmt)
            .err()
            .unwrap();
        assert!(fehler.contains("alte Passwort gilt weiter"));
        assert_eq!(oeffnen(&datei(&o, A), "gemeinsam1").as_deref(), Some("a"), "erste Datei ist zurueck");
        assert_eq!(oeffnen(&datei(&o, B), "gemeinsam1").as_deref(), Some("b"));
        assert!(oeffnen(&datei(&o, A), "nagelneu99").is_none());
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn passwortwechsel_mit_falschem_alten_schreibt_nichts() {
        let o = orgs_ordner("falsch", &[(A, "gemeinsam1", "a"), (B, "gemeinsam1", "b")]);
        let vorher = datei(&o, A);
        let e = passwort_wechseln(&o, A, Nachweis::Passwort("vertippt!"), "nagelneu99", "a", true, normal_schreiben).err().unwrap();
        assert_eq!(e, "falsches_passwort");
        assert_eq!(datei(&o, A), vorher);
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn gesperrte_neu_aufsetzen_trifft_nur_sie() {
        let o = orgs_ordner("einzel-neu", &[(A, "gemeinsam1", "a"), (B, "eigenes-pw", "b")]);
        fs::write(o.join(format!("{A}.merken")), b"x").unwrap();
        fs::write(o.join(format!("{B}.merken")), b"x").unwrap();
        let mut geheim = HashMap::new();
        // ohne offene Sitzung: nein
        assert!(gesperrte_beiseitelegen(&o, &geheim, B, "2026-11-14").is_err());
        geheim.insert(A.to_string(), Geheimnis { schluessel: [1; 32], salt: [0; SALT_LAENGE], ueber_merken: false });
        // die offene selbst: nein
        assert!(gesperrte_beiseitelegen(&o, &geheim, A, "2026-11-14").is_err());
        // unbekannte: nein
        assert!(gesperrte_beiseitelegen(&o, &geheim, C, "2026-11-14").is_err());
        gesperrte_beiseitelegen(&o, &geheim, B, "2026-11-14").unwrap();
        assert_eq!(org_ids(&o), vec![A.to_string()]);
        assert!(o.join(format!("alt-2026-11-14-{B}.enc")).exists());
        assert!(!o.join(format!("{B}.merken")).exists());
        assert!(o.join(format!("{A}.merken")).exists(), "Merken der anderen bleibt");
        let _ = fs::remove_dir_all(&o);
    }

    #[test]
    fn merken_gezielt_vergessen() {
        let o = orgs_ordner("merken", &[(A, "p1p1p1p1", "a"), (B, "p1p1p1p1", "b")]);
        fs::write(o.join(format!("{A}.merken")), b"x").unwrap();
        fs::write(o.join(format!("{B}.merken")), b"x").unwrap();
        merken_entfernen(&o, Some(&[B.to_string()])).unwrap();
        assert!(o.join(format!("{A}.merken")).exists());
        assert!(!o.join(format!("{B}.merken")).exists());
        merken_entfernen(&o, None).unwrap();
        assert!(!o.join(format!("{A}.merken")).exists());
        let _ = fs::remove_dir_all(&o);
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
