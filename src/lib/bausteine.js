// ============================================================
// Bausteine: Teile eines Projekts, etwa die einzelnen Konzerte einer Reihe.
//
// Ein Projekt hat optional eine Liste `bausteine` ([{id, name}]). Ein
// Kosten-Posten gehört über `bausteinId` zu einem Baustein; ohne gültige
// Zuordnung gilt er als gemeinsam (z. B. Plakate für die ganze Reihe). Ohne
// Bausteine sieht alles aus wie bisher.
//
// Wichtig: Filtern blendet nur aus und ändert nie die Nummern. An der Nummer
// „2.3" hängen Belegnummern und Stempel in der Abrechnung.
//
// Reine Logik ohne Svelte – darum mit `npm run bausteine` testbar.
// ============================================================

/// Filterwerte neben den Baustein-Kennungen.
export const ALLE = "alle";
export const GEMEINSAM = "gemeinsam";

/// Baustein-Kennung eines Postens, "" wenn gemeinsam. Ein Verweis auf einen
/// nicht mehr vorhandenen Baustein zählt als gemeinsam.
/**
 * @param {any} posten
 * @param {any[]} bausteine
 */
export function bausteinVon(posten, bausteine) {
  const id = posten?.bausteinId ?? "";
  return id && (bausteine ?? []).some((b) => b.id === id) ? id : "";
}

/// Anzeigename des Bausteins eines Postens, z. B. für die Excel-Spalte.
/**
 * @param {any} posten
 * @param {any[]} bausteine
 */
export function bausteinName(posten, bausteine) {
  const id = bausteinVon(posten, bausteine);
  return id ? bausteine.find((b) => b.id === id).name : "gemeinsam";
}

/// Gehört ein Posten zum gewählten Filter (ALLE, GEMEINSAM oder Baustein-id)?
/**
 * @param {any} posten
 * @param {string} filter
 * @param {any[]} bausteine
 */
export function passtZuFilter(posten, filter, bausteine) {
  if (filter === ALLE) return true;
  const id = bausteinVon(posten, bausteine);
  return filter === GEMEINSAM ? id === "" : id === filter;
}

/// Kosten für die Anzeige filtern. Jede Kategorie und jeder Posten behält
/// seine ORIGINAL-Nummer (`nummer`), egal was ausgeblendet ist. Kategorien
/// ohne passenden Posten fehlen, außer bei ALLE.
/// Liefert [{ kategorie, nummer, posten: [{ posten, nummer, index }] }].
/**
 * @param {any[]} kosten
 * @param {string} filter
 * @param {any[]} bausteine
 */
export function kostenGefiltert(kosten, filter, bausteine) {
  const liste = [];
  (kosten ?? []).forEach((kategorie, ki) => {
    const posten = [];
    (kategorie.posten ?? []).forEach((/** @type {any} */ p, pi) => {
      if (passtZuFilter(p, filter, bausteine)) {
        posten.push({ posten: p, nummer: `${ki + 1}.${pi + 1}`, index: pi });
      }
    });
    if (filter === ALLE || posten.length) liste.push({ kategorie, nummer: String(ki + 1), posten });
  });
  return liste;
}

/// Summe der Kosten-Posten, die zum Filter passen.
/**
 * @param {any[]} kosten
 * @param {string} filter
 * @param {any[]} bausteine
 * @param {(p: any) => number} betrag
 */
export function summeGefiltert(kosten, filter, bausteine, betrag) {
  let summe = 0;
  for (const k of kosten ?? []) {
    for (const p of k.posten ?? []) if (passtZuFilter(p, filter, bausteine)) summe += betrag(p);
  }
  return summe;
}

/// Prüft den Namen eines Bausteins: nicht leer, nicht doppelt im Projekt.
/// Liefert { ok, name, fehler }; bei leerer Eingabe ohne Meldung.
/**
 * @param {any} eingabe
 * @param {any[]} bausteine
 * @param {any} [eigeneId]
 */
export function bausteinNamePruefen(eingabe, bausteine, eigeneId = null) {
  const name = String(eingabe ?? "").trim();
  if (!name) return { ok: false, name, fehler: "" };
  const schon = (bausteine ?? []).find(
    (b) => b.id !== eigeneId && String(b.name).toLowerCase() === name.toLowerCase()
  );
  if (schon) return { ok: false, name, fehler: `Es gibt schon einen Baustein „${schon.name}".` };
  return { ok: true, name, fehler: "" };
}

/// Entfernt einen Baustein. Seine Posten bleiben erhalten und gelten danach
/// als gemeinsam; sein Anteil an der Finanzierung fällt weg (siehe
/// teilantrag.js). Ändert `bausteine` und `kfp` direkt.
/**
 * @param {any[]} bausteine
 * @param {any} kfp
 * @param {string} id
 */
export function bausteinEntfernen(bausteine, kfp, id) {
  const i = bausteine.findIndex((b) => b.id === id);
  if (i >= 0) bausteine.splice(i, 1);
  for (const k of kfp?.kosten ?? []) {
    for (const p of k.posten ?? []) if (p.bausteinId === id) p.bausteinId = "";
  }
  for (const k of kfp?.finanzierung ?? []) {
    for (const p of k.posten ?? []) if (p.anteile) delete p.anteile[id];
  }
}

/// Texte je Baustein im Sammel-Formular: [Schlüssel, Beschriftung, Eingabetyp].
/// Alle sind freiwillig; ein leeres Feld erscheint nirgends.
export const BAUSTEIN_FELDER = [
  ["beschreibung", "Beschreibung", "textarea"],
  ["zeitraum", "Zeitraum", "input"],
  ["ort", "Ort", "input"],
];

/// Texte aus dem Sammel-Formular übernehmen: `texte` = { [id]: {beschreibung,
/// zeitraum, ort} }. Name und Zuordnungen bleiben unberührt (die gehören dem
/// Kostenplan); Texte zu inzwischen gelöschten Bausteinen fallen weg.
/**
 * @param {any[]} bausteine
 * @param {Record<string, any>} texte
 */
export function bausteinTexteUebernehmen(bausteine, texte) {
  for (const b of bausteine ?? []) {
    const t = texte?.[b.id];
    if (!t) continue;
    for (const [key] of BAUSTEIN_FELDER) {
      const wert = String(t[key] ?? "").trim();
      if (wert) b[key] = wert;
      else delete b[key];
    }
  }
}

/// PDF-Abschnitte mit den Texten der Bausteine: bei einem Teil-Antrag nur die
/// abgedeckten, sonst alle. Bausteine ohne Text fehlen, darum bleibt ein PDF
/// ohne Baustein-Texte genau wie bisher.
/**
 * @param {any[]} bausteine
 * @param {string[] | null} abdeckung  null = ganzes Projekt
 */
export function bausteinAbschnitte(bausteine, abdeckung) {
  const abschnitte = [];
  for (const b of bausteine ?? []) {
    if (abdeckung !== null && !abdeckung.includes(b.id)) continue;
    const beschreibung = String(b.beschreibung ?? "").trim();
    const zeitraum = String(b.zeitraum ?? "").trim();
    const ort = String(b.ort ?? "").trim();
    const absaetze = [];
    if (beschreibung) absaetze.push(beschreibung);
    if (zeitraum) absaetze.push(`Zeitraum: ${zeitraum}`);
    if (ort) absaetze.push(`Ort: ${ort}`);
    if (absaetze.length) abschnitte.push({ ueberschrift: `Projektteil: ${b.name}`, absaetze, tabelle: [] });
  }
  return abschnitte;
}

/// Bringt ein Projekt aus älteren Ständen auf den Stand mit Bausteinen und
/// räumt Verweise auf fehlende Bausteine weg. Liefert true bei Änderung.
/** @param {any} p */
export function bausteineUmstellen(p) {
  let veraendert = false;
  if (!Array.isArray(p.bausteine)) {
    p.bausteine = [];
    veraendert = true;
  }
  const ids = new Set(p.bausteine.map((/** @type {any} */ b) => b?.id));
  for (const k of p.kfp?.kosten ?? []) {
    for (const po of k.posten ?? []) {
      if (po.bausteinId && !ids.has(po.bausteinId)) {
        po.bausteinId = "";
        veraendert = true;
      }
    }
  }
  return veraendert;
}
