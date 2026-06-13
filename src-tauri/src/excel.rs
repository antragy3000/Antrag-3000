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

    let mut row: u32 = 0;
    let _ = blatt.write_string_with_format(row, 0, &projekt, &titel_fmt);
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
         kategorien: &[XKategorie],
         gesamt_text: &str,
         gesamt: f64|
         -> u32 {
            let mut row = start;
            let _ = blatt.write_string_with_format(row, 0, "Nr.", &kopf_fmt);
            let _ = blatt.write_string_with_format(row, 1, titel, &kopf_fmt);
            let _ = blatt.write_string_with_format(row, 2, spalte2, &kopf_fmt);
            let _ = blatt.write_string_with_format(row, 3, "Betrag", &kopf_fmt);
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
        .save(&pfad)
        .map_err(|e| format!("Excel-Datei nicht schreibbar: {e}"))?;

    Ok(pfad.to_string_lossy().to_string())
}
