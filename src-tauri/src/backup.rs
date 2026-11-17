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
// mit "Zip-Bomben"). Erst kommen die Tresore, danach
// "dateien/<Pfad relativ zu Dokumente\Antrag 3000>".
//
//  - Version 2 (ab 0.6, Organisationen): ein oder mehrere Eintraege
//    "tresor/<ID>.enc" – eine Organisation ("diese") oder alle auf dem Geraet.
//    Bei "diese" kommen nur die Dateien aus ihrem eigenen Ordner mit.
//  - Version 1 (bis 0.5): genau ein Eintrag "tresor.enc" ohne ID. Wird weiter
//    gelesen; 0.5 meldet bei Version 2 "neuere Programmversion".
//
// Wiederherstellen laeuft in zwei Durchgaengen: erst wird die ganze Sicherung
// geprueft (Siegel + jeder Dateipfad), dann erst wird etwas geschrieben. Eine
// beschaedigte oder manipulierte Sicherung veraendert also nichts. Vorhandene
// Dateien werden nie ueberschrieben, die Tresore werden als Letztes getauscht.
//
// Welche Organisation ersetzt wird, entscheidet der Nutzer im Dialog: Die
// erste Organisation hat auf JEDEM Geraet dieselbe ID. Eine Sicherung vom
// alten Computer duerfte also nie stillschweigend eine fremde Organisation
// ersetzen. Ohne ausdrueckliches "ersetzen" bleibt eine vorhandene unberuehrt.
// ============================================================

use std::collections::{BTreeMap, HashMap, HashSet};
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

pub(crate) const MAGIC_KOMPLETT: &[u8; 8] = b"ANTRAG3S";
/// Eine weitergegebene Organisation (weitergabe.rs): derselbe Behaelter, aber
/// eigene Kennung – so wird keine Datei je fuer die andere Art gehalten.
pub(crate) const MAGIC_WEITERGABE: &[u8; 8] = b"ANTRAG3W";
pub(crate) const ENDUNG_WEITERGABE: &str = ".a3korganisation";
/// Version 2: Tresore mit ID ("tresor/<ID>.enc"), eine oder alle Organisationen.
const VERSION: u8 = 2;
/// Version 1 (bis 0.5): genau ein "tresor.enc" ohne ID – wird weiter gelesen.
const VERSION_1: u8 = 1;
pub(crate) const SALT_ENDE: usize = 9 + tresor::SALT_LAENGE;
const HUELLE_NONCE_ENDE: usize = SALT_ENDE + 12;
const VERPACKT_ENDE: usize = HUELLE_NONCE_ENDE + 32 + 16;
pub(crate) const KOPF_LAENGE: usize = VERPACKT_ENDE + 7;
const BLOCK: usize = 64 * 1024;
const BLOCK_VERSCHLUESSELT: usize = BLOCK + 16;
pub(crate) const NAME_MAX: usize = 4096;

const EINTRAG_TRESOR: &str = "tresor.enc";
const PRAEFIX_TRESORE: &str = "tresor/";
pub(crate) const PRAEFIX_DATEIEN: &str = "dateien/";
/// Obergrenze fuer einen Tresor in der Sicherung (er wird im Speicher gehalten).
pub(crate) const TRESOR_MAX: u64 = 256 * 1024 * 1024;
const ENDUNG_KOMPLETT: &str = ".a3ksicherung";

const KEINE_SICHERUNG: &str = "Das ist keine gültige Antrag-3000-Sicherung.";
pub(crate) const BESCHAEDIGT: &str = "Die Sicherung ist beschädigt oder wurde verändert. Es wurde nichts eingespielt.";
pub(crate) const UNZULAESSIG: &str = "Die Sicherung enthält unzulässige Dateipfade. Es wurde nichts eingespielt.";
const IST_WEITERGABE: &str =
    "Das ist eine weitergegebene Organisation, keine Sicherung. Du fügst sie über das Organisations-Menü hinzu: „Organisation aus Datei hinzufügen“.";
const IST_SICHERUNG: &str =
    "Das ist eine Sicherung, keine weitergegebene Organisation. Du spielst sie unter „Sicherung“ ein.";

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
    /// Organisationen in der Sicherung bzw. eingespielte Organisationen.
    pub organisationen: u64,
}

/// Entfernt eine halb fertige Datei, falls wir vorzeitig aussteigen.
/// Nach einem erfolgreichen Umbenennen ist nichts mehr da – dann passiert nichts.
pub(crate) struct Aufraeumen(pub(crate) PathBuf);

/// "<pfad>.teil": dort entsteht eine Datei, bis sie komplett und gegengelesen ist.
pub(crate) fn teil_pfad(ziel: &Path) -> PathBuf {
    let mut teil = ziel.as_os_str().to_owned();
    teil.push(".teil");
    PathBuf::from(teil)
}

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
    magic: &[u8; 8],
    version: u8,
) -> Result<[u8; KOPF_LAENGE], String> {
    let mut kopf = [0u8; KOPF_LAENGE];
    kopf[..8].copy_from_slice(magic);
    kopf[8] = version;
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
pub(crate) fn datenschluessel_entpacken(
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

/// Liest den Kopf und prueft, dass die Datei von der erwarteten Art ist
/// (`magic`): Sicherung (Version 1 oder 2) oder Weitergabe (Version 1).
pub(crate) fn kopf_lesen(quelle: &Path, magic: &[u8; 8]) -> Result<(BufReader<File>, [u8; KOPF_LAENGE]), String> {
    let datei = File::open(quelle).map_err(|e| format!("Datei nicht lesbar: {e}"))?;
    let mut leser = BufReader::new(datei);
    let mut kopf = [0u8; KOPF_LAENGE];
    leser.read_exact(&mut kopf).map_err(|_| KEINE_SICHERUNG.to_string())?;
    if &kopf[..8] != magic {
        let andere_art = if &kopf[..8] == MAGIC_WEITERGABE {
            IST_WEITERGABE
        } else if &kopf[..8] == MAGIC_KOMPLETT {
            IST_SICHERUNG
        } else {
            KEINE_SICHERUNG
        };
        return Err(andere_art.into());
    }
    let bekannt = if magic == MAGIC_KOMPLETT { kopf[8] == VERSION || kopf[8] == VERSION_1 } else { kopf[8] == 1 };
    if !bekannt {
        return Err("Diese Datei stammt aus einer neueren Programmversion. Bitte aktualisiere Antrag 3000.".into());
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
pub(crate) struct BlockSchreiber<W: Write> {
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
    pub(crate) fn abschliessen(mut self) -> io::Result<W> {
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
pub(crate) struct BlockLeser<R: Read> {
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
pub(crate) fn schreiber_oeffnen(
    pfad: &Path,
    schluessel: &[u8; 32],
    salt: &[u8; tresor::SALT_LAENGE],
    magic: &[u8; 8],
    version: u8,
) -> Result<(BlockSchreiber<BufWriter<File>>, Zeroizing<[u8; 32]>), String> {
    let mut datenschluessel = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(&mut datenschluessel[..]);
    let kopf = kopf_bauen(&datenschluessel, schluessel, salt, magic, version)?;

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

pub(crate) fn leser_oeffnen(
    quelle: &Path,
    datenschluessel: &[u8; 32],
    magic: &[u8; 8],
) -> Result<BlockLeser<BufReader<File>>, String> {
    let (leser, kopf) = kopf_lesen(quelle, magic)?;
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

pub(crate) fn eintrag_schreiben(
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

pub(crate) fn lesefehler(e: io::Error) -> String {
    match e.kind() {
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => BESCHAEDIGT.to_string(),
        _ => format!("Sicherungsdatei nicht lesbar: {e}"),
    }
}

/// Geht alle Eintraege durch. Was der Besucher nicht liest, wird trotzdem
/// entschluesselt – so prueft jeder Durchgang das Siegel der ganzen Datei.
pub(crate) fn eintraege_durchlaufen<R: Read>(
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
pub(crate) fn relativer_pfad(name: &str) -> Result<Option<PathBuf>, String> {
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

/// Ist `relativ` die Marke eines Organisations-Ordners ("<Ordner>/<MARKE>")?
/// Dann der Ordnername. Marken zaehlen nicht als Dateien des Nutzers.
fn marken_ordner(relativ: &str) -> Option<&str> {
    let (oben, rest) = relativ.split_once('/')?;
    (rest == ordner::MARKE).then_some(oben)
}

/// Was der Pruefdurchgang ueber eine Sicherung weiss.
#[derive(Debug, Default)]
struct Inhalt {
    version: u8,
    /// Die Tresore in der Reihenfolge der Sicherung: ID (None in Version 1)
    /// und der verschluesselte Inhalt. Er stammt aus dem geprueften Strom.
    tresore: Vec<(Option<String>, Vec<u8>)>,
    /// Datei-Eintraege ohne Ordner-Marken.
    dateien: u64,
    /// Ordner der obersten Ebene mit Marke -> die ID darin (falls gueltig).
    marken: BTreeMap<String, Option<String>>,
}

impl Inhalt {
    fn ids(&self) -> Vec<String> {
        self.tresore.iter().filter_map(|(id, _)| id.clone()).collect()
    }
}

/// Erster Durchgang: prueft die ganze Datei, ohne etwas zu schreiben.
fn inhalt_pruefen(quelle: &Path, datenschluessel: &[u8; 32]) -> Result<Inhalt, String> {
    let mut leser = leser_oeffnen(quelle, datenschluessel, MAGIC_KOMPLETT)?;
    let version = leser.kopf[8];
    let mut inhalt = Inhalt { version, ..Inhalt::default() };
    let mut dateien_begonnen = false;
    eintraege_durchlaufen(&mut leser, |name, daten| {
        if let Some(relativ) = name.strip_prefix(PRAEFIX_DATEIEN) {
            // Erst die Tresore, dann die Dateien.
            if inhalt.tresore.is_empty() {
                return Err(BESCHAEDIGT.into());
            }
            dateien_begonnen = true;
            let brauchbar = relativer_pfad(relativ)?.is_some();
            match marken_ordner(relativ) {
                Some(oben) if brauchbar => {
                    let mut text = Vec::new();
                    daten.take(4096).read_to_end(&mut text).map_err(lesefehler)?;
                    inhalt.marken.insert(oben.to_string(), ordner::marke_id(&String::from_utf8_lossy(&text)));
                }
                Some(_) => {}
                None => inhalt.dateien += 1,
            }
            return Ok(());
        }
        let id = match version {
            VERSION_1 if name == EINTRAG_TRESOR => {
                if !inhalt.tresore.is_empty() {
                    return Err(BESCHAEDIGT.into());
                }
                None
            }
            VERSION if name.starts_with(PRAEFIX_TRESORE) => {
                let id = name[PRAEFIX_TRESORE.len()..]
                    .strip_suffix(".enc")
                    .filter(|id| tresor::id_gueltig(id))
                    .ok_or(UNZULAESSIG.to_string())?;
                if inhalt.tresore.iter().any(|(i, _)| i.as_deref() == Some(id)) {
                    return Err(BESCHAEDIGT.into());
                }
                Some(id.to_string())
            }
            _ => return Err(UNZULAESSIG.into()),
        };
        if dateien_begonnen {
            return Err(BESCHAEDIGT.into());
        }
        let mut tresor_inhalt = Vec::new();
        daten.take(TRESOR_MAX + 1).read_to_end(&mut tresor_inhalt).map_err(lesefehler)?;
        if tresor_inhalt.len() as u64 > TRESOR_MAX || tresor_inhalt.len() < 9 || &tresor_inhalt[..8] != tresor::MAGIC {
            return Err(BESCHAEDIGT.into());
        }
        inhalt.tresore.push((id, tresor_inhalt));
        Ok(())
    })?;
    if inhalt.tresore.is_empty() {
        return Err(BESCHAEDIGT.into());
    }
    Ok(inhalt)
}

// ============================================================
// Sicherung erstellen
// ============================================================

/// Eigene Sicherungen nicht mitsichern (sie koennen im selben Ordner liegen),
/// die kurzlebigen Sperrdateien von Word ("~$...") auch nicht.
fn gehoert_nicht_in_sicherung(name: &str) -> bool {
    let name = name.to_lowercase();
    let name = name.strip_suffix(".teil").unwrap_or(&name);
    name.starts_with("~$") || name.ends_with(ENDUNG_KOMPLETT) || name.ends_with(ENDUNG_WEITERGABE)
}

/// Sammelt alle normalen Dateien unter `start`; ihre Namen in der Sicherung
/// beginnen mit `praefix` (relativ zu Dokumente\Antrag 3000). Verknuepfungen
/// werden bewusst nicht verfolgt: Sie koennten aus dem Ordner hinausfuehren,
/// und was dort liegt, gehoert nicht in diese Sicherung. Sie werden gemeldet.
/// Mit `fremde_auslassen` bleiben Organisations-Ordner (mit Marke) draussen.
pub(crate) fn dateien_sammeln(start: &Path, praefix: &str, fremde_auslassen: bool) -> (Vec<(String, PathBuf)>, Vec<String>) {
    let mut dateien = Vec::new();
    let mut probleme = Vec::new();
    let mut offen = vec![(start.to_path_buf(), praefix.to_string())];
    while let Some((ordner_pfad, praefix)) = offen.pop() {
        let eintraege = match fs::read_dir(&ordner_pfad) {
            Ok(e) => e,
            // Gibt es den Hauptordner noch nicht, ist schlicht nichts zu sichern.
            Err(_) if ordner_pfad == start => continue,
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
                Ok(art) if art.is_dir() => {
                    if !(fremde_auslassen && eintrag.path().join(ordner::MARKE).exists()) {
                        offen.push((eintrag.path(), format!("{relativ}/")));
                    }
                }
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

/// Welche Dateien zu EINER Organisation gehoeren: ihr eigener Ordner. Solange
/// ihr Umzug noch aussteht (`org_ordner` leer), die Wurzel ohne die Ordner
/// anderer Organisationen. Der Name im Tresor ist nur ein Hinweis – traegt der
/// Ordner die Marke einer ANDEREN Organisation, wird abgebrochen, statt deren
/// Dateien einzupacken (beim naechsten Anmelden wird neu zugeordnet).
fn org_dateien(wurzel: &Path, id: &str, org_ordner: &str) -> Result<(Vec<(String, PathBuf)>, Vec<String>), String> {
    if org_ordner.is_empty() {
        return Ok(dateien_sammeln(wurzel, "", true));
    }
    let name = ordner::bereinigen(org_ordner)?;
    if let ordner::Besitz::Id(besitzer) = ordner::besitz(&wurzel.join(&name)) {
        if besitzer != id {
            return Err(FREMDER_ORDNER.into());
        }
    }
    Ok(dateien_sammeln(&wurzel.join(&name), &format!("{name}/"), false))
}

const FREMDER_ORDNER: &str =
    "Der Ordner dieser Organisation ist gerade nicht eindeutig zugeordnet. Bitte einmal sperren, neu anmelden und dann erneut sichern.";

/// Schreibt die Sicherung erst unter ".teil" und benennt sie erst um, wenn
/// sie komplett und gegengelesen ist. So bleibt z. B. bei vollem USB-Stick
/// keine kaputte Datei liegen, die man spaeter fuer eine Sicherung haelt.
fn komplett_schreiben(
    ziel: &Path,
    tresore: &[(String, Vec<u8>)],
    dateien: Vec<(String, PathBuf)>,
    probleme: Vec<String>,
    schluessel: &[u8; 32],
    salt: &[u8; tresor::SALT_LAENGE],
) -> Result<Bericht, String> {
    let mut bericht = Bericht { probleme, organisationen: tresore.len() as u64, ..Bericht::default() };

    let teil = teil_pfad(ziel);
    let _aufraeumen = Aufraeumen(teil.clone());
    let (mut schreiber, datenschluessel) = schreiber_oeffnen(&teil, schluessel, salt, MAGIC_KOMPLETT, VERSION)?;

    let fehlgeschlagen = |e: io::Error| format!("Sicherung fehlgeschlagen: {e}");
    for (id, inhalt) in tresore {
        eintrag_schreiben(&mut schreiber, &format!("{PRAEFIX_TRESORE}{id}.enc"), inhalt.len() as u64, &mut &**inhalt)
            .map_err(fehlgeschlagen)?;
    }
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
        if marken_ordner(&relativ).is_none() {
            bericht.dateien += 1;
            bericht.bytes += groesse;
        }
    }
    schreiber.write_all(&0u16.to_le_bytes()).map_err(fehlgeschlagen)?;
    {
        let puffer = schreiber.abschliessen().map_err(fehlgeschlagen)?;
        let datei = puffer.into_inner().map_err(|e| fehlgeschlagen(e.into_error()))?;
        datei.sync_all().map_err(fehlgeschlagen)?;
    }

    // Gegenlesen: Eine Sicherung ist erst etwas wert, wenn sie sich oeffnen laesst.
    let gelesen = inhalt_pruefen(&teil, &datenschluessel)?;
    let ids: Vec<String> = tresore.iter().map(|(id, _)| id.clone()).collect();
    if gelesen.dateien != bericht.dateien || gelesen.ids() != ids {
        return Err("Die Kontrolle der Sicherung ist fehlgeschlagen.".into());
    }
    fs::rename(&teil, ziel).map_err(|e| format!("Sicherung nicht speicherbar: {e}"))?;
    Ok(bericht)
}

// ============================================================
// Sicherung einspielen
// ============================================================

/// Schreibt eine Datei, ohne je etwas zu ueberschreiben (create_new).
pub(crate) fn datei_einspielen(wurzel: &Path, relativ: &Path, inhalt: &mut dyn Read) -> io::Result<()> {
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

/// Wohin die Tresore einer Sicherung kommen – das Ergebnis der Wahl im Dialog.
#[derive(Debug, Default)]
struct Einspielplan {
    /// Version 1 ("tresor.enc" ohne ID): Zielpfad des Tresors.
    einzel: Option<PathBuf>,
    /// Version 2: Sicherungs-ID -> (Ziel-ID, Zielpfad). Bei "als Kopie" ist die
    /// Ziel-ID neu. Fehlt eine ID, wird diese Organisation samt ihrem Ordner
    /// weggelassen.
    orgs: HashMap<String, (String, PathBuf)>,
}

/// Neben dem Ziel: "<ziel>.neu" (wird erst ganz am Ende eingetauscht).
fn neu_pfad(ziel: &Path) -> PathBuf {
    let mut s = ziel.as_os_str().to_owned();
    s.push(".neu");
    PathBuf::from(s)
}

/// Aus der Wahl im Dialog (ID -> "ersetzen" | "kopie" | "weglassen") den Plan
/// bauen. Eine VORHANDENE Organisation wird nur ersetzt, wenn ausdruecklich
/// "ersetzen" gewaehlt ist – sonst bleibt sie unberuehrt (Grund: oben). Eine
/// neue kommt dazu, ausser sie ist abgewaehlt.
fn plan_bauen(ids: &[String], orgs: &Path, wahl: &HashMap<String, String>) -> Result<Einspielplan, String> {
    let mut plan = Einspielplan::default();
    let mut vergeben: HashSet<String> = HashSet::new();
    for id in ids {
        let vorhanden = orgs.join(format!("{id}.enc")).exists();
        let ziel = match (vorhanden, wahl.get(id).map(String::as_str)) {
            (true, Some("ersetzen")) => Some(id.clone()),
            (true, Some("kopie")) => Some(loop {
                let (neu, _) = tresor::neue_id_in(orgs);
                if !ids.contains(&neu) && vergeben.insert(neu.clone()) {
                    break neu;
                }
            }),
            (true, _) => None,
            (false, Some("weglassen")) => None,
            (false, _) => Some(id.clone()),
        };
        if let Some(z) = ziel {
            let pfad = orgs.join(format!("{z}.enc"));
            plan.orgs.insert(id.clone(), (z, pfad));
        }
    }
    if plan.orgs.is_empty() {
        return Err("Es ist keine Organisation zum Einspielen ausgewählt.".into());
    }
    Ok(plan)
}

/// Welche Ordner der obersten Ebene beim Einspielen anders heissen muessen
/// (Some) oder wegfallen (None). Ein Ordner aus der Sicherung landet nur dann
/// unter seinem Namen, wenn der hier frei ist oder schon derselben
/// Organisation gehoert. Sonst bekommt er einen freien Namen und die Marke der
/// Organisation – sie findet ihn beim naechsten Anmelden ueber ihre ID. So
/// mischen sich nie die Dateien zweier Organisationen.
fn ordner_planen(
    wurzel: &Path,
    inhalt: &Inhalt,
    plan: &Einspielplan,
) -> Result<HashMap<String, Option<String>>, String> {
    let in_sicherung: HashSet<String> = inhalt.ids().into_iter().collect();
    let mut umleitung = HashMap::new();
    for (oben, marke) in &inhalt.marken {
        // Marke ohne ID (aelterer Stand): unter ihrem Namen nur, wenn der Ordner
        // hier keiner Organisation gehoert – sonst unter freiem Namen.
        let Some(m) = marke else {
            if matches!(ordner::besitz(&wurzel.join(oben)), ordner::Besitz::Id(_)) {
                let neu = ordner::freier_org_name(wurzel, oben, None)?;
                ordner::marke_setzen(&wurzel.join(&neu), None)?;
                umleitung.insert(oben.clone(), Some(neu));
            }
            continue;
        };
        let ziel_id = if in_sicherung.contains(m) {
            match plan.orgs.get(m) {
                Some((z, _)) => z.clone(),
                None => {
                    umleitung.insert(oben.clone(), None);
                    continue;
                }
            }
        } else {
            m.clone()
        };
        let hier = wurzel.join(oben);
        let behalten = ziel_id == *m && (!hier.exists() || ordner::besitz(&hier) == ordner::Besitz::Id(m.clone()));
        if !behalten {
            let neu = ordner::freier_org_name(wurzel, oben, None)?;
            ordner::marke_setzen(&wurzel.join(&neu), Some(&ziel_id))?;
            umleitung.insert(oben.clone(), Some(neu));
        }
    }
    Ok(umleitung)
}

/// Prueft die Sicherung komplett und entpackt sie dann: die Dateien nach
/// `wurzel`, jeden Tresor nach "<Ziel>.neu" (noch NICHT an seinen Platz).
/// `planen` entscheidet nach dem Pruefdurchgang, welcher Tresor wohin kommt.
/// Rueckgabe: Bericht und die anstehenden Tausche (neu, Ziel).
fn komplett_entpacken(
    quelle: &Path,
    passwort: &str,
    planen: impl FnOnce(&Inhalt) -> Result<Einspielplan, String>,
    wurzel: &Path,
) -> Result<(Bericht, Vec<(PathBuf, PathBuf)>), String> {
    let (_, kopf) = kopf_lesen(quelle, MAGIC_KOMPLETT)?;
    let schluessel = Zeroizing::new(tresor::schluessel_ableiten(passwort, &kopf[9..SALT_ENDE])?);
    let datenschluessel = datenschluessel_entpacken(&kopf, &schluessel)?;

    // 1. Durchgang: Stimmt alles? Bis hierher wurde nichts geschrieben.
    let inhalt = inhalt_pruefen(quelle, &datenschluessel)?;
    let plan = planen(&inhalt)?;

    let mut tausch: Vec<(PathBuf, PathBuf)> = Vec::new();
    let ergebnis = (|| -> Result<Bericht, String> {
        // Die Tresore stammen aus dem schon geprueften Strom.
        for (id, tresor_inhalt) in &inhalt.tresore {
            let ziel = match id {
                None => plan.einzel.clone(),
                Some(id) => plan.orgs.get(id).map(|(_, pfad)| pfad.clone()),
            };
            let Some(ziel) = ziel else { continue };
            let neu = neu_pfad(&ziel);
            tausch.push((neu.clone(), ziel));
            let mut datei = File::create(&neu).map_err(|e| format!("Tresor nicht schreibbar: {e}"))?;
            datei.write_all(tresor_inhalt).map_err(|e| format!("Tresor nicht schreibbar: {e}"))?;
            datei.sync_all().map_err(|e| format!("Tresor nicht schreibbar: {e}"))?;
        }

        // 2. Durchgang: die Dateien.
        fs::create_dir_all(wurzel).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
        let umleitung = ordner_planen(wurzel, &inhalt, &plan)?;
        let mut bericht = Bericht { organisationen: tausch.len() as u64, ..Bericht::default() };
        let mut leser = leser_oeffnen(quelle, &datenschluessel, MAGIC_KOMPLETT)?;
        eintraege_durchlaufen(&mut leser, |name, inhalt| {
            let Some(relativ) = name.strip_prefix(PRAEFIX_DATEIEN) else {
                return Ok(()); // Tresor-Eintrag: oben schon erledigt
            };
            let Some(mut pfad) = relativer_pfad(relativ)? else {
                bericht.probleme.push(relativ.to_string());
                return Ok(());
            };
            if let Some((oben, rest)) = relativ.split_once('/') {
                match umleitung.get(oben) {
                    Some(None) => return Ok(()), // Organisation weggelassen
                    Some(Some(neu)) => match relativer_pfad(&format!("{neu}/{rest}"))? {
                        Some(p) => pfad = p,
                        None => {
                            bericht.probleme.push(relativ.to_string());
                            return Ok(());
                        }
                    },
                    None => {}
                }
            }
            let marke = marken_ordner(relativ).is_some();
            let mut gezaehlt = inhalt.take(u64::MAX);
            match datei_einspielen(wurzel, &pfad, &mut gezaehlt) {
                Ok(()) if marke => {}
                Ok(()) => {
                    bericht.dateien += 1;
                    bericht.bytes += u64::MAX - gezaehlt.limit();
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    if !marke {
                        bericht.vorhanden += 1;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::InvalidData => return Err(BESCHAEDIGT.into()),
                // Z. B. Pfad zu lang: Eine einzelne Datei soll nicht den ganzen
                // Umzug scheitern lassen – sie wird gemeldet.
                Err(_) => bericht.probleme.push(relativ.to_string()),
            }
            Ok(())
        })?;
        Ok(bericht)
    })();
    match ergebnis {
        Ok(bericht) => Ok((bericht, tausch)),
        Err(e) => {
            for (neu, _) in &tausch {
                let _ = fs::remove_file(neu);
            }
            Err(e)
        }
    }
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

/// Mehrere Tresore eintauschen – alles oder nichts: Scheitert einer, kommen
/// die schon getauschten zurueck an ihren Platz (wie beim Passwortwechsel).
fn tresore_austauschen(tausch: &[(PathBuf, PathBuf)]) -> Result<(), String> {
    let mut erledigt: Vec<(&Path, bool)> = Vec::new();
    for (neu, ziel) in tausch {
        let hatte_tresor = ziel.exists();
        if let Err(e) = tresor_austauschen(neu, ziel) {
            for (z, hatte) in erledigt.into_iter().rev() {
                let _ = fs::remove_file(z);
                if hatte {
                    let _ = fs::rename(z.with_extension("enc.vor-wiederherstellung"), z);
                }
            }
            return Err(e);
        }
        erledigt.push((ziel, hatte_tresor));
    }
    Ok(())
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
    } else if &anfang[..8] == MAGIC_WEITERGABE {
        Ok("organisation")
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

/// Sicherung erstellen (die Organisation `org` muss offen sein; mit ihrem
/// Passwort laesst sich die Sicherung spaeter oeffnen).
///  - alle = false: nur diese Organisation, mit Dateien nur ihr eigener Ordner
///    (`org_ordner` aus ihrem Tresor).
///  - alle = true: alle Organisationen auf dem Geraet (auch gesperrte, jede
///    mit ihrem eigenen Passwort) und mit Dateien der ganze Ordner.
/// Kann bei vielen Dateien dauern, darum ausserhalb des Hauptfadens.
#[tauri::command]
pub async fn sicherung_erstellen(
    app: tauri::AppHandle,
    org: String,
    ziel: String,
    alle: bool,
    mit_dateien: bool,
    org_ordner: String,
) -> Result<Bericht, String> {
    let (schluessel, salt, tresor_inhalt) = tresor::offener_stand(&app, &org)?;
    let tresore = if alle { tresor::alle_tresore(&app, &org, tresor_inhalt)? } else { vec![(org.clone(), tresor_inhalt)] };
    let wurzel = ordner::wurzel(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (dateien, probleme) = match (mit_dateien, alle) {
            (false, _) => (Vec::new(), Vec::new()),
            (true, true) => dateien_sammeln(&wurzel, "", false),
            (true, false) => org_dateien(&wurzel, &org, &org_ordner)?,
        };
        komplett_schreiben(Path::new(&ziel), &tresore, dateien, probleme, &schluessel, &salt)
    })
    .await
    .map_err(|e| format!("Sicherung abgebrochen: {e}"))?
}

/// Eine Organisation in der Sicherung, fuer den Dialog vor dem Einspielen.
#[derive(Serialize, Debug)]
pub struct VorschauOrg {
    pub id: String,
    /// Gibt es diese Organisation (ID) auf diesem Geraet schon?
    pub vorhanden: bool,
    /// Ihr Inhalt, wenn sich ihr Tresor mit dem eingegebenen Passwort oeffnen
    /// laesst (fuer den Namen im Dialog); sonst None (eigenes Passwort).
    pub daten: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct Vorschau {
    pub version: u8,
    pub orgs: Vec<VorschauOrg>,
    pub dateien: u64,
}

fn vorschau_bauen(quelle: &Path, passwort: &str, orgs: &Path) -> Result<Vorschau, String> {
    let (_, kopf) = kopf_lesen(quelle, MAGIC_KOMPLETT)?;
    let schluessel = Zeroizing::new(tresor::schluessel_ableiten(passwort, &kopf[9..SALT_ENDE])?);
    let datenschluessel = datenschluessel_entpacken(&kopf, &schluessel)?;
    let inhalt = inhalt_pruefen(quelle, &datenschluessel)?;
    let liste = inhalt
        .tresore
        .iter()
        .filter_map(|(id, t)| {
            id.as_ref().map(|id| VorschauOrg {
                id: id.clone(),
                vorhanden: orgs.join(format!("{id}.enc")).exists(),
                daten: tresor::mit_passwort_oeffnen(t, passwort),
            })
        })
        .collect();
    Ok(Vorschau { version: inhalt.version, orgs: liste, dateien: inhalt.dateien })
}

/// Prueft eine Sicherung mit Dateien komplett (schreibt nichts) und sagt, was
/// darin ist – damit der Nutzer vor dem Einspielen je Organisation waehlt.
#[tauri::command]
pub async fn sicherung_pruefen(app: tauri::AppHandle, quelle: String, passwort: String) -> Result<Vorschau, String> {
    let passwort = Zeroizing::new(passwort);
    let orgs = tresor::org_ordner(&app)?;
    tauri::async_runtime::spawn_blocking(move || vorschau_bauen(Path::new(&quelle), &passwort, &orgs))
        .await
        .map_err(|e| format!("Prüfung abgebrochen: {e}"))?
}

/// Sicherung mit Dateien einspielen. Braucht KEINEN offenen Tresor – so geht es
/// auch auf einem neuen Computer direkt beim ersten Start.
///  - Version 1 (ohne IDs): wie bisher, ersetzt `org` bzw. kommt neu dazu.
///  - Version 2: `wahl` je ID ("ersetzen" | "kopie" | "weglassen"), siehe
///    plan_bauen. Eine vorhandene wird nur mit "ersetzen" ersetzt.
/// Danach ist alles gesperrt; man entsperrt mit dem Passwort der Sicherung.
#[tauri::command]
pub async fn sicherung_einspielen(
    app: tauri::AppHandle,
    org: Option<String>,
    quelle: String,
    passwort: String,
    wahl: Option<HashMap<String, String>>,
) -> Result<Bericht, String> {
    let passwort = Zeroizing::new(passwort);
    let einzel = einspiel_ziel(&app, org)?;
    let orgs = tresor::org_ordner(&app)?;
    let wahl = wahl.unwrap_or_default();
    let wurzel = ordner::wurzel(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let (bericht, tausch) = komplett_entpacken(
            Path::new(&quelle),
            &passwort,
            |inhalt| {
                if inhalt.version == VERSION_1 {
                    Ok(Einspielplan { einzel: Some(einzel), ..Einspielplan::default() })
                } else {
                    plan_bauen(&inhalt.ids(), &orgs, &wahl)
                }
            },
            &wurzel,
        )?;

        // Wie in tresor_backup_einspielen: Tausch und Sperren unter dem Schloss.
        let zustand = app.state::<tresor::TresorZustand>();
        let mut geheim = zustand.geheim.lock().unwrap();
        let ergebnis = tresore_austauschen(&tausch);
        for (neu, _) in &tausch {
            let _ = fs::remove_file(neu);
        }
        ergebnis?;
        // Ein gemerkter Schluessel passt nicht zum eingespielten Tresor.
        for (_, ziel) in &tausch {
            let _ = fs::remove_file(ziel.with_extension("merken"));
        }
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
    const ID_T: &str = "aaaaaaaaaaaaaaaa";
    const T_NAME: &str = "tresor/aaaaaaaaaaaaaaaa.enc";

    /// Wie frueher: eine Organisation, alle Dateien unter `wurzel`.
    fn schreiben_test(
        ziel: &Path,
        tresor_inhalt: &[u8],
        wurzel: &Path,
        schluessel: &[u8; 32],
        salt: &[u8; tresor::SALT_LAENGE],
    ) -> Result<Bericht, String> {
        let (dateien, probleme) = dateien_sammeln(wurzel, "", false);
        komplett_schreiben(ziel, &[(ID_T.to_string(), tresor_inhalt.to_vec())], dateien, probleme, schluessel, salt)
    }

    /// Wie frueher: jeder Tresor der Sicherung nach `neu_tresor` ohne ".neu".
    fn entpacken_test(quelle: &Path, passwort: &str, neu_tresor: &Path, wurzel: &Path) -> Result<Bericht, String> {
        let ziel = PathBuf::from(neu_tresor.to_str().unwrap().strip_suffix(".neu").unwrap());
        komplett_entpacken(
            quelle,
            passwort,
            |inhalt| {
                let mut plan = Einspielplan { einzel: Some(ziel.clone()), ..Einspielplan::default() };
                for id in inhalt.ids() {
                    plan.orgs.insert(id.clone(), (id, ziel.clone()));
                }
                Ok(plan)
            },
            wurzel,
        )
        .map(|(b, _)| b)
    }

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
        schreiben_test(&ziel, &tresor_inhalt, &alt, &schluessel, &salt).unwrap();
        (ziel, tresor_inhalt)
    }

    /// Ordnerbaum als sortierte Liste (zum Vergleich "nichts veraendert").
    fn dateiliste(wurzel: &Path) -> Vec<String> {
        dateien_sammeln(wurzel, "", false).0.into_iter().map(|(n, _)| n).collect()
    }

    /// Sicherung mit frei waehlbaren Eintraegen (wie sie ein Angreifer baut).
    fn sicherung_von_hand(ziel: &Path, eintraege: &[(&str, &[u8])]) {
        sicherung_von_hand_als(ziel, eintraege, VERSION)
    }

    fn sicherung_von_hand_als(ziel: &Path, eintraege: &[(&str, &[u8])], version: u8) {
        let (schluessel, salt, _) = zugang();
        let (mut s, _) = schreiber_oeffnen(ziel, &schluessel, &salt, MAGIC_KOMPLETT, version).unwrap();
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
        let b = schreiben_test(&ziel, &tresor_inhalt, &alt, &schluessel, &salt).unwrap();
        assert_eq!(b.dateien, groessen.len() as u64 + 1);
        assert!(b.probleme.is_empty());
        assert!(!o.pfad("s.a3ksicherung.teil").exists(), "Teil-Datei muss weg sein");
        assert_eq!(art_erkennen(&ziel).unwrap(), "komplett");

        let neu_wurzel = o.pfad("neu");
        let neu_tresor = o.pfad("tresor.enc.neu");
        let b = entpacken_test(&ziel, PASSWORT, &neu_tresor, &neu_wurzel).unwrap();
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
        let fehler = entpacken_test(&ziel, "falsch-geraten", &neu_tresor, &neu).unwrap_err();
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
            assert!(entpacken_test(&boese, PASSWORT, &neu_tresor, &neu).is_err(), "Stelle {stelle}");
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
        assert!(entpacken_test(&boese, PASSWORT, &neu_tresor, &neu).is_err());
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
            assert!(entpacken_test(&boese, PASSWORT, &neu_tresor, &neu).is_err(), "Laenge {laenge}");
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
            sicherung_von_hand(&s, &[(T_NAME, &t), ("dateien/gut.txt", b"gut"), (boese, b"boese")]);
            let fehler = entpacken_test(&s, PASSWORT, &neu_tresor, &neu).unwrap_err();
            assert_eq!(fehler, UNZULAESSIG, "{boese}");
            assert!(!neu.exists() && !neu_tresor.exists(), "{boese}");
            assert!(!o.pfad("tief/boese.txt").exists() && !o.pfad("boese.txt").exists());
        }

        // Unbrauchbarer Name: wird uebersprungen und gemeldet, Rest kommt an.
        let s = o.pfad("name.a3ksicherung");
        sicherung_von_hand(
            &s,
            &[(T_NAME, &t), ("dateien/a:b", b"strom"), ("dateien/C:/x.txt", b"x"), ("dateien/gut.txt", b"gut")],
        );
        let b = entpacken_test(&s, PASSWORT, &neu_tresor, &neu).unwrap();
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
            &[(T_NAME, b"kein tresor, nur text")],
            &[(T_NAME, &t), (T_NAME, &t)],
        ];
        for eintraege in faelle {
            sicherung_von_hand(&s, eintraege);
            assert!(entpacken_test(&s, PASSWORT, &neu_tresor, &neu).is_err());
            assert!(!neu.exists() && !neu_tresor.exists());
        }
    }

    #[test]
    fn vorhandene_dateien_bleiben_unberuehrt() {
        let o = Ordner::neu();
        let (ziel, _) = sicherung_bauen(&o);
        let neu = o.pfad("neu");
        datei_anlegen(&neu, "Projekt A/Belege/beleg.pdf", b"meine neuere Fassung");

        let b = entpacken_test(&ziel, PASSWORT, &o.pfad("t.neu"), &neu).unwrap();
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
        let b = schreiben_test(&ziel, &tresor_inhalt, &alt, &schluessel, &salt).unwrap();
        assert_eq!(b.dateien, 1);
        let neu = o.pfad("neu");
        entpacken_test(&ziel, PASSWORT, &o.pfad("t.neu"), &neu).unwrap();
        assert_eq!(dateiliste(&neu), ["Projekt/antrag.docx"]);
    }

    #[test]
    fn fehlender_dateien_ordner_ist_kein_fehler() {
        let o = Ordner::neu();
        let (schluessel, salt, tresor_inhalt) = zugang();
        let ziel = o.pfad("s.a3ksicherung");
        let b = schreiben_test(&ziel, &tresor_inhalt, &o.pfad("gibt-es-nicht"), &schluessel, &salt).unwrap();
        assert_eq!(b.dateien, 0);
        let b = entpacken_test(&ziel, PASSWORT, &o.pfad("t.neu"), &o.pfad("neu")).unwrap();
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

    // ---------------- Organisationen (Version 2) ----------------

    const ID_A: &str = "aaaaaaaaaaaaaaaa";
    const ID_B: &str = "bbbbbbbbbbbbbbbb";
    const ID_X: &str = "cccccccccccccccc";

    fn marke(wurzel: &Path, ordner_name: &str, id: &str) {
        ordner::marke_setzen(&wurzel.join(ordner_name), Some(id)).unwrap();
    }

    #[test]
    fn eine_organisation_sichert_nur_ihren_eigenen_ordner() {
        let o = Ordner::neu();
        let w = o.pfad("w");
        marke(&w, "Verein", ID_A);
        datei_anlegen(&w, "Verein/Fest/beleg.pdf", b"A");
        marke(&w, "Duo", ID_B);
        datei_anlegen(&w, "Duo/Tour/geheim.pdf", b"B");
        datei_anlegen(&w, "Altprojekt/alt.pdf", b"alt");

        let (d, _) = org_dateien(&w, ID_A, "Verein").unwrap();
        let namen: Vec<String> = d.into_iter().map(|(n, _)| n).collect();
        assert_eq!(namen, [format!("Verein/{}", ordner::MARKE), "Verein/Fest/beleg.pdf".into()]);

        // Umzug steht noch aus: die Wurzel, aber ohne fremde Organisations-Ordner.
        let (d, _) = org_dateien(&w, ID_A, "").unwrap();
        let namen: Vec<String> = d.into_iter().map(|(n, _)| n).collect();
        assert_eq!(namen, ["Altprojekt/alt.pdf"]);

        // Zeigt der gespeicherte Name auf den Ordner einer ANDEREN Organisation
        // (z. B. eine Kopie vor dem ersten Anmelden): Abbruch statt fremder Dateien.
        assert_eq!(org_dateien(&w, ID_A, "Duo").unwrap_err(), FREMDER_ORDNER);
        // Pfad-Tricks bleiben im Wurzelordner ("../Duo" wird zu ".._Duo").
        assert!(org_dateien(&w, ID_A, "..").is_err());
        assert!(org_dateien(&w, ID_A, "../Duo").unwrap().0.is_empty());
    }

    #[test]
    fn ordner_mit_marke_ohne_id_mischt_sich_nicht_in_einen_fremden() {
        let o = Ordner::neu();
        let (schluessel, salt, tresor_inhalt) = zugang();
        let alt = o.pfad("alt");
        ordner::marke_setzen(&alt.join("Verein"), None).unwrap();
        datei_anlegen(&alt, "Verein/x.pdf", b"x");
        let (dateien, probleme) = dateien_sammeln(&alt, "", false);
        let s = o.pfad("ohne-id.a3ksicherung");
        komplett_schreiben(&s, &[(ID_A.into(), tresor_inhalt)], dateien, probleme, &schluessel, &salt).unwrap();

        let w = o.pfad("w");
        marke(&w, "Verein", ID_X);
        let orgs = o.pfad("orgs");
        fs::create_dir_all(&orgs).unwrap();
        einspielen_mit(&s, &w, &orgs, &[]).unwrap();
        assert_eq!(fs::read(w.join("Verein (2)/x.pdf")).unwrap(), b"x");
        assert!(!w.join("Verein/x.pdf").exists());
        assert_eq!(ordner::besitz(&w.join("Verein")), ordner::Besitz::Id(ID_X.into()));
    }

    #[test]
    fn version_1_wird_weiter_eingespielt() {
        let o = Ordner::neu();
        let s = o.pfad("alt.a3ksicherung");
        let t = tresor_eintrag();
        sicherung_von_hand_als(&s, &[("tresor.enc", &t), ("dateien/Projekt/a.txt", b"a")], VERSION_1);
        let b = entpacken_test(&s, PASSWORT, &o.pfad("t.enc.neu"), &o.pfad("neu")).unwrap();
        assert_eq!(b.dateien, 1);
        assert_eq!(fs::read(o.pfad("t.enc.neu")).unwrap(), t);
        // Die Namen der jeweils anderen Version werden abgewiesen.
        sicherung_von_hand_als(&s, &[(T_NAME, &t)], VERSION_1);
        assert!(entpacken_test(&s, PASSWORT, &o.pfad("u.enc.neu"), &o.pfad("neu2")).is_err());
        sicherung_von_hand(&s, &[("tresor.enc", &t)]);
        assert!(entpacken_test(&s, PASSWORT, &o.pfad("u.enc.neu"), &o.pfad("neu2")).is_err());
        sicherung_von_hand(&s, &[("tresor/../../x.enc", &t)]);
        assert_eq!(entpacken_test(&s, PASSWORT, &o.pfad("u.enc.neu"), &o.pfad("neu2")).unwrap_err(), UNZULAESSIG);
        assert!(!o.pfad("u.enc.neu").exists() && !o.pfad("neu2").exists());
    }

    #[test]
    fn vorhandene_organisation_wird_nur_auf_wunsch_ersetzt() {
        let o = Ordner::neu();
        let orgs = o.pfad("orgs");
        fs::create_dir_all(&orgs).unwrap();
        fs::write(orgs.join(format!("{ID_A}.enc")), b"hier").unwrap();
        let ids = vec![ID_A.to_string(), ID_B.to_string()];

        // Ohne Wahl: die vorhandene bleibt, die neue kommt dazu.
        let plan = plan_bauen(&ids, &orgs, &HashMap::new()).unwrap();
        assert!(!plan.orgs.contains_key(ID_A));
        assert_eq!(plan.orgs[ID_B].0, ID_B);

        let wahl = |a: &str, b: &str| HashMap::from([(ID_A.to_string(), a.to_string()), (ID_B.to_string(), b.to_string())]);
        let plan = plan_bauen(&ids, &orgs, &wahl("ersetzen", "weglassen")).unwrap();
        assert_eq!(plan.orgs[ID_A], (ID_A.to_string(), orgs.join(format!("{ID_A}.enc"))));
        assert!(!plan.orgs.contains_key(ID_B));

        // Als Kopie: neue ID, die vorhandene bleibt unberuehrt.
        let plan = plan_bauen(&ids, &orgs, &wahl("kopie", "dazu")).unwrap();
        let (kopie, _) = &plan.orgs[ID_A];
        assert!(kopie != ID_A && kopie != ID_B && tresor::id_gueltig(kopie));

        // Alles abgewaehlt: Fehler, nichts passiert.
        assert!(plan_bauen(&ids, &orgs, &wahl("weglassen", "weglassen")).is_err());
    }

    /// Sicherung einer Organisation (ID_A) mit Ordner "Verein" und einem Beleg.
    fn org_sicherung(o: &Ordner) -> PathBuf {
        let (schluessel, salt, tresor_inhalt) = zugang();
        let alt = o.pfad("alt");
        marke(&alt, "Verein", ID_A);
        datei_anlegen(&alt, "Verein/Fest/beleg.pdf", GEHEIMER_INHALT);
        let (dateien, probleme) = org_dateien(&alt, ID_A, "Verein").unwrap();
        let ziel = o.pfad("org.a3ksicherung");
        let b = komplett_schreiben(&ziel, &[(ID_A.into(), tresor_inhalt)], dateien, probleme, &schluessel, &salt).unwrap();
        assert_eq!((b.dateien, b.organisationen), (1, 1), "Marke zaehlt nicht als Datei");
        ziel
    }

    fn einspielen_mit(quelle: &Path, wurzel: &Path, orgs: &Path, wahl: &[(&str, &str)]) -> Result<(Bericht, Vec<(PathBuf, PathBuf)>), String> {
        let wahl: HashMap<String, String> = wahl.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        komplett_entpacken(quelle, PASSWORT, |i| plan_bauen(&i.ids(), orgs, &wahl), wurzel)
    }

    #[test]
    fn ordner_einer_anderen_organisation_wird_nie_vermischt() {
        let o = Ordner::neu();
        let s = org_sicherung(&o);
        let w = o.pfad("w");
        let orgs = o.pfad("orgs");
        fs::create_dir_all(&orgs).unwrap();
        // Hier gibt es schon "Verein" – aber von einer ANDEREN Organisation.
        marke(&w, "Verein", ID_X);
        datei_anlegen(&w, "Verein/eigen.pdf", b"fremd");

        let (b, tausch) = einspielen_mit(&s, &w, &orgs, &[]).unwrap();
        assert_eq!(b.dateien, 1);
        assert_eq!(tausch.len(), 1);
        assert_eq!(fs::read(w.join("Verein (2)/Fest/beleg.pdf")).unwrap(), GEHEIMER_INHALT);
        assert_eq!(ordner::besitz(&w.join("Verein (2)")), ordner::Besitz::Id(ID_A.into()));
        assert_eq!(ordner::besitz(&w.join("Verein")), ordner::Besitz::Id(ID_X.into()));
        assert!(!w.join("Verein/Fest").exists());
    }

    #[test]
    fn eigener_ordner_wird_ergaenzt_kopie_bekommt_einen_neuen() {
        let o = Ordner::neu();
        let s = org_sicherung(&o);
        let w = o.pfad("w");
        let orgs = o.pfad("orgs");
        fs::create_dir_all(&orgs).unwrap();
        fs::write(orgs.join(format!("{ID_A}.enc")), b"hier").unwrap();
        marke(&w, "Verein", ID_A);
        datei_anlegen(&w, "Verein/neu.pdf", b"neuer");

        // Ersetzen: derselbe Ordner, nichts ueberschrieben.
        let (b, tausch) = einspielen_mit(&s, &w, &orgs, &[(ID_A, "ersetzen")]).unwrap();
        assert_eq!((b.dateien, b.vorhanden), (1, 0));
        assert_eq!(tausch[0].1, orgs.join(format!("{ID_A}.enc")));
        assert!(w.join("Verein/Fest/beleg.pdf").exists() && w.join("Verein/neu.pdf").exists());
        let _ = fs::remove_file(&tausch[0].0);

        // Als Kopie: eigener Ordner mit der NEUEN ID, der bisherige bleibt.
        let (_, tausch) = einspielen_mit(&s, &w, &orgs, &[(ID_A, "kopie")]).unwrap();
        let ziel = tausch[0].1.file_stem().unwrap().to_str().unwrap().to_string();
        assert_ne!(ziel, ID_A);
        assert_eq!(ordner::besitz(&w.join("Verein (2)")), ordner::Besitz::Id(ziel));
        assert!(w.join("Verein (2)/Fest/beleg.pdf").exists());
        assert_eq!(ordner::besitz(&w.join("Verein")), ordner::Besitz::Id(ID_A.into()));
        let _ = fs::remove_file(&tausch[0].0);

        // Nicht bestaetigt: weder Tresor noch Dateien.
        let w2 = o.pfad("w2");
        fs::write(orgs.join(format!("{ID_A}.enc")), b"hier").unwrap();
        let s2 = o.pfad("zwei.a3ksicherung");
        {
            let (schluessel, salt, tresor_inhalt) = zugang();
            let alt = o.pfad("alt");
            marke(&alt, "Duo", ID_B);
            datei_anlegen(&alt, "Duo/d.pdf", b"d");
            let (dateien, probleme) = dateien_sammeln(&alt, "", false);
            komplett_schreiben(&s2, &[(ID_A.into(), tresor_inhalt.clone()), (ID_B.into(), tresor_inhalt)], dateien, probleme, &schluessel, &salt).unwrap();
        }
        let (_, tausch) = einspielen_mit(&s2, &w2, &orgs, &[]).unwrap();
        assert_eq!(tausch.len(), 1, "nur die neue (B)");
        assert!(w2.join("Duo/d.pdf").exists());
        assert!(!w2.join("Verein").exists(), "Ordner der weggelassenen A kommt nicht mit");
        assert_eq!(fs::read(orgs.join(format!("{ID_A}.enc"))).unwrap(), b"hier");
    }

    #[test]
    fn mehrfach_tausch_ist_alles_oder_nichts() {
        let o = Ordner::neu();
        let a = o.pfad("a.enc");
        fs::write(&a, b"alt-a").unwrap();
        fs::write(neu_pfad(&a), b"neu-a").unwrap();
        // Der zweite Tausch scheitert (keine ".neu"-Datei).
        let b = o.pfad("b.enc");
        let tausch = vec![(neu_pfad(&a), a.clone()), (neu_pfad(&b), b.clone())];
        assert!(tresore_austauschen(&tausch).is_err());
        assert_eq!(fs::read(&a).unwrap(), b"alt-a", "A ist wieder wie vorher");
        assert!(!b.exists());
    }

    #[test]
    fn vorschau_nennt_organisationen_und_oeffnet_nur_mit_passendem_passwort() {
        let o = Ordner::neu();
        let orgs = o.pfad("orgs");
        fs::create_dir_all(&orgs).unwrap();
        fs::write(orgs.join(format!("{ID_A}.enc")), b"hier").unwrap();
        let (schluessel, salt, _) = zugang();
        let echt = tresor::verschluesseln("{\"organisation\":{\"name\":\"Verein\"}}", &schluessel, &salt).unwrap();
        // B hat ein eigenes Passwort.
        let mut salt_b = [0u8; tresor::SALT_LAENGE];
        OsRng.fill_bytes(&mut salt_b);
        let schluessel_b = tresor::schluessel_ableiten("anderes-passwort", &salt_b).unwrap();
        let fremd = tresor::verschluesseln("{}", &schluessel_b, &salt_b).unwrap();
        let s = o.pfad("alle.a3ksicherung");
        komplett_schreiben(&s, &[(ID_A.into(), echt), (ID_B.into(), fremd)], vec![], vec![], &schluessel, &salt).unwrap();

        let v = vorschau_bauen(&s, PASSWORT, &orgs).unwrap();
        assert_eq!(v.version, VERSION);
        assert_eq!(v.orgs.len(), 2);
        assert!(v.orgs[0].vorhanden && !v.orgs[1].vorhanden);
        assert!(v.orgs[0].daten.as_deref().unwrap().contains("Verein"));
        assert!(v.orgs[1].daten.is_none());
        assert_eq!(vorschau_bauen(&s, "falsch-geraten", &orgs).unwrap_err(), "falsches_passwort");
    }
}
