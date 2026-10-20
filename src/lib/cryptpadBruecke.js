// ============================================================
// Brücke Sammel-Formular ↔ CryptPad – über die ZWISCHENABLAGE.
//
// Die App selbst hat dabei keine Netzverbindung: Hinweg = Text mit
// Abschnitts-Markern in die Zwischenablage, der Nutzer fügt ihn in ein
// neues CryptPad-Dokument ein. Rückweg = der Nutzer kopiert den Text aus
// CryptPad zurück; die App liest NUR die markierten Abschnitte und zeigt
// vor dem Übernehmen, was sich ändert (CLAUDE.md: Rück-Sync nur über klar
// markierte Felder, nie über freies Einlesen von Fließtext).
// Reine Funktionen (ohne Svelte) – getestet in tools/sicherheit-test.mjs.
// ============================================================

import { FORMULAR_FELDER } from "./antrag.js";

/// Neues CryptPad-Textdokument (Rich Text). Öffnen legt direkt ein Dokument
/// an (im Browser geprüft: …/pad/#/2/pad/edit/<Schlüssel>/).
export const CRYPTPAD_NEU = "https://cryptpad.fr/pad/";

/// Obergrenze je Feld beim Einlesen (gegen versehentlich riesige Einfügungen).
export const FELD_MAX = 100_000;

const MARKER = /^\s*===\s*(.+?)\s*===\s*$/;

function eigenTitel(feld) {
  return "Eigenes Feld: " + (feld.beschriftung ?? "").trim();
}

/// Steuerzeichen entfernen, Zeilenumbrüche und Tabs behalten; geschützte
/// Leerzeichen (kommen beim Kopieren aus Rich Text vor) zu normalen.
function saeubern(s) {
  return s
    // eslint-disable-next-line no-control-regex
    .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g, "")
    .replace(/ /g, " ")
    .slice(0, FELD_MAX);
}

/// Vergleich ohne Rücksicht auf Leerraum: Rich Text in CryptPad macht aus
/// einfachen Zeilenumbrüchen oft Absätze. Ist nur der Leerraum anders, gilt
/// das Feld als unverändert (die bisherige Formatierung bleibt).
function gleichBisAufLeerraum(a, b) {
  const n = (s) => s.replace(/\s+/g, " ").trim();
  return n(a) === n(b);
}

/// Baut den Text für CryptPad: Kopfzeilen + je Feld ein Marker und der Inhalt.
export function formularAlsText(formular, projektName = "") {
  const zeilen = [
    "Antrag 3000 · Sammel-Formular" + (projektName ? " · Projekt: " + projektName : ""),
    "Hinweis: Die Zeilen mit === … === bitte nicht ändern oder löschen –",
    "an ihnen erkennt Antrag 3000 beim Zurückholen, welcher Text in welches Feld gehört.",
    "",
  ];
  for (const [key, titel] of FORMULAR_FELDER) {
    zeilen.push(`=== ${titel} ===`, (formular?.[key] ?? "").toString(), "");
  }
  for (const feld of formular?._eigeneFelder ?? []) {
    if (!(feld.beschriftung ?? "").trim()) continue;
    zeilen.push(`=== ${eigenTitel(feld)} ===`, (feld.wert ?? "").toString(), "");
  }
  return zeilen.join("\n");
}

/// Liest einen aus CryptPad zurückkopierten Text. Gibt zurück:
///  aenderungen: [{ ziel, titel, alt, neu }]  (nur Felder mit anderem Inhalt)
///  unveraendert: Anzahl gefundener Felder ohne Änderung
///  fehlend: Feld-Titel ohne Marker im Text (bleiben wie bisher)
///  unbekannt: Marker im Text, zu denen es kein Feld gibt (werden ignoriert)
///  doppelt: Marker, die mehrfach vorkommen (nur das erste Vorkommen zählt)
/// `ziel` ist { key } für Standardfelder oder { eigenId } für eigene Felder.
export function textEinlesen(text, formular) {
  /** @type {Map<string, string>} */
  const abschnitte = new Map();
  /** @type {string[]} */
  const doppelt = [];
  /** @type {string | null} */
  let aktuell = null;
  /** @type {string[]} */
  let puffer = [];
  const abschliessen = () => {
    if (aktuell === null) return;
    if (abschnitte.has(aktuell)) doppelt.push(aktuell);
    else abschnitte.set(aktuell, puffer.join("\n").trim());
  };
  for (const zeile of (text ?? "").toString().replace(/\r\n?/g, "\n").split("\n")) {
    const m = MARKER.exec(zeile);
    if (m) {
      abschliessen();
      aktuell = m[1];
      puffer = [];
    } else if (aktuell !== null) {
      puffer.push(zeile);
    }
  }
  abschliessen();

  // Welche Felder gibt es? Titel → Ziel + aktueller Wert + Eingabetyp.
  const felder = new Map();
  for (const [key, titel, typ] of FORMULAR_FELDER) {
    felder.set(titel, { ziel: { key }, alt: (formular?.[key] ?? "").toString(), typ });
  }
  for (const feld of formular?._eigeneFelder ?? []) {
    const t = eigenTitel(feld);
    if (!(feld.beschriftung ?? "").trim() || felder.has(t)) continue;
    felder.set(t, { ziel: { eigenId: feld.id }, alt: (feld.wert ?? "").toString(), typ: feld.typ });
  }

  const aenderungen = [];
  const fehlend = [];
  let unveraendert = 0;
  for (const [titel, f] of felder) {
    if (!abschnitte.has(titel)) {
      fehlend.push(titel);
      continue;
    }
    let neu = saeubern(abschnitte.get(titel));
    // Einzeilige Felder: Zeilenumbrüche zu Leerzeichen.
    if (f.typ === "input") neu = neu.replace(/\s*\n\s*/g, " ");
    if (gleichBisAufLeerraum(neu, f.alt)) unveraendert++;
    else aenderungen.push({ ziel: f.ziel, titel, alt: f.alt, neu });
  }
  const unbekannt = [...abschnitte.keys()].filter((t) => !felder.has(t));
  return { aenderungen, unveraendert, fehlend, unbekannt, doppelt };
}

/// Übernimmt die (bestätigten) Änderungen in die Formular-Arbeitskopie.
export function aenderungenAnwenden(formular, aenderungen) {
  for (const a of aenderungen) {
    if (a.ziel.key) formular[a.ziel.key] = a.neu;
    else {
      const feld = (formular._eigeneFelder ?? []).find((x) => x.id === a.ziel.eigenId);
      if (feld) feld.wert = a.neu;
    }
  }
}
