// ============================================================
// Kostenfinanzplan (KFP) - Datenmodell und Rechenlogik.
//
// Struktur (orientiert an realen Festival-/Projekt-KFPs):
//   kfp = {
//     kosten:       [ { name, posten: [{bezeichnung, erlaeuterung, betrag}] } ]
//     finanzierung: [ { name, posten: [{bezeichnung, status, betrag}] } ]
//   }
// Betraege sind Zahlen (Euro/Franken). Tresor-Inhalt: Budget ist
// laut CLAUDE.md sensibel.
// ============================================================

export const FINANZIERUNGS_STATUS = ["geplant", "beantragt", "zugesagt", "abgesagt"];

export function leererKfp() {
  return { kosten: [], finanzierung: [] };
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
    finanzierung: [
      kat("Öffentliche Mittel"),
      kat("Stiftungen und Sponsoren"),
      kat("Eigenmittel / Einnahmen"),
    ],
  };
}

/// "1.234,56" | "1234,56" | "1234.56" -> Zahl. Ungueltiges -> 0.
export function betragParsen(text) {
  if (typeof text === "number") return text;
  let s = String(text ?? "").trim().replace(/[€\s]/g, "");
  if (s === "") return 0;
  if (s.includes(",")) s = s.replace(/\./g, "").replace(",", ".");
  const zahl = Number(s);
  return Number.isFinite(zahl) ? zahl : 0;
}

const FORMAT = new Intl.NumberFormat("de-DE", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export function betragFormat(zahl) {
  return FORMAT.format(zahl) + " €";
}

export function kategorieSumme(kategorie) {
  return kategorie.posten.reduce((s, p) => s + betragParsen(p.betrag), 0);
}

export function seitenSumme(kategorien) {
  return kategorien.reduce((s, k) => s + kategorieSumme(k), 0);
}

/// Fehlbedarf (< 0) bzw. Ueberschuss (> 0): Finanzierung - Kosten.
export function differenz(kfp) {
  return seitenSumme(kfp.finanzierung) - seitenSumme(kfp.kosten);
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
      zeilen.push([`**${i + 1}. ${k.name}`, "", "**" + betragFormat(kategorieSumme(k))]);
      for (const p of k.posten) {
        zeilen.push([p.bezeichnung, p.erlaeuterung || "", betragFormat(betragParsen(p.betrag))]);
      }
    });
    zeilen.push(["**Gesamtkosten", "", "**" + betragFormat(seitenSumme(kfp.kosten))]);
    abschnitte.push({ ueberschrift: "Kostenplan", absaetze: [], tabelle: zeilen });
  }

  if (kfp.finanzierung.length) {
    const zeilen = [["Finanzierung", "Status", "Betrag"]];
    kfp.finanzierung.forEach((k, i) => {
      zeilen.push([`**${i + 1}. ${k.name}`, "", "**" + betragFormat(kategorieSumme(k))]);
      for (const p of k.posten) {
        zeilen.push([p.bezeichnung, p.status || "", betragFormat(betragParsen(p.betrag))]);
      }
    });
    zeilen.push(["**Gesamtfinanzierung", "", "**" + betragFormat(seitenSumme(kfp.finanzierung))]);
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
