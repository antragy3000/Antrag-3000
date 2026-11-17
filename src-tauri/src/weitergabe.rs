// ============================================================
// Organisation weitergeben (O6c)
//
// Wozu: Eine Organisation samt Projekten (und auf Wunsch ihren Dateien) an
// eine andere Person geben – z. B. wenn jemand anderes die Antraege eines
// Vereins uebernimmt. Die Datei (.a3korganisation) verlaesst das Geraet NICHT
// ueber das Netz: Der Nutzer speichert sie selbst (USB-Stick, eigener Weg).
//
// Warum nicht einfach eine Sicherung? Eine Sicherung ist mit dem Passwort
// der Organisation verschluesselt und enthaelt den ganzen Tresor – auch die
// Team-Verbindung (Ausweis), die nie an Dritte gehen darf. Die Weitergabe
// enthaelt nur den INHALT (das Frontend waehlt ihn per Positivliste aus,
// siehe organisationen.js) und bekommt ein EIGENES Weitergabe-Passwort, das
// man der anderen Person auf einem zweiten Weg mitteilt.
//
// Aufbau: derselbe Behaelter wie die Sicherung (backup.rs: Kopf, verpackter
// Schluessel, STREAM-AES-GCM in Bloecken), nur mit eigener Kennung
// "ANTRAG3W". Darin:
//
//   "organisation.json"              genau einmal, zuerst
//   "dateien/<Pfad im Organisations-Ordner>"   beliebig viele, ohne Marke
//
// Einlesen in zwei Durchgaengen wie bei der Sicherung: erst wird alles
// geprueft (Siegel, jeder Pfad), dann erst geschrieben – und nur in einen
// FRISCH angelegten, leeren Ordner. Vorhandenes wird nie ueberschrieben.
// ============================================================

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::OsRng;
use serde::Serialize;
use zeroize::Zeroizing;

use crate::backup::{
    self, Aufraeumen, Bericht, BESCHAEDIGT, MAGIC_WEITERGABE, NAME_MAX, PRAEFIX_DATEIEN, SALT_ENDE, TRESOR_MAX,
    UNZULAESSIG,
};
use crate::{ordner, tresor};

const VERSION: u8 = 1;
const EINTRAG_DATEN: &str = "organisation.json";
const PASSWORT_MIN: usize = 8;

const NICHT_ZUGEORDNET: &str = "Der Ordner dieser Organisation ist noch nicht eindeutig zugeordnet. Bitte einmal sperren und neu anmelden – oder ohne Dateien weitergeben.";
const ZIEL_NICHT_FRISCH: &str = "Der Ordner für die neue Organisation ist nicht leer. Es wurde nichts hineingeschrieben.";

/// Was die Pruefung ueber eine Weitergabe-Datei weiss.
#[derive(Serialize, Debug)]
pub struct Weitergabe {
    /// Der Inhalt (JSON) – vom Frontend wie fremde Daten behandelt.
    pub daten: String,
    /// Anzahl der mitgegebenen Dateien.
    pub dateien: u64,
}

/// Die Dateien einer Organisation fuer die Weitergabe: NUR aus einem Ordner,
/// dessen Marke ihre ID traegt. Strenger als bei der Sicherung, weil die
/// Datei an eine andere Person geht – im Zweifel lieber gar keine Dateien als
/// die einer anderen Organisation. Die Marke selbst bleibt draussen; die
/// Pfade sind relativ zum Organisations-Ordner.
fn eigene_dateien(wurzel: &Path, id: &str, org_ordner: &str) -> Result<(Vec<(String, PathBuf)>, Vec<String>), String> {
    if org_ordner.is_empty() {
        return Err(NICHT_ZUGEORDNET.into());
    }
    let ordner_pfad = wurzel.join(ordner::bereinigen(org_ordner)?);
    if ordner::besitz(&ordner_pfad) != ordner::Besitz::Id(id.to_string()) {
        return Err(NICHT_ZUGEORDNET.into());
    }
    let (dateien, probleme) = backup::dateien_sammeln(&ordner_pfad, "", false);
    let dateien = dateien.into_iter().filter(|(relativ, _)| relativ != ordner::MARKE).collect();
    Ok((dateien, probleme))
}

/// Schluessel aus dem Weitergabe-Passwort und dem Salt der Datei.
fn schluessel(passwort: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    Ok(Zeroizing::new(tresor::schluessel_ableiten(passwort, salt)?))
}

/// Schreibt die Weitergabe-Datei (erst unter ".teil", gegengelesen, dann
/// umbenannt – wie die Sicherung).
fn schreiben(
    ziel: &Path,
    daten: &str,
    dateien: Vec<(String, PathBuf)>,
    probleme: Vec<String>,
    passwort: &str,
) -> Result<Bericht, String> {
    let mut salt = [0u8; tresor::SALT_LAENGE];
    OsRng.fill_bytes(&mut salt);
    let schluessel = schluessel(passwort, &salt)?;
    let mut bericht = Bericht { probleme, organisationen: 1, ..Bericht::default() };

    let teil = backup::teil_pfad(ziel);
    let _aufraeumen = Aufraeumen(teil.clone());
    let (mut schreiber, datenschluessel) =
        backup::schreiber_oeffnen(&teil, &schluessel, &salt, MAGIC_WEITERGABE, VERSION)?;
    let fehlgeschlagen = |e: io::Error| format!("Weitergabe fehlgeschlagen: {e}");

    backup::eintrag_schreiben(&mut schreiber, EINTRAG_DATEN, daten.len() as u64, &mut daten.as_bytes())
        .map_err(fehlgeschlagen)?;
    for (relativ, pfad) in dateien {
        let name = format!("{PRAEFIX_DATEIEN}{relativ}");
        let geoeffnet = fs::File::open(&pfad).and_then(|d| Ok((d.metadata()?.len(), d)));
        let (groesse, mut datei) = match geoeffnet {
            Ok(g) if name.len() <= NAME_MAX => g,
            _ => {
                bericht.probleme.push(relativ);
                continue;
            }
        };
        backup::eintrag_schreiben(&mut schreiber, &name, groesse, &mut datei)
            .map_err(|e| format!("Weitergabe abgebrochen bei „{relativ}“: {e}"))?;
        bericht.dateien += 1;
        bericht.bytes += groesse;
    }
    schreiber.write_all(&0u16.to_le_bytes()).map_err(fehlgeschlagen)?;
    {
        let puffer = schreiber.abschliessen().map_err(fehlgeschlagen)?;
        let datei = puffer.into_inner().map_err(|e| fehlgeschlagen(e.into_error()))?;
        datei.sync_all().map_err(fehlgeschlagen)?;
    }

    // Gegenlesen: Erst eine Datei, die sich oeffnen laesst, ist etwas wert.
    let gelesen = pruefen(&teil, &datenschluessel)?;
    if gelesen.dateien != bericht.dateien || gelesen.daten != daten {
        return Err("Die Kontrolle der Weitergabe-Datei ist fehlgeschlagen.".into());
    }
    fs::rename(&teil, ziel).map_err(|e| format!("Weitergabe nicht speicherbar: {e}"))?;
    Ok(bericht)
}

/// Prueft die ganze Datei, ohne etwas zu schreiben: Siegel, Aufbau, jeder Pfad.
fn pruefen(quelle: &Path, datenschluessel: &[u8; 32]) -> Result<Weitergabe, String> {
    let mut leser = backup::leser_oeffnen(quelle, datenschluessel, MAGIC_WEITERGABE)?;
    let mut daten: Option<String> = None;
    let mut dateien = 0u64;
    backup::eintraege_durchlaufen(&mut leser, |name, inhalt| {
        if name == EINTRAG_DATEN {
            if daten.is_some() {
                return Err(BESCHAEDIGT.into());
            }
            let mut text = Vec::new();
            inhalt.take(TRESOR_MAX + 1).read_to_end(&mut text).map_err(backup::lesefehler)?;
            if text.len() as u64 > TRESOR_MAX {
                return Err(BESCHAEDIGT.into());
            }
            daten = Some(String::from_utf8(text).map_err(|_| BESCHAEDIGT.to_string())?);
            return Ok(());
        }
        let relativ = name.strip_prefix(PRAEFIX_DATEIEN).ok_or(UNZULAESSIG.to_string())?;
        // Erst der Inhalt, dann die Dateien.
        if daten.is_none() {
            return Err(BESCHAEDIGT.into());
        }
        backup::relativer_pfad(relativ)?;
        if relativ != ordner::MARKE {
            dateien += 1;
        }
        Ok(())
    })?;
    let daten = daten.ok_or(BESCHAEDIGT.to_string())?;
    Ok(Weitergabe { daten, dateien })
}

/// Oeffnet den Kopf mit dem Weitergabe-Passwort ("falsches_passwort" wie beim Tresor).
fn oeffnen(quelle: &Path, passwort: &str) -> Result<Zeroizing<[u8; 32]>, String> {
    let (_, kopf) = backup::kopf_lesen(quelle, MAGIC_WEITERGABE)?;
    let schluessel = schluessel(passwort, &kopf[9..SALT_ENDE])?;
    backup::datenschluessel_entpacken(&kopf, &schluessel)
}

/// Ist `ordner_pfad` ein frisch angelegter Organisations-Ordner – Marke ohne
/// ID (noch von niemandem beansprucht) und sonst leer?
fn ist_frisch(ordner_pfad: &Path) -> bool {
    if ordner::besitz(ordner_pfad) != ordner::Besitz::OhneId {
        return false;
    }
    match fs::read_dir(ordner_pfad) {
        Ok(eintraege) => eintraege.flatten().all(|e| e.file_name() == ordner::MARKE),
        Err(_) => false,
    }
}

/// Spielt die Dateien in den frisch angelegten Ordner `ziel` ein. Erst wird
/// die ganze Datei geprueft, dann geschrieben; nie wird etwas ueberschrieben.
fn dateien_einspielen(quelle: &Path, passwort: &str, ziel: &Path) -> Result<Bericht, String> {
    let datenschluessel = oeffnen(quelle, passwort)?;
    pruefen(quelle, &datenschluessel)?;
    if !ist_frisch(ziel) {
        return Err(ZIEL_NICHT_FRISCH.into());
    }
    let mut bericht = Bericht { organisationen: 1, ..Bericht::default() };
    let mut leser = backup::leser_oeffnen(quelle, &datenschluessel, MAGIC_WEITERGABE)?;
    backup::eintraege_durchlaufen(&mut leser, |name, inhalt| {
        let Some(relativ) = name.strip_prefix(PRAEFIX_DATEIEN) else {
            return Ok(()); // organisation.json: schon geprueft, kommt ins Frontend
        };
        // Eine Marke in der Datei wird nie uebernommen: Wem der Ordner gehoert,
        // bestimmt allein diese App (beim Anlegen der Organisation).
        if relativ == ordner::MARKE {
            return Ok(());
        }
        let Some(pfad) = backup::relativer_pfad(relativ)? else {
            bericht.probleme.push(relativ.to_string());
            return Ok(());
        };
        let mut gezaehlt = inhalt.take(u64::MAX);
        match backup::datei_einspielen(ziel, &pfad, &mut gezaehlt) {
            Ok(()) => {
                bericht.dateien += 1;
                bericht.bytes += u64::MAX - gezaehlt.limit();
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => bericht.vorhanden += 1,
            Err(e) if e.kind() == io::ErrorKind::InvalidData => return Err(BESCHAEDIGT.into()),
            Err(_) => bericht.probleme.push(relativ.to_string()),
        }
        Ok(())
    })?;
    Ok(bericht)
}

// ============================================================
// Tauri-Befehle
// ============================================================

/// Organisation `org` (muss offen sein) weitergeben. `daten` ist der vom
/// Frontend ausgewaehlte Inhalt (Positivliste, ohne Team-Verbindung). Das
/// Weitergabe-Passwort muss sich vom Passwort der Organisation unterscheiden –
/// sonst kennte die andere Person danach auch dieses.
#[tauri::command]
pub async fn org_weitergeben(
    app: tauri::AppHandle,
    org: String,
    ziel: String,
    passwort: String,
    daten: String,
    mit_dateien: bool,
    org_ordner: String,
) -> Result<Bericht, String> {
    let passwort = Zeroizing::new(passwort);
    if passwort.chars().count() < PASSWORT_MIN {
        return Err("Das Weitergabe-Passwort muss mindestens 8 Zeichen haben.".into());
    }
    let (_, _, tresor_inhalt) = tresor::offener_stand(&app, &org)?;
    let wurzel = ordner::wurzel(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        if tresor::mit_passwort_oeffnen(&tresor_inhalt, &passwort).is_some() {
            return Err(
                "Bitte wähle ein anderes Passwort als das deiner Organisation – sonst kennt die andere Person danach auch dieses."
                    .into(),
            );
        }
        let (dateien, probleme) =
            if mit_dateien { eigene_dateien(&wurzel, &org, &org_ordner)? } else { (Vec::new(), Vec::new()) };
        schreiben(Path::new(&ziel), &daten, dateien, probleme, &passwort)
    })
    .await
    .map_err(|e| format!("Weitergabe abgebrochen: {e}"))?
}

/// Prueft eine Weitergabe-Datei komplett (schreibt nichts) und liefert ihren
/// Inhalt zum Uebernehmen.
#[tauri::command]
pub async fn weitergabe_pruefen(quelle: String, passwort: String) -> Result<Weitergabe, String> {
    let passwort = Zeroizing::new(passwort);
    tauri::async_runtime::spawn_blocking(move || {
        let quelle = Path::new(&quelle);
        let datenschluessel = oeffnen(quelle, &passwort)?;
        pruefen(quelle, &datenschluessel)
    })
    .await
    .map_err(|e| format!("Prüfung abgebrochen: {e}"))?
}

/// Die Dateien einer Weitergabe in den gerade angelegten (leeren) Ordner
/// `org_ordner` einspielen – bevor der Tresor der neuen Organisation entsteht.
#[tauri::command]
pub async fn weitergabe_dateien_einspielen(
    app: tauri::AppHandle,
    quelle: String,
    passwort: String,
    org_ordner: String,
) -> Result<Bericht, String> {
    let passwort = Zeroizing::new(passwort);
    if org_ordner.is_empty() {
        return Err(ZIEL_NICHT_FRISCH.into());
    }
    let ziel = ordner::org_pfad(&ordner::wurzel(&app)?, &org_ordner)?;
    tauri::async_runtime::spawn_blocking(move || dateien_einspielen(Path::new(&quelle), &passwort, &ziel))
        .await
        .map_err(|e| format!("Einspielen abgebrochen: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORT: &str = "weitergabe-geheim";
    const ID: &str = "aaaaaaaaaaaaaaaa";
    const ANDERE: &str = "bbbbbbbbbbbbbbbb";
    const INHALT: &str = r#"{"format":"antrag3000-organisation","version":1,"organisation":{"name":"Verein"}}"#;

    struct Ordner(PathBuf);

    impl Ordner {
        fn neu() -> Self {
            let mut zufall = [0u8; 8];
            OsRng.fill_bytes(&mut zufall);
            let pfad = std::env::temp_dir().join(format!("a3000-weitergabe-test-{:x}", u64::from_le_bytes(zufall)));
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

    fn datei(wurzel: &Path, relativ: &str, inhalt: &[u8]) {
        let p = wurzel.join(relativ);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, inhalt).unwrap();
    }

    /// Wurzel mit dem Ordner "Verein" (Marke ID) und einem fremden "Duo".
    fn wurzel_mit_orgs(o: &Ordner) -> PathBuf {
        let w = o.pfad("w");
        ordner::marke_setzen(&w.join("Verein"), Some(ID)).unwrap();
        datei(&w, "Verein/Fest/Belege/beleg.pdf", b"BELEG");
        ordner::marke_setzen(&w.join("Duo"), Some(ANDERE)).unwrap();
        datei(&w, "Duo/geheim.pdf", b"FREMD");
        w
    }

    fn weitergabe(o: &Ordner, w: &Path) -> PathBuf {
        let (d, p) = eigene_dateien(w, ID, "Verein").unwrap();
        let ziel = o.pfad("verein.a3korganisation");
        let b = schreiben(&ziel, INHALT, d, p, PASSWORT).unwrap();
        assert_eq!(b.dateien, 1, "die Marke zaehlt nicht und kommt nicht mit");
        ziel
    }

    fn frischer_ordner(w: &Path, name: &str) -> PathBuf {
        ordner::marke_setzen(&w.join(name), None).unwrap();
        w.join(name)
    }

    #[test]
    fn weitergeben_und_einlesen() {
        let o = Ordner::neu();
        let w = wurzel_mit_orgs(&o);
        let s = weitergabe(&o, &w);

        // Inhalt und Dateien sind verschluesselt.
        let roh = fs::read(&s).unwrap();
        assert!(!roh.windows(5).any(|f| f == b"BELEG") && !roh.windows(7).any(|f| f == b"Verein\""));

        let dk = oeffnen(&s, PASSWORT).unwrap();
        let g = pruefen(&s, &dk).unwrap();
        assert_eq!((g.daten.as_str(), g.dateien), (INHALT, 1));

        let ziel = frischer_ordner(&o.pfad("neu"), "Verein");
        let b = dateien_einspielen(&s, PASSWORT, &ziel).unwrap();
        assert_eq!(b.dateien, 1);
        assert_eq!(fs::read(ziel.join("Fest/Belege/beleg.pdf")).unwrap(), b"BELEG");
        assert_eq!(ordner::besitz(&ziel), ordner::Besitz::OhneId, "die Marke aus der Datei wird nie uebernommen");
    }

    #[test]
    fn nur_aus_dem_eigenen_markierten_ordner() {
        let o = Ordner::neu();
        let w = wurzel_mit_orgs(&o);
        assert_eq!(eigene_dateien(&w, ID, "Duo").unwrap_err(), NICHT_ZUGEORDNET);
        assert_eq!(eigene_dateien(&w, ID, "").unwrap_err(), NICHT_ZUGEORDNET);
        datei(&w, "Ohne/a.pdf", b"a");
        assert_eq!(eigene_dateien(&w, ID, "Ohne").unwrap_err(), NICHT_ZUGEORDNET);
        ordner::marke_setzen(&w.join("Frisch"), None).unwrap();
        assert_eq!(eigene_dateien(&w, ID, "Frisch").unwrap_err(), NICHT_ZUGEORDNET);
        assert!(eigene_dateien(&w, ID, "..").is_err());
        let (d, _) = eigene_dateien(&w, ID, "Verein").unwrap();
        assert_eq!(d.into_iter().map(|(n, _)| n).collect::<Vec<_>>(), ["Fest/Belege/beleg.pdf"]);
    }

    #[test]
    fn falsches_passwort_schreibt_nichts() {
        let o = Ordner::neu();
        let w = wurzel_mit_orgs(&o);
        let s = weitergabe(&o, &w);
        let ziel = frischer_ordner(&o.pfad("neu"), "Verein");
        assert_eq!(dateien_einspielen(&s, "falsch-falsch", &ziel).unwrap_err(), "falsches_passwort");
        assert!(ist_frisch(&ziel));
    }

    #[test]
    fn nur_in_einen_frischen_leeren_ordner() {
        let o = Ordner::neu();
        let w = wurzel_mit_orgs(&o);
        let s = weitergabe(&o, &w);
        // Ein Ordner, der schon einer Organisation gehoert: nie.
        assert_eq!(dateien_einspielen(&s, PASSWORT, &w.join("Duo")).unwrap_err(), ZIEL_NICHT_FRISCH);
        // Ein frischer, aber nicht leerer Ordner: auch nicht.
        let ziel = frischer_ordner(&o.pfad("neu"), "Verein");
        datei(&ziel, "schon-da.txt", b"x");
        assert_eq!(dateien_einspielen(&s, PASSWORT, &ziel).unwrap_err(), ZIEL_NICHT_FRISCH);
        // Ohne Marke: nein.
        let ohne = o.pfad("ohne");
        fs::create_dir_all(&ohne).unwrap();
        assert_eq!(dateien_einspielen(&s, PASSWORT, &ohne).unwrap_err(), ZIEL_NICHT_FRISCH);
        assert_eq!(fs::read(w.join("Duo/geheim.pdf")).unwrap(), b"FREMD");
    }

    #[test]
    fn sicherung_und_weitergabe_werden_nicht_verwechselt() {
        let o = Ordner::neu();
        let w = wurzel_mit_orgs(&o);
        let s = weitergabe(&o, &w);
        // Eine Weitergabe ist keine Sicherung …
        let e = backup::kopf_lesen(&s, backup::MAGIC_KOMPLETT).unwrap_err();
        assert!(e.contains("weitergegebene Organisation"), "{e}");
        // … und eine Sicherung keine Weitergabe.
        let sicherung = o.pfad("s.a3ksicherung");
        let mut kopf = fs::read(&s).unwrap();
        kopf[..8].copy_from_slice(backup::MAGIC_KOMPLETT);
        fs::write(&sicherung, &kopf).unwrap();
        let e = oeffnen(&sicherung, PASSWORT).unwrap_err();
        assert!(e.contains("eine Sicherung"), "{e}");
        // Eine unbekannte Version wird abgewiesen.
        let mut gestempelt = fs::read(&s).unwrap();
        gestempelt[8] = 2;
        let falsch = o.pfad("version.a3korganisation");
        fs::write(&falsch, &gestempelt).unwrap();
        assert!(oeffnen(&falsch, PASSWORT).is_err());
    }

    #[test]
    fn pfad_tricks_und_falscher_aufbau_werden_abgewiesen() {
        let o = Ordner::neu();
        let mut salt = [0u8; tresor::SALT_LAENGE];
        OsRng.fill_bytes(&mut salt);
        let schl = schluessel(PASSWORT, &salt).unwrap();
        let von_hand = |name: &str, eintraege: &[(&str, &[u8])]| -> PathBuf {
            let ziel = o.pfad(name);
            let (mut s, _) = backup::schreiber_oeffnen(&ziel, &schl, &salt, MAGIC_WEITERGABE, VERSION).unwrap();
            for (n, inhalt) in eintraege {
                backup::eintrag_schreiben(&mut s, n, inhalt.len() as u64, &mut &**inhalt).unwrap();
            }
            s.write_all(&0u16.to_le_bytes()).unwrap();
            s.abschliessen().unwrap();
            ziel
        };
        let pruefe = |p: &Path| oeffnen(p, PASSWORT).and_then(|dk| pruefen(p, &dk));

        let a = von_hand("a", &[(EINTRAG_DATEN, b"{}"), ("dateien/../boese.txt", b"x")]);
        assert_eq!(pruefe(&a).unwrap_err(), UNZULAESSIG);
        // Unter Windows unbrauchbarer Name: wird gemeldet, nie geschrieben.
        let b = von_hand("b", &[(EINTRAG_DATEN, b"{}"), ("dateien/C:/Windows/x", b"x")]);
        assert_eq!(pruefe(&b).unwrap().dateien, 1);
        let ziel_b = frischer_ordner(&o.pfad("neu-b"), "X");
        let bericht = dateien_einspielen(&b, PASSWORT, &ziel_b).unwrap();
        assert_eq!((bericht.dateien, bericht.probleme.clone()), (0, vec!["C:/Windows/x".to_string()]));
        let c = von_hand("c", &[("dateien/a.txt", b"x"), (EINTRAG_DATEN, b"{}")]);
        assert_eq!(pruefe(&c).unwrap_err(), BESCHAEDIGT);
        let d = von_hand("d", &[(EINTRAG_DATEN, b"{}"), (EINTRAG_DATEN, b"{}")]);
        assert_eq!(pruefe(&d).unwrap_err(), BESCHAEDIGT);
        let e = von_hand("e", &[(EINTRAG_DATEN, b"{}"), ("tresor/aaaaaaaaaaaaaaaa.enc", b"x")]);
        assert_eq!(pruefe(&e).unwrap_err(), UNZULAESSIG);
        let f = von_hand("f", &[]);
        assert_eq!(pruefe(&f).unwrap_err(), BESCHAEDIGT);

        // Selbst wenn so eine Datei durchkaeme: Geschrieben wird nur unterhalb
        // des Zielordners, eine Marke darin wird ignoriert.
        let g = von_hand("g", &[(EINTRAG_DATEN, b"{}"), (&format!("dateien/{}", ordner::MARKE), b"id=bbbbbbbbbbbbbbbb")]);
        let ziel = frischer_ordner(&o.pfad("neu"), "X");
        dateien_einspielen(&g, PASSWORT, &ziel).unwrap();
        assert_eq!(ordner::besitz(&ziel), ordner::Besitz::OhneId);
        // Abgeschnitten: beschaedigt, nichts geschrieben.
        let mut kurz = fs::read(&a).unwrap();
        kurz.truncate(kurz.len() - 5);
        fs::write(o.pfad("kurz"), &kurz).unwrap();
        assert!(pruefe(&o.pfad("kurz")).is_err());
    }
}
