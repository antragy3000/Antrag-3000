// ============================================================
// Antrags-PDF erzeugen (CLAUDE.md):
//
// Setzt eine PDF zusammen aus:
//   1. Stammblatt (Stammdaten)
//   2. Daten aus dem Formular
//   3. Kostenfinanzplan
//   4. Anhang-Liste (Titel der benoetigten Dokumente)
//   5. die hochgeladenen Dokumente in Reihenfolge der Anhang-Liste
//
// Der Rust-Kern bekommt fertige Abschnitte (Ueberschrift + Absaetze +
// Tabelle) vom Frontend und rendert nur. Die hochgeladenen Anhaenge
// (PDF/Bild) werden mit lopdf an das erzeugte Vorblatt angehaengt.
//
// Schrift: Open Sans ist eingebettet (Umlaute), kein externes Programm.
// ============================================================

use std::collections::BTreeMap;
use std::fs;

use base64::Engine;
use genpdf::{elements, fonts, style, Alignment, Document, Element, SimplePageDecorator};
use image::{DynamicImage, GenericImageView};
use lopdf::{Dictionary, Document as LoDocument, Object, ObjectId};
use serde::Deserialize;

use crate::ordner;

/// Dekodiert ein als Data-URL oder reines base64 übergebenes Logo zu
/// Bytes. None, wenn leer oder unlesbar.
pub fn logo_bytes(daten: &str) -> Option<Vec<u8>> {
    let s = daten.trim();
    if s.is_empty() {
        return None;
    }
    let b64 = s.split_once(";base64,").map(|(_, b)| b).unwrap_or(s);
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()
        .filter(|v| !v.is_empty())
}

const FONT_REGULAR: &[u8] = include_bytes!("../fonts/OpenSans-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../fonts/OpenSans-Bold.ttf");

/// Ein Abschnitt des Vorblatts: Ueberschrift, Absaetze und/oder eine
/// Tabelle. In Tabellenzellen bedeutet ** am Anfang fett (Kategorie-/
/// Summenzeilen) - dieselbe Konvention wie bei der Word-Erzeugung.
#[derive(Deserialize)]
pub struct PdfAbschnitt {
    pub ueberschrift: String,
    #[serde(default)]
    pub absaetze: Vec<String>,
    #[serde(default)]
    pub tabelle: Vec<Vec<String>>,
}

// --- Schrift / Grunddokument -------------------------------------------

fn schrift(data: &[u8]) -> Result<fonts::FontData, String> {
    fonts::FontData::new(data.to_vec(), None).map_err(|e| format!("Schrift nicht ladbar: {e}"))
}

fn neues_dokument() -> Result<Document, String> {
    let familie = fonts::FontFamily {
        regular: schrift(FONT_REGULAR)?,
        bold: schrift(FONT_BOLD)?,
        italic: schrift(FONT_REGULAR)?,
        bold_italic: schrift(FONT_BOLD)?,
    };
    let mut doc = Document::new(familie);
    doc.set_font_size(10);
    let mut deko = SimplePageDecorator::new();
    deko.set_margins(18);
    doc.set_page_decorator(deko);
    Ok(doc)
}

// --- Vorblatt-Inhalt fuellen -------------------------------------------

fn tabelle_einfuegen(doc: &mut Document, zeilen: &[Vec<String>]) {
    let spalten = zeilen.iter().map(|z| z.len()).max().unwrap_or(0);
    if spalten == 0 {
        return;
    }
    let gewichte: Vec<usize> = match spalten {
        2 => vec![6, 2],
        3 => vec![5, 4, 2],
        // Belegliste des Verwendungsnachweises: Nr · Datum · Beleg ·
        // Kostenstelle · Belegsumme · Zugeordnet.
        6 => vec![2, 2, 5, 4, 3, 3],
        n => vec![1; n],
    };
    let mut tabelle = elements::TableLayout::new(gewichte);
    tabelle.set_cell_decorator(elements::FrameCellDecorator::new(true, true, false));

    for (zi, zeile) in zeilen.iter().enumerate() {
        let mut reihe = tabelle.row();
        for si in 0..spalten {
            let roh = zeile.get(si).map(|s| s.as_str()).unwrap_or("");
            let (text, fett) = match roh.strip_prefix("**") {
                Some(rest) => (rest, true),
                None => (roh, zi == 0),
            };
            let mut st = style::Style::new();
            if fett {
                st = st.bold();
            }
            reihe.push_element(elements::Paragraph::new(text).styled(st).padded(1));
        }
        let _ = reihe.push();
    }
    doc.push(tabelle);
}

/// Fügt das Logo als Briefkopf oben ein (links ausgerichtet, ca. 55 mm
/// breit / 28 mm hoch maximal). Fehler werden still übergangen – ein
/// kaputtes Logo darf das PDF nicht verhindern.
fn briefkopf_einfuegen(doc: &mut Document, logo: Option<&str>) {
    let Some(daten) = logo.and_then(logo_bytes) else { return };
    let Ok(img) = image::load_from_memory(&daten) else { return };
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return;
    }
    // DPI so wählen, dass das Bild in eine Briefkopf-Fläche passt
    // (max. 55 mm breit, 28 mm hoch). Größeres DPI = kleineres Bild.
    let breite_in = 55.0 / 25.4;
    let hoehe_in = 28.0 / 25.4;
    let dpi = (w as f64 / breite_in).max(h as f64 / hoehe_in).max(1.0);
    if let Ok(bild) = elements::Image::from_dynamic_image(flach_auf_weiss(&img)) {
        doc.push(bild.with_alignment(Alignment::Left).with_dpi(dpi));
        doc.push(elements::Break::new(0.6));
    }
}

fn vorblatt_fuellen(doc: &mut Document, titel: &str, abschnitte: &[PdfAbschnitt], logo: Option<&str>) {
    briefkopf_einfuegen(doc, logo);
    doc.push(
        elements::Paragraph::new(titel).styled(style::Style::new().bold().with_font_size(16)),
    );
    doc.push(elements::Break::new(1.0));

    for a in abschnitte {
        if !a.ueberschrift.is_empty() {
            doc.push(
                elements::Paragraph::new(a.ueberschrift.as_str())
                    .styled(style::Style::new().bold().with_font_size(12)),
            );
        }
        for absatz in &a.absaetze {
            for zeile in absatz.lines() {
                doc.push(elements::Paragraph::new(zeile));
            }
        }
        if !a.tabelle.is_empty() {
            tabelle_einfuegen(doc, &a.tabelle);
        }
        doc.push(elements::Break::new(0.6));
    }
}

// --- Bild -> einseitiges PDF -------------------------------------------

/// Legt ein Bild mit Transparenz auf einen weissen Hintergrund und gibt
/// ein RGB-Bild zurueck. Noetig, weil die PDF-Einbettung keine Bilder mit
/// Alphakanal unterstuetzt (PNG-Screenshots/Logos haben oft Transparenz).
fn flach_auf_weiss(img: &DynamicImage) -> DynamicImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut rgb = image::RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as f32 / 255.0;
        let misch = |c: u8| (c as f32 * a + 255.0 * (1.0 - a)).round() as u8;
        rgb.put_pixel(x, y, image::Rgb([misch(p[0]), misch(p[1]), misch(p[2])]));
    }
    DynamicImage::ImageRgb8(rgb)
}

fn bild_pdf(daten: Vec<u8>) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(&daten).map_err(|e| format!("Bild nicht lesbar: {e}"))?;
    let (w, h) = img.dimensions();

    // DPI so waehlen, dass das Bild in die Druckflaeche (A4 minus 18 mm
    // Rand ringsum) passt.
    let breite_in = (210.0 - 36.0) / 25.4;
    let hoehe_in = (297.0 - 36.0) / 25.4;
    let dpi = (w as f64 / breite_in)
        .max(h as f64 / hoehe_in)
        .max(1.0);

    let mut doc = neues_dokument()?;
    let bild = elements::Image::from_dynamic_image(flach_auf_weiss(&img))
        .map_err(|e| format!("Bild nicht ladbar: {e}"))?
        .with_alignment(Alignment::Center)
        .with_dpi(dpi);
    doc.push(bild);

    let mut out = Vec::new();
    doc.render(&mut out)
        .map_err(|e| format!("Bild-PDF nicht erzeugbar: {e}"))?;
    Ok(out)
}

// --- PDFs zusammenfuegen (lopdf) ---------------------------------------

fn typ_ist(d: &Dictionary, typ: &[u8]) -> bool {
    matches!(d.get(b"Type").and_then(|t| t.as_name()), Ok(n) if n == typ)
}

/// Fuegt mehrere PDF-Bloecke (das Vorblatt plus die Anhaenge) zu einer
/// einzigen PDF zusammen, indem die Seiten aneinandergehaengt werden.
fn zusammenfuegen(bloecke: Vec<Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut max_id = 1u32;
    let mut seiten: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut objekte: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut ziel = LoDocument::with_version("1.5");

    for block in &bloecke {
        let mut doc = LoDocument::load_mem(block).map_err(|e| format!("PDF nicht lesbar: {e}"))?;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;
        for (_, oid) in doc.get_pages() {
            if let Ok(obj) = doc.get_object(oid) {
                seiten.insert(oid, obj.to_owned());
            }
        }
        objekte.extend(doc.objects);
    }

    // Catalog und Pages-Wurzel des Ergebnisses bestimmen.
    let mut catalog: Option<(ObjectId, Dictionary)> = None;
    let mut pages: Option<(ObjectId, Dictionary)> = None;
    for (oid, obj) in &objekte {
        if let Ok(d) = obj.as_dict() {
            if catalog.is_none() && typ_ist(d, b"Catalog") {
                catalog = Some((*oid, d.clone()));
            }
            if pages.is_none() && typ_ist(d, b"Pages") {
                pages = Some((*oid, d.clone()));
            }
        }
    }
    let (catalog_id, mut catalog_d) = catalog.ok_or("Kein Catalog im PDF gefunden.")?;
    let (pages_id, mut pages_d) = pages.ok_or("Keine Seiten im PDF gefunden.")?;

    // Alle uebrigen Objekte uebernehmen (ohne Catalog/Pages/Seiten).
    for (oid, obj) in &objekte {
        if *oid == catalog_id || *oid == pages_id || seiten.contains_key(oid) {
            continue;
        }
        if let Ok(d) = obj.as_dict() {
            if typ_ist(d, b"Catalog") || typ_ist(d, b"Pages") {
                continue;
            }
        }
        ziel.objects.insert(*oid, obj.clone());
    }

    // Seiten an die gemeinsame Pages-Wurzel haengen.
    let mut kinder: Vec<Object> = Vec::with_capacity(seiten.len());
    for (oid, obj) in &seiten {
        if let Ok(d) = obj.as_dict() {
            let mut d = d.clone();
            d.set("Parent", Object::Reference(pages_id));
            ziel.objects.insert(*oid, Object::Dictionary(d));
            kinder.push(Object::Reference(*oid));
        }
    }

    pages_d.set("Count", kinder.len() as i64);
    pages_d.set("Kids", Object::Array(kinder));
    pages_d.remove(b"Parent");
    ziel.objects.insert(pages_id, Object::Dictionary(pages_d));

    catalog_d.set("Pages", Object::Reference(pages_id));
    catalog_d.remove(b"Outlines");
    ziel.objects.insert(catalog_id, Object::Dictionary(catalog_d));

    ziel.trailer.set("Root", Object::Reference(catalog_id));
    ziel.max_id = max_id;
    ziel.renumber_objects();
    ziel.compress();

    let mut out = Vec::new();
    ziel.save_to(&mut out)
        .map_err(|e| format!("PDF nicht speicherbar: {e}"))?;
    Ok(out)
}

// --- gemeinsamer Aufbau -------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn baue_antrags_pdf(
    app: &tauri::AppHandle,
    projekt: &str,
    foerderung: &str,
    titel: &str,
    abschnitte: &[PdfAbschnitt],
    anhaenge: &[String],
    logo: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut doc = neues_dokument()?;
    vorblatt_fuellen(&mut doc, titel, abschnitte, logo);
    let mut vorblatt = Vec::new();
    doc.render(&mut vorblatt)
        .map_err(|e| format!("PDF-Inhalt nicht erzeugbar: {e}"))?;

    let mut bloecke = vec![vorblatt];

    let dateien = ordner::wurzel(app)?
        .join(ordner::bereinigen(projekt)?)
        .join(ordner::bereinigen(foerderung)?)
        .join("Dateien");

    for name in anhaenge {
        // SICHERHEIT: Der Anhang-Name darf nicht aus dem Dateien-Ordner
        // ausbrechen. Normalerweise liefert dokument_hochladen bereinigte
        // Namen; diese Sink-Pruefung faengt einen praeparierten Tresor/Backup
        // ab (kein Pfad-Trenner, kein .., kein Laufwerk/ADS, nicht absolut).
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name.contains(':')
            || name == "."
            || name == ".."
            || std::path::Path::new(name).is_absolute()
        {
            return Err(format!("Ungueltiger Anhang-Name: {name}"));
        }
        let pfad = dateien.join(name);
        let daten = fs::read(&pfad).map_err(|e| format!("Anhang nicht lesbar ({name}): {e}"))?;
        let ext = pfad
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "pdf" => bloecke.push(daten),
            "png" | "jpg" | "jpeg" => bloecke.push(bild_pdf(daten)?),
            _ => {} // unbekannte Endung ueberspringen
        }
    }

    if bloecke.len() == 1 {
        return Ok(bloecke.pop().unwrap());
    }
    zusammenfuegen(bloecke)
}

// --- Tauri-Befehle ------------------------------------------------------

/// Erzeugt das Antrags-PDF und legt es als Vorschau in den temporaeren
/// Ordner; oeffnet es im Standard-PDF-Programm. Gibt den Pfad zurueck.
#[tauri::command]
pub fn antrags_pdf_vorschau(
    app: tauri::AppHandle,
    projekt: String,
    foerderung: String,
    titel: String,
    abschnitte: Vec<PdfAbschnitt>,
    anhaenge: Vec<String>,
    logo: Option<String>,
) -> Result<String, String> {
    let bytes = baue_antrags_pdf(&app, &projekt, &foerderung, &titel, &abschnitte, &anhaenge, logo.as_deref())?;
    // Eindeutiger Name, damit ein erneutes Erzeugen nicht an einer noch im
    // PDF-Programm geoeffneten (gesperrten) Vorschaudatei scheitert.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut pfad = std::env::temp_dir();
    pfad.push(format!("antrag3000-vorschau-{nonce}.pdf"));
    fs::write(&pfad, &bytes).map_err(|e| format!("Vorschau nicht schreibbar: {e}"))?;
    tauri_plugin_opener::open_path(pfad.clone(), None::<&str>)
        .map_err(|e| format!("Vorschau laesst sich nicht oeffnen: {e}"))?;
    Ok(pfad.to_string_lossy().to_string())
}

/// Erzeugt das Antrags-PDF und speichert es im Foerderer-Ordner als
/// Antrag_[Projekt]_[Foerderer].pdf; oeffnet den Ordner. Gibt den
/// Dateipfad zurueck (fuer den Mail-Versand in Schritt 4).
#[tauri::command]
pub fn antrags_pdf_speichern(
    app: tauri::AppHandle,
    projekt: String,
    foerderung: String,
    titel: String,
    abschnitte: Vec<PdfAbschnitt>,
    anhaenge: Vec<String>,
    logo: Option<String>,
) -> Result<String, String> {
    let bytes = baue_antrags_pdf(&app, &projekt, &foerderung, &titel, &abschnitte, &anhaenge, logo.as_deref())?;
    let ordner_pfad = ordner::wurzel(&app)?
        .join(ordner::bereinigen(&projekt)?)
        .join(ordner::bereinigen(&foerderung)?);
    fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let name = ordner::bereinigen(&format!("Antrag_{}_{}", projekt.trim(), foerderung.trim()))?
        + ".pdf";
    let pfad = ordner_pfad.join(&name);
    fs::write(&pfad, &bytes).map_err(|e| format!("PDF nicht schreibbar: {e}"))?;
    tauri_plugin_opener::open_path(ordner_pfad.clone(), None::<&str>)
        .map_err(|e| format!("Ordner laesst sich nicht oeffnen: {e}"))?;
    Ok(pfad.to_string_lossy().to_string())
}

/// Verwendungsnachweis (Abrechnung) als PDF: nur das Vorblatt (Titel +
/// Abschnitte), ohne Anhaenge. Wird in den Unterordner _Abrechnung des
/// Projekts geschrieben und die Datei geoeffnet.
#[tauri::command]
pub fn verwendungsnachweis_pdf(
    app: tauri::AppHandle,
    projekt: String,
    foerderer: String,
    titel: String,
    abschnitte: Vec<PdfAbschnitt>,
    logo: Option<String>,
) -> Result<String, String> {
    let mut doc = neues_dokument()?;
    vorblatt_fuellen(&mut doc, &titel, &abschnitte, logo.as_deref());
    let mut bytes = Vec::new();
    doc.render(&mut bytes)
        .map_err(|e| format!("PDF-Inhalt nicht erzeugbar: {e}"))?;

    let ordner_pfad = ordner::wurzel(&app)?
        .join(ordner::bereinigen(&projekt)?)
        .join("_Abrechnung");
    fs::create_dir_all(&ordner_pfad).map_err(|e| format!("Ordner nicht anlegbar: {e}"))?;
    let name =
        ordner::bereinigen(&format!("Verwendungsnachweis_{}_{}", projekt.trim(), foerderer.trim()))?
            + ".pdf";
    let pfad = ordner_pfad.join(&name);
    fs::write(&pfad, &bytes).map_err(|e| format!("PDF nicht schreibbar: {e}"))?;
    tauri_plugin_opener::open_path(pfad.clone(), None::<&str>)
        .map_err(|e| format!("PDF laesst sich nicht oeffnen: {e}"))?;
    Ok(pfad.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Smoke-Test: Schrift laden, Vorblatt mit Umlauten + Tabelle rendern
    // und zwei PDFs zu einem zusammenfuegen.
    #[test]
    fn vorblatt_und_merge() {
        let abschnitte = vec![
            PdfAbschnitt {
                ueberschrift: "Antragsteller:in".into(),
                absaetze: vec!["Ärztin Größe – Übung für 100 €".into()],
                tabelle: vec![],
            },
            PdfAbschnitt {
                ueberschrift: "Kostenplan".into(),
                absaetze: vec![],
                tabelle: vec![
                    vec!["Ausgaben".into(), "Erläuterung".into(), "Betrag".into()],
                    vec!["**1 Personal".into(), "".into(), "**100,00 €".into()],
                    vec!["1.1 Honorar".into(), "pro Tag".into(), "100,00 €".into()],
                ],
            },
        ];

        let mut doc = neues_dokument().unwrap();
        vorblatt_fuellen(&mut doc, "Förderantrag ÄÖÜ", &abschnitte, None);
        let mut a = Vec::new();
        doc.render(&mut a).unwrap();
        assert!(a.starts_with(b"%PDF"), "Vorblatt ist kein PDF");

        let mut doc2 = neues_dokument().unwrap();
        vorblatt_fuellen(&mut doc2, "Anhang", &[], None);
        let mut b = Vec::new();
        doc2.render(&mut b).unwrap();

        let zusammen = zusammenfuegen(vec![a, b]).unwrap();
        assert!(zusammen.starts_with(b"%PDF"), "Merge ist kein PDF");
        assert!(zusammen.len() > 800, "Merge zu klein: {}", zusammen.len());
    }

    // Bild -> einseitiges PDF (prueft das genpdf-"images"-Feature und
    // die Groessen-Ermittlung) mit einem minimalen 1x1-PNG.
    #[test]
    fn bild_zu_pdf() {
        const PNG_1X1: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        // PNG mit Alphakanal (RGBA) - muss auf Weiss geglaettet werden.
        let pdf = bild_pdf(PNG_1X1.to_vec()).unwrap();
        assert!(pdf.starts_with(b"%PDF"), "Bild-PDF ist kein PDF");
    }
}
