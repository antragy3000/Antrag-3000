// ============================================================
// Kostenfinanzplan (KFP) - Datenmodell und Rechenlogik.
//
// Struktur (orientiert an realen Festival-/Projekt-KFPs):
//   kfp = {
//     kosten:       [ { name, posten: [{bezeichnung, erlaeuterung, betrag}] } ]
//     finanzierung: [ { name, posten: [{bezeichnung, betrag}] } ]
//   }
// Das Feld "betrag" ist ein Text und darf eine Rechnung enthalten
// (z. B. "50 × 4 × 5 × 3"); der Wert wird live ausgerechnet.
// Eine Finanzierungs-Position kann frei eingegeben sein
// (bezeichnung) ODER per foerderId mit einer Foerderung aus der
// Merkliste verknuepft sein - dann liefert die Foerder-Datenbank
// den Namen.
// Der Status einer Foerderung gehoert NICHT hierher, sondern zur
// Foerderoption selbst (Antrag-Status, Schritt 8).
// Tresor-Inhalt: Budget ist laut CLAUDE.md sensibel.
// ============================================================

import { foerderungen } from "./katalog.svelte.js";
import { bausteinName } from "./bausteine.js";

/// Anzeigename einer verknuepften Foerderung oder null.
export function foerderLabel(id) {
  const f = foerderungen().find((x) => x.id === id);
  return f ? `${f.name} (${f.foerdergeber})` : null;
}

/// Bezeichnung einer Finanzierungs-Position: bei Verknuepfung der
/// Foerder-Name, sonst der frei eingegebene Text.
export function finanzBezeichnung(p) {
  if (p?.foerderId) {
    return foerderLabel(p.foerderId) ?? p.bezeichnung ?? "(Förderung nicht mehr gemerkt)";
  }
  return p?.bezeichnung ?? "";
}

export function leererKfp() {
  return { kosten: [], finanzierung: [] };
}

/// Stabile ID fuer einen KFP-Posten. Kosten-Posten dienen als
/// "Kostenstelle" im Abrechnungs-Modus; ueber diese ID bleibt die
/// Verknuepfung eines Belegs erhalten, auch wenn der KFP umgebaut wird.
export function neuePostenId() {
  return crypto?.randomUUID
    ? crypto.randomUUID()
    : "ks-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
}

/// Startvorlage mit den ueblichen Kategorien (aus echten KFPs).
export function vorlageKfp() {
  const kat = (name) => ({ name, posten: [] });
  return {
    kosten: [
      kat("Personalkosten"),
      kat("Honorare / Gagen Gäste"),
      kat("Materialkosten"),
      kat("Reisekosten / Unterbringung"),
      kat("Räume / Miete"),
      kat("Presse & Öffentlichkeitsarbeit"),
      kat("Technik"),
      kat("Versicherungen, Rechte & Abgaben (GEMA, KSK …)"),
      kat("Logistik / Transport"),
    ],
    // Finanzierung ist eine flache Förderer-Liste (keine Kategorien) – sie
    // wird über „+ Förderer" aus der Merkliste befüllt, daher leer.
    finanzierung: [],
  };
}

// Rechenregeln liegen in kfpRechnen.js und werden hier weitergereicht.
import {
  betragParsen,
  istFormel,
  formelAuswerten,
  postenBetrag,
  betragFormat,
  kategorieSumme,
  seitenSumme,
  differenz,
} from "./kfpRechnen.js";
export {
  betragParsen,
  istFormel,
  formelAuswerten,
  postenBetrag,
  betragFormat,
  kategorieSumme,
  seitenSumme,
  differenz,
};

/// Maschinenlesbare Fassung fuer antworten.json: mit ausgerechneten
/// Werten, Kategoriesummen und Gesamtsummen. Hat das Projekt Bausteine,
/// traegt jede Ausgabe ihren Baustein (fuer die Excel-Spalte).
export function kfpExport(kfp, bausteine = []) {
  const mitBausteinen = bausteine.length > 0;
  const kosten = kfp.kosten.map((k, i) => ({
    nummer: String(i + 1),
    name: k.name,
    summe: kategorieSumme(k),
    posten: k.posten.map((p, j) => ({
      nummer: `${i + 1}.${j + 1}`,
      bezeichnung: p.bezeichnung,
      erlaeuterung: p.erlaeuterung || "",
      betrag_formel: String(p.betrag ?? ""),
      betrag: postenBetrag(p),
      baustein: mitBausteinen ? bausteinName(p, bausteine) : "",
    })),
  }));
  const finanzierung = kfp.finanzierung.map((k, i) => ({
    nummer: String(i + 1),
    name: k.name,
    summe: kategorieSumme(k),
    posten: k.posten.map((p, j) => ({
      nummer: `${i + 1}.${j + 1}`,
      bezeichnung: finanzBezeichnung(p),
      foerder_id: p.foerderId || null,
      betrag_formel: String(p.betrag ?? ""),
      betrag: postenBetrag(p),
    })),
  }));
  return {
    kosten,
    finanzierung,
    summe_kosten: seitenSumme(kfp.kosten),
    summe_finanzierung: seitenSumme(kfp.finanzierung),
    differenz: differenz(kfp),
    mit_bausteinen: mitBausteinen,
  };
}

/// Baut die Word-Abschnitte (Tabellen) fuer den Antrag.
/// ** am Zellenanfang = fett (Kategorie-/Summenzeilen).
export function kfpAbschnitte(kfp) {
  const abschnitte = [];
  if (!kfp || (kfp.kosten.length === 0 && kfp.finanzierung.length === 0)) {
    return abschnitte;
  }

  if (kfp.kosten.length) {
    const zeilen = [["Ausgaben", "Erläuterung", "Betrag"]];
    kfp.kosten.forEach((k, i) => {
      zeilen.push([`**${i + 1} ${k.name}`, "", "**" + betragFormat(kategorieSumme(k))]);
      k.posten.forEach((p, j) => {
        zeilen.push([
          `${i + 1}.${j + 1}  ${p.bezeichnung}`,
          p.erlaeuterung || "",
          betragFormat(postenBetrag(p)),
        ]);
      });
    });
    zeilen.push(["**Gesamtkosten", "", "**" + betragFormat(seitenSumme(kfp.kosten))]);
    abschnitte.push({ ueberschrift: "Kostenplan", absaetze: [], tabelle: zeilen });
  }

  // Finanzierung = flache Förderer-Liste (keine Kategorie-Zwischenüberschriften).
  const finanzPosten = kfp.finanzierung.flatMap((k) => k.posten ?? []);
  if (finanzPosten.length) {
    const zeilen = [["Finanzierung", "Betrag"]];
    finanzPosten.forEach((p, j) => {
      zeilen.push([`${j + 1}  ${finanzBezeichnung(p)}`, betragFormat(postenBetrag(p))]);
    });
    zeilen.push(["**Gesamtfinanzierung", "**" + betragFormat(seitenSumme(kfp.finanzierung))]);
    abschnitte.push({ ueberschrift: "Finanzierungsplan", absaetze: [], tabelle: zeilen });
  }

  const diff = differenz(kfp);
  const text =
    Math.abs(diff) < 0.005
      ? "Der Kostenfinanzplan ist ausgeglichen."
      : diff < 0
        ? `Fehlbedarf: ${betragFormat(Math.abs(diff))}`
        : `Überschuss: ${betragFormat(diff)}`;
  abschnitte.push({ ueberschrift: "Kosten-/Finanzierungs-Bilanz", absaetze: [text], tabelle: [] });

  return abschnitte;
}

/// Word-Abschnitte FÜR EINEN bestimmten Antrag: Die Finanzierung
/// listet nur die ANDEREN Mittel (alle Positionen, die nicht mit
/// dieser Förderung verknüpft sind). Die bei dieser Förderung zu
/// beantragende Summe (Fehlbetrag) ist der im KFP für GENAU diesen
/// Förderer eingeplante Betrag (Summe der mit ihm verknüpften
/// Finanzierungs-Positionen); ist nichts verknüpft, ersatzweise der
/// Fehlbetrag Gesamtkosten − andere Mittel.
export function kfpAbschnitteFuerAntrag(kfp, foerderungId, foerderName = "") {
  const abschnitte = [];
  if (!kfp || (kfp.kosten.length === 0 && kfp.finanzierung.length === 0)) {
    return abschnitte;
  }

  // 1. Kostenplan (vollständig)
  if (kfp.kosten.length) {
    const zeilen = [["Ausgaben", "Erläuterung", "Betrag"]];
    kfp.kosten.forEach((k, i) => {
      zeilen.push([`**${i + 1} ${k.name}`, "", "**" + betragFormat(kategorieSumme(k))]);
      k.posten.forEach((p, j) => {
        zeilen.push([
          `${i + 1}.${j + 1}  ${p.bezeichnung}`,
          p.erlaeuterung || "",
          betragFormat(postenBetrag(p)),
        ]);
      });
    });
    zeilen.push(["**Gesamtkosten", "", "**" + betragFormat(seitenSumme(kfp.kosten))]);
    abschnitte.push({ ueberschrift: "Kostenplan", absaetze: [], tabelle: zeilen });
  }

  // 2. Andere Mittel (alle Förderer/Quellen außer der aktuellen Förderung),
  //    als flache Liste (keine Kategorien).
  const gesamtKosten = seitenSumme(kfp.kosten);
  const anderePosten = kfp.finanzierung
    .flatMap((k) => k.posten ?? [])
    .filter((p) => (p.foerderId || "") !== foerderungId);
  const summeAndere = anderePosten.reduce((s, p) => s + postenBetrag(p), 0);
  if (anderePosten.length === 0) {
    abschnitte.push({
      ueberschrift: "Bereits vorhandene / beantragte Mittel",
      absaetze: ["Keine weiteren Mittel beantragt."],
      tabelle: [],
    });
  } else {
    const zeilen = [["Finanzierung (andere Mittel)", "Betrag"]];
    for (const p of anderePosten) {
      zeilen.push([finanzBezeichnung(p), betragFormat(postenBetrag(p))]);
    }
    zeilen.push(["**Summe andere Mittel", "**" + betragFormat(summeAndere)]);
    abschnitte.push({
      ueberschrift: "Bereits vorhandene / beantragte Mittel",
      absaetze: [],
      tabelle: zeilen,
    });
  }

  // 3. Beantragte Summe (Fehlbetrag) = der im KFP für DIESE Förderung
  //    eingeplante Betrag (Summe der mit ihr verknüpften Finanzierungs-
  //    Positionen). Ist nichts verknüpft, ersatzweise der rechnerische
  //    Fehlbetrag Gesamtkosten − andere Mittel.
  const eigenePosten = kfp.finanzierung
    .flatMap((k) => k.posten ?? [])
    .filter((p) => (p.foerderId || "") === foerderungId);
  const eingeplant = eigenePosten.reduce((s, p) => s + postenBetrag(p), 0);
  const beantragt = eigenePosten.length
    ? eingeplant
    : Math.max(0, gesamtKosten - summeAndere);
  const label = (foerderName || "").trim() || "Beantragte Förderung";
  abschnitte.push({
    ueberschrift: "Bei dieser Förderung beantragte Summe",
    absaetze: [],
    tabelle: [
      ["Förderer", "Betrag"],
      ["**" + label + " (Fehlbetrag)", "**" + betragFormat(beantragt)],
    ],
  });

  return abschnitte;
}
