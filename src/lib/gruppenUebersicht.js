// ============================================================
// Gruppen-Übersicht: alle Ausgaben einer Projektgruppe auf einen Blick.
//
// B · Zahlen je Ausgabe: Kosten, Beantragt, Bewilligt, Abgerechnet und die
//     nächste Frist für einen Verwendungsnachweis.
// A · Förderer im Vergleich: je Förderung Status und Betrag in jeder Ausgabe.
//
// Nur Ansicht: Hier wird nichts gespeichert, bearbeitet wird weiter in der
// einzelnen Ausgabe. Gerechnet wird nur aus dem entsperrten Tresor auf
// diesem Gerät; das Team-Board bekommt nichts davon (Entscheidung 4).
//
// Reine Logik ohne Svelte – darum mit `npm run gruppenuebersicht` testbar.
// ============================================================

import { postenBetrag, seitenSumme } from "./kfpRechnen.js";
import { abdeckungVon, teilBeantragt } from "./teilantrag.js";
import { quelleSoll, zugeordnetJeQuelle } from "./abrechnung.js";
import { ANTRAG_STATUS, ANTRAG_STANDARD, statusLabel, statusFarbe } from "./status.js";

/// Ausgaben von alt nach neu. Grundlage ist die Kette über `vorgaengerId`
/// (eine Ausgabe steht hinter ihrer Vorlage); ohne Kette bleibt die
/// Reihenfolge der Projektliste.
/**
 * @param {any[]} projekte
 */
export function ausgabenReihenfolge(projekte) {
  const liste = projekte ?? [];
  const nachId = new Map(liste.map((p) => [p.id, p]));
  /** @type {Map<string, number>} */
  const tiefe = new Map();
  /** @type {(p: any) => number} */
  const tiefeVon = (p) => {
    if (tiefe.has(p.id)) return /** @type {number} */ (tiefe.get(p.id));
    tiefe.set(p.id, 0); // Schutz gegen Ringe
    const vorg = p.vorgaengerId ? nachId.get(p.vorgaengerId) : null;
    const t = vorg ? tiefeVon(vorg) + 1 : 0;
    tiefe.set(p.id, t);
    return t;
  };
  return liste
    .map((p, i) => ({ p, i, t: tiefeVon(p) }))
    .sort((a, b) => a.t - b.t || a.i - b.i)
    .map((x) => x.p);
}

/// Beantragte Summe eines Antrags – so wie im Antrags-PDF:
/// - Teil-Antrag: wie teilBeantragt (nur die abgedeckten Teile).
/// - Ganzes Projekt: der im Kostenplan für diesen Förderer eingeplante
///   Betrag; ist nichts verknüpft, ersatzweise Gesamtkosten minus die
///   übrigen Mittel.
/**
 * @param {any} kfp
 * @param {string} foerderId
 * @param {any} antrag
 * @param {any[]} bausteine
 */
export function antragBeantragt(kfp, foerderId, antrag, bausteine) {
  const abdeckung = abdeckungVon(antrag, bausteine);
  if (abdeckung !== null) return teilBeantragt(kfp, foerderId, abdeckung, bausteine);
  /** @type {any[]} */
  const finanz = (kfp?.finanzierung ?? []).flatMap((/** @type {any} */ k) => k.posten ?? []);
  const eigene = finanz.filter((p) => (p.foerderId || "") === foerderId);
  if (eigene.length) return eigene.reduce((s, p) => s + postenBetrag(p), 0);
  const andere = finanz.reduce((s, p) => s + postenBetrag(p), 0);
  return Math.max(0, seitenSumme(kfp?.kosten ?? []) - andere);
}

/// Status-Schlüssel eines gemerkten Antrags (noch nicht geöffnet = Standard).
/** @param {any} antrag */
function statusVon(antrag) {
  return antrag?.status ?? ANTRAG_STANDARD;
}

/// "JJJJ-MM-TT" des Tages (lokale Zeit).
/** @param {Date} d */
function tagText(d) {
  const z = (/** @type {number} */ n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}`;
}

/// Die Zahlen einer Ausgabe für Tabelle B.
/// - kosten: Summe des Kostenplans
/// - beantragt: alle Anträge auf der Merkliste, auch die noch in Arbeit
/// - bewilligt: nur Anträge mit „Zusage erhalten“; steht der Förderer schon
///   als Geldquelle mit Soll in der Abrechnung, gilt dieser Betrag
/// - abgerechnet: was Belegen zugeordnet ist, nur bei Förderer-Quellen
/// - nachweis: Verwendungsnachweise mit Frist ab heute ({ offen, naechste })
/**
 * @param {any} projekt
 * @param {Date} [heute]
 */
export function ausgabeZahlen(projekt, heute = new Date()) {
  const kfp = projekt?.kfp ?? { kosten: [], finanzierung: [] };
  const bausteine = projekt?.bausteine ?? [];
  const quellen = projekt?.abrechnung?.quellen ?? [];
  let beantragt = 0;
  let bewilligt = 0;
  for (const id of projekt?.merkliste ?? []) {
    const antrag = projekt.antraege?.[id];
    const betrag = antragBeantragt(kfp, id, antrag, bausteine);
    beantragt += betrag;
    if (statusVon(antrag) === "zusage") {
      const quelle = quellen.find((/** @type {any} */ q) => q.typ === "foerderung" && q.foerderId === id && quelleSoll(q) > 0);
      bewilligt += quelle ? quelleSoll(quelle) : betrag;
    }
  }
  const jeQuelle = zugeordnetJeQuelle(projekt?.abrechnung?.belege ?? []);
  const abgerechnet = quellen
    .filter((/** @type {any} */ q) => q.typ === "foerderung")
    .reduce((/** @type {number} */ s, /** @type {any} */ q) => s + (jeQuelle.get(q.id) ?? 0), 0);
  const ab = tagText(heute);
  const fristen = quellen
    .map((/** @type {any} */ q) => q.nachweisFrist || "")
    .filter((/** @type {string} */ d) => /^\d{4}-\d{2}-\d{2}$/.test(d) && d >= ab)
    .sort();
  return {
    kosten: seitenSumme(kfp.kosten ?? []),
    beantragt,
    bewilligt,
    abgerechnet,
    nachweis: { offen: fristen.length, naechste: fristen[0] ?? null },
  };
}

/// Tabelle A: Je Förderung (Zeile) ihr Ergebnis in jeder Ausgabe (Spalte,
/// alt nach neu). Zeilen in der Reihenfolge, in der die Förderungen von
/// der neuesten Ausgabe rückwärts zuerst vorkommen.
/// `nameVon(foerderId, projekt)` liefert den Anzeigenamen.
/**
 * @param {any[]} ausgaben  von alt nach neu
 * @param {(id: string, projekt: any) => string} nameVon
 */
export function foerdererVergleich(ausgaben, nameVon) {
  /** @type {string[]} */
  const ids = [];
  /** @type {Map<string, string>} */
  const namen = new Map();
  for (const p of [...ausgaben].reverse()) {
    for (const id of p.merkliste ?? []) {
      if (!namen.has(id)) {
        ids.push(id);
        namen.set(id, nameVon(id, p) || "Förderung");
      }
    }
  }
  return ids.map((id) => ({
    id,
    name: /** @type {string} */ (namen.get(id)),
    zellen: ausgaben.map((p) => {
      if (!(p.merkliste ?? []).includes(id)) return { beantragt: false, label: "nicht beantragt", farbe: "grau", betrag: 0 };
      const antrag = p.antraege?.[id];
      const key = statusVon(antrag);
      return {
        beantragt: true,
        label: statusLabel(ANTRAG_STATUS, key, antrag?.statusFrei),
        farbe: statusFarbe(ANTRAG_STATUS, key),
        betrag: antragBeantragt(p.kfp, id, antrag, p.bausteine ?? []),
      };
    }),
  }));
}

/// Alles für die Ansicht einer Gruppe. `projekte` = alle Projekte des
/// Tresors (auch archivierte); genommen werden die der Gruppe.
/// Liefert { gruppe, ausgaben (alt nach neu), zahlen (je Ausgabe, gleiche
/// Reihenfolge), summe, vergleich } oder null, wenn die Gruppe leer ist.
/**
 * @param {any} gruppe
 * @param {any[]} projekte
 * @param {{ nameVon: (id: string, projekt: any) => string, heute?: Date }} hilfen
 */
export function gruppenUebersicht(gruppe, projekte, { nameVon, heute = new Date() }) {
  if (!gruppe) return null;
  const ausgaben = ausgabenReihenfolge((projekte ?? []).filter((p) => p.gruppeId === gruppe.id));
  if (!ausgaben.length) return null;
  const zahlen = ausgaben.map((p) => ausgabeZahlen(p, heute));
  const summe = { kosten: 0, beantragt: 0, bewilligt: 0, abgerechnet: 0 };
  for (const z of zahlen) {
    summe.kosten += z.kosten;
    summe.beantragt += z.beantragt;
    summe.bewilligt += z.bewilligt;
    summe.abgerechnet += z.abgerechnet;
  }
  return { gruppe, ausgaben, zahlen, summe, vergleich: foerdererVergleich(ausgaben, nameVon) };
}
