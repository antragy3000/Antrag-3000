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
///
/// Die Marke traegt ausserdem die ID der Organisation ("id=..."). Sie ist die
/// eigentliche Antwort auf "welcher Ordner gehoert wem?" – der Name im Tresor
/// ist nur ein Hinweis. So koennen zwei Organisationen nie denselben Ordner
/// benutzen, auch nicht nach dem Einspielen einer Sicherung oder Weitergabe,
/// deren Ordner hier schon unter gleichem Namen existiert.
pub(crate) const MARKE: &str = ".antrag3000-organisation";
const MARKE_TEXT: &str = "Dieser Ordner gehoert zu einer Organisation in Antrag 3000. Bitte diese Datei nicht loeschen.\n";
const MARKE_ID: &str = "id=";

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

/// Was die Marke eines Ordners ueber seinen Besitzer sagt.
#[derive(Debug, PartialEq)]
pub(crate) enum Besitz {
    /// Keine Marke (oder kein Ordner).
    KeineMarke,
    /// Marke ohne ID: angelegt, aber noch von keiner Organisation beansprucht
    /// (z. B. aus Version 0.6 vor dieser Etappe oder gerade erst reserviert).
    OhneId,
    Id(String),
}

/// Die (gueltige) ID aus dem Text einer Marke.
pub(crate) fn marke_id(text: &str) -> Option<String> {
    text.lines()
        .find_map(|z| z.trim().strip_prefix(MARKE_ID))
        .map(str::trim)
        .filter(|id| crate::tresor::id_gueltig(id))
        .map(str::to_string)
}

pub(crate) fn besitz(ordner: &Path) -> Besitz {
    let marke = ordner.join(MARKE);
    match fs::read_to_string(&marke) {
        Ok(text) => marke_id(&text).map(Besitz::Id).unwrap_or(Besitz::OhneId),
        Err(_) if marke.exists() => Besitz::OhneId,
        Err(_) => Besitz::KeineMarke,
    }
}

/// Legt Ordner und Marke an. Mit `id` beansprucht die Organisation eine Marke,
/// die noch keine ID traegt. Eine fremde ID wird nie ueberschrieben – wem ein
/// Ordner gehoert, entscheidet allein eigener_ordner().
pub(crate) fn marke_setzen(ordner: &Path, id: Option<&str>) -> Result<(), String> {
    fs::create_dir_all(ordner).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let schreiben = match (besitz(ordner), id) {
        (Besitz::KeineMarke, _) => true,
        (Besitz::OhneId, Some(_)) => true,
        _ => false,
    };
    if schreiben {
        let text = match id {
            Some(id) => format!("{MARKE_TEXT}{MARKE_ID}{id}\n"),
            None => MARKE_TEXT.to_string(),
        };
        fs::write(ordner.join(MARKE), text).map_err(|e| format!("Ordner nicht beschreibbar: {e}"))?;
    }
    Ok(())
}

/// Welcher Ordner gehoert der Organisation `id`? In dieser Reihenfolge:
///  1. der gespeicherte, wenn seine Marke ihre ID traegt;
///  2. sonst irgendein Ordner der Wurzel mit ihrer ID (z. B. nach dem
///     Einspielen einer Sicherung unter freiem Namen);
///  3. sonst der gespeicherte, wenn er niemandem gehoert (Marke ohne ID, keine
///     Marke oder gar nicht mehr da – dann wird er neu angelegt);
///  4. sonst (der gespeicherte gehoert einer ANDEREN Organisation) ein neuer.
/// Liefert nur den Namen; angelegt und beansprucht wird in org_ordner_abgleichen.
fn eigener_ordner(wurzel: &Path, id: &str, gespeichert: &str, name: &str) -> Result<String, String> {
    let meine = Besitz::Id(id.to_string());
    let gespeichert = if gespeichert.is_empty() { None } else { Some(bereinigen(gespeichert)?) };
    if let Some(g) = &gespeichert {
        if besitz(&wurzel.join(g)) == meine {
            return Ok(g.clone());
        }
    }
    if let Ok(eintraege) = fs::read_dir(wurzel) {
        let mut treffer: Vec<String> = eintraege
            .flatten()
            .filter(|e| e.file_type().map(|a| a.is_dir()).unwrap_or(false))
            .filter(|e| besitz(&e.path()) == meine)
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        treffer.sort();
        if let Some(n) = treffer.into_iter().next() {
            return Ok(n);
        }
    }
    if let Some(g) = gespeichert {
        if !matches!(besitz(&wurzel.join(&g)), Besitz::Id(_)) {
            return Ok(g);
        }
    }
    freier_org_name(wurzel, name, None)
}

/// Der erste freie Ordnername fuer `name`: "Verein", sonst "Verein (2)" usw.
/// `eigen` ist der bisherige Ordner der Organisation; er gilt nicht als belegt.
/// Windows unterscheidet Gross- und Kleinschreibung nicht, darum vergleicht
/// canonicalize die echten Ordner statt der Schreibweisen.
pub(crate) fn freier_org_name(wurzel: &Path, name: &str, eigen: Option<&str>) -> Result<String, String> {
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
    marke_setzen(&wurzel.join(&frei), None)?;
    Ok(frei)
}

/// Legt den Ordner einer Organisation an und gibt den vergebenen Namen zurueck.
/// Das Frontend speichert ihn im Tresor (organisation.ordner).
#[tauri::command]
pub fn org_ordner_anlegen(app: tauri::AppHandle, name: String) -> Result<String, String> {
    org_ordner_neu(&wurzel(&app)?, &name)
}

/// Findet den Ordner der Organisation `id` (siehe eigener_ordner), bringt ihn
/// auf ihren Namen, stellt sicher, dass es ihn gibt, und schreibt ihre ID in
/// die Marke. Passt alles schon, passiert nichts. Gibt den Namen zurueck, der
/// danach gilt – das Frontend speichert ihn im Tresor. Scheitert das
/// Umbenennen (Datei offen), bleibt alles, wie es war.
fn org_ordner_abgleichen(wurzel: &Path, id: &str, alt: &str, name: &str) -> Result<String, String> {
    if !crate::tresor::id_gueltig(id) {
        return Err("Unbekannte Organisation.".into());
    }
    let eigen = eigener_ordner(wurzel, id, alt, name)?;
    let neu = freier_org_name(wurzel, name, Some(&eigen))?;
    if neu != eigen {
        ordner_verschieben(&wurzel.join(&eigen), &wurzel.join(&neu))?;
    }
    marke_setzen(&wurzel.join(&neu), Some(id))?;
    Ok(neu)
}

#[tauri::command]
pub fn org_ordner_umbenennen(
    app: tauri::AppHandle,
    org: String,
    alt: String,
    name: String,
) -> Result<String, String> {
    org_ordner_abgleichen(&wurzel(&app)?, &org, &alt, &name)
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
///
/// Ziel ist nur ein Ordner, dessen Marke schon die ID der Organisation traegt.
/// Der Name allein reicht nicht: Nach dem Einspielen einer Sicherung kann ein
/// gleichnamiger Ordner einer ANDEREN Organisation gehoeren. Darum beansprucht
/// das Frontend den Ordner zuerst (org_ordner_umbenennen) und zieht erst dann um.
fn projekte_umziehen(
    wurzel: &Path,
    id: &str,
    org_ordner: &str,
    projekte: &[String],
    verschieben: impl Fn(&Path, &Path) -> std::io::Result<()>,
) -> Result<OrdnerUmzug, String> {
    if org_ordner.is_empty() {
        return Err("Die Organisation hat noch keinen Ordner.".into());
    }
    let ziel = org_pfad(wurzel, org_ordner)?;
    if !crate::tresor::id_gueltig(id) || besitz(&ziel) != Besitz::Id(id.to_string()) {
        return Err("Der Ordner gehoert (noch) nicht dieser Organisation.".into());
    }

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
    org: String,
    org_ordner: String,
    projekte: Vec<String>,
) -> Result<OrdnerUmzug, String> {
    projekte_umziehen(&wurzel(&app)?, &org, &org_ordner, &projekte, |v, n| fs::rename(v, n))
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

    const ID_A: &str = "aaaaaaaaaaaaaaaa";
    const ID_B: &str = "bbbbbbbbbbbbbbbb";

    #[test]
    fn abgleich_benennt_um_und_trifft_keinen_fremden_ordner() {
        let w = testordner("abgleich");
        let a = org_ordner_neu(&w, "Alt").unwrap();
        fs::create_dir(w.join("Alt").join("Sommerfest")).unwrap();
        assert_eq!(org_ordner_abgleichen(&w, ID_A, &a, "Neu").unwrap(), "Neu");
        assert!(w.join("Neu").join("Sommerfest").exists());
        assert!(!w.join("Alt").exists());
        assert_eq!(besitz(&w.join("Neu")), Besitz::Id(ID_A.into()));
        // Name schon vergeben: naechster freier Name, der fremde bleibt, wie er ist.
        org_ordner_neu(&w, "Belegt").unwrap();
        assert_eq!(org_ordner_abgleichen(&w, ID_A, "Neu", "Belegt").unwrap(), "Belegt (2)");
        assert!(!w.join("Belegt").join("Sommerfest").exists());
        // Nichts zu tun, wenn es schon stimmt (auch mit Nummer im Namen).
        assert_eq!(org_ordner_abgleichen(&w, ID_A, "Belegt (2)", "Belegt").unwrap(), "Belegt (2)");
        // Vom Nutzer geloeschter Ordner kommt wieder.
        fs::remove_dir_all(w.join("Belegt (2)")).unwrap();
        assert_eq!(org_ordner_abgleichen(&w, ID_A, "Belegt (2)", "Wieder").unwrap(), "Wieder");
        assert_eq!(besitz(&w.join("Wieder")), Besitz::Id(ID_A.into()));
        // Ungueltige ID wird abgewiesen.
        assert!(org_ordner_abgleichen(&w, "../x", "Wieder", "Wieder").is_err());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn zwei_organisationen_mit_demselben_ordner_bekommen_je_einen_eigenen() {
        // Lage nach dem Einspielen einer Kopie: Beide Tresore sagen "Verein".
        let w = testordner("doppelt");
        let v = org_ordner_neu(&w, "Verein").unwrap();
        fs::create_dir(w.join("Verein").join("Sommerfest")).unwrap();
        // A oeffnet zuerst und beansprucht den Ordner.
        assert_eq!(org_ordner_abgleichen(&w, ID_A, &v, "Verein").unwrap(), "Verein");
        // B bekommt einen eigenen, leeren Ordner – A behaelt seine Dateien.
        let b = org_ordner_abgleichen(&w, ID_B, &v, "Verein").unwrap();
        assert_eq!(b, "Verein (2)");
        assert_eq!(besitz(&w.join("Verein (2)")), Besitz::Id(ID_B.into()));
        assert!(w.join("Verein").join("Sommerfest").exists());
        // Benennt B sich um, bleibt der Ordner von A unberuehrt.
        assert_eq!(org_ordner_abgleichen(&w, ID_B, &v, "Duo").unwrap(), "Duo");
        assert!(w.join("Verein").join("Sommerfest").exists());
        assert_eq!(besitz(&w.join("Verein")), Besitz::Id(ID_A.into()));
        assert!(!w.join("Verein (2)").exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn ordner_wird_ueber_die_id_gefunden_auch_unter_anderem_namen() {
        // Eine Sicherung wurde unter freiem Namen eingespielt: der Tresor sagt
        // noch "Verein", seine Dateien liegen in "Verein (2)" mit seiner ID.
        let w = testordner("finden");
        let v = org_ordner_neu(&w, "Verein").unwrap();
        org_ordner_abgleichen(&w, ID_A, &v, "Verein").unwrap();
        marke_setzen(&w.join("Verein (2)"), Some(ID_B)).unwrap();
        fs::create_dir(w.join("Verein (2)").join("Festival")).unwrap();
        // Name passt nicht mehr ganz – der Ordner von B heisst danach wie B.
        assert_eq!(org_ordner_abgleichen(&w, ID_B, "Verein", "Verein").unwrap(), "Verein (2)");
        assert_eq!(org_ordner_abgleichen(&w, ID_B, "Verein", "Duo Lichtung").unwrap(), "Duo Lichtung");
        assert!(w.join("Duo Lichtung").join("Festival").exists());
        assert_eq!(besitz(&w.join("Verein")), Besitz::Id(ID_A.into()));
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn fremde_id_in_der_marke_wird_nie_ueberschrieben() {
        let w = testordner("marke");
        marke_setzen(&w.join("X"), Some(ID_A)).unwrap();
        marke_setzen(&w.join("X"), Some(ID_B)).unwrap();
        assert_eq!(besitz(&w.join("X")), Besitz::Id(ID_A.into()));
        // Marke ohne ID wird beansprucht, kaputte ID zaehlt als "ohne".
        fs::create_dir_all(w.join("Y")).unwrap();
        fs::write(w.join("Y").join(MARKE), "id=../../etc\n").unwrap();
        assert_eq!(besitz(&w.join("Y")), Besitz::OhneId);
        marke_setzen(&w.join("Y"), Some(ID_B)).unwrap();
        assert_eq!(besitz(&w.join("Y")), Besitz::Id(ID_B.into()));
        assert_eq!(besitz(&w.join("gibt-es-nicht")), Besitz::KeineMarke);
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_bewegt_nur_bekannte_projekte_und_darf_wiederholt_laufen() {
        let w = testordner("umzug");
        fs::create_dir(w.join("Sommerfest")).unwrap();
        fs::write(w.join("Sommerfest").join("a.pdf"), "x").unwrap();
        fs::create_dir(w.join("Fremd")).unwrap();
        let name = org_ordner_neu(&w, "Mira Kern").unwrap();
        marke_setzen(&w.join(&name), Some(ID_A)).unwrap();
        let projekte = liste(&["Sommerfest", "Noch ohne Ordner"]);

        let b = projekte_umziehen(&w, ID_A, &name, &projekte, normal).unwrap();
        assert_eq!(b.verschoben, 1);
        assert!(b.uebrig.is_empty());
        assert!(w.join("Mira Kern").join("Sommerfest").join("a.pdf").exists());
        assert!(!w.join("Sommerfest").exists());
        assert!(w.join("Fremd").exists(), "nicht zugeordnete Ordner bleiben");

        let b = projekte_umziehen(&w, ID_A, &name, &projekte, normal).unwrap();
        assert_eq!(b.verschoben, 0, "zweiter Lauf hat nichts mehr zu tun");
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_nimmt_nie_den_ordner_einer_anderen_organisation_mit() {
        let w = testordner("marke");
        let verein = org_ordner_neu(&w, "Verein").unwrap();
        fs::write(w.join("Verein").join("geheim.pdf"), "x").unwrap();
        let ich = org_ordner_neu(&w, "Ich").unwrap();
        marke_setzen(&w.join(&ich), Some(ID_A)).unwrap();
        let b = projekte_umziehen(&w, ID_A, &ich, &liste(&[&verein]), normal).unwrap();
        assert_eq!(b.verschoben, 0);
        assert_eq!(b.uebrig, vec!["Verein".to_string()]);
        assert!(w.join("Verein").join("geheim.pdf").exists());
        assert!(!w.join("Ich").join("Verein").exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_nur_in_den_eigenen_beanspruchten_ordner() {
        let w = testordner("umzug-besitz");
        fs::create_dir(w.join("Fest")).unwrap();
        fs::write(w.join("Fest").join("a.pdf"), "x").unwrap();
        // Gleichnamiger Ordner gehoert einer anderen Organisation (z. B. nach
        // dem Einspielen einer Sicherung): nichts darf dort hinein.
        let verein = org_ordner_neu(&w, "Verein").unwrap();
        marke_setzen(&w.join(&verein), Some(ID_B)).unwrap();
        assert!(projekte_umziehen(&w, ID_A, &verein, &liste(&["Fest"]), normal).is_err());
        // Noch nicht beanspruchter oder fehlender Ordner: ebenfalls nicht.
        let frei = org_ordner_neu(&w, "Frei").unwrap();
        assert!(projekte_umziehen(&w, ID_A, &frei, &liste(&["Fest"]), normal).is_err());
        assert!(projekte_umziehen(&w, ID_A, "Gibt es nicht", &liste(&["Fest"]), normal).is_err());
        assert!(projekte_umziehen(&w, "../x", &frei, &liste(&["Fest"]), normal).is_err());
        assert!(w.join("Fest").join("a.pdf").exists(), "alles blieb am alten Platz");
        assert!(!w.join("Gibt es nicht").exists(), "es wurde nichts angelegt");
        // Erst nach dem Beanspruchen klappt es.
        marke_setzen(&w.join(&frei), Some(ID_A)).unwrap();
        let b = projekte_umziehen(&w, ID_A, &frei, &liste(&["Fest"]), normal).unwrap();
        assert_eq!(b.verschoben, 1);
        assert!(w.join("Frei").join("Fest").join("a.pdf").exists());
        let _ = fs::remove_dir_all(&w);
    }

    #[test]
    fn umzug_mischt_nichts_zusammen() {
        let w = testordner("konflikt");
        let ich = org_ordner_neu(&w, "Ich").unwrap();
        marke_setzen(&w.join(&ich), Some(ID_A)).unwrap();
        fs::create_dir(w.join("Ich").join("Fest")).unwrap();
        fs::write(w.join("Ich").join("Fest").join("neu.pdf"), "neu").unwrap();
        fs::create_dir(w.join("Fest")).unwrap();
        fs::write(w.join("Fest").join("alt.pdf"), "alt").unwrap();
        let b = projekte_umziehen(&w, ID_A, &ich, &liste(&["Fest"]), normal).unwrap();
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
        marke_setzen(&w.join(&ich), Some(ID_A)).unwrap();
        // "Drei" laesst sich nicht bewegen, als waere darin eine Datei offen.
        let klemmt = |v: &Path, n: &Path| {
            if v.ends_with("Drei") {
                Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "offen"))
            } else {
                fs::rename(v, n)
            }
        };
        let fehler = projekte_umziehen(&w, ID_A, &ich, &liste(&["Eins", "Zwei", "Drei"]), klemmt).unwrap_err();
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
