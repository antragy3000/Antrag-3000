// ============================================================
// Teil-Wahl: Welcher Teil einer Ausgabe gerade im Blick ist.
//
// Im Projektmenü lassen sich die Teile (Bausteine) einer Ausgabe einzeln
// wählen. Das ist nur ein Blickwinkel auf dem Bildschirm: Gespeichert wird
// weiter alles in der Ausgabe, und die Wahl selbst kommt nicht in den
// Tresor. Die Wahl merkt sich darum auch ihr Projekt – eine neue Ausgabe
// übernimmt die Teile samt Kennung, und ein Teil der alten Ausgabe soll
// nicht plötzlich in der neuen gewählt sein.
//
// Reine Logik ohne Svelte – darum mit `npm run teilwahl` testbar.
// ============================================================

import { bausteinNamePruefen, bausteinVon } from "./bausteine.js";
import { abdeckungVon } from "./teilantrag.js";
import { belegBaustein } from "./abrechnung.js";

/// Der gewählte Teil des Projekts, oder null (= ganze Ausgabe). Null auch,
/// wenn die Wahl zu einem anderen Projekt gehört oder der Teil inzwischen
/// gelöscht ist.
/**
 * @param {{ projektId: string, teilId: string } | null} wahl
 * @param {any} projekt
 */
export function teilImBlick(wahl, projekt) {
  if (!wahl || !projekt || projekt.archiviert || wahl.projektId !== projekt.id) return null;
  return (projekt.bausteine ?? []).find((/** @type {any} */ b) => b.id === wahl.teilId) ?? null;
}

/// Neue Kennung für einen Teil (gleiches Muster wie die Posten im Kostenplan).
export function neueTeilId() {
  return globalThis.crypto?.randomUUID
    ? globalThis.crypto.randomUUID()
    : "teil-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
}

/// Legt im Projekt einen neuen Teil an (direkt aus dem Projektmenü).
/// Liefert { ok, fehler, teil }; bei leerem Namen ohne Meldung.
/**
 * @param {any} projekt
 * @param {any} eingabe
 * @param {() => string} [neueId]
 */
export function teilAnlegen(projekt, eingabe, neueId = neueTeilId) {
  const pruefung = bausteinNamePruefen(eingabe, projekt?.bausteine);
  if (!pruefung.ok) return { ok: false, fehler: pruefung.fehler, teil: null };
  if (!Array.isArray(projekt.bausteine)) projekt.bausteine = [];
  const teil = { id: neueId(), name: pruefung.name };
  projekt.bausteine.push(teil);
  return { ok: true, fehler: "", teil };
}

// ------------------------------------------------------------
// Was ein gewählter Teil in Merkliste und Abrechnung zeigt. Alles
// liefert neben der gefilterten Liste die Zahl der ausgeblendeten
// Einträge, damit die Ansicht sagen kann, dass es mehr gibt.
// ------------------------------------------------------------

/// Deckt der Antrag den Teil ab? Ein Antrag über das ganze Projekt (oder
/// ein noch nicht angelegter Antrag) deckt jeden Teil ab.
/**
 * @param {any} antrag
 * @param {string} teilId
 * @param {any[]} bausteine
 */
export function decktTeil(antrag, teilId, bausteine) {
  const abdeckung = abdeckungVon(antrag, bausteine);
  return abdeckung === null || abdeckung.includes(teilId);
}

/// Gemerkte Förderungen, deren Antrag den Teil abdeckt.
/**
 * @param {string[]} merkliste
 * @param {Record<string, any>} antraege
 * @param {any} teil
 * @param {any[]} bausteine
 */
export function merklisteFuerTeil(merkliste, antraege, teil, bausteine) {
  const ids = (merkliste ?? []).filter((id) => !teil || decktTeil(antraege?.[id], teil.id, bausteine));
  return { ids, ausgeblendet: (merkliste ?? []).length - ids.length };
}

/// Belege, deren Kostenstelle zum Teil gehört. Gemeinsame Belege und
/// Belege ohne Kostenstelle stehen nur in der ganzen Ausgabe.
/**
 * @param {any[]} belege
 * @param {any} teil
 * @param {any} kfp
 * @param {any[]} bausteine
 */
export function belegeFuerTeil(belege, teil, kfp, bausteine) {
  const liste = (belege ?? []).filter((b) => !teil || belegBaustein(b, kfp, bausteine) === teil.id);
  return { belege: liste, ausgeblendet: (belege ?? []).length - liste.length };
}

/// Kennungen der Kostenstellen (Kosten-Posten) eines Teils.
/**
 * @param {any} teil
 * @param {any} kfp
 * @param {any[]} bausteine
 */
export function kostenstellenVonTeil(teil, kfp, bausteine) {
  const ids = new Set();
  for (const k of kfp?.kosten ?? []) {
    for (const p of k.posten ?? []) if (p.id && teil && bausteinVon(p, bausteine) === teil.id) ids.add(p.id);
  }
  return ids;
}
