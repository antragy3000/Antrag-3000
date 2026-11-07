// ============================================================
// Kostenfinanzplan als Excel-Datei (.xlsx) im Projektordner.
//
// Wird bei JEDEM Speichern des Kostenplans neu geschrieben, damit im
// allgemeinen Projektordner (Dokumente\Antrag 3000\[Projekt]\) immer
// ein aktueller Stand liegt - unabhaengig von der Word-Erzeugung,
// die pro Foerderung in den jeweiligen Unterordner schreibt.
//
// Wie beim Word bleibt Rust "dumm": Es bekommt die fertig
// ausgerechneten Zahlen (kfpExport im Frontend) und legt sie nur
// formatiert in die Tabelle.
//
// Hat das Projekt Bausteine (z. B. einzelne Konzerte einer Reihe), bekommt
// die Ausgaben-Seite eine zusaetzliche Spalte "Teil". Ohne Bausteine
// sieht die Datei aus wie bisher.
// ============================================================

use rust_xlsxwriter::{Color, Format, Workbook};
use serde::Deserialize;

use crate::ordner;

#[derive(Deserialize)]
pub struct XPosten {
    pub nummer: String,
    pub bezeichnung: String,
    #[serde(default)]
    pub erlaeuterung: String,
    pub betrag: f64,
    // Name des Bausteins oder "gemeinsam"; fehlt bei aelteren Staenden.
    #[serde(default)]
    pub baustein: String,
}

#[derive(Deserialize)]
pub struct XKategorie {
    pub nummer: String,
    pub name: String,
    pub summe: f64,
    pub posten: Vec<XPosten>,
}

#[derive(Deserialize)]
pub struct XKfp {
    pub kosten: Vec<XKategorie>,
    pub finanzierung: Vec<XKategorie>,
    pub summe_kosten: f64,
    pub summe_finanzierung: f64,
    pub differenz: f64,
    #[serde(default)]
    pub mit_bausteinen: bool,
}

#[tauri::command]
pub fn kfp_excel_schreiben(
    app: tauri::AppHandle,
    projekt: String,
    kfp: XKfp,
) -> Result<String, String> {
    let ordner_pfad = ordner::wurzel(&app)?.join(ordner::bereinigen(&projekt)?);
    std::fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let pfad = ordner_pfad.join("Kostenfinanzplan.xlsx");

    arbeitsmappe_bauen(&projekt, &kfp)
        .save(&pfad)
        .map_err(|e| format!("Excel-Datei nicht schreibbar: {e}"))?;

    Ok(pfad.to_string_lossy().to_string())
}

/// Baut die Tabelle im Speicher. Eigene Funktion, damit ein Test sie ohne
/// laufende App pruefen kann.
fn arbeitsmappe_bauen(projekt: &str, kfp: &XKfp) -> Workbook {
    let mut workbook = Workbook::new();
    let blatt = workbook.add_worksheet();
    let _ = blatt.set_name("Kostenfinanzplan");

    // Formate
    let titel_fmt = Format::new().set_bold().set_font_size(14.0);
    let unter_fmt = Format::new()
        .set_italic()
        .set_font_color(Color::RGB(0x8590A2));
    let kopf_fmt = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xEEF1FF))
        .set_font_color(Color::RGB(0x172B4D));
    let kat_fmt = Format::new().set_bold();
    let geld_fmt = Format::new().set_num_format("#,##0.00 €");
    let kat_geld_fmt = Format::new().set_bold().set_num_format("#,##0.00 €");

    let _ = blatt.set_column_width(0, 6.0);
    let _ = blatt.set_column_width(1, 42.0);
    let _ = blatt.set_column_width(2, 30.0);
    let _ = blatt.set_column_width(3, 16.0);
    if kfp.mit_bausteinen {
        let _ = blatt.set_column_width(4, 22.0);
    }

    let mut row: u32 = 0;
    let _ = blatt.write_string_with_format(row, 0, projekt, &titel_fmt);
    row += 1;
    let _ = blatt.write_string_with_format(
        row,
        0,
        "Kostenfinanzplan – automatisch aktualisiert beim Speichern",
        &unter_fmt,
    );
    row += 2;

    // Eine Seite (Ausgaben oder Finanzierung) schreiben.
    let seite_schreiben =
        |blatt: &mut rust_xlsxwriter::Worksheet,
         start: u32,
         titel: &str,
         spalte2: &str,
         mit_erlaeuterung: bool,
         mit_baustein: bool,
         kategorien: &[XKategorie],
         gesamt_text: &str,
         gesamt: f64|
         -> u32 {
            let mut row = start;
            let _ = blatt.write_string_with_format(row, 0, "Nr.", &kopf_fmt);
            let _ = blatt.write_string_with_format(row, 1, titel, &kopf_fmt);
            let _ = blatt.write_string_with_format(row, 2, spalte2, &kopf_fmt);
            let _ = blatt.write_string_with_format(row, 3, "Betrag", &kopf_fmt);
            if mit_baustein {
                let _ = blatt.write_string_with_format(row, 4, "Teil", &kopf_fmt);
            }
            row += 1;
            for kat in kategorien {
                let _ = blatt.write_string_with_format(row, 0, &kat.nummer, &kat_fmt);
                let _ = blatt.write_string_with_format(row, 1, &kat.name, &kat_fmt);
                let _ = blatt.write_number_with_format(row, 3, kat.summe, &kat_geld_fmt);
                row += 1;
                for p in &kat.posten {
                    let _ = blatt.write_string(row, 0, &p.nummer);
                    let _ = blatt.write_string(row, 1, &p.bezeichnung);
                    if mit_erlaeuterung {
                        let _ = blatt.write_string(row, 2, &p.erlaeuterung);
                    }
                    let _ = blatt.write_number_with_format(row, 3, p.betrag, &geld_fmt);
                    if mit_baustein {
                        let _ = blatt.write_string(row, 4, &p.baustein);
                    }
                    row += 1;
                }
            }
            let _ = blatt.write_string_with_format(row, 1, gesamt_text, &kat_fmt);
            let _ = blatt.write_number_with_format(row, 3, gesamt, &kat_geld_fmt);
            row + 2
        };

    row = seite_schreiben(
        blatt,
        row,
        "Ausgaben",
        "Erläuterung",
        true,
        kfp.mit_bausteinen,
        &kfp.kosten,
        "Gesamtkosten",
        kfp.summe_kosten,
    );
    row = seite_schreiben(
        blatt,
        row,
        "Finanzierung",
        "",
        false,
        false,
        &kfp.finanzierung,
        "Gesamtfinanzierung",
        kfp.summe_finanzierung,
    );

    // Bilanz
    let bilanz_text = if kfp.differenz.abs() < 0.005 {
        "Ausgeglichen"
    } else if kfp.differenz < 0.0 {
        "Fehlbedarf"
    } else {
        "Überschuss"
    };
    let _ = blatt.write_string_with_format(row, 1, bilanz_text, &kat_fmt);
    let _ = blatt.write_number_with_format(row, 3, kfp.differenz, &kat_geld_fmt);

    workbook
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kfp_aus(json: &str) -> XKfp {
        serde_json::from_str(json).expect("KFP muss sich lesen lassen")
    }

    // Aeltere Staende ohne Baustein-Felder muessen weiter funktionieren, mit
    // Bausteinen kommt die Spalte dazu. Die Dateien landen zusaetzlich im
    // Temp-Ordner (antrag3000-kfp-test-*.xlsx), falls man sie ansehen will.
    #[test]
    fn excel_mit_und_ohne_bausteine() {
        let ohne = kfp_aus(
            r#"{"kosten":[{"nummer":"1","name":"Honorare","summe":300,
                "posten":[{"nummer":"1.1","bezeichnung":"Gage","betrag":300}]}],
               "finanzierung":[],"summe_kosten":300,"summe_finanzierung":0,"differenz":-300}"#,
        );
        assert!(!ohne.mit_bausteinen);
        let mit = kfp_aus(
            r#"{"kosten":[{"nummer":"1","name":"Honorare","summe":3000,
                "posten":[{"nummer":"1.1","bezeichnung":"Gage März","betrag":1000,"baustein":"Konzert März"},
                          {"nummer":"1.2","bezeichnung":"Plakate","betrag":2000,"baustein":"gemeinsam"}]}],
               "finanzierung":[],"summe_kosten":3000,"summe_finanzierung":0,"differenz":-3000,
               "mit_bausteinen":true}"#,
        );
        for (name, kfp) in [("ohne", &ohne), ("mit", &mit)] {
            let bytes = arbeitsmappe_bauen("Test – Reihe", kfp)
                .save_to_buffer()
                .expect("Excel-Datei muss sich bauen lassen");
            assert!(bytes.starts_with(b"PK\x03\x04"), "keine ZIP-Datei");
            let _ = std::fs::write(
                std::env::temp_dir().join(format!("antrag3000-kfp-test-{name}.xlsx")),
                &bytes,
            );
        }
    }
}
