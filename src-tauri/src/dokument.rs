// ============================================================
// Formular-Word erzeugen (CLAUDE.md):
//
// Aus dem Sammel-Formular entsteht im PROJEKT-Ordner
//   Dokumente\Antrag 3000\[Projekt]\Projektbeschrieb - [Projekt].docx
// eine menschenlesbare Kopie mit Warnhinweis am Dateianfang
// (Aenderungen dort fliessen NICHT zurueck). Bewusst OHNE Stammdaten
// und OHNE Kostenfinanzplan - diese sensiblen Daten sollen nicht
// unverschluesselt in der Datei stehen.
//
// Der Rust-Kern bleibt dumm: Er bekommt fertige Abschnitte
// (Ueberschrift + Absaetze) vom Frontend und setzt sie nur in eine
// Word-Datei um. WAS im Word steht, entscheidet das Frontend.
// ============================================================

use std::fs;

use docx_rs::{AlignmentType, Docx, Paragraph, Pic, Run, Table, TableCell, TableRow};
use serde::Deserialize;

use crate::ordner;
use crate::pdf::logo_bytes;

/// Baut aus dem Stammdaten-Logo einen Briefkopf-Absatz (Bild oben links),
/// auf höchstens 5 cm Breite / 2,5 cm Höhe skaliert (Seitenverhältnis
/// bleibt). None, wenn kein/unlesbares Logo. EMU = English Metric Units,
/// 360000 pro cm.
fn briefkopf_absatz(logo: Option<&str>) -> Option<Paragraph> {
    let daten = logo.and_then(logo_bytes)?;
    let img = image::load_from_memory(&daten).ok()?;
    let (w_px, h_px) = image::GenericImageView::dimensions(&img);
    if w_px == 0 || h_px == 0 {
        return None;
    }
    let max_w = 1_800_000u64; // 5 cm
    let max_h = 900_000u64; // 2,5 cm
    let aspect = w_px as f64 / h_px as f64;
    let mut w = max_w;
    let mut h = (max_w as f64 / aspect) as u64;
    if h > max_h {
        h = max_h;
        w = (max_h as f64 * aspect) as u64;
    }
    let pic = Pic::new(&daten).size(w as u32, h as u32);
    Some(Paragraph::new().add_run(Run::new().add_image(pic)))
}

#[derive(Deserialize)]
pub struct DocAbschnitt {
    pub ueberschrift: String,
    #[serde(default)]
    pub absaetze: Vec<String>,
    /// Optionale Tabelle: Zeilen aus Zellen. Die erste Zeile wird als
    /// Kopfzeile fett gesetzt; Zellen, deren Text mit ** beginnt,
    /// ebenfalls (Markierung wird entfernt) - so kann das Frontend
    /// z. B. Kategorie- und Summenzeilen hervorheben.
    #[serde(default)]
    pub tabelle: Vec<Vec<String>>,
}

/// Eine Tabellenzelle bauen; ** am Anfang bedeutet fett.
fn zelle(text: &str, fett: bool) -> TableCell {
    let (inhalt, fett) = match text.strip_prefix("**") {
        Some(rest) => (rest, true),
        None => (text, fett),
    };
    let mut run = Run::new().add_text(inhalt).size(20); // 10 pt
    if fett {
        run = run.bold();
    }
    TableCell::new().add_paragraph(Paragraph::new().add_run(run))
}

/// Baut ein Word-Dokument aus Titel, optionalem (rotem) Warnhinweis und
/// Abschnitten (Ueberschrift + Absaetze + Tabelle). Gemeinsam genutzt von
/// der Formular-Kopie und dem Verwendungsnachweis (Abrechnung).
pub(crate) fn docx_bauen(
    titel: &str,
    warnhinweis: &str,
    abschnitte: &[DocAbschnitt],
    logo: Option<&str>,
) -> Docx {
    let mut docx = Docx::new();

    // Briefkopf (Logo) ganz oben, falls vorhanden.
    if let Some(absatz) = briefkopf_absatz(logo) {
        docx = docx.add_paragraph(absatz);
    }

    // Warnhinweis ganz oben, deutlich abgesetzt (rot, fett).
    for zeile in warnhinweis.lines() {
        docx = docx.add_paragraph(
            Paragraph::new().add_run(
                Run::new().add_text(zeile).bold().color("C0392B").size(18), // 9 pt
            ),
        );
    }
    docx = docx.add_paragraph(Paragraph::new());

    // Titel.
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Left)
            .add_run(Run::new().add_text(titel).bold().size(36)), // 18 pt
    );
    docx = docx.add_paragraph(Paragraph::new());

    // Inhaltliche Abschnitte.
    for abschnitt in abschnitte {
        docx = docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_text(&abschnitt.ueberschrift).bold().size(26)),
        );
        for absatz in &abschnitt.absaetze {
            for zeile in absatz.lines() {
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text(zeile).size(22)), // 11 pt
                );
            }
            if absatz.lines().count() == 0 {
                docx = docx.add_paragraph(Paragraph::new());
            }
        }
        if !abschnitt.tabelle.is_empty() {
            let zeilen: Vec<TableRow> = abschnitt
                .tabelle
                .iter()
                .enumerate()
                .map(|(nr, zellen)| TableRow::new(zellen.iter().map(|t| zelle(t, nr == 0)).collect()))
                .collect();
            docx = docx.add_table(Table::new(zeilen));
        }
        docx = docx.add_paragraph(Paragraph::new());
    }
    docx
}

/// Schreibt die Word-Datei in den PROJEKT-Ordner und oeffnet diesen
/// im Explorer. Ohne antworten.json, ohne Foerderungs-Unterordner.
#[tauri::command]
pub fn formular_word_erzeugen(
    app: tauri::AppHandle,
    projekt: String,
    titel: String,
    warnhinweis: String,
    abschnitte: Vec<DocAbschnitt>,
    logo: Option<String>,
) -> Result<String, String> {
    let ordner_pfad = ordner::wurzel(&app)?.join(ordner::bereinigen(&projekt)?);
    fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;

    // Menschenlesbare Word-Datei (gemeinsamer Bau-Helfer).
    let docx = docx_bauen(&titel, &warnhinweis, &abschnitte, logo.as_deref());

    let docx_name = ordner::bereinigen(&format!("Projektbeschrieb - {projekt}"))? + ".docx";
    let docx_pfad = ordner_pfad.join(&docx_name);
    let datei = fs::File::create(&docx_pfad)
        .map_err(|e| format!("Word-Datei nicht anlegbar: {e}"))?;
    docx.build()
        .pack(datei)
        .map_err(|e| format!("Word-Datei nicht schreibbar: {e}"))?;

    // Ergebnis direkt zeigen.
    tauri_plugin_opener::open_path(ordner_pfad.clone(), None::<&str>)
        .map_err(|e| format!("Ordner laesst sich nicht oeffnen: {e}"))?;

    Ok(docx_pfad.to_string_lossy().to_string())
}

/// Kopiert eine vom Nutzer gewaehlte Datei (PDF oder Bild) in den
/// Unterordner "Dateien" der Foerderung und benennt sie einheitlich:
///   [Dokumentart]_Antrag_[Projekt].[ext]
/// Eine bereits vorhandene Datei gleichen Namens wird ueberschrieben
/// (so ersetzt ein erneutes Hochladen die alte). Gibt den neuen
/// Dateinamen (ohne Pfad) zurueck, den das Frontend im Tresor merkt.
#[tauri::command]
pub fn dokument_hochladen(
    app: tauri::AppHandle,
    projekt: String,
    foerderung: String,
    dokumentart: String,
    quelle: String,
) -> Result<String, String> {
    let quell_pfad = std::path::Path::new(&quelle);
    let ext = quell_pfad
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    const ERLAUBT: [&str; 4] = ["pdf", "jpg", "jpeg", "png"];
    if !ERLAUBT.contains(&ext.as_str()) {
        return Err("Nur PDF-, JPG- oder PNG-Dateien sind erlaubt.".into());
    }

    let dateien_ordner = ordner::wurzel(&app)?
        .join(ordner::bereinigen(&projekt)?)
        .join(ordner::bereinigen(&foerderung)?)
        .join("Dateien");
    fs::create_dir_all(&dateien_ordner).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;

    let basis = ordner::bereinigen(&format!("{}_Antrag_{}", dokumentart.trim(), projekt.trim()))?;
    let datei_name = format!("{basis}.{ext}");
    let ziel = dateien_ordner.join(&datei_name);

    fs::copy(quell_pfad, &ziel).map_err(|e| format!("Datei nicht kopierbar: {e}"))?;
    Ok(datei_name)
}

/// Verwendungsnachweis (Abrechnung) als Word: Titel + Abschnitte (Sachbericht,
/// Belegliste, Kostenuebersicht) werden in den Unterordner _Abrechnung des
/// Projekts geschrieben und die Datei geoeffnet. Kein Warnhinweis (es ist ein
/// fertiges Dokument, keine Entwurfs-Kopie).
#[tauri::command]
pub fn verwendungsnachweis_word(
    app: tauri::AppHandle,
    projekt: String,
    foerderer: String,
    titel: String,
    abschnitte: Vec<DocAbschnitt>,
    logo: Option<String>,
) -> Result<String, String> {
    let ordner_pfad = ordner::wurzel(&app)?
        .join(ordner::bereinigen(&projekt)?)
        .join("_Abrechnung");
    fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;

    let docx = docx_bauen(&titel, "", &abschnitte, logo.as_deref());
    let name =
        ordner::bereinigen(&format!("Verwendungsnachweis_{}_{}", projekt.trim(), foerderer.trim()))?
            + ".docx";
    let pfad = ordner_pfad.join(&name);
    let datei = fs::File::create(&pfad).map_err(|e| format!("Word-Datei nicht anlegbar: {e}"))?;
    docx.build()
        .pack(datei)
        .map_err(|e| format!("Word-Datei nicht schreibbar: {e}"))?;

    tauri_plugin_opener::open_path(pfad.clone(), None::<&str>)
        .map_err(|e| format!("Datei laesst sich nicht oeffnen: {e}"))?;
    Ok(pfad.to_string_lossy().to_string())
}
