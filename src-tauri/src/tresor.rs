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

pub(crate) const MAGIC: &[u8; 8] = b"ANTRAG3K";
const VERSION: u8 = 1;
const SALT_LAENGE: usize = 16;
const NONCE_LAENGE: usize = 12;
const KOPF_LAENGE: usize = 8 + 1 + SALT_LAENGE + NONCE_LAENGE;

/// Wird nach erfolgreichem Entsperren im Arbeitsspeicher gehalten
/// (nie auf der Festplatte). Beim Sperren oder Beenden ist es weg.
pub struct Geheimnis {
    schluessel: [u8; 32],
    salt: [u8; SALT_LAENGE],
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
fn schluessel_ableiten(passwort: &str, salt: &[u8]) -> Result<[u8; 32], String> {
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
    passwort: String,
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
    let schluessel = schluessel_ableiten(&passwort, &salt)?;

    let inhalt = verschluesseln(&daten, &schluessel, &salt)?;
    sicher_schreiben(&pfad, &inhalt)?;

    // Schluessel im Arbeitsspeicher behalten, damit Speichern ohne
    // erneute Passworteingabe moeglich ist.
    *state.geheim.lock().unwrap() = Some(Geheimnis { schluessel, salt });
    Ok(())
}

/// Tresor mit Passwort oeffnen. Liefert die entschluesselten Daten (JSON).
#[tauri::command]
pub fn tresor_entsperren(
    app: tauri::AppHandle,
    state: tauri::State<TresorZustand>,
    passwort: String,
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

    let schluessel = schluessel_ableiten(&passwort, &salt)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&schluessel));

    // Bricht das Echtheits-Siegel, ist (praktisch immer) das Passwort
    // falsch. Wir geben einen festen Code zurueck, den das Frontend
    // in eine freundliche Meldung uebersetzt.
    let klartext = cipher
        .decrypt(Nonce::from_slice(nonce), verschluesselt)
        .map_err(|_| "falsches_passwort".to_string())?;

    let daten = String::from_utf8(klartext)
        .map_err(|_| "Tresor-Inhalt ist kein gueltiger Text.".to_string())?;

    *state.geheim.lock().unwrap() = Some(Geheimnis { schluessel, salt });
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

    *state.geheim.lock().unwrap() = Some(Geheimnis { schluessel, salt });
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
