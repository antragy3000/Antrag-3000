// ============================================================
// Sicherung (Backup): Tresor, wahlweise mit allen Dateien.
//
// Zwei Arten:
//
// 1. "Nur Tresor" (.enc): schlicht eine Kopie der ohnehin verschluesselten
//    tresor.enc.
//
// 2. "Mit Dateien" (.a3ksicherung): Tresor PLUS alles aus
//    Dokumente\Antrag 3000 (Belege, Anhaenge, PDFs, Word/Excel). Diese
//    Dateien liegen auf dem Computer lesbar. Wuerden sie so auf einen
//    USB-Stick oder in einen Cloud-Ordner wandern, laegen Belege und
//    Antrags-PDFs (IBAN!) offen herum. Darum wird diese Sicherung als
//    GANZES verschluesselt. Waehrend des Sicherns entsteht nirgends eine
//    lesbare Zwischendatei: alles laeuft direkt durch die Verschluesselung.
//
// Aufbau einer .a3ksicherung:
//
//   [Magic "ANTRAG3S" 8][Version 1][Salt 16]
//   [Huellen-Nonce 12][verpackter Schluessel 48][Strom-Nonce 7]
//   [verschluesselte Bloecke zu je 64 KiB ...]
//
// - Jede Sicherung bekommt einen eigenen Zufalls-Schluessel. Der ist mit dem
//   Tresor-Schluessel "verpackt" (AES-GCM). So geht das Sichern auch in einer
//   Sitzung ohne Passwort ("Auf diesem Geraet merken"), und zum Oeffnen reicht
//   das Passwort plus das hier mitgespeicherte Salt.
// - Die Bloecke verschluesselt das STREAM-Verfahren aus dem etablierten
//   RustCrypto-Baustein aead (keine eigene Krypto). Es erkennt veraenderte,
//   vertauschte und abgeschnittene Bloecke, ohne dass die ganze Datei in den
//   Arbeitsspeicher muss.
//
// Der entschluesselte Inhalt ist eine einfache Folge von Eintraegen:
//
//   [Namenslaenge u16][Name][Groesse u64][Inhalt]   ... Namenslaenge 0 = Ende
//
// Bewusst kein ZIP: ZIP braucht zum Lesen eine lesbare Zwischendatei (siehe
// oben) und kennt Dinge, die wir nicht wollen (Verknuepfungen, Kompression
// mit "Zip-Bomben"). Der erste Eintrag ist immer "tresor.enc", danach kommen
// "dateien/<Pfad relativ zu Dokumente\Antrag 3000>".
//
// Wiederherstellen laeuft in zwei Durchgaengen: erst wird die ganze Sicherung
// geprueft (Siegel + jeder Dateipfad), dann erst wird etwas geschrieben. Eine
// beschaedigte oder manipulierte Sicherung veraendert also nichts. Vorhandene
// Dateien werden nie ueberschrieben, der Tresor wird als Letztes getauscht.
// ============================================================

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::stream::{DecryptorBE32, EncryptorBE32};
use aes_gcm::aead::{Aead, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use serde::Serialize;
use tauri::Manager;
use zeroize::{Zeroize, Zeroizing};

use crate::{ordner, tresor};

const MAGIC_KOMPLETT: &[u8; 8] = b"ANTRAG3S";
const VERSION: u8 = 1;
const SALT_ENDE: usize = 9 + tresor::SALT_LAENGE;
const HUELLE_NONCE_ENDE: usize = SALT_ENDE + 12;
const VERPACKT_ENDE: usize = HUELLE_NONCE_ENDE + 32 + 16;
const KOPF_LAENGE: usize = VERPACKT_ENDE + 7;
const BLOCK: usize = 64 * 1024;
const BLOCK_VERSCHLUESSELT: usize = BLOCK + 16;
const NAME_MAX: usize = 4096;

const EINTRAG_TRESOR: &str = "tresor.enc";
const PRAEFIX_DATEIEN: &str = "dateien/";
const ENDUNG_KOMPLETT: &str = ".a3ksicherung";

const KEINE_SICHERUNG: &str = "Das ist keine gültige Antrag-3000-Sicherung.";
const BESCHAEDIGT: &str = "Die Sicherung ist beschädigt oder wurde verändert. Es wurde nichts eingespielt.";
const UNZULAESSIG: &str = "Die Sicherung enthält unzulässige Dateipfade. Es wurde nichts eingespielt.";

/// Rueckmeldung ans Frontend.
#[derive(Serialize, Default, Debug)]
pub struct Bericht {
    /// Gesicherte bzw. wiederhergestellte Dateien (ohne Tresor).
    pub dateien: u64,
    pub bytes: u64,
    /// Wiederherstellen: schon vorhanden, darum nicht angefasst.
    pub vorhanden: u64,
    /// Dateien, die nicht gesichert bzw. nicht wiederhergestellt werden konnten.
    pub probleme: Vec<String>,
}

/// Entfernt eine halb fertige Datei, falls wir vorzeitig aussteigen.
/// Nach einem erfolgreichen Umbenennen ist nichts mehr da – dann passiert nichts.
struct Aufraeumen(PathBuf);

impl Drop for Aufraeumen {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

// ============================================================
// Der Kopf der Datei und der verpackte Schluessel
// ============================================================

fn kopf_bauen(
    datenschluessel: &[u8; 32],
    schluessel: &[u8; 32],
    salt: &[u8; tresor::SALT_LAENGE],
) -> Result<[u8; KOPF_LAENGE], String> {
    let mut kopf = [0u8; KOPF_LAENGE];
    kopf[..8].copy_from_slice(MAGIC_KOMPLETT);
    kopf[8] = VERSION;
    kopf[9..SALT_ENDE].copy_from_slice(salt);
    OsRng.fill_bytes(&mut kopf[SALT_ENDE..HUELLE_NONCE_ENDE]);
    OsRng.fill_bytes(&mut kopf[VERPACKT_ENDE..]);
    let verpackt = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(schluessel))
        .encrypt(
            Nonce::from_slice(&kopf[SALT_ENDE..HUELLE_NONCE_ENDE]),
            Payload { msg: datenschluessel, aad: &kopf[..SALT_ENDE] },
        )
        .map_err(|_| "Verschluesselung fehlgeschlagen".to_string())?;
    kopf[HUELLE_NONCE_ENDE..VERPACKT_ENDE].copy_from_slice(&verpackt);
    Ok(kopf)
}

/// Passt der Schluessel nicht, ist (praktisch immer) das Passwort falsch:
/// fester Code wie beim Entsperren des Tresors.
fn datenschluessel_entpacken(
    kopf: &[u8; KOPF_LAENGE],
    schluessel: &[u8; 32],
) -> Result<Zeroizing<[u8; 32]>, String> {
    let mut klar = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(schluessel))
        .decrypt(
            Nonce::from_slice(&kopf[SALT_ENDE..HUELLE_NONCE_ENDE]),
            Payload { msg: &kopf[HUELLE_NONCE_ENDE..VERPACKT_ENDE], aad: &kopf[..SALT_ENDE] },
        )
        .map_err(|_| "falsches_passwort".to_string())?;
    let mut datenschluessel = Zeroizing::new([0u8; 32]);
    datenschluessel.copy_from_slice(&klar);
    klar.zeroize();
    Ok(datenschluessel)
}

fn kopf_lesen(quelle: &Path) -> Result<(BufReader<File>, [u8; KOPF_LAENGE]), String> {
    let datei = File::open(quelle).map_err(|e| format!("Sicherungsdatei nicht lesbar: {e}"))?;
    let mut leser = BufReader::new(datei);
    let mut kopf = [0u8; KOPF_LAENGE];
    leser.read_exact(&mut kopf).map_err(|_| KEINE_SICHERUNG.to_string())?;
    if &kopf[..8] != MAGIC_KOMPLETT {
        return Err(KEINE_SICHERUNG.into());
    }
    if kopf[8] != VERSION {
        return Err("Diese Sicherung stammt aus einer neueren Programmversion. Bitte aktualisiere Antrag 3000.".into());
    }
    Ok((leser, kopf))
}

// ============================================================
// Verschluesselter Strom: schreiben und lesen in Bloecken
// ============================================================

/// Sammelt Daten zu Bloecken und schreibt sie verschluesselt weiter. Jeder
/// volle Block ist genau BLOCK_VERSCHLUESSELT gross; der letzte ist immer
/// kuerzer (notfalls leer) und traegt die Ende-Markierung. Daran erkennt der
/// Leser ohne Vorausschau den Schluss, und abgeschnittene Dateien fallen auf.
struct BlockSchreiber<W: Write> {
    ziel: W,
    enc: Option<EncryptorBE32<Aes256Gcm>>,
    puffer: Vec<u8>,
    kopf: [u8; KOPF_LAENGE],
}

impl<W: Write> Write for BlockSchreiber<W> {
    fn write(&mut self, daten: &[u8]) -> io::Result<usize> {
        let n = (BLOCK - self.puffer.len()).min(daten.len());
        self.puffer.extend_from_slice(&daten[..n]);
        if self.puffer.len() == BLOCK {
            let enc = self.enc.as_mut().ok_or_else(strom_fehler)?;
            let block = enc
                .encrypt_next(Payload { msg: &self.puffer, aad: &self.kopf })
                .map_err(|_| strom_fehler())?;
            self.ziel.write_all(&block)?;
            self.puffer.clear();
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.ziel.flush()
    }
}

impl<W: Write> BlockSchreiber<W> {
    fn abschliessen(mut self) -> io::Result<W> {
        let enc = self.enc.take().ok_or_else(strom_fehler)?;
        let block = enc
            .encrypt_last(Payload { msg: &self.puffer, aad: &self.kopf })
            .map_err(|_| strom_fehler())?;
        self.ziel.write_all(&block)?;
        self.ziel.flush()?;
        Ok(self.ziel)
    }
}

/// Gegenstueck: liefert nur Daten, deren Siegel gestimmt hat.
struct BlockLeser<R: Read> {
    quelle: R,
    dec: Option<DecryptorBE32<Aes256Gcm>>,
    kopf: [u8; KOPF_LAENGE],
    roh: Vec<u8>,
    klartext: Vec<u8>,
    pos: usize,
}

impl<R: Read> Read for BlockLeser<R> {
    fn read(&mut self, ziel: &mut [u8]) -> io::Result<usize> {
        while self.pos == self.klartext.len() {
            if self.dec.is_none() {
                return Ok(0);
            }
            let n = voll_lesen(&mut self.quelle, &mut self.roh)?;
            let block = if n == BLOCK_VERSCHLUESSELT {
                let dec = self.dec.as_mut().ok_or_else(strom_fehler)?;
                dec.decrypt_next(Payload { msg: &self.roh[..n], aad: &self.kopf })
            } else {
                let dec = self.dec.take().ok_or_else(strom_fehler)?;
                dec.decrypt_last(Payload { msg: &self.roh[..n], aad: &self.kopf })
            };
            self.klartext = block.map_err(|_| strom_fehler())?;
            self.pos = 0;
        }
        let n = ziel.len().min(self.klartext.len() - self.pos);
        ziel[..n].copy_from_slice(&self.klartext[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

fn strom_fehler() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, BESCHAEDIGT)
}

/// Liest, bis der Puffer voll oder die Datei zu Ende ist.
fn voll_lesen(quelle: &mut impl Read, puffer: &mut [u8]) -> io::Result<usize> {
    let mut gelesen = 0;
    while gelesen < puffer.len() {
        match quelle.read(&mut puffer[gelesen..]) {
            Ok(0) => break,
            Ok(n) => gelesen += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(gelesen)
}

/// Legt die Zieldatei an, schreibt den Kopf und wuerfelt den Sicherungs-Schluessel.
fn schreiber_oeffnen(
    pfad: &Path,
    schluessel: &[u8; 32],
    salt: &[u8; tresor::SALT_LAENGE],
) -> Result<(BlockSchreiber<BufWriter<File>>, Zeroizing<[u8; 32]>), String> {
    let mut datenschluessel = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(&mut datenschluessel[..]);
    let kopf = kopf_bauen(&datenschluessel, schluessel, salt)?;

    let datei = File::create(pfad).map_err(|e| format!("Sicherung nicht anlegbar: {e}"))?;
    let mut ziel = BufWriter::new(datei);
    ziel.write_all(&kopf).map_err(|e| format!("Sicherung fehlgeschlagen: {e}"))?;

    let enc = EncryptorBE32::from_aead(
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&datenschluessel[..])),
        GenericArray::from_slice(&kopf[VERPACKT_ENDE..]),
    );
    let schreiber = BlockSchreiber { ziel, enc: Some(enc), puffer: Vec::with_capacity(BLOCK), kopf };
    Ok((schreiber, datenschluessel))
}

fn leser_oeffnen(
    quelle: &Path,
    datenschluessel: &[u8; 32],
) -> Result<BlockLeser<BufReader<File>>, String> {
    let (leser, kopf) = kopf_lesen(quelle)?;
    let dec = DecryptorBE32::from_aead(
        Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(datenschluessel)),
        GenericArray::from_slice(&kopf[VERPACKT_ENDE..]),
    );
    Ok(BlockLeser {
        quelle: leser,
        dec: Some(dec),
        kopf,
        roh: vec![0u8; BLOCK_VERSCHLUESSELT],
        klartext: Vec::new(),
        pos: 0,
    })
}

// ============================================================
// Die Eintraege im Strom
// ============================================================

fn eintrag_schreiben(
    ziel: &mut impl Write,
    name: &str,
    groesse: u64,
    inhalt: &mut impl Read,
) -> io::Result<()> {
    ziel.write_all(&(name.len() as u16).to_le_bytes())?;
    ziel.write_all(name.as_bytes())?;
    ziel.write_all(&groesse.to_le_bytes())?;
    let kopiert = io::copy(&mut inhalt.take(groesse), ziel)?;
    if kopiert != groesse {
        // Die Groesse steht schon im Strom und laesst sich nicht mehr korrigieren.
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Die Datei hat sich während des Sicherns verändert. Schließe sie und versuche es noch einmal.",
        ));
    }
    Ok(())
}

fn lesefehler(e: io::Error) -> String {
    match e.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => BESCHAEDIGT.to_string(),
        _ => format!("Sicherungsdatei nicht lesbar: {e}"),
    }
}

/// Geht alle Eintraege durch. Was der Besucher nicht liest, wird trotzdem
/// entschluesselt – so prueft jeder Durchgang das Siegel der ganzen Datei.
fn eintraege_durchlaufen<R: Read>(
    leser: &mut R,
    mut besucher: impl FnMut(&str, &mut dyn Read) -> Result<(), String>,
) -> Result<(), String> {
    loop {
        let mut laenge = [0u8; 2];
        leser.read_exact(&mut laenge).map_err(lesefehler)?;
        let laenge = u16::from_le_bytes(laenge) as usize;
        if laenge == 0 {
            // Endmarke: danach darf nichts mehr kommen, und dieses Lesen
            // erreicht auch den Abschluss-Block mit der Ende-Markierung.
            let mut rest = [0u8; 1];
            if leser.read(&mut rest).map_err(lesefehler)? != 0 {
                return Err(BESCHAEDIGT.into());
            }
            return Ok(());
        }
        if laenge > NAME_MAX {
            return Err(UNZULAESSIG.into());
        }
        let mut name = vec![0u8; laenge];
        leser.read_exact(&mut name).map_err(lesefehler)?;
        let name = String::from_utf8(name).map_err(|_| UNZULAESSIG.to_string())?;
        let mut groesse = [0u8; 8];
        leser.read_exact(&mut groesse).map_err(lesefehler)?;

        let mut inhalt = (&mut *leser).take(u64::from_le_bytes(groesse));
        besucher(&name, &mut inhalt)?;
        io::copy(&mut inhalt, &mut io::sink()).map_err(lesefehler)?;
        if inhalt.limit() != 0 {
            return Err(BESCHAEDIGT.into());
        }
    }
}

/// Schutz vor Pfad-Tricks ("Zip-Slip"): Macht aus einem Namen in der Sicherung
/// einen Pfad, der garantiert UNTERHALB des Zielordners bleibt.
///  - Err: Der Name versucht aus dem Ordner auszubrechen (..\, absolute Pfade)
///    → das kommt in einer echten Sicherung nie vor, darum Abbruch.
///  - Ok(None): harmlos, aber auf diesem System kein gueltiger Name (z. B. ein
///    Doppelpunkt aus einer Mac-Sicherung, "a:b" oder CON.txt) → wird
///    uebersprungen und gemeldet, nie geschrieben.
fn relativer_pfad(name: &str) -> Result<Option<PathBuf>, String> {
    if name.split(['/', '\\']).any(|t| t.is_empty() || t == "." || t == "..") {
        return Err(UNZULAESSIG.into());
    }
    let mut pfad = PathBuf::new();
    for teil in name.split('/') {
        // bereinigen() kennt alle Windows-Regeln: Ein Name, den es veraendern
        // wuerde, ist so nicht verwendbar.
        if ordner::bereinigen(teil).as_deref() != Ok(teil) {
            return Ok(None);
        }
        pfad.push(teil);
    }
    if !pfad.components().all(|k| matches!(k, Component::Normal(_))) {
        return Err(UNZULAESSIG.into());
    }
    Ok(Some(pfad))
}

/// Erster Durchgang: prueft die ganze Datei, ohne etwas zu schreiben.
/// Rueckgabe: Zahl der Datei-Eintraege.
fn inhalt_pruefen(quelle: &Path, datenschluessel: &[u8; 32]) -> Result<u64, String> {
    let mut leser = leser_oeffnen(quelle, datenschluessel)?;
    let mut tresor_gesehen = false;
    let mut dateien = 0u64;
    eintraege_durchlaufen(&mut leser, |name, inhalt| {
        if name == EINTRAG_TRESOR {
            let mut anfang = [0u8; 9];
            inhalt.read_exact(&mut anfang).map_err(|_| BESCHAEDIGT.to_string())?;
            if tresor_gesehen || &anfang[..8] != tresor::MAGIC {
                return Err(BESCHAEDIGT.into());
            }
            tresor_gesehen = true;
            return Ok(());
        }
        // Der Tresor steht immer vorn; alles andere muss unter dateien/ liegen.
        let relativ = name.strip_prefix(PRAEFIX_DATEIEN).ok_or(UNZULAESSIG.to_string())?;
        if !tresor_gesehen {
            return Err(BESCHAEDIGT.into());
        }
        relativer_pfad(relativ)?;
        dateien += 1;
        Ok(())
    })?;
    if !tresor_gesehen {
        return Err(BESCHAEDIGT.into());
    }
    Ok(dateien)
}

// ============================================================
// Sicherung erstellen
// ============================================================

/// Eigene Sicherungen nicht mitsichern (sie koennen im selben Ordner liegen),
/// die kurzlebigen Sperrdateien von Word ("~$...") auch nicht.
fn gehoert_nicht_in_sicherung(name: &str) -> bool {
    name.starts_with("~$")
        || name.ends_with(ENDUNG_KOMPLETT)
        || name.ends_with(&format!("{ENDUNG_KOMPLETT}.teil"))
}

/// Sammelt alle normalen Dateien unter `wurzel`. Verknuepfungen werden bewusst
/// nicht verfolgt: Sie koennten aus dem Ordner hinausfuehren, und was dort
/// liegt, gehoert nicht in diese Sicherung. Sie werden gemeldet.
fn dateien_sammeln(wurzel: &Path) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let mut dateien = Vec::new();
    let mut probleme = Vec::new();
    let mut offen = vec![(wurzel.to_path_buf(), String::new())];
    while let Some((ordner_pfad, praefix)) = offen.pop() {
        let eintraege = match fs::read_dir(&ordner_pfad) {
            Ok(e) => e,
            // Gibt es den Hauptordner noch nicht, ist schlicht nichts zu sichern.
            Err(_) if praefix.is_empty() => continue,
            Err(_) => {
                probleme.push(praefix);
                continue;
            }
        };
        for eintrag in eintraege.flatten() {
            let name = match eintrag.file_name().into_string() {
                Ok(n) => n,
                Err(n) => {
                    probleme.push(format!("{praefix}{}", n.to_string_lossy()));
                    continue;
                }
            };
            let relativ = format!("{praefix}{name}");
            match eintrag.file_type() {
                Ok(art) if art.is_symlink() => probleme.push(relativ),
                Ok(art) if art.is_dir() => offen.push((eintrag.path(), format!("{relativ}/"))),
                Ok(art) if art.is_file() => {
                    if !gehoert_nicht_in_sicherung(&name) {
                        dateien.push((relativ, eintrag.path()));
                    }
                }
                Ok(_) => {}
                Err(_) => probleme.push(relativ),
            }
        }
    }
    dateien.sort();
    (dateien, probleme)
}

/// Schreibt die Sicherung erst unter ".teil" und benennt sie erst um, wenn
/// sie komplett und gegengelesen ist. So bleibt z. B. bei vollem USB-Stick
/// keine kaputte Datei liegen, die man spaeter fuer eine Sicherung haelt.
fn komplett_schreiben(
    ziel: &Path,
    tresor_inhalt: &[u8],
    wurzel: &Path,
    schluessel: &[u8; 32],
    salt: &[u8; tresor::SALT_LAENGE],
) -> Result<Bericht, String> {
    let (dateien, probleme) = dateien_sammeln(wurzel);
    let mut bericht = Bericht { probleme, ..Bericht::default() };

    let mut teil = ziel.as_os_str().to_owned();
    teil.push(".teil");
    let teil = PathBuf::from(teil);
    let _aufraeumen = Aufraeumen(teil.clone());
    let (mut schreiber, datenschluessel) = schreiber_oeffnen(&teil, schluessel, salt)?;

    let fehlgeschlagen = |e: io::Error| format!("Sicherung fehlgeschlagen: {e}");
    eintrag_schreiben(&mut schreiber, EINTRAG_TRESOR, tresor_inhalt.len() as u64, &mut &*tresor_inhalt)
        .map_err(fehlgeschlagen)?;
    for (relativ, pfad) in dateien {
        let name = format!("{PRAEFIX_DATEIEN}{relativ}");
        let geoeffnet = File::open(&pfad).and_then(|d| Ok((d.metadata()?.len(), d)));
        let (groesse, mut datei) = match geoeffnet {
            Ok(g) if name.len() <= NAME_MAX => g,
            _ => {
                bericht.probleme.push(relativ);
                continue;
            }
        };
        eintrag_schreiben(&mut schreiber, &name, groesse, &mut datei)
            .map_err(|e| format!("Sicherung abgebrochen bei „{relativ}“: {e}"))?;
        bericht.dateien += 1;
        bericht.bytes += groesse;
    }
    schreiber.write_all(&0u16.to_le_bytes()).map_err(fehlgeschlagen)?;
    {
        let puffer = schreiber.abschliessen().map_err(fehlgeschlagen)?;
        let datei = puffer.into_inner().map_err(|e| fehlgeschlagen(e.into_error()))?;
        datei.sync_all().map_err(fehlgeschlagen)?;
    }

    // Gegenlesen: Eine Sicherung ist erst etwas wert, wenn sie sich oeffnen laesst.
    if inhalt_pruefen(&teil, &datenschluessel)? != bericht.dateien {
        return Err("Die Kontrolle der Sicherung ist fehlgeschlagen.".into());
    }
    fs::rename(&teil, ziel).map_err(|e| format!("Sicherung nicht speicherbar: {e}"))?;
    Ok(bericht)
}

// ============================================================
// Sicherung einspielen
// ============================================================

/// Schreibt eine Datei, ohne je etwas zu ueberschreiben (create_new).
fn datei_einspielen(wurzel: &Path, relativ: &Path, inhalt: &mut dyn Read) -> io::Result<()> {
    let ziel = wurzel.join(relativ);
    if ziel.symlink_metadata().is_ok() {
        return Err(io::ErrorKind::AlreadyExists.into());
    }
    if let Some(eltern) = ziel.parent() {
        fs::create_dir_all(eltern)?;
        // Letzte Kontrolle: Eine im Ordnerbaum vorhandene Verknuepfung darf
        // nicht aus dem Zielordner hinausfuehren.
        if !eltern.canonicalize()?.starts_with(wurzel.canonicalize()?) {
            return Err(io::ErrorKind::PermissionDenied.into());
        }
    }
    let mut datei = OpenOptions::new().write(true).create_new(true).open(&ziel)?;
    if let Err(e) = io::copy(inhalt, &mut datei) {
        drop(datei);
        let _ = fs::remove_file(&ziel);
        return Err(e);
    }
    Ok(())
}

/// Prueft die Sicherung komplett und entpackt sie dann: die Dateien nach
/// `wurzel`, den Tresor nach `tresor_neu` (noch NICHT an seinen Platz).
fn komplett_entpacken(
    quelle: &Path,
    passwort: &str,
    tresor_neu: &Path,
    wurzel: &Path,
) -> Result<Bericht, String> {
    let (_, kopf) = kopf_lesen(quelle)?;
    let schluessel = Zeroizing::new(tresor::schluessel_ableiten(passwort, &kopf[9..SALT_ENDE])?);
    let datenschluessel = datenschluessel_entpacken(&kopf, &schluessel)?;

    // 1. Durchgang: Stimmt alles? Bis hierher wurde nichts geschrieben.
    inhalt_pruefen(quelle, &datenschluessel)?;

    // 2. Durchgang: wirklich schreiben.
    fs::create_dir_all(wurzel).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let mut bericht = Bericht::default();
    let mut leser = leser_oeffnen(quelle, &datenschluessel)?;
    eintraege_durchlaufen(&mut leser, |name, inhalt| {
        if name == EINTRAG_TRESOR {
            let mut neu = File::create(tresor_neu).map_err(|e| format!("Tresor nicht schreibbar: {e}"))?;
            io::copy(inhalt, &mut neu).map_err(|e| format!("Tresor nicht schreibbar: {e}"))?;
            neu.sync_all().map_err(|e| format!("Tresor nicht schreibbar: {e}"))?;
            return Ok(());
        }
        let relativ = name.strip_prefix(PRAEFIX_DATEIEN).ok_or(UNZULAESSIG.to_string())?;
        let Some(pfad) = relativer_pfad(relativ)? else {
            bericht.probleme.push(relativ.to_string());
            return Ok(());
        };
        let mut gezaehlt = inhalt.take(u64::MAX);
        match datei_einspielen(wurzel, &pfad, &mut gezaehlt) {
            Ok(()) => {
                bericht.dateien += 1;
                bericht.bytes += u64::MAX - gezaehlt.limit();
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => bericht.vorhanden += 1,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => return Err(BESCHAEDIGT.into()),
            // Z. B. Pfad zu lang: Eine einzelne Datei soll nicht den ganzen
            // Umzug scheitern lassen – sie wird gemeldet.
            Err(_) => bericht.probleme.push(relativ.to_string()),
        }
        Ok(())
    })?;
    Ok(bericht)
}

/// Legt den bisherigen Tresor beiseite (nicht loeschen) und setzt den neuen ein.
fn tresor_austauschen(neu: &Path, ziel: &Path) -> Result<(), String> {
    let vorher = ziel.with_extension("enc.vor-wiederherstellung");
    let hatte_tresor = ziel.exists();
    if hatte_tresor {
        let _ = fs::remove_file(&vorher);
        fs::rename(ziel, &vorher)
            .map_err(|e| format!("Aktueller Tresor konnte nicht gesichert werden: {e}"))?;
    }
    fs::rename(neu, ziel).map_err(|e| {
        if hatte_tresor {
            let _ = fs::rename(&vorher, ziel);
        }
        format!("Wiederherstellen fehlgeschlagen: {e}")
    })
}

/// Welche Art von Sicherung ist diese Datei? Fremde Dateien werden abgewiesen.
fn art_erkennen(quelle: &Path) -> Result<&'static str, String> {
    let mut anfang = [0u8; 9];
    File::open(quelle)
        .and_then(|mut d| d.read_exact(&mut anfang))
        .map_err(|_| KEINE_SICHERUNG.to_string())?;
    if &anfang[..8] == tresor::MAGIC {
        Ok("tresor")
    } else if &anfang[..8] == MAGIC_KOMPLETT {
        Ok("komplett")
    } else {
        Err(KEINE_SICHERUNG.into())
    }
}

// ============================================================
// Tauri-Befehle
// ============================================================

/// Wohin eine Sicherung eingespielt wird: in eine vorhandene Organisation
/// (`org` = ihre ID, sie wird ersetzt) oder – ohne `org` – als NEUE
/// Organisation (erster Start auf einem neuen Computer).
fn einspiel_ziel(app: &tauri::AppHandle, org: Option<String>) -> Result<PathBuf, String> {
    match org {
        Some(id) => tresor::tresor_pfad(app, &id),
        None => tresor::neuer_tresor_pfad(app).map(|(_, pfad)| pfad),
    }
}

/// Kopiert den Tresor einer Organisation an den gewaehlten Zielpfad ("nur Tresor").
#[tauri::command]
pub fn tresor_backup_erstellen(app: tauri::AppHandle, org: String, ziel: String) -> Result<(), String> {
    let quelle = tresor::tresor_pfad(&app, &org)?;
    if !quelle.exists() {
        return Err("Es gibt noch keinen Tresor zum Sichern.".into());
    }
    fs::copy(&quelle, &ziel).map_err(|e| format!("Sicherung fehlgeschlagen: {e}"))?;
    Ok(())
}

/// Spielt eine "Nur Tresor"-Sicherung ein und ersetzt den aktuellen Tresor.
/// Der bisherige Tresor wird vorher beiseitegelegt (nicht geloescht).
#[tauri::command]
pub fn tresor_backup_einspielen(
    app: tauri::AppHandle,
    state: tauri::State<tresor::TresorZustand>,
    org: Option<String>,
    quelle: String,
) -> Result<(), String> {
    let daten = fs::read(&quelle).map_err(|e| format!("Sicherungsdatei nicht lesbar: {e}"))?;
    // Nur eine echte Antrag-3000-Sicherung akzeptieren.
    if daten.len() < 9 || &daten[..8] != tresor::MAGIC {
        return Err(KEINE_SICHERUNG.into());
    }

    let ziel = einspiel_ziel(&app, org)?;
    let neu = ziel.with_extension("enc.neu");
    let _aufraeumen = Aufraeumen(neu.clone());
    fs::write(&neu, &daten).map_err(|e| format!("Wiederherstellen fehlgeschlagen: {e}"))?;

    // Das Schloss haelt gleichzeitiges Speichern fern: Sonst koennte der alte
    // Schluessel noch in den gerade getauschten Tresor schreiben. Danach ist
    // gesperrt: Der wiederhergestellte Tresor wird mit (s)einem Passwort
    // neu entsperrt.
    let mut geheim = state.geheim.lock().unwrap();
    tresor_austauschen(&neu, &ziel)?;
    geheim.clear();
    Ok(())
}

/// "tresor" (.enc) oder "komplett" (.a3ksicherung). Das Frontend fragt danach
/// nur bei "komplett" schon vor dem Einspielen nach dem Passwort.
#[tauri::command]
pub fn sicherung_art(quelle: String) -> Result<String, String> {
    art_erkennen(Path::new(&quelle)).map(String::from)
}

/// Sicherung mit Dateien erstellen (Tresor muss offen sein). Kann bei vielen
/// Dateien dauern, darum ausserhalb des Hauptfadens – sonst friert die App ein.
#[tauri::command]
pub async fn sicherung_erstellen(app: tauri::AppHandle, org: String, ziel: String) -> Result<Bericht, String> {
    let (schluessel, salt, tresor_inhalt) = tresor::offener_stand(&app, &org)?;
    let wurzel = ordner::wurzel(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        komplett_schreiben(Path::new(&ziel), &tresor_inhalt, &wurzel, &schluessel, &salt)
    })
    .await
    .map_err(|e| format!("Sicherung abgebrochen: {e}"))?
}

/// Sicherung mit Dateien einspielen. Braucht KEINEN offenen Tresor – so geht es
/// auch auf einem neuen Computer direkt beim ersten Start.
#[tauri::command]
pub async fn sicherung_einspielen(
    app: tauri::AppHandle,
    org: Option<String>,
    quelle: String,
    passwort: String,
) -> Result<Bericht, String> {
    let passwort = Zeroizing::new(passwort);
    let tresor_ziel = einspiel_ziel(&app, org)?;
    let wurzel = ordner::wurzel(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let neu = tresor_ziel.with_extension("enc.neu");
        let _aufraeumen = Aufraeumen(neu.clone());
        let bericht = komplett_entpacken(Path::new(&quelle), &passwort, &neu, &wurzel)?;

        // Wie in tresor_backup_einspielen: Tausch und Sperren unter dem Schloss.
        let zustand = app.state::<tresor::TresorZustand>();
        let mut geheim = zustand.geheim.lock().unwrap();
        tresor_austauschen(&neu, &tresor_ziel)?;
        geheim.clear();
        Ok(bericht)
    })
    .await
    .map_err(|e| format!("Wiederherstellen abgebrochen: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORT: &str = "test-passwort";
    const GEHEIMER_INHALT: &[u8] = b"GEHEIMER-BELEG-INHALT IBAN CH00 1234";

    /// Eigener Ordner je Test, wird am Ende entfernt.
    struct Ordner(PathBuf);

    impl Ordner {
        fn neu() -> Self {
            let mut zufall = [0u8; 8];
            OsRng.fill_bytes(&mut zufall);
            let pfad = std::env::temp_dir().join(format!("a3000-sicherung-test-{:x}", u64::from_le_bytes(zufall)));
            fs::create_dir_all(&pfad).unwrap();
            Ordner(pfad)
        }
        fn pfad(&self, teil: &str) -> PathBuf {
            self.0.join(teil)
        }
    }

    impl Drop for Ordner {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Passwort-Schluessel und ein Tresor-Inhalt, wie sie die App hat.
    fn zugang() -> ([u8; 32], [u8; tresor::SALT_LAENGE], Vec<u8>) {
        let mut salt = [0u8; tresor::SALT_LAENGE];
        OsRng.fill_bytes(&mut salt);
        let schluessel = tresor::schluessel_ableiten(PASSWORT, &salt).unwrap();
        let mut inhalt = tresor::MAGIC.to_vec();
        inhalt.extend_from_slice(b"\x01-tresor-testinhalt");
        (schluessel, salt, inhalt)
    }

    fn datei_anlegen(wurzel: &Path, relativ: &str, inhalt: &[u8]) {
        let pfad = wurzel.join(relativ);
        fs::create_dir_all(pfad.parent().unwrap()).unwrap();
        fs::write(pfad, inhalt).unwrap();
    }

    /// Standard-Sicherung mit einem kleinen und einem mehrere Bloecke grossen Beleg.
    fn sicherung_bauen(o: &Ordner) -> (PathBuf, Vec<u8>) {
        let (schluessel, salt, tresor_inhalt) = zugang();
        let alt = o.pfad("alt");
        datei_anlegen(&alt, "Projekt A/Belege/beleg.pdf", GEHEIMER_INHALT);
        datei_anlegen(&alt, "Projekt A/gross.bin", &vec![42u8; 3 * BLOCK + 17]);
        let ziel = o.pfad("s.a3ksicherung");
        komplett_schreiben(&ziel, &tresor_inhalt, &alt, &schluessel, &salt).unwrap();
        (ziel, tresor_inhalt)
    }

    /// Ordnerbaum als sortierte Liste (zum Vergleich "nichts veraendert").
    fn dateiliste(wurzel: &Path) -> Vec<String> {
        dateien_sammeln(wurzel).0.into_iter().map(|(n, _)| n).collect()
    }

    /// Sicherung mit frei waehlbaren Eintraegen (wie sie ein Angreifer baut).
    fn sicherung_von_hand(ziel: &Path, eintraege: &[(&str, &[u8])]) {
        let (schluessel, salt, _) = zugang();
        let (mut s, _) = schreiber_oeffnen(ziel, &schluessel, &salt).unwrap();
        for (name, inhalt) in eintraege {
            eintrag_schreiben(&mut s, name, inhalt.len() as u64, &mut &**inhalt).unwrap();
        }
        s.write_all(&0u16.to_le_bytes()).unwrap();
        s.abschliessen().unwrap().into_inner().unwrap();
    }

    fn tresor_eintrag() -> Vec<u8> {
        let mut t = tresor::MAGIC.to_vec();
        t.push(1);
        t
    }

    #[test]
    fn hin_und_zurueck_stellt_alles_wieder_her() {
        let o = Ordner::neu();
        let (schluessel, salt, tresor_inhalt) = zugang();
        let alt = o.pfad("alt");
        // Dateigroessen rund um die Blockgrenze sind die heiklen Faelle.
        let groessen = [0, 1, BLOCK - 1, BLOCK, BLOCK + 1, 2 * BLOCK, 3 * BLOCK + 5];
        for g in groessen {
            datei_anlegen(&alt, &format!("Projekt Ä/Förderung/{g}.bin"), &vec![(g % 251) as u8; g]);
        }
        datei_anlegen(&alt, "Projekt Ä/Belege/beleg.pdf", GEHEIMER_INHALT);

        let ziel = o.pfad("s.a3ksicherung");
        let b = komplett_schreiben(&ziel, &tresor_inhalt, &alt, &schluessel, &salt).unwrap();
        assert_eq!(b.dateien, groessen.len() as u64 + 1);
        assert!(b.probleme.is_empty());
        assert!(!o.pfad("s.a3ksicherung.teil").exists(), "Teil-Datei muss weg sein");
        assert_eq!(art_erkennen(&ziel).unwrap(), "komplett");

        let neu_wurzel = o.pfad("neu");
        let neu_tresor = o.pfad("tresor.enc.neu");
        let b = komplett_entpacken(&ziel, PASSWORT, &neu_tresor, &neu_wurzel).unwrap();
        assert_eq!(b.dateien, groessen.len() as u64 + 1);
        assert_eq!(b.vorhanden, 0);
        assert_eq!(fs::read(&neu_tresor).unwrap(), tresor_inhalt);
        for g in groessen {
            let inhalt = fs::read(neu_wurzel.join(format!("Projekt Ä/Förderung/{g}.bin"))).unwrap();
            assert_eq!(inhalt, vec![(g % 251) as u8; g], "Groesse {g}");
        }
        assert_eq!(fs::read(neu_wurzel.join("Projekt Ä/Belege/beleg.pdf")).unwrap(), GEHEIMER_INHALT);
    }

    #[test]
    fn sicherung_enthaelt_nichts_lesbares() {
        let o = Ordner::neu();
        let (ziel, _) = sicherung_bauen(&o);
        let roh = fs::read(ziel).unwrap();
        let enthaelt = |nadel: &[u8]| roh.windows(nadel.len()).any(|f| f == nadel);
        assert!(!enthaelt(GEHEIMER_INHALT), "Beleg-Inhalt darf nicht lesbar sein");
        assert!(!enthaelt(b"Projekt A"), "Auch Dateinamen duerfen nicht lesbar sein");
        assert!(!enthaelt(b"-tresor-testinhalt"));
    }

    #[test]
    fn falsches_passwort_schreibt_nichts() {
        let o = Ordner::neu();
        let (ziel, _) = sicherung_bauen(&o);
        let neu = o.pfad("neu");
        let neu_tresor = o.pfad("tresor.enc.neu");
        let fehler = komplett_entpacken(&ziel, "falsch-geraten", &neu_tresor, &neu).unwrap_err();
        assert_eq!(fehler, "falsches_passwort");
        assert!(!neu.exists() && !neu_tresor.exists());
    }

    #[test]
    fn veraenderte_sicherung_schreibt_nichts() {
        let o = Ordner::neu();
        let (ziel, _) = sicherung_bauen(&o);
        let original = fs::read(&ziel).unwrap();
        let neu = o.pfad("neu");
        let neu_tresor = o.pfad("tresor.enc.neu");
        let boese = o.pfad("boese.a3ksicherung");

        // Ein Bit im letzten Block, im ersten Block und im Kopf (alle Felder).
        let mut stellen = vec![original.len() - 5, KOPF_LAENGE + 3];
        stellen.extend([9, SALT_ENDE, HUELLE_NONCE_ENDE, VERPACKT_ENDE]);
        for stelle in stellen {
            let mut kaputt = original.clone();
            kaputt[stelle] ^= 1;
            fs::write(&boese, &kaputt).unwrap();
            assert!(komplett_entpacken(&boese, PASSWORT, &neu_tresor, &neu).is_err(), "Stelle {stelle}");
            assert!(!neu.exists() && !neu_tresor.exists(), "Stelle {stelle}");
        }

        // Zwei Bloecke vertauscht.
        let mut vertauscht = original.clone();
        let a = KOPF_LAENGE;
        let b = KOPF_LAENGE + BLOCK_VERSCHLUESSELT;
        for i in 0..BLOCK_VERSCHLUESSELT {
            vertauscht.swap(a + i, b + i);
        }
        fs::write(&boese, &vertauscht).unwrap();
        assert!(komplett_entpacken(&boese, PASSWORT, &neu_tresor, &neu).is_err());
        assert!(!neu.exists() && !neu_tresor.exists());
    }

    #[test]
    fn abgeschnittene_sicherung_schreibt_nichts() {
        let o = Ordner::neu();
        let (ziel, _) = sicherung_bauen(&o);
        let original = fs::read(&ziel).unwrap();
        let neu = o.pfad("neu");
        let neu_tresor = o.pfad("tresor.enc.neu");
        let boese = o.pfad("kurz.a3ksicherung");

        // Mitten im Block, genau auf der Blockgrenze und nur der Kopf.
        for laenge in [original.len() - 1, KOPF_LAENGE + BLOCK_VERSCHLUESSELT, KOPF_LAENGE] {
            fs::write(&boese, &original[..laenge]).unwrap();
            assert!(komplett_entpacken(&boese, PASSWORT, &neu_tresor, &neu).is_err(), "Laenge {laenge}");
            assert!(!neu.exists() && !neu_tresor.exists(), "Laenge {laenge}");
        }
    }

    #[test]
    fn pfad_tricks_werden_abgewiesen() {
        for boese in [
            "../boese.txt",
            "a/../../boese.txt",
            "..\\boese.txt",
            "a\\..\\..\\boese.txt",
            "/etc/passwd",
            "\\\\server\\freigabe\\boese.txt",
            "\\boese.txt",
            "a//b",
            "./a",
            "a/.",
            "",
        ] {
            assert!(relativer_pfad(boese).is_err(), "{boese:?} muss abgewiesen werden");
        }
        // Auf Windows gefaehrlich (Laufwerk, Datenstrom, Geraetename), aber
        // kein Ausbruchsversuch im Namen: wird nie geschrieben.
        for unnuetz in [
            "C:/Windows/boese.txt",
            "C:boese.txt",
            "beleg.pdf:versteckt",
            "CON",
            "CON.txt",
            "con .txt",
            "a/NUL",
            "punkt.",
            "leer ",
            "a<b",
        ] {
            assert!(relativer_pfad(unnuetz).unwrap().is_none(), "{unnuetz:?} darf nie geschrieben werden");
        }
        assert_eq!(
            relativer_pfad("Projekt Ä/Belege/Müller (2).pdf").unwrap(),
            Some(Path::new("Projekt Ä").join("Belege").join("Müller (2).pdf"))
        );
    }

    #[test]
    fn manipulierte_sicherung_kann_nichts_ausserhalb_schreiben() {
        let o = Ordner::neu();
        let neu = o.pfad("tief/neu");
        let neu_tresor = o.pfad("tresor.enc.neu");
        let t = tresor_eintrag();

        // Ausbruchsversuch: Alles wird abgewiesen, auch der harmlose Teil.
        for boese in ["dateien/../boese.txt", "dateien/..\\boese.txt", "../boese.txt", "boese.txt"] {
            let s = o.pfad("slip.a3ksicherung");
            sicherung_von_hand(&s, &[("tresor.enc", &t), ("dateien/gut.txt", b"gut"), (boese, b"boese")]);
            let fehler = komplett_entpacken(&s, PASSWORT, &neu_tresor, &neu).unwrap_err();
            assert_eq!(fehler, UNZULAESSIG, "{boese}");
            assert!(!neu.exists() && !neu_tresor.exists(), "{boese}");
            assert!(!o.pfad("tief/boese.txt").exists() && !o.pfad("boese.txt").exists());
        }

        // Unbrauchbarer Name: wird uebersprungen und gemeldet, Rest kommt an.
        let s = o.pfad("name.a3ksicherung");
        sicherung_von_hand(
            &s,
            &[("tresor.enc", &t), ("dateien/a:b", b"strom"), ("dateien/C:/x.txt", b"x"), ("dateien/gut.txt", b"gut")],
        );
        let b = komplett_entpacken(&s, PASSWORT, &neu_tresor, &neu).unwrap();
        assert_eq!(b.dateien, 1);
        assert_eq!(b.probleme, ["a:b", "C:/x.txt"]);
        assert_eq!(dateiliste(&neu), ["gut.txt"]);
    }

    #[test]
    fn sicherung_ohne_gueltigen_tresor_wird_abgewiesen() {
        let o = Ordner::neu();
        let neu = o.pfad("neu");
        let neu_tresor = o.pfad("tresor.enc.neu");
        let s = o.pfad("x.a3ksicherung");
        let t = tresor_eintrag();

        let faelle: [&[(&str, &[u8])]; 4] = [
            &[],
            &[("dateien/a.txt", b"a")],
            &[("tresor.enc", b"kein tresor, nur text")],
            &[("tresor.enc", &t), ("tresor.enc", &t)],
        ];
        for eintraege in faelle {
            sicherung_von_hand(&s, eintraege);
            assert!(komplett_entpacken(&s, PASSWORT, &neu_tresor, &neu).is_err());
            assert!(!neu.exists() && !neu_tresor.exists());
        }
    }

    #[test]
    fn vorhandene_dateien_bleiben_unberuehrt() {
        let o = Ordner::neu();
        let (ziel, _) = sicherung_bauen(&o);
        let neu = o.pfad("neu");
        datei_anlegen(&neu, "Projekt A/Belege/beleg.pdf", b"meine neuere Fassung");

        let b = komplett_entpacken(&ziel, PASSWORT, &o.pfad("t.neu"), &neu).unwrap();
        assert_eq!(b.vorhanden, 1);
        assert_eq!(b.dateien, 1);
        assert_eq!(fs::read(neu.join("Projekt A/Belege/beleg.pdf")).unwrap(), b"meine neuere Fassung");
        assert_eq!(fs::read(neu.join("Projekt A/gross.bin")).unwrap().len(), 3 * BLOCK + 17);
    }

    #[test]
    fn eigene_sicherungen_und_sperrdateien_werden_nicht_mitgesichert() {
        let o = Ordner::neu();
        let (schluessel, salt, tresor_inhalt) = zugang();
        let alt = o.pfad("alt");
        datei_anlegen(&alt, "alte.a3ksicherung", b"alt");
        datei_anlegen(&alt, "halb.a3ksicherung.teil", b"halb");
        datei_anlegen(&alt, "Projekt/~$antrag.docx", b"sperre");
        datei_anlegen(&alt, "Projekt/antrag.docx", b"word");

        // Auch wenn die neue Sicherung selbst in diesem Ordner landet.
        let ziel = alt.join("neu.a3ksicherung");
        let b = komplett_schreiben(&ziel, &tresor_inhalt, &alt, &schluessel, &salt).unwrap();
        assert_eq!(b.dateien, 1);
        let neu = o.pfad("neu");
        komplett_entpacken(&ziel, PASSWORT, &o.pfad("t.neu"), &neu).unwrap();
        assert_eq!(dateiliste(&neu), ["Projekt/antrag.docx"]);
    }

    #[test]
    fn fehlender_dateien_ordner_ist_kein_fehler() {
        let o = Ordner::neu();
        let (schluessel, salt, tresor_inhalt) = zugang();
        let ziel = o.pfad("s.a3ksicherung");
        let b = komplett_schreiben(&ziel, &tresor_inhalt, &o.pfad("gibt-es-nicht"), &schluessel, &salt).unwrap();
        assert_eq!(b.dateien, 0);
        let b = komplett_entpacken(&ziel, PASSWORT, &o.pfad("t.neu"), &o.pfad("neu")).unwrap();
        assert_eq!(b.dateien, 0);
        assert_eq!(fs::read(o.pfad("t.neu")).unwrap(), tresor_inhalt);
    }

    #[test]
    fn tresor_tausch_legt_den_alten_beiseite() {
        let o = Ordner::neu();
        let ziel = o.pfad("tresor.enc");
        let neu = o.pfad("tresor.enc.neu");
        fs::write(&ziel, b"alt").unwrap();
        fs::write(&neu, b"neu").unwrap();
        tresor_austauschen(&neu, &ziel).unwrap();
        assert_eq!(fs::read(&ziel).unwrap(), b"neu");
        assert_eq!(fs::read(o.pfad("tresor.enc.vor-wiederherstellung")).unwrap(), b"alt");
        assert!(!neu.exists());

        // Neuer Computer: kein alter Tresor vorhanden.
        let ziel2 = o.pfad("zwei/tresor.enc");
        fs::create_dir_all(ziel2.parent().unwrap()).unwrap();
        let neu2 = o.pfad("zwei/tresor.enc.neu");
        fs::write(&neu2, b"neu").unwrap();
        tresor_austauschen(&neu2, &ziel2).unwrap();
        assert_eq!(fs::read(&ziel2).unwrap(), b"neu");
    }

    #[test]
    fn art_erkennen_unterscheidet_alt_neu_und_fremd() {
        let o = Ordner::neu();
        let alt = o.pfad("alt.enc");
        let mut inhalt = tresor::MAGIC.to_vec();
        inhalt.extend_from_slice(&[1; 40]);
        fs::write(&alt, inhalt).unwrap();
        assert_eq!(art_erkennen(&alt).unwrap(), "tresor");

        let fremd = o.pfad("fremd.a3ksicherung");
        fs::write(&fremd, b"irgendeine andere datei").unwrap();
        assert!(art_erkennen(&fremd).is_err());
        assert!(art_erkennen(&o.pfad("gibt-es-nicht")).is_err());
    }
}
