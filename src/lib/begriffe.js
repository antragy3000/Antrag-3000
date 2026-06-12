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
  veroeffentlichung: "Veröffentlichung",
  vermittlung: "Vermittlung",
};

export const TRAEGERSCHAFT = {
  einzelperson: "Einzelperson",
  gruppe: "Gruppe / GbR",
  organisation: "Verein / Organisation",
};

/** "Frist: 15.09.2026" | "laufend einreichbar" | Hinweis */
export function fristText(f) {
  if (f.weiche_kriterien.zeitpunkt === "laufend") return "laufend einreichbar";
  return f.fristen.length
    ? "Frist: " +
        f.fristen.map((d) => new Date(d).toLocaleDateString("de-DE")).join(", ")
    : "Fristen siehe Webseite";
}
