// ============================================================
// Teil-Anträge: Ein Antrag deckt nur bestimmte Bausteine eines Projekts ab
// (z. B. ein Förderer zahlt nur das Konzert im März).
//
// `antrag.abdeckung`: null = das ganze Projekt (wie bisher), sonst eine
// Liste aus Baustein-ids und ggf. GEMEINSAM für die gemeinsamen Kosten.
// Ohne Bausteine gibt es nur das ganze Projekt.
//
// `anteile` an einer Finanzierungs-Position: { [Baustein-id | GEMEINSAM]:
// Betrag als Text } – wie viel dieser Quelle auf welchen Teil fällt. Leer =
// nicht aufgeteilt. Gemeinsame Kosten werden nie automatisch umgelegt
// (Entscheidung 2): Wer sie in einem Teil-Antrag braucht, wählt sie mit aus.
//
// Wichtigster Punkt: Die beantragte Summe eines Teil-Antrags rechnet nur
// mit den abgedeckten Posten, nie mit den Gesamtkosten. Die Nummern der
// Posten bleiben die des Gesamtplans, weil Belege daran hängen.
//
// Reine Logik ohne Svelte – darum mit `npm run teilantrag` testbar.
// ============================================================

import { GEMEINSAM, bausteinVon } from "./bausteine.js";
import { betragFormat, formelAuswerten, postenBetrag } from "./kfpRechnen.js";

/** @param {any[]} bausteine */
function gueltigeSchluessel(bausteine) {
  return new Set([GEMEINSAM, ...(bausteine ?? []).map((b) => b.id)]);
}

/// Abdeckung eines Antrags: null = ganzes Projekt, sonst die gültigen Teile.
/**
 * @param {any} antrag
 * @param {any[]} bausteine
 * @returns {string[] | null}
 */
export function abdeckungVon(antrag, bausteine) {
  if (!(bausteine ?? []).length || !Array.isArray(antrag?.abdeckung)) return null;
  const gueltig = gueltigeSchluessel(bausteine);
  return antrag.abdeckung.filter((/** @type {string} */ x) => gueltig.has(x));
}

/// Gehört ein Kosten-Posten zu dieser Abdeckung?
/**
 * @param {string[] | null} abdeckung
 * @param {any} posten
 * @param {any[]} bausteine
 */
export function decktPosten(abdeckung, posten, bausteine) {
  return abdeckung === null || abdeckung.includes(bausteinVon(posten, bausteine) || GEMEINSAM);
}

/// Lesbare Beschreibung, z. B. „Konzert März und gemeinsame Kosten".
/**
 * @param {string[] | null} abdeckung
 * @param {any[]} bausteine
 */
export function abdeckungText(abdeckung, bausteine) {
  if (abdeckung === null) return "das ganze Projekt";
  const teile = (bausteine ?? [])
    .filter((b) => abdeckung.includes(b.id))
    .map((b) => b.name);
  if (abdeckung.includes(GEMEINSAM)) teile.push("gemeinsame Kosten");
  if (teile.length === 0) return "noch keinen Teil";
  return teile.length === 1 ? teile[0] : `${teile.slice(0, -1).join(", ")} und ${teile.at(-1)}`;
}

/// Abgedeckte Kosten mit ORIGINAL-Nummern; Kategorien ohne Treffer fehlen.
/// Liefert [{ kategorie, nummer, summe, posten: [{ posten, nummer, betrag }] }].
/**
 * @param {any[]} kosten
 * @param {string[] | null} abdeckung
 * @param {any[]} bausteine
 */
export function teilKosten(kosten, abdeckung, bausteine) {
  const liste = [];
  (kosten ?? []).forEach((kategorie, ki) => {
    const posten = [];
    (kategorie.posten ?? []).forEach((/** @type {any} */ p, pi) => {
      if (decktPosten(abdeckung, p, bausteine)) {
        posten.push({ posten: p, nummer: `${ki + 1}.${pi + 1}`, betrag: postenBetrag(p) });
      }
    });
    if (posten.length) {
      const summe = posten.reduce((s, e) => s + e.betrag, 0);
      liste.push({ kategorie, nummer: String(ki + 1), summe, posten });
    }
  });
  return liste;
}

/// Summe der abgedeckten Kosten.
/**
 * @param {any[]} kosten
 * @param {string[] | null} abdeckung
 * @param {any[]} bausteine
 */
export function teilKostenSumme(kosten, abdeckung, bausteine) {
  return teilKosten(kosten, abdeckung, bausteine).reduce((s, k) => s + k.summe, 0);
}

// ---------------------------------------------------------------- Finanzierung

/// Ist eine Finanzierungs-Position auf Bausteine aufgeteilt?
/**
 * @param {any} p
 * @param {any[]} bausteine
 */
export function istAufgeteilt(p, bausteine) {
  if (!(bausteine ?? []).length) return false;
  const gueltig = gueltigeSchluessel(bausteine);
  return Object.entries(p?.anteile ?? {}).some(
    ([id, v]) => gueltig.has(id) && String(v ?? "").trim() !== ""
  );
}

/// Betrag des Anteils einer Position an einem Teil (Formeln erlaubt).
/**
 * @param {any} p
 * @param {string} id
 */
export function anteilBetrag(p, id) {
  return formelAuswerten(p?.anteile?.[id]).wert;
}

/// Was von einer Position noch keinem Teil zugeordnet ist (0 = genau verteilt,
/// negativ = mehr verteilt als vorhanden).
/**
 * @param {any} p
 * @param {any[]} bausteine
 */
export function aufteilungRest(p, bausteine) {
  let verteilt = 0;
  for (const id of gueltigeSchluessel(bausteine)) verteilt += anteilBetrag(p, id);
  return postenBetrag(p) - verteilt;
}

/// Betrag einer Position für diese Abdeckung; null, wenn sie nicht aufgeteilt
/// ist (dann gilt sie für das ganze Projekt).
/**
 * @param {any} p
 * @param {string[]} abdeckung
 * @param {any[]} bausteine
 */
export function teilFinanzBetrag(p, abdeckung, bausteine) {
  if (!istAufgeteilt(p, bausteine)) return null;
  return abdeckung.reduce((s, id) => s + anteilBetrag(p, id), 0);
}

/// Alle Finanzierungs-Positionen als flache Liste (wie im Kostenplan).
/** @param {any} kfp */
function finanzListe(kfp) {
  return (kfp?.finanzierung ?? []).flatMap((/** @type {any} */ k) => k.posten ?? []);
}

/// Hinweise vor dem PDF eines Teil-Antrags (Entscheidung 3: warnen, nicht
/// blockieren). Liefert { nichtsAbgedeckt, nichtAufgeteilt: [Positionen] }.
/**
 * @param {any} kfp
 * @param {string[] | null} abdeckung
 * @param {any[]} bausteine
 */
export function teilAntragHinweise(kfp, abdeckung, bausteine) {
  if (abdeckung === null) return { nichtsAbgedeckt: false, nichtAufgeteilt: [] };
  return {
    nichtsAbgedeckt: abdeckung.length === 0,
    nichtAufgeteilt: finanzListe(kfp).filter((p) => !istAufgeteilt(p, bausteine)),
  };
}

/// Beantragte Summe eines Teil-Antrags bei diesem Förderer:
/// - Ist der Förderer im Kostenplan eingeplant: sein Anteil an den
///   abgedeckten Teilen (bzw. sein ganzer Betrag, wenn nicht aufgeteilt).
/// - Sonst ersatzweise: abgedeckte Kosten minus die aufgeteilten anderen
///   Mittel dieser Teile – nie die Gesamtkosten.
/**
 * @param {any} kfp
 * @param {string} foerderungId
 * @param {string[]} abdeckung
 * @param {any[]} bausteine
 */
export function teilBeantragt(kfp, foerderungId, abdeckung, bausteine) {
  const alle = finanzListe(kfp);
  const eigene = alle.filter((p) => (p.foerderId || "") === foerderungId);
  if (eigene.length) {
    return eigene.reduce((s, p) => s + (teilFinanzBetrag(p, abdeckung, bausteine) ?? postenBetrag(p)), 0);
  }
  const andere = alle
    .filter((p) => (p.foerderId || "") !== foerderungId)
    .reduce((s, p) => s + (teilFinanzBetrag(p, abdeckung, bausteine) ?? 0), 0);
  return Math.max(0, teilKostenSumme(kfp?.kosten, abdeckung, bausteine) - andere);
}

/// PDF-Abschnitte des Kostenfinanzplans für einen Teil-Antrag. Gegenstück
/// zu kfpAbschnitteFuerAntrag (kfp.js), das für das ganze Projekt bleibt.
/// `bezeichnung(p)` liefert den Namen einer Finanzierungs-Position.
/**
 * @param {any} kfp
 * @param {string} foerderungId
 * @param {string} foerderName
 * @param {string[]} abdeckung
 * @param {any[]} bausteine
 * @param {(p: any) => string} bezeichnung
 */
export function teilAntragAbschnitte(kfp, foerderungId, foerderName, abdeckung, bausteine, bezeichnung) {
  const abschnitte = [];
  const umfang = `Dieser Antrag umfasst ${abdeckungText(abdeckung, bausteine)}.`;

  // 1. Kostenplan: nur die abgedeckten Posten, Nummern wie im Gesamtplan.
  const kosten = teilKosten(kfp?.kosten, abdeckung, bausteine);
  const kostenSumme = kosten.reduce((s, k) => s + k.summe, 0);
  if (kosten.length) {
    const zeilen = [["Ausgaben", "Erläuterung", "Betrag"]];
    for (const k of kosten) {
      zeilen.push([`**${k.nummer} ${k.kategorie.name}`, "", "**" + betragFormat(k.summe)]);
      for (const e of k.posten) {
        zeilen.push([`${e.nummer}  ${e.posten.bezeichnung}`, e.posten.erlaeuterung || "", betragFormat(e.betrag)]);
      }
    }
    zeilen.push(["**Kosten dieses Teils", "", "**" + betragFormat(kostenSumme)]);
    abschnitte.push({
      ueberschrift: "Kostenplan",
      absaetze: [umfang, "Die Nummern entsprechen dem Kostenplan des Gesamtprojekts."],
      tabelle: zeilen,
    });
  } else {
    abschnitte.push({ ueberschrift: "Kostenplan", absaetze: [umfang, "Diesem Teil sind keine Kosten zugeordnet."], tabelle: [] });
  }

  // 2. Andere Mittel: aufgeteilte Quellen mit ihrem Anteil an diesen Teilen.
  //    Nicht aufgeteilte gelten für das ganze Projekt und stehen gesondert.
  const andere = finanzListe(kfp).filter((p) => (p.foerderId || "") !== foerderungId);
  const fuerTeil = [];
  const fuerGanzes = [];
  for (const p of andere) {
    const teil = teilFinanzBetrag(p, abdeckung, bausteine);
    if (teil === null) fuerGanzes.push(p);
    else if (Math.abs(teil) >= 0.005) fuerTeil.push({ p, betrag: teil });
  }
  const absaetze = [];
  let tabelle = [];
  if (fuerTeil.length) {
    tabelle = [["Finanzierung dieses Teils (andere Mittel)", "Betrag"]];
    for (const { p, betrag } of fuerTeil) tabelle.push([bezeichnung(p), betragFormat(betrag)]);
    const summe = fuerTeil.reduce((s, e) => s + e.betrag, 0);
    tabelle.push(["**Summe andere Mittel", "**" + betragFormat(summe)]);
  }
  if (fuerGanzes.length) {
    absaetze.push(
      "Außerdem für das ganze Projekt (nicht auf einzelne Teile aufgeteilt): " +
        fuerGanzes.map((p) => `${bezeichnung(p)} ${betragFormat(postenBetrag(p))}`).join("; ") +
        "."
    );
  }
  if (!fuerTeil.length && !fuerGanzes.length) absaetze.push("Keine weiteren Mittel beantragt.");
  abschnitte.push({ ueberschrift: "Bereits vorhandene / beantragte Mittel", absaetze, tabelle });

  // 3. Beantragte Summe – nur aus den abgedeckten Teilen gerechnet.
  const label = (foerderName || "").trim() || "Beantragte Förderung";
  abschnitte.push({
    ueberschrift: "Bei dieser Förderung beantragte Summe",
    absaetze: [],
    tabelle: [
      ["Förderer", "Betrag"],
      ["**" + label + " (Fehlbetrag)", "**" + betragFormat(teilBeantragt(kfp, foerderungId, abdeckung, bausteine))],
    ],
  });
  return abschnitte;
}

/// Räumt Abdeckungen und Anteile eines Projekts auf: Verweise auf entfernte
/// Deckt mindestens ein Antrag des Projekts nur einzelne Teile ab? Nur dann
/// braucht der Kostenplan „Auf Teile aufteilen" – sonst wäre der Knopf unter
/// jeder Geldquelle nur Ballast. Zählt nur Förderungen auf der Merkliste.
/** @param {any} p */
export function hatTeilAntraege(p) {
  const gemerkt = new Set(p?.merkliste ?? []);
  return Object.entries(p?.antraege ?? {}).some(
    ([id, a]) => gemerkt.has(id) && abdeckungVon(a, p.bausteine) !== null
  );
}

/// Bausteine fallen weg; ohne Bausteine gilt jeder Antrag wieder für das
/// ganze Projekt. Eine leer gewordene Abdeckung bleibt leer (der Antrag
/// meldet dann „noch keinen Teil") und wird nie still zum ganzen Projekt.
/// Liefert true bei Änderung.
/** @param {any} p */
export function abdeckungAufraeumen(p) {
  let veraendert = false;
  const mitBausteinen = (p.bausteine ?? []).length > 0;
  const gueltig = gueltigeSchluessel(p.bausteine);
  for (const a of Object.values(p.antraege ?? {})) {
    if (!a || !Array.isArray(a.abdeckung)) continue;
    if (!mitBausteinen) {
      a.abdeckung = null;
      veraendert = true;
      continue;
    }
    const sauber = a.abdeckung.filter((/** @type {string} */ x) => gueltig.has(x));
    if (sauber.length !== a.abdeckung.length) {
      a.abdeckung = sauber;
      veraendert = true;
    }
  }
  for (const k of p.kfp?.finanzierung ?? []) {
    for (const po of k.posten ?? []) {
      if (!po?.anteile || typeof po.anteile !== "object") continue;
      for (const id of Object.keys(po.anteile)) {
        if (!mitBausteinen || !gueltig.has(id)) {
          delete po.anteile[id];
          veraendert = true;
        }
      }
      if (Object.keys(po.anteile).length === 0) {
        delete po.anteile;
        veraendert = true;
      }
    }
  }
  return veraendert;
}
