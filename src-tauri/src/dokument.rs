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

use docx_rs::{AlignmentType, Docx, Paragraph, Run};
use serde::Deserialize;

use crate::ordner;

#[derive(Deserialize)]
pub struct DocAbschnitt {
    pub ueberschrift: String,
    pub absaetze: Vec<String>,
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
