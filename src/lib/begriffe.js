// Zentrale Übersetzung der maschinenlesbaren Codes in Klartext.
// Wird von Liste, Detailansicht und Matching gemeinsam genutzt,
// damit überall dieselben Begriffe stehen.

export const LAENDER = {
  DE: "Deutschland",
  AT: "Österreich",
  CH: "Schweiz",
  INT: "International",
  ANDERES: "anderes Land",
};

export const SPARTEN = {
  musik: "Musik",
  theater: "Theater",
  tanz: "Tanz",
  performance: "Performance",
  bildende_kunst: "Bildende Kunst",
  medienkunst: "Medienkunst",
  literatur: "Literatur",
  film: "Film",
  interdisziplinaer: "Interdisziplinär",
};

export const PROJEKTARTEN = {
  produktion: "Produktion",
  recherche_entwicklung: "Recherche & Entwicklung",
  residenz: "Residenz",
  gastspiel_tournee: "Gastspiel / Tournee",
  festival: "Festival",
  veroeffentlichung: "Veröffentlichung",
  vermittlung: "Vermittlung",
  barrierefreiheit: "Förderung für Barrierefreiheit",
};

export const TRAEGERSCHAFT = {
  einzelperson: "Einzelperson",
  gruppe: "Gruppe / GbR",
  organisation: "Verein / Organisation",
};

// ============================================================
// Fristen. Ein Frist-Eintrag ist entweder ein einfacher String (alt) oder
// ein Objekt { datum, hinweis }. Das Datum ist entweder ein konkretes
// Datum "JJJJ-MM-TT" ODER – fuer wiederkehrende Fristen – ein Datum OHNE
// Jahr "MM-TT" (z. B. "09-15" = jeder 15. September). Der optionale Hinweis
// erklaert die Frist (z. B. "fuer das erste Halbjahr").
// Zusaetzlich kann die Foerderung einen allgemeinen Frist-Hinweis tragen
// (Feld frist_hinweis), z. B. "mind. 3 Monate vor Projektstart" – auch bei
// laufender Einreichung sinnvoll.
// ============================================================

const ISO_RE = /^\d{4}-\d{2}-\d{2}$/;
const MD_RE = /^\d{2}-\d{2}$/; // Monat-Tag ohne Jahr

/** Macht aus einem Frist-Eintrag immer { datum, hinweis }. */
export function fristNormalisieren(eintrag) {
  if (eintrag && typeof eintrag === "object") {
    return { datum: (eintrag.datum ?? "").trim(), hinweis: (eintrag.hinweis ?? "").trim() };
  }
  return { datum: String(eintrag ?? "").trim(), hinweis: "" };
}

/** Ist das ein wiederkehrendes Datum ohne Jahr ("MM-TT")? */
export function fristOhneJahr(datum) {
  return MD_RE.test((datum ?? "").trim());
}

/**
 * Loest einen Frist-Datumswert zum naechsten konkreten Datum auf (Date).
 *  - "JJJJ-MM-TT": genau dieses Datum.
 *  - "MM-TT" (ohne Jahr): das naechste Vorkommen – dieses Jahr, sonst naechstes.
 * Gibt null zurueck, wenn nicht interpretierbar.
 */
export function fristAlsDatum(datum, heute = new Date()) {
  const d = (datum ?? "").trim();
  if (ISO_RE.test(d)) {
    const dt = new Date(d);
    return isNaN(dt) ? null : dt;
  }
  if (MD_RE.test(d)) {
    const [m, t] = d.split("-").map(Number);
    const heuteNur = new Date(heute.getFullYear(), heute.getMonth(), heute.getDate());
    let dt = new Date(heute.getFullYear(), m - 1, t);
    if (dt < heuteNur) dt = new Date(heute.getFullYear() + 1, m - 1, t);
    return isNaN(dt) ? null : dt;
  }
  const dt = new Date(d);
  return isNaN(dt) ? null : dt;
}

/** Menschlich lesbarer Text eines Frist-Datums. */
export function fristDatumText(datum) {
  const d = (datum ?? "").trim();
  if (MD_RE.test(d)) {
    const [m, t] = d.split("-");
    return `${t}.${m}. (jährlich)`;
  }
  if (ISO_RE.test(d)) return new Date(d).toLocaleDateString("de-DE");
  return d;
}

/**
 * "Frist: 15.09.2026" | "laufend einreichbar" |
 * "wiederkehrend – nächste Frist: …" | Hinweis.
 *
 * Drei Zeitpunkt-Arten:
 *  - "laufend": jederzeit einreichbar.
 *  - "periodisch": regelmässig wiederkehrende Fristen (z. B. halbjährlich).
 *    Es gibt immer eine nächste Runde; wir zeigen die nächste hinterlegte
 *    Frist, sonst einen Hinweis auf die Webseite.
 *  - sonst (Standard "fristen"): feste Einreichfristen.
 * Per-Frist-Hinweise und der allgemeine frist_hinweis werden angehängt.
 */
export function fristText(f) {
  const z = f.weiche_kriterien.zeitpunkt;
  const allg = (f.frist_hinweis ?? "").trim();
  const anhang = allg ? ` (${allg})` : "";
  if (z === "laufend") return "laufend einreichbar" + anhang;

  const eintraege = (f.fristen ?? []).map(fristNormalisieren).filter((e) => e.datum);

  if (z === "periodisch") {
    const heute = new Date();
    const naechste = eintraege
      .map((e) => ({ e, dt: fristAlsDatum(e.datum, heute) }))
      .filter((x) => x.dt)
      .sort((a, b) => a.dt - b.dt)[0];
    if (naechste) {
      const h = naechste.e.hinweis ? ` – ${naechste.e.hinweis}` : "";
      return `wiederkehrend – nächste Frist: ${fristDatumText(naechste.e.datum)}${h}` + anhang;
    }
    return "wiederkehrende Fristen (siehe Webseite)" + anhang;
  }

  if (!eintraege.length) return "Fristen siehe Webseite" + anhang;
  const txt = eintraege
    .map((e) => fristDatumText(e.datum) + (e.hinweis ? ` (${e.hinweis})` : ""))
    .join(", ");
  return "Frist: " + txt + anhang;
}
