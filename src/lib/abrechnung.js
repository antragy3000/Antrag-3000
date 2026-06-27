// ============================================================
// Abrechnungs-Modus – Datenmodell und Helfer (Phase A1: Belege).
//
// Struktur pro Projekt (alles Tresor-Inhalt, hochsensibel – verlaesst das
// Geraet NIE):
//   abrechnung = {
//     belege:  [ { id, nr, datum, empfaenger, zweck, brutto, mwst_satz,
//                  zahlungsart, kostenstelle, status, dateien, zuordnungen,
//                  notiz } ]
//     quellen: [ ... ]   // Geldquellen (Foerderer + Eigenmittel) – Phase A4
//   }
//
// Felder, die erst in spaeteren Phasen befuellt werden, sind hier schon
// angelegt (leer), damit das Datenmodell stabil bleibt:
//   - kostenstelle (Verknuepfung mit einem KFP-Kosten-Posten)  -> Phase A3
//   - dateien      (verschluesselte Beleg-Dateien)             -> Phase A2
//   - zuordnungen  (anteilige Aufteilung auf Geldquellen)      -> Phase A4
//
// Betraege werden wie im KFP als Text gespeichert und beim Rechnen geparst
// (deutsche Schreibweise, z. B. "1.234,56"); so kann man tippen wie gewohnt.
// ============================================================

import { betragParsen, betragFormat } from "./kfp.js";

export { betragFormat };

// Zahlungsarten (Auswahl im Beleg-Formular).
export const ZAHLUNGSARTEN = {
  bar: "Bar",
  karte: "Karte",
  ueberweisung: "Überweisung",
  lastschrift: "Lastschrift",
  sonstige: "Sonstige",
};

// Schlanke Status-Kette eines Belegs (analog zu den uebrigen Status im
// Programm). „zugeordnet"/„abgerechnet" werden in spaeteren Phasen aktiv
// genutzt; in Phase A1 ist alles erst einmal „erfasst".
export const BELEG_STATUS = {
  erfasst: "erfasst",
  zugeordnet: "zugeordnet",
  abgerechnet: "abgerechnet",
};

/// Leere Abrechnungs-Struktur fuer ein neues Projekt.
export function leereAbrechnung() {
  return { belege: [], quellen: [] };
}

/// Naechste freie laufende Beleg-Nummer (fortlaufend je Projekt).
export function naechsteBelegNr(belege) {
  let max = 0;
  for (const b of belege ?? []) {
    const n = Number(b?.nr);
    if (Number.isFinite(n) && n > max) max = n;
  }
  return max + 1;
}

/// Eindeutige Kennung fuer einen neuen Beleg.
function neueId() {
  return crypto?.randomUUID
    ? crypto.randomUUID()
    : "b-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
}

/// Frischer Beleg mit sinnvollen Vorgaben (Datum = heute).
export function neuerBeleg(nr) {
  return {
    id: neueId(),
    nr,
    datum: new Date().toISOString().slice(0, 10),
    empfaenger: "",
    zweck: "",
    brutto: "",
    mwst_satz: "",
    zahlungsart: "",
    kostenstelle: null, // Phase A3
    status: "erfasst",
    dateien: [], // Phase A2
    zuordnungen: [], // Phase A4
    notiz: "",
  };
}

/// Brutto-Betrag eines Belegs als Zahl.
export function belegBrutto(b) {
  return betragParsen(b?.brutto);
}

/// MwSt-Satz in Prozent als Zahl (0, wenn leer/ungueltig).
export function mwstSatz(b) {
  const s = betragParsen(b?.mwst_satz);
  return Number.isFinite(s) && s > 0 ? s : 0;
}

/// Netto = Brutto / (1 + Satz/100). Ohne Satz = Brutto.
export function belegNetto(b) {
  const brutto = belegBrutto(b);
  const satz = mwstSatz(b);
  return satz > 0 ? brutto / (1 + satz / 100) : brutto;
}

/// Enthaltener MwSt-Betrag = Brutto - Netto.
export function belegMwstBetrag(b) {
  return belegBrutto(b) - belegNetto(b);
}

/// Summe aller Belege (brutto).
export function belegeSumme(belege) {
  return (belege ?? []).reduce((s, b) => s + belegBrutto(b), 0);
}

/// Menschlich lesbares Datum ("JJJJ-MM-TT" -> "TT.MM.JJJJ").
export function datumText(iso) {
  const s = String(iso ?? "").trim();
  if (!/^\d{4}-\d{2}-\d{2}$/.test(s)) return s;
  const [j, m, t] = s.split("-");
  return `${t}.${m}.${j}`;
}
