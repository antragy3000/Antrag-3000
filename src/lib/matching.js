// ============================================================
// Matching-Logik (CLAUDE.md):
// - HARTE Kriterien: Wohnsitz, Durchführungsort, Trägerschaft
//   (inkl. studentisch). Nichterfüllung => "Weitere Vorschläge"
//   mit Begründung, was nicht passt.
// - WEICHE Kriterien: Sparte, Projektart, Budget, Zeitpunkt.
//   Sie ergeben die Rangfolge der passenden Förderungen.
// ============================================================

import { LAENDER, TRAEGERSCHAFT } from "./begriffe";
import { regionName } from "./daten/orte.js";

// Budget-Auswahl des Fragebogens -> Zahlenbereich [von, bis]
const BUDGET_BEREICHE = {
  unter_5000: [0, 5000],
  von_5000_bis_15000: [5000, 15000],
  von_15000_bis_50000: [15000, 50000],
  ueber_50000: [50000, Number.MAX_SAFE_INTEGER],
};

// Gewichtung der weichen Kriterien (Summe = 100)
const GEWICHTE = { sparte: 40, projektart: 25, budget: 20, zeitpunkt: 15 };

/** Prüft die harten Kriterien. Liefert Liste der Gründe, die NICHT passen. */
function harteGruende(f, a) {
  const h = f.harte_kriterien;
  const gruende = [];

  // Wohnsitz – Land. Stimmt das Land, werden (falls vorhanden UND von der
  // Nutzer:in angegeben) Bundesland/Kanton und Stadt zusätzlich geprüft.
  const wohnsitzLandOk = !h.wohnsitz.length || h.wohnsitz.includes(a.wohnsitz);
  if (!wohnsitzLandOk) {
    gruende.push(
      "Wohnsitz in " + h.wohnsitz.map((l) => LAENDER[l] ?? l).join(" oder ") + " erforderlich"
    );
  } else {
    if (h.wohnsitz_regionen?.length && a.wohnsitzRegion && !h.wohnsitz_regionen.includes(a.wohnsitzRegion)) {
      gruende.push(
        "Wohnsitz in " +
          h.wohnsitz_regionen.map((c) => regionName(a.wohnsitz, c)).join(" oder ") +
          " erforderlich"
      );
    }
    if (h.wohnsitz_staedte?.length && a.wohnsitzStadt && !h.wohnsitz_staedte.includes(a.wohnsitzStadt)) {
      gruende.push("Wohnsitz in " + h.wohnsitz_staedte.join(" oder ") + " erforderlich");
    }
  }

  // Durchführungsort – analog.
  const durchfuehrungLandOk =
    !h.durchfuehrungsort.length || h.durchfuehrungsort.includes(a.durchfuehrungsort);
  if (!durchfuehrungLandOk) {
    gruende.push(
      "Projekt muss in " +
        h.durchfuehrungsort.map((l) => LAENDER[l] ?? l).join(" oder ") +
        " stattfinden"
    );
  } else {
    if (
      h.durchfuehrungsort_regionen?.length &&
      a.durchfuehrungRegion &&
      !h.durchfuehrungsort_regionen.includes(a.durchfuehrungRegion)
    ) {
      gruende.push(
        "Projekt muss in " +
          h.durchfuehrungsort_regionen.map((c) => regionName(a.durchfuehrungsort, c)).join(" oder ") +
          " stattfinden"
      );
    }
    if (
      h.durchfuehrungsort_staedte?.length &&
      a.durchfuehrungStadt &&
      !h.durchfuehrungsort_staedte.includes(a.durchfuehrungStadt)
    ) {
      gruende.push("Projekt muss in " + h.durchfuehrungsort_staedte.join(" oder ") + " stattfinden");
    }
  }
  if (!h.traegerschaft.includes(a.traegerschaft)) {
    gruende.push(
      "Antragstellung nur als " +
        h.traegerschaft.map((t) => TRAEGERSCHAFT[t] ?? t).join(" oder ")
    );
  }
  if (a.studentisch && !h.studentisch_erlaubt) {
    gruende.push("Studierende sind ausgeschlossen");
  }
  return gruende;
}

/** Nächste in der Zukunft liegende Frist (Date) oder null. */
function naechsteFrist(f) {
  const heute = new Date();
  const kommende = f.fristen
    .map((d) => new Date(d))
    .filter((d) => d >= heute)
    .sort((x, y) => x - y);
  return kommende[0] ?? null;
}

/** Bewertet die weichen Kriterien: 0–100 Punkte plus Treffer-Etiketten. */
function weichBewerten(f, a) {
  const w = f.weiche_kriterien;
  let punkte = 0;
  const treffer = [];
  const notizen = [];

  // Sparte: Überschneidung mit den gewählten Sparten.
  // Leere Liste in der Förderung = spartenoffen (zählt, aber etwas
  // weniger als ein echter Spartentreffer).
  if (w.sparten.length === 0) {
    punkte += GEWICHTE.sparte * 0.75;
    treffer.push("spartenoffen");
  } else if (w.sparten.some((s) => a.sparten.includes(s))) {
    punkte += GEWICHTE.sparte;
    treffer.push("Sparte");
  }

  // Projektart: analog.
  if (w.projektarten.length === 0) {
    punkte += GEWICHTE.projektart * 0.75;
  } else if (w.projektarten.some((p) => a.projektarten.includes(p))) {
    punkte += GEWICHTE.projektart;
    treffer.push("Projektart");
  }

  // Budget: Überlappen sich Nutzer-Bereich und Förder-Bereich?
  const [vonNutzer, bisNutzer] = BUDGET_BEREICHE[a.budget];
  const vonF = w.budget_min ?? 0;
  const bisF = w.budget_max ?? Number.MAX_SAFE_INTEGER;
  if (vonNutzer <= bisF && bisNutzer >= vonF) {
    punkte += GEWICHTE.budget;
    treffer.push("Budget");
  } else {
    notizen.push("Budgetrahmen weicht ab");
  }

  // Zeitpunkt: laufende Einreichung passt immer; sonst zählt die
  // nächste kommende Frist im Vergleich zum gewünschten Zeitraum.
  if (w.zeitpunkt === "laufend") {
    punkte += GEWICHTE.zeitpunkt;
    treffer.push("laufend einreichbar");
  } else {
    const frist = naechsteFrist(f);
    if (!frist) {
      notizen.push("keine kommende Frist hinterlegt");
    } else {
      const tage = Math.round((frist - new Date()) / 86400000);
      const passt =
        a.zeitpunkt === "flexibel" ||
        (a.zeitpunkt === "bald" && tage <= 92) ||
        (a.zeitpunkt === "mittel" && tage <= 183) ||
        (a.zeitpunkt === "spaeter" && tage >= 90);
      if (passt) {
        punkte += GEWICHTE.zeitpunkt;
        treffer.push("Zeitpunkt");
      } else {
        punkte += GEWICHTE.zeitpunkt / 2;
        notizen.push("Frist passt nur bedingt zum Zeitplan");
      }
    }
  }

  return { punkte: Math.round(punkte), treffer, notizen };
}

/**
 * Wendet das Matching auf alle Förderungen an.
 * Ergebnis: { passende: [...absteigend sortiert], weitere: [...] }
 */
export function matchen(foerderungen, antworten) {
  const passende = [];
  const weitere = [];

  for (const f of foerderungen) {
    const gruende = harteGruende(f, antworten);
    if (gruende.length) {
      weitere.push({ foerderung: f, gruende });
    } else {
      const { punkte, treffer, notizen } = weichBewerten(f, antworten);
      passende.push({ foerderung: f, punkte, treffer, notizen });
    }
  }

  passende.sort((x, y) => y.punkte - x.punkte);
  return { passende, weitere };
}
