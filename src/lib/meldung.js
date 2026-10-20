// ============================================================
// Problem-Meldung zur Förder-Datenbank per E-Mail (Einzelplatz-Modus).
//
// Ohne Team-Synchronisation gibt es (noch) keinen Weg, Meldungen an den
// Server zu senden. Statt sie dauerhaft auf „noch nicht gesendet" stehen
// zu lassen, bereitet die App eine E-Mail vor – der Nutzer sieht sie im
// eigenen Mailprogramm und schickt sie selbst ab. Die App sendet nichts.
// Reine Funktionen, getestet in tools/sicherheit-test.mjs.
// ============================================================

import { sichereMailUrl, kappen } from "./sicherheit.js";

/// Adresse für Meldungen (dieselbe wie im Impressum).
export const MELDE_ADRESSE = "pilot@antrag3000.de";

/// mailto-Link mit vorbereitetem Betreff und Text. Alle Fremdtexte
/// (Förderungs-Name aus dem Katalog, Anmerkung) werden gekappt und
/// vollständig kodiert – sie können keine weiteren mailto-Felder (z. B.
/// bcc) einschleusen.
export function meldungMailUrl({ foerderungId, foerderungName, art, text, stand }) {
  const basis = sichereMailUrl(MELDE_ADRESSE);
  const name = kappen(foerderungName || foerderungId, 200).trim();
  const betreff = `Antrag 3000 – Meldung zur Förder-Datenbank: ${name}`;
  const zeilen = [
    "Guten Tag,",
    "",
    "ich möchte einen Fehler in der Förder-Datenbank melden:",
    "",
    `Förderung: ${name}`,
    `Kennung: ${kappen(foerderungId, 120)}`,
    `Art: ${kappen(art, 80)}`,
  ];
  const anmerkung = kappen(text, 500).trim();
  if (anmerkung) zeilen.push(`Anmerkung: ${anmerkung}`);
  if (stand) zeilen.push(`Datenbank-Stand: ${kappen(stand, 20)}`);
  zeilen.push("", "Viele Grüße");
  return `${basis}?subject=${encodeURIComponent(betreff)}&body=${encodeURIComponent(zeilen.join("\n"))}`;
}
