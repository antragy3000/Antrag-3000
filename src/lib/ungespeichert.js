// Ungespeicherte Änderungen in Formularen mit „Speichern"-Knopf
// (Sammel-Formular, Kostenplan, Stammdaten). Die Formulare melden hier, ob
// sie gerade etwas Ungespeichertes enthalten; vor einem Wechsel des Reiters
// oder Projekts fragt die Seite nach, statt die Eingaben still zu verwerfen.

/** @type {Map<string, string>} Formular-Schlüssel → Beschriftung für die Frage */
const offen = new Map();

/**
 * Ein Formular meldet seinen Stand. Beim Schließen des Formulars mit
 * `aktiv = false` abmelden.
 * @param {string} schluessel z. B. "kostenplan"
 * @param {string} beschriftung z. B. "im Kostenplan"
 * @param {boolean} aktiv gibt es ungespeicherte Änderungen?
 */
export function ungespeichertMelden(schluessel, beschriftung, aktiv) {
  if (aktiv) offen.set(schluessel, beschriftung);
  else offen.delete(schluessel);
}

/**
 * Darf die Ansicht gewechselt werden? Ohne offene Änderungen sofort ja,
 * sonst entscheidet die Nutzerin/der Nutzer. Bei „OK" gelten die
 * Änderungen als verworfen.
 * @returns {boolean}
 */
export function verlassenErlaubt() {
  if (offen.size === 0) return true;
  const wo = [...offen.values()].join(" und ");
  const ok = confirm(
    `Du hast ungespeicherte Änderungen ${wo}.\n\n` +
      `OK: ohne Speichern weiter – die Änderungen gehen verloren.\n` +
      `Abbrechen: hierbleiben und zuerst „Speichern" drücken.`
  );
  if (ok) offen.clear();
  return ok;
}
