// ============================================================
// Ausgaben: Ein wiederkehrendes Projekt (Festival 2026, 2027 …) übernimmt die
// Arbeit der vorigen Ausgabe, damit niemand alles neu eintippen muss.
//
// Die neue Ausgabe ist ein normales Projekt mit eigenem Ordner und eigener
// Abrechnung. `vorgaengerId` verweist nur zurück, damit die Merkliste das
// Ergebnis der vorigen Ausgabe zeigen kann. Die vorige Ausgabe bleibt unverändert.
//
// Nie übernommen: Belege, Geldquellen, Sachbericht, hochgeladene Dateien,
// eigene und interne Fristen, Links zu Arbeitsdokumenten. Darum kopiert diese
// Datei nur ausdrücklich genannte Felder und nie das ganze Projekt: Ein später
// neu hinzukommendes Feld bleibt so von selbst draußen.
//
// Reine Logik ohne Svelte – darum mit `npm run ausgaben` testbar.
// ============================================================

import { projektNamePruefen } from "./projektname.js";
import { ANTRAG_STATUS, ANTRAG_STANDARD, CHECK_STANDARD, statusLabel, statusFarbe } from "./status.js";
import { abdeckungAufraeumen } from "./teilantrag.js";

/// Tiefe Kopie. JSON statt structuredClone, weil die App mit $state-Proxys
/// arbeitet, die structuredClone ablehnt. Der Tresor ist ohnehin reines JSON.
/** @param {any} wert */
function kopie(wert) {
  return JSON.parse(JSON.stringify(wert));
}

const JAHR = /(19|20)\d{2}(?!.*(19|20)\d{2})/;

/// Schlägt einen freien Namen vor: Die letzte Jahreszahl zählt hoch („Festival
/// 2026" -> „Festival 2027"). Ohne Jahreszahl zählt eine Nummer am Ende hoch
/// oder kommt neu dazu („Workshop" -> „Workshop 2").
/**
 * @param {any} name
 * @param {any[]} projekte
 */
export function ausgabeNameVorschlag(name, projekte) {
  const text = String(name ?? "").trim();
  const jahr = text.match(JAHR);
  /** @type {(n: number) => string} */
  let bauen;
  let zahl;
  if (jahr) {
    const stelle = /** @type {number} */ (jahr.index);
    bauen = (n) => text.slice(0, stelle) + n + text.slice(stelle + 4);
    zahl = Number(jahr[0]) + 1;
  } else {
    const nummer = text.match(/^(.*\S)\s+(\d{1,3})$/);
    const basis = nummer ? nummer[1] : text;
    bauen = (n) => `${basis} ${n}`;
    zahl = nummer ? Number(nummer[2]) + 1 : 2;
  }
  while (!projektNamePruefen(bauen(zahl), projekte).ok && zahl < 10000) zahl++;
  return bauen(zahl);
}

/// Name für eine neue Gruppe aus zwei Ausgaben: der Projektname ohne Jahreszahl.
/** @param {any} name */
export function gruppenNameVorschlag(name) {
  const text = String(name ?? "").trim();
  const ohneJahr = text.replace(JAHR, "").replace(/\s+/g, " ").replace(/[\s–-]+$/, "").trim();
  return ohneJahr || text;
}

/// Kostenplan übernehmen. Ohne `betraege` bleibt nur die Struktur. Ohne `merkliste`
/// fallen Finanzierungs-Zeilen weg, die auf eine gemerkte Förderung zeigen, weil
/// es die in der neuen Ausgabe nicht gibt.
/**
 * @param {any} kfp
 * @param {{betraege?: boolean, merkliste?: boolean}} wahl
 */
function kfpUebernehmen(kfp, wahl) {
  const neu = {
    kosten: kopie(kfp?.kosten ?? []),
    finanzierung: kopie(kfp?.finanzierung ?? []),
  };
  if (!wahl.merkliste) {
    for (const k of neu.finanzierung) k.posten = (k.posten ?? []).filter((/** @type {any} */ p) => !p.foerderId);
  }
  if (!wahl.betraege) {
    for (const k of [...neu.kosten, ...neu.finanzierung]) {
      for (const p of k.posten ?? []) {
        p.betrag = "";
        // Die Aufteilung auf Bausteine besteht auch aus Beträgen.
        delete p.anteile;
      }
    }
  }
  return neu;
}

/// Antrag einer gemerkten Förderung neu beginnen: Status von vorn, Fristen wie
/// übergeben. Vom alten Antrag bleiben nur Kontakt und Checklisten-Titel.
/**
 * @param {any} alt
 * @param {any[]} fristen
 */
function antragUebernehmen(alt, fristen) {
  return {
    status: ANTRAG_STANDARD,
    statusFrei: "",
    offizielleFristen: fristen,
    eigeneFristen: [],
    kontakt: kopie(alt.kontakt ?? { ansprechpartner: "", email: "", telefon: "", notiz: "" }),
    // Welche Bausteine der Antrag abdeckt (null = ganzes Projekt).
    abdeckung: kopie(alt.abdeckung ?? null),
    checkliste: (alt.checkliste ?? []).map((/** @type {any} */ p) => ({
      text: p.text,
      status: CHECK_STANDARD,
      statusFrei: "",
      datei: "",
    })),
  };
}

/// Füllt ein frisch angelegtes Projekt aus der vorigen Ausgabe.
/// `fristenFuer(id)` liefert die aktuellen offiziellen Fristen laut Katalog.
/// Eigene und nicht mehr im Katalog geführte Förderungen haben keine aktuelle
/// Quelle: Ihre alten Daten stimmen im neuen Jahr nicht, darum starten sie leer.
/**
 * @param {any} neu
 * @param {any} quelle
 * @param {{texte?: boolean, kostenplan?: boolean, betraege?: boolean, merkliste?: boolean}} wahl
 * @param {(id: string) => any[]} fristenFuer
 */
export function ausgabeBefuellen(neu, quelle, wahl, fristenFuer) {
  neu.vorgaengerId = quelle.id;

  if (wahl.texte) {
    neu.formular = kopie(quelle.formular);
    // Der Zeitraum der vorigen Ausgabe ist für die neue sicher falsch.
    neu.formular.zeitraum = "";
  }

  if (wahl.kostenplan) neu.kfp = kfpUebernehmen(quelle.kfp, wahl);
  // Bausteine gliedern Texte und Kostenplan; ohne beides braucht es sie nicht.
  if (wahl.texte || wahl.kostenplan) {
    neu.bausteine = kopie(quelle.bausteine ?? []);
    for (const b of neu.bausteine) {
      // Wie beim Projekt: Der Zeitraum der vorigen Ausgabe stimmt nicht mehr.
      delete b.zeitraum;
      // Nur mit dem Kostenplan kommen die Teile ohne ihre Texte mit.
      if (!wahl.texte) {
        delete b.beschreibung;
        delete b.ort;
      }
    }
  }

  if (wahl.merkliste) {
    const liste = [...(quelle.merkliste ?? [])];
    const gemerkt = (/** @type {any} */ f) => liste.includes(f.id);
    neu.merkliste = liste;
    neu.eigeneFoerderungen = (quelle.eigeneFoerderungen ?? []).filter(gemerkt).map(kopie);
    neu.katalogGhosts = (quelle.katalogGhosts ?? []).filter(gemerkt).map(kopie);
    const ohneQuelle = new Set();
    for (const f of [...neu.eigeneFoerderungen, ...neu.katalogGhosts]) {
      f.fristen = [];
      ohneQuelle.add(f.id);
    }
    neu.antraege = {};
    for (const id of liste) {
      const alt = quelle.antraege?.[id];
      if (alt) neu.antraege[id] = antragUebernehmen(alt, ohneQuelle.has(id) ? [] : fristenFuer(id));
    }
  }
  // Kamen die Bausteine nicht mit, gelten die Anträge wieder fürs ganze Projekt.
  abdeckungAufraeumen(neu);
  return neu;
}

/// Das Ergebnis einer Förderung in der vorigen Ausgabe, für die Merkliste.
/// Liefert { label, farbe, beantragt }.
/**
 * @param {any} vorige
 * @param {string} foerderId
 */
export function vorigesErgebnis(vorige, foerderId) {
  if (!(vorige?.merkliste ?? []).includes(foerderId)) {
    return { label: "nicht beantragt", farbe: "grau", beantragt: false };
  }
  const a = vorige.antraege?.[foerderId];
  const key = a?.status ?? ANTRAG_STANDARD;
  return {
    label: statusLabel(ANTRAG_STATUS, key, a?.statusFrei),
    farbe: statusFarbe(ANTRAG_STATUS, key),
    beantragt: true,
  };
}
