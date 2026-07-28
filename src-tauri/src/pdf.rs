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
use genpdf::elements::CellDecorator;
use genpdf::{elements, fonts, style, Alignment, Document, Element, SimplePageDecorator};
use image::{DynamicImage, GenericImageView};
use lopdf::{Dictionary, Document as LoDocument, Object, ObjectId, Stream};
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

/// Ein anzuhaengender Beleg (Verwendungsnachweis): der Dateiname im
/// Belegordner des Projekts plus der rote Stempel-Text, der oben links auf
/// jede Seite dieses Belegs gedruckt wird (z. B. „Beleg 1.2.1 · 1.2 Technik").
#[derive(Deserialize)]
pub struct BelegAnhang {
    pub datei: String,
    #[serde(default)]
    pub stempel: String,
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
    // Seitenraender nach DIN 5008 / DIN 676 (Geschaeftsbrief): links 25 mm,
    // rechts 20 mm, oben + unten 20 mm. Reihenfolge: (oben, rechts, unten, links).
    deko.set_margins((20.0, 20.0, 20.0, 25.0));
    doc.set_page_decorator(deko);
    Ok(doc)
}

// --- Vorblatt-Inhalt fuellen -------------------------------------------

/// Tabellen-Element mit Kopfzeilen-WIEDERHOLUNG auf jeder Folgeseite und
/// ZUSAMMENHALTEN kleiner Tabellen. Ersetzt genpdf's TableLayout, dessen
/// Seitenumbruch die Kopfzeile verliert und eine Zeile von ihrer Kopfzeile
/// abtrennt (siehe Antrags-PDF-Umbrueche). Die erste Zeile gilt als Kopfzeile.
struct KopfTabelle {
    kopf: Vec<String>,
    zeilen: Vec<Vec<String>>,
    gewichte: Vec<usize>,
    render_idx: usize,
}

impl KopfTabelle {
    /// Rendert EINE Zeile (Zellen nebeneinander) an der aktuellen Position;
    /// gibt Zeilenhoehe und has_more (Zelle passte nicht ganz) zurueck.
    /// ** am Zellanfang = fett; Kopfzeile (zeilen_idx == 0) immer fett.
    #[allow(clippy::too_many_arguments)]
    fn zeile_rendern(
        gewichte: &[usize],
        zellen: &[String],
        zeilen_idx: usize,
        deko: &mut elements::FrameCellDecorator,
        context: &genpdf::Context,
        area: genpdf::render::Area<'_>,
        style: style::Style,
    ) -> Result<(genpdf::Mm, bool), genpdf::error::Error> {
        let bereiche = area.split_horizontally(gewichte);
        let mut hoehe = genpdf::Mm::from(0.0_f32);
        let mut mehr = false;
        for (si, bereich) in bereiche.iter().enumerate() {
            let roh = zellen.get(si).map(|s| s.as_str()).unwrap_or("");
            let (text, fett) = match roh.strip_prefix("**") {
                Some(rest) => (rest, true),
                None => (roh, zeilen_idx == 0),
            };
            let mut st = style::Style::new();
            if fett {
                st = st.bold();
            }
            let mut p = elements::Paragraph::new(text)
                .styled(st)
                .padded(genpdf::Margins::from((2.5, 3.0, 2.5, 3.0)));
            let r = p.render(context, bereich.clone(), style)?;
            mehr |= r.has_more;
            hoehe = hoehe.max(r.size.height);
        }
        for (i, mut bereich) in bereiche.into_iter().enumerate() {
            bereich.set_height(hoehe);
            deko.decorate_cell(i, zeilen_idx, mehr, bereich, style);
        }
        Ok((hoehe, mehr))
    }
}

impl Element for KopfTabelle {
    fn render(
        &mut self,
        context: &genpdf::Context,
        mut area: genpdf::render::Area<'_>,
        style: style::Style,
    ) -> Result<genpdf::RenderResult, genpdf::error::Error> {
        let mut result = genpdf::RenderResult::default();
        if self.gewichte.is_empty() {
            return Ok(result);
        }
        result.size.width = area.size().width;

        let gesamt = self.zeilen.len() + 1; // inkl. Kopfzeile

        // Kleine Tabelle am Seitenende ganz auf die naechste Seite schieben,
        // damit Kopfzeile + Zeilen zusammenbleiben. Nur auf einer TEIL-Seite
        // (Schwelle 230 mm < nutzbare Seite ~257 mm) -> keine Endlosschleife.
        if self.render_idx == 0 && gesamt <= 8 {
            let noetig = genpdf::Mm::from(gesamt as f32 * 11.0);
            if area.size().height < noetig && area.size().height < genpdf::Mm::from(230.0_f32) {
                result.has_more = true;
                return Ok(result);
            }
        }

        // Dekorator frisch pro Seite -> die Kopfzeile bekommt oben wieder einen
        // Rahmen, und jedes Seiten-Fragment ist sauber umrandet.
        let mut deko = elements::FrameCellDecorator::new(true, true, false);
        deko.set_table_size(self.gewichte.len(), gesamt);

        // 1. Kopfzeile immer zuoberst (Wiederholung auf Folgeseiten).
        let kopf = self.kopf.clone();
        let (kh, _) = Self::zeile_rendern(
            &self.gewichte, &kopf, 0, &mut deko, context, area.clone(), style,
        )?;
        area.add_offset(genpdf::Position::new(0, kh));
        result.size.height += kh;

        // 2. Datenzeilen ab render_idx, bis die Seite voll ist.
        while self.render_idx < self.zeilen.len() {
            // Passt noch mindestens eine (einzeilige) Zeile? (~9 mm)
            if area.size().height < genpdf::Mm::from(9.0_f32) {
                break;
            }
            let zeile = self.zeilen[self.render_idx].clone();
            let (zh, mehr) = Self::zeile_rendern(
                &self.gewichte, &zeile, self.render_idx + 1, &mut deko, context, area.clone(), style,
            )?;
            area.add_offset(genpdf::Position::new(0, zh));
            result.size.height += zh;
            self.render_idx += 1;
            if mehr {
                break; // mehrzeilige Zelle hat sich geteilt -> Rest naechste Seite
            }
        }

        result.has_more = self.render_idx < self.zeilen.len();
        Ok(result)
    }
}

fn tabelle_einfuegen(doc: &mut Document, zeilen: &[Vec<String>]) {
    let spalten = zeilen.iter().map(|z| z.len()).max().unwrap_or(0);
    if spalten == 0 {
        return;
    }
    let gewichte: Vec<usize> = match spalten {
        2 => vec![6, 2],
        3 => vec![5, 4, 2],
        // Nach Kostenstelle gruppierte Belegliste des Verwendungsnachweises:
        // Nr · Datum · Beleg · Summe · Anteil (Gewichte ~mm bei 165 mm Breite).
        5 => vec![16, 27, 62, 30, 30],
        // Belegliste des Verwendungsnachweises: Nr · Datum · Beleg ·
        // Kostenstelle · Summe · Anteil. Die Gewichte entsprechen ~mm (Textbreite
        // 165 mm): schmale Spalten (Nr) klein, aber Datum- und Betragsspalten
        // breit genug, dass die nicht-umbrechbaren Werte (z. B. "05.03.2026",
        // "1.250,00 €") komplett hineinpassen – sonst verwirft genpdf die Zelle.
        6 => vec![16, 27, 36, 34, 26, 26],
        n => vec![1; n],
    };
    doc.push(KopfTabelle {
        kopf: zeilen[0].clone(),
        zeilen: zeilen[1..].to_vec(),
        gewichte,
        render_idx: 0,
    });
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

fn vorblatt_fuellen(
    doc: &mut Document,
    titel: &str,
    untertitel: &[String],
    absender: &[String],
    abschnitte: &[PdfAbschnitt],
    logo: Option<&str>,
) {
    briefkopf_einfuegen(doc, logo);

    // Absender-Briefkopf: Name + Adresse der antragstellenden Person oben.
    // Der BLOCK sitzt rechts, die Zeilen darin sind aber LINKSBUENDIG (nicht
    // flatterrandig). Umsetzung: randlose 2-Spalten-Tabelle, linke Spalte
    // leer, rechte Spalte der linksbuendige Adressblock (ein einzelner Absatz
    // nutzt in genpdf immer die volle Breite, daher der Umweg ueber die Tabelle).
    if !absender.is_empty() {
        let st = style::Style::new().with_font_size(9);
        let mut block = elements::LinearLayout::vertical();
        for zeile in absender {
            block.push(elements::Paragraph::new(zeile).styled(st));
        }
        // DIN-5008-Informationsblock: beginnt 124 mm vom linken Blattrand.
        // Bei 25 mm linkem + 20 mm rechtem Rand (Textbreite 165 mm) trifft das
        // Verhaeltnis 3:2 genau diese Position: 25 + 165*3/5 = 124 mm.
        let mut t = elements::TableLayout::new(vec![3, 2]);
        t.set_cell_decorator(elements::FrameCellDecorator::new(false, false, false));
        let mut reihe = t.row();
        reihe.push_element(elements::Paragraph::new("")); // linke Spalte leer
        reihe.push_element(block); // rechte Spalte: Adressblock, linksbuendig
        let _ = reihe.push();
        doc.push(t);
        doc.push(elements::Break::new(1.0));
    }

    doc.push(
        elements::Paragraph::new(titel).styled(style::Style::new().bold().with_font_size(16)),
    );
    // Untertitel-Zeilen direkt unter dem Titel (z. B. „für das Projekt X",
    // Soll/Ist), etwas kleiner und in Grau abgesetzt.
    if !untertitel.is_empty() {
        doc.push(elements::Break::new(0.3));
        let st = style::Style::new()
            .with_font_size(11)
            .with_color(style::Color::Rgb(90, 90, 90));
        for zeile in untertitel {
            doc.push(elements::Paragraph::new(zeile).styled(st));
        }
    }
    doc.push(elements::Break::new(1.0));

    for a in abschnitte {
        if !a.ueberschrift.is_empty() {
            doc.push(
                elements::Paragraph::new(a.ueberschrift.as_str())
                    .styled(style::Style::new().bold().with_font_size(12)),
            );
            // Luft zwischen Ueberschrift und folgendem Inhalt/Tabelle, sonst
            // "klebt" die Ueberschrift am Tabellenrahmen darunter.
            doc.push(elements::Break::new(0.4));
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
    // Schutz vor "Dekompressions-Bomben" (Audit D2): erst die Abmessungen aus
    // dem Datei-Kopf lesen und begrenzen, BEVOR das Bild voll dekodiert wird
    // (sonst koennte ein winziges Bild mit riesigen Abmessungen w*h*3 Byte
    // Speicher anfordern und das Programm zum Absturz bringen).
    const MAX_PIXEL: u64 = 40_000_000; // ~40 Megapixel
    if let Ok((w, h)) = image::io::Reader::new(std::io::Cursor::new(&daten))
        .with_guessed_format()
        .map_err(|e| format!("Bild nicht lesbar: {e}"))?
        .into_dimensions()
    {
        if (w as u64) * (h as u64) > MAX_PIXEL {
            return Err("Das Bild hat zu viele Pixel (höchstens ~40 Megapixel).".into());
        }
    }

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

// --- Beleg-Anhaenge mit rotem Stempel (Verwendungsnachweis) ------------

/// Wandelt Text in WinAnsi-Bytes (Helvetica-Standardkodierung). Deckt Latin-1
/// inkl. Umlaute und den Mittelpunkt „·" ab sowie die gaengigen Typografie-
/// Zeichen; alles andere wird zu '?'.
fn winansi_bytes(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            let u = c as u32;
            match c {
                '\u{2013}' => 0x96, // –
                '\u{2014}' => 0x97, // —
                '\u{2022}' => 0x95, // •
                '\u{2018}' => 0x91,
                '\u{2019}' => 0x92,
                '\u{201A}' => 0x82,
                '\u{201C}' => 0x93,
                '\u{201D}' => 0x94,
                '\u{201E}' => 0x84,
                '\u{2026}' => 0x85, // …
                '\u{20AC}' => 0x80, // €
                _ if u < 0x80 => u as u8,
                _ if (0xA0..=0xFF).contains(&u) => u as u8, // Latin-1 == WinAnsi
                _ => b'?',
            }
        })
        .collect()
}

/// Maskiert die in PDF-Textstrings kritischen Zeichen ( ) und \.
fn pdf_string_escape(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 4);
    for &b in bytes {
        if b == b'(' || b == b')' || b == b'\\' {
            out.push(b'\\');
        }
        out.push(b);
    }
    out
}

fn zahl(o: &Object) -> Option<f32> {
    match o {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r as f32),
        _ => None,
    }
}

/// Objekt ggf. dereferenzieren und als Dictionary klonen.
fn als_dict(doc: &LoDocument, o: &Object) -> Option<Dictionary> {
    match o {
        Object::Reference(id) => doc.get_object(*id).ok()?.as_dict().ok().cloned(),
        Object::Dictionary(d) => Some(d.clone()),
        _ => None,
    }
}

/// Seitenhoehe (in PDF-Punkten) aus der MediaBox der Seite oder eines
/// Eltern-Knotens; Fallback A4-Hoehe (842 pt).
fn seite_hoehe(doc: &LoDocument, page_id: ObjectId) -> f32 {
    let mut cur = Some(page_id);
    while let Some(id) = cur {
        let Ok(d) = doc.get_object(id).and_then(|o| o.as_dict()) else {
            break;
        };
        if let Ok(roh) = d.get(b"MediaBox") {
            let mb = match roh {
                Object::Reference(r) => doc.get_object(*r).ok().cloned(),
                other => Some(other.clone()),
            };
            if let Some(arr) = mb.as_ref().and_then(|o| o.as_array().ok()) {
                if arr.len() == 4 {
                    let y0 = zahl(&arr[1]).unwrap_or(0.0);
                    let y1 = zahl(&arr[3]).unwrap_or(842.0);
                    return (y1 - y0).abs();
                }
            }
        }
        cur = d.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    842.0
}

/// Die (ggf. von einem Eltern-Knoten geerbten) Resources einer Seite als
/// klonbares Dictionary.
fn effektive_resources(doc: &LoDocument, page_id: ObjectId) -> Dictionary {
    let mut cur = Some(page_id);
    while let Some(id) = cur {
        let Ok(d) = doc.get_object(id).and_then(|o| o.as_dict()) else {
            break;
        };
        if let Ok(r) = d.get(b"Resources") {
            if let Some(dict) = als_dict(doc, r) {
                return dict;
            }
        }
        cur = d.get(b"Parent").ok().and_then(|p| p.as_reference().ok());
    }
    Dictionary::new()
}

/// Legt den roten Stempel-Text auf JEDE Seite des PDF-Blocks (oben links) und
/// gibt den neuen PDF-Block zurueck. Best-effort: schlaegt etwas fehl, kommt
/// der Beleg unveraendert zurueck (nie verlieren wir den Beleg selbst).
fn stempel_auf_block(block: &[u8], text: &str) -> Vec<u8> {
    if text.trim().is_empty() {
        return block.to_vec();
    }
    let Ok(mut doc) = LoDocument::load_mem(block) else {
        return block.to_vec();
    };

    // Stempel-Schrift (die rote Farbe setzt der Content-Stream, nicht die Font).
    let mut font = Dictionary::new();
    font.set("Type", Object::Name(b"Font".to_vec()));
    font.set("Subtype", Object::Name(b"Type1".to_vec()));
    font.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    font.set("Encoding", Object::Name(b"WinAnsiEncoding".to_vec()));
    let font_id = doc.add_object(Object::Dictionary(font));

    let escaped = pdf_string_escape(&winansi_bytes(text));
    let seiten: Vec<ObjectId> = doc.get_pages().into_values().collect();

    for page_id in seiten {
        let hoehe = seite_hoehe(&doc, page_id);
        // Roter Text, 12 pt, 28 pt vom linken und oberen Blattrand.
        let mut inhalt: Vec<u8> = Vec::new();
        inhalt.extend_from_slice(b"q 1 0 0 rg BT /A3Stamp 12 Tf ");
        inhalt.extend_from_slice(format!("28 {:.1} Td (", hoehe - 28.0).as_bytes());
        inhalt.extend_from_slice(&escaped);
        inhalt.extend_from_slice(b") Tj ET Q\n");
        let stream_id = doc.add_object(Stream::new(Dictionary::new(), inhalt));

        // Stempel-Font in die Resources der Seite mergen (eigene Resources
        // setzen, damit die vorhandenen Ressourcen erhalten bleiben).
        let mut res = effektive_resources(&doc, page_id);
        let mut fonts = res
            .get(b"Font")
            .ok()
            .and_then(|o| als_dict(&doc, o))
            .unwrap_or_default();
        fonts.set("A3Stamp", Object::Reference(font_id));
        res.set("Font", Object::Dictionary(fonts));

        if let Ok(page) = doc.get_object_mut(page_id).and_then(|o| o.as_dict_mut()) {
            page.set("Resources", Object::Dictionary(res));
            // Stempel-Stream ans Ende der Content-Streams haengen (Overlay).
            let neu = match page.get(b"Contents") {
                Ok(Object::Reference(r)) => {
                    Object::Array(vec![Object::Reference(*r), Object::Reference(stream_id)])
                }
                Ok(Object::Array(a)) => {
                    let mut a = a.clone();
                    a.push(Object::Reference(stream_id));
                    Object::Array(a)
                }
                _ => Object::Array(vec![Object::Reference(stream_id)]),
            };
            page.set("Contents", neu);
        }
    }

    let mut out = Vec::new();
    if doc.save_to(&mut out).is_err() {
        return block.to_vec();
    }
    out
}

// --- gemeinsamer Aufbau -------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn baue_antrags_pdf(
    app: &tauri::AppHandle,
    projekt: &str,
    foerderung: &str,
    titel: &str,
    absender: &[String],
    abschnitte: &[PdfAbschnitt],
    anhaenge: &[String],
    logo: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut doc = neues_dokument()?;
    vorblatt_fuellen(&mut doc, titel, &[], absender, abschnitte, logo);
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
    absender: Vec<String>,
    abschnitte: Vec<PdfAbschnitt>,
    anhaenge: Vec<String>,
    logo: Option<String>,
) -> Result<String, String> {
    let bytes = baue_antrags_pdf(&app, &projekt, &foerderung, &titel, &absender, &abschnitte, &anhaenge, logo.as_deref())?;
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
    absender: Vec<String>,
    abschnitte: Vec<PdfAbschnitt>,
    anhaenge: Vec<String>,
    logo: Option<String>,
) -> Result<String, String> {
    let bytes = baue_antrags_pdf(&app, &projekt, &foerderung, &titel, &absender, &abschnitte, &anhaenge, logo.as_deref())?;
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

/// Verwendungsnachweis (Abrechnung) als PDF: Vorblatt (Kopfzeile + Titel +
/// gruppierte Belegliste) und dahinter die Beleg-Dateien dieser Geldquelle
/// als Anhang – jede Seite oben links mit rotem Stempel (Beleg-Nr. +
/// Kostenstelle). Wird in den Unterordner _Abrechnung geschrieben und geoeffnet.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn verwendungsnachweis_pdf(
    app: tauri::AppHandle,
    projekt: String,
    foerderer: String,
    titel: String,
    untertitel: Vec<String>,
    absender: Vec<String>,
    abschnitte: Vec<PdfAbschnitt>,
    anhaenge: Vec<BelegAnhang>,
    logo: Option<String>,
) -> Result<String, String> {
    let mut doc = neues_dokument()?;
    // Kopfzeile wie beim Antrags-PDF: Logo + Absender (Stammdaten), darunter
    // Titel + Untertitel (Projekt, Soll/Ist).
    vorblatt_fuellen(&mut doc, &titel, &untertitel, &absender, &abschnitte, logo.as_deref());
    let mut vorblatt = Vec::new();
    doc.render(&mut vorblatt)
        .map_err(|e| format!("PDF-Inhalt nicht erzeugbar: {e}"))?;

    // Beleg-Dateien dieser Geldquelle aus dem (lesbaren) Belegordner anhaengen,
    // jede Seite mit rotem Stempel. Fehlende/ungueltige Dateien werden still
    // uebersprungen – der Nachweis selbst entsteht immer.
    let belegordner = ordner::wurzel(&app)?
        .join(ordner::bereinigen(&projekt)?)
        .join("Belege");
    let mut bloecke = vec![vorblatt];
    for a in &anhaenge {
        let name = a.datei.as_str();
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name.contains(':')
            || name == "."
            || name == ".."
            || std::path::Path::new(name).is_absolute()
        {
            continue;
        }
        let pfad = belegordner.join(name);
        let Ok(daten) = fs::read(&pfad) else { continue };
        let ext = pfad
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        let block = match ext.as_str() {
            "pdf" => daten,
            "png" | "jpg" | "jpeg" => match bild_pdf(daten) {
                Ok(b) => b,
                Err(_) => continue,
            },
            _ => continue,
        };
        bloecke.push(stempel_auf_block(&block, &a.stempel));
    }

    let bytes = if bloecke.len() == 1 {
        bloecke.pop().unwrap()
    } else {
        zusammenfuegen(bloecke)?
    };

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

        let absender = vec![
            "Max Muster".to_string(),
            "Musterstraße 1".to_string(),
            "8000 Zürich".to_string(),
        ];
        let mut doc = neues_dokument().unwrap();
        vorblatt_fuellen(&mut doc, "Förderantrag ÄÖÜ", &[], &absender, &abschnitte, None);
        let mut a = Vec::new();
        doc.render(&mut a).unwrap();
        assert!(a.starts_with(b"%PDF"), "Vorblatt ist kein PDF");

        let mut doc2 = neues_dokument().unwrap();
        vorblatt_fuellen(&mut doc2, "Anhang", &[], &[], &[], None);
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

    // Erzeugt ein mehrseitiges Muster-Antrags-PDF zum Sichten (Zell-Abstand,
    // Absender-Briefkopf, Tabellen-Umbruch ueber Seitengrenzen). Laeuft nur auf
    // Anfrage:  cargo test muster_pdf_datei -- --ignored --nocapture
    // Danach z. B. mit PyMuPDF zu PNG rendern und ansehen.
    #[test]
    #[ignore = "Werkzeug: schreibt ein Muster-PDF in den Temp-Ordner zum Sichten"]
    fn muster_pdf_datei() {
        let absender = vec![
            "Max Muster".to_string(),
            "Organisation/Träger: Kollektiv X".to_string(),
            "Musterstraße 1".to_string(),
            "8000 Zürich".to_string(),
            "E-Mail: max@example.ch".to_string(),
            "Telefon: 044 123 45 67".to_string(),
        ];
        let mut tab = vec![vec![
            "Ausgaben".to_string(),
            "Erläuterung".to_string(),
            "Betrag".to_string(),
        ]];
        for i in 1..=10 {
            tab.push(vec![format!("**{i} Kostengruppe"), "".into(), "**2.000,00 €".into()]);
            tab.push(vec![format!("{i}.1 Künstlerische Leitung"), "".into(), "1.000,00 €".into()]);
            tab.push(vec![format!("{i}.2 Assistenz Leitung"), "".into(), "1.000,00 €".into()]);
        }
        let abschnitte = vec![
            PdfAbschnitt {
                ueberschrift: "Kurzbeschreibung".into(),
                absaetze: vec!["Ein kurzer Beschreibungstext für das Testprojekt.".into()],
                tabelle: vec![],
            },
            PdfAbschnitt {
                ueberschrift: "Kostenplan".into(),
                absaetze: vec![],
                tabelle: tab,
            },
            PdfAbschnitt {
                ueberschrift: "Bankverbindung".into(),
                absaetze: vec!["IBAN: CH00 0000 0000 0000 0000 0".into(), "Bank: ZKB".into()],
                tabelle: vec![],
            },
            // Fueller, damit die kleine Tabelle darunter nahe an ein
            // Seitenende faellt (testet das Zusammenhalten kleiner Tabellen).
            PdfAbschnitt {
                ueberschrift: "Ausfuehrliche Projektbeschreibung".into(),
                absaetze: vec!["Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam. ".repeat(20)],
                tabelle: vec![],
            },
            PdfAbschnitt {
                ueberschrift: "Bei dieser Foerderung beantragte Summe".into(),
                absaetze: vec![],
                tabelle: vec![
                    vec!["Foerderer".into(), "Betrag".into()],
                    vec!["**Internationales Kuenstler:innengremium (Fehlbetrag)".into(), "**20.620,00 €".into()],
                ],
            },
        ];
        let mut doc = neues_dokument().unwrap();
        vorblatt_fuellen(
            &mut doc,
            "Förderantrag: Test Projekt – Stadt Zürich – Kulturförderung",
            &[],
            &absender,
            &abschnitte,
            None,
        );
        let mut bytes = Vec::new();
        doc.render(&mut bytes).unwrap();
        let pfad = std::env::temp_dir().join("antrag3000-muster.pdf");
        std::fs::write(&pfad, &bytes).unwrap();
        eprintln!("MUSTER-PDF geschrieben: {}", pfad.display());
    }

    // Erzeugt einen Muster-VERWENDUNGSNACHWEIS (Abrechnung) zum Sichten, genau
    // wie ihn das Frontend (verwendungsnachweisAbschnitte) baut: Angaben,
    // Sachbericht, Belegliste (mit Summenzeile) und Kostenuebersicht. Kein
    // Absender-Briefkopf (wie in verwendungsnachweis_pdf). Laeuft nur auf
    // Anfrage:  cargo test muster_verwendungsnachweis -- --ignored --nocapture
    #[test]
    #[ignore = "Werkzeug: schreibt einen Muster-Verwendungsnachweis in den Temp-Ordner"]
    fn muster_verwendungsnachweis() {
        let z = |s: &str| s.to_string();
        // Kopf-Beleg einer Gruppe (Nr · Datum · Beleg · Summe · Anteil).
        let kopf = || vec![z("Nr."), z("Datum"), z("Beleg"), z("Summe"), z("Anteil")];
        let abschnitte = vec![
            PdfAbschnitt {
                ueberschrift: "Sachbericht".into(),
                absaetze: vec![
                    z("Das Projekt Klangraum realisierte eine begehbare Klang- und Lichtinstallation im Kulturhaus Zürich. Über sechs Wochen entstand gemeinsam mit vier Kunstschaffenden eine interaktive Umgebung, in der Besucher:innen durch Bewegung Klänge auslösen."),
                    z("Die Förderung der Stadt Zürich deckte die Material- und Technikkosten sowie einen Teil der Honorare. Alle geplanten Programmpunkte konnten umgesetzt werden; die Publikumsresonanz war mit rund 1.200 Besuchen deutlich über der Erwartung."),
                ],
                tabelle: vec![],
            },
            PdfAbschnitt {
                ueberschrift: "Kostenstelle 1.1 Material".into(),
                absaetze: vec![],
                tabelle: vec![
                    kopf(),
                    vec![z("1.1.1"), z("05.03.2026"), z("Bühnenbau GmbH · Rohmaterial"), z("1.250,00 €"), z("1.250,00 €")],
                    vec![z("1.1.2"), z("03.05.2026"), z("Bauhaus · Farben, Kleinmaterial"), z("480,00 €"), z("300,00 €")],
                    vec![z(""), z(""), z("**Zwischensumme"), z(""), z("**1.550,00 €")],
                ],
            },
            PdfAbschnitt {
                ueberschrift: "Kostenstelle 1.2 Technik".into(),
                absaetze: vec![],
                tabelle: vec![
                    kopf(),
                    vec![z("1.2.1"), z("12.03.2026"), z("Tonstudio Klang · Aufnahme"), z("2.400,00 €"), z("2.000,00 €")],
                    vec![z(""), z(""), z("**Zwischensumme"), z(""), z("**2.000,00 €")],
                ],
            },
            PdfAbschnitt {
                ueberschrift: "Kostenstelle 2.1 Werbung".into(),
                absaetze: vec![],
                tabelle: vec![
                    kopf(),
                    vec![z("2.1.1"), z("20.04.2026"), z("Grafikbüro Nord · Plakate & Flyer"), z("900,00 €"), z("900,00 €")],
                    vec![z(""), z(""), z("**Zwischensumme"), z(""), z("**900,00 €")],
                ],
            },
            PdfAbschnitt {
                ueberschrift: "Kostenstelle 3.1 Honorare".into(),
                absaetze: vec![],
                tabelle: vec![
                    kopf(),
                    vec![z("3.1.1"), z("30.06.2026"), z("Honorar Regie · Aisha Ndiaye"), z("2.000,00 €"), z("2.000,00 €")],
                    vec![z(""), z(""), z("**Zwischensumme"), z(""), z("**2.000,00 €")],
                ],
            },
        ];
        let absender = vec![
            z("Kollektiv Klangraum"),
            z("Organisation/Träger: Verein Klangraum"),
            z("Bahnhofstraße 3"),
            z("8001 Zürich"),
            z("E-Mail: hallo@klangraum.ch"),
        ];
        let untertitel = vec![
            z("für das Projekt Klangraum – Interaktive Installation"),
            z("Bewilligt 8.000,00 € · Abgerechnet 6.450,00 € · Stand 28.07.2026"),
        ];
        let mut doc = neues_dokument().unwrap();
        vorblatt_fuellen(
            &mut doc,
            "Verwendungsnachweis – Stadt Zürich – Kulturförderung",
            &untertitel,
            &absender,
            &abschnitte,
            None,
        );
        let mut vorblatt = Vec::new();
        doc.render(&mut vorblatt).unwrap();

        // Realistische Beispiel-Belege (wie eingescannte Rechnungen) mit rotem
        // Stempel oben links – so ist die Anhang-Darstellung anschaulich.
        let grau = || style::Style::new().with_font_size(9).with_color(style::Color::Rgb(90, 90, 90));
        let beispiel_beleg = |firma: &str, adresse: &str, nr: &str, datum: &str,
                              posten: &[(&str, &str)], summe: &str| {
            let mut d = neues_dokument().unwrap();
            d.push(elements::Paragraph::new(firma).styled(style::Style::new().bold().with_font_size(15)));
            d.push(elements::Paragraph::new(adresse).styled(grau()));
            d.push(elements::Break::new(0.6));
            d.push(elements::Paragraph::new(format!("Rechnung Nr. {nr}          Datum: {datum}")));
            d.push(elements::Break::new(0.6));
            let mut zeilen = vec![vec![z("Position"), z("Betrag")]];
            for (p, b) in posten {
                zeilen.push(vec![z(p), z(b)]);
            }
            zeilen.push(vec![z("**Summe"), format!("**{summe}")]);
            tabelle_einfuegen(&mut d, &zeilen);
            d.push(elements::Break::new(0.8));
            d.push(elements::Paragraph::new("Zahlung: Karte · Betrag dankend erhalten.").styled(grau()));
            let mut v = Vec::new();
            d.render(&mut v).unwrap();
            v
        };
        let beleg1 = stempel_auf_block(
            &beispiel_beleg(
                "Bühnenbau GmbH",
                "Werkstrasse 8 · 8004 Zürich",
                "2026-0342",
                "05.03.2026",
                &[
                    ("Holzplatten Multiplex 18 mm (12 Stk.)", "840,00 €"),
                    ("Beschläge & Schrauben", "260,00 €"),
                    ("Lieferung", "150,00 €"),
                ],
                "1.250,00 €",
            ),
            "Beleg 1.1.1 · 1.1 Material",
        );
        let beleg2 = stempel_auf_block(
            &beispiel_beleg(
                "Tonstudio Klang",
                "Seefeldstrasse 21 · 8008 Zürich",
                "R-2026-118",
                "12.03.2026",
                &[
                    ("Studiomiete (2 Tage)", "1.600,00 €"),
                    ("Toningenieur", "640,00 €"),
                    ("Export & Datenträger", "160,00 €"),
                ],
                "2.400,00 €",
            ),
            "Beleg 1.2.1 · 1.2 Technik",
        );
        let bytes = zusammenfuegen(vec![vorblatt, beleg1, beleg2]).unwrap();

        let pfad = std::env::temp_dir().join("antrag3000-verwendungsnachweis.pdf");
        std::fs::write(&pfad, &bytes).unwrap();
        eprintln!("MUSTER-VERWENDUNGSNACHWEIS geschrieben: {}", pfad.display());
    }
}
