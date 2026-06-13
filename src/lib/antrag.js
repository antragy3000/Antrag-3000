// Felder des Sammel-Formulars und Aufbau des Formular-Words.
//
// Das Word entsteht aus dem Sammel-Formular und landet im PROJEKT-
// Ordner – bewusst OHNE Stammdaten und OHNE Kostenfinanzplan, damit
// diese sensiblen Daten nicht unverschlüsselt in der Datei stehen.
// WAS im Word steht, ist Anwendungslogik (hier im Frontend); Rust
// setzt die fertigen Abschnitte nur noch in eine .docx-Datei um.

// Felder des Sammel-Formulars: [Schlüssel, Beschriftung, Eingabetyp]
// Kosten und Finanzierung haben eine eigene Funktion: den
// Kostenfinanzplan (Bereich "Kostenplan", kfp.js).
export const FORMULAR_FELDER = [
  ["projekttitel", "Projekttitel", "input"],
  ["kurzbeschreibung", "Kurzbeschreibung (2–3 Sätze)", "textarea"],
  ["beschreibung", "Ausführliche Projektbeschreibung", "textarea"],
  ["ziele", "Ziele und Zielgruppe", "textarea"],
  ["zeitraum", "Zeitraum (von – bis)", "input"],
  ["ort", "Durchführungsort(e)", "input"],
  ["beteiligte", "Beteiligte / Mitwirkende / Partner*innen", "textarea"],
];

export function leeresFormular() {
  const f = {};
  for (const [key] of FORMULAR_FELDER) f[key] = "";
  return f;
}

// Baut aus dem Sammel-Formular die Abschnitte für das Word im
// Projektordner. Enthält nur die Projektangaben aus dem Formular –
// keine Stammdaten, kein KFP.
export function formularWordBauen(formular, projektName = "") {
  const heute = new Date().toLocaleDateString("de-DE");

  const warnhinweis =
    `AUTOMATISCH ERZEUGT – Antrag 3000, ${heute}\n` +
    `Verbindliche Quelle ist die App (verschlüsselter Tresor).\n` +
    `Änderungen in dieser Word-Datei werden NICHT in die App übernommen.\n` +
    `Diese Datei enthält weder Stammdaten noch Kostenfinanzplan.`;

  const titel =
    (formular.projekttitel || "").trim() ||
    (projektName ? `Projektbeschrieb – ${projektName}` : "Projektbeschrieb");

  const abschnitte = [];
  for (const [key, beschriftung] of FORMULAR_FELDER) {
    if (key === "projekttitel") continue; // steht schon im Titel
    const wert = (formular[key] || "").trim();
    if (wert) abschnitte.push({ ueberschrift: beschriftung, absaetze: [wert] });
  }

  return { titel, warnhinweis, abschnitte };
}
