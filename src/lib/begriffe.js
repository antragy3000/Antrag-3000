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
 */
export function fristText(f) {
  const z = f.weiche_kriterien.zeitpunkt;
  if (z === "laufend") return "laufend einreichbar";
  if (z === "periodisch") {
    const heute = new Date();
    const naechste = (f.fristen ?? [])
      .map((d) => new Date(d))
      .filter((d) => !isNaN(d) && d >= heute)
      .sort((a, b) => a - b)[0];
    return naechste
      ? "wiederkehrend – nächste Frist: " + naechste.toLocaleDateString("de-DE")
      : "wiederkehrende Fristen (siehe Webseite)";
  }
  return f.fristen.length
    ? "Frist: " +
        f.fristen.map((d) => new Date(d).toLocaleDateString("de-DE")).join(", ")
    : "Fristen siehe Webseite";
}
