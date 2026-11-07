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

import { bausteinNamePruefen } from "./bausteine.js";

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
