// ============================================================
// Ordnerverwaltung der Nutzdaten (CLAUDE.md):
//   [Dokumente]\Antrag 3000\[Organisation]\[Projektname]\[Foerderungsname]\
//
// Jede Organisation hat genau einen eigenen Ordner. Sein Name steht im Tresor
// der Organisation (organisation.ordner); das Frontend gibt ihn bei jedem
// Befehl mit. So kann kein Befehl aus Versehen im Ordner einer anderen
// Organisation landen: Fehlt die Angabe, schlaegt der Aufruf fehl.
//
// Der Rust-Kern uebernimmt nur die Systemzugriffe: Ordner anlegen,
// im Windows-Explorer oeffnen, bei Projekt-Umbenennung mitziehen.
// Welche Ordner wann gebraucht werden, entscheidet das Frontend.
// ============================================================

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::Manager;

/// Markiert einen Ordner als Ordner einer Organisation. Der Umzug alter
/// Projektordner laesst solche Ordner in Ruhe: Ein Projekt, das zufaellig so
/// heisst wie eine andere Organisation, darf deren Ordner nie mitnehmen.
const MARKE: &str = ".antrag3000-organisation";
const MARKE_TEXT: &str = "Dieser Ordner gehoert zu einer Organisation in Antrag 3000. Bitte diese Datei nicht loeschen.\n";

/// Wurzelordner aller Nutzdaten: Dokumente\Antrag 3000
pub(crate) fn wurzel(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dokumente = app
        .path()
        .document_dir()
        .map_err(|e| format!("Dokumente-Ordner nicht ermittelbar: {e}"))?;
    Ok(dokumente.join("Antrag 3000"))
}

/// Macht aus einem freien Namen einen gueltigen Windows-Ordnernamen:
/// verbotene Zeichen werden durch _ ersetzt, Leerraum und Punkte am
/// Rand entfernt (Windows erlaubt sie dort nicht).
pub(crate) fn bereinigen(name: &str) -> Result<String, String> {
    const VERBOTEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let ersetzt: String = name
        .chars()
        .map(|c| {
            if VERBOTEN.contains(&c) || (c as u32) < 32 {
                '_'
            } else {
                c
            }
        })
        .collect();
    let sauber = ersetzt
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_string();
    if sauber.is_empty() {
        return Err("Der Name ergibt keinen gueltigen Ordnernamen.".into());
    }
    // Reservierte Windows-Geraetenamen (CON, NUL, COM1 ... LPT9) sind als
    // Datei-/Ordnername verboten – auch mit Endung (CON.txt). Wir stellen
    // ihnen ein "_" voran, damit das Anlegen nicht fehlschlaegt. (Audit D3.)
    const RESERVIERT: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    // trim_end: Windows behandelt auch "CON .txt" als Geraetenamen.
    let basis = sauber.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    if RESERVIERT.contains(&basis.as_str()) {
        return Ok(format!("_{sauber}"));
    }
    Ok(sauber)
}

/// Der Ordner einer Organisation unterhalb der Wurzel. Der Name kommt aus dem
/// Tresor und wird darum hier bereinigt: Pfad-Tricks ("..", Trenner) kommen so
/// nicht aus der Wurzel heraus. Leer = die Wurzel selbst; das gilt nur, solange
/// der Umzug einer Organisation noch offen ist (ihre Projekte liegen dann noch dort).
pub(crate) fn org_pfad(wurzel: &Path, org_ordner: &str) -> Result<PathBuf, String> {
    if org_ordner.is_empty() {
        return Ok(wurzel.to_path_buf());
    }
    Ok(wurzel.join(bereinigen(org_ordner)?))
}

/// Der Ordner einer Organisation unter Dokumente\Antrag 3000.
pub(crate) fn org_wurzel(app: &tauri::AppHandle, org_ordner: &str) -> Result<PathBuf, String> {
    org_pfad(&wurzel(app)?, org_ordner)
}

fn marke_setzen(ordner: &Path) -> Result<(), String> {
    fs::create_dir_all(ordner).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let marke = ordner.join(MARKE);
    if !marke.exists() {
        fs::write(&marke, MARKE_TEXT).map_err(|e| format!("Ordner nicht beschreibbar: {e}"))?;
    }
    Ok(())
}

/// Der erste freie Ordnername fuer `name`: "Verein", sonst "Verein (2)" usw.
/// `eigen` ist der bisherige Ordner der Organisation; er gilt nicht als belegt.
/// Windows unterscheidet Gross- und Kleinschreibung nicht, darum vergleicht
/// canonicalize die echten Ordner statt der Schreibweisen.
fn freier_org_name(wurzel: &Path, name: &str, eigen: Option<&str>) -> Result<String, String> {
    let basis = bereinigen(name).unwrap_or_else(|_| "Organisation".into());
    let eigen_pfad = eigen.and_then(|e| fs::canonicalize(wurzel.join(e)).ok());
    for n in 1..1000 {
        let kandidat = if n == 1 { basis.clone() } else { format!("{basis} ({n})") };
        let pfad = wurzel.join(&kandidat);
        if !pfad.exists() {
            return Ok(kandidat);
        }
        if eigen_pfad.is_some() && fs::canonicalize(&pfad).ok() == eigen_pfad {
            return Ok(kandidat);
        }
    }
    Err("Es konnte kein freier Ordnername gefunden werden.".into())
}

fn org_ordner_neu(wurzel: &Path, name: &str) -> Result<String, String> {
    fs::create_dir_all(wurzel).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let frei = freier_org_name(wurzel, name, None)?;
    marke_setzen(&wurzel.join(&frei))?;
    Ok(frei)
}

/// Legt den Ordner einer Organisation an und gibt den vergebenen Namen zurueck.
/// Das Frontend speichert ihn im Tresor (organisation.ordner).
#[tauri::command]
pub fn org_ordner_anlegen(app: tauri::AppHandle, name: String) -> Result<String, String> {
    org_ordner_neu(&wurzel(&app)?, &name)
}

/// Bringt den Ordner auf den Namen der Organisation und stellt sicher, dass es
/// ihn gibt. Passt der Name schon, passiert nichts. Gibt den Namen zurueck, der
/// danach gilt. Scheitert das Umbenennen (Datei offen), bleibt alles, wie es war.
fn org_ordner_abgleichen(wurzel: &Path, alt: &str, name: &str) -> Result<String, String> {
    let alt_sauber = bereinigen(alt)?;
    let neu = freier_org_name(wurzel, name, Some(&alt_sauber))?;
    if neu != alt_sauber {
        ordner_verschieben(&wurzel.join(&alt_sauber), &wurzel.join(&neu))?;
    }
    marke_setzen(&wurzel.join(&neu))?;
    Ok(neu)
}

#[tauri::command]
pub fn org_ordner_umbenennen(
    app: tauri::AppHandle,
    alt: String,
    name: String,
) -> Result<String, String> {
    org_ordner_abgleichen(&wurzel(&app)?, &alt, &name)
}

/// Ergebnis des Umzugs: wie viele Projektordner umgezogen sind und welche
/// bewusst liegen blieben, weil am Ziel schon etwas liegt.
#[derive(Serialize, Debug)]
pub struct OrdnerUmzug {
    pub verschoben: usize,
    pub uebrig: Vec<String>,
}

/// Zieht die Projektordner einer Organisation aus der Wurzel in ihren eigenen
/// Ordner. Das darf mehrmals laufen: Was schon umgezogen ist, wird uebersprungen.
///
/// Alles oder nichts: Laesst sich ein Ordner nicht bewegen (z. B. weil eine Datei
/// darin in Word offen ist), wandern die schon bewegten zurueck und der Aufruf
/// meldet den Fehler. Die Organisation arbeitet dann weiter in der Wurzel und
/// versucht es beim naechsten Anmelden erneut.
///
/// Nur Ordner bekannter Projekte werden angefasst, nie etwas anderes. Liegt am
/// Ziel schon ein Ordner oder ist die Quelle der Ordner einer Organisation,
/// bleibt die Quelle stehen (nichts wird zusammengemischt oder ueberschrieben).
fn projekte_umziehen(
    wurzel: &Path,
    org_ordner: &str,
    projekte: &[String],
    verschieben: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<OrdnerUmzug, String> {
    if org_ordner.is_empty() {
        return Err("Die Organisation hat noch keinen Ordner.".into());
    }
    let ziel = org_pfad(wurzel, org_ordner)?;
    marke_setzen(&ziel)?;

    let mut bewegt: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut uebrig = Vec::new();
    for projekt in projekte {
        let Ok(name) = bereinigen(projekt) else { continue };
        let von = wurzel.join(&name);
        let nach = ziel.join(&name);
        if !von.is_dir() {
            continue;
        }
        if von.join(MARKE).exists() || nach.exists() {
            uebrig.push(name);
            continue;
        }
        if let Err(e) = verschieben(&von, &nach) {
            for (v, n) in bewegt.iter().rev() {
                let _ = fs::rename(n, v);
            }
            return Err(format!("Der Ordner \"{name}\" konnte nicht verschoben werden: {e}"));
        }
        bewegt.push((von, nach));
    }
    Ok(OrdnerUmzug { verschoben: bewegt.len(), uebrig })
}

#[tauri::command]
pub fn org_ordner_umziehen(
    app: tauri::AppHandle,
    org_ordner: String,
    projekte: Vec<String>,
) -> Result<OrdnerUmzug, String> {
    projekte_umziehen(&wurzel(&app)?, &org_ordner, &projekte, |v, n| fs::rename(v, n))
}

/// Legt den Ordner (Projekt, optional mit Foerderungs-Unterordner)
/// bei Bedarf an und oeffnet ihn im Windows-Explorer.
#[tauri::command]
pub fn ordner_oeffnen(
    app: tauri::AppHandle,
    org_ordner: String,
    projekt: String,
    foerderung: Option<String>,
) -> Result<String, String> {
    let mut pfad = org_wurzel(&app, &org_ordner)?.join(bereinigen(&projekt)?);
    if let Some(f) = foerderung {
        pfad = pfad.join(bereinigen(&f)?);
    }
    fs::create_dir_all(&pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    tauri_plugin_opener::open_path(pfad.clone(), None::<&str>)
        .map_err(|e| format!("Ordner laesst sich nicht oeffnen: {e}"))?;
    Ok(pfad.to_string_lossy().to_string())
}

/// Entfernt die kurzlebigen KLARTEXT-Dateien aus dem System-Temp-Ordner:
/// den Ordner mit entschluesselten Belegen und die Antrags-PDF-Vorschauen.
/// Wird beim Sperren des Tresors und beim Programmstart aufgerufen, damit
/// keine sensiblen Reste (IBAN/Steuer im PDF, Belege) liegen bleiben.
/// Best-effort: eine gerade im Betrachter geoeffnete (gesperrte) Datei wird
/// uebersprungen und beim naechsten Mal entfernt.
#[tauri::command]
pub fn temp_aufraeumen() -> Result<(), String> {
    let temp = std::env::temp_dir();
    // 1. Entschluesselte Belege (ganzer Ordner).
    let belege = temp.join("Antrag3000-Belege");
    if belege.exists() {
        let _ = fs::remove_dir_all(&belege);
    }
    // 2. Antrags-PDF-Vorschauen (antrag3000-vorschau-*.pdf).
    if let Ok(eintraege) = fs::read_dir(&temp) {
        for e in eintraege.flatten() {
            if let Some(n) = e.file_name().to_str() {
                if n.starts_with("antrag3000-vorschau-") && n.ends_with(".pdf") {
                    let _ = fs::remove_file(e.path());
                }
            }
        }
    }
    Ok(())
}

/// Zieht den Projektordner bei einer Umbenennung mit um.
/// Existiert der alte Ordner nicht, ist nichts zu tun.
#[tauri::command]
pub fn ordner_umbenennen(
    app: tauri::AppHandle,
    org_ordner: String,
    alt: String,
    neu: String,
) -> Result<(), String> {
    let w = org_wurzel(&app, &org_ordner)?;
    let alter_pfad = w.join(bereinigen(&alt)?);
    let neuer_pfad = w.join(bereinigen(&neu)?);
    ordner_verschieben(&alter_pfad, &neuer_pfad)
}

/// Verschiebt einen Ordner, ohne je einen fremden Ordner zu treffen.
/// Windows unterscheidet Gross- und Kleinschreibung nicht: Bei "festival" ->
/// "Festival" meldet es das Ziel als vorhanden, obwohl es derselbe Ordner ist.
/// Darum vergleicht canonicalize die echten Ordner statt der Schreibweisen.
fn ordner_verschieben(alter_pfad: &Path, neuer_pfad: &Path) -> Result<(), String> {
    if alter_pfad == neuer_pfad || !alter_pfad.exists() {
        return Ok(());
    }
    let selber_ordner = fs::canonicalize(alter_pfad).ok() == fs::canonicalize(neuer_pfad).ok();
    if neuer_pfad.exists() && !selber_ordner {
        return Err(format!(
            "Es existiert bereits ein Ordner namens \"{}\".",
            neuer_pfad.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    fs::rename(alter_pfad, neuer_pfad).map_err(|e| format!("Umbenennen fehlgeschlagen: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frischer, leerer Testordner mit eindeutigem Namen.
    fn testordner(zweck: &str) -> PathBuf {
        let eindeutig = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pfad = std::env::temp_dir().join(format!("a3000-ordner-{zweck}-{eindeutig}"));
        fs::create_dir_all(&pfad).unwrap();
        pfad
    }

    #[test]
    fn verschiebt_auf_freien_namen_samt_inhalt() {
        let w = testordner("frei");
        fs::create_dir(w.join("Alt")).unwrap();
        fs::write(w.join("Alt").join("beleg.pdf"), "x").unwrap();
        ordner_verschieben(&w.join("Alt"), &w.join("Neu")).unwrap();
        assert!(!w.join("Alt").exists());
        assert!(w.join("Neu").join("beleg.pdf").exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn fremder_ordner_bleibt_unberuehrt() {
        let w = testordner("fremd");
        fs::create_dir(w.join("A")).unwrap();
        fs::create_dir(w.join("B")).unwrap();
        fs::write(w.join("A").join("von-a.pdf"), "x").unwrap();
        fs::write(w.join("B").join("von-b.pdf"), "x").unwrap();
        assert!(ordner_verschieben(&w.join("A"), &w.join("B")).is_err());
        assert!(w.join("A").join("von-a.pdf").exists());
        assert!(w.join("B").join("von-b.pdf").exists());
        assert!(!w.join("B").join("von-a.pdf").exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn ohne_alten_ordner_ist_nichts_zu_tun() {
        let w = testordner("ohne");
        ordner_verschieben(&w.join("Gibt-es-nicht"), &w.join("Neu")).unwrap();
        assert!(!w.join("Neu").exists());
        let _ = fs::remove_dir_all(&w);
    }

    fn liste(namen: &[&str]) -> Vec<String> {
        namen.iter().map(|s| s.to_string()).collect()
    }

    fn normal(v: &Path, n: &Path) -> std::io::Result<()> {
        fs::rename(v, n)
    }

    #[test]
    fn org_pfad_bleibt_in_der_wurzel() {
        let w = testordner("pfad");
        assert_eq!(org_pfad(&w, "").unwrap(), w);
        assert_eq!(org_pfad(&w, "Verein").unwrap(), w.join("Verein"));
        // Pfad-Tricks aus einem praeparierten Tresor
        assert!(org_pfad(&w, "..").is_err());
        assert!(org_pfad(&w, ".").is_err());
        assert_eq!(org_pfad(&w, "..\\boese").unwrap(), w.join(".._boese"));
        assert_eq!(org_pfad(&w, "../boese").unwrap(), w.join(".._boese"));
        assert_eq!(org_pfad(&w, "C:\\Windows").unwrap(), w.join("C__Windows"));
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn neuer_org_ordner_nimmt_nie_einen_belegten_namen() {
        let w = testordner("anlegen");
        assert_eq!(org_ordner_neu(&w, "Verein").unwrap(), "Verein");
        assert!(w.join("Verein").join(MARKE).exists());
        assert_eq!(org_ordner_neu(&w, "Verein").unwrap(), "Verein (2)");
        // Auch ein alter Projektordner gleichen Namens bleibt unangetastet.
        fs::create_dir(w.join("Festival")).unwrap();
        assert_eq!(org_ordner_neu(&w, "Festival").unwrap(), "Festival (2)");
        assert!(!w.join("Festival").join(MARKE).exists());
        // Name ohne gueltige Zeichen
        assert_eq!(org_ordner_neu(&w, "...").unwrap(), "Organisation");
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn abgleich_benennt_um_und_trifft_keinen_fremden_ordner() {
        let w = testordner("abgleich");
        let a = org_ordner_neu(&w, "Alt").unwrap();
        fs::create_dir(w.join("Alt").join("Sommerfest")).unwrap();
        assert_eq!(org_ordner_abgleichen(&w, &a, "Neu").unwrap(), "Neu");
        assert!(w.join("Neu").join("Sommerfest").exists());
        assert!(!w.join("Alt").exists());
        // Name schon vergeben: naechster freier Name, der fremde bleibt, wie er ist.
        org_ordner_neu(&w, "Belegt").unwrap();
        assert_eq!(org_ordner_abgleichen(&w, "Neu", "Belegt").unwrap(), "Belegt (2)");
        assert!(!w.join("Belegt").join("Sommerfest").exists());
        // Nichts zu tun, wenn es schon stimmt (auch mit Nummer im Namen).
        assert_eq!(org_ordner_abgleichen(&w, "Belegt (2)", "Belegt").unwrap(), "Belegt (2)");
        // Vom Nutzer geloeschter Ordner kommt wieder.
        fs::remove_dir_all(w.join("Belegt (2)")).unwrap();
        assert_eq!(org_ordner_abgleichen(&w, "Belegt (2)", "Wieder").unwrap(), "Wieder");
        assert!(w.join("Wieder").join(MARKE).exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_bewegt_nur_bekannte_projekte_und_darf_wiederholt_laufen() {
        let w = testordner("umzug");
        fs::create_dir(w.join("Sommerfest")).unwrap();
        fs::write(w.join("Sommerfest").join("a.pdf"), "x").unwrap();
        fs::create_dir(w.join("Fremd")).unwrap();
        let name = org_ordner_neu(&w, "Mira Kern").unwrap();
        let projekte = liste(&["Sommerfest", "Noch ohne Ordner"]);

        let b = projekte_umziehen(&w, &name, &projekte, normal).unwrap();
        assert_eq!(b.verschoben, 1);
        assert!(b.uebrig.is_empty());
        assert!(w.join("Mira Kern").join("Sommerfest").join("a.pdf").exists());
        assert!(!w.join("Sommerfest").exists());
        assert!(w.join("Fremd").exists(), "nicht zugeordnete Ordner bleiben");

        let b = projekte_umziehen(&w, &name, &projekte, normal).unwrap();
        assert_eq!(b.verschoben, 0, "zweiter Lauf hat nichts mehr zu tun");
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_nimmt_nie_den_ordner_einer_anderen_organisation_mit() {
        let w = testordner("marke");
        let verein = org_ordner_neu(&w, "Verein").unwrap();
        fs::write(w.join("Verein").join("geheim.pdf"), "x").unwrap();
        let ich = org_ordner_neu(&w, "Ich").unwrap();
        let b = projekte_umziehen(&w, &ich, &liste(&[&verein]), normal).unwrap();
        assert_eq!(b.verschoben, 0);
        assert_eq!(b.uebrig, vec!["Verein".to_string()]);
        assert!(w.join("Verein").join("geheim.pdf").exists());
        assert!(!w.join("Ich").join("Verein").exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_mischt_nichts_zusammen() {
        let w = testordner("konflikt");
        let ich = org_ordner_neu(&w, "Ich").unwrap();
        fs::create_dir(w.join("Ich").join("Fest")).unwrap();
        fs::write(w.join("Ich").join("Fest").join("neu.pdf"), "neu").unwrap();
        fs::create_dir(w.join("Fest")).unwrap();
        fs::write(w.join("Fest").join("alt.pdf"), "alt").unwrap();
        let b = projekte_umziehen(&w, &ich, &liste(&["Fest"]), normal).unwrap();
        assert_eq!(b.uebrig, vec!["Fest".to_string()]);
        assert!(w.join("Fest").join("alt.pdf").exists());
        assert!(!w.join("Ich").join("Fest").join("alt.pdf").exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_ist_alles_oder_nichts() {
        let w = testordner("rollback");
        for p in ["Eins", "Zwei", "Drei"] {
            fs::create_dir(w.join(p)).unwrap();
            fs::write(w.join(p).join("x.pdf"), p).unwrap();
        }
        let ich = org_ordner_neu(&w, "Ich").unwrap();
        // "Drei" laesst sich nicht bewegen, als waere darin eine Datei offen.
        let klemmt = |v: &Path, n: &Path| {
            if v.ends_with("Drei") {
                Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "offen"))
            } else {
                fs::rename(v, n)
            }
        };
        let fehler = projekte_umziehen(&w, &ich, &liste(&["Eins", "Zwei", "Drei"]), klemmt).unwrap_err();
        assert!(fehler.contains("Drei"));
        for p in ["Eins", "Zwei", "Drei"] {
            assert!(w.join(p).join("x.pdf").exists(), "{p} ist wieder am alten Platz");
            assert!(!w.join("Ich").join(p).exists());
        }
        let _ = fs::remove_dir_all(&w);
    }

    #[cfg(windows)]
    #[test]
    fn nur_die_schreibweise_aendern_geht_unter_windows() {
        let w = testordner("schreibweise");
        fs::create_dir(w.join("festival")).unwrap();
        ordner_verschieben(&w.join("festival"), &w.join("Festival")).unwrap();
        let namen: Vec<String> = fs::read_dir(&w)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(namen, vec!["Festival".to_string()]);
        let _ = fs::remove_dir_all(&w);
    }
}
