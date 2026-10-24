// Verbindungsprotokoll (Paket 6): führt Buch über JEDE Verbindung der App ins
// Internet – Zeit, Zweck, Server, was mitgeschickt wurde, Auslöser, Ergebnis.
// Inhalte stehen hier nie, nur die Klartext-Beschreibung aus netzBefehle.js.
//
// Die Liste liegt nur im Arbeitsspeicher (diese Sitzung) und wird nicht
// gespeichert: Der Team-Abgleich läuft alle 10 Sekunden; ihn in den Tresor zu
// schreiben, wäre unnötige Last – und das Protokoll soll ja gerade zeigen,
// was seit dem Start passiert ist.
import { invoke } from "@tauri-apps/api/core";
import { NETZ_BEFEHLE, zielAusArgs, zielName } from "./netzBefehle.js";

const MAX_EINTRAEGE = 300;
// Gleiche Verbindungen (Befehl, Server, Auslöser, Ergebnis) unter den letzten
// so vielen Einträgen werden zusammengefasst („90× seit 10:02") – sonst würde
// der 10-Sekunden-Takt des Team-Abgleichs die Liste fluten.
const ZUSAMMENFASSEN_IN = 10;

/** Zeitpunkt, ab dem das Protokoll zählt (Start der App). */
export const sitzungsStart = new Date().toISOString();

/**
 * Neueste zuerst.
 * @type {{ schluessel: string, befehl: string, zweck: string, mit: string, ziel: string,
 *          automatisch: boolean, ok: boolean, von: string, bis: string, anzahl: number }[]}
 */
export const verbindungen = $state([]);

/**
 * @param {string} befehl
 * @param {string} ziel
 * @param {boolean} automatisch
 * @param {boolean} ok
 * @param {string} von
 */
function vermerken(befehl, ziel, automatisch, ok, von) {
  const bis = new Date().toISOString();
  const host = zielName(ziel);
  const schluessel = [befehl, host, automatisch, ok].join("|");
  const i = verbindungen.slice(0, ZUSAMMENFASSEN_IN).findIndex((v) => v.schluessel === schluessel);
  if (i >= 0) {
    const [v] = verbindungen.splice(i, 1);
    v.anzahl += 1;
    v.bis = bis;
    verbindungen.unshift(v);
    return;
  }
  const { zweck, mit } = NETZ_BEFEHLE[befehl];
  verbindungen.unshift({ schluessel, befehl, zweck, mit, ziel: host, automatisch, ok, von, bis, anzahl: 1 });
  if (verbindungen.length > MAX_EINTRAEGE) verbindungen.splice(MAX_EINTRAEGE);
}

/**
 * Führt eine Netz-Aktion aus und trägt sie ins Protokoll ein (auch, wenn sie
 * scheitert). Für Wege ohne eigenen Rust-Befehl, z. B. den Update-Prüfer.
 * @template T
 * @param {string} befehl  Schlüssel aus NETZ_BEFEHLE
 * @param {() => Promise<T>} ausfuehren
 * @param {{ ziel?: string, automatisch?: boolean }} [optionen]
 * @returns {Promise<T>}
 */
export async function netzMit(befehl, ausfuehren, { ziel = "", automatisch = false } = {}) {
  if (!NETZ_BEFEHLE[befehl]) {
    throw new Error(`Unbekannter Netz-Befehl „${befehl}" – zuerst in netzBefehle.js eintragen.`);
  }
  const von = new Date().toISOString();
  // Nur ein echtes true gilt als „automatisch" (Klick-Handler übergeben
  // sonst womöglich ein Ereignis-Objekt).
  const auto = automatisch === true;
  try {
    const ergebnis = await ausfuehren();
    vermerken(befehl, ziel, auto, true, von);
    return ergebnis;
  } catch (e) {
    vermerken(befehl, ziel, auto, false, von);
    throw e;
  }
}

/**
 * Ruft einen Rust-Netzbefehl auf – der einzige erlaubte Weg dafür (siehe
 * npm run sicherheit). Der Server wird aus den Argumenten abgelesen.
 * @param {string} befehl
 * @param {Record<string, unknown>} [args]
 * @param {{ automatisch?: boolean }} [optionen]
 * @returns {Promise<any>}
 */
export function netzAufruf(befehl, args = {}, optionen = {}) {
  return netzMit(befehl, () => invoke(befehl, args), { ziel: zielAusArgs(args), ...optionen });
}
