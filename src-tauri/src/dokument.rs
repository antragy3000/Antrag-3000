// ============================================================
// Antrags-Dokumente erzeugen (CLAUDE.md):
//
// Pro Foerderung entstehen im Ordner
//   Dokumente\Antrag 3000\[Projekt]\[Foerderung]\
// zwei Dateien:
//   1. antworten.json  - maschinenlesbar, Quelle der Wahrheit
//   2. Antrag - ....docx - menschenlesbare Kopie mit Warnhinweis
//      am Dateianfang (Aenderungen dort fliessen NICHT zurueck)
//
// Der Rust-Kern bleibt dumm: Er bekommt fertige Abschnitte
// (Ueberschrift + Absaetze) vom Frontend und setzt sie nur in eine
// Word-Datei um. WAS im Antrag steht, entscheidet das Frontend.
// ============================================================

use std::fs;

use docx_rs::{AlignmentType, Docx, Paragraph, Run, Table, TableCell, TableRow};
use serde::Deserialize;

use crate::ordner;

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

/// Schreibt antworten.json und die Word-Datei in den
/// Foerderungs-Ordner und oeffnet diesen im Explorer.
#[tauri::command]
pub fn antrag_erzeugen(
    app: tauri::AppHandle,
    projekt: String,
    foerderung: String,
    titel: String,
    warnhinweis: String,
    abschnitte: Vec<DocAbschnitt>,
    antworten_json: String,
) -> Result<String, String> {
    let ordner_pfad = ordner::wurzel(&app)?
        .join(ordner::bereinigen(&projekt)?)
        .join(ordner::bereinigen(&foerderung)?);
    fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;

    // 1. Maschinenlesbare Quelle der Wahrheit.
    let json_pfad = ordner_pfad.join("antworten.json");
    fs::write(&json_pfad, antworten_json.as_bytes())
        .map_err(|e| format!("antworten.json nicht schreibbar: {e}"))?;

    // 2. Menschenlesbare Word-Datei.
    let mut docx = Docx::new();

    // Warnhinweis ganz oben, deutlich abgesetzt (rot, fett).
    for zeile in warnhinweis.lines() {
        docx = docx.add_paragraph(
            Paragraph::new().add_run(
                Run::new()
                    .add_text(zeile)
                    .bold()
                    .color("C0392B")
                    .size(18), // 9 pt (Angabe in Halbpunkten)
            ),
        );
    }
    docx = docx.add_paragraph(Paragraph::new());

    // Titel.
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Left)
            .add_run(Run::new().add_text(&titel).bold().size(36)), // 18 pt
    );
    docx = docx.add_paragraph(Paragraph::new());

    // Inhaltliche Abschnitte.
    for abschnitt in &abschnitte {
        docx = docx.add_paragraph(
            Paragraph::new().add_run(
                Run::new()
                    .add_text(&abschnitt.ueberschrift)
                    .bold()
                    .size(26), // 13 pt
            ),
        );
        for absatz in &abschnitt.absaetze {
            // Mehrzeilige Texte: jede Zeile als eigener Absatz,
            // damit Zeilenumbrueche aus dem Formular erhalten bleiben.
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
                .map(|(nr, zellen)| {
                    TableRow::new(
                        zellen.iter().map(|t| zelle(t, nr == 0)).collect(),
                    )
                })
                .collect();
            docx = docx.add_table(Table::new(zeilen));
        }
        docx = docx.add_paragraph(Paragraph::new());
    }

    let docx_name = ordner::bereinigen(&format!("Antrag - {foerderung}"))? + ".docx";
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
